// Dart source: pkg/analysis_server/lib/src/legacy_analysis_server.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/analysis_*.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/server_*.dart
// Dart source: pkg/analysis_server/lib/src/protocol_server.dart
// Dart source: pkg/analyzer_plugin/lib/utilities/analyzer_converter.dart

//! Legacy request handlers using the shared parse/lint diagnostic pipeline.
//! Resolution-dependent requests return UNKNOWN_REQUEST until the resolver is ready.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use dartr_cli::DriverSession;
use dartr_cli::provider::{AnalyzedFile, FileDiagnostics};
use dartr_cli::server::{AnalysisError, LineInfoCache, analysis_errors};
use dartr_diagnostics::DiagnosticType;
use dartr_element::{ElementId, NoopSink, Tag};
use dartr_project::{AnalysisContextCollection, CollectionOptions, FileKind, fs, non_dart, paths};
use dartr_server::args::ServerOptions;
use dartr_syntax::LineInfo;
use dartr_typesystem::type_ext::TypeExt;
use indexmap::{IndexMap, IndexSet};
use rustc_hash::FxHashMap;
use serde_json::{Value, json};

use crate::convert::{convert_element, is_local_element};
use crate::protocol;
use crate::search::{
    LegacyUnitIndex, OwnedFiles, ResolvedUnitRef, SElem, SearchEngine, SearchScope,
};
use crate::transport::{Channel, LineReader};

/// The supported subset of the pinned spec. Notifications are not requests.
pub const IMPLEMENTED_REQUESTS: &[&str] = &[
    "server.getVersion",
    "server.shutdown",
    "server.setSubscriptions",
    "server.cancelRequest",
    "server.setClientCapabilities",
    "analysis.getErrors",
    "analysis.getHover",
    "analysis.getImportedElements",
    "analysis.getLibraryDependencies",
    "analysis.getNavigation",
    "analysis.getReachableSources",
    "analysis.getSignature",
    "analysis.reanalyze",
    "analysis.setAnalysisRoots",
    "analysis.setGeneralSubscriptions",
    "analysis.setPriorityFiles",
    "analysis.setSubscriptions",
    "analysis.updateContent",
    "analysis.updateOptions",
    "completion.getSuggestions2",
    "completion.getSuggestionDetails2",
    "search.findElementReferences",
    "search.findMemberDeclarations",
    "search.findMemberReferences",
    "search.findTopLevelDeclarations",
    "search.getElementDeclarations",
    "search.getTypeHierarchy",
    "edit.format",
    "edit.getAssists",
    "edit.getAvailableRefactorings",
    "edit.getFixes",
    "edit.getPostfixCompletion",
    "edit.getRefactoring",
    "edit.getStatementCompletion",
    "edit.importElements",
    "edit.isPostfixCompletionApplicable",
    "edit.listPostfixCompletionTemplates",
    "edit.organizeDirectives",
    "edit.sortMembers",
    "edit.formatIfEnabled",
    "edit.bulkFixes",
    "execution.createContext",
    "execution.deleteContext",
    "execution.getSuggestions",
    "execution.mapUri",
    "execution.setSubscriptions",
    "diagnostic.getDiagnostics",
    "diagnostic.getServerPort",
    "analytics.isEnabled",
    "analytics.enable",
    "analytics.sendEvent",
    "analytics.sendTiming",
    "flutter.getWidgetDescription",
    "flutter.setWidgetPropertyValue",
    "flutter.setSubscriptions",
    "lsp.handle",
];
pub const IMPLEMENTED_NOTIFICATIONS: &[&str] = &[
    "server.connected",
    "server.error",
    "server.pluginError",
    "server.log",
    "server.status",
    "analysis.analyzedFiles",
    "analysis.closingLabels",
    "analysis.errors",
    "analysis.flushResults",
    "analysis.folding",
    "analysis.highlights",
    "analysis.implemented",
    "analysis.invalidate",
    "analysis.navigation",
    "analysis.occurrences",
    "analysis.outline",
    "analysis.overrides",
    "search.results",
    "execution.launchData",
    "flutter.outline",
    "lsp.notification",
];

#[derive(Debug)]
struct RequestFailure {
    code: &'static str,
    message: String,
}
type Result<T> = std::result::Result<T, RequestFailure>;

impl RequestFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn missing(path: &str, key: &str) -> Self {
        Self::new(
            "INVALID_PARAMETER",
            format!(
                "Invalid parameter '{path}'. Expected to contain key {}.",
                json!(key)
            ),
        )
    }
    fn mismatch(path: &str, expected: &str, actual: &Value) -> Self {
        let found = if actual.is_null() {
            String::new()
        } else {
            format!("; found \"{actual}\"")
        };
        Self::new(
            "INVALID_PARAMETER",
            format!("Invalid parameter '{path}'. Expected to be {expected}{found}."),
        )
    }
    fn overlay() -> Self {
        Self::new("INVALID_OVERLAY_CHANGE", "Invalid overlay change")
    }
    fn file_not_analyzed(file: &str) -> Self {
        Self::new(
            "FILE_NOT_ANALYZED",
            format!("File is not analyzed: {file}."),
        )
    }
}

fn field<'a>(params: &'a Value, key: &str) -> Result<&'a Value> {
    params
        .get(key)
        .ok_or_else(|| RequestFailure::mismatch("params", key, params))
}
fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| RequestFailure::mismatch(path, "String", value))
}
fn boolean(value: &Value, path: &str) -> Result<bool> {
    value
        .as_bool()
        .ok_or_else(|| RequestFailure::mismatch(path, "bool", value))
}
fn strings(value: &Value, path: &str) -> Result<Vec<String>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let values = value
        .as_array()
        .ok_or_else(|| RequestFailure::mismatch(path, "List", value))?;
    values
        .iter()
        .enumerate()
        .map(|(i, v)| string(v, &format!("{path}[{i}]")).map(str::to_owned))
        .collect()
}
fn object<'a>(value: &'a Value, path: &str) -> Result<&'a serde_json::Map<String, Value>> {
    if value.is_null() {
        static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> =
            std::sync::OnceLock::new();
        return Ok(EMPTY.get_or_init(Default::default));
    }
    value
        .as_object()
        .ok_or_else(|| RequestFailure::mismatch(path, "Map", value))
}
fn valid_path(path: &str) -> Result<()> {
    if !path.starts_with('/') || paths::absolute_normalized(path) != path {
        return Err(RequestFailure::new(
            "INVALID_FILE_PATH_FORMAT",
            format!("Invalid file path format: {path}"),
        ));
    }
    Ok(())
}
fn valid_paths(paths: &[String]) -> Result<()> {
    for path in paths {
        valid_path(path)?;
    }
    Ok(())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn is_client_response(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    if obj.contains_key("method") {
        return false;
    }
    if !obj.get("id").is_some_and(Value::is_string) {
        return false;
    }
    if !obj
        .get("result")
        .is_none_or(|v| v.is_null() || v.is_object())
    {
        return false;
    }
    if let Some(err) = obj.get("error")
        && !err.is_null()
        && serde_json::from_value::<protocol::RequestError>(err.clone()).is_err()
    {
        return false;
    }
    true
}

struct RequestStat {
    method: String,
    client_request_time: i64,
    server_request_time: i64,
}

struct Server<W: Write> {
    channel: Channel<W>,
    options: ServerOptions,
    included: Vec<String>,
    excluded: Vec<String>,
    overlays: IndexMap<String, String>,
    priority: Vec<String>,
    subscriptions: IndexMap<String, Vec<String>>,
    flutter_subscriptions: IndexMap<String, Vec<String>>,
    widget_descriptions: crate::flutter_outline::WidgetDescriptions,
    lsp_capabilities: dartr_server::capabilities::ClientCapabilities,
    send_lsp_notifications: bool,
    status_subscribed: bool,
    log_subscribed: bool,
    general_analyzed_files_subscribed: bool,
    prev_analyzed_files: Option<IndexSet<String>>,
    client_requests: Vec<String>,
    supports_uris: bool,
    execution_contexts: IndexMap<String, String>,
    next_execution_context_id: u64,
    diagnostic_listener: Option<std::net::TcpListener>,
    request_stats: FxHashMap<String, RequestStat>,
    collection: Option<AnalysisContextCollection>,
    driver_session: DriverSession,
    owned_files: OwnedFiles,
    search_scope: Option<Arc<SearchScope>>,
    search_words: FxHashMap<String, Arc<HashSet<String>>>,
    indexes: FxHashMap<(usize, String), Arc<LegacyUnitIndex>>,
    next_search_id: u64,
    published: IndexSet<String>,
    dirty: bool,
    roots_dirty: bool,
}

/// Runs line-delimited JSON until shutdown or input closure.
pub fn run<R: BufRead, W: Write>(options: ServerOptions, input: R, channel: Channel<W>) -> i32 {
    let mut server = Server {
        channel,
        options,
        included: Vec::new(),
        excluded: Vec::new(),
        overlays: IndexMap::new(),
        priority: Vec::new(),
        subscriptions: IndexMap::new(),
        flutter_subscriptions: IndexMap::new(),
        widget_descriptions: crate::flutter_outline::WidgetDescriptions::default(),
        lsp_capabilities: default_legacy_lsp_capabilities(),
        send_lsp_notifications: false,
        status_subscribed: false,
        log_subscribed: false,
        general_analyzed_files_subscribed: false,
        prev_analyzed_files: None,
        client_requests: Vec::new(),
        supports_uris: false,
        execution_contexts: IndexMap::new(),
        next_execution_context_id: 0,
        diagnostic_listener: None,
        request_stats: FxHashMap::default(),
        collection: None,
        driver_session: DriverSession::default(),
        owned_files: OwnedFiles::default(),
        search_scope: None,
        search_words: FxHashMap::default(),
        indexes: FxHashMap::default(),
        next_search_id: 0,
        published: IndexSet::new(),
        dirty: false,
        roots_dirty: false,
    };
    if server
        .notify(
            "server.connected",
            wire(protocol::ServerConnectedParams {
                version: protocol::PROTOCOL_VERSION.into(),
                pid: std::process::id().into(),
            }),
        )
        .is_err()
    {
        return 1;
    }
    let mut input = LineReader::new(input);
    loop {
        let line = match input.next_line() {
            Ok(None) => return 0,
            Err(error) => {
                eprintln!("dartr: {error}");
                return 1;
            }
            Ok(Some(line)) => line,
        };
        server.channel.log_incoming(&line);
        let message = serde_json::from_str::<Value>(&line);
        let message = match message {
            Ok(value)
                if value.get("id").is_some_and(Value::is_string)
                    && value.get("method").is_some_and(Value::is_string)
                    && value
                        .get("params")
                        .is_none_or(|v| v.is_null() || v.is_object())
                    && value
                        .get("clientRequestTime")
                        .is_none_or(|v| v.is_null() || v.is_i64()) =>
            {
                value
            }
            Ok(value) if is_client_response(&value) => {
                continue;
            }
            _ => {
                if server
                    .error_response(
                        "",
                        RequestFailure::new("INVALID_REQUEST", "Invalid request"),
                    )
                    .is_err()
                {
                    return 1;
                }
                continue;
            }
        };
        let id = message["id"].as_str().unwrap();
        let method = message["method"].as_str().unwrap();
        let empty = json!({});
        let params = message
            .get("params")
            .filter(|v| !v.is_null())
            .unwrap_or(&empty);
        let client_request_time = message.get("clientRequestTime").and_then(Value::as_i64);
        if server
            .log_request(id, method, params, client_request_time)
            .is_err()
        {
            return 1;
        }
        let handled = catch_unwind(AssertUnwindSafe(|| server.handle(id, method, params)));
        match handled {
            Ok(Ok(true)) => return 0,
            Ok(Ok(false)) => {}
            Ok(Err(error)) => {
                if server.error_response(id, error).is_err() {
                    return 1;
                }
            }
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("Internal server error");
                if server.server_error(message, false).is_err()
                    || server
                        .error_response(id, RequestFailure::new("SERVER_ERROR", message))
                        .is_err()
                {
                    return 1;
                }
            }
        }
        if server.dirty {
            match catch_unwind(AssertUnwindSafe(|| server.run_analysis())) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    eprintln!("dartr: {error}");
                    return 1;
                }
                Err(_) => {
                    server.dirty = false;
                    if server
                        .server_error("Exception during analysis", false)
                        .is_err()
                        || server.status(false).is_err()
                    {
                        return 1;
                    }
                }
            }
        }
    }
}

