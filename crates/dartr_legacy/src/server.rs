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
    "search.findElementReferences",
    "search.findMemberDeclarations",
    "search.findMemberReferences",
    "search.findTopLevelDeclarations",
    "search.getElementDeclarations",
    "search.getTypeHierarchy",
    "edit.format",
    "edit.sortMembers",
    "edit.organizeDirectives",
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
                if let Some(lsp_caps) = params.get("lspCapabilities")
                    && let Some(obj) = lsp_caps.as_object()
                {
                    for key in [
                        "general",
                        "notebookDocument",
                        "textDocument",
                        "window",
                        "workspace",
                    ] {
                        if let Some(val) = obj.get(key)
                            && !val.is_null()
                            && !val.is_object()
                        {
                            let ty = match val {
                                Value::Bool(_) => "bool",
                                Value::Number(_) => "int",
                                Value::String(_) => "String",
                                Value::Array(_) => "List",
                                _ => "Object",
                            };
                            return Err(RequestFailure::new(
                                "INVALID_PARAMETER",
                                format!(
                                    "The 'lspCapabilities' parameter was invalid: {key} must be of type {ty}"
                                ),
                            ));
                        }
                    }
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
            self.dirty = true;
        }
        Ok(())
    }
}
impl<W: Write> Drop for Server<W> {
    fn drop(&mut self) {
        for file in self.overlays.keys() {
            fs::set_overlay(file, None);
        }
    }
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

fn wire(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("protocol types have JSON-compatible fields")
}
