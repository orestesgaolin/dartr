// Dart source: pkg/analysis_server/lib/src/legacy_analysis_server.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/analysis_*.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/server_*.dart
// Dart source: pkg/analysis_server/lib/src/protocol_server.dart
// Dart source: pkg/analyzer_plugin/lib/utilities/analyzer_converter.dart

//! Legacy request handlers using the shared parse/lint diagnostic pipeline.
//! Resolution-dependent requests return UNKNOWN_REQUEST until the resolver is ready.

use std::io::{self, BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use dartr_cli::provider::{AnalyzedFile, FileDiagnostics, diagnostics_with_reader};
use dartr_cli::server::{AnalysisError, LineInfoCache, analysis_errors};
use dartr_project::{AnalysisContextCollection, CollectionOptions, FileKind, fs, non_dart, paths};
use dartr_server::args::ServerOptions;
use dartr_syntax::LineInfo;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Value, json};

use crate::protocol;
use crate::transport::{Channel, LineReader};

/// The supported subset of the pinned spec. Notifications are not requests.
pub const IMPLEMENTED_REQUESTS: &[&str] = &[
    "server.getVersion",
    "server.shutdown",
    "server.setSubscriptions",
    "analysis.setAnalysisRoots",
    "analysis.updateContent",
    "analysis.setPriorityFiles",
    "analysis.setSubscriptions",
    "analysis.getErrors",
    "analysis.reanalyze",
];
pub const IMPLEMENTED_NOTIFICATIONS: &[&str] = &[
    "server.connected",
    "server.status",
    "server.error",
    "analysis.errors",
    "analysis.flushResults",
    "analysis.folding",
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

struct Server<W: Write> {
    channel: Channel<W>,
    options: ServerOptions,
    included: Vec<String>,
    excluded: Vec<String>,
    overlays: IndexMap<String, String>,
    priority: Vec<String>,
    subscriptions: IndexMap<String, Vec<String>>,
    status_subscribed: bool,
    collection: Option<AnalysisContextCollection>,
    published: IndexSet<String>,
    dirty: bool,
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
        collection: None,
        published: IndexSet::new(),
        dirty: false,
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
    fn notify(&mut self, event: &str, params: Value) -> io::Result<()> {
        self.channel
            .send(&json!({"event": event, "params": params}))
    }
    fn response(&mut self, id: &str, result: Option<Value>) -> Result<()> {
        let mut response = json!({"id": id});
        if let Some(result) = result {
            response["result"] = result;
        }
        self.channel
            .send(&response)
            .map_err(|e| RequestFailure::new("SERVER_ERROR", e.to_string()))
    }
    fn error_response(&mut self, id: &str, error: RequestFailure) -> io::Result<()> {
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
                self.dirty = true;
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
                // AST-only folding is computed even when diagnostics did not change.
                self.dirty = self.collection.is_some();
                self.response(id, None)?;
            }
            "analysis.updateContent" => {
                self.update_content(params)?;
                self.dirty = true;
                self.response(id, Some(json!({})))?;
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
            "analysis.reanalyze" => {
                self.response(id, None)?;
                self.dirty = true;
            }
            _ => return Err(RequestFailure::new("UNKNOWN_REQUEST", "Unknown request")),
        }
        Ok(false)
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
        self.collection = Some(AnalysisContextCollection::new(
            &self.included,
            &CollectionOptions {
                sdk_path: self.options.dart_sdk.clone(),
                package_config_file: self.options.packages.clone(),
                enabled_experiments: self.options.enabled_experiments.clone(),
                ..Default::default()
            },
        ));
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
    fn diagnostics(&self, paths: &[String]) -> IndexMap<String, Vec<Value>> {
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
        let read = |path: &str| fs::read_string(path);
        let results = diagnostics_with_reader(collection, &files, &read);
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
        self.status(false)
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
