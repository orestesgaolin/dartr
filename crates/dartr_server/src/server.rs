// Dart source: pkg/analysis_server/lib/src/lsp/lsp_analysis_server.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_states.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_initialize.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_initialized.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_text_document_changes.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_change_workspace_folders.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_workspace_configuration.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_shutdown.dart, handler_exit.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_cancel_request.dart
// Dart source: pkg/analysis_server/lib/src/lsp/progress.dart
// Dart source: pkg/analysis_server/lib/src/lsp/server_capabilities_computer.dart (performDynamicRegistration, _applyRegistrations)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_formatting.dart, handler_format_range.dart, handler_format_on_type.dart (handle, formatFile, formatRange)
// Dart source: pkg/analysis_server/lib/src/scheduler/message_scheduler.dart (queue, cancellation)

//! The LSP server: the main loop, the server states, the document overlays,
//! the analysis of the workspace (parse diagnostics for now) and the
//! notifications for open files.
//!
//! # Threads and the queue
//!
//! A reader thread reads framed messages from stdin and sends them to the
//! main loop. The main loop takes all available messages into a queue, marks
//! cancelled requests (`$/cancelRequest` is handled when it arrives, like
//! the Dart `MessageScheduler`), handles the queue in order, and then runs
//! the pending analysis. Requests that the server sends (configuration,
//! progress tokens, registrations) are matched with their responses by id.
//!
//! # Analysis
//!
//! Until `dartr_driver` provides resolved units, "analysis" parses every
//! Dart file of the analysis roots (in parallel) and publishes the parse
//! diagnostics. Changed files (overlays) are parsed again. Analysis is
//! reported with `$/progress` (token `ANALYZING`) like the Dart server, or
//! `$/analyzerStatus` for clients without `window.workDoneProgress`.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::io::{BufReader, Read};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use dartr_ast_builder::{ParsedUnit, parse_file};
use dartr_cli::DriverSession;
use dartr_cli::provider::{AnalyzedFile, diagnostics_with_reader};
use dartr_cli::server::processed_diagnostics;
use dartr_parser::ExperimentalFlag;
use dartr_project::{AnalysisContextCollection, CollectionOptions, FileKind, non_dart};
use dartr_syntax::LineInfo;
use serde_json::{Value, json};

use crate::args::ServerOptions;
use crate::capabilities::{self, ClientCapabilities, DynamicRegistration};
use crate::client_configuration::LspClientConfiguration;
use crate::features;
use crate::formatting::{self, FormatterOptions};
use crate::mapping::{self, DiagnosticOptions, ErrorOr, ResponseError, codes};
use crate::source_edits::apply_changes;
use crate::transport::{Channel, read_message};
use crate::uri::{UriError, normalize, path_to_uri, uri_to_path};

mod editor;
mod nav;
mod search;

/// The progress token of analysis (Dart `analyzingProgressToken`).
const ANALYZING_TOKEN: &str = "ANALYZING";

/// Dart `LspAnalysisServer` message handler states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Uninitialized,
    Initializing,
    Initialized,
    ShuttingDown,
    /// `FailureStateMessageHandler`: after an inconsistent state error.
    Failed,
}

/// Dart `LspInitializationOptions` (the options that dartr reads).
#[derive(Clone, Debug, Default)]
pub struct InitializationOptions {
    pub only_analyze_projects_with_open_files: bool,
    pub closing_labels: bool,
    pub outline: bool,
    pub flutter_outline: bool,
}

impl InitializationOptions {
    fn parse(v: Option<&Value>) -> Self {
        let flag = |name: &str| v.and_then(|v| v.get(name)).and_then(Value::as_bool) == Some(true);
        InitializationOptions {
            only_analyze_projects_with_open_files: flag("onlyAnalyzeProjectsWithOpenFiles"),
            closing_labels: flag("closingLabels"),
            outline: flag("outline"),
            flutter_outline: flag("flutterOutline"),
        }
    }
}

/// An open document (overlay).
struct Document {
    content: String,
}

/// A parsed file, cached for requests and notifications of open files.
pub struct ParsedFile {
    pub content: String,
    pub unit: ParsedUnit,
}

/// How a Dart file is parsed (Dart `FileState.parseCode`).
struct ParseSettings {
    /// The language version of the package.
    version: (u32, u32),
    experiments: Vec<ExperimentalFlag>,
}

impl ParseSettings {
    /// No context: the current language version, no experiments.
    fn latest() -> ParseSettings {
        let v = dartr_project::experiments::CURRENT_LANGUAGE_VERSION;
        ParseSettings {
            version: (v.major, v.minor),
            experiments: Vec::new(),
        }
    }
}

/// What a request sent to the client is for.
enum Pending {
    Configuration { folders: Vec<String> },
    ProgressCreate,
    Registration,
    Unregistration,
}

/// Dart `_ServerCreatedProgressReporter` for the analysis token.
struct ProgressReporter {
    create_id: i64,
    /// `None` while the `window/workDoneProgress/create` request is open;
    /// then whether `begin` was sent.
    began: Option<bool>,
    end_requested: bool,
}

pub struct Server {
    channel: Channel,
    #[allow(dead_code)]
    options: ServerOptions,
    state: State,
    client: ClientCapabilities,
    init: InitializationOptions,
    workspace_folders: Vec<String>,
    /// The `dart` settings (`workspace/configuration`).
    client_configuration: LspClientConfiguration,
    overlays: HashMap<String, Document>,
    /// Open files (Dart priority files), in open order.
    priority: Vec<String>,
    collection: Option<AnalysisContextCollection>,
    excluded: Vec<String>,
    analyzed: BTreeSet<String>,
    /// The analyzed non-Dart files (options, pubspec, manifest).
    non_dart_analyzed: BTreeSet<String>,
    /// Files to analyze.
    dirty: BTreeSet<String>,
    roots_dirty: bool,
    /// Analysis waits for the first configuration response.
    waiting_for_config: bool,
    files_with_client_diagnostics: HashSet<String>,
    parsed: HashMap<String, Rc<ParsedFile>>,
    /// The analysis drivers of the contexts of [Self::collection] (full
    /// diagnostics; design §4.2 invalidation on file changes).
    session: DriverSession,
    /// The search index of each unit (`textDocument/references`), until a
    /// file changes.
    indexes: HashMap<(usize, String), std::sync::Arc<crate::index::UnitIndex>>,
    /// The files of a search by driver, until a file changes.
    search_scope: Option<std::sync::Arc<search::SearchScope>>,
    /// The owner of each file of a search, until the roots change.
    owned: search::OwnedFiles,
    /// The words of each file of a search, until a file changes.
    search_words: HashMap<String, std::sync::Arc<HashSet<String>>>,
    /// `DARTR_PARSE_ONLY=1`: the parse-only diagnostics (for comparison).
    parse_only: bool,
    next_request_id: i64,
    pending: HashMap<i64, Pending>,
    progress: Option<ProgressReporter>,
    cancelled: HashSet<String>,
    /// Dart `ServerCapabilitiesComputer.currentRegistrations`: the id and
    /// the registration.
    registrations: Vec<(String, DynamicRegistration)>,
    last_registration_id: u64,
    shutdown_called: bool,
}