impl<W: Write> Server<W> {
    fn send_log_entry(&mut self, kind: &str, data: Value) -> io::Result<()> {
        if !self.log_subscribed {
            return Ok(());
        }
        self.channel.send(&json!({
            "event": "server.log",
            "params": {
                "time": now_millis(),
                "kind": kind,
                "data": data,
                "sdkVersion": "3.13.3",
            }
        }))
    }
    fn log_request(
        &mut self,
        id: &str,
        method: &str,
        params: &Value,
        client_request_time: Option<i64>,
    ) -> io::Result<()> {
        if self.log_subscribed {
            let mut map = serde_json::Map::new();
            map.insert("id".into(), json!(id));
            map.insert("method".into(), json!(method));
            if method == "analysis.updateContent" {
                if let Some(t) = client_request_time {
                    map.insert("clientRequestTime".into(), json!(t));
                }
                if let Some(files) = params.get("files").and_then(Value::as_object) {
                    let keys: Vec<String> = files.keys().cloned().collect();
                    map.insert("files".into(), json!(keys));
                }
            } else {
                if let Some(obj) = params.as_object()
                    && !obj.is_empty()
                {
                    map.insert("params".into(), params.clone());
                }
                if let Some(t) = client_request_time {
                    map.insert("clientRequestTime".into(), json!(t));
                }
            }
            self.send_log_entry("REQUEST", Value::Object(map))?;
        }
        if let Some(client_ms) = client_request_time {
            self.request_stats.insert(
                id.to_owned(),
                RequestStat {
                    method: method.to_owned(),
                    client_request_time: client_ms,
                    server_request_time: now_millis(),
                },
            );
        }
        Ok(())
    }
    fn log_response(&mut self, id: &str) -> io::Result<()> {
        let Some(stat) = self.request_stats.remove(id) else {
            return Ok(());
        };
        if !self.log_subscribed {
            return Ok(());
        }
        self.send_log_entry(
            "RESPONSE",
            json!({
                "id": id,
                "method": stat.method,
                "clientRequestTime": stat.client_request_time,
                "serverRequestTime": stat.server_request_time,
                "responseTime": now_millis(),
            }),
        )
    }
    fn notify(&mut self, event: &str, params: Value) -> io::Result<()> {
        self.channel
            .send(&json!({"event": event, "params": &params}))?;
        if self.log_subscribed
            && event != "server.log"
            && event != "analysis.errors"
            && event != "completion.availableSuggestions"
        {
            let mut map = serde_json::Map::new();
            map.insert("event".into(), json!(event));
            if matches!(
                event,
                "analysis.highlights"
                    | "analysis.implemented"
                    | "analysis.navigation"
                    | "analysis.outline"
                    | "analysis.overrides"
            ) && let Some(f) = params.get("file")
            {
                map.insert("file".into(), f.clone());
            }
            if event == "server.status"
                && let Some(is_analyzing) =
                    params.get("analysis").and_then(|a| a.get("isAnalyzing"))
            {
                map.insert("isAnalyzing".into(), is_analyzing.clone());
            }
            self.send_log_entry("NOTIFICATION", Value::Object(map))?;
        }
        Ok(())
    }
    fn response(&mut self, id: &str, result: Option<Value>) -> Result<()> {
        self.log_response(id)
            .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))?;
        let mut response = json!({"id": id});
        if let Some(result) = result {
            response["result"] = result;
        }
        self.channel
            .send(&response)
            .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }
    fn error_response(&mut self, id: &str, error: RequestFailure) -> io::Result<()> {
        self.log_response(id)?;
        let error = protocol::RequestError {
            code: serde_json::from_value(json!(error.code)).expect("known protocol error code"),
            message: error.message,
            stack_trace: None,
        };
        self.channel.send(&json!({"id":id,"error":error}))
    }
    fn server_error(&mut self, message: &str, fatal: bool) -> io::Result<()> {
        self.notify(
            "server.error",
            wire(protocol::ServerErrorParams {
                is_fatal: fatal,
                message: message.into(),
                stack_trace: std::backtrace::Backtrace::force_capture().to_string(),
            }),
        )
    }
    fn status(&mut self, analyzing: bool) -> io::Result<()> {
        if self.status_subscribed {
            self.notify(
                "server.status",
                wire(protocol::ServerStatusParams {
                    analysis: Some(protocol::AnalysisStatus {
                        is_analyzing: analyzing,
                        analysis_target: None,
                    }),
                    pub_: None,
                }),
            )?;
        }
        Ok(())
    }
    fn handle(&mut self, id: &str, method: &str, params: &Value) -> Result<bool> {
        match method {
            "server.getVersion" => self.response(
                id,
                Some(wire(protocol::ServerGetVersionResult {
                    version: protocol::PROTOCOL_VERSION.into(),
                })),
            )?,
            "server.shutdown" => {
                self.flush(self.published.iter().cloned().collect())?;
                self.response(id, None)?;
                return Ok(true);
            }
            "server.setSubscriptions" => {
                let services = strings(field(params, "subscriptions")?, "params.subscriptions")?;
                for (i, service) in services.iter().enumerate() {
                    if !["STATUS", "LOG"].contains(&service.as_str()) {
                        return Err(RequestFailure::mismatch(
                            &format!("params.subscriptions[{i}]"),
                            "ServerService",
                            &json!(service),
                        ));
                    }
                }
                self.status_subscribed = services.iter().any(|s| s == "STATUS");
                self.log_subscribed = services.iter().any(|s| s == "LOG");
                self.response(id, None)?;
            }
            "server.cancelRequest" => {
                let _cancel_id = string(field(params, "id")?, "params.id")?;
                self.response(id, None)?;
            }
            "server.setClientCapabilities" => {
                let requests = strings(field(params, "requests")?, "params.requests")?;
                let supports_uris = match params.get("supportsUris") {
                    Some(v) if !v.is_null() => boolean(v, "params.supportsUris")?,
                    _ => false,
                };
                if let Some(obj) = params.get("lspCapabilities").and_then(Value::as_object) {
                    for (key, expected_ty) in [
                        ("general", "GeneralClientCapabilities"),
                        ("notebookDocument", "NotebookDocumentClientCapabilities"),
                        ("textDocument", "TextDocumentClientCapabilities"),
                        ("window", "WindowClientCapabilities"),
                        ("workspace", "WorkspaceClientCapabilities"),
                    ] {
                        if let Some(val) = obj.get(key)
                            && !val.is_null()
                            && !val.is_object()
                        {
                            return Err(RequestFailure::new(
                                "INVALID_PARAMETER",
                                format!(
                                    "The 'lspCapabilities' parameter was invalid: {key} must be of type {expected_ty}"
                                ),
                            ));
                        }
                    }
                    let mut merged = default_legacy_lsp_capabilities_json();
                    if let Some(m) = merged.as_object_mut() {
                        for (k, v) in obj {
                            m.insert(k.clone(), v.clone());
                        }
                    }
                    self.lsp_capabilities =
                        dartr_server::capabilities::ClientCapabilities::new(merged);
                    self.send_lsp_notifications = true;
                }
                self.client_requests = requests;
                self.supports_uris = supports_uris;
                self.response(id, None)?;
            }
            "analysis.setAnalysisRoots" => {
                let included = strings(field(params, "included")?, "params.included")?;
                let excluded = strings(field(params, "excluded")?, "params.excluded")?;
                if let Some(roots) = params.get("packageRoots") {
                    for (k, v) in object(roots, "params.packageRoots")? {
                        string(v, &format!("params.packageRoots[{}]", json!(k)))?;
                    }
                }
                valid_paths(&included)?;
                valid_paths(&excluded)?;
                // The pinned handler decodes packageRoots but no longer passes it to setAnalysisRoots.
                self.included = included;
                self.excluded = excluded;
                self.roots_dirty = true;
                self.dirty = true;
                self.response(id, None)?;
            }
            "analysis.setGeneralSubscriptions" => {
                let services = strings(field(params, "subscriptions")?, "params.subscriptions")?;
                for (i, service) in services.iter().enumerate() {
                    if service != "ANALYZED_FILES" {
                        return Err(RequestFailure::mismatch(
                            &format!("params.subscriptions[{i}]"),
                            "GeneralAnalysisService",
                            &json!(service),
                        ));
                    }
                }
                let new_subscribed = services.iter().any(|s| s == "ANALYZED_FILES");
                let old_subscribed = self.general_analyzed_files_subscribed;
                self.general_analyzed_files_subscribed = new_subscribed;
                if new_subscribed && !old_subscribed && !self.dirty {
                    self.send_analyzed_files()
                        .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))?;
                } else if !new_subscribed && old_subscribed {
                    self.prev_analyzed_files = None;
                }
                self.response(id, None)?;
            }
            "analysis.setPriorityFiles" => {
                let files = strings(field(params, "files")?, "params.files")?;
                valid_paths(&files)?;
                self.priority = files;
                self.response(id, None)?;
            }
            "analysis.setSubscriptions" => {
                let map = object(field(params, "subscriptions")?, "params.subscriptions")?;
                let mut subscriptions = IndexMap::new();
                for (service, files) in map {
                    if ![
                        "CLOSING_LABELS",
                        "FOLDING",
                        "HIGHLIGHTS",
                        "IMPLEMENTED",
                        "INVALIDATE",
                        "NAVIGATION",
                        "OCCURRENCES",
                        "OUTLINE",
                        "OVERRIDES",
                    ]
                    .contains(&service.as_str())
                    {
                        return Err(RequestFailure::mismatch(
                            "params.subscriptions.key",
                            "AnalysisService",
                            &json!(service),
                        ));
                    }
                    let files =
                        strings(files, &format!("params.subscriptions[{}]", json!(service)))?;
                    valid_paths(&files)?;
                    subscriptions.insert(service.clone(), files);
                }
                self.subscriptions = subscriptions;
                self.dirty = self.collection.is_some();
                self.response(id, None)?;
            }
            "analysis.updateContent" => {
                self.update_content(params)?;
                self.dirty = true;
                self.response(id, Some(json!({})))?;
            }
            "analysis.updateOptions" => {
                let options = object(field(params, "options")?, "params.options")?;
                for key in [
                    "enableAsync",
                    "enableDeferredLoading",
                    "enableEnums",
                    "enableNullAwareOperators",
                    "generateDart2jsHints",
                    "generateHints",
                    "generateLints",
                ] {
                    if let Some(v) = options.get(key)
                        && !v.is_null()
                    {
                        boolean(v, &format!("params.options.{key}"))?;
                    }
                }
                self.response(id, None)?;
            }
            "analysis.getErrors" => {
                let file = string(field(params, "file")?, "params.file")?;
                valid_path(file)?;
                if !file.ends_with(".dart")
                    || self
                        .collection
                        .as_ref()
                        .is_none_or(|c| c.contexts.is_empty())
                    || fs::read_string(file).is_none()
                {
                    return Err(RequestFailure::new(
                        "GET_ERRORS_INVALID_FILE",
                        "Error during `analysis.getErrors`: invalid file.",
                    ));
                }
                let errors = self.diagnostics(&[file.to_owned()]);
                self.response(
                    id,
                    Some(json!({"errors":errors.get(file).cloned().unwrap_or_default()})),
                )?;
            }
            "analysis.getHover" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let hovers = self.get_hover(file, offset)?;
                self.response(id, Some(wire(protocol::AnalysisGetHoverResult { hovers })))?;
            }
            "analysis.getImportedElements" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let length = integer(field(params, "length")?, "params.length")?;
                valid_path(file)?;
                let elements = self.get_imported_elements(file, offset, length)?;
                self.response(
                    id,
                    Some(wire(protocol::AnalysisGetImportedElementsResult {
                        elements,
                    })),
                )?;
            }
            "analysis.getLibraryDependencies" => {
                return Err(RequestFailure::new(
                    "UNSUPPORTED_FEATURE",
                    "Please contact the Dart analyzer team if you need this request.",
                ));
            }
            "analysis.getNavigation" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let length = integer(field(params, "length")?, "params.length")?;
                valid_path(file)?;
                let result = self.get_navigation(file, offset, length)?;
                self.response(id, Some(wire(result)))?;
            }
            "analysis.getReachableSources" => {
                return Err(RequestFailure::new(
                    "UNSUPPORTED_FEATURE",
                    "Please contact the Dart analyzer team if you need this request.",
                ));
            }
            "analysis.getSignature" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let result = self.get_signature(file, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "analysis.reanalyze" => {
                self.response(id, None)?;
                self.roots_dirty = true;
                self.dirty = true;
            }
            "search.findElementReferences" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let include_potential = boolean(
                    field(params, "includePotential")?,
                    "params.includePotential",
                )?;
                valid_path(file)?;
                self.handle_find_element_references(id, file, offset, include_potential)?;
            }
            "search.findMemberDeclarations" => {
                let name = string(field(params, "name")?, "params.name")?;
                self.handle_find_member_declarations(id, name)?;
            }
            "search.findMemberReferences" => {
                let name = string(field(params, "name")?, "params.name")?;
                self.handle_find_member_references(id, name)?;
            }
            "search.findTopLevelDeclarations" => {
                let pattern = string(field(params, "pattern")?, "params.pattern")?;
                self.handle_find_top_level_declarations(id, pattern)?;
            }
            "search.getElementDeclarations" => {
                let file = match params.get("file") {
                    Some(v) if !v.is_null() => Some(string(v, "params.file")?.to_owned()),
                    _ => None,
                };
                let pattern = match params.get("pattern") {
                    Some(v) if !v.is_null() => Some(string(v, "params.pattern")?.to_owned()),
                    _ => None,
                };
                let max_results = match params.get("maxResults") {
                    Some(v) if !v.is_null() => Some(integer(v, "params.maxResults")?),
                    _ => None,
                };
                let result = self.get_element_declarations(
                    pattern.as_deref().unwrap_or(""),
                    max_results,
                    file.as_deref(),
                );
                self.response(id, Some(wire(result)))?;
            }
            "search.getTypeHierarchy" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let super_only = match params.get("superOnly") {
                    Some(v) if !v.is_null() => Some(boolean(v, "params.superOnly")?),
                    _ => None,
                };
                valid_path(file)?;
                let result = self.get_type_hierarchy(file, offset, super_only)?;
                self.response(id, Some(wire(result)))?;
            }
            "completion.getSuggestions2" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let max_results = integer(field(params, "maxResults")?, "params.maxResults")?;
                if let Some(v) = params.get("completionCaseMatchingMode")
                    && !v.is_null()
                {
                    let s = string(v, "params.completionCaseMatchingMode")?;
                    if !["FIRST_CHAR", "ALL_CHARS", "NONE"].contains(&s) {
                        return Err(RequestFailure::mismatch(
                            "params.completionCaseMatchingMode",
                            "CompletionCaseMatchingMode",
                            v,
                        ));
                    }
                }
                if let Some(v) = params.get("completionMode")
                    && !v.is_null()
                {
                    let s = string(v, "params.completionMode")?;
                    if !["BASIC", "SMART"].contains(&s) {
                        return Err(RequestFailure::mismatch(
                            "params.completionMode",
                            "CompletionMode",
                            v,
                        ));
                    }
                }
                if let Some(v) = params.get("invocationCount")
                    && !v.is_null()
                {
                    integer(v, "params.invocationCount")?;
                }
                let timeout = match params.get("timeout") {
                    Some(v) if !v.is_null() => Some(integer(v, "params.timeout")?),
                    _ => None,
                };
                valid_path(file)?;
                let result =
                    self.completion_get_suggestions2(file, offset, max_results, timeout)?;
                self.response(id, Some(wire(result)))?;
            }
            "completion.getSuggestionDetails2" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let completion = string(field(params, "completion")?, "params.completion")?;
                let library_uri = string(field(params, "libraryUri")?, "params.libraryUri")?;
                valid_path(file)?;
                let result =
                    self.completion_get_suggestion_details2(file, offset, completion, library_uri)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.format" => {
                let file = string(field(params, "file")?, "params.file")?;
                let selection_offset =
                    integer(field(params, "selectionOffset")?, "params.selectionOffset")?;
                let selection_length =
                    integer(field(params, "selectionLength")?, "params.selectionLength")?;
                let line_length = match params.get("lineLength") {
                    Some(v) if !v.is_null() => Some(integer(v, "params.lineLength")?),
                    _ => None,
                };
                let result =
                    self.edit_format(file, selection_offset, selection_length, line_length)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.getAssists" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let length = integer(field(params, "length")?, "params.length")?;
                valid_path(file)?;
                let result = self.edit_get_assists(file, offset, length)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.getAvailableRefactorings" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let length = integer(field(params, "length")?, "params.length")?;
                valid_path(file)?;
                let resolved = self.resolve_unit(file);
                let result = crate::refactoring::get_available_refactorings(
                    resolved.as_ref(),
                    offset,
                    length,
                );
                self.response(id, Some(wire(result)))?;
            }
            "edit.getFixes" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let result = self.edit_get_fixes(file, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.getPostfixCompletion" => {
                let file = string(field(params, "file")?, "params.file")?;
                let key = string(field(params, "key")?, "params.key")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let result = self.edit_get_postfix_completion(file, key, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.getRefactoring" => {
                let kind_val = field(params, "kind")?;
                let kind_str = string(kind_val, "params.kind")?;
                let kind: protocol::RefactoringKind = serde_json::from_value(json!(kind_str))
                    .map_err(|_| {
                        RequestFailure::mismatch("params.kind", "RefactoringKind", kind_val)
                    })?;
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                let length = integer(field(params, "length")?, "params.length")?;
                let validate_only = boolean(field(params, "validateOnly")?, "params.validateOnly")?;
                let options = params.get("options").filter(|v| !v.is_null());
                valid_path(file)?;
                let result =
                    self.edit_get_refactoring(kind, file, offset, length, validate_only, options)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.getStatementCompletion" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let result = self.edit_get_statement_completion(file, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.importElements" => {
                let file = string(field(params, "file")?, "params.file")?;
                let elements =
                    decode_imported_elements_list(field(params, "elements")?, "params.elements")?;
                let offset = match params.get("offset") {
                    Some(v) if !v.is_null() => Some(integer(v, "params.offset")?),
                    _ => None,
                };
                valid_path(file)?;
                let result = self.edit_import_elements(file, &elements, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.isPostfixCompletionApplicable" => {
                let file = string(field(params, "file")?, "params.file")?;
                let key = string(field(params, "key")?, "params.key")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let value = self.resolve_unit(file).is_some_and(|r| {
                    crate::postfix::is_postfix_completion_applicable(&r, file, key, offset)
                });
                self.response(
                    id,
                    Some(wire(protocol::EditIsPostfixCompletionApplicableResult {
                        value,
                    })),
                )?;
            }
            "edit.listPostfixCompletionTemplates" => {
                let result = crate::postfix::list_postfix_completion_templates();
                self.response(id, Some(wire(result)))?;
            }
            "edit.sortMembers" => {
                let file = string(field(params, "file")?, "params.file")?;
                valid_path(file)?;
                let result = self.edit_sort_members(file)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.organizeDirectives" => {
                let file = string(field(params, "file")?, "params.file")?;
                valid_path(file)?;
                let result = self.edit_organize_directives(file)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.formatIfEnabled" => {
                let directories = strings(field(params, "directories")?, "params.directories")?;
                valid_paths(&directories)?;
                let result = self.edit_format_if_enabled(&directories)?;
                self.response(id, Some(wire(result)))?;
            }
            "edit.bulkFixes" => {
                let included = strings(field(params, "included")?, "params.included")?;
                let in_test_mode = match params.get("inTestMode") {
                    Some(v) if !v.is_null() => Some(boolean(v, "params.inTestMode")?),
                    _ => None,
                };
                let update_pubspec = match params.get("updatePubspec") {
                    Some(v) if !v.is_null() => Some(boolean(v, "params.updatePubspec")?),
                    _ => None,
                };
                let codes = match params.get("codes") {
                    Some(v) if !v.is_null() => Some(strings(v, "params.codes")?),
                    _ => None,
                };
                valid_paths(&included)?;
                let result = self.edit_bulk_fixes(
                    &included,
                    in_test_mode,
                    update_pubspec,
                    codes.as_deref(),
                )?;
                self.response(id, Some(wire(result)))?;
            }
            "execution.createContext" => {
                let context_root = string(field(params, "contextRoot")?, "params.contextRoot")?;
                let context_id = self.next_execution_context_id.to_string();
                self.next_execution_context_id += 1;
                self.execution_contexts
                    .insert(context_id.clone(), context_root.to_owned());
                self.response(
                    id,
                    Some(wire(protocol::ExecutionCreateContextResult {
                        id: context_id,
                    })),
                )?;
            }
            "execution.deleteContext" => {
                let context_id = string(field(params, "id")?, "params.id")?;
                self.execution_contexts.shift_remove(context_id);
                self.response(id, None)?;
            }
            "execution.getSuggestions" => {
                self.response(
                    id,
                    Some(wire(protocol::ExecutionGetSuggestionsResult {
                        suggestions: Some(Vec::new()),
                        expressions: Some(Vec::new()),
                    })),
                )?;
            }
            "execution.mapUri" => {
                let context_id = string(field(params, "id")?, "params.id")?;
                let file = match params.get("file") {
                    Some(v) if !v.is_null() => Some(string(v, "params.file")?),
                    _ => None,
                };
                let uri = match params.get("uri") {
                    Some(v) if !v.is_null() => Some(string(v, "params.uri")?),
                    _ => None,
                };
                let result = self.execution_map_uri(context_id, file, uri)?;
                self.response(id, Some(wire(result)))?;
            }
            "execution.setSubscriptions" => {
                self.response(id, None)?;
            }
            "diagnostic.getDiagnostics" => {
                let result = self.get_diagnostics();
                self.response(id, Some(wire(result)))?;
            }
            "diagnostic.getServerPort" => {
                let result = self.get_server_port()?;
                self.response(id, Some(wire(result)))?;
            }
            "analytics.isEnabled" => {
                self.response(
                    id,
                    Some(wire(protocol::AnalyticsIsEnabledResult { enabled: false })),
                )?;
            }
            "analytics.enable" | "analytics.sendEvent" | "analytics.sendTiming" => {
                self.response(id, None)?;
            }
            "flutter.getWidgetDescription" => {
                let file = string(field(params, "file")?, "params.file")?;
                let offset = integer(field(params, "offset")?, "params.offset")?;
                valid_path(file)?;
                let result = self.flutter_get_widget_description(file, offset)?;
                self.response(id, Some(wire(result)))?;
            }
            "flutter.setWidgetPropertyValue" => {
                let prop_id = integer(field(params, "id")?, "params.id")?;
                let value = match params.get("value") {
                    Some(v) if !v.is_null() => {
                        Some(decode_flutter_widget_property_value(v, "params.value")?)
                    }
                    _ => None,
                };
                let result = self.flutter_set_widget_property_value(prop_id, value)?;
                self.response(id, Some(wire(result)))?;
            }
            "flutter.setSubscriptions" => {
                let map = object(field(params, "subscriptions")?, "params.subscriptions")?;
                let mut subscriptions = IndexMap::new();
                for (service, files) in map {
                    if service != "OUTLINE" {
                        return Err(RequestFailure::mismatch(
                            "params.subscriptions.key",
                            "FlutterService",
                            &json!(service),
                        ));
                    }
                    let files =
                        strings(files, &format!("params.subscriptions[{}]", json!(service)))?;
                    subscriptions.insert(service.clone(), files);
                }
                self.flutter_subscriptions = subscriptions;
                self.dirty = self.collection.is_some();
                self.response(id, None)?;
            }
            "lsp.handle" => {
                let lsp_message = field(params, "lspMessage")?;
                let result = self.handle_lsp(lsp_message)?;
                self.response(id, Some(wire(result)))?;
            }
            _ => return Err(RequestFailure::new("UNKNOWN_REQUEST", "Unknown request")),
        }
        Ok(false)
    }

    fn next_search_id(&mut self) -> String {
        let id = self.next_search_id.to_string();
        self.next_search_id += 1;
        id
    }

    fn resolve_unit(&mut self, file: &str) -> Option<ResolvedUnitRef> {
        if !file.ends_with(".dart") {
            return None;
        }
        if self.roots_dirty {
            self.refresh_roots();
        }
        let collection = self.collection.as_ref()?;
        if collection.contexts.is_empty() {
            return None;
        }
        fs::read_string(file)?;
        let library = self.driver_session.resolved_library(collection, file)?;
        let index = library.unit_index(file)?;
        Some(ResolvedUnitRef { library, index })
    }

    fn dartdoc_templates(&mut self, path: &str) -> HashMap<String, String> {
        let Some(collection) = &self.collection else {
            return Default::default();
        };
        let mut templates = HashMap::new();
        for (_, parsed) in self.driver_session.known_parsed_units(collection, path) {
            dartr_server::hover::extract_templates_from_unit(
                &parsed.ast,
                parsed.unit,
                &mut templates,
            );
        }
        templates
    }

    fn hover_library_name(&self, library_path: &str, library_uri: &str) -> String {
        if !library_uri.starts_with("file:") {
            return library_uri.to_string();
        }
        let root = self.collection.as_ref().and_then(|c| {
            let context = c.context_for(library_path)?;
            let root = context
                .root
                .workspace
                .find_package_for(library_path)
                .map(|p| p.root().to_string())
                .unwrap_or_else(|| context.root.root.clone());
            Some(root)
        });
        match root {
            Some(root) => library_path
                .strip_prefix(&format!("{root}/"))
                .unwrap_or(library_path)
                .to_string(),
            None => library_path.to_string(),
        }
    }

    fn get_hover(&mut self, file: &str, offset: i64) -> Result<Vec<protocol::HoverInformation>> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::file_not_analyzed(file));
        };
        let Ok(offset) = u32::try_from(offset) else {
            return Ok(Vec::new());
        };
        let templates = self.dartdoc_templates(file);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let library_name = |p: &str, uri: &str| self.hover_library_name(p, uri);
        let computer = dartr_server::hover::HoverComputer {
            unit: &u,
            root: unit.unit,
            templates: &templates,
            library_name: &library_name,
        };
        let Some(hover) = computer.compute(offset) else {
            return Ok(Vec::new());
        };
        let element = unit
            .ast
            .node_covering(unit.unit, offset, 0)
            .and_then(|n| hover_target_node(&unit.ast, n))
            .and_then(|n| u.locate(n));
        let (element_kind, is_deprecated, containing_library_path) = match element {
            Some(el) => {
                let kind = Some(el.kind().display_name().to_string());
                let deprecated = Some(hover.is_deprecated);
                let enclosing = ctx.element_data(el).and_then(|d| d.enclosing);
                let local = enclosing.is_some_and(dartr_server::element_locator::is_executable)
                    || is_local_element(el);
                let lib_path = if !local {
                    dartr_resolver::error::support::library_of(&ctx, el).map(|lib| {
                        let first = ctx.get(lib).first_fragment();
                        ctx.fragment(first).source.path.to_string()
                    })
                } else {
                    None
                };
                (kind, deprecated, lib_path)
            }
            None => (None, None, None),
        };
        Ok(vec![protocol::HoverInformation {
            offset: hover.offset as i64,
            length: hover.length as i64,
            containing_library_path,
            containing_library_name: hover.containing_library_name,
            containing_class_description: hover.containing_class_description,
            dartdoc: hover.dartdoc,
            element_description: hover.element_description,
            element_kind,
            is_deprecated,
            parameter: hover.parameter,
            propagated_type: None,
            static_type: hover.static_type,
        }])
    }

    fn get_navigation(
        &mut self,
        file: &str,
        offset: i64,
        length: i64,
    ) -> Result<protocol::AnalysisGetNavigationResult> {
        if self
            .collection
            .as_ref()
            .is_none_or(|c| c.contexts.is_empty())
        {
            return Err(RequestFailure::new(
                "GET_NAVIGATION_INVALID_FILE",
                "Error during `analysis.getNavigation`: invalid file.",
            ));
        }
        let Some(resolved) = self.resolve_unit(file) else {
            return Ok(protocol::AnalysisGetNavigationResult {
                files: Vec::new(),
                targets: Vec::new(),
                regions: Vec::new(),
            });
        };
        let offset = u32::try_from(offset.max(0)).unwrap_or(u32::MAX);
        let length = u32::try_from(length.max(0)).unwrap_or(u32::MAX);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let nav = crate::navigation::compute_dart_navigation(
            &ctx,
            &unit.ast,
            &unit.tables,
            unit.unit,
            Some(offset),
            Some(length),
        );
        Ok(protocol::AnalysisGetNavigationResult {
            files: nav.files,
            targets: nav.targets,
            regions: nav.regions,
        })
    }

    fn get_element_at_offset(
        &mut self,
        file: &str,
        offset: i64,
    ) -> Option<(ResolvedUnitRef, ElementId)> {
        let offset = u32::try_from(offset).ok()?;
        let is_priority = self.priority.iter().any(|p| p == file);
        let resolved = self.resolve_unit(file)?;
        let element = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            if !is_priority
                && let Some(frag) =
                    crate::search::find_fragment_by_name_offset(&ctx, unit.fragment, offset)
                && let Some(el) = ctx
                    .fragment_data(frag)
                    .and_then(|d| d.element.try_get().copied())
            {
                Some(el)
            } else {
                let u = dartr_server::element_locator::Unit {
                    ctx: &ctx,
                    ast: &unit.ast,
                    tables: &unit.tables,
                };
                let node = unit.ast.node_covering(unit.unit, offset, 0)?;
                dartr_server::element_locator::get_element(&u, node)
            }
        }?;
        Some((resolved, element))
    }

    fn handle_find_element_references(
        &mut self,
        id: &str,
        file: &str,
        offset: i64,
        include_potential: bool,
    ) -> Result<()> {
        let search_id = self.next_search_id();
        let Some((resolved, mut element)) = self.get_element_at_offset(file, offset) else {
            return self.response(
                id,
                Some(wire(protocol::SearchFindElementReferencesResult {
                    id: None,
                    element: None,
                })),
            );
        };
        let proto_element = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            if element.tag() == Tag::FieldFormalParameter
                && let dartr_element::AnyElement::FormalParameter(p) = ctx.any(element)
                && let Some(field) = p.field.get()
            {
                element = field.raw();
            }
            if matches!(element.tag(), Tag::Getter | Tag::Setter)
                && let Some(var) =
                    dartr_resolver::element_metadata::accessor_variable_any(&ctx, element)
            {
                element = var;
            }
            convert_element(&ctx, element, None)
        };
        self.response(
            id,
            Some(wire(protocol::SearchFindElementReferencesResult {
                id: Some(search_id.clone()),
                element: Some(proto_element),
            })),
        )?;
        let target = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        let results = match &self.collection {
            Some(collection) => {
                let mut engine = SearchEngine {
                    collection,
                    excluded: &self.excluded,
                    session: &mut self.driver_session,
                    owned: &mut self.owned_files,
                    search_scope: &mut self.search_scope,
                    search_words: &mut self.search_words,
                    indexes: &mut self.indexes,
                };
                engine.find_element_references(&target, include_potential)
            }
            None => Vec::new(),
        };
        self.notify(
            "search.results",
            wire(protocol::SearchResultsParams {
                id: search_id,
                results,
                is_last: true,
            }),
        )
        .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }

    fn handle_find_member_declarations(&mut self, id: &str, name: &str) -> Result<()> {
        let search_id = self.next_search_id();
        self.response(
            id,
            Some(wire(protocol::SearchFindMemberDeclarationsResult {
                id: search_id.clone(),
            })),
        )?;
        if self.roots_dirty {
            self.refresh_roots();
        }
        let results = match &self.collection {
            Some(collection) => {
                let mut engine = SearchEngine {
                    collection,
                    excluded: &self.excluded,
                    session: &mut self.driver_session,
                    owned: &mut self.owned_files,
                    search_scope: &mut self.search_scope,
                    search_words: &mut self.search_words,
                    indexes: &mut self.indexes,
                };
                engine.find_member_declarations(name)
            }
            None => Vec::new(),
        };
        self.notify(
            "search.results",
            wire(protocol::SearchResultsParams {
                id: search_id,
                results,
                is_last: true,
            }),
        )
        .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }

    fn handle_find_member_references(&mut self, id: &str, name: &str) -> Result<()> {
        let search_id = self.next_search_id();
        self.response(
            id,
            Some(wire(protocol::SearchFindMemberReferencesResult {
                id: search_id.clone(),
            })),
        )?;
        if self.roots_dirty {
            self.refresh_roots();
        }
        let results = match &self.collection {
            Some(collection) => {
                let mut engine = SearchEngine {
                    collection,
                    excluded: &self.excluded,
                    session: &mut self.driver_session,
                    owned: &mut self.owned_files,
                    search_scope: &mut self.search_scope,
                    search_words: &mut self.search_words,
                    indexes: &mut self.indexes,
                };
                engine.find_member_references(name)
            }
            None => Vec::new(),
        };
        self.notify(
            "search.results",
            wire(protocol::SearchResultsParams {
                id: search_id,
                results,
                is_last: true,
            }),
        )
        .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }

    fn handle_find_top_level_declarations(&mut self, id: &str, pattern: &str) -> Result<()> {
        let regex = regress::Regex::new(pattern).map_err(|e| {
            RequestFailure::new(
                "INVALID_PARAMETER",
                format!("Invalid parameter 'pattern'. RegExp: {e}."),
            )
        })?;
        let search_id = self.next_search_id();
        self.response(
            id,
            Some(wire(protocol::SearchFindTopLevelDeclarationsResult {
                id: search_id.clone(),
            })),
        )?;
        if self.roots_dirty {
            self.refresh_roots();
        }
        let results = match &self.collection {
            Some(collection) => {
                let mut engine = SearchEngine {
                    collection,
                    excluded: &self.excluded,
                    session: &mut self.driver_session,
                    owned: &mut self.owned_files,
                    search_scope: &mut self.search_scope,
                    search_words: &mut self.search_words,
                    indexes: &mut self.indexes,
                };
                engine.find_top_level_declarations(&regex)
            }
            None => Vec::new(),
        };
        self.notify(
            "search.results",
            wire(protocol::SearchResultsParams {
                id: search_id,
                results,
                is_last: true,
            }),
        )
        .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }

    fn get_type_hierarchy(
        &mut self,
        file: &str,
        offset: i64,
        super_only: Option<bool>,
    ) -> Result<protocol::SearchGetTypeHierarchyResult> {
        let Some((resolved, element)) = self.get_element_at_offset(file, offset) else {
            return Ok(protocol::SearchGetTypeHierarchyResult {
                hierarchy_items: None,
            });
        };
        let Some(collection) = &self.collection else {
            return Ok(protocol::SearchGetTypeHierarchyResult {
                hierarchy_items: None,
            });
        };
        let target = SElem {
            lib: resolved.library,
            unit: resolved.index,
            id: element,
        };
        let mut engine = SearchEngine {
            collection,
            excluded: &self.excluded,
            session: &mut self.driver_session,
            owned: &mut self.owned_files,
            search_scope: &mut self.search_scope,
            search_words: &mut self.search_words,
            indexes: &mut self.indexes,
        };
        let hierarchy_items = engine.compute_type_hierarchy(&target, super_only.unwrap_or(false));
        Ok(protocol::SearchGetTypeHierarchyResult { hierarchy_items })
    }

    fn parse_file_for_context(
        &self,
        file: &str,
    ) -> Option<(
        String,
        dartr_ast_builder::ParsedUnit,
        &dartr_project::AnalysisOptions,
    )> {
        let collection = self.collection.as_ref()?;
        let context = collection
            .context_for(file)
            .or_else(|| collection.contexts.first())?;
        let content = fs::read_string(file).unwrap_or_default();
        let content = dartr_syntax::strip_bom(&content).to_string();
        let version = context.file_info(file).language_version;
        let options = collection.options_for(context, file);
        let experiments = options
            .enabled_experiments()
            .iter()
            .filter_map(|name| {
                dartr_parser::ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .find(|flag| flag.name() == *name)
            })
            .collect::<Vec<_>>();
        let parsed = dartr_ast_builder::parse_file(
            &content,
            file,
            (version.major, version.minor),
            &experiments,
        );
        Some((content, parsed, options))
    }

    fn edit_format(
        &self,
        file: &str,
        selection_offset: i64,
        selection_length: i64,
        line_length: Option<i64>,
    ) -> Result<protocol::EditFormatResult> {
        if valid_path(file).is_err()
            || !file.ends_with(".dart")
            || self
                .collection
                .as_ref()
                .is_none_or(|c| c.contexts.is_empty())
        {
            return Err(RequestFailure::new(
                "FORMAT_INVALID_FILE",
                "Error during `edit.format`: invalid file.",
            ));
        }
        let Some((content, _, options)) = self.parse_file_for_context(file) else {
            return Err(RequestFailure::new(
                "FORMAT_INVALID_FILE",
                "Error during `edit.format`: invalid file.",
            ));
        };
        let collection = self.collection.as_ref().unwrap();
        let context = collection
            .context_for(file)
            .or_else(|| collection.contexts.first())
            .unwrap();
        let version = context.file_info(file).language_version;
        match crate::edit::format_code(
            &content,
            (version.major, version.minor),
            &options.enabled_experiments(),
            options.formatter_page_width,
            options.formatter_trailing_commas,
            selection_offset,
            selection_length,
            line_length,
        ) {
            crate::edit::FormatOutcome::Ok(result) => Ok(result),
            crate::edit::FormatOutcome::FormatWithErrors
            | crate::edit::FormatOutcome::InvalidSelection(_) => Err(RequestFailure::new(
                "FORMAT_WITH_ERRORS",
                "Error during `edit.format`: source contains syntax errors.",
            )),
        }
    }

    fn edit_sort_members(&self, file: &str) -> Result<protocol::EditSortMembersResult> {
        if !file.ends_with(".dart") {
            return Err(RequestFailure::new(
                "SORT_MEMBERS_INVALID_FILE",
                "Error during `edit.sortMembers`: invalid file.",
            ));
        }
        if self
            .collection
            .as_ref()
            .is_none_or(|c| c.contexts.is_empty())
            || fs::read_string(file).is_none()
        {
            return Err(RequestFailure::file_not_analyzed(file));
        }
        let Some((content, parsed, options)) = self.parse_file_for_context(file) else {
            return Err(RequestFailure::file_not_analyzed(file));
        };
        let num_errors = parsed
            .diagnostics
            .iter()
            .filter(|d| d.code.diagnostic_type == DiagnosticType::SyntacticError)
            .count();
        if num_errors != 0 {
            return Err(RequestFailure::new(
                "SORT_MEMBERS_PARSE_ERRORS",
                format!(
                    "Error during `edit.sortMembers`: file has {num_errors} scan/parse errors."
                ),
            ));
        }
        let sort_constructors_first = options
            .lint_rules
            .iter()
            .any(|r| r == "sort_constructors_first");
        let edits = crate::edit::sort_members(
            &content,
            &parsed.ast,
            parsed.unit,
            &parsed.line_info,
            sort_constructors_first,
        );
        Ok(protocol::EditSortMembersResult {
            edit: protocol::SourceFileEdit {
                file: file.to_string(),
                file_stamp: -1,
                edits,
            },
        })
    }

    fn edit_organize_directives(
        &mut self,
        file: &str,
    ) -> Result<protocol::EditOrganizeDirectivesResult> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::file_not_analyzed(file));
        };
        let input = &resolved.library.inputs[resolved.index];
        let unit = resolved.unit();
        let num_errors = input
            .parsed
            .diagnostics
            .iter()
            .filter(|d| d.code.diagnostic_type == DiagnosticType::SyntacticError)
            .count();
        if num_errors != 0 {
            return Err(RequestFailure::new(
                "ORGANIZE_DIRECTIVES_ERROR",
                format!("File has {num_errors} scan/parse errors."),
            ));
        }
        let code = &input.parsed.ast.tokens.source;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let edits = crate::edit::organize_directives(
            code,
            &unit.ast,
            unit.unit,
            &input.parsed.line_info,
            &unit.diagnostics,
            Some((&ctx, &unit.tables, unit.fragment)),
        );
        Ok(protocol::EditOrganizeDirectivesResult {
            edit: protocol::SourceFileEdit {
                file: file.to_string(),
                file_stamp: -1,
                edits,
            },
        })
    }

    fn flush(&mut self, files: Vec<String>) -> Result<()> {
        if !files.is_empty() {
            self.notify("analysis.flushResults", json!({"files":files}))
                .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))?;
            for file in files {
                self.published.shift_remove(&file);
            }
        }
        Ok(())
    }
    fn is_excluded(&self, file: &str) -> bool {
        self.excluded
            .iter()
            .any(|root| paths::is_or_within(root, file))
    }
    fn is_analyzed(&self, file: &str) -> bool {
        !self.is_excluded(file)
            && self.collection.as_ref().is_some_and(|c| {
                c.contexts
                    .iter()
                    .any(|context| context.root.is_analyzed(file))
            })
    }
    fn refresh_roots(&mut self) -> IndexSet<String> {
        if self.roots_dirty || self.collection.is_none() {
            self.roots_dirty = false;
            self.collection = Some(AnalysisContextCollection::new(
                &self.included,
                &CollectionOptions {
                    sdk_path: self.options.dart_sdk.clone(),
                    package_config_file: self.options.packages.clone(),
                    enabled_experiments: self.options.enabled_experiments.clone(),
                    ..Default::default()
                },
            ));
            self.driver_session = DriverSession::default();
            self.owned_files = OwnedFiles::default();
            self.search_scope = None;
            self.search_words.clear();
            self.indexes.clear();
            self.widget_descriptions.flush();
        }
        let mut files = IndexSet::new();
        if let Some(collection) = &self.collection {
            for context in &collection.contexts {
                for file in context.root.analyzed_files() {
                    if self.is_analyzed(&file) && FileKind::of(&file) != FileKind::Other {
                        files.insert(file);
                    }
                }
            }
        }
        // Context traversal sees disk files. Include overlay-only Dart files too.
        for file in self.overlays.keys() {
            if file.ends_with(".dart") && self.is_analyzed(file) {
                files.insert(file.clone());
            }
        }
        files
    }
    fn diagnostics(&mut self, paths: &[String]) -> IndexMap<String, Vec<Value>> {
        let Some(collection) = &self.collection else {
            return IndexMap::new();
        };
        let files: Vec<_> = paths
            .iter()
            .filter(|path| path.ends_with(".dart"))
            .filter_map(|path| {
                let context = collection
                    .context_for(path)
                    .or_else(|| collection.contexts.first())?;
                let index = collection
                    .contexts
                    .iter()
                    .position(|c| std::ptr::eq(c, context))?;
                Some(AnalyzedFile {
                    path: path.clone(),
                    context: index,
                })
            })
            .collect();
        let results = self.driver_session.diagnostics(collection, &files);
        let mut infos = LineInfoCache::default();
        let mut output = IndexMap::new();
        for file in results {
            let errors = analysis_errors(collection, &file, &mut infos)
                .iter()
                .map(error_json)
                .collect();
            output.insert(file.path, errors);
        }
        for path in paths.iter().filter(|p| !p.ends_with(".dart")) {
            let Some(context) = collection.context_for(path) else {
                continue;
            };
            let file = FileDiagnostics {
                path: path.clone(),
                line_info: LineInfo::from_content(
                    &fs::read_string_strict(path).unwrap_or_default(),
                ),
                diagnostics: non_dart::diagnostics_for_file(context, path),
            };
            output.insert(
                path.clone(),
                analysis_errors(collection, &file, &mut infos)
                    .iter()
                    .map(error_json)
                    .collect(),
            );
        }
        output
    }
    fn run_analysis(&mut self) -> io::Result<()> {
        self.dirty = false;
        self.status(true)?;
        let files = self.refresh_roots();
        let removed = self.published.difference(&files).cloned().collect();
        self.flush(removed)
            .map_err(|e| io::Error::other(e.message))?;
        let mut paths: Vec<_> = files.iter().cloned().collect();
        // Priority files precede other files. Preserve root traversal order otherwise.
        paths.sort_by_key(|file| {
            self.priority
                .iter()
                .position(|p| p == file)
                .unwrap_or(usize::MAX)
        });
        let errors = self.diagnostics(&paths);
        for file in paths {
            self.notify(
                "analysis.errors",
                json!({"file":file,"errors":errors.get(&file).cloned().unwrap_or_default()}),
            )?;
            self.published.insert(file);
        }
        if let Some(files) = self.subscriptions.get("FOLDING").cloned() {
            for file in files {
                if let Some(regions) = self.folding(&file) {
                    self.notify("analysis.folding", json!({"file":file,"regions":regions}))?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("NAVIGATION").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let nav = crate::navigation::compute_dart_navigation(
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        unit.unit,
                        None,
                        None,
                    );
                    self.notify(
                        "analysis.navigation",
                        wire(protocol::AnalysisNavigationParams {
                            file,
                            regions: nav.regions,
                            targets: nav.targets,
                            files: nav.files,
                        }),
                    )?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("HIGHLIGHTS").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let regions = crate::highlights::compute_dart_highlights(
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        unit.unit,
                    );
                    self.notify(
                        "analysis.highlights",
                        wire(protocol::AnalysisHighlightsParams { file, regions }),
                    )?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("OCCURRENCES").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let occurrences = crate::occurrences::compute_dart_occurrences(
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        unit.unit,
                    );
                    self.notify(
                        "analysis.occurrences",
                        wire(protocol::AnalysisOccurrencesParams { file, occurrences }),
                    )?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("OUTLINE").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let params = crate::outline::compute_analysis_outline(
                        &file,
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        resolved.line_info(),
                        unit.unit,
                    );
                    self.notify("analysis.outline", wire(params))?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("OVERRIDES").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let overrides = crate::overrides::compute_dart_overrides(
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        unit.unit,
                    );
                    self.notify(
                        "analysis.overrides",
                        wire(protocol::AnalysisOverridesParams { file, overrides }),
                    )?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("IMPLEMENTED").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file)
                    && let Some(collection) = &self.collection
                {
                    let mut engine = SearchEngine {
                        collection,
                        excluded: &self.excluded,
                        session: &mut self.driver_session,
                        owned: &mut self.owned_files,
                        search_scope: &mut self.search_scope,
                        search_words: &mut self.search_words,
                        indexes: &mut self.indexes,
                    };
                    let params = crate::implemented::compute_implemented(&resolved, &file, |cls| {
                        engine.members_of_subtypes(cls)
                    });
                    self.notify("analysis.implemented", wire(params))?;
                }
            }
        }
        if let Some(files) = self.subscriptions.get("CLOSING_LABELS").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let unit = resolved.unit();
                    let line_info = resolved.line_info();
                    let labels = dartr_server::computer::closing_labels::compute_closing_labels(
                        &unit.ast, unit.unit, line_info,
                    )
                    .into_iter()
                    .map(|l| protocol::ClosingLabel {
                        offset: l.offset as i64,
                        length: l.length as i64,
                        label: l.label,
                    })
                    .collect();
                    self.notify(
                        "analysis.closingLabels",
                        wire(protocol::AnalysisClosingLabelsParams { file, labels }),
                    )?;
                }
            }
        }
        if let Some(files) = self.flutter_subscriptions.get("OUTLINE").cloned() {
            for file in files {
                if let Some(resolved) = self.resolve_unit(&file) {
                    let sink = NoopSink;
                    let ctx = resolved.ctx(&sink);
                    let unit = resolved.unit();
                    let outline = crate::flutter_outline::compute_flutter_outline(
                        &ctx,
                        &unit.ast,
                        &unit.tables,
                        unit.unit,
                        &file,
                        &unit.ast.tokens.source,
                        resolved.line_info(),
                    );
                    self.notify(
                        "flutter.outline",
                        wire(protocol::FlutterOutlineParams { file, outline }),
                    )?;
                }
            }
        }
        if self.general_analyzed_files_subscribed {
            self.send_analyzed_files()?;
        }
        self.status(false)
    }

    fn send_analyzed_files(&mut self) -> io::Result<()> {
        if !self.general_analyzed_files_subscribed {
            return Ok(());
        }
        let mut analyzed_files = IndexSet::new();
        if let Some(collection) = &self.collection {
            for (idx, context) in collection.contexts.iter().enumerate() {
                for file in self.driver_session.known_files(idx) {
                    if !file.ends_with(".yaml") {
                        analyzed_files.insert(file);
                    }
                }
                for file in context.root.analyzed_files() {
                    if !self.is_excluded(&file) && !file.ends_with(".yaml") {
                        analyzed_files.insert(file);
                    }
                }
            }
        }
        if self.prev_analyzed_files.as_ref() == Some(&analyzed_files) {
            return Ok(());
        }
        self.prev_analyzed_files = Some(analyzed_files.clone());
        let directories: Vec<String> = analyzed_files.into_iter().collect();
        self.notify(
            "analysis.analyzedFiles",
            wire(protocol::AnalysisAnalyzedFilesParams { directories }),
        )
    }

    fn get_imported_elements(
        &mut self,
        file: &str,
        offset: i64,
        length: i64,
    ) -> Result<Vec<protocol::ImportedElements>> {
        // Dart source: pkg/analysis_server/lib/src/domain_analysis_flags.dart
        const DISABLE_MANAGE_IMPORTS_ON_PASTE: bool = true;
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "GET_IMPORTED_ELEMENTS_INVALID_FILE",
                "Error during `analysis.getImportedElements`: invalid file.",
            ));
        };
        if DISABLE_MANAGE_IMPORTS_ON_PASTE {
            return Ok(Vec::new());
        }
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        Ok(crate::search::compute_imported_elements(
            &ctx,
            &unit.ast,
            &unit.tables,
            unit.unit,
            offset,
            length,
        ))
    }

    fn get_signature(
        &mut self,
        file: &str,
        offset: i64,
    ) -> Result<protocol::AnalysisGetSignatureResult> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "GET_SIGNATURE_INVALID_FILE",
                "Error during `analysis.getSignature`: invalid file.",
            ));
        };
        let templates = self.dartdoc_templates(file);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let offset_valid = u32::try_from(offset)
            .ok()
            .and_then(|o| unit.ast.node_covering(unit.unit, o, 0))
            .is_some();
        if !offset_valid {
            return Err(RequestFailure::new(
                "GET_SIGNATURE_INVALID_OFFSET",
                "Error during `analysis.getSignature`: invalid offset.",
            ));
        }
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let Some(sig) =
            dartr_server::signature::compute_signature(&u, unit.unit, offset as u32, &templates)
        else {
            return Err(RequestFailure::new(
                "GET_SIGNATURE_UNKNOWN_FUNCTION",
                "Error during `analysis.getSignature`: unknown function.",
            ));
        };
        let parameters = sig
            .parameters
            .into_iter()
            .map(|p| {
                let kind = match p.kind {
                    dartr_element::ParameterKind::Named => protocol::ParameterKind::OptionalNamed,
                    dartr_element::ParameterKind::Positional => {
                        protocol::ParameterKind::OptionalPositional
                    }
                    dartr_element::ParameterKind::NamedRequired => {
                        protocol::ParameterKind::RequiredNamed
                    }
                    dartr_element::ParameterKind::Required => {
                        protocol::ParameterKind::RequiredPositional
                    }
                };
                let type_ = dartr_element::display_string::type_display_string_with(
                    &ctx,
                    p.ty,
                    dartr_element::display_string::DisplayOptions::default(),
                );
                protocol::ParameterInfo {
                    kind,
                    name: p.name,
                    type_,
                    default_value: p.default_code,
                }
            })
            .collect();
        Ok(protocol::AnalysisGetSignatureResult {
            name: sig.name,
            parameters,
            dartdoc: sig.dartdoc,
        })
    }

    fn get_element_declarations(
        &mut self,
        pattern: &str,
        max_results: Option<i64>,
        only_for_file: Option<&str>,
    ) -> protocol::SearchGetElementDeclarationsResult {
        if self.roots_dirty {
            self.refresh_roots();
        }
        let Some(collection) = &self.collection else {
            return protocol::SearchGetElementDeclarationsResult {
                declarations: Vec::new(),
                files: Vec::new(),
            };
        };
        let mut engine = SearchEngine {
            collection,
            excluded: &self.excluded,
            session: &mut self.driver_session,
            owned: &mut self.owned_files,
            search_scope: &mut self.search_scope,
            search_words: &mut self.search_words,
            indexes: &mut self.indexes,
        };
        engine.get_element_declarations(pattern, max_results, only_for_file)
    }

    fn execution_map_uri(
        &mut self,
        context_id: &str,
        file: Option<&str>,
        uri: Option<&str>,
    ) -> Result<protocol::ExecutionMapUriResult> {
        let Some(context_path) = self.execution_contexts.get(context_id).cloned() else {
            return Err(RequestFailure::new(
                "INVALID_PARAMETER",
                format!(
                    "Invalid parameter 'id'. There is no execution context with an id of {context_id}."
                ),
            ));
        };
        if self.roots_dirty {
            self.refresh_roots();
        }
        let Some(collection) = &self.collection else {
            return Err(RequestFailure::new(
                "INVALID_EXECUTION_CONTEXT",
                format!("Invalid execution context: {context_id}"),
            ));
        };
        let Some(context) = collection
            .context_for(&context_path)
            .or_else(|| collection.contexts.first())
        else {
            return Err(RequestFailure::new(
                "INVALID_EXECUTION_CONTEXT",
                format!("Invalid execution context: {context_id}"),
            ));
        };
        let sdk = context.sdk.as_deref().or(collection.sdk.as_deref());
        if let Some(file) = file {
            if uri.is_some() {
                return Err(RequestFailure::new(
                    "INVALID_PARAMETER",
                    "Invalid parameter 'file'. Either file or uri must be provided, but not both.",
                ));
            }
            if fs::folder_exists(file) {
                return Err(RequestFailure::new(
                    "INVALID_PARAMETER",
                    "Invalid parameter 'file'. Must not refer to a directory.",
                ));
            }
            if !fs::file_exists(file) {
                return Err(RequestFailure::new(
                    "INVALID_PARAMETER",
                    "Invalid parameter 'file'. Must exist.",
                ));
            }
            let mapped_uri = context.root.workspace.path_to_uri(file, sdk);
            Ok(protocol::ExecutionMapUriResult {
                file: None,
                uri: Some(mapped_uri),
            })
        } else if let Some(uri) = uri {
            let Some(resolved_file) = context.root.workspace.resolve_uri(uri, sdk) else {
                return Err(RequestFailure::new(
                    "INVALID_PARAMETER",
                    "Invalid parameter 'uri'. Invalid URI.",
                ));
            };
            Ok(protocol::ExecutionMapUriResult {
                file: Some(resolved_file),
                uri: None,
            })
        } else {
            Err(RequestFailure::new(
                "INVALID_PARAMETER",
                "Invalid parameter 'file'. Either file or uri must be provided.",
            ))
        }
    }

    fn get_diagnostics(&mut self) -> protocol::DiagnosticGetDiagnosticsResult {
        if self.roots_dirty {
            self.refresh_roots();
        }
        let mut contexts = Vec::new();
        if let Some(collection) = &self.collection {
            for (idx, context) in collection.contexts.iter().enumerate() {
                let explicit_file_count = context
                    .root
                    .analyzed_files()
                    .into_iter()
                    .filter(|f| f.ends_with(".dart") && !self.is_excluded(f))
                    .count() as i64;
                let known_count = self.driver_session.known_files(idx).len() as i64;
                let implicit_file_count = (known_count - explicit_file_count).max(0);
                contexts.push(protocol::ContextData {
                    name: context.root.root.clone(),
                    explicit_file_count,
                    implicit_file_count,
                    work_item_queue_length: 0,
                    cache_entry_exceptions: Vec::new(),
                });
            }
        }
        protocol::DiagnosticGetDiagnosticsResult { contexts }
    }

    fn get_server_port(&mut self) -> Result<protocol::DiagnosticGetServerPortResult> {
        if self.diagnostic_listener.is_none() {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| {
                RequestFailure::new("DEBUG_PORT_COULD_NOT_BE_OPENED", e.to_string())
            })?;
            self.diagnostic_listener = Some(listener);
        }
        let port = self
            .diagnostic_listener
            .as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|a| a.port() as i64)
            .ok_or_else(|| {
                RequestFailure::new("DEBUG_PORT_COULD_NOT_BE_OPENED", "Could not get port")
            })?;
        Ok(protocol::DiagnosticGetServerPortResult { port })
    }
    fn folding(&self, file: &str) -> Option<Vec<Value>> {
        let collection = self.collection.as_ref()?;
        let context = collection
            .context_for(file)
            .or_else(|| collection.contexts.first())?;
        if !file.ends_with(".dart") {
            return None;
        }
        let content = fs::read_string(file)?;
        let version = context.file_info(file).language_version;
        let experiments = collection
            .options_for(context, file)
            .enabled_experiments()
            .iter()
            .filter_map(|name| {
                dartr_parser::ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .find(|flag| flag.name() == *name)
            })
            .collect::<Vec<_>>();
        let parsed = dartr_ast_builder::parse_file(
            &content,
            file,
            (version.major, version.minor),
            &experiments,
        );
        Some(
            dartr_server::computer::folding::DartUnitFoldingComputer::new(
                &parsed.ast,
                parsed.unit,
                &parsed.line_info,
            )
            .compute()
            .iter()
            .map(|r| json!({"kind":folding_kind(r.kind),"offset":r.offset,"length":r.length}))
            .collect(),
        )
    }
    fn update_content(&mut self, params: &Value) -> Result<()> {
        let files = object(field(params, "files")?, "params.files")?;
        // Decode all changes before applying any, as AnalysisUpdateContentParams.fromRequest does.
        let mut changes = IndexMap::new();
        for (file, value) in files {
            valid_path(file)?;
            changes.insert(
                file.clone(),
                OverlayChange::decode(value, &format!("params.files[{}]", json!(file)))?,
            );
        }
        for (file, change) in changes {
            let content = match change {
                OverlayChange::Add(content) => Some(content),
                OverlayChange::Change(edits) => {
                    let old = self
                        .overlays
                        .get(&file)
                        .ok_or_else(RequestFailure::overlay)?;
                    let mut units: Vec<u16> = old.encode_utf16().collect();
                    for (offset, length, replacement) in edits {
                        let end = offset
                            .checked_add(length)
                            .ok_or_else(RequestFailure::overlay)?;
                        if end > units.len() {
                            return Err(RequestFailure::overlay());
                        }
                        units.splice(offset..end, replacement.encode_utf16());
                    }
                    Some(String::from_utf16_lossy(&units))
                }
                OverlayChange::Remove => None,
            };
            match &content {
                Some(content) => {
                    self.overlays.insert(file.clone(), content.clone());
                }
                None => {
                    self.overlays.shift_remove(&file);
                }
            }
            fs::set_overlay(&file, content);
            self.indexes.retain(|(_, p), _| p != &file);
            self.search_words.remove(&file);
            self.search_scope = None;
            if file.ends_with(".dart") {
                for affected in self.driver_session.change_file(&file) {
                    self.indexes.retain(|(_, p), _| p != &affected);
                }
            }
            if FileKind::of(&file) == FileKind::AnalysisOptions {
                self.roots_dirty = true;
            }
            self.widget_descriptions.flush();
            self.dirty = true;
        }
        Ok(())
    }

    fn flutter_get_widget_description(
        &mut self,
        file: &str,
        offset: i64,
    ) -> Result<protocol::FlutterGetWidgetDescriptionResult> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "FLUTTER_GET_WIDGET_DESCRIPTION_NO_WIDGET",
                "No widget instance creation at the offset.",
            ));
        };
        let version = self
            .collection
            .as_ref()
            .and_then(|c| c.context_for(file).or_else(|| c.contexts.first()))
            .map(|ctx| {
                let v = ctx.file_info(file).language_version;
                (v.major, v.minor)
            })
            .unwrap_or((3, 0));
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        self.widget_descriptions
            .get_description(
                &ctx,
                &unit.ast,
                &unit.tables,
                unit.unit,
                file,
                &unit.ast.tokens.source,
                version,
                offset,
            )
            .ok_or_else(|| {
                RequestFailure::new(
                    "FLUTTER_GET_WIDGET_DESCRIPTION_NO_WIDGET",
                    "No widget instance creation at the offset.",
                )
            })
    }

    fn flutter_set_widget_property_value(
        &mut self,
        id: i64,
        value: Option<protocol::FlutterWidgetPropertyValue>,
    ) -> Result<protocol::FlutterSetWidgetPropertyValueResult> {
        self.widget_descriptions
            .set_property_value(id, value)
            .map_err(|code| RequestFailure::new(code, ""))
    }

    #[allow(dead_code)]
    pub(crate) fn send_lsp_notification(&mut self, method: &str, params: Value) -> io::Result<()> {
        if !self.send_lsp_notifications {
            return Ok(());
        }
        self.notify(
            "lsp.notification",
            wire(protocol::LspNotificationParams {
                lsp_notification: json!({
                    "jsonrpc": "2.0",
                    "method": method,
                    "params": params,
                }),
            }),
        )
    }

    fn handle_lsp(&mut self, lsp_message: &Value) -> Result<protocol::LspHandleResult> {
        if let Err(err) = validate_lsp_request_message(lsp_message) {
            return Err(RequestFailure::new(
                "INVALID_PARAMETER",
                format!("The 'lspMessage' parameter was not a valid LSP request:\n{err}"),
            ));
        }
        let req_id = lsp_message["id"].clone();
        let method = lsp_message["method"].as_str().unwrap();
        let params = lsp_message.get("params").cloned().unwrap_or(Value::Null);
        let result = self.dispatch_lsp_method(method, &params);
        let lsp_response = match result {
            Ok(val) => json!({
                "id": req_id,
                "jsonrpc": "2.0",
                "result": val,
            }),
            Err(err) => json!({
                "id": req_id,
                "jsonrpc": "2.0",
                "error": err.to_json(),
            }),
        };
        Ok(protocol::LspHandleResult { lsp_response })
    }

    fn dispatch_lsp_method(
        &mut self,
        method: &str,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_server::mapping::{ResponseError, codes};
        match method {
            "initialize" | "initialized" => Err(ResponseError::new(
                codes::SERVER_ALREADY_INITIALIZED,
                "Server already initialized",
            )),
            "$/cancelRequest" => Ok(Value::Null),
            "experimental/echo" => {
                if params.is_null() || params.as_object().is_some_and(|o| o.is_empty()) {
                    Ok(Value::Null)
                } else {
                    Ok(params.clone())
                }
            }
            "dart/diagnosticServer" => {
                let port = self
                    .get_server_port()
                    .map(|r| r.port)
                    .map_err(|e| ResponseError::new(codes::UNHANDLED_ERROR, e.message))?;
                Ok(json!({"port": port}))
            }
            "dart/updateDiagnosticInformation" => Ok(Value::Null),
            "textDocument/documentSymbol" => self.lsp_document_symbols(params),
            "textDocument/hover" => self.lsp_hover(params),
            "textDocument/documentHighlight" => self.lsp_document_highlights(params),
            "textDocument/formatting" => self.lsp_format(params, None, None),
            "textDocument/rangeFormatting" => {
                let range = params
                    .get("range")
                    .cloned()
                    .ok_or_else(|| lsp_invalid_params(method))?;
                self.lsp_format(params, Some(range), None)
            }
            "textDocument/onTypeFormatting" => {
                let pos = params
                    .get("position")
                    .cloned()
                    .ok_or_else(|| lsp_invalid_params(method))?;
                let ch = params
                    .get("ch")
                    .and_then(Value::as_str)
                    .ok_or_else(|| lsp_invalid_params(method))?
                    .to_string();
                self.lsp_format(params, None, Some((pos, ch)))
            }
            "textDocument/signatureHelp" => self.lsp_signature_help(params),
            "textDocument/typeDefinition" => self.lsp_type_definition(params),
            "textDocument/implementation" => self.lsp_implementation(params),
            "textDocument/prepareTypeHierarchy" => self.lsp_prepare_type_hierarchy(params),
            "typeHierarchy/supertypes" => self.lsp_type_hierarchy_supertypes(params),
            "typeHierarchy/subtypes" => self.lsp_type_hierarchy_subtypes(params),
            "textDocument/prepareCallHierarchy" => self.lsp_prepare_call_hierarchy(params),
            "callHierarchy/incomingCalls" => self.lsp_call_hierarchy_incoming(params),
            "callHierarchy/outgoingCalls" => self.lsp_call_hierarchy_outgoing(params),
            "workspace/symbol" => self.lsp_workspace_symbol(params),
            "dart/textDocument/super" => self.lsp_super(params),
            "dart/textDocument/augmentation" => self.lsp_augmentation(params, true),
            "dart/textDocument/augmented" => self.lsp_augmentation(params, false),
            "textDocument/codeAction" => self.lsp_code_action(params),
            _ => Err(ResponseError::new(
                codes::METHOD_NOT_FOUND,
                format!("Unknown method {method}"),
            )),
        }
    }

    fn code_style_for(&self, path: &str) -> dartr_server::completion::CodeStyle {
        let Some(collection) = &self.collection else {
            return dartr_server::completion::CodeStyle::default();
        };
        let Some(context) = collection
            .context_for(path)
            .or_else(|| collection.contexts.first())
        else {
            return dartr_server::completion::CodeStyle::default();
        };
        let options = collection.options_for(context, path);
        let has = |name: &str| options.lint_rules.iter().any(|r| r == name);
        let quote = if has("prefer_single_quotes") {
            '\''
        } else if has("prefer_double_quotes") {
            '"'
        } else {
            '\''
        };
        let lint_quote = if has("prefer_single_quotes") {
            Some('\'')
        } else if has("prefer_double_quotes") {
            Some('"')
        } else {
            None
        };
        dartr_server::completion::CodeStyle {
            specify_types: has("always_specify_types"),
            make_locals_final: has("prefer_final_locals"),
            quote,
            lint_quote,
        }
    }

    fn completion_get_suggestions2(
        &mut self,
        file: &str,
        offset: i64,
        max_results: i64,
        timeout: Option<i64>,
    ) -> Result<protocol::CompletionGetSuggestions2Result> {
        use dartr_server::completion::{self as c, KnownLibrary, RequestInputs, candidate::Kind};
        if file.ends_with(".yaml") {
            return Ok(crate::completion::compute_yaml_suggestions(file, offset));
        }
        let empty = || protocol::CompletionGetSuggestions2Result {
            replacement_offset: offset,
            replacement_length: 0,
            suggestions: Vec::new(),
            is_incomplete: false,
        };
        if !file.ends_with(".dart") {
            return Ok(empty());
        }
        let Some(resolved) = self.resolve_unit(file) else {
            return Ok(empty());
        };
        let Ok(offset_u32) = u32::try_from(offset) else {
            return Ok(empty());
        };
        let line_info = resolved.line_info().clone();
        let templates = self.dartdoc_templates(file);
        let style = self.code_style_for(file);
        let context_index = resolved.library.context;
        let (package, sdk_libraries, packages, known) = match &self.collection {
            Some(collection) => {
                let context = &collection.contexts[context_index];
                let package = match context.root.workspace.find_package_for(file) {
                    Some(dartr_project::workspace::WorkspacePackage::Pub {
                        root, name, ..
                    }) => Some((root, name)),
                    _ => None,
                };
                let sdk = context.sdk.as_ref().or(collection.sdk.as_ref());
                let sdk_libraries = sdk
                    .map(|s| {
                        s.libraries()
                            .iter()
                            .map(|l| (l.short_name.clone(), l.is_internal(), l.implementation))
                            .collect()
                    })
                    .unwrap_or_default();
                let packages = context
                    .packages
                    .packages()
                    .iter()
                    .map(|p| (p.name.clone(), p.lib.clone()))
                    .collect();
                let known = self
                    .driver_session
                    .link_known_libraries(collection, context_index)
                    .map(|(world, libraries)| {
                        let filter = c::FileFilter::new(package.clone(), file);
                        let workspace = collection.contexts[context_index].root.workspace.clone();
                        let root_of = |p: &str| match workspace.find_package_for(p) {
                            Some(dartr_project::workspace::WorkspacePackage::Pub {
                                root, ..
                            }) => Some(root),
                            _ => None,
                        };
                        let mut included: Vec<String> = libraries
                            .iter()
                            .filter(|l| filter.should_include(l, &root_of))
                            .map(|l| l.uri.clone())
                            .collect();
                        let sink = NoopSink;
                        let core_ctx = dartr_element::Ctx {
                            world: &world,
                            current: None,
                            local: None,
                            tp: &resolved.library.type_provider,
                            features: &resolved.library.features,
                            req: &sink,
                        };
                        let mut front = vec!["dart:core".to_string()];
                        if let Some(core) = core_ctx.library_by_uri("dart:core") {
                            let first = core_ctx.get(core).first_fragment();
                            let f = core_ctx.fragment(first);
                            let uris = f.library_exports.iter().map(|e| &e.directive.uri).chain(
                                f.library_imports
                                    .iter()
                                    .filter(|i| !i.is_synthetic)
                                    .map(|i| &i.directive.uri),
                            );
                            for uri in uris {
                                if let dartr_element::DirectiveUri::Library { library, .. } = uri {
                                    let u = c::elem::library_uri(&core_ctx, *library);
                                    if !front.contains(&u) {
                                        front.push(u);
                                    }
                                }
                            }
                        }
                        let mut ordered: Vec<String> =
                            front.into_iter().filter(|u| included.contains(u)).collect();
                        included.retain(|u| !ordered.contains(u));
                        ordered.append(&mut included);
                        (world, ordered)
                    });
                (package, sdk_libraries, packages, known)
            }
            None => (None, Vec::new(), Vec::new(), None),
        };
        let unit = resolved.unit();
        if (offset_u32 as usize) > unit.ast.tokens.source.len() {
            return Ok(empty());
        }
        let sink = NoopSink;
        let world = known
            .as_ref()
            .map(|k| &k.0)
            .unwrap_or(&resolved.library.world);
        let ctx = dartr_element::Ctx {
            world,
            current: None,
            local: Some(&unit.local),
            tp: &resolved.library.type_provider,
            features: &resolved.library.features,
            req: &sink,
        };
        let content = unit.ast.tokens.source.clone();
        let package_root = package.as_ref().map(|(root, _)| root.clone());
        let in_test_directory = package_root
            .as_ref()
            .is_some_and(|root| file.starts_with(&format!("{root}/test/")));
        let Some(q) = c::build_request(RequestInputs {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
            root: unit.unit,
            content: &content,
            path: file,
            offset: offset_u32,
            line_info: &line_info,
            style,
            package_root,
            in_test_directory,
            sdk_libraries,
            packages,
        }) else {
            return Ok(empty());
        };
        let known_list = || -> Vec<KnownLibrary> {
            let Some((world, uris)) = &known else {
                return Vec::new();
            };
            uris.iter()
                .filter_map(|u| world.libraries.get(u.as_str()).copied())
                .map(|element| KnownLibrary { element })
                .collect()
        };
        let budget_ms = timeout.map(|t| t.max(0) as u64).unwrap_or(1000);
        let result = c::compute(
            &q,
            budget_ms,
            max_results,
            Some(&known_list as &dyn Fn() -> Vec<KnownLibrary>),
        );
        let max_len = max_results.max(0) as usize;
        let mut suggestions = Vec::new();
        let mut is_incomplete = result.is_incomplete;
        for mut candidate in result.candidates {
            if suggestions.len() >= max_len {
                is_incomplete = true;
                break;
            }
            match &candidate.kind {
                Kind::Override { .. } => {
                    let data = c::overrides::override_data(&q, &candidate);
                    if let Kind::Override { data: d, .. } = &mut candidate.kind {
                        *d = data;
                    }
                }
                _ => {
                    if candidate.typed().is_some() {
                        let data = c::overrides::typed_data(&q, &candidate);
                        if let Some(t) = candidate.typed_mut() {
                            t.data = data;
                        }
                    }
                }
            }
            if let Some(s) =
                crate::completion::candidate_to_completion_suggestion(&q, candidate, &templates)
            {
                suggestions.push(s);
            }
        }
        Ok(protocol::CompletionGetSuggestions2Result {
            replacement_offset: q.replacement.0 as i64,
            replacement_length: q.replacement.1 as i64,
            suggestions,
            is_incomplete,
        })
    }

    fn completion_get_suggestion_details2(
        &mut self,
        file: &str,
        _offset: i64,
        completion: &str,
        library_uri: &str,
    ) -> Result<protocol::CompletionGetSuggestionDetails2Result> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "INVALID_FILE_PATH_FORMAT",
                format!("Invalid file path: {file}"),
            ));
        };
        let Some(collection) = &self.collection else {
            return Err(RequestFailure::new(
                "INVALID_FILE_PATH_FORMAT",
                format!("Invalid file path: {file}"),
            ));
        };
        let eol = dartr_server::correction::change_builder::end_of_line(
            &resolved.unit().ast.tokens.source,
        );
        let mut workspace = crate::edit::LegacyWorkspace {
            session: &mut self.driver_session,
            collection,
        };
        let mut builder =
            dartr_server::correction::change_builder::ChangeBuilder::new(&mut workspace, eol);
        let mut prefix = None;
        builder.add_dart_file_edit(file, |fb| {
            prefix = fb.import_library_element(library_uri, None, None, false);
        });
        let change = crate::edit::to_protocol_source_change(builder.source_change());
        let result_completion = match prefix {
            Some(p) if !p.is_empty() => format!("{p}.{completion}"),
            _ => completion.to_string(),
        };
        Ok(protocol::CompletionGetSuggestionDetails2Result {
            completion: result_completion,
            change,
        })
    }

    fn edit_get_fixes(&mut self, file: &str, offset: i64) -> Result<protocol::EditGetFixesResult> {
        if self.roots_dirty {
            self.refresh_roots();
        }
        if !self.is_analyzed(file) || fs::read_string(file).is_none() {
            return Err(RequestFailure::new(
                "GET_FIXES_INVALID_FILE",
                "Error during `edit.getFixes`: invalid file.",
            ));
        }
        if !file.ends_with(".dart") {
            return Ok(protocol::EditGetFixesResult { fixes: Vec::new() });
        }
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "GET_FIXES_INVALID_FILE",
                "Error during `edit.getFixes`: invalid file.",
            ));
        };
        let Ok(off) = u32::try_from(offset) else {
            return Err(RequestFailure::new(
                "GET_FIXES_INVALID_FILE",
                "Error during `edit.getFixes`: invalid offset.",
            ));
        };
        let line_info = resolved.line_info().clone();
        if (off as usize) > resolved.unit().ast.tokens.source.len() {
            return Err(RequestFailure::new(
                "GET_FIXES_INVALID_FILE",
                "Error during `edit.getFixes`: invalid offset.",
            ));
        }
        let request_line = line_info.get_location(off).line_number;
        let Some(collection) = &self.collection else {
            return Ok(protocol::EditGetFixesResult { fixes: Vec::new() });
        };
        let context_index = resolved.library.context;
        let diag_files = self.driver_session.diagnostics(
            collection,
            &[AnalyzedFile {
                path: file.to_string(),
                context: context_index,
            }],
        );
        let Some(file_diags) = diag_files.into_iter().next() else {
            return Ok(protocol::EditGetFixesResult { fixes: Vec::new() });
        };
        let mut infos = LineInfoCache::default();
        let all_errors = analysis_errors(collection, &file_diags, &mut infos);
        let context = &collection.contexts[context_index];
        let options = collection.options_for(context, file).clone();
        let resolved_rc = std::rc::Rc::new(dartr_server::server::ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        });
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let utils = dartr_server::correction::utils::CorrectionUtils::new(&unit.ast, &line_info);
        let engine_diags: Vec<dartr_diagnostics::Diagnostic> = file_diags
            .diagnostics
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d.severity = d.code.severity();
                d
            })
            .collect();
        let fix_unit = dartr_server::correction::fix_processor::FixUnit {
            resolved: &resolved_rc,
            ctx: &ctx,
            utils: &utils,
            path: file,
            options: &options,
            diagnostics: &engine_diags,
        };
        let mut workspace = crate::edit::LegacyWorkspace {
            session: &mut self.driver_session,
            collection,
        };
        let mut result_fixes = Vec::new();
        for (diag, err) in engine_diags.iter().zip(all_errors.iter()) {
            let diag_line = line_info.get_location(diag.offset as u32).line_number;
            if diag_line != request_line {
                continue;
            }
            let mut fixes = dartr_server::correction::fix_processor::compute_fixes(
                &fix_unit,
                diag,
                &mut workspace,
                None,
            );
            fixes.sort_by(|a, b| {
                b.kind
                    .priority
                    .cmp(&a.kind.priority)
                    .then_with(|| a.change.message.cmp(&b.change.message))
            });
            let proto_error: protocol::AnalysisError =
                serde_json::from_value(error_json(err)).expect("valid AnalysisError");
            let proto_fixes = fixes
                .into_iter()
                .map(|f| crate::edit::to_protocol_source_change(f.change))
                .collect();
            result_fixes.push(protocol::AnalysisErrorFixes {
                error: proto_error,
                fixes: proto_fixes,
            });
        }
        Ok(protocol::EditGetFixesResult {
            fixes: result_fixes,
        })
    }

    fn edit_get_assists(
        &mut self,
        file: &str,
        offset: i64,
        length: i64,
    ) -> Result<protocol::EditGetAssistsResult> {
        if self.roots_dirty {
            self.refresh_roots();
        }
        if !file.ends_with(".dart") {
            return Ok(protocol::EditGetAssistsResult {
                assists: Vec::new(),
            });
        }
        let (Ok(off), Ok(len)) = (u32::try_from(offset), u32::try_from(length)) else {
            return Ok(protocol::EditGetAssistsResult {
                assists: Vec::new(),
            });
        };
        let Some(resolved) = self.resolve_unit(file) else {
            return Ok(protocol::EditGetAssistsResult {
                assists: Vec::new(),
            });
        };
        let Some(collection) = &self.collection else {
            return Ok(protocol::EditGetAssistsResult {
                assists: Vec::new(),
            });
        };
        let line_info = resolved.line_info().clone();
        let context_index = resolved.library.context;
        let context = &collection.contexts[context_index];
        let options = collection.options_for(context, file).clone();
        let resolved_rc = std::rc::Rc::new(dartr_server::server::ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        });
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        if (off as usize) + (len as usize) > unit.ast.tokens.source.len() {
            return Ok(protocol::EditGetAssistsResult {
                assists: Vec::new(),
            });
        }
        let utils = dartr_server::correction::utils::CorrectionUtils::new(&unit.ast, &line_info);
        let fix_unit = dartr_server::correction::fix_processor::FixUnit {
            resolved: &resolved_rc,
            ctx: &ctx,
            utils: &utils,
            path: file,
            options: &options,
            diagnostics: &unit.diagnostics,
        };
        let mut workspace = crate::edit::LegacyWorkspace {
            session: &mut self.driver_session,
            collection,
        };
        let mut assists = dartr_server::correction::assist_processor::compute_assists(
            &fix_unit,
            off,
            len,
            &mut workspace,
        );
        assists.sort_by(|a, b| {
            b.kind
                .priority
                .cmp(&a.kind.priority)
                .then_with(|| a.change.message.cmp(&b.change.message))
        });
        let proto_assists = assists
            .into_iter()
            .map(|a| crate::edit::to_protocol_source_change(a.change))
            .collect();
        Ok(protocol::EditGetAssistsResult {
            assists: proto_assists,
        })
    }

    fn edit_get_refactoring(
        &mut self,
        kind: protocol::RefactoringKind,
        file: &str,
        offset: i64,
        length: i64,
        validate_only: bool,
        options: Option<&Value>,
    ) -> Result<crate::refactoring::RefactoringResponse> {
        let resolved = self.resolve_unit(file);
        let search_engine = self.collection.as_ref().map(|collection| SearchEngine {
            collection,
            excluded: &self.excluded,
            session: &mut self.driver_session,
            owned: &mut self.owned_files,
            search_scope: &mut self.search_scope,
            search_words: &mut self.search_words,
            indexes: &mut self.indexes,
        });
        crate::refactoring::get_refactoring(
            resolved,
            search_engine,
            kind,
            file,
            offset,
            length,
            validate_only,
            options,
        )
        .map_err(|(code, msg)| RequestFailure::new(code, msg))
    }

    fn edit_get_postfix_completion(
        &mut self,
        file: &str,
        key: &str,
        offset: i64,
    ) -> Result<protocol::EditGetPostfixCompletionResult> {
        let change = match self.resolve_unit(file) {
            Some(resolved) => crate::postfix::get_postfix_completion(&resolved, file, key, offset),
            None => protocol::SourceChange {
                message: String::new(),
                edits: Vec::new(),
                linked_edit_groups: Vec::new(),
                selection: None,
                selection_length: None,
                id: None,
            },
        };
        Ok(protocol::EditGetPostfixCompletionResult { change })
    }

    fn edit_get_statement_completion(
        &mut self,
        file: &str,
        offset: i64,
    ) -> Result<protocol::EditGetStatementCompletionResult> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Ok(protocol::EditGetStatementCompletionResult {
                change: protocol::SourceChange {
                    message: String::new(),
                    edits: Vec::new(),
                    linked_edit_groups: Vec::new(),
                    selection: None,
                    selection_length: None,
                    id: None,
                },
                whitespace_only: false,
            });
        };
        Ok(crate::statement::get_statement_completion(
            &resolved, file, offset,
        ))
    }

    fn edit_import_elements(
        &mut self,
        file: &str,
        elements: &[protocol::ImportedElements],
        _offset: Option<i64>,
    ) -> Result<protocol::EditImportElementsResult> {
        let Some(resolved) = self.resolve_unit(file) else {
            return Err(RequestFailure::new(
                "IMPORT_ELEMENTS_INVALID_FILE",
                "Error during `edit.importElements`: invalid file.",
            ));
        };
        let Some(collection) = &self.collection else {
            return Err(RequestFailure::new(
                "IMPORT_ELEMENTS_INVALID_FILE",
                "Error during `edit.importElements`: invalid file.",
            ));
        };
        let mut workspace = crate::edit::LegacyWorkspace {
            session: &mut self.driver_session,
            collection,
        };
        let edit = crate::edit::compute_import_elements(&mut workspace, &resolved, file, elements);
        Ok(protocol::EditImportElementsResult { edit })
    }

    fn edit_format_if_enabled(
        &mut self,
        directories: &[String],
    ) -> Result<protocol::EditFormatIfEnabledResult> {
        if self.roots_dirty {
            self.refresh_roots();
        }
        let collection = AnalysisContextCollection::new(
            directories,
            &CollectionOptions {
                sdk_path: self.options.dart_sdk.clone(),
                package_config_file: self.options.packages.clone(),
                enabled_experiments: self.options.enabled_experiments.clone(),
                ..Default::default()
            },
        );
        let mut edits = Vec::new();
        for context in &collection.contexts {
            for file in context.root.analyzed_files() {
                if !file.ends_with(".dart") || self.is_excluded(&file) {
                    continue;
                }
                let options = collection.options_for(context, &file);
                if !options.code_style_use_formatter {
                    continue;
                }
                let Some(content) = fs::read_string(&file) else {
                    continue;
                };
                let content = dartr_syntax::strip_bom(&content).to_string();
                let version = context.file_info(&file).language_version;
                if let crate::edit::FormatOutcome::Ok(fmt) = crate::edit::format_code(
                    &content,
                    (version.major, version.minor),
                    &options.enabled_experiments(),
                    options.formatter_page_width,
                    options.formatter_trailing_commas,
                    0,
                    0,
                    None,
                ) && !fmt.edits.is_empty()
                {
                    edits.push(protocol::SourceFileEdit {
                        file,
                        file_stamp: 0,
                        edits: fmt.edits,
                    });
                }
            }
        }
        Ok(protocol::EditFormatIfEnabledResult { edits })
    }

    fn edit_bulk_fixes(
        &mut self,
        included: &[String],
        _in_test_mode: Option<bool>,
        _update_pubspec: Option<bool>,
        codes: Option<&[String]>,
    ) -> Result<protocol::EditBulkFixesResult> {
        if self.roots_dirty {
            self.refresh_roots();
        }
        let collection = AnalysisContextCollection::new(
            included,
            &CollectionOptions {
                sdk_path: self.options.dart_sdk.clone(),
                package_config_file: self.options.packages.clone(),
                enabled_experiments: self.options.enabled_experiments.clone(),
                ..Default::default()
            },
        );
        let code_filter: Option<HashSet<String>> =
            codes.map(|c| c.iter().map(|s| s.to_lowercase()).collect());
        let mut session = DriverSession::default();
        let mut edits = Vec::new();
        let mut details = Vec::new();

        for (ctx_idx, context) in collection.contexts.iter().enumerate() {
            for file in context.root.analyzed_files() {
                if !file.ends_with(".dart") || self.is_excluded(&file) {
                    continue;
                }
                let diag_files = session.diagnostics(
                    &collection,
                    &[AnalyzedFile {
                        path: file.clone(),
                        context: ctx_idx,
                    }],
                );
                let Some(file_diags) = diag_files.into_iter().next() else {
                    continue;
                };
                if file_diags.diagnostics.is_empty() {
                    continue;
                }
                let Some(library) = session.resolved_library(&collection, &file) else {
                    continue;
                };
                let Some(u_idx) = library.unit_index(&file) else {
                    continue;
                };
                let resolved_rc = std::rc::Rc::new(dartr_server::server::ResolvedUnitRef {
                    library: library.clone(),
                    index: u_idx,
                });
                let line_info = resolved_rc.line_info().clone();
                let options = collection.options_for(context, &file).clone();
                let sink = NoopSink;
                let ctx = resolved_rc.ctx(&sink);
                let unit = resolved_rc.unit();
                let utils =
                    dartr_server::correction::utils::CorrectionUtils::new(&unit.ast, &line_info);
                let engine_diags: Vec<dartr_diagnostics::Diagnostic> = file_diags
                    .diagnostics
                    .iter()
                    .map(|d| {
                        let mut d = d.clone();
                        d.severity = d.code.severity();
                        d
                    })
                    .collect();
                let fix_unit = dartr_server::correction::fix_processor::FixUnit {
                    resolved: &resolved_rc,
                    ctx: &ctx,
                    utils: &utils,
                    path: &file,
                    options: &options,
                    diagnostics: &engine_diags,
                };
                let mut workspace = crate::edit::LegacyWorkspace {
                    session: &mut session,
                    collection: &collection,
                };
                let mut file_fixes_map: IndexMap<String, i64> = IndexMap::new();
                let mut file_edits: Vec<protocol::SourceEdit> = Vec::new();
                for diag in &engine_diags {
                    let code_lower = diag.code.lower_case_name().to_string();
                    if let Some(filter) = &code_filter
                        && !filter.contains(&code_lower)
                    {
                        continue;
                    }
                    let fixes = dartr_server::correction::fix_processor::compute_fixes(
                        &fix_unit,
                        diag,
                        &mut workspace,
                        None,
                    );
                    if let Some(first_fix) = fixes.into_iter().find(|f| {
                        !f.change
                            .id
                            .as_deref()
                            .unwrap_or_default()
                            .starts_with("dart.fix.ignore")
                    }) {
                        let proto_change = crate::edit::to_protocol_source_change(first_fix.change);
                        let desc = if proto_change.message.is_empty() {
                            None
                        } else {
                            Some(proto_change.message.clone())
                        };
                        for fe in proto_change.edits {
                            if fe.file == file {
                                for mut e in fe.edits {
                                    e.description = desc.clone();
                                    file_edits.push(e);
                                }
                                *file_fixes_map.entry(code_lower.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                }
                if !file_edits.is_empty() {
                    file_edits.sort_by(|a, b| b.offset.cmp(&a.offset));
                    edits.push(protocol::SourceFileEdit {
                        file: file.clone(),
                        file_stamp: 0,
                        edits: file_edits,
                    });
                    details.push(protocol::BulkFix {
                        path: file,
                        fixes: file_fixes_map
                            .into_iter()
                            .map(|(code, occurrences)| protocol::BulkFixDetail {
                                code,
                                occurrences,
                            })
                            .collect(),
                    });
                }
            }
        }
        Ok(protocol::EditBulkFixesResult {
            message: String::new(),
            edits,
            details,
        })
    }

    #[allow(dead_code)]
    fn lsp_completion(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_server::completion::{
            self as c, RequestInputs,
            candidate::Kind,
            lsp::{ItemCapabilities, ItemContext},
        };
        let path = self.lsp_path_of_doc(params)?;
        if !path.ends_with(".dart") {
            return Ok(json!({"isIncomplete": false, "items": []}));
        }
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let raw_caps = &self.lsp_capabilities.raw;
        let bool_cap = |ptr: &str| {
            raw_caps
                .pointer(ptr)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        let int_set_cap = |ptr: &str| {
            raw_caps
                .pointer(ptr)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_i64).collect::<Vec<_>>())
        };
        let item_ptr = "/textDocument/completion/completionItem";
        let defaults_list: Vec<String> = raw_caps
            .pointer("/textDocument/completion/completionList/itemDefaults")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let doc_formats: Option<Vec<String>> = raw_caps
            .pointer(&format!("{item_ptr}/documentationFormat"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
        let caps = ItemCapabilities {
            snippets: bool_cap(&format!("{item_ptr}/snippetSupport")),
            insert_replace: bool_cap(&format!("{item_ptr}/insertReplaceSupport")),
            deprecated_flag: bool_cap(&format!("{item_ptr}/deprecatedSupport")),
            deprecated_tag: int_set_cap(&format!("{item_ptr}/tagSupport/valueSet"))
                .is_some_and(|s| s.contains(&1)),
            label_details: bool_cap(&format!("{item_ptr}/labelDetailsSupport")),
            as_is_insert_mode: int_set_cap(&format!("{item_ptr}/insertTextModeSupport/valueSet"))
                .is_some_and(|s| s.contains(&1)),
            item_kinds: int_set_cap("/textDocument/completion/completionItemKind/valueSet")
                .unwrap_or_else(|| (1..=18).collect()),
            documentation_formats: doc_formats,
            default_edit_range: defaults_list.iter().any(|d| d == "editRange"),
            default_text_mode: defaults_list.iter().any(|d| d == "insertTextMode"),
            default_data: bool_cap("/textDocument/completion/completionList/applyKindSupport"),
        };
        let templates = self.dartdoc_templates(&path);
        let style = self.code_style_for(&path);
        let context_index = resolved.library.context;
        let (package, sdk_libraries, packages) = match &self.collection {
            Some(collection) => {
                let context = &collection.contexts[context_index];
                let package = match context.root.workspace.find_package_for(&path) {
                    Some(dartr_project::workspace::WorkspacePackage::Pub {
                        root, name, ..
                    }) => Some((root, name)),
                    _ => None,
                };
                let sdk = context.sdk.as_ref().or(collection.sdk.as_ref());
                let sdk_libraries = sdk
                    .map(|s| {
                        s.libraries()
                            .iter()
                            .map(|l| (l.short_name.clone(), l.is_internal(), l.implementation))
                            .collect()
                    })
                    .unwrap_or_default();
                let packages = context
                    .packages
                    .packages()
                    .iter()
                    .map(|p| (p.name.clone(), p.lib.clone()))
                    .collect();
                (package, sdk_libraries, packages)
            }
            None => (None, Vec::new(), Vec::new()),
        };
        let unit = resolved.unit();
        let sink = NoopSink;
        let ctx = dartr_element::Ctx {
            world: &resolved.library.world,
            current: None,
            local: Some(&unit.local),
            tp: &resolved.library.type_provider,
            features: &resolved.library.features,
            req: &sink,
        };
        let content = unit.ast.tokens.source.clone();
        let package_root = package.as_ref().map(|(root, _)| root.clone());
        let in_test_directory = package_root
            .as_ref()
            .is_some_and(|root| path.starts_with(&format!("{root}/test/")));
        let Some(q) = c::build_request(RequestInputs {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
            root: unit.unit,
            content: &content,
            path: &path,
            offset,
            line_info: &line_info,
            style,
            package_root,
            in_test_directory,
            sdk_libraries,
            packages,
        }) else {
            return Ok(json!({"isIncomplete": false, "items": []}));
        };
        let result = c::compute(&q, 100, 1000, None);
        let (r_offset, r_length) = q.replacement;
        let insert_length = offset.saturating_sub(r_offset).min(r_length);
        let default_replacement = dartr_server::mapping::to_range(&line_info, r_offset, r_length);
        let default_insertion =
            dartr_server::mapping::to_range(&line_info, r_offset, insert_length);
        let ic = ItemContext {
            caps: &caps,
            commit_characters_enabled: false,
            complete_function_calls: false,
            has_default_text_mode: false,
            file_path: &path,
        };
        let mut items = Vec::new();
        for mut candidate in result.candidates {
            match &candidate.kind {
                Kind::Override { .. } => {
                    let data = c::overrides::override_data(&q, &candidate);
                    if let Kind::Override { data: d, .. } = &mut candidate.kind {
                        *d = data;
                    }
                }
                _ => {
                    if candidate.typed().is_some() {
                        let data = c::overrides::typed_data(&q, &candidate);
                        if let Some(t) = candidate.typed_mut() {
                            t.data = data;
                        }
                    }
                }
            }
            let element = candidate
                .element()
                .map(|e| dartr_typesystem::member::base_element(&ctx, e));
            let location = element.and_then(|e| c::lsp::element_location(&ctx, e));
            let resolution = location.map(|l| json!({"file": path, "ref": l}));
            let doc = element
                .and_then(|e| dartr_server::hover::documentation(&ctx, e, &templates))
                .map(|d| c::lsp::remove_dart_doc_delimiters(&d))
                .map(|d| dartr_server::hover::clean_dartdoc(&d));
            if let Some(item) = c::lsp::to_item(
                &ctx,
                &ic,
                &candidate,
                default_replacement.clone(),
                default_insertion.clone(),
                false,
                resolution,
                doc,
                // TODO: color previews (Dart `getColorHexString`) need the driver's constant
                // values, as the LSP handler in dartr_server/src/server/completion.rs computes them.
                None,
            ) {
                items.push(item);
            }
        }
        Ok(json!({
            "isIncomplete": result.is_incomplete,
            "items": items,
        }))
    }

    #[allow(dead_code)]
    fn lsp_completion_resolve(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        let Some(data) = params.get("data").filter(|d| d.is_object()) else {
            return Ok(params.clone());
        };
        let Some(file) = data.get("file").and_then(Value::as_str).map(str::to_string) else {
            return Ok(params.clone());
        };
        let reference = data.get("ref").and_then(Value::as_str).map(str::to_string);
        let Ok(resolved) = self.lsp_require_resolved(&file) else {
            return Ok(params.clone());
        };
        let templates = self.dartdoc_templates(&file);
        let formats: Option<Vec<String>> = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/completion/completionItem/documentationFormat")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let mut item = params.as_object().cloned().unwrap_or_default();
        if let Some(r) = reference
            && let Some(el) = lsp_locate_element(&ctx, &r)
            && let Some(doc) = dartr_server::hover::documentation(&ctx, el, &templates)
        {
            let cleaned = dartr_server::hover::clean_dartdoc(
                &dartr_server::completion::lsp::remove_dart_doc_delimiters(&doc),
            );
            if !cleaned.is_empty() {
                item.insert(
                    "documentation".into(),
                    lsp_markup_content_or_string(&formats, cleaned),
                );
            }
        }
        item.entry("additionalTextEdits".to_string())
            .or_insert_with(|| json!([]));
        Ok(Value::Object(item))
    }

    fn lsp_code_action(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        let Ok(resolved) = self.lsp_require_resolved(&path) else {
            return Ok(json!([]));
        };
        let line_info = resolved.line_info().clone();
        let range = params.get("range").cloned().unwrap_or(Value::Null);
        let start = range
            .get("start")
            .and_then(dartr_server::mapping::read_position);
        let end = range
            .get("end")
            .and_then(dartr_server::mapping::read_position);
        let (Some(start), Some(end)) = (start, end) else {
            return Ok(json!([]));
        };
        let (Ok(start_offset), Ok(end_offset)) = (
            dartr_server::mapping::to_offset(&line_info, start.0, start.1, false),
            dartr_server::mapping::to_offset(&line_info, end.0, end.1, false),
        ) else {
            return Ok(json!([]));
        };
        let literal = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/codeAction/codeActionLiteralSupport")
            .is_some();
        let mut actions = Vec::new();
        if let Ok(assists) = self.edit_get_assists(
            &path,
            start_offset as i64,
            end_offset.saturating_sub(start_offset) as i64,
        ) {
            for assist in assists.assists {
                let kind = assist
                    .id
                    .as_deref()
                    .map(|id| id.replace("dart.assist", "refactor"))
                    .unwrap_or_else(|| "refactor".to_string());
                if literal {
                    let mut doc_changes = Vec::new();
                    for fe in &assist.edits {
                        let fe_lines = fs::read_string(&fe.file)
                            .map(|c| LineInfo::from_content(&c))
                            .unwrap_or_else(|| line_info.clone());
                        let edits: Vec<Value> = fe
                            .edits
                            .iter()
                            .map(|e| {
                                json!({
                                    "range": dartr_server::mapping::to_range(&fe_lines, e.offset as u32, e.length as u32),
                                    "newText": e.replacement,
                                })
                            })
                            .collect();
                        doc_changes.push(json!({
                            "textDocument": {"uri": dartr_server::uri::path_to_uri(&fe.file), "version": Value::Null},
                            "edits": edits,
                        }));
                    }
                    actions.push(json!({
                        "title": assist.message,
                        "kind": kind,
                        "diagnostics": [],
                        "edit": {"documentChanges": doc_changes},
                    }));
                } else {
                    actions.push(json!({
                        "title": assist.message,
                        "command": "dart.edit.codeAction.apply",
                        "arguments": [{
                            "textDocument": {"uri": dartr_server::uri::path_to_uri(&path), "version": Value::Null},
                            "range": range,
                            "kind": kind,
                            "loggedAction": assist.id,
                        }],
                    }));
                }
            }
        }
        Ok(Value::Array(actions))
    }

    fn lsp_path_of_doc(&self, params: &Value) -> dartr_server::mapping::ErrorOr<String> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                dartr_server::mapping::ResponseError::new(
                    dartr_server::mapping::codes::INVALID_FILE_PATH,
                    "Document URI was not supplied",
                )
            })?;
        self.lsp_path_of_uri(uri)
    }

    fn lsp_path_of_uri(&self, uri: &str) -> dartr_server::mapping::ErrorOr<String> {
        use dartr_server::mapping::{ResponseError, codes};
        use dartr_server::uri::{UriError, uri_to_path};
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

    fn lsp_require_resolved(
        &mut self,
        path: &str,
    ) -> dartr_server::mapping::ErrorOr<ResolvedUnitRef> {
        use dartr_server::mapping::{ResponseError, codes};
        let not_analyzed = || {
            ResponseError::with_data(codes::FILE_NOT_ANALYZED, "File is not being analyzed", path)
        };
        if !path.ends_with(".dart") {
            return Err(not_analyzed());
        }
        if self.roots_dirty {
            self.refresh_roots();
        }
        if self
            .collection
            .as_ref()
            .is_none_or(|c| c.contexts.is_empty())
        {
            return Err(not_analyzed());
        }
        if fs::read_string(path).is_none() {
            return Err(ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File does not exist",
                path,
            ));
        }
        self.resolve_unit(path).ok_or_else(not_analyzed)
    }

    fn lsp_position_offset(
        &self,
        line_info: &LineInfo,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<u32> {
        use dartr_server::mapping::{ResponseError, codes, read_position, to_offset};
        let (line, character) = params
            .get("position")
            .and_then(read_position)
            .ok_or_else(|| ResponseError::new(codes::INVALID_PARAMS, "Invalid params"))?;
        to_offset(line_info, line, character, false)
    }

    fn lsp_document_symbols(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_server::mapping::{ResponseError, codes};
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        if self.roots_dirty {
            self.refresh_roots();
        }
        if self
            .collection
            .as_ref()
            .is_none_or(|c| c.contexts.is_empty())
        {
            return Err(ResponseError::with_data(
                codes::FILE_NOT_ANALYZED,
                "File is not being analyzed",
                &path,
            ));
        }
        if fs::read_string(&path).is_none() {
            return Err(ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File does not exist",
                &path,
            ));
        }
        let Some((content, unit, _)) = self.parse_file_for_context(&path) else {
            return Err(ResponseError::with_data(
                codes::FILE_NOT_ANALYZED,
                "File is not being analyzed",
                &path,
            ));
        };
        let parsed = dartr_server::server::ParsedFile { content, unit };
        Ok(dartr_server::features::document_symbols(
            &self.lsp_capabilities,
            &path,
            &parsed,
        ))
    }

    fn lsp_hover(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        if !lsp_is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let templates = self.dartdoc_templates(&path);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let library_name = |p: &str, uri: &str| self.hover_library_name(p, uri);
        let computer = dartr_server::hover::HoverComputer {
            unit: &u,
            root: unit.unit,
            templates: &templates,
            library_name: &library_name,
        };
        let Some(hover) = computer.compute(offset) else {
            return Ok(Value::Null);
        };
        let content = dartr_server::hover::hover_markdown(&hover);
        let formats: Option<Vec<String>> = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/hover/contentFormat")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
        let contents = lsp_markup_content_or_string(&formats, content);
        Ok(json!({
            "contents": contents,
            "range": dartr_server::mapping::to_range(&line_info, hover.offset, hover.length),
        }))
    }

    fn lsp_document_highlights(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let tokens = dartr_server::highlights::compute(&u, unit.unit, offset);
        let highlights: Vec<Value> = tokens
            .into_iter()
            .map(|(token, kind)| {
                let t = unit.ast.tokens.get(token);
                json!({
                    "range": dartr_server::mapping::to_range(&line_info, t.offset, t.end() - t.offset),
                    "kind": kind,
                })
            })
            .collect();
        Ok(Value::Array(highlights))
    }

    fn lsp_format(
        &mut self,
        params: &Value,
        range: Option<Value>,
        on_type: Option<(Value, String)>,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_server::formatting::{FormatterOptions, format_file, should_trigger_formatting};
        use dartr_server::mapping::{ResponseError, codes};
        if !lsp_is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.lsp_path_of_doc(params)?;
        if self.roots_dirty {
            self.refresh_roots();
        }
        if fs::read_string(&path).is_none() {
            return Err(ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File does not exist",
                path,
            ));
        }
        let Some((content, unit, options)) = self.parse_file_for_context(&path) else {
            return Ok(Value::Null);
        };
        if !unit.diagnostics.is_empty() {
            return Ok(Value::Null);
        }
        let fmt_options = FormatterOptions {
            page_width: options.formatter_page_width,
            trailing_commas: options.formatter_trailing_commas,
        };
        let file = dartr_server::server::ParsedFile { content, unit };
        if let Some((pos, ch)) = on_type {
            match should_trigger_formatting(&file, &pos, &ch) {
                Ok(true) => format_file(&file, &fmt_options, None, None),
                Ok(false) => Ok(Value::Null),
                Err(_) => Err(ResponseError::new(
                    codes::UNHANDLED_ERROR,
                    "An error occurred while handling textDocument/onTypeFormatting request",
                )),
            }
        } else {
            format_file(&file, &fmt_options, None, range.as_ref())
        }
    }

    fn lsp_signature_help(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        if !lsp_is_dart_document(params) {
            return Ok(Value::Null);
        }
        let auto_triggered = params
            .pointer("/context/triggerKind")
            .and_then(Value::as_i64)
            == Some(2)
            && params
                .pointer("/context/isRetrigger")
                .and_then(Value::as_bool)
                == Some(false);
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let formats: Option<Vec<String>> = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/signatureHelp/signatureInformation/documentationFormat")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
        let null_active = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/signatureHelp/signatureInformation/noActiveParameterSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let templates = self.dartdoc_templates(&path);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let documentation = |doc: String| lsp_markup_content_or_string(&formats, doc);
        if unit.ast.node_covering(unit.unit.raw(), offset, 0).is_none() {
            return Ok(Value::Null);
        }
        if let Some((help, list_offset)) = dartr_server::signature::compute_type_arguments_signature(
            &u,
            unit.unit,
            offset,
            &templates,
            documentation,
            null_active,
        ) && !(auto_triggered && offset != list_offset + 1)
        {
            return Ok(help);
        }
        let Some(signature) =
            dartr_server::signature::compute_signature(&u, unit.unit, offset, &templates)
        else {
            return Ok(Value::Null);
        };
        if auto_triggered && offset != signature.argument_list_offset + 1 {
            return Ok(Value::Null);
        }
        Ok(dartr_server::signature::to_signature_help(
            &ctx,
            &signature,
            documentation,
            null_active,
        ))
    }

    fn lsp_type_definition(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let supports_link = self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/typeDefinition/linkSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let path = self.lsp_path_of_doc(params)?;
        let Ok(resolved) = self.lsp_require_resolved(&path) else {
            return Ok(json!([]));
        };
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(node) = ast.node_covering(unit.unit, offset, 0) else {
            return Ok(json!([]));
        };
        let token_range = |t: dartr_syntax::TokenId| {
            let t = ast.tokens.get(t);
            (t.offset, t.end() - t.offset)
        };
        let variable_type = |e: Option<dartr_element::ElementId>| {
            e.map(|e| dartr_resolver::element_ext::variable_type(&ctx, e))
        };
        let pattern_type = |n: NodeId| {
            unit.tables
                .pattern_info
                .get(n)
                .and_then(|i| i.matched_value_type)
        };
        let mut origin: Option<(u32, u32)> = None;
        let mut ty: Option<dartr_element::TypeId> = None;
        let mut element: Option<dartr_element::ElementId> = None;
        if let Some(n) = ast.cast::<NamedType>(node) {
            origin = Some(token_range(ast[n].name));
            element = u
                .element(n)
                .filter(|e| e.cast::<dartr_element::InterfaceElement>().is_some());
        } else if let Some(n) = ast.cast::<VariableDeclaration>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(u.declared_element(n));
        } else if let Some(n) = ast.cast::<DeclaredIdentifier>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(u.declared_element(n));
        } else if let Some(n) = ast.cast::<DeclaredVariablePattern>(node) {
            origin = Some(token_range(ast[n].name));
            ty = pattern_type(node);
        } else if let Some(n) = ast.cast::<AssignedVariablePattern>(node) {
            origin = Some(token_range(ast[n].name));
            ty = pattern_type(node);
        } else if let Some(n) = ast.cast::<PatternFieldName>(node) {
            if let Some(name) = ast[n].name {
                origin = Some(token_range(name));
            }
            if let Some(field) = ast.parent(n).and_then(|p| ast.cast::<PatternField>(p)) {
                ty = pattern_type(ast[field].pattern.raw());
            }
        } else if let Some(n) = ast.cast::<NamedArgument>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(dartr_resolver::error::support::corresponding_parameter(
                &ctx,
                ast,
                &unit.tables,
                n.raw(),
            ));
        } else if ast.is::<Expression>(node) {
            origin = Some((ast.offset(node), ast.length(node)));
            let mut done = false;
            if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
                let e = u.element(s);
                if let Some(e) = e
                    && e.cast::<dartr_element::InterfaceElement>().is_some()
                {
                    element = Some(e);
                    done = true;
                } else if let Some(e) = e
                    && e.cast::<dartr_element::VariableElement>().is_some()
                    && dartr_resolver::error::support::in_declaration_context(ast, s)
                {
                    ty = variable_type(Some(e));
                    done = true;
                } else if dartr_resolver::ast_ext::simple_identifier_in_setter_context(ast, s) {
                    let write = u.write_or_read_element(s);
                    if let Some(w) = write
                        && matches!(w.tag(), Tag::Getter | Tag::Setter)
                        && let Some(variable) =
                            dartr_resolver::element_metadata::accessor_variable(&ctx, w)
                    {
                        ty = variable_type(Some(variable));
                        done = true;
                    }
                }
            }
            if !done {
                ty = unit.tables.static_type.get(node).copied();
            }
        } else if ast.is::<FormalParameter>(node) {
            origin = dartr_resolver::ast_ext::formal_parameter_parts(ast, node)
                .name
                .map(token_range);
            ty = variable_type(u.declared_element(node));
        }
        let Some((origin_offset, origin_length)) = origin else {
            return Ok(json!([]));
        };
        if element.is_none()
            && let Some(t) = ty
        {
            element = match ctx.ty(t) {
                dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
                dartr_element::TypeKind::TypeParameter { param, .. } => Some(param.raw()),
                _ => None,
            };
        }
        let Some(element) = element else {
            return Ok(json!([]));
        };
        let Some(fragment) = dartr_server::navigation::element_fragment(&ctx, element) else {
            return Ok(json!([]));
        };
        let Some(file) = dartr_server::navigation::fragment_path(&ctx, fragment) else {
            return Ok(json!([]));
        };
        let Some(data) = ctx.fragment_data(fragment) else {
            return Ok(json!([]));
        };
        let (Some(name_offset), Some(name)) = (data.name_offset, data.name) else {
            return Ok(json!([]));
        };
        let name_length = ctx.name_str(name).encode_utf16().count() as u32;
        let Some(target_content) = fs::read_string(&file) else {
            return Ok(json!([]));
        };
        let target_lines = LineInfo::from_content(&target_content);
        let name_range = dartr_server::mapping::to_range(&target_lines, name_offset, name_length);
        let uri = dartr_server::uri::path_to_uri(&file);
        if supports_link {
            let code_range = match (data.code_offset, data.code_length) {
                (Some(o), Some(l)) => dartr_server::mapping::to_range(&target_lines, o, l),
                _ => name_range.clone(),
            };
            Ok(json!([{
                "originSelectionRange": dartr_server::mapping::to_range(&line_info, origin_offset, origin_length),
                "targetUri": uri,
                "targetRange": code_range,
                "targetSelectionRange": name_range,
            }]))
        } else {
            Ok(json!({"uri": uri, "range": name_range}))
        }
    }

    fn lsp_implementation(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let element = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            let u = dartr_server::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            unit.ast
                .node_covering(unit.unit, offset, 0)
                .and_then(|n| dartr_server::element_locator::get_element(&u, n))
        };
        let Some(element) = element else {
            return Ok(json!([]));
        };
        let pivot = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        let helper = pivot.with(|ctx| {
            crate::search::HierarchyHelper::from_element(ctx, pivot.lib.context, element)
        });
        let Some(pivot_class) = helper.pivot_class else {
            return Ok(json!([]));
        };
        let pivot_class = pivot.same(pivot_class);
        let needs_member = pivot_class
            .with(|ctx| helper.find_member(ctx, pivot_class.lib.context, pivot_class.id))
            .is_some();
        let Some(collection) = &self.collection else {
            return Ok(json!([]));
        };
        let mut engine = SearchEngine {
            collection,
            excluded: &self.excluded,
            session: &mut self.driver_session,
            owned: &mut self.owned_files,
            search_scope: &mut self.search_scope,
            search_words: &mut self.search_words,
            indexes: &mut self.indexes,
        };
        let mut all = Vec::new();
        let mut keys = Vec::new();
        engine.append_all_subtypes(&pivot_class, &mut all, &mut keys);
        let mut seen: Vec<(usize, dartr_server::index::ElementKey)> = Vec::new();
        let mut locations = Vec::new();
        for sub in all {
            let target = if needs_member {
                let found = sub.with(|ctx| {
                    helper
                        .find_member(ctx, sub.lib.context, sub.id)
                        .map(|m| dartr_element::diagnostics::non_synthetic(ctx, m))
                });
                match found {
                    Some(m) => sub.same(m),
                    None => continue,
                }
            } else {
                sub
            };
            if let Some(k) = target.identity() {
                if seen.contains(&k) {
                    continue;
                }
                seen.push(k);
            }
            let location = target.with(|ctx| {
                let first = ctx.element_data(target.id)?.first_fragment;
                let path = dartr_server::navigation::fragment_path(ctx, first)?;
                let data = ctx.fragment_data(first)?;
                let name = data.name?;
                Some((
                    path,
                    data.name_offset?,
                    ctx.name_str(name).encode_utf16().count() as u32,
                ))
            });
            if let Some((path, o, l)) = location
                && let Some(content) = fs::read_string(&path)
            {
                let lines = LineInfo::from_content(&content);
                locations.push(json!({
                    "uri": dartr_server::uri::path_to_uri(&path),
                    "range": dartr_server::mapping::to_range(&lines, o, l),
                }));
            }
        }
        Ok(Value::Array(locations))
    }

    fn lsp_prepare_type_hierarchy(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let Some(node) = ast.node_covering(unit.unit.raw(), offset, 0) else {
            return Ok(Value::Null);
        };
        let mut current = Some(node);
        let mut target = None;
        while let Some(n) = current {
            if ast.is::<NamedType>(n)
                || ast.is::<CommentReference>(n)
                || ast.is::<ClassDeclaration>(n)
                || ast.is::<MixinDeclaration>(n)
                || ast.is::<ExtensionTypeDeclaration>(n)
                || ast.is::<EnumDeclaration>(n)
            {
                target = Some(n);
                break;
            }
            current = ast.parent(n);
        }
        let Some(target) = target else {
            return Ok(Value::Null);
        };
        let is_iface = |e: ElementId| {
            matches!(
                e.tag(),
                Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
            )
        };
        let element = if ast.is::<NamedType>(target) {
            unit.tables
                .annotation_type
                .get(target)
                .and_then(|&t| match *ctx.ty(t) {
                    dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
                    _ => None,
                })
        } else if let Some(c) = ast.cast::<CommentReference>(target) {
            let expression = ast[c].expression.raw();
            if ast.is::<Identifier>(expression) {
                let element = unit.tables.element.get(expression).copied().or_else(|| {
                    let p = ast.cast::<PrefixedIdentifier>(expression)?;
                    unit.tables.element.get(ast[p].identifier.raw()).copied()
                });
                element
                    .map(|e| dartr_typesystem::member::base_element(&ctx, e))
                    .filter(|&e| is_iface(e))
            } else {
                None
            }
        } else {
            dartr_resolver::error::support::declared_element(&ctx, &unit.tables, target)
                .filter(|&e| is_iface(e))
        };
        let Some(element) = element else {
            return Ok(Value::Null);
        };
        match lsp_type_hierarchy_item(&ctx, element) {
            Some(item) => Ok(json!([item])),
            None => Ok(Value::Null),
        }
    }

    fn lsp_type_hierarchy_target(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Option<SElem>> {
        use dartr_server::mapping::{ResponseError, codes};
        let item = params.get("item").cloned().unwrap_or(Value::Null);
        let uri = item.get("uri").and_then(Value::as_str).unwrap_or_default();
        let path = self.lsp_path_of_uri(uri)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let Some(reference) = item.pointer("/data/ref").and_then(Value::as_str) else {
            return Err(ResponseError::new(
                codes::INVALID_PARAMS,
                "TypeHierarchyItem is missing the data field",
            ));
        };
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let element = lsp_locate_element(&ctx, reference).filter(|&e| {
            matches!(
                e.tag(),
                Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
            )
        });
        Ok(element.map(|id| SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id,
        }))
    }

    fn lsp_type_hierarchy_supertypes(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        let Some(target) = self.lsp_type_hierarchy_target(params)? else {
            return Ok(Value::Null);
        };
        let items = target.with(|ctx| {
            let interface = target.id.cast::<dartr_element::InterfaceElement>().unwrap();
            let data = ctx.interface(interface);
            let mut types: Vec<dartr_element::TypeId> = Vec::new();
            if let Some(s) = data.supertype.get() {
                types.push(s);
            }
            if let Some(mixin) = target.id.cast::<dartr_element::MixinElement>()
                && let Some(list) = ctx.get(mixin).superclass_constraints.get()
            {
                types.extend(ctx.list(list).iter().copied());
            }
            if let Some(list) = data.interfaces.get() {
                types.extend(ctx.list(list).iter().copied());
            }
            if let Some(list) = data.mixins.get() {
                types.extend(ctx.list(list).iter().copied());
            }
            types
                .into_iter()
                .filter_map(|t| match *ctx.ty(t) {
                    dartr_element::TypeKind::Interface { element, .. } => {
                        lsp_type_hierarchy_item(ctx, element.raw())
                    }
                    _ => None,
                })
                .collect::<Vec<Value>>()
        });
        Ok(Value::Array(items))
    }

    fn lsp_type_hierarchy_subtypes(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        let Some(target) = self.lsp_type_hierarchy_target(params)? else {
            return Ok(Value::Null);
        };
        let Some(collection) = &self.collection else {
            return Ok(json!([]));
        };
        let mut engine = SearchEngine {
            collection,
            excluded: &self.excluded,
            session: &mut self.driver_session,
            owned: &mut self.owned_files,
            search_scope: &mut self.search_scope,
            search_words: &mut self.search_words,
            indexes: &mut self.indexes,
        };
        let Some(hierarchy) = engine.compute_type_hierarchy(&target, false) else {
            return Ok(json!([]));
        };
        let Some(first) = hierarchy.first() else {
            return Ok(json!([]));
        };
        let mut items = Vec::new();
        for &sub_idx in &first.subclasses {
            let Some(sub_item) = hierarchy.get(sub_idx as usize) else {
                continue;
            };
            let Some(loc) = &sub_item.class_element.location else {
                continue;
            };
            if let Some((resolved, el)) = self.get_element_at_offset(&loc.file, loc.offset) {
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                if let Some(val) = lsp_type_hierarchy_item(&ctx, el) {
                    items.push(val);
                }
            }
        }
        Ok(Value::Array(items))
    }

    fn lsp_workspace_symbol(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        let query = params
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if query.is_empty() {
            return Ok(json!([]));
        }
        let supported: Vec<i64> = match self
            .lsp_capabilities
            .raw
            .pointer("/workspace/symbol/symbolKind/valueSet")
            .and_then(Value::as_array)
        {
            Some(list) => list.iter().filter_map(Value::as_i64).collect(),
            None => dartr_server::mapping::DEFAULT_SYMBOL_KINDS.collect(),
        };
        let decls = self.get_element_declarations("", None, None);
        let mut matcher = dartr_server::fuzzy::FuzzyMatcher::new(&query);
        let mut out = Vec::new();
        for d in decls.declarations {
            if out.len() >= 500 {
                break;
            }
            let container = d.class_name.clone().or(d.mixin_name.clone());
            let filtered = if d.kind == protocol::ElementKind::CONSTRUCTOR {
                let c = container.clone().unwrap_or_else(|| "<null>".into());
                if d.name.is_empty() || d.name == "new" {
                    c
                } else {
                    format!("{c}.{}", d.name)
                }
            } else {
                d.name.clone()
            };
            if matcher.score(&filtered) < 0.0 {
                continue;
            }
            let Some(file) = decls.files.get(d.file_index as usize) else {
                continue;
            };
            let Some(content) = fs::read_string(file) else {
                continue;
            };
            let lines = LineInfo::from_content(&content);
            let full_name = if d.kind == protocol::ElementKind::CONSTRUCTOR {
                let c = container.clone().unwrap_or_else(|| "null".to_string());
                if d.name.is_empty() || d.name == "new" {
                    c
                } else {
                    format!("{c}.{}", d.name)
                }
            } else {
                d.name.clone()
            };
            let params_str = match d.parameters.as_deref() {
                Some("") | None => "",
                Some("()") => "()",
                Some(_) => "(…)",
            };
            let prefs: &[i64] = match d.kind {
                protocol::ElementKind::CLASS | protocol::ElementKind::ClassTypeAlias => &[5],
                protocol::ElementKind::CONSTRUCTOR => &[9],
                protocol::ElementKind::ENUM => &[10],
                protocol::ElementKind::EnumConstant => &[22, 10],
                protocol::ElementKind::EXTENSION | protocol::ElementKind::ExtensionType => &[5],
                protocol::ElementKind::FIELD => &[8],
                protocol::ElementKind::FUNCTION => &[12],
                protocol::ElementKind::GETTER | protocol::ElementKind::SETTER => &[7],
                protocol::ElementKind::METHOD => &[6],
                protocol::ElementKind::MIXIN => &[5],
                protocol::ElementKind::TypeAlias => &[5],
                _ => &[13],
            };
            let kind = prefs
                .iter()
                .copied()
                .find(|k| supported.contains(k))
                .unwrap_or(19);
            let mut symbol = json!({
                "name": format!("{full_name}{params_str}"),
                "kind": kind,
                "location": {
                    "uri": dartr_server::uri::path_to_uri(file),
                    "range": dartr_server::mapping::to_range(
                        &lines,
                        d.code_offset as u32,
                        d.code_length as u32,
                    ),
                },
            });
            if let Some(c) = container {
                symbol["containerName"] = json!(c);
            }
            out.push(symbol);
        }
        Ok(Value::Array(out))
    }

    fn lsp_call_item_json(&self, item: &LspCallItem) -> Option<Value> {
        let content = fs::read_string(&item.file)?;
        let lines = LineInfo::from_content(&content);
        let supported: Vec<i64> = match self
            .lsp_capabilities
            .raw
            .pointer("/textDocument/documentSymbol/symbolKind/valueSet")
            .and_then(Value::as_array)
        {
            Some(list) => list.iter().filter_map(Value::as_i64).collect(),
            None => dartr_server::mapping::DEFAULT_SYMBOL_KINDS.collect(),
        };
        let mut kind = item.kind.symbol_kind();
        if let Some(k) = kind
            && !supported.contains(&k)
        {
            kind = if k == 1 { Some(2) } else { None };
        }
        let mut v = json!({
            "name": item.display_name,
            "kind": kind.unwrap_or(19),
            "uri": dartr_server::uri::path_to_uri(&item.file),
            "range": dartr_server::mapping::to_range(&lines, item.code_range.0, item.code_range.1),
            "selectionRange": dartr_server::mapping::to_range(&lines, item.name_range.0, item.name_range.1),
        });
        if let Some(c) = &item.container_name {
            v["detail"] = json!(c);
        }
        Some(v)
    }

    fn lsp_prepare_call_hierarchy(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_server::mapping::{ResponseError, codes};
        if !lsp_is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let element = lsp_call_target_node(&unit.ast, unit.unit.raw(), offset)
            .and_then(|n| lsp_call_element_of_node(&u, n))
            .filter(|&e| dartr_server::element_locator::is_executable(e));
        let Some(item) = element.and_then(|e| lsp_call_item(&ctx, e)) else {
            return Ok(Value::Null);
        };
        match self.lsp_call_item_json(&item) {
            Some(v) => Ok(json!([v])),
            None => Err(ResponseError::new(
                codes::INTERNAL_ERROR,
                format!(
                    "Call Hierarchy target was in an unavailable file: {} in {}",
                    item.display_name, item.file
                ),
            )),
        }
    }

    fn lsp_call_target(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Option<LspCallTarget>> {
        use dartr_server::mapping::{ResponseError, codes, read_position, to_offset};
        let item = params.get("item").cloned().unwrap_or(Value::Null);
        let uri = item.get("uri").and_then(Value::as_str).unwrap_or_default();
        let path = self.lsp_path_of_uri(uri)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let read = |key: &str| -> Option<(u32, u32)> {
            let start = item
                .pointer(&format!("/{key}/start"))
                .and_then(read_position)?;
            let end = item
                .pointer(&format!("/{key}/end"))
                .and_then(read_position)?;
            let s = to_offset(&line_info, start.0, start.1, false).ok()?;
            let e = to_offset(&line_info, end.0, end.1, false).ok()?;
            Some((s, e.saturating_sub(s)))
        };
        let (Some(name_range), Some(_)) = (read("selectionRange"), read("range")) else {
            return Err(ResponseError::new(
                codes::CONTENT_MODIFIED,
                "Content was modified since Call Hierarchy node was produced",
            ));
        };
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let kind =
            LspCallKind::from_symbol_kind(item.get("kind").and_then(Value::as_i64).unwrap_or(0));
        let found = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            let u = dartr_server::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            let node = lsp_call_target_node(&unit.ast, unit.unit.raw(), name_range.0);
            node.and_then(|n| lsp_call_element_of_node(&u, n).map(|e| (n, e)))
                .filter(|(_, e)| lsp_call_display_name(&ctx, *e) == name)
        };
        Ok(found.map(|(n, e)| (resolved, n, e, kind)))
    }

    fn lsp_call_hierarchy_incoming(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        let Some((resolved, _, mut element, kind)) = self.lsp_call_target(params)? else {
            return Ok(json!([]));
        };
        if matches!(
            element.tag(),
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
        ) && kind == LspCallKind::Constructor
        {
            let unnamed = {
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                let interface = element.cast::<dartr_element::InterfaceElement>().unwrap();
                ctx.interface(interface)
                    .constructors
                    .iter()
                    .map(|c| c.raw())
                    .find(|&c| {
                        dartr_resolver::error::support::display_name(&ctx, c).is_empty()
                            || ctx
                                .element_data(c)
                                .and_then(|d| d.name)
                                .map(|n| ctx.name_str(n) == "new")
                                .unwrap_or(false)
                    })
            };
            match unnamed {
                Some(c) => element = c,
                None => return Ok(json!([])),
            }
        }
        if !dartr_server::element_locator::is_executable(element) {
            return Ok(json!([]));
        }
        let target = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        let Some(collection) = &self.collection else {
            return Ok(json!([]));
        };
        let matches = {
            let mut engine = SearchEngine {
                collection,
                excluded: &self.excluded,
                session: &mut self.driver_session,
                owned: &mut self.owned_files,
                search_scope: &mut self.search_scope,
                search_words: &mut self.search_words,
                indexes: &mut self.indexes,
            };
            engine.element_reference_matches(&target)
        };
        let mut groups: Vec<LspIncomingCallGroup> = Vec::new();
        for m in matches {
            let Some(collection) = &self.collection else {
                continue;
            };
            let Some(lib) = self
                .driver_session
                .resolved_library_in(collection, m.context, &m.path)
            else {
                continue;
            };
            let Some(unit_idx) = lib.unit_index(&m.path) else {
                continue;
            };
            let r = ResolvedUnitRef {
                library: lib,
                index: unit_idx,
            };
            let sink = NoopSink;
            let ctx = r.ctx(&sink);
            let unit = r.unit();
            let u = dartr_server::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            let Some(enclosing) = lsp_enclosing_element(&u, unit.unit.raw(), m.offset) else {
                continue;
            };
            let Some(container) = lsp_container_of(&ctx, enclosing) else {
                continue;
            };
            let Some(key) = dartr_server::index::element_key(&ctx, container) else {
                continue;
            };
            let identity = (m.context, key);
            let ast = &unit.ast;
            let mut range = (m.offset, m.length);
            if let Some(node) = ast.node_covering(unit.unit.raw(), m.offset, 0) {
                let parent = ast.parent(node);
                if ast.is::<SimpleIdentifier>(node)
                    && let Some(mi) = parent.and_then(|p| ast.cast::<MethodInvocation>(p))
                {
                    let n = ast[mi].method_name.raw();
                    range = (ast.offset(n), ast.length(n));
                } else if m.length == 0 {
                    range = (ast.offset(node), ast.length(node));
                }
            }
            match groups.iter_mut().find(|g| g.0 == identity) {
                Some(g) => g.2.push(range),
                None => {
                    let Some(item) = lsp_call_item(&ctx, container) else {
                        continue;
                    };
                    groups.push((identity, item, vec![range]));
                }
            }
        }
        let mut out = Vec::new();
        for (_, item, ranges) in groups {
            let Some(content) = fs::read_string(&item.file) else {
                continue;
            };
            let lines = LineInfo::from_content(&content);
            let Some(from) = self.lsp_call_item_json(&item) else {
                continue;
            };
            let from_ranges: Vec<Value> = ranges
                .iter()
                .map(|r| dartr_server::mapping::to_range(&lines, r.0, r.1))
                .collect();
            out.push(json!({"from": from, "fromRanges": from_ranges}));
        }
        Ok(Value::Array(out))
    }

    fn lsp_call_hierarchy_outgoing(
        &mut self,
        params: &Value,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        let Some((resolved, mut node, _, _)) = self.lsp_call_target(params)? else {
            return Ok(json!([]));
        };
        let local_lines = resolved.line_info().clone();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(node)
            && let Some(body) = lsp_primary_constructor_body(ast, pc)
        {
            node = body;
        }
        if !(ast.is::<FunctionDeclaration>(node)
            || ast.is::<ConstructorDeclaration>(node)
            || ast.is::<MethodDeclaration>(node)
            || ast.is::<PrimaryConstructorBody>(node))
        {
            return Ok(json!([]));
        }
        let mut visitor = LspOutboundCalls {
            unit: &u,
            root: node,
            nodes: Vec::new(),
        };
        ast.accept(node, &mut visitor);
        let mut groups: Vec<LspOutgoingCallGroup> = Vec::new();
        for n in visitor.nodes {
            let Some(target) = lsp_call_element_of_node(&u, n) else {
                continue;
            };
            let range = lsp_range_for_node(ast, n);
            match groups.iter_mut().find(|g| g.0 == target) {
                Some(g) => g.2.push(range),
                None => {
                    let Some(item) = lsp_call_item(&ctx, target) else {
                        continue;
                    };
                    groups.push((target, item, vec![range]));
                }
            }
        }
        let mut out = Vec::new();
        for (_, item, ranges) in groups {
            if fs::read_string(&item.file).is_none() {
                continue;
            }
            let Some(to) = self.lsp_call_item_json(&item) else {
                continue;
            };
            let from_ranges: Vec<Value> = ranges
                .iter()
                .map(|r| dartr_server::mapping::to_range(&local_lines, r.0, r.1))
                .collect();
            out.push(json!({"to": to, "fromRanges": from_ranges}));
        }
        Ok(Value::Array(out))
    }

    fn lsp_super(&mut self, params: &Value) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        if !lsp_is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let can_have_super = |n: NodeId| {
            let mut test = n;
            if ast.is::<VariableDeclaration>(test)
                && let Some(p) = ast.parent(test)
                && ast.is::<VariableDeclarationList>(p)
            {
                test = p;
            }
            if ast.is::<VariableDeclarationList>(test) {
                if let Some(p) = ast.parent(test) {
                    test = p;
                } else {
                    return false;
                }
            }
            ast.is::<ClassDeclaration>(test)
                || ast.is::<ClassMember>(test)
                || ast.is::<PrimaryConstructorDeclaration>(test)
        };
        let mut cur = ast.node_covering(unit.unit.raw(), offset, 0);
        let mut target_node = None;
        while let Some(n) = cur {
            if can_have_super(n) {
                target_node = Some(n);
                break;
            }
            cur = ast.parent(n);
        }
        let Some(node) = target_node else {
            return Ok(Value::Null);
        };
        let Some(mut element) = u.locate(node) else {
            return Ok(Value::Null);
        };
        if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(node)
            && element.tag() == Tag::Class
            && offset > ast.tokens.get(ast[pc].type_name).end()
            && let Some(c) =
                dartr_resolver::scope_context::primary_constructor_of(&ctx, element.cast().unwrap())
        {
            element = c.raw();
        }
        let target_fragment = if let Some(ctor) =
            element.cast::<dartr_element::ConstructorElement>()
        {
            let super_ctor = ctx
                .get(ctor)
                .super_constructor
                .get()
                .map(|c| dartr_typesystem::member::base_element(&ctx, c));
            lsp_last_fragment(&ctx, super_ctor)
        } else if let Some(iface) = element.cast::<dartr_element::InterfaceElement>() {
            ctx.interface(iface)
                .supertype
                .get()
                .and_then(|t| match *ctx.ty(t) {
                    dartr_element::TypeKind::Interface { element, .. } => {
                        ctx.element_data(element.raw()).map(|d| d.first_fragment)
                    }
                    _ => None,
                })
        } else {
            let (super_els, iface_els) = crate::overrides::find_overridden_elements(&ctx, element);
            let member = super_els.first().or_else(|| iface_els.first()).copied();
            lsp_last_fragment(&ctx, member)
        };
        Ok(target_fragment
            .and_then(|f| lsp_fragment_to_location(&ctx, f))
            .unwrap_or(Value::Null))
    }

    fn lsp_augmentation(
        &mut self,
        params: &Value,
        next: bool,
    ) -> dartr_server::mapping::ErrorOr<Value> {
        use dartr_ast::*;
        if !lsp_is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.lsp_path_of_doc(params)?;
        let resolved = self.lsp_require_resolved(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.lsp_position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let mut cur = ast.node_covering(unit.unit.raw(), offset, 0);
        let mut decl = None;
        while let Some(n) = cur {
            if ast.is::<Declaration>(n) {
                decl = Some(n);
                break;
            }
            cur = ast.parent(n);
        }
        let target_fragment = decl
            .and_then(|d| unit.tables.declared_fragment.get(d).copied())
            .and_then(|f| {
                let data = ctx.fragment_data(f)?;
                if next {
                    data.next_fragment
                } else {
                    data.previous_fragment
                }
            });
        Ok(target_fragment
            .and_then(|f| lsp_fragment_to_location(&ctx, f))
            .unwrap_or(Value::Null))
    }
}
impl<W: Write> Drop for Server<W> {
    fn drop(&mut self) {
        for file in self.overlays.keys() {
            fs::set_overlay(file, None);
        }
    }
}

