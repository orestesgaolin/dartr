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

use dartr_ast_builder::{ParsedUnit, parse_string};
use dartr_project::{ContextRoot, OptionsParseSession, context_locator::locate_context_roots};
use dartr_syntax::LineInfo;
use rayon::prelude::*;
use serde_json::{Value, json};

use crate::args::ServerOptions;
use crate::capabilities::{self, ClientCapabilities, DynamicRegistration};
use crate::features;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::source_edits::apply_changes;
use crate::transport::{Channel, read_message};
use crate::uri::{UriError, normalize, path_to_uri, uri_to_path};

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

/// What a request sent to the client is for.
enum Pending {
    Configuration { folders: Vec<String> },
    ProgressCreate,
    Registration,
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
    /// The global `dart` settings (`workspace/configuration`).
    config: Value,
    overlays: HashMap<String, Document>,
    /// Open files (Dart priority files), in open order.
    priority: Vec<String>,
    roots: Vec<ContextRoot>,
    excluded: Vec<String>,
    analyzed: BTreeSet<String>,
    /// Files to analyze.
    dirty: BTreeSet<String>,
    roots_dirty: bool,
    /// Analysis waits for the first configuration response.
    waiting_for_config: bool,
    files_with_client_diagnostics: HashSet<String>,
    parsed: HashMap<String, Rc<ParsedFile>>,
    next_request_id: i64,
    pending: HashMap<i64, Pending>,
    progress: Option<ProgressReporter>,
    cancelled: HashSet<String>,
    registrations: Vec<DynamicRegistration>,
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
            config: json!({}),
            overlays: HashMap::new(),
            priority: Vec::new(),
            roots: Vec::new(),
            excluded: Vec::new(),
            analyzed: BTreeSet::new(),
            dirty: BTreeSet::new(),
            roots_dirty: false,
            waiting_for_config: false,
            files_with_client_diagnostics: HashSet::new(),
            parsed: HashMap::new(),
            next_request_id: 1,
            pending: HashMap::new(),
            progress: None,
            cancelled: HashSet::new(),
            registrations: Vec::new(),
            last_registration_id: 0,
            shutdown_called: false,
        }
    }

    /// Adds [message] to the queue. Cancellations are applied at once
    /// (Dart `MessageScheduler.add`).
    fn enqueue(&mut self, queue: &mut VecDeque<Value>, message: Value) {
        if message.get("method").and_then(Value::as_str) == Some("$/cancelRequest") {
            if let Some(id) = message.get("params").and_then(|p| p.get("id")) {
                self.cancelled.insert(id.to_string());
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
        self.channel.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
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
        let method = message.get("method").and_then(Value::as_str).map(str::to_string);
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
                let path = self.path_of_doc(&params)?;
                if !path.ends_with(".dart") {
                    return Ok(json!([]));
                }
                let file = self.require_parsed(&path, true)?;
                Ok(features::document_symbols(&self.client, &path, &file))
            }
            "textDocument/foldingRange" => {
                let path = self.path_of_doc(&params)?;
                match self.parsed_if_analyzed(&path) {
                    Some(file) => Ok(features::folding_ranges(&self.client, &file)),
                    None => Ok(json!([])),
                }
            }
            "textDocument/selectionRange" => {
                let path = self.path_of_doc(&params)?;
                if !path.ends_with(".dart") {
                    return Ok(Value::Null);
                }
                let file = self.require_parsed(&path, false)?;
                let positions = params
                    .get("positions")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid_params(method))?;
                features::selection_ranges(&file, positions)
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
                        let new_global = list.last().cloned().unwrap_or(Value::Null);
                        let new_global = if new_global.is_object() {
                            new_global
                        } else {
                            json!({})
                        };
                        let old_excluded = excluded_folders(&self.config);
                        self.config = new_global;
                        if excluded_folders(&self.config) != old_excluded {
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
            params.get("capabilities").cloned().unwrap_or_else(|| json!({})),
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
            "capabilities": capabilities::server_capabilities(&self.client),
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
    /// `_applyRegistrations`.
    fn perform_dynamic_registration(&mut self) {
        let wanted = capabilities::dynamic_registrations(&self.client);
        let to_add: Vec<DynamicRegistration> = wanted
            .iter()
            .filter(|r| !self.registrations.contains(r))
            .cloned()
            .collect();
        if to_add.is_empty() {
            return;
        }
        let mut registrations = Vec::new();
        for r in &to_add {
            let mut reg = serde_json::Map::new();
            reg.insert("id".into(), json!(self.last_registration_id.to_string()));
            reg.insert("method".into(), json!(r.method));
            if let Some(o) = &r.options {
                reg.insert("registerOptions".into(), o.clone());
            }
            self.last_registration_id += 1;
            registrations.push(Value::Object(reg));
        }
        self.registrations.extend(to_add);
        self.send_request(
            "client/registerCapability",
            json!({"registrations": registrations}),
            Pending::Registration,
        );
    }

    // ---------------------------------------------------------------------
    // Documents.

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
        self.overlays.insert(
            path.clone(),
            Document {
                content: text.to_string(),
            },
        );
        self.file_changed(&path);
        if !self.priority.contains(&path) {
            self.priority.push(path.clone());
            // Open files get closing labels and outlines.
            self.dirty.insert(path.clone());
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
        doc.content = apply_changes(&doc.content, changes)?;
        self.file_changed(&path);
        Ok(())
    }

    fn did_close(&mut self, params: Value) -> ErrorOr<()> {
        let path = self.path_of_doc(&params)?;
        self.overlays.remove(&path);
        self.file_changed(&path);
        self.priority.retain(|p| *p != path);
        if self.workspace_folders.is_empty() {
            self.roots_dirty = true;
        }
        Ok(())
    }

    fn file_changed(&mut self, path: &str) {
        self.parsed.remove(path);
        if self.analyzed.contains(path) {
            self.dirty.insert(path.to_string());
        }
    }

    /// The content of [path]: the overlay, the file, or `None`.
    fn content(&self, path: &str) -> Option<String> {
        match self.overlays.get(path) {
            Some(d) => Some(d.content.clone()),
            None => std::fs::read_to_string(path).ok(),
        }
    }

    /// Dart `isAnalyzed`.
    fn is_analyzed(&self, path: &str) -> bool {
        self.roots.iter().any(|r| r.is_analyzed(path)) && !self.is_excluded(path)
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.excluded
            .iter()
            .any(|e| path == e || path.starts_with(&format!("{e}/")))
    }

    /// The parsed unit of [path] (Dart `getParsedUnit`): `None` if the file
    /// is not a Dart file of the analysis roots.
    fn parsed_if_analyzed(&mut self, path: &str) -> Option<Rc<ParsedFile>> {
        if !path.ends_with(".dart") || !self.is_analyzed(path) {
            return None;
        }
        if let Some(p) = self.parsed.get(path) {
            return Some(p.clone());
        }
        let content = self.content(path).unwrap_or_default();
        let unit = parse_string(&content, path);
        let file = Rc::new(ParsedFile { content, unit });
        self.parsed.insert(path.to_string(), file.clone());
        Some(file)
    }

    /// Dart `requireUnresolvedUnit` / `requireResolvedUnit`.
    fn require_parsed(&mut self, path: &str, resolved: bool) -> ErrorOr<Rc<ParsedFile>> {
        match self.parsed_if_analyzed(path) {
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
        self.excluded = excluded_folders(&self.config)
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
        let session = OptionsParseSession::new();
        self.roots = if included.is_empty() {
            Vec::new()
        } else {
            locate_context_roots(&included, None, None, &session)
        };
        let mut analyzed = BTreeSet::new();
        for root in &self.roots {
            for file in root.analyzed_files() {
                if file.ends_with(".dart") && !self.is_excluded(&file) {
                    analyzed.insert(file);
                }
            }
        }
        // Dart `flushResults` for files that are no longer analyzed.
        let removed: Vec<String> = self.analyzed.difference(&analyzed).cloned().collect();
        for path in removed {
            self.publish_diagnostics(&path, Vec::new());
            self.parsed.remove(&path);
        }
        // Open files outside of the roots are analyzed too when they are in
        // a context root (for example an open file that is not under a
        // folder, without workspace folders).
        self.dirty = analyzed.clone();
        self.analyzed = analyzed;
        self.parsed.clear();
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
        let contents: Vec<(String, Option<String>)> = dirty
            .iter()
            .map(|p| (p.clone(), self.overlays.get(p).map(|d| d.content.clone())))
            .collect();
        let options = self.client.diagnostic_options();
        let results: Vec<(String, Vec<Value>)> = contents
            .into_par_iter()
            .map(|(path, overlay)| {
                let content = overlay
                    .or_else(|| std::fs::read_to_string(&path).ok())
                    .unwrap_or_default();
                let unit = parse_string(&content, &path);
                let diagnostics = unit
                    .diagnostics
                    .iter()
                    .map(|d| {
                        mapping::to_diagnostic(&unit.line_info, &path, d, &options, &|f| {
                            std::fs::read_to_string(f)
                                .ok()
                                .map(|c| LineInfo::from_content(&c))
                        })
                    })
                    .collect();
                (path, diagnostics)
            })
            .collect();
        for (path, diagnostics) in results {
            self.publish_diagnostics(&path, diagnostics);
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
    fn publish_open_file_notifications(&mut self, path: &str) {
        let Some(file) = self.parsed_if_analyzed(path) else {
            return;
        };
        let uri = path_to_uri(path);
        if self.init.closing_labels {
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

fn invalid_params(method: &str) -> ResponseError {
    ResponseError::new(codes::INVALID_PARAMS, format!("Invalid params for {method}"))
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
        .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}