/// What the main loop does after a message.
enum Flow {
    Continue,
    Exit(i32),
}

/// Runs the server on [input] and [channel] until `exit` or the end of
/// the input. Returns the exit code.
pub fn run(options: ServerOptions, input: Box<dyn Read + Send>, channel: Channel) -> i32 {
    let (tx, rx) = mpsc::channel::<Option<Value>>();
    let log = channel.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(input);
        loop {
            match read_message(&mut reader) {
                Ok(Some(m)) => {
                    log.log_incoming(&m);
                    if tx.send(Some(m)).is_err() {
                        return;
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
            }
        }
    });
    let mut server = Server::new(options, channel);
    let mut queue: VecDeque<Value> = VecDeque::new();
    let mut input_closed = false;
    loop {
        if queue.is_empty() {
            if input_closed {
                return 0;
            }
            match rx.recv() {
                Ok(Some(m)) => server.enqueue(&mut queue, m),
                Ok(None) | Err(_) => input_closed = true,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(Some(m)) => server.enqueue(&mut queue, m),
                Ok(None) => {
                    input_closed = true;
                    break;
                }
                Err(_) => break,
            }
        }
        while let Some(message) = queue.pop_front() {
            if let Flow::Exit(code) = server.handle_message(message) {
                return code;
            }
            // Take new messages after each one, so that cancellations of
            // queued requests are seen.
            while let Ok(Some(m)) = rx.try_recv() {
                server.enqueue(&mut queue, m);
            }
        }
        server.run_analysis();
    }
}

impl Server {
    fn new(options: ServerOptions, channel: Channel) -> Self {
        Server {
            channel,
            options,
            state: State::Uninitialized,
            client: ClientCapabilities::default(),
            init: InitializationOptions::default(),
            workspace_folders: Vec::new(),
            client_configuration: LspClientConfiguration::default(),
            overlays: HashMap::new(),
            priority: Vec::new(),
            collection: None,
            excluded: Vec::new(),
            analyzed: BTreeSet::new(),
            non_dart_analyzed: BTreeSet::new(),
            dirty: BTreeSet::new(),
            roots_dirty: false,
            waiting_for_config: false,
            files_with_client_diagnostics: HashSet::new(),
            parsed: HashMap::new(),
            session: DriverSession::default(),
            indexes: HashMap::new(),
            search_scope: None,
            owned: search::OwnedFiles::default(),
            search_words: HashMap::new(),
            parse_only: std::env::var_os("DARTR_PARSE_ONLY").is_some(),
            next_request_id: 1,
            pending: HashMap::new(),
            progress: None,
            cancelled: HashSet::new(),
            registrations: Vec::new(),
            last_registration_id: 0,
            shutdown_called: false,
        }
    }

    /// Adds [message] to the queue. Cancellations are applied at once to
    /// the queued requests (Dart `MessageScheduler.add`); a cancellation of
    /// a request that is not in the queue (answered already) does nothing.
    fn enqueue(&mut self, queue: &mut VecDeque<Value>, message: Value) {
        if message.get("method").and_then(Value::as_str) == Some("$/cancelRequest") {
            if let Some(id) = message.get("params").and_then(|p| p.get("id")) {
                let queued = queue
                    .iter()
                    .any(|m| m.get("method").is_some() && m.get("id") == Some(id));
                if queued {
                    self.cancelled.insert(id.to_string());
                }
            }
            return;
        }
        queue.push_back(message);
    }

    // ---------------------------------------------------------------------
    // Sending.