type LspCallTarget = (ResolvedUnitRef, dartr_ast::NodeId, ElementId, LspCallKind);
type LspIncomingCallGroup = (
    (usize, dartr_server::index::ElementKey),
    LspCallItem,
    Vec<(u32, u32)>,
);
type LspOutgoingCallGroup = (ElementId, LspCallItem, Vec<(u32, u32)>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LspCallKind {
    Class,
    Constructor,
    Extension,
    File,
    Function,
    Method,
    Mixin,
    Property,
    Unknown,
}

impl LspCallKind {
    fn for_element(e: ElementId) -> LspCallKind {
        match e.tag() {
            Tag::Class => LspCallKind::Class,
            Tag::Library => LspCallKind::File,
            Tag::Constructor => LspCallKind::Constructor,
            Tag::Extension => LspCallKind::Extension,
            Tag::TopLevelFunction | Tag::LocalFunction => LspCallKind::Function,
            Tag::Getter | Tag::Setter => LspCallKind::Property,
            Tag::Method => LspCallKind::Method,
            Tag::Mixin => LspCallKind::Mixin,
            _ => LspCallKind::Unknown,
        }
    }

    fn symbol_kind(self) -> Option<i64> {
        Some(match self {
            LspCallKind::Class | LspCallKind::Extension | LspCallKind::Mixin => 5,
            LspCallKind::Constructor => 9,
            LspCallKind::File => 1,
            LspCallKind::Function => 12,
            LspCallKind::Method => 6,
            LspCallKind::Property => 7,
            LspCallKind::Unknown => return None,
        })
    }

    fn from_symbol_kind(kind: i64) -> LspCallKind {
        match kind {
            5 => LspCallKind::Class,
            9 => LspCallKind::Constructor,
            1 => LspCallKind::File,
            12 => LspCallKind::Function,
            6 => LspCallKind::Method,
            7 => LspCallKind::Property,
            _ => LspCallKind::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
struct LspCallItem {
    display_name: String,
    container_name: Option<String>,
    kind: LspCallKind,
    file: String,
    name_range: (u32, u32),
    code_range: (u32, u32),
}

fn lsp_container_of(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> Option<ElementId> {
    let mut current = Some(element);
    while let Some(e) = current {
        if matches!(
            e.tag(),
            Tag::Class
                | Tag::Library
                | Tag::Constructor
                | Tag::Enum
                | Tag::Extension
                | Tag::ExtensionType
                | Tag::TopLevelFunction
                | Tag::LocalFunction
                | Tag::Getter
                | Tag::Method
                | Tag::Mixin
                | Tag::Setter
        ) {
            return Some(e);
        }
        current = ctx.element_data(e).and_then(|d| d.enclosing).or_else(|| {
            let first = ctx.element_data(e)?.first_fragment;
            let enclosing = ctx.fragment_data(first)?.enclosing_fragment?;
            ctx.fragment_data(enclosing)?.element.try_get().copied()
        });
    }
    None
}

fn lsp_call_display_name(ctx: &dartr_element::Ctx<'_>, e: ElementId) -> String {
    match e.tag() {
        Tag::Library => {
            let library = e.cast::<dartr_element::LibraryElement>().unwrap();
            let path = ctx
                .fragment(ctx.get(library).first_fragment())
                .source
                .path
                .to_string();
            path.rsplit('/').next().unwrap_or_default().to_string()
        }
        Tag::Getter => format!(
            "get {}",
            dartr_resolver::error::support::display_name(ctx, e)
        ),
        Tag::Setter => format!(
            "set {}",
            dartr_resolver::error::support::display_name(ctx, e)
        ),
        _ => dartr_resolver::error::support::display_name(ctx, e),
    }
}

fn lsp_call_item(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> Option<LspCallItem> {
    let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, element);
    let first = ctx.element_data(non_synthetic)?.first_fragment;
    let data = ctx.fragment_data(first)?;
    let code_range = (data.code_offset.unwrap_or(0), data.code_length.unwrap_or(0));
    let name_range = match (data.name_offset, data.name) {
        (Some(o), Some(n)) => (o, ctx.name_str(n).encode_utf16().count() as u32),
        _ => {
            let mut range = (0, 0);
            if let Some(f) = first.cast::<dartr_element::ConstructorFragment>() {
                let c = ctx.fragment(f);
                if let (Some(o), Some(n)) = (c.type_name_offset, c.type_name) {
                    range = (o, ctx.name_str(n).encode_utf16().count() as u32);
                } else if let Some(o) = c.new_keyword_offset {
                    range = (o, 3);
                } else if let Some(o) = c.factory_keyword_offset {
                    range = (o, 7);
                }
            }
            range
        }
    };
    let file =
        dartr_server::navigation::fragment_path(ctx, ctx.element_data(element)?.first_fragment)?;
    let enclosing = ctx
        .element_data(element)
        .and_then(|d| d.enclosing)
        .or_else(|| {
            let first = ctx.element_data(element)?.first_fragment;
            let enclosing = ctx.fragment_data(first)?.enclosing_fragment?;
            ctx.fragment_data(enclosing)?.element.try_get().copied()
        });
    let container_name = enclosing
        .and_then(|e| lsp_container_of(ctx, e))
        .map(|c| lsp_call_display_name(ctx, c));
    Some(LspCallItem {
        display_name: lsp_call_display_name(ctx, element),
        container_name,
        kind: LspCallKind::for_element(element),
        file,
        name_range,
        code_range,
    })
}

fn lsp_call_element_of_node(
    unit: &dartr_server::element_locator::Unit<'_, '_>,
    node: dartr_ast::NodeId,
) -> Option<ElementId> {
    use dartr_ast::*;
    let ast = unit.ast;
    let ctx = unit.ctx;
    let parent = ast.parent(node);
    let element_of = |n: NodeId| {
        unit.tables
            .element
            .get(n)
            .map(|&e| dartr_typesystem::member::base_element(ctx, e))
    };
    let mut node = node;
    if ast.is::<NamedType>(node) && parent.is_some_and(|p| ast.is::<ConstructorName>(p)) {
        return element_of(parent.unwrap());
    } else if ast.is::<ConstructorName>(node) {
        return element_of(node);
    } else if let Some(p) = ast.cast::<PropertyAccess>(node) {
        node = ast[p].property_name.raw();
    }
    let mut element = unit.locate(node)?;
    if element.tag() == Tag::Class && ast.is::<PrimaryConstructorDeclaration>(node) {
        element =
            dartr_resolver::scope_context::primary_constructor_of(ctx, element.cast()?)?.raw();
    }
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && dartr_resolver::element_metadata::is_origin_variable(ctx, element)
    {
        return None;
    }
    Some(element)
}

fn lsp_call_target_node(
    ast: &dartr_ast::Ast,
    root: dartr_ast::NodeId,
    offset: u32,
) -> Option<dartr_ast::NodeId> {
    use dartr_ast::*;
    let node = ast.node_covering(root, offset, 0)?;
    let parent = ast.parent(node);
    if ast.is::<NamedType>(node)
        && let Some(cn) = parent.and_then(|p| ast.cast::<ConstructorName>(p))
        && let Some(name) = ast[cn].name
        && offset < ast.offset(name.raw())
    {
        return None;
    }
    if ast.is::<Identifier>(node)
        && let Some(cd) = parent.and_then(|p| ast.cast::<ConstructorDeclaration>(p))
    {
        match ast[cd].name {
            Some(name) if offset < ast.tokens.get(name).offset => return None,
            None => return parent,
            _ => {}
        }
    }
    if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(node)
        && let Some(cn) = ast[pc].constructor_name
        && offset < ast.tokens.get(ast[cn].name).offset
    {
        return None;
    }
    if ast.is::<PrimaryConstructorName>(node) {
        return parent;
    }
    Some(node)
}

struct LspOutboundCalls<'u, 'c, 'a> {
    unit: &'u dartr_server::element_locator::Unit<'c, 'a>,
    root: dartr_ast::NodeId,
    nodes: Vec<dartr_ast::NodeId>,
}

impl LspOutboundCalls<'_, '_, '_> {
    fn collect(&mut self, node: dartr_ast::NodeId) {
        if !self.nodes.contains(&node) {
            self.nodes.push(node);
        }
    }
}

impl dartr_ast::AstVisitor for LspOutboundCalls<'_, '_, '_> {
    fn visit_constructor_name(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::ConstructorName>,
    ) {
        self.collect(ast[node].name.map(|n| n.raw()).unwrap_or(node.raw()));
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::DotShorthandConstructorInvocation>,
    ) {
        self.collect(ast[node].constructor_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_invocation(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::DotShorthandInvocation>,
    ) {
        self.collect(ast[node].member_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::DotShorthandPropertyAccess>,
    ) {
        self.collect(ast[node].property_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_function_declaration(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::FunctionDeclaration>,
    ) {
        if node.raw() == self.root {
            ast.visit_children(node.raw(), self);
        }
    }

    fn visit_function_reference(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::FunctionReference>,
    ) {
        self.collect(node.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_method_invocation(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::MethodInvocation>,
    ) {
        self.collect(ast[node].method_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_prefixed_identifier(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::PrefixedIdentifier>,
    ) {
        if !ast
            .parent(node.raw())
            .is_some_and(|p| ast.is::<dartr_ast::NamedType>(p))
        {
            self.collect(ast[node].identifier.raw());
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_property_access(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::PropertyAccess>,
    ) {
        self.collect(ast[node].property_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_simple_identifier(
        &mut self,
        ast: &dartr_ast::Ast,
        node: dartr_ast::Id<dartr_ast::SimpleIdentifier>,
    ) {
        let element = self
            .unit
            .tables
            .element
            .get(node.raw())
            .map(|&e| dartr_typesystem::member::base_element(self.unit.ctx, e));
        if element.is_some_and(|e| matches!(e.tag(), Tag::LocalFunction | Tag::TopLevelFunction))
            && !lsp_in_declaration_context(ast, node)
        {
            self.collect(node.raw());
        }
        ast.visit_children(node.raw(), self);
    }
}

fn lsp_range_for_node(ast: &dartr_ast::Ast, node: dartr_ast::NodeId) -> (u32, u32) {
    use dartr_ast::*;
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        let n = ast[m].method_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        let n = ast[i].constructor_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    if let Some(p) = ast.cast::<PropertyAccess>(node) {
        let n = ast[p].property_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    (ast.offset(node), ast.length(node))
}

fn lsp_enclosing_element(
    unit: &dartr_server::element_locator::Unit<'_, '_>,
    root: dartr_ast::NodeId,
    offset: u32,
) -> Option<ElementId> {
    use dartr_ast::*;
    let ast = unit.ast;
    let mut node = ast.node_covering(root, offset, 0);
    let top_level = |n: NodeId| ast.parent(n).is_some_and(|p| ast.is::<CompilationUnit>(p));
    let member_variable = |n: NodeId| {
        ast.parent(n)
            .and_then(|list| ast.parent(list))
            .is_some_and(|d| {
                ast.is::<FieldDeclaration>(d) || ast.is::<TopLevelVariableDeclaration>(d)
            })
    };
    while let Some(n) = node {
        if (ast.is::<ConstructorDeclaration>(n)
            || (ast.is::<FunctionDeclaration>(n) && top_level(n))
            || ast.is::<MethodDeclaration>(n)
            || ast.is::<ClassDeclaration>(n)
            || ast.is::<ClassTypeAlias>(n)
            || ast.is::<EnumDeclaration>(n)
            || ast.is::<ExtensionDeclaration>(n)
            || ast.is::<ExtensionTypeDeclaration>(n)
            || ast.is::<MixinDeclaration>(n)
            || (ast.is::<VariableDeclaration>(n) && member_variable(n)))
            && let Some(e) = unit.declared_element(n)
        {
            return Some(e);
        }
        node = ast.parent(n);
    }
    let unit_node = ast.cast::<CompilationUnit>(root)?;
    let fragment = unit.tables.declared_fragment.get(unit_node.raw())?;
    let library = fragment.cast::<dartr_element::LibraryFragment>()?;
    Some(unit.ctx.fragment(library).library.raw())
}

fn lsp_primary_constructor_body(
    ast: &dartr_ast::Ast,
    node: dartr_ast::Id<dartr_ast::PrimaryConstructorDeclaration>,
) -> Option<dartr_ast::NodeId> {
    use dartr_ast::*;
    let declaration = ast.parent(node.raw())?;
    let body = if let Some(c) = ast.cast::<ClassDeclaration>(declaration) {
        ast[c].body.raw()
    } else {
        let e = ast.cast::<ExtensionTypeDeclaration>(declaration)?;
        ast[e].body.raw()
    };
    let block = ast.cast::<BlockClassBody>(body)?;
    ast.list_raw(ast[block].members)
        .iter()
        .copied()
        .find(|&m| ast.is::<PrimaryConstructorBody>(m))
}

fn lsp_in_declaration_context(
    ast: &dartr_ast::Ast,
    node: dartr_ast::Id<dartr_ast::SimpleIdentifier>,
) -> bool {
    use dartr_ast::*;
    let Some(parent) = ast.parent(node.raw()) else {
        return false;
    };
    if let Some(i) = ast.cast::<ImportDirective>(parent) {
        return ast[i].prefix == Some(node);
    }
    if ast.is::<Label>(parent) {
        return ast
            .parent(parent)
            .is_some_and(|g| ast.is::<Statement>(g) || ast.is::<SwitchMember>(g));
    }
    false
}

fn lsp_last_fragment(
    ctx: &dartr_element::Ctx<'_>,
    element: Option<ElementId>,
) -> Option<dartr_element::FragmentId> {
    let element = element?;
    let mut frag = ctx.element_data(element)?.first_fragment;
    while let Some(next) = ctx.fragment_data(frag).and_then(|d| d.next_fragment) {
        frag = next;
    }
    Some(frag)
}

fn lsp_fragment_to_location(
    ctx: &dartr_element::Ctx<'_>,
    fragment: dartr_element::FragmentId,
) -> Option<Value> {
    let target = dartr_server::navigation::fragment_target(ctx, fragment)?;
    if target.length == 0 {
        return None;
    }
    let content = fs::read_string(&target.file)?;
    let lines = LineInfo::from_content(&content);
    Some(json!({
        "uri": dartr_server::uri::path_to_uri(&target.file),
        "range": dartr_server::mapping::to_range(&lines, target.offset, target.length),
    }))
}

fn default_legacy_lsp_capabilities_json() -> Value {
    json!({
        "textDocument": {
            "hover": {
                "contentFormat": ["markdown"]
            }
        },
        "workspace": {
            "workspaceEdit": {
                "documentChanges": true
            }
        }
    })
}

fn default_legacy_lsp_capabilities() -> dartr_server::capabilities::ClientCapabilities {
    dartr_server::capabilities::ClientCapabilities::new(default_legacy_lsp_capabilities_json())
}

fn validate_lsp_request_message(msg: &Value) -> std::result::Result<(), &'static str> {
    let Some(obj) = msg.as_object() else {
        return Err("");
    };
    if let Some(crt) = obj.get("clientRequestTime")
        && !crt.is_null()
        && !crt.is_i64()
    {
        return Err("clientRequestTime must be of type int");
    }
    match obj.get("id") {
        None => return Err("id must not be undefined"),
        Some(v) if v.is_null() => return Err("id must not be null"),
        Some(v) if !v.is_i64() && !v.is_string() => {
            return Err("id must be of type Either2<int, String>");
        }
        _ => {}
    }
    match obj.get("jsonrpc") {
        None => return Err("jsonrpc must not be undefined"),
        Some(v) if v.is_null() => return Err("jsonrpc must not be null"),
        Some(v) if !v.is_string() => return Err("jsonrpc must be of type String"),
        _ => {}
    }
    match obj.get("method") {
        None => return Err("method must not be undefined"),
        Some(v) if v.is_null() => return Err("method must not be null"),
        Some(v) if !v.is_string() => return Err(""),
        _ => {}
    }
    Ok(())
}

fn lsp_is_dart_document(params: &Value) -> bool {
    params
        .get("textDocument")
        .and_then(|d| d.get("uri"))
        .and_then(Value::as_str)
        .map(|u| u.split(['?', '#']).next().unwrap_or(u).ends_with(".dart"))
        .unwrap_or(false)
}

fn lsp_invalid_params(method: &str) -> dartr_server::mapping::ResponseError {
    dartr_server::mapping::ResponseError::new(
        dartr_server::mapping::codes::INVALID_PARAMS,
        format!("Invalid params for {method}"),
    )
}

fn lsp_markup_content_or_string(formats: &Option<Vec<String>>, content: String) -> Value {
    match formats {
        None => Value::String(content),
        Some(formats) => {
            let markdown = formats.is_empty() || formats.iter().any(|f| f == "markdown");
            let plain = formats.iter().any(|f| f == "plaintext");
            let kind = if plain && !markdown {
                "plaintext"
            } else {
                "markdown"
            };
            json!({"kind": kind, "value": content})
        }
    }
}

fn lsp_element_location(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> Option<String> {
    let library = dartr_resolver::error::support::library_of(ctx, element)?;
    let library_uri = ctx
        .fragment(ctx.get(library).first_fragment())
        .source
        .uri
        .to_string();
    let enclosing = ctx.element_data(element)?.enclosing;
    let lookup =
        |e: ElementId| dartr_typesystem::member::lookup_name(ctx, dartr_element::ElemRef::Base(e));
    if enclosing == Some(library.raw()) {
        let top = lookup(element)?;
        return Some(format!("{library_uri};{top}"));
    }
    let enclosing = enclosing?;
    if ctx.element_data(enclosing)?.enclosing == Some(library.raw()) {
        let member = lookup(element)?;
        let top = lookup(enclosing)?;
        return Some(format!("{library_uri};{top};{member}"));
    }
    None
}

fn lsp_locate_element(ctx: &dartr_element::Ctx<'_>, encoded: &str) -> Option<ElementId> {
    let parts: Vec<&str> = encoded.split(';').collect();
    let (uri, top, member) = match parts.as_slice() {
        [uri, top] => (*uri, *top, None),
        [uri, top, member] => (*uri, *top, Some(*member)),
        _ => return None,
    };
    let library = ctx.world.libraries.get(uri).copied()?;
    let l = ctx.get(library);
    let mut children: Vec<ElementId> = Vec::new();
    children.extend(l.classes.iter().map(|e| e.raw()));
    children.extend(l.enums.iter().map(|e| e.raw()));
    children.extend(l.extensions.iter().map(|e| e.raw()));
    children.extend(l.extension_types.iter().map(|e| e.raw()));
    children.extend(l.getters.iter().map(|e| e.raw()));
    children.extend(l.mixins.iter().map(|e| e.raw()));
    children.extend(l.setters.iter().map(|e| e.raw()));
    children.extend(l.top_level_functions.iter().map(|e| e.raw()));
    children.extend(l.top_level_variables.iter().map(|e| e.raw()));
    children.extend(l.type_aliases.iter().map(|e| e.raw()));
    let lookup =
        |e: ElementId| dartr_typesystem::member::lookup_name(ctx, dartr_element::ElemRef::Base(e));
    let top_el = children
        .into_iter()
        .find(|&e| lookup(e).as_deref() == Some(top))?;
    match member {
        None => Some(top_el),
        Some(m) => {
            let instance = top_el.cast::<dartr_element::InstanceElement>()?;
            let data = ctx.instance(instance);
            let mut members: Vec<ElementId> = Vec::new();
            if let Some(interface) = top_el.cast::<dartr_element::InterfaceElement>() {
                members.extend(
                    ctx.interface(interface)
                        .constructors
                        .iter()
                        .map(|e| e.raw()),
                );
            }
            members.extend(data.fields.iter().map(|e| e.raw()));
            members.extend(data.getters.iter().map(|e| e.raw()));
            members.extend(data.methods.iter().map(|e| e.raw()));
            members.extend(data.setters.iter().map(|e| e.raw()));
            members.extend(data.type_params.iter().map(|e| e.raw()));
            members
                .into_iter()
                .find(|&e| lookup(e).as_deref() == Some(m))
        }
    }
}

fn lsp_type_hierarchy_item(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> Option<Value> {
    let location = lsp_element_location(ctx, element)?;
    let interface = element.cast::<dartr_element::InterfaceElement>()?;
    let this_type = ctx.interface_this_type(interface);
    let name = dartr_element::display_string::type_display_string_with(
        ctx,
        this_type,
        dartr_element::display_string::DisplayOptions::default(),
    );
    let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, element);
    let first = ctx.element_data(non_synthetic)?.first_fragment;
    let data = ctx.fragment_data(first)?;
    let path = dartr_server::navigation::fragment_path(ctx, first)?;
    let content = fs::read_string(&path)?;
    let lines = LineInfo::from_content(&content);
    let name_length = data
        .name
        .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
        .unwrap_or(0);
    let name_offset = data.name_offset.unwrap_or(0);
    let code = (data.code_offset.unwrap_or(0), data.code_length.unwrap_or(0));
    Some(json!({
        "name": name,
        "kind": 5,
        "uri": dartr_server::uri::path_to_uri(&path),
        "range": dartr_server::mapping::to_range(&lines, code.0, code.1),
        "selectionRange": dartr_server::mapping::to_range(&lines, name_offset, name_length),
        "data": {"ref": location},
    }))
}

fn decode_flutter_widget_property_value(
    value: &Value,
    path: &str,
) -> Result<protocol::FlutterWidgetPropertyValue> {
    let Some(obj) = value.as_object() else {
        return Err(RequestFailure::mismatch(
            path,
            "FlutterWidgetPropertyValue",
            value,
        ));
    };
    let bool_value = match obj.get("boolValue") {
        Some(v) if !v.is_null() => Some(boolean(v, &format!("{path}.boolValue"))?),
        _ => None,
    };
    let double_value = match obj.get("doubleValue") {
        Some(v) if !v.is_null() => Some(v.as_f64().ok_or_else(|| {
            RequestFailure::mismatch(&format!("{path}.doubleValue"), "double", v)
        })?),
        _ => None,
    };
    let int_value = match obj.get("intValue") {
        Some(v) if !v.is_null() => Some(integer(v, &format!("{path}.intValue"))?),
        _ => None,
    };
    let string_value = match obj.get("stringValue") {
        Some(v) if !v.is_null() => Some(string(v, &format!("{path}.stringValue"))?.to_owned()),
        _ => None,
    };
    let enum_value = match obj.get("enumValue") {
        Some(v) if !v.is_null() => {
            let e_path = format!("{path}.enumValue");
            let Some(e_obj) = v.as_object() else {
                return Err(RequestFailure::mismatch(
                    &e_path,
                    "FlutterWidgetPropertyValueEnumItem",
                    v,
                ));
            };
            let get = |k: &str| {
                e_obj
                    .get(k)
                    .ok_or_else(|| RequestFailure::mismatch(&e_path, k, v))
            };
            let library_uri =
                string(get("libraryUri")?, &format!("{e_path}.libraryUri"))?.to_owned();
            let class_name = string(get("className")?, &format!("{e_path}.className"))?.to_owned();
            let name = string(get("name")?, &format!("{e_path}.name"))?.to_owned();
            let documentation = match e_obj.get("documentation") {
                Some(d) if !d.is_null() => {
                    Some(string(d, &format!("{e_path}.documentation"))?.to_owned())
                }
                _ => None,
            };
            Some(protocol::FlutterWidgetPropertyValueEnumItem {
                library_uri,
                class_name,
                name,
                documentation,
            })
        }
        _ => None,
    };
    let expression = match obj.get("expression") {
        Some(v) if !v.is_null() => Some(string(v, &format!("{path}.expression"))?.to_owned()),
        _ => None,
    };
    Ok(protocol::FlutterWidgetPropertyValue {
        bool_value,
        double_value,
        int_value,
        string_value,
        enum_value,
        expression,
    })
}

fn hover_target_node(ast: &dartr_ast::Ast, node: dartr_ast::NodeId) -> Option<dartr_ast::NodeId> {
    use dartr_ast::*;
    let parent = ast.parent(node);
    let parent2 = parent.and_then(|p| ast.parent(p));
    if ast.is::<ClassNamePart>(node) {
        return parent;
    }
    if let (Some(p), Some(p2)) = (parent, parent2)
        && ast.is::<NamedType>(p)
        && ast.is::<ConstructorName>(p2)
        && ast
            .parent(p2)
            .is_some_and(|p3| ast.is::<InstanceCreationExpression>(p3))
    {
        return ast.parent(p2);
    }
    if let (Some(p), Some(p2)) = (parent, parent2)
        && ast.is::<ConstructorName>(p)
        && ast.is::<InstanceCreationExpression>(p2)
    {
        return Some(p2);
    }
    if ast.is::<SimpleIdentifier>(node)
        && let Some(p) = parent
        && let Some(c) = ast.cast::<ConstructorDeclaration>(p)
        && ast[c].name.is_some()
    {
        return Some(p);
    }
    if ast.is::<SimpleIdentifier>(node)
        && let Some(p) = parent
        && ast.is::<DotShorthandConstructorInvocation>(p)
    {
        return Some(p);
    }
    Some(node)
}

fn protocol_location(location: &dartr_cli::server::Location) -> protocol::Location {
    protocol::Location {
        file: location.file.clone(),
        offset: location.offset as i64,
        length: location.length as i64,
        start_line: location.start_line.into(),
        start_column: location.start_column.into(),
        end_line: Some(location.end_line.into()),
        end_column: Some(location.end_column.into()),
    }
}
fn error_json(error: &AnalysisError) -> Value {
    wire(protocol::AnalysisError {
        severity: serde_json::from_value(json!(error.severity.name()))
            .expect("diagnostic severity"),
        type_: serde_json::from_value(json!(error.type_)).expect("diagnostic type"),
        location: protocol_location(&error.location),
        message: error.message.clone(),
        code: error.code.clone(),
        // AnalyzerConverter uses true for non-Dart diagnostics; the engine converter uses false.
        has_fix: Some(FileKind::of(&error.location.file) != FileKind::Dart),
        correction: error.correction.clone(),
        url: error.url.clone(),
        context_messages: if error.context_messages.is_empty() {
            None
        } else {
            Some(
                error
                    .context_messages
                    .iter()
                    .map(|message| protocol::DiagnosticMessage {
                        message: message.message.clone(),
                        location: protocol_location(&message.location),
                    })
                    .collect(),
            )
        },
    })
}
fn folding_kind(kind: dartr_server::computer::folding::FoldingKind) -> &'static str {
    use dartr_server::computer::folding::FoldingKind::*;
    match kind {
        Annotations => "ANNOTATIONS",
        Block => "BLOCK",
        ClassBody => "CLASS_BODY",
        Comment => "COMMENT",
        Directives => "DIRECTIVES",
        DocumentationComment => "DOCUMENTATION_COMMENT",
        FileHeader => "FILE_HEADER",
        FunctionBody => "FUNCTION_BODY",
        Invocation => "INVOCATION",
        Literal => "LITERAL",
        Parameters => "PARAMETERS",
    }
}

enum OverlayChange {
    Add(String),
    Change(Vec<(usize, usize, String)>),
    Remove,
}
impl OverlayChange {
    fn decode(value: &Value, path: &str) -> Result<Self> {
        if !value.is_object() {
            return Err(RequestFailure::mismatch(path, "Map", value));
        }
        let get = |key: &str| {
            value.get(key).ok_or_else(|| {
                if key == "type" {
                    RequestFailure::missing(path, key)
                } else {
                    RequestFailure::mismatch(path, key, value)
                }
            })
        };
        let type_ = string(get("type")?, &format!("{path}[\"type\"]"))?;
        let change = match type_ {
            "add" => Ok(Self::Add(
                string(get("content")?, &format!("{path}.content"))?.to_owned(),
            )),
            "remove" => Ok(Self::Remove),
            "change" => {
                let edits = get("edits")?;
                let empty_edits = Vec::new();
                let edits = if edits.is_null() {
                    &empty_edits
                } else {
                    edits.as_array().ok_or_else(|| {
                        RequestFailure::mismatch(&format!("{path}.edits"), "List", edits)
                    })?
                };
                let mut result = Vec::new();
                for (i, edit) in edits.iter().enumerate() {
                    let path = format!("{path}.edits[{i}]");
                    object(edit, &path)?;
                    let get = |key: &str| {
                        edit.get(key)
                            .ok_or_else(|| RequestFailure::mismatch(&path, key, edit))
                    };
                    let offset = integer(get("offset")?, &format!("{path}.offset"))?;
                    let length = integer(get("length")?, &format!("{path}.length"))?;
                    let replacement = string(get("replacement")?, &format!("{path}.replacement"))?;
                    if let Some(id) = edit.get("id") {
                        string(id, &format!("{path}.id"))?;
                    }
                    result.push((
                        usize::try_from(offset).unwrap_or(usize::MAX),
                        usize::try_from(length).unwrap_or(usize::MAX),
                        replacement.to_owned(),
                    ));
                }
                Ok(Self::Change(result))
            }
            _ => Err(RequestFailure::mismatch(
                &format!("{path}[\"type\"]"),
                "One of: [add, change, remove]",
                value,
            )),
        }?;
        if type_ != "remove"
            && let Some(version) = value.get("version")
        {
            integer(version, &format!("{path}.version"))?;
        }
        Ok(change)
    }
}
fn integer(value: &Value, path: &str) -> Result<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
        .ok_or_else(|| RequestFailure::mismatch(path, "int", value))
}

fn decode_imported_elements_list(
    value: &Value,
    path: &str,
) -> Result<Vec<protocol::ImportedElements>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let arr = value
        .as_array()
        .ok_or_else(|| RequestFailure::mismatch(path, "List", value))?;
    let mut out = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        let item_path = format!("{path}[{i}]");
        let obj = item
            .as_object()
            .ok_or_else(|| RequestFailure::mismatch(&item_path, "ImportedElements", item))?;
        let get = |k: &str| {
            obj.get(k)
                .ok_or_else(|| RequestFailure::mismatch(&item_path, k, item))
        };
        let p = string(get("path")?, &format!("{item_path}.path"))?.to_owned();
        valid_path(&p)?;
        let prefix = string(get("prefix")?, &format!("{item_path}.prefix"))?.to_owned();
        let elements = strings(get("elements")?, &format!("{item_path}.elements"))?;
        out.push(protocol::ImportedElements {
            path: p,
            prefix,
            elements,
        });
    }
    Ok(out)
}

fn wire(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("protocol types have JSON-compatible fields")
}