    fn send_notification(&self, method: &str, params: Value) {
        self.channel
            .send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn send_request(&mut self, method: &str, params: Value, pending: Pending) -> i64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.pending.insert(id, pending);
        self.channel
            .send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn send_response(&self, id: &Value, result: ErrorOr<Value>) {
        let message = match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e.to_json()}),
        };
        self.channel.send(&message);
    }

    /// Dart `showErrorMessageToUser`.
    fn show_error_message(&self, message: &str) {
        self.send_notification("window/showMessage", json!({"type": 1, "message": message}));
    }

    /// Dart `logErrorToClient`.
    fn log_error(&self, message: &str) {
        self.send_notification("window/logMessage", json!({"type": 1, "message": message}));
    }

    // ---------------------------------------------------------------------
    // Dispatch.

    fn handle_message(&mut self, message: Value) -> Flow {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string);
        let id = message.get("id").cloned();
        match (method, id) {
            (Some(method), Some(id)) => {
                let params = message.get("params").cloned().unwrap_or(Value::Null);
                self.handle_request(&method, &id, params)
            }
            (Some(method), None) => {
                let params = message.get("params").cloned().unwrap_or(Value::Null);
                self.handle_notification(&method, params)
            }
            (None, Some(id)) => {
                self.handle_response(&id, &message);
                Flow::Continue
            }
            (None, None) => Flow::Continue,
        }
    }

    fn handle_request(&mut self, method: &str, id: &Value, params: Value) -> Flow {
        let cancelled = self.cancelled.remove(&id.to_string());
        let result = match (self.state, method) {
            (_, "exit") => {
                // `exit` is a notification; a request is answered first.
                self.send_response(id, Ok(Value::Null));
                return Flow::Exit(if self.shutdown_called { 0 } else { 1 });
            }
            (State::Failed, _) => Err(ResponseError::new(
                codes::INTERNAL_ERROR,
                "An unrecoverable error occurred and the server cannot process messages",
            )),
            (State::ShuttingDown, _) => Err(ResponseError::new(
                codes::INVALID_REQUEST,
                format!("Unable to handle {method} after shutdown request"),
            )),
            (_, "shutdown") => {
                self.state = State::ShuttingDown;
                self.shutdown_called = true;
                Ok(Value::Null)
            }
            (State::Uninitialized, "initialize") => self.initialize(params),
            (State::Uninitialized, _) => Err(ResponseError::new(
                codes::SERVER_NOT_INITIALIZED,
                format!("Unable to handle {method} before client has sent initialize request"),
            )),
            (_, "initialize") => Err(ResponseError::new(
                codes::SERVER_ALREADY_INITIALIZED,
                "Server already initialized",
            )),
            (State::Initializing, _) => Err(ResponseError::new(
                codes::SERVER_NOT_INITIALIZED,
                format!(
                    "Unable to handle {method} before the server is initialized and the client \
                     has sent the initialized notification"
                ),
            )),
            _ if cancelled => Err(ResponseError::new(
                codes::REQUEST_CANCELLED,
                "Request was cancelled",
            )),
            (State::Initialized, _) => self.handle_initialized_request(method, params),
        };
        if let Err(e) = &result {
            if e.code == codes::CLIENT_SERVER_INCONSISTENT_STATE {
                self.enter_failed_state(e);
            }
        }
        self.send_response(id, result);
        Flow::Continue
    }

    fn handle_initialized_request(&mut self, method: &str, params: Value) -> ErrorOr<Value> {
        match method {
            "textDocument/documentSymbol" => {
                if !is_dart_document(&params) {
                    return Ok(json!([]));
                }
                let path = self.path_of_doc(&params)?;
                let file = self.require_parsed(&path, true)?;
                Ok(features::document_symbols(&self.client, &path, &file))
            }
            "textDocument/foldingRange" => {
                let path = self.path_of_doc(&params)?;
                match self.parsed_unit(&path) {
                    Some(file) => Ok(features::folding_ranges(&self.client, &file)),
                    None => Ok(json!([])),
                }
            }
            "textDocument/selectionRange" => {
                if !is_dart_document(&params) {
                    return Ok(Value::Null);
                }
                let path = self.path_of_doc(&params)?;
                let file = self.require_parsed(&path, false)?;
                let positions = params
                    .get("positions")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid_params(method))?;
                features::selection_ranges(&file, positions)
            }
            "textDocument/definition" => self.catching(method, |s| s.definition(&params)),
            "textDocument/typeDefinition" => self.catching(method, |s| s.type_definition(&params)),
            "textDocument/hover" => self.catching(method, |s| s.hover(&params)),
            "textDocument/documentHighlight" => {
                self.catching(method, |s| s.document_highlights(&params))
            }
            "textDocument/references" => self.catching(method, |s| s.references(&params)),
            "textDocument/implementation" => self.catching(method, |s| s.implementation(&params)),
            "textDocument/signatureHelp" => self.catching(method, |s| s.signature_help(&params)),
            "textDocument/semanticTokens/full" => {
                self.catching(method, |s| s.semantic_tokens(&params, false))
            }
            "textDocument/semanticTokens/range" => {
                self.catching(method, |s| s.semantic_tokens(&params, true))
            }
            "textDocument/inlayHint" => self.catching(method, |s| s.inlay_hints(&params)),
            "textDocument/formatting" => self.format_request(&params, FormatKind::Document),
            "textDocument/rangeFormatting" => {
                let range = params
                    .get("range")
                    .cloned()
                    .ok_or_else(|| invalid_params(method))?;
                self.format_request(&params, FormatKind::Range(range))
            }
            "textDocument/onTypeFormatting" => {
                let position = params
                    .get("position")
                    .cloned()
                    .ok_or_else(|| invalid_params(method))?;
                let ch = params
                    .get("ch")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid_params(method))?
                    .to_string();
                match self.format_request(&params, FormatKind::OnType(position, ch)) {
                    Err(e) if e.code == codes::UNHANDLED_ERROR => {
                        // Dart `logException` of the `ArgumentError`.
                        self.log_error(&format!(
                            "{}: Invalid argument(s): {}",
                            e.message,
                            e.data.clone().unwrap_or_default()
                        ));
                        Err(ResponseError::new(e.code, e.message))
                    }
                    result => result,
                }
            }
            // Not in the 3.13.3 server: newer Flutter tools call it to wait
            // for the end of analysis. Messages are handled after the
            // analysis of earlier changes, so analysis is complete here.
            "dart/workspace/analysis/complete" => {
                self.run_analysis();
                Ok(Value::Null)
            }
            _ => Err(ResponseError::new(
                codes::METHOD_NOT_FOUND,
                format!("Unknown method {method}"),
            )),
        }
    }

    fn handle_notification(&mut self, method: &str, params: Value) -> Flow {
        let result: ErrorOr<()> = match (self.state, method) {
            (_, "exit") => return Flow::Exit(if self.shutdown_called { 0 } else { 1 }),
            (State::Initializing, "initialized") => {
                self.initialized();
                Ok(())
            }
            (State::Initialized, "initialized") => Err(ResponseError::new(
                codes::SERVER_ALREADY_INITIALIZED,
                "Server already initialized",
            )),
            // Other states drop notifications (Dart logs them).
            (State::Initialized, _) => self.handle_initialized_notification(method, params),
            (State::Failed, _) => Err(ResponseError::new(
                codes::INTERNAL_ERROR,
                "An unrecoverable error occurred and the server cannot process messages",
            )),
            _ => Ok(()),
        };
        if let Err(e) = result {
            // Dart `sendErrorResponse` for notifications.
            self.show_error_message(&e.message);
            if e.code == codes::CLIENT_SERVER_INCONSISTENT_STATE {
                self.enter_failed_state(&e);
            }
        }
        Flow::Continue
    }

    fn handle_initialized_notification(&mut self, method: &str, params: Value) -> ErrorOr<()> {
        match method {
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            "workspace/didChangeWorkspaceFolders" => {
                if self.init.only_analyze_projects_with_open_files {
                    return Ok(());
                }
                let event = params.get("event").ok_or_else(|| invalid_params(method))?;
                let added = folder_paths(event.get("added"));
                let removed = folder_paths(event.get("removed"));
                self.update_workspace_folders(added, removed);
                Ok(())
            }
            "workspace/didChangeConfiguration" => {
                self.fetch_configuration_and_register();
                Ok(())
            }
            m if m.starts_with("$/") => Ok(()),
            _ => Err(ResponseError::new(
                codes::METHOD_NOT_FOUND,
                format!("Unknown method {method}"),
            )),
        }
    }

    fn handle_response(&mut self, id: &Value, message: &Value) {
        let Some(id) = id.as_i64() else {
            self.show_error_message("Unexpected response with no ID!");
            return;
        };
        let Some(pending) = self.pending.remove(&id) else {
            self.show_error_message(&format!("Response with ID {id} was unexpected"));
            return;
        };
        let error = message.get("error");
        match pending {
            Pending::Configuration { folders } => {
                let result = message.get("result");
                if let Some(list) = result.and_then(Value::as_array) {
                    if list.len() == folders.len() + 1 {
                        let workspace_folder_config: Vec<(String, Value)> =
                            folders.iter().cloned().zip(list.iter().cloned()).collect();
                        let new_global = list.last().cloned().unwrap_or(Value::Null);
                        let old_excluded =
                            excluded_folders(&self.client_configuration.global_value());
                        self.client_configuration
                            .replace(&new_global, &workspace_folder_config);
                        if excluded_folders(&self.client_configuration.global_value())
                            != old_excluded
                        {
                            self.roots_dirty = true;
                        }
                    }
                }
                self.perform_dynamic_registration();
                if self.waiting_for_config {
                    self.waiting_for_config = false;
                    self.roots_dirty = true;
                }
            }
            Pending::ProgressCreate => {
                if let Some(p) = &mut self.progress {
                    if p.create_id == id {
                        let began = error.is_none();
                        if began {
                            self.send_notification(
                                "$/progress",
                                json!({"token": ANALYZING_TOKEN, "value": {"kind": "begin", "title": "Analyzing…"}}),
                            );
                        }
                        let p = self.progress.as_mut().unwrap();
                        p.began = Some(began);
                        if p.end_requested {
                            self.end_progress();
                        }
                    }
                }
            }
            Pending::Unregistration => {}
            Pending::Registration => {
                if let Some(e) = error {
                    let code = e.get("code").cloned().unwrap_or(Value::Null);
                    let msg = e.get("message").and_then(Value::as_str).unwrap_or("");
                    self.log_error(&format!(
                        "Failed to register capabilities with client: ({code}) {msg}"
                    ));
                }
            }
        }
    }

    fn enter_failed_state(&mut self, e: &ResponseError) {
        self.state = State::Failed;
        self.log_error(&format!(
            "An unrecoverable error occurred.\n\n{}\n\n{}\n\n{}",
            e.message,
            e.code,
            e.data.clone().unwrap_or_else(|| "null".into())
        ));
    }

    // ---------------------------------------------------------------------
    // Lifecycle.

    /// Dart `InitializeMessageHandler.handle`.
    fn initialize(&mut self, params: Value) -> ErrorOr<Value> {
        self.client = ClientCapabilities::new(
            params
                .get("capabilities")
                .cloned()
                .unwrap_or_else(|| json!({})),
        );
        self.init = InitializationOptions::parse(params.get("initializationOptions"));
        let mut workspace_paths = Vec::new();
        if !self.init.only_analyze_projects_with_open_files {
            if let Some(folders) = params.get("workspaceFolders").and_then(Value::as_array) {
                for f in folders {
                    if let Some(Ok(path)) = f.get("uri").and_then(Value::as_str).map(uri_to_path) {
                        workspace_paths.push(path);
                    }
                }
            }
            match params.get("rootUri").and_then(Value::as_str) {
                Some(uri) => {
                    if let Ok(path) = uri_to_path(uri) {
                        workspace_paths.push(path);
                    }
                }
                None => {
                    if let Some(path) = params.get("rootPath").and_then(Value::as_str) {
                        workspace_paths.push(path.to_string());
                    }
                }
            }
        }
        // Kept until `initialized` (Dart `InitializingStateMessageHandler`).
        self.workspace_folders = workspace_paths;
        self.state = State::Initializing;
        Ok(json!({
            "capabilities": capabilities::server_capabilities(&self.client, &self.client_configuration),
            "serverInfo": {"name": "dartr LSP Analysis Server", "version": "3.13.3"},
        }))
    }

    /// Dart `InitializedMessageHandler.handle`.
    fn initialized(&mut self) {
        self.state = State::Initialized;
        // Analysis starts after the first configuration response.
        self.waiting_for_config = self.client.configuration();
        let added = std::mem::take(&mut self.workspace_folders);
        if self.init.only_analyze_projects_with_open_files {
            self.fetch_configuration_and_register();
        } else {
            self.update_workspace_folders(added, Vec::new());
        }
        self.roots_dirty = true;
    }

    /// Dart `updateWorkspaceFolders`.
    fn update_workspace_folders(&mut self, added: Vec<String>, removed: Vec<String>) {
        for path in added {
            let path = normalize(&path);
            if !self.workspace_folders.contains(&path) {
                self.workspace_folders.push(path);
            }
        }
        for path in removed {
            let path = normalize(&path);
            self.workspace_folders.retain(|p| *p != path);
        }
        self.fetch_configuration_and_register();
        self.roots_dirty = true;
    }

    /// Dart `fetchClientConfigurationAndPerformDynamicRegistration`. The
    /// registration is sent after the configuration response.
    fn fetch_configuration_and_register(&mut self) {
        if self.client.configuration() {
            let folders = self.workspace_folders.clone();
            let mut items: Vec<Value> = folders
                .iter()
                .map(|f| json!({"scopeUri": path_to_uri(f), "section": "dart"}))
                .collect();
            items.push(json!({"section": "dart"}));
            self.send_request(
                "workspace/configuration",
                json!({"items": items}),
                Pending::Configuration { folders },
            );
        } else {
            self.perform_dynamic_registration();
        }
    }

    /// Dart `ServerCapabilitiesComputer.performDynamicRegistration` and
    /// `_applyRegistrations`: registers the new registrations and
    /// unregisters the ones that are not wanted any more (for example the
    /// formatter after `dart.enableSdkFormatter` changed to `false`). Like
    /// Dart, every computed registration takes a new id.
    fn perform_dynamic_registration(&mut self) {
        let wanted: Vec<(String, DynamicRegistration)> =
            capabilities::dynamic_registrations(&self.client, &self.client_configuration)
                .into_iter()
                .map(|r| {
                    let id = self.last_registration_id.to_string();
                    self.last_registration_id += 1;
                    (id, r)
                })
                .collect();
        let current: Vec<&DynamicRegistration> =
            self.registrations.iter().map(|(_, r)| r).collect();
        let to_add: Vec<(String, DynamicRegistration)> = wanted
            .iter()
            .filter(|(_, r)| !current.contains(&r))
            .cloned()
            .collect();
        let to_remove: Vec<(String, DynamicRegistration)> = self
            .registrations
            .iter()
            .filter(|(_, r)| !wanted.iter().any(|(_, w)| w == r))
            .cloned()
            .collect();
        self.registrations.retain(|r| !to_remove.contains(r));
        self.registrations.extend(to_add.iter().cloned());

        if !to_remove.is_empty() {
            let unregistrations: Vec<Value> = to_remove
                .iter()
                .map(|(id, r)| json!({"id": id, "method": r.method}))
                .collect();
            // The Dart field name has this spelling.
            self.send_request(
                "client/unregisterCapability",
                json!({"unregisterations": unregistrations}),
                Pending::Unregistration,
            );
        }
        // Only send the registration request if there is at least one
        // (otherwise it is not known that the client supports it).
        if !to_add.is_empty() {
            let registrations: Vec<Value> = to_add
                .iter()
                .map(|(id, r)| {
                    let mut reg = serde_json::Map::new();
                    reg.insert("id".into(), json!(id));
                    reg.insert("method".into(), json!(r.method));
                    if let Some(o) = &r.options {
                        reg.insert("registerOptions".into(), o.clone());
                    }
                    Value::Object(reg)
                })
                .collect();
            self.send_request(
                "client/registerCapability",
                json!({"registrations": registrations}),
                Pending::Registration,
            );
        }
    }

    // ---------------------------------------------------------------------
    // Documents.

    /// Runs a request handler; a panic becomes an error response (Dart
    /// reports the exception of a handler to the client).
    fn catching(
        &mut self,
        method: &str,
        f: impl FnOnce(&mut Self) -> ErrorOr<Value>,
    ) -> ErrorOr<Value> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(r) => r,
            Err(e) => {
                let message = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panic".to_string());
                Err(ResponseError::new(
                    codes::UNHANDLED_ERROR,
                    format!("An error occurred while handling {method} request: {message}"),
                ))
            }
        }
    }

    /// Dart `pathOfDoc` for `params.textDocument`.
    fn path_of_doc(&self, params: &Value) -> ErrorOr<String> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str);
        let Some(uri) = uri else {
            return Err(ResponseError::new(
                codes::INVALID_FILE_PATH,
                "Document URI was not supplied",
            ));
        };
        uri_to_path(uri).map_err(|e| match e {
            UriError::NoScheme => ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "URI is not a valid file:// URI",
                uri,
            ),
            UriError::UnsupportedScheme(s) => ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                format!("URI scheme '{s}' is not supported. Allowed schemes are 'file'."),
                uri,
            ),
            UriError::Invalid => ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File URI did not contain a valid file path",
                uri,
            ),
        })
    }

    fn did_open(&mut self, params: Value) -> ErrorOr<()> {
        let path = self.path_of_doc(&params)?;
        let text = params
            .get("textDocument")
            .and_then(|d| d.get("text"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_params("textDocument/didOpen"))?;
        // Dart analyzes a document with a leading byte order mark like the
        // text without it (checked with `dart language-server`: no
        // `illegal_character`, columns without the mark).
        let text = dartr_syntax::strip_bom(text).to_string();
        dartr_project::fs::set_overlay(&path, Some(text.clone()));
        self.overlays
            .insert(path.clone(), Document { content: text });
        self.file_changed(&path);
        if !self.priority.contains(&path) {
            self.priority.push(path.clone());
            // Open files get closing labels and outlines.
            if path.ends_with(".dart") {
                self.dirty.insert(path.clone());
            }
            if self.workspace_folders.is_empty() {
                self.roots_dirty = true;
            }
        }
        Ok(())
    }

    fn did_change(&mut self, params: Value) -> ErrorOr<()> {
        let path = self.path_of_doc(&params)?;
        let Some(doc) = self.overlays.get_mut(&path) else {
            return Err(ResponseError::new(
                codes::CLIENT_SERVER_INCONSISTENT_STATE,
                format!(
                    "Unable to edit document because the file was not previously opened: {path}"
                ),
            ));
        };
        let changes = params
            .get("contentChanges")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_params("textDocument/didChange"))?;
        let content = apply_changes(&doc.content, changes)?;
        doc.content = dartr_syntax::strip_bom(&content).to_string();
        dartr_project::fs::set_overlay(&path, Some(doc.content.clone()));
        self.file_changed(&path);
        // Checked with `dart language-server`: a change of an open
        // `analysis_options.yaml` creates the contexts again and analyzes all
        // non-Dart files again. Changes of an open pubspec or manifest do
        // nothing until the next context rebuild.
        if FileKind::of(&path) == FileKind::AnalysisOptions {
            self.roots_dirty = true;
        }
        Ok(())
    }

    fn did_close(&mut self, params: Value) -> ErrorOr<()> {
        let path = self.path_of_doc(&params)?;
        self.overlays.remove(&path);
        dartr_project::fs::set_overlay(&path, None);
        self.file_changed(&path);
        self.priority.retain(|p| *p != path);
        if self.workspace_folders.is_empty() {
            self.roots_dirty = true;
        }
        Ok(())
    }

    fn file_changed(&mut self, path: &str) {
        self.parsed.remove(path);
        // The search data of the file and of the units of the libraries
        // that the change affects.
        self.indexes.retain(|(_, p), _| p != path);
        self.search_scope = None;
        self.search_words.remove(path);
        if path.ends_with(".dart") && !self.parse_only {
            // Dart `AnalysisDriver.changeFile`: the units of the libraries
            // to analyze again (only the library of the file when only
            // function bodies changed; after an API change the relinked
            // libraries too).
            for affected in self.session.change_file(path) {
                self.indexes.retain(|(_, p), _| *p != affected);
                if self.analyzed.contains(&affected) || self.priority.contains(&affected) {
                    self.dirty.insert(affected);
                }
            }
        }
        if self.analyzed.contains(path)
            || (path.ends_with(".dart") && self.priority.iter().any(|p| p == path))
        {
            self.dirty.insert(path.to_string());
        }
    }

    /// The content of [path]: the overlay, the file, or `None`.
    fn content(&self, path: &str) -> Option<String> {
        match self.overlays.get(path) {
            Some(d) => Some(d.content.clone()),
            None => read_file(path),
        }
    }

    /// Dart `isAnalyzed`.
    fn is_analyzed(&self, path: &str) -> bool {
        self.collection
            .as_ref()
            .is_some_and(|c| c.contexts.iter().any(|x| x.root.is_analyzed(path)))
            && !self.is_excluded(path)
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.excluded
            .iter()
            .any(|e| path == e || path.starts_with(&format!("{e}/")))
    }

    /// The parsed unit of [path] (Dart `getParsedUnit`): `None` if the file
    /// is not a Dart file or there is no analysis context. Like Dart, a
    /// file outside of the analysis roots is parsed too (Dart uses the
    /// first analysis driver), and a missing file has empty content.
    fn parsed_unit(&mut self, path: &str) -> Option<Rc<ParsedFile>> {
        if !path.ends_with(".dart") || self.collection.is_none() {
            return None;
        }
        if let Some(p) = self.parsed.get(path) {
            return Some(p.clone());
        }
        let content = self.content(path).unwrap_or_default();
        let settings = self.parse_settings(path);
        let unit = parse_file(&content, path, settings.version, &settings.experiments);
        let file = Rc::new(ParsedFile { content, unit });
        self.parsed.insert(path.to_string(), file.clone());
        Some(file)
    }

    /// The language version and experiments to parse [path] with: the
    /// package of its context (the first context for a file outside of all
    /// contexts, like Dart's first analysis driver) and the analysis options
    /// of its folder.
    fn parse_settings(&self, path: &str) -> ParseSettings {
        let Some(collection) = &self.collection else {
            return ParseSettings::latest();
        };
        let Some(context) = collection
            .context_for(path)
            .or_else(|| collection.contexts.first())
        else {
            return ParseSettings::latest();
        };
        let version = context.file_info(path).language_version;
        let options = collection.options_for(context, path);
        ParseSettings {
            version: (version.major, version.minor),
            experiments: options
                .enabled_experiments()
                .iter()
                .filter_map(|name| {
                    ExperimentalFlag::VALUES
                        .iter()
                        .copied()
                        .find(|f| f.name() == *name)
                })
                .collect(),
        }
    }

    /// The formatter options of the analysis options of [path] (Dart
    /// `result.analysisOptions.formatterOptions`).
    fn formatter_options(&self, path: &str) -> FormatterOptions {
        let Some(collection) = &self.collection else {
            return FormatterOptions::default();
        };
        let Some(context) = collection
            .context_for(path)
            .or_else(|| collection.contexts.first())
        else {
            return FormatterOptions::default();
        };
        let options = collection.options_for(context, path);
        FormatterOptions {
            page_width: options.formatter_page_width,
            trailing_commas: options.formatter_trailing_commas,
        }
    }

    /// Dart `FormattingHandler.handle` / `formatFile`,
    /// `FormatRangeHandler.handle` / `formatRange` and
    /// `FormatOnTypeHandler.handle` / `formatFile`.
    fn format_request(&mut self, params: &Value, kind: FormatKind) -> ErrorOr<Value> {
        if !is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let config = self.client_configuration.for_resource(&path);
        if !config.enable_sdk_formatter() {
            // Formatting can be disabled for some workspace folders: do
            // nothing for those.
            return Ok(Value::Null);
        }
        let line_length = config.line_length();
        if self.content(&path).is_none() {
            return Err(ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File does not exist",
                path,
            ));
        }
        let Some(file) = self.parsed_unit(&path) else {
            return Ok(Value::Null);
        };
        if !file.unit.diagnostics.is_empty() {
            return Ok(Value::Null);
        }
        let options = self.formatter_options(&path);
        match kind {
            FormatKind::Document => formatting::format_file(&file, &options, line_length, None),
            FormatKind::Range(range) => {
                formatting::format_file(&file, &options, line_length, Some(&range))
            }
            FormatKind::OnType(position, ch) => {
                // The client sends a request for every trigger character,
                // also in comments and strings.
                match formatting::should_trigger_formatting(&file, &position, &ch) {
                    Ok(true) => formatting::format_file(&file, &options, line_length, None),
                    Ok(false) => Ok(Value::Null),
                    Err(message) => Err(ResponseError::with_data(
                        codes::UNHANDLED_ERROR,
                        "An error occurred while handling textDocument/onTypeFormatting request",
                        message,
                    )),
                }
            }
        }
    }

    /// Dart `requireUnresolvedUnit` / `requireResolvedUnit`.
    fn require_parsed(&mut self, path: &str, resolved: bool) -> ErrorOr<Rc<ParsedFile>> {
        match self.parsed_unit(path) {
            Some(file) => {
                if resolved && self.content(path).is_none() {
                    return Err(ResponseError::with_data(
                        codes::INVALID_FILE_PATH,
                        "File does not exist",
                        path,
                    ));
                }
                Ok(file)
            }
            None => Err(ResponseError::with_data(
                codes::FILE_NOT_ANALYZED,
                "File is not being analyzed",
                path,
            )),
        }
    }

    // ---------------------------------------------------------------------
    // Analysis.

    /// Dart `_refreshAnalysisRoots`.
    fn refresh_roots(&mut self) {
        self.roots_dirty = false;
        let included: Vec<String> = if !self.workspace_folders.is_empty() {
            self.workspace_folders.clone()
        } else {
            self.roots_for_open_files()
        };
        self.excluded = excluded_folders(&self.client_configuration.global_value())
            .into_iter()
            .flat_map(|e| {
                if e.starts_with('/') {
                    vec![normalize(&e)]
                } else {
                    self.workspace_folders
                        .iter()
                        .map(|root| normalize(&format!("{root}/{e}")))
                        .collect()
                }
            })
            .collect();
        // New contexts: new drivers.
        self.session = DriverSession::default();
        self.indexes.clear();
        self.search_scope = None;
        self.search_words.clear();
        self.owned = search::OwnedFiles::default();
        self.collection = if included.is_empty() {
            None
        } else {
            Some(AnalysisContextCollection::new(
                &included,
                &CollectionOptions::default(),
            ))
        };
        let mut analyzed = BTreeSet::new();
        let mut non_dart_analyzed = BTreeSet::new();
        if let Some(collection) = &self.collection {
            for context in &collection.contexts {
                for file in context.root.analyzed_files() {
                    if self.is_excluded(&file) {
                        continue;
                    }
                    match FileKind::of(&file) {
                        FileKind::Dart => {
                            analyzed.insert(file);
                        }
                        FileKind::Other => {}
                        _ => {
                            non_dart_analyzed.insert(file);
                        }
                    }
                }
                // `ContextManagerImpl._analyzeFixDataYaml`: the data files of
                // each context root, also when they are excluded.
                non_dart_analyzed.extend(context.fix_data_files());
            }
        }
        // Dart `flushResults` for files that are no longer analyzed.
        let removed: Vec<String> = self.analyzed.difference(&analyzed).cloned().collect();
        for path in removed {
            self.publish_diagnostics(&path, Vec::new());
            self.parsed.remove(&path);
        }
        self.publish_non_dart_diagnostics(non_dart_analyzed);
        // Open files outside of the roots are analyzed too when they are in
        // a context root (for example an open file that is not under a
        // folder, without workspace folders).
        self.dirty = analyzed.clone();
        self.dirty.extend(
            self.priority
                .iter()
                .filter(|p| p.ends_with(".dart"))
                .cloned(),
        );
        self.analyzed = analyzed;
        self.parsed.clear();
    }

    /// Dart `ContextManager._analyzeAnalysisOptionsYaml`,
    /// `_analyzePubspecYaml` and `_analyzeAndroidManifestXml` for all
    /// analyzed non-Dart files, after the contexts were created. Files that
    /// are not analyzed any more get an empty list (Dart `flushResults`).
    fn publish_non_dart_diagnostics(&mut self, files: BTreeSet<String>) {
        let removed: Vec<String> = self.non_dart_analyzed.difference(&files).cloned().collect();
        for path in removed {
            self.publish_diagnostics(&path, Vec::new());
        }
        let mut options = self.client.diagnostic_options();
        // The analysis server converts these diagnostics without `url`.
        options.code_description = false;
        for path in &files {
            let diagnostics = match self
                .collection
                .as_ref()
                .and_then(|c| non_dart::context_for_file(c, path))
            {
                Some(context) => non_dart::diagnostics_for_file(context, path),
                None => Vec::new(),
            };
            let line_info = LineInfo::from_content(
                &dartr_project::fs::read_string_strict(path).unwrap_or_default(),
            );
            let values = diagnostics
                .into_iter()
                .map(|mut d| {
                    // `AnalyzerConverter` uses the line info of the file for
                    // all context messages, with the URL in the text.
                    for m in &mut d.context_messages {
                        m.message = m.message_text(true);
                    }
                    mapping::to_diagnostic(&line_info, path, &d, &options, &|_| {
                        Some(line_info.clone())
                    })
                })
                .collect();
            self.publish_diagnostics(path, values);
        }
        self.non_dart_analyzed = files;
    }

    /// Dart `_getRootsForOpenFiles`: the package folder of each open file
    /// (the nearest folder with a `pubspec.yaml`), or the file.
    fn roots_for_open_files(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for file in &self.priority {
            let mut root = file.clone();
            let mut dir = std::path::Path::new(file).parent();
            while let Some(d) = dir {
                if d.join("pubspec.yaml").is_file() {
                    if d.parent().is_some() {
                        root = d.to_string_lossy().into_owned();
                    }
                    break;
                }
                dir = d.parent();
            }
            if !out.contains(&root) {
                out.push(root);
            }
        }
        out
    }

    /// The diagnostics of Dart files, as LSP diagnostics: the diagnostics of
    /// the analysis driver, AST-only lints, ignore-comment handling and the
    /// `errors:` processors of the analysis options (the same pipeline as
    /// `dartr analyze`, with the content of open documents). Files without
    /// content get an empty list.
    fn dart_diagnostics(
        &mut self,
        paths: &[String],
        options: &DiagnosticOptions,
    ) -> Vec<(String, Vec<Value>)> {
        let Some(collection) = &self.collection else {
            return Vec::new();
        };
        let files: Vec<AnalyzedFile> = paths
            .iter()
            .filter(|p| p.ends_with(".dart"))
            .map(|p| AnalyzedFile {
                path: p.clone(),
                // Like Dart, a file outside of all contexts uses the first
                // context.
                context: collection
                    .context_for(p)
                    .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))
                    .unwrap_or(0),
            })
            .filter(|f| f.context < collection.contexts.len())
            .collect();
        // The server holds `Rc`s: the reader owns a snapshot of the overlays.
        let overlays: HashMap<&str, &str> = self
            .overlays
            .iter()
            .map(|(p, d)| (p.as_str(), d.content.as_str()))
            .collect();
        let reader = |path: &str| match overlays.get(path) {
            Some(content) => Some(content.to_string()),
            None => read_file(path),
        };
        let results = if self.parse_only {
            diagnostics_with_reader(collection, &files, &reader)
        } else {
            // The driver reads open documents through the overlays of
            // `dartr_project::fs`.
            self.session.diagnostics(collection, &files)
        };
        let mut by_path: HashMap<String, Vec<Value>> = HashMap::new();
        for file in &results {
            let values = processed_diagnostics(collection, file)
                .into_iter()
                .map(|(d, severity)| {
                    let mut d = d.clone();
                    d.severity = severity;
                    mapping::to_diagnostic(&file.line_info, &file.path, &d, options, &|f| {
                        read_file(f).map(|c| LineInfo::from_content(&c))
                    })
                })
                .collect();
            by_path.insert(file.path.clone(), values);
        }
        paths
            .iter()
            .map(|p| (p.clone(), by_path.remove(p).unwrap_or_default()))
            .collect()
    }

    /// Analyzes the dirty files: parse diagnostics, then closing labels and
    /// outlines of open files.
    pub fn run_analysis(&mut self) {
        if self.state != State::Initialized || self.waiting_for_config {
            return;
        }
        if self.roots_dirty {
            self.refresh_roots();
        }
        if self.dirty.is_empty() {
            return;
        }
        self.begin_progress();
        let dirty: Vec<String> = std::mem::take(&mut self.dirty).into_iter().collect();
        let options = self.client.diagnostic_options();
        let results: Vec<(String, Vec<Value>)> = self.dart_diagnostics(&dirty, &options);
        for (path, diagnostics) in results {
            // Dart `handleFileResult`: diagnostics only for analyzed files.
            if self.analyzed.contains(&path) {
                self.publish_diagnostics(&path, diagnostics);
            }
            if self.priority.contains(&path) {
                self.publish_open_file_notifications(&path);
            }
        }
        self.end_progress();
    }

    /// Dart `publishDiagnostics`.
    fn publish_diagnostics(&mut self, path: &str, diagnostics: Vec<Value>) {
        if diagnostics.is_empty() && !self.files_with_client_diagnostics.contains(path) {
            return;
        }
        if diagnostics.is_empty() {
            self.files_with_client_diagnostics.remove(path);
        } else {
            self.files_with_client_diagnostics.insert(path.to_string());
        }
        self.send_notification(
            "textDocument/publishDiagnostics",
            json!({"uri": path_to_uri(path), "diagnostics": diagnostics}),
        );
    }

    /// Dart `LspServerContextManagerCallbacks.handleResolvedUnitResult`:
    /// closing labels and the outline of an open file. The Flutter outline
    /// needs resolution and is not sent yet.
    ///
    /// The outline is sent for every open Dart file that has an analysis
    /// context (also excluded files), closing labels only for analyzed files
    /// (Dart `shouldSendClosingLabelsFor`, `shouldSendOutlineFor`). Dart
    /// sends nothing for an excluded open file until its analysis driver
    /// knows the file (an analyzed file imports it, or after the next
    /// change); dartr sends the outline at once.
    fn publish_open_file_notifications(&mut self, path: &str) {
        let Some(file) = self.parsed_unit(path) else {
            return;
        };
        let uri = path_to_uri(path);
        if self.init.closing_labels && self.is_analyzed(path) {
            self.send_notification(
                "dart/textDocument/publishClosingLabels",
                json!({"uri": uri, "labels": features::closing_labels(&file)}),
            );
        }
        if self.init.outline {
            self.send_notification(
                "dart/textDocument/publishOutline",
                json!({"uri": uri, "outline": features::outline(&file)}),
            );
        }
    }

    /// Dart `sendStatusNotification(isAnalyzing: true)`.
    fn begin_progress(&mut self) {
        if !self.client.work_done_progress() {
            self.send_notification("$/analyzerStatus", json!({"isAnalyzing": true}));
            return;
        }
        if let Some(p) = &mut self.progress {
            // Still waiting for the end of the last progress: continue it.
            p.end_requested = false;
            return;
        }
        let create_id = self.send_request(
            "window/workDoneProgress/create",
            json!({"token": ANALYZING_TOKEN}),
            Pending::ProgressCreate,
        );
        self.progress = Some(ProgressReporter {
            create_id,
            began: None,
            end_requested: false,
        });
    }

    /// Dart `sendStatusNotification(isAnalyzing: false)`.
    fn end_progress(&mut self) {
        if !self.client.work_done_progress() {
            self.send_notification("$/analyzerStatus", json!({"isAnalyzing": false}));
            return;
        }
        let Some(p) = &mut self.progress else {
            return;
        };
        match p.began {
            None => p.end_requested = true,
            Some(true) => {
                self.progress = None;
                self.send_notification(
                    "$/progress",
                    json!({"token": ANALYZING_TOKEN, "value": {"kind": "end"}}),
                );
            }
            Some(false) => self.progress = None,
        }
    }
}

/// The kind of a formatting request.
enum FormatKind {
    Document,
    /// `textDocument/rangeFormatting` with its range.
    Range(Value),
    /// `textDocument/onTypeFormatting` with the position and the typed
    /// character.
    OnType(Value, String),
}

/// Reads a file like the analyzer's file system: UTF-8 (malformed bytes
/// replaced) without a leading byte order mark.
fn read_file(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(dartr_syntax::strip_bom(&text).to_string())
}

/// Dart `isDartDocument`: the path of the document URI ends with `.dart`.
fn is_dart_document(params: &Value) -> bool {
    params
        .get("textDocument")
        .and_then(|d| d.get("uri"))
        .and_then(Value::as_str)
        .map(|u| u.split(['?', '#']).next().unwrap_or(u).ends_with(".dart"))
        .unwrap_or(false)
}

fn invalid_params(method: &str) -> ResponseError {
    ResponseError::new(
        codes::INVALID_PARAMS,
        format!("Invalid params for {method}"),
    )
}

fn folder_paths(list: Option<&Value>) -> Vec<String> {
    list.and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|f| f.get("uri").and_then(Value::as_str))
                .filter_map(|u| uri_to_path(u).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The `dart.analysisExcludedFolders` setting.
fn excluded_folders(config: &Value) -> Vec<String> {
    config
        .get("analysisExcludedFolders")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    /// An output that keeps the bytes.
    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Buffer {
        /// The messages written so far.
        fn messages(&self) -> Vec<Value> {
            let bytes = self.0.lock().unwrap().clone();
            let mut input = std::io::Cursor::new(bytes);
            let mut out = Vec::new();
            while let Ok(Some(m)) = read_message(&mut input) {
                out.push(m);
            }
            out
        }
    }

    fn server(capabilities: Value) -> (Server, Buffer) {
        let buffer = Buffer::default();
        let channel = Channel::new(Box::new(buffer.clone()), None);
        let options = ServerOptions::parse(&[], true).unwrap();
        let mut s = Server::new(options, channel);
        let root = normalize(&format!(
            "{}/../dartr/tests/lsp_fixtures/lsp_project",
            env!("CARGO_MANIFEST_DIR")
        ));
        s.handle_message(
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "capabilities": capabilities, "rootUri": path_to_uri(&root),
            }}),
        );
        s.handle_message(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
        (s, buffer)
    }

    fn response(messages: &[Value], id: i64) -> Value {
        messages
            .iter()
            .find(|m| m.get("method").is_none() && m["id"] == json!(id))
            .cloned()
            .unwrap_or_else(|| panic!("no response {id}"))
    }

    #[test]
    fn cancel_queued_request() {
        let (mut s, out) = server(json!({}));
        let uri = path_to_uri(&format!("{}/src/lib.rs.dart", env!("CARGO_MANIFEST_DIR")));
        let mut queue = VecDeque::new();
        let request = |id: i64| {
            json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/foldingRange",
                "params": {"textDocument": {"uri": uri}}})
        };
        s.enqueue(&mut queue, request(2));
        s.enqueue(&mut queue, request(3));
        s.enqueue(
            &mut queue,
            json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 2}}),
        );
        // A cancellation of an unknown request does nothing.
        s.enqueue(
            &mut queue,
            json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 99}}),
        );
        while let Some(m) = queue.pop_front() {
            s.handle_message(m);
        }
        let messages = out.messages();
        assert_eq!(
            response(&messages, 2)["error"],
            json!({"code": -32800, "message": "Request was cancelled"})
        );
        assert_eq!(response(&messages, 3)["result"], json!([]));
        assert!(s.cancelled.is_empty());
    }

    #[test]
    fn analyzer_status_without_work_done_progress() {
        let (mut s, out) = server(json!({}));
        s.run_analysis();
        let statuses: Vec<Value> = out
            .messages()
            .into_iter()
            .filter(|m| m["method"] == json!("$/analyzerStatus"))
            .map(|m| m["params"].clone())
            .collect();
        assert_eq!(
            statuses,
            [json!({"isAnalyzing": true}), json!({"isAnalyzing": false})]
        );
    }

    #[test]
    fn progress_waits_for_token_creation() {
        let (mut s, out) = server(json!({"window": {"workDoneProgress": true}}));
        s.run_analysis();
        let messages = out.messages();
        let create = messages
            .iter()
            .find(|m| m["method"] == json!("window/workDoneProgress/create"))
            .expect("create request");
        assert!(!messages.iter().any(|m| m["method"] == json!("$/progress")));
        // The client creates the token: begin and end follow.
        s.handle_message(json!({"jsonrpc": "2.0", "id": create["id"], "result": null}));
        let kinds: Vec<Value> = out
            .messages()
            .into_iter()
            .filter(|m| m["method"] == json!("$/progress"))
            .map(|m| m["params"]["value"]["kind"].clone())
            .collect();
        assert_eq!(kinds, [json!("begin"), json!("end")]);
    }

    #[test]
    fn requests_before_initialized() {
        let buffer = Buffer::default();
        let channel = Channel::new(Box::new(buffer.clone()), None);
        let mut s = Server::new(ServerOptions::parse(&[], true).unwrap(), channel);
        s.handle_message(
            json!({"jsonrpc": "2.0", "id": 1, "method": "textDocument/hover", "params": {}}),
        );
        s.handle_message(json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"capabilities": {}}}));
        s.handle_message(json!({"jsonrpc": "2.0", "id": 3, "method": "initialize", "params": {"capabilities": {}}}));
        s.handle_message(
            json!({"jsonrpc": "2.0", "id": 4, "method": "textDocument/hover", "params": {}}),
        );
        let m = buffer.messages();
        assert_eq!(response(&m, 1)["error"]["code"], json!(-32002));
        assert!(response(&m, 2)["result"]["capabilities"].is_object());
        assert_eq!(
            response(&m, 3)["error"],
            json!({"code": -32002, "message": "Server already initialized"})
        );
        assert_eq!(response(&m, 4)["error"]["code"], json!(-32002));
    }
}
