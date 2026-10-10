// Dart source: pkg/analysis_server/lib/src/server/driver.dart
// Dart source: pkg/analysis_server/lib/src/legacy_analysis_server.dart

//! Differential test of the legacy analysis-server protocol.
//!
//! One dartdev-style session and one IntelliJ-style overlay session run
//! against Dart 3.13.3 and dartr. The test compares syntactic diagnostics and
//! AST-only lints until dartr has a resolver. SourceEdit offsets exercise
//! UTF-16 by placing a supplementary-plane character before an edit.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use dartr_diagnostics::{DiagnosticType, codes_by_name};
use serde_json::{Map, Value, json};

const QUIET: Duration = Duration::from_millis(400);
const STEP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq)]
struct Transcript {
    connected: Value,
    set_subscriptions: Value,
    roots: Value,
    get_errors: Value,
    analysis_subscriptions: Value,
    priority: Value,
    overlay_add: Value,
    overlay_change: Value,
    overlay_remove: Value,
    negative_requests: Vec<Value>,
    reanalyze: Value,
    excluded_root: Value,
    unknown_request: Value,
    shutdown: Value,
    flushed: BTreeSet<String>,
    exit_code: i32,
}

struct LegacyClient {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Option<Value>>,
    next_id: u32,
    responses: BTreeMap<String, Value>,
    diagnostics: BTreeMap<String, Vec<Value>>,
    notifications: BTreeMap<String, BTreeMap<String, Value>>,
    search_results: BTreeMap<String, Value>,
    analyzed_files: Vec<Value>,
    server_logs: Vec<Value>,
    analysis_started: usize,
    analysis_completed: usize,
    analyzing: bool,
    flushed: BTreeSet<String>,
}

impl LegacyClient {
    fn spawn(program: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", program.display()));
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("server stdout");
        let stderr = child.stderr.take().expect("server stderr");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let message = serde_json::from_str(&line)
                    .unwrap_or_else(|error| panic!("invalid server JSON `{line}`: {error}"));
                if tx.send(Some(message)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
        thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut text);
            if !text.trim().is_empty() {
                eprintln!("[legacy server stderr] {text}");
            }
        });
        LegacyClient {
            child,
            stdin,
            rx,
            next_id: 0,
            responses: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            notifications: BTreeMap::new(),
            search_results: BTreeMap::new(),
            analyzed_files: Vec::new(),
            server_logs: Vec::new(),
            analysis_started: 0,
            analysis_completed: 0,
            analyzing: false,
            flushed: BTreeSet::new(),
        }
    }

    fn send_raw(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("server stdin");
        serde_json::to_writer(&mut *stdin, message).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    fn send_request(&mut self, method: &str, params: Value) -> String {
        self.next_id += 1;
        let id = self.next_id.to_string();
        let message = json!({"id": id, "method": method, "params": params});
        self.send_raw(&message);
        self.next_id.to_string()
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params);
        self.wait_for_response(&id)
    }

    fn wait_for_response(&mut self, id: &str) -> Value {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            if let Some(response) = self.responses.remove(id) {
                return normalize_response(response);
            }
            let message = self
                .next_message(deadline)
                .unwrap_or_else(|| panic!("server exited or timed out before response {id}"));
            self.handle(message);
        }
    }

    fn wait_for_search_results(&mut self, search_id: &str) -> Value {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            if let Some(params) = self.search_results.remove(search_id) {
                return params;
            }
            let message = self.next_message(deadline).unwrap_or_else(|| {
                panic!("server exited or timed out before search.results {search_id}")
            });
            self.handle(message);
        }
    }

    fn wait_for_event(&mut self, event: &str) -> Value {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            let message = self
                .next_message(deadline)
                .unwrap_or_else(|| panic!("server exited or timed out before {event}"));
            if message["event"] == event {
                self.handle(message.clone());
                return message;
            }
            self.handle(message);
        }
    }

    fn settle(&mut self, completed_before: usize, expect_analysis: bool) {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            match self.next_message(Instant::now() + QUIET) {
                Some(message) => self.handle(message),
                None => {
                    let completed = !expect_analysis || self.analysis_completed > completed_before;
                    if !self.analyzing && completed {
                        return;
                    }
                }
            }
            assert!(Instant::now() < deadline, "legacy server did not settle");
        }
    }

    fn next_message(&self, deadline: Instant) -> Option<Value> {
        let timeout = deadline.saturating_duration_since(Instant::now());
        match self.rx.recv_timeout(timeout) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }

    fn handle(&mut self, message: Value) {
        if let Some(id) = message.get("id").and_then(Value::as_str) {
            self.responses.insert(id.to_string(), message);
            return;
        }
        match message.get("event").and_then(Value::as_str) {
            Some("analysis.errors") => {
                let params = &message["params"];
                let file = params["file"].as_str().expect("errors file").to_string();
                let errors = params["errors"].as_array().expect("errors array").clone();
                self.diagnostics.insert(file, errors);
            }
            Some(
                event @ ("analysis.navigation"
                | "analysis.highlights"
                | "analysis.occurrences"
                | "analysis.outline"
                | "analysis.implemented"
                | "analysis.overrides"
                | "analysis.folding"
                | "analysis.closingLabels"
                | "flutter.outline"
                | "completion.existingImports"),
            ) => {
                let params = &message["params"];
                if let Some(file) = params["file"].as_str() {
                    self.notifications
                        .entry(event.to_string())
                        .or_default()
                        .insert(file.to_string(), params.clone());
                }
            }
            Some("analysis.analyzedFiles") => {
                self.analyzed_files.push(message["params"].clone());
            }
            Some("server.log") => {
                self.server_logs.push(message["params"].clone());
            }
            Some("search.results") => {
                let params = &message["params"];
                if let Some(id) = params["id"].as_str() {
                    self.search_results.insert(id.to_string(), params.clone());
                }
            }
            Some("server.status") => match message["params"]["analysis"]["isAnalyzing"].as_bool() {
                Some(true) => {
                    self.analyzing = true;
                    self.analysis_started += 1;
                }
                Some(false) => {
                    self.analyzing = false;
                    self.analysis_completed += 1;
                }
                None => {}
            },
            Some("analysis.flushResults") => {
                if let Some(files) = message["params"]["files"].as_array() {
                    for file in files.iter().filter_map(Value::as_str) {
                        self.flushed.insert(file.to_string());
                        self.diagnostics.remove(file);
                    }
                }
            }
            _ => {}
        }
    }

    fn snapshot(&self, root: &Path) -> Value {
        let root = root.to_string_lossy();
        let mut files = Map::new();
        for (file, diagnostics) in &self.diagnostics {
            if !file.starts_with(root.as_ref()) {
                continue;
            }
            let kept: Vec<Value> = diagnostics
                .iter()
                .filter(|diagnostic| keep_diagnostic(diagnostic))
                .map(|diagnostic| normalize_paths(diagnostic.clone(), root.as_ref()))
                .collect();
            if !kept.is_empty() {
                files.insert(file[root.len()..].to_string(), Value::Array(kept));
            }
        }
        Value::Object(files)
    }

    fn finish(mut self) -> (BTreeSet<String>, i32) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            while let Ok(Some(message)) = self.rx.try_recv() {
                self.handle(message);
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                return (
                    std::mem::take(&mut self.flushed),
                    status.code().unwrap_or(-1),
                );
            }
            assert!(Instant::now() < deadline, "legacy server did not exit");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for LegacyClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn keep_diagnostic(diagnostic: &Value) -> bool {
    if diagnostic["location"]["file"]
        .as_str()
        .is_some_and(|file| file.ends_with(".yaml") || file.ends_with(".xml"))
    {
        return true;
    }
    if diagnostic["type"] == "SYNTACTIC_ERROR" {
        return true;
    }
    if diagnostic["type"] != "LINT" {
        return false;
    }
    let Some(code) = diagnostic["code"].as_str() else {
        return false;
    };
    codes_by_name(code)
        .iter()
        .any(|descriptor| descriptor.diagnostic_type == DiagnosticType::Lint)
}

fn normalize_response(mut response: Value) -> Value {
    if let Some(object) = response.as_object_mut() {
        object.remove("id");
    }
    response
}

fn normalize_paths(value: Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(root, "${ROOT}")),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| normalize_paths(value, root))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, normalize_paths(value, root)))
                .collect(),
        ),
        other => other,
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/legacy_fixtures/project")
}

fn dart_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DART_BIN") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let output = Command::new("sh")
        .args(["-c", "command -v dart"])
        .output()
        .ok()?;
    let wrapper = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    let direct = wrapper.parent()?.join("cache/dart-sdk/bin/dart");
    if direct.is_file() {
        return Some(direct);
    }
    Command::new(&wrapper)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?
        .success()
        .then_some(wrapper)
}

fn run_session(program: &Path, args: &[&str]) -> Transcript {
    let root = fixture_root().canonicalize().unwrap();
    let source = root.join("lib/main.dart");
    let mut client = LegacyClient::spawn(program, args);

    let mut connected = client.wait_for_event("server.connected");
    connected["params"].as_object_mut().unwrap().remove("pid");

    let set_subscriptions = client.request(
        "server.setSubscriptions",
        json!({"subscriptions": ["STATUS"]}),
    );
    let completed = client.analysis_completed;
    let roots_response = client.request(
        "analysis.setAnalysisRoots",
        json!({"included": [root], "excluded": [], "packageRoots": {}}),
    );
    client.settle(completed, true);
    let roots = json!({
        "response": roots_response,
        "diagnostics": client.snapshot(&fixture_root().canonicalize().unwrap()),
    });

    let get_errors_response = client.request("analysis.getErrors", json!({"file": source}));
    let mut get_errors = get_errors_response;
    if let Some(errors) = get_errors["result"]["errors"].as_array_mut() {
        errors.retain(keep_diagnostic);
    }
    get_errors = normalize_paths(get_errors, root.to_string_lossy().as_ref());

    let analysis_subscriptions = client.request(
        "analysis.setSubscriptions",
        json!({"subscriptions": {"OUTLINE": [source]}}),
    );
    let priority = client.request("analysis.setPriorityFiles", json!({"files": [source]}));
    client.settle(client.analysis_completed, false);

    let overlay = "void main() {\n  print('😀');\n  print('broken';\n}\n";
    let completed = client.analysis_completed;
    let add_response = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "add", "content": overlay}}}),
    );
    client.settle(completed, true);
    let overlay_add = json!({"response": add_response, "diagnostics": client.snapshot(&root)});

    let edit_offset = "void main() {\n  print('😀');\n  print('broken'"
        .encode_utf16()
        .count();
    let completed = client.analysis_completed;
    let change_response = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "change", "edits": [{
            "offset": edit_offset, "length": 0, "replacement": ")"
        }]}}}),
    );
    client.settle(completed, true);
    let overlay_change =
        json!({"response": change_response, "diagnostics": client.snapshot(&root)});

    let completed = client.analysis_completed;
    let remove_response = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "remove"}}}),
    );
    client.settle(completed, true);
    let overlay_remove =
        json!({"response": remove_response, "diagnostics": client.snapshot(&root)});

    let missing_required = client.request("analysis.setPriorityFiles", json!({}));
    let invalid_enum = client.request(
        "analysis.setSubscriptions",
        json!({"subscriptions": {"NOT_A_SERVICE": []}}),
    );
    let invalid_path = client.request(
        "analysis.setPriorityFiles",
        json!({"files": ["relative.dart"]}),
    );
    let change_without_add = client.request(
        "analysis.updateContent",
        json!({"files": {(root.join("lib/no_overlay.dart").to_string_lossy().as_ref()): {
            "type": "change", "edits": [{"offset": 0, "length": 0, "replacement": "x"}]
        }}}),
    );
    let invalid_discriminator = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "unknown"}}}),
    );
    let add_for_invalid_edit = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "add", "content": "void main() {}\n"}}}),
    );
    let out_of_range_edit = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {
            "type": "change", "edits": [{"offset": 9999, "length": 0, "replacement": "x"}]
        }}}),
    );
    let remove_after_invalid_edit = client.request(
        "analysis.updateContent",
        json!({"files": {(source.to_string_lossy().as_ref()): {"type": "remove"}}}),
    );
    let negative_requests = vec![
        missing_required,
        invalid_enum,
        invalid_path,
        change_without_add,
        invalid_discriminator,
        add_for_invalid_edit,
        out_of_range_edit,
        remove_after_invalid_edit,
    ];

    let completed = client.analysis_completed;
    let reanalyze_response = client.request("analysis.reanalyze", json!({}));
    client.settle(completed, true);
    let reanalyze = json!({"response": reanalyze_response, "diagnostics": client.snapshot(&root)});

    let completed = client.analysis_completed;
    let excluded_response = client.request(
        "analysis.setAnalysisRoots",
        json!({"included": [root], "excluded": [root.join("lib")]}),
    );
    client.settle(completed, true);
    let excluded_root =
        json!({"response": excluded_response, "diagnostics": client.snapshot(&root)});

    let unknown_request = client.request("not.aRealRequest", json!({}));
    let shutdown = client.request("server.shutdown", json!({}));
    let (flushed, exit_code) = client.finish();
    let flushed = flushed
        .into_iter()
        .map(|file| file.replace(root.to_string_lossy().as_ref(), "${ROOT}"))
        .collect();

    Transcript {
        connected,
        set_subscriptions,
        roots,
        get_errors,
        analysis_subscriptions,
        priority,
        overlay_add,
        overlay_change,
        overlay_remove,
        negative_requests,
        reanalyze,
        excluded_root,
        unknown_request,
        shutdown,
        flushed,
        exit_code,
    }
}

#[test]
fn dartdev_and_intellij_legacy_sessions_match_dart_3_13_3() {
    let Some(dart) = dart_binary() else {
        eprintln!("skipped: Dart 3.13.3 is not available");
        return;
    };
    let version = Command::new(&dart).arg("--version").output().unwrap();
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        version.contains("3.13.3"),
        "requires Dart 3.13.3, got {version}"
    );

    let dart_transcript = run_session(
        &dart,
        &[
            "language-server",
            "--protocol=analyzer",
            "--client-id=legacy-parity",
        ],
    );
    assert_eq!(
        dart_transcript.connected,
        json!({"event": "server.connected", "params": {"version": "1.40.1"}})
    );
    assert_eq!(dart_transcript.set_subscriptions, json!({}));
    assert_eq!(dart_transcript.analysis_subscriptions, json!({}));
    assert_eq!(dart_transcript.priority, json!({}));
    assert_eq!(
        dart_transcript.unknown_request,
        json!({"error": {"code": "UNKNOWN_REQUEST", "message": "Unknown request"}})
    );
    assert_eq!(dart_transcript.shutdown, json!({}));
    assert_eq!(dart_transcript.exit_code, 0);
    assert!(dart_transcript.flushed.contains("${ROOT}/lib/main.dart"));

    let original = dart_transcript.roots["diagnostics"]["/lib/main.dart"]
        .as_array()
        .expect("initial diagnostics");
    assert_eq!(
        original
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["expected_token", "camel_case_types", "avoid_empty_else"]
    );
    assert!(
        original
            .iter()
            .all(|diagnostic| diagnostic["hasFix"] == false)
    );
    assert!(
        original
            .iter()
            .filter(|diagnostic| diagnostic["type"] == "LINT")
            .all(|diagnostic| diagnostic["url"].as_str().is_some())
    );
    let overlay_error = &dart_transcript.overlay_add["diagnostics"]["/lib/main.dart"][0];
    assert_eq!(overlay_error["location"]["offset"], 45);
    assert_eq!(overlay_error["location"]["startLine"], 3);
    assert_eq!(overlay_error["location"]["startColumn"], 17);
    assert!(dart_transcript.overlay_change["diagnostics"]["/lib/main.dart"].is_null());
    assert_eq!(
        dart_transcript.overlay_remove["diagnostics"],
        dart_transcript.roots["diagnostics"]
    );
    assert_eq!(
        dart_transcript
            .negative_requests
            .iter()
            .filter_map(|response| response["error"]["code"].as_str())
            .collect::<Vec<_>>(),
        [
            "INVALID_PARAMETER",
            "INVALID_PARAMETER",
            "INVALID_FILE_PATH_FORMAT",
            "INVALID_OVERLAY_CHANGE",
            "INVALID_PARAMETER",
            "INVALID_OVERLAY_CHANGE",
        ]
    );
    assert_eq!(
        dart_transcript.reanalyze["diagnostics"],
        dart_transcript.roots["diagnostics"]
    );
    assert!(dart_transcript.excluded_root["diagnostics"]["/lib/main.dart"].is_null());
    assert_eq!(
        dart_transcript.excluded_root["diagnostics"]["/analysis_options.yaml"],
        dart_transcript.roots["diagnostics"]["/analysis_options.yaml"]
    );
    assert_eq!(
        dart_transcript.excluded_root["diagnostics"]["/pubspec.yaml"],
        dart_transcript.roots["diagnostics"]["/pubspec.yaml"]
    );

    let options_error = &dart_transcript.roots["diagnostics"]["/analysis_options.yaml"][0];
    assert_eq!(options_error["code"], "included_file_warning");
    assert_eq!(options_error["hasFix"], true);
    let pubspec_error = &dart_transcript.roots["diagnostics"]["/pubspec.yaml"][0];
    assert_eq!(pubspec_error["code"], "deprecated_field");
    assert_eq!(pubspec_error["hasFix"], true);

    let dartr_transcript = run_session(
        Path::new(env!("CARGO_BIN_EXE_dartr")),
        &[
            "language-server",
            "--protocol=analyzer",
            "--client-id=legacy-parity",
        ],
    );
    assert_eq!(dartr_transcript, dart_transcript);
}

#[derive(Debug)]
struct ExtendedTranscript {
    steps: BTreeMap<String, Value>,
}

fn find_offset(source: &str, needle: &str) -> i64 {
    let byte_idx = source
        .find(needle)
        .unwrap_or_else(|| panic!("needle `{needle}` not found"));
    source[..byte_idx].encode_utf16().count() as i64
}

fn find_line_col(source: &str, needle: &str) -> (u32, u32) {
    let byte_idx = source
        .find(needle)
        .unwrap_or_else(|| panic!("needle `{needle}` not found"));
    let prefix = &source[..byte_idx];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = prefix[line_start..].encode_utf16().count() as u32;
    (line, col)
}

fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn flutter_sdk_root(dart: &Path) -> Option<PathBuf> {
    for anc in dart.ancestors() {
        if anc.join("packages/flutter/lib/widgets.dart").is_file()
            && anc
                .join("bin/cache/pkg/sky_engine/lib/_embedder.yaml")
                .is_file()
        {
            return Some(anc.to_path_buf());
        }
    }
    if let Ok(output) = Command::new("sh")
        .args(["-c", "command -v flutter"])
        .output()
        && let Ok(text) = String::from_utf8(output.stdout)
    {
        let bin = PathBuf::from(text.trim());
        if let Ok(canon) = bin.canonicalize() {
            for anc in canon.ancestors() {
                if anc.join("packages/flutter/lib/widgets.dart").is_file()
                    && anc
                        .join("bin/cache/pkg/sky_engine/lib/_embedder.yaml")
                        .is_file()
                {
                    return Some(anc.to_path_buf());
                }
            }
        }
    }
    None
}

fn run_extended_session(program: &Path, args: &[&str], root: &Path) -> ExtendedTranscript {
    let root_str = root.to_string_lossy().to_string();
    let lib_file = root.join("lib/lib.dart");
    let part_file = root.join("lib/part_a.dart");
    let helper_file = root.join("lib/helper.dart");
    let edit_file = root.join("lib/edit_target.dart");
    let broken_file = root.join("lib/broken.dart");

    let lib_src = std::fs::read_to_string(&lib_file).unwrap();
    let part_src = std::fs::read_to_string(&part_file).unwrap();

    let mut client = LegacyClient::spawn(program, args);
    client.wait_for_event("server.connected");

    client.request(
        "server.setSubscriptions",
        json!({"subscriptions": ["STATUS"]}),
    );
    client.request(
        "analysis.setSubscriptions",
        json!({
            "subscriptions": {
                "NAVIGATION": [&lib_file, &part_file],
                "HIGHLIGHTS": [&lib_file, &part_file],
                "OCCURRENCES": [&lib_file, &part_file],
                "OUTLINE": [&lib_file, &part_file],
                "IMPLEMENTED": [&lib_file, &helper_file],
                "OVERRIDES": [&lib_file, &part_file],
                "CLOSING_LABELS": [&lib_file, &part_file],
                "INVALIDATE": [&lib_file],
            }
        }),
    );
    let general_sub_resp = client.request(
        "analysis.setGeneralSubscriptions",
        json!({"subscriptions": ["ANALYZED_FILES"]}),
    );
    client.request(
        "analysis.setPriorityFiles",
        json!({"files": [&lib_file, &part_file, &helper_file]}),
    );
    let completed = client.analysis_completed;
    client.request(
        "analysis.setAnalysisRoots",
        json!({"included": [root], "excluded": []}),
    );
    client.settle(completed, true);

    let mut steps = BTreeMap::new();
    steps.insert(
        "analysis.setGeneralSubscriptions:subscribe".to_string(),
        general_sub_resp,
    );

    // 1. analysis.getHover
    for (label, file, src, needle) in [
        ("class_Sub", &lib_file, &lib_src, "Sub<T extends num>"),
        ("method_compute", &lib_file, &lib_src, "compute(T input)"),
        ("field_seed", &lib_file, &lib_src, "seed;"),
        ("type_param_T", &lib_file, &lib_src, "T helperValue"),
        ("ctor_HelperBox", &lib_file, &lib_src, "HelperBox<T>(seed)"),
        ("extension_triple", &part_file, &part_src, "triple() =>"),
    ] {
        let offset = find_offset(src, needle);
        let resp = client.request("analysis.getHover", json!({"file": file, "offset": offset}));
        steps.insert(
            format!("analysis.getHover:{label}"),
            normalize_paths(resp, &root_str),
        );
    }

    // 2. analysis.getNavigation
    let nav_offset = find_offset(&lib_src, "final box = HelperBox<T>(seed);");
    let nav_resp = client.request(
        "analysis.getNavigation",
        json!({"file": lib_file, "offset": nav_offset, "length": 80}),
    );
    steps.insert(
        "analysis.getNavigation".to_string(),
        normalize_paths(nav_resp, &root_str),
    );

    // 3. analysis.getReachableSources & analysis.getLibraryDependencies
    let reachable_resp = client.request("analysis.getReachableSources", json!({"file": lib_file}));
    steps.insert(
        "analysis.getReachableSources".to_string(),
        normalize_paths(reachable_resp, &root_str),
    );
    let lib_deps_resp = client.request("analysis.getLibraryDependencies", json!({}));
    steps.insert("analysis.getLibraryDependencies".to_string(), lib_deps_resp);

    // 4. search.findElementReferences
    for (label, needle, include_potential) in [
        ("compute_exact", "compute(T input)", false),
        ("compute_potential", "compute(T input)", true),
        ("seed_field", "seed;", false),
        ("class_Base", "Base<T extends num>", false),
    ] {
        let offset = find_offset(&lib_src, needle);
        let mut resp = client.request(
            "search.findElementReferences",
            json!({"file": lib_file, "offset": offset, "includePotential": include_potential}),
        );
        let search_id = resp["result"]["id"]
            .as_str()
            .expect("search id")
            .to_string();
        resp["result"]["id"] = json!("${SEARCH_ID}");
        let mut results_notif = client.wait_for_search_results(&search_id);
        results_notif["id"] = json!("${SEARCH_ID}");
        steps.insert(
            format!("search.findElementReferences:{label}"),
            normalize_paths(
                json!({"response": resp, "results": results_notif}),
                &root_str,
            ),
        );
    }
    // Also test findElementReferences at whitespace (no element)
    let no_elem_resp = client.request(
        "search.findElementReferences",
        json!({"file": lib_file, "offset": 0, "includePotential": false}),
    );
    steps.insert(
        "search.findElementReferences:none".to_string(),
        normalize_paths(no_elem_resp, &root_str),
    );

    // 5. search.findMemberDeclarations
    {
        let mut resp = client.request("search.findMemberDeclarations", json!({"name": "compute"}));
        let search_id = resp["result"]["id"].as_str().unwrap().to_string();
        resp["result"]["id"] = json!("${SEARCH_ID}");
        let mut results_notif = client.wait_for_search_results(&search_id);
        results_notif["id"] = json!("${SEARCH_ID}");
        steps.insert(
            "search.findMemberDeclarations".to_string(),
            normalize_paths(
                json!({"response": resp, "results": results_notif}),
                &root_str,
            ),
        );
    }

    // 6. search.findMemberReferences
    {
        let mut resp = client.request("search.findMemberReferences", json!({"name": "compute"}));
        let search_id = resp["result"]["id"].as_str().unwrap().to_string();
        resp["result"]["id"] = json!("${SEARCH_ID}");
        let mut results_notif = client.wait_for_search_results(&search_id);
        results_notif["id"] = json!("${SEARCH_ID}");
        steps.insert(
            "search.findMemberReferences".to_string(),
            normalize_paths(
                json!({"response": resp, "results": results_notif}),
                &root_str,
            ),
        );
    }

    // 7. search.findTopLevelDeclarations
    {
        let mut resp = client.request(
            "search.findTopLevelDeclarations",
            json!({"pattern": "^(Base|Sub|PartChild|IntExt|runDemo)$"}),
        );
        let search_id = resp["result"]["id"].as_str().unwrap().to_string();
        resp["result"]["id"] = json!("${SEARCH_ID}");
        let mut results_notif = client.wait_for_search_results(&search_id);
        results_notif["id"] = json!("${SEARCH_ID}");
        steps.insert(
            "search.findTopLevelDeclarations".to_string(),
            normalize_paths(
                json!({"response": resp, "results": results_notif}),
                &root_str,
            ),
        );
    }

    // 8. search.getTypeHierarchy
    for (label, needle, super_only) in [
        ("Sub_full", "Sub<T extends num>", false),
        ("Sub_superOnly", "Sub<T extends num>", true),
        ("Base_full", "Base<T extends num>", false),
        ("compute_method", "compute(T input)", false),
    ] {
        let offset = find_offset(&lib_src, needle);
        let resp = client.request(
            "search.getTypeHierarchy",
            json!({"file": lib_file, "offset": offset, "superOnly": super_only}),
        );
        steps.insert(
            format!("search.getTypeHierarchy:{label}"),
            normalize_paths(resp, &root_str),
        );
    }

    // 9. edit.format
    let format_ok = client.request(
        "edit.format",
        json!({"file": edit_file, "selectionOffset": 0, "selectionLength": 0, "lineLength": 80}),
    );
    steps.insert(
        "edit.format:ok".to_string(),
        normalize_paths(format_ok, &root_str),
    );
    let format_err = client.request(
        "edit.format",
        json!({"file": broken_file, "selectionOffset": 0, "selectionLength": 0}),
    );
    steps.insert(
        "edit.format:syntax_error".to_string(),
        normalize_paths(format_err, &root_str),
    );

    // 10. edit.sortMembers
    let sort_ok = client.request("edit.sortMembers", json!({"file": edit_file}));
    steps.insert(
        "edit.sortMembers:ok".to_string(),
        normalize_paths(sort_ok, &root_str),
    );
    let sort_err = client.request("edit.sortMembers", json!({"file": broken_file}));
    steps.insert(
        "edit.sortMembers:syntax_error".to_string(),
        normalize_paths(sort_err, &root_str),
    );

    // 11. edit.organizeDirectives
    let org_ok = client.request("edit.organizeDirectives", json!({"file": edit_file}));
    steps.insert(
        "edit.organizeDirectives:ok".to_string(),
        normalize_paths(org_ok, &root_str),
    );
    let org_err = client.request("edit.organizeDirectives", json!({"file": broken_file}));
    steps.insert(
        "edit.organizeDirectives:syntax_error".to_string(),
        normalize_paths(org_err, &root_str),
    );

    // 12. server domain additions
    let cancel_resp = client.request("server.cancelRequest", json!({"id": "99999"}));
    steps.insert("server.cancelRequest".to_string(), cancel_resp);
    let caps_ok = client.request(
        "server.setClientCapabilities",
        json!({
            "requests": ["openUrlRequest", "showMessageRequest"],
            "supportsUris": true,
            "lspCapabilities": {"textDocument": {}}
        }),
    );
    steps.insert("server.setClientCapabilities:ok".to_string(), caps_ok);
    let caps_non_map = client.request(
        "server.setClientCapabilities",
        json!({"requests": [], "lspCapabilities": "bad"}),
    );
    steps.insert(
        "server.setClientCapabilities:non_map_lsp".to_string(),
        caps_non_map,
    );
    let caps_err = client.request(
        "server.setClientCapabilities",
        json!({"requests": [], "lspCapabilities": {"textDocument": "bad"}}),
    );
    steps.insert("server.setClientCapabilities:invalid".to_string(), caps_err);
    let open_url_req = client.request("server.openUrlRequest", json!({"url": "https://dart.dev"}));
    steps.insert(
        "server.openUrlRequest:client_sent".to_string(),
        open_url_req,
    );
    let show_msg_req = client.request(
        "server.showMessageRequest",
        json!({"type": "INFO", "message": "hi", "actions": []}),
    );
    steps.insert(
        "server.showMessageRequest:client_sent".to_string(),
        show_msg_req,
    );
    // Client response to server-initiated request should be accepted without INVALID_REQUEST
    client.send_raw(&json!({"id": "server-req-1", "result": {}}));
    client.send_raw(&json!({"id": "server-req-2", "result": {"action": "OK"}}));
    let ver_after_client_resp = client.request("server.getVersion", json!({}));
    assert!(
        !client.responses.contains_key(""),
        "unexpected INVALID_REQUEST after client response"
    );
    steps.insert(
        "server.clientResponse:followed_by_getVersion".to_string(),
        ver_after_client_resp,
    );

    // server.log subscription
    client.server_logs.clear();
    let log_sub_on = client.request(
        "server.setSubscriptions",
        json!({"subscriptions": ["STATUS", "LOG"]}),
    );
    steps.insert("server.setSubscriptions:log_on".to_string(), log_sub_on);
    let _ = client.request("server.getVersion", json!({}));
    let log_sub_off = client.request(
        "server.setSubscriptions",
        json!({"subscriptions": ["STATUS"]}),
    );
    steps.insert("server.setSubscriptions:log_off".to_string(), log_sub_off);
    let mut normalized_logs = Vec::new();
    for entry in &client.server_logs {
        let kind = entry["kind"].as_str().unwrap_or("");
        let data = &entry["data"];
        assert!(
            entry["time"].as_i64().is_some(),
            "server.log entry missing time"
        );
        assert!(
            entry["sdkVersion"].as_str().is_some(),
            "server.log entry missing sdkVersion"
        );
        if kind == "REQUEST" && data["method"] == "server.getVersion" {
            normalized_logs.push(json!({
                "kind": "REQUEST",
                "method": data["method"],
                "hasId": data["id"].as_str().is_some(),
                "hasServerRequestTime": data["serverRequestTime"].as_i64().is_some(),
            }));
        } else if kind == "RESPONSE" && data["result"]["version"] == "1.40.1" {
            normalized_logs.push(json!({
                "kind": "RESPONSE",
                "result": data["result"],
                "hasId": data["id"].as_str().is_some(),
                "hasResponseTime": data["responseTime"].as_i64().is_some(),
            }));
        }
    }
    steps.insert(
        "notification:server.log".to_string(),
        Value::Array(normalized_logs),
    );

    // 13. analysis.updateOptions, analysis.getImportedElements, analysis.getSignature
    let update_opts_ok = client.request(
        "analysis.updateOptions",
        json!({"options": {"enableSuperMixins": true, "generateHints": true, "generateLints": false}}),
    );
    steps.insert("analysis.updateOptions:ok".to_string(), update_opts_ok);
    let update_opts_err = client.request(
        "analysis.updateOptions",
        json!({"options": {"generateHints": "not_bool"}}),
    );
    steps.insert(
        "analysis.updateOptions:invalid".to_string(),
        update_opts_err,
    );

    let imp_needle = "final box = HelperBox<T>(seed);\n    final m = math.max(1, 2);";
    let imp_offset = find_offset(&lib_src, imp_needle);
    let imp_length = imp_needle.encode_utf16().count() as i64;
    let mut imp_ok = client.request(
        "analysis.getImportedElements",
        json!({"file": lib_file, "offset": imp_offset, "length": imp_length}),
    );
    if let Some(elements) = imp_ok["result"]["elements"].as_array_mut() {
        for el in elements.iter_mut() {
            if let Some(path) = el["path"].as_str()
                && path.ends_with("/lib/math/math.dart")
            {
                el["path"] = json!("${SDK}/lib/math/math.dart");
            }
        }
        elements.sort_by(|a, b| {
            let ka = (
                a["path"].as_str().unwrap_or(""),
                a["prefix"].as_str().unwrap_or(""),
            );
            let kb = (
                b["path"].as_str().unwrap_or(""),
                b["prefix"].as_str().unwrap_or(""),
            );
            ka.cmp(&kb)
        });
    }
    steps.insert(
        "analysis.getImportedElements:ok".to_string(),
        normalize_paths(imp_ok, &root_str),
    );
    let imp_invalid_path = client.request(
        "analysis.getImportedElements",
        json!({"file": "relative.dart", "offset": 0, "length": 1}),
    );
    steps.insert(
        "analysis.getImportedElements:invalid_path".to_string(),
        imp_invalid_path,
    );
    let imp_invalid_file = client.request(
        "analysis.getImportedElements",
        json!({"file": root.join("pubspec.yaml"), "offset": 0, "length": 1}),
    );
    steps.insert(
        "analysis.getImportedElements:invalid_file".to_string(),
        normalize_paths(imp_invalid_file, &root_str),
    );

    for (label, needle, delta) in [
        (
            "function_formatGreeting",
            "formatGreeting('hi'",
            "formatGreeting(".len() as i64,
        ),
        (
            "method_helperValue",
            "helperValue(box.item)",
            "helperValue(".len() as i64,
        ),
        (
            "ctor_HelperBox",
            "HelperBox<T>(seed)",
            "HelperBox<T>(".len() as i64,
        ),
        ("unknown_function", "class Sub", 0),
    ] {
        let offset = find_offset(&lib_src, needle) + delta;
        let resp = client.request(
            "analysis.getSignature",
            json!({"file": lib_file, "offset": offset}),
        );
        steps.insert(
            format!("analysis.getSignature:{label}"),
            normalize_paths(resp, &root_str),
        );
    }
    let sig_bad_offset = client.request(
        "analysis.getSignature",
        json!({"file": lib_file, "offset": 999999}),
    );
    steps.insert(
        "analysis.getSignature:invalid_offset".to_string(),
        sig_bad_offset,
    );
    let sig_bad_file = client.request(
        "analysis.getSignature",
        json!({"file": root.join("pubspec.yaml"), "offset": 0}),
    );
    steps.insert(
        "analysis.getSignature:invalid_file".to_string(),
        normalize_paths(sig_bad_file, &root_str),
    );

    // 14. search.getElementDeclarations
    let decls_pattern = client.request(
        "search.getElementDeclarations",
        json!({"pattern": "HelperBox"}),
    );
    steps.insert(
        "search.getElementDeclarations:pattern".to_string(),
        normalize_paths(decls_pattern, &root_str),
    );
    let decls_file = client.request(
        "search.getElementDeclarations",
        json!({"pattern": "compute", "file": lib_file}),
    );
    steps.insert(
        "search.getElementDeclarations:onlyForFile".to_string(),
        normalize_paths(decls_file, &root_str),
    );
    let decls_max = client.request(
        "search.getElementDeclarations",
        json!({"pattern": "", "file": lib_file, "maxResults": 4}),
    );
    steps.insert(
        "search.getElementDeclarations:maxResults".to_string(),
        normalize_paths(decls_max, &root_str),
    );

    // 15. execution domain
    let mut create_ctx = client.request("execution.createContext", json!({"contextRoot": root}));
    let exec_ctx_id = create_ctx["result"]["id"]
        .as_str()
        .expect("execution context id")
        .to_string();
    create_ctx["result"]["id"] = json!("${EXEC_CTX}");
    steps.insert("execution.createContext".to_string(), create_ctx);

    let map_file = client.request(
        "execution.mapUri",
        json!({"id": exec_ctx_id, "file": lib_file}),
    );
    let mapped_uri = map_file["result"]["uri"]
        .as_str()
        .expect("mapped uri")
        .to_string();
    steps.insert(
        "execution.mapUri:file_to_uri".to_string(),
        normalize_paths(map_file, &root_str),
    );
    let map_uri = client.request(
        "execution.mapUri",
        json!({"id": exec_ctx_id, "uri": mapped_uri}),
    );
    steps.insert(
        "execution.mapUri:uri_to_file".to_string(),
        normalize_paths(map_uri, &root_str),
    );
    let map_both = client.request(
        "execution.mapUri",
        json!({"id": exec_ctx_id, "file": lib_file, "uri": mapped_uri}),
    );
    steps.insert("execution.mapUri:both_error".to_string(), map_both);
    let map_neither = client.request("execution.mapUri", json!({"id": exec_ctx_id}));
    steps.insert("execution.mapUri:neither_error".to_string(), map_neither);
    let map_dir = client.request("execution.mapUri", json!({"id": exec_ctx_id, "file": root}));
    steps.insert("execution.mapUri:dir_error".to_string(), map_dir);
    let map_missing = client.request(
        "execution.mapUri",
        json!({"id": exec_ctx_id, "file": root.join("lib/missing.dart")}),
    );
    steps.insert("execution.mapUri:missing_error".to_string(), map_missing);
    let map_bad_ctx = client.request(
        "execution.mapUri",
        json!({"id": "nonexistent_ctx", "file": lib_file}),
    );
    steps.insert("execution.mapUri:invalid_ctx".to_string(), map_bad_ctx);
    let exec_sugg = client.request(
        "execution.getSuggestions",
        json!({
            "code": "x",
            "offset": 1,
            "contextFile": lib_file,
            "contextOffset": 0,
            "variables": []
        }),
    );
    steps.insert("execution.getSuggestions".to_string(), exec_sugg);
    let exec_sub = client.request(
        "execution.setSubscriptions",
        json!({"subscriptions": ["LAUNCH_DATA"]}),
    );
    steps.insert("execution.setSubscriptions".to_string(), exec_sub);
    let delete_ctx = client.request("execution.deleteContext", json!({"id": exec_ctx_id}));
    steps.insert("execution.deleteContext".to_string(), delete_ctx);

    // 16. diagnostic domain
    let mut diag_resp = client.request("diagnostic.getDiagnostics", json!({}));
    if let Some(contexts) = diag_resp["result"]["contexts"].as_array_mut() {
        for ctx in contexts.iter_mut() {
            assert!(
                ctx["explicitFileCount"].as_i64().unwrap_or(0) > 0,
                "expected positive explicitFileCount"
            );
            assert!(
                ctx["implicitFileCount"].as_i64().unwrap_or(-1) >= 0,
                "expected non-negative implicitFileCount"
            );
            ctx["implicitFileCount"] = json!("${IMPLICIT}");
            ctx["workItemQueueLength"] = json!(0);
        }
    }
    steps.insert(
        "diagnostic.getDiagnostics".to_string(),
        normalize_paths(diag_resp, &root_str),
    );
    let mut port_resp = client.request("diagnostic.getServerPort", json!({}));
    assert!(
        port_resp["result"]["port"].as_i64().unwrap_or(0) > 0,
        "expected positive server port"
    );
    port_resp["result"]["port"] = json!("${PORT}");
    steps.insert("diagnostic.getServerPort".to_string(), port_resp);

    // 17. analytics domain
    let analytics_enabled = client.request("analytics.isEnabled", json!({}));
    steps.insert("analytics.isEnabled".to_string(), analytics_enabled);
    let analytics_enable = client.request("analytics.enable", json!({"value": true}));
    steps.insert("analytics.enable".to_string(), analytics_enable);
    let analytics_event = client.request("analytics.sendEvent", json!({"action": "testAction"}));
    steps.insert("analytics.sendEvent".to_string(), analytics_event);
    let analytics_timing = client.request(
        "analytics.sendTiming",
        json!({"event": "testEvent", "millis": 42}),
    );
    steps.insert("analytics.sendTiming:ok".to_string(), analytics_timing);
    let analytics_timing_err = client.request(
        "analytics.sendTiming",
        json!({"event": "testEvent", "millis": -1}),
    );
    steps.insert(
        "analytics.sendTiming:invalid".to_string(),
        analytics_timing_err,
    );

    let general_unsub_resp = client.request(
        "analysis.setGeneralSubscriptions",
        json!({"subscriptions": []}),
    );
    steps.insert(
        "analysis.setGeneralSubscriptions:unsubscribe".to_string(),
        general_unsub_resp,
    );

    // 18. flutter domain on non-Flutter fixture (subscriptions + negative cases)
    let completed = client.analysis_completed;
    let flutter_sub_ok = client.request(
        "flutter.setSubscriptions",
        json!({"subscriptions": {"OUTLINE": [&lib_file]}}),
    );
    client.settle(completed, false);
    steps.insert("flutter.setSubscriptions:ok".to_string(), flutter_sub_ok);
    let flutter_sub_bad_service = client.request(
        "flutter.setSubscriptions",
        json!({"subscriptions": {"NOT_A_SERVICE": [&lib_file]}}),
    );
    steps.insert(
        "flutter.setSubscriptions:invalid_service".to_string(),
        flutter_sub_bad_service,
    );
    let flutter_sub_bad_path = client.request(
        "flutter.setSubscriptions",
        json!({"subscriptions": {"OUTLINE": ["relative.dart"]}}),
    );
    steps.insert(
        "flutter.setSubscriptions:invalid_path".to_string(),
        flutter_sub_bad_path,
    );
    let flutter_desc_no_widget = client.request(
        "flutter.getWidgetDescription",
        json!({"file": lib_file, "offset": find_offset(&lib_src, "HelperBox<T>(seed)")}),
    );
    steps.insert(
        "flutter.getWidgetDescription:no_widget".to_string(),
        flutter_desc_no_widget,
    );
    let flutter_desc_bad_path = client.request(
        "flutter.getWidgetDescription",
        json!({"file": "relative.dart", "offset": 0}),
    );
    steps.insert(
        "flutter.getWidgetDescription:invalid_path".to_string(),
        flutter_desc_bad_path,
    );
    let flutter_set_bad_id = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": 999999, "value": {"intValue": 1}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:invalid_id".to_string(),
        flutter_set_bad_id,
    );

    // 19. lsp.handle domain
    let lib_uri = path_to_uri(&lib_file);
    let part_uri = path_to_uri(&part_file);
    let edit_uri = path_to_uri(&edit_file);

    for (label, bad_msg) in [
        ("not_object", json!("bad")),
        (
            "missing_id",
            json!({"jsonrpc": "2.0", "method": "textDocument/hover"}),
        ),
        ("missing_method", json!({"jsonrpc": "2.0", "id": 1})),
    ] {
        let resp = client.request("lsp.handle", json!({"lspMessage": bad_msg}));
        steps.insert(format!("lsp.handle:invalid_{label}"), resp);
    }

    let lsp_unknown = client.request(
        "lsp.handle",
        json!({
            "lspMessage": {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "custom/unknownMethod",
                "params": {}
            }
        }),
    );
    steps.insert("lsp.handle:unknown_method".to_string(), lsp_unknown);

    for (label, needle) in [
        ("class_Sub", "Sub<T extends num>"),
        ("method_compute", "compute(T input)"),
        ("field_seed", "seed;"),
    ] {
        let (line, col) = find_line_col(&lib_src, needle);
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "textDocument/hover",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        steps.insert(
            format!("lsp.handle:hover:{label}"),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "helperValue(box.item)");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "textDocument/definition",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:definition".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "box.item");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "textDocument/typeDefinition",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:typeDefinition".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "Base<T extends num>");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 5,
                    "method": "textDocument/implementation",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:implementation".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "seed;");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 6,
                    "method": "textDocument/references",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col},
                        "context": {"includeDeclaration": false}
                    }
                }
            }),
        );
        let mut norm = normalize_paths(resp, &root_str);
        if let Some(arr) = norm["result"]["lspResponse"]["result"].as_array_mut() {
            arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        }
        steps.insert("lsp.handle:references".to_string(), norm);
    }

    {
        let (line, col) = find_line_col(&lib_src, "seed;");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 7,
                    "method": "textDocument/documentHighlight",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        let mut norm = normalize_paths(resp, &root_str);
        if let Some(arr) = norm["result"]["lspResponse"]["result"].as_array_mut() {
            arr.sort_by_key(|v| {
                (
                    v["range"]["start"]["line"].as_i64().unwrap_or(0),
                    v["range"]["start"]["character"].as_i64().unwrap_or(0),
                )
            });
        }
        steps.insert("lsp.handle:documentHighlight".to_string(), norm);
    }

    {
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 8,
                    "method": "textDocument/documentSymbol",
                    "params": {
                        "textDocument": {"uri": lib_uri}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:documentSymbol".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 9,
                    "method": "textDocument/foldingRange",
                    "params": {
                        "textDocument": {"uri": lib_uri}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:foldingRange".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "formatGreeting('hi'");
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 10,
                    "method": "textDocument/signatureHelp",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col + ("formatGreeting(".len() as u32)}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:signatureHelp".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 11,
                    "method": "textDocument/formatting",
                    "params": {
                        "textDocument": {"uri": edit_uri},
                        "options": {"tabSize": 2, "insertSpaces": true}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:formatting".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 12,
                    "method": "textDocument/semanticTokens/full",
                    "params": {
                        "textDocument": {"uri": lib_uri}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:semanticTokens/full".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 13,
                    "method": "textDocument/inlayHint",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 50, "character": 0}
                        }
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:inlayHint".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&lib_src, "Sub<T extends num>");
        let prep_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 14,
                    "method": "textDocument/prepareTypeHierarchy",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        let item = prep_resp["result"]["lspResponse"]["result"][0].clone();
        steps.insert(
            "lsp.handle:prepareTypeHierarchy".to_string(),
            normalize_paths(prep_resp, &root_str),
        );
        if !item.is_null() {
            let supertypes_resp = client.request(
                "lsp.handle",
                json!({
                    "lspMessage": {
                        "jsonrpc": "2.0",
                        "id": 15,
                        "method": "typeHierarchy/supertypes",
                        "params": {"item": item}
                    }
                }),
            );
            steps.insert(
                "lsp.handle:typeHierarchy/supertypes".to_string(),
                normalize_paths(supertypes_resp, &root_str),
            );
            let subtypes_resp = client.request(
                "lsp.handle",
                json!({
                    "lspMessage": {
                        "jsonrpc": "2.0",
                        "id": 16,
                        "method": "typeHierarchy/subtypes",
                        "params": {"item": item}
                    }
                }),
            );
            steps.insert(
                "lsp.handle:typeHierarchy/subtypes".to_string(),
                normalize_paths(subtypes_resp, &root_str),
            );
        }
    }

    {
        let (line, col) = find_line_col(&lib_src, "mixed() => tag;");
        let prep_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 17,
                    "method": "textDocument/prepareCallHierarchy",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        let item = prep_resp["result"]["lspResponse"]["result"][0].clone();
        steps.insert(
            "lsp.handle:prepareCallHierarchy".to_string(),
            normalize_paths(prep_resp, &root_str),
        );
        if !item.is_null() {
            let incoming_resp = client.request(
                "lsp.handle",
                json!({
                    "lspMessage": {
                        "jsonrpc": "2.0",
                        "id": 18,
                        "method": "callHierarchy/incomingCalls",
                        "params": {"item": item}
                    }
                }),
            );
            steps.insert(
                "lsp.handle:callHierarchy/incomingCalls".to_string(),
                normalize_paths(incoming_resp, &root_str),
            );
            let outgoing_resp = client.request(
                "lsp.handle",
                json!({
                    "lspMessage": {
                        "jsonrpc": "2.0",
                        "id": 19,
                        "method": "callHierarchy/outgoingCalls",
                        "params": {"item": item}
                    }
                }),
            );
            steps.insert(
                "lsp.handle:callHierarchy/outgoingCalls".to_string(),
                normalize_paths(outgoing_resp, &root_str),
            );
        }
    }

    {
        let ws_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 20,
                    "method": "workspace/symbol",
                    "params": {"query": "HelperBox"}
                }
            }),
        );
        steps.insert(
            "lsp.handle:workspace/symbol".to_string(),
            normalize_paths(ws_resp, &root_str),
        );
    }

    {
        let (line, col) = find_line_col(&part_src, "compute(int input)");
        let super_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 21,
                    "method": "dart/textDocument/super",
                    "params": {
                        "textDocument": {"uri": part_uri},
                        "position": {"line": line, "character": col}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:dart/textDocument/super".to_string(),
            normalize_paths(super_resp, &root_str),
        );
    }

    assert!(
        client.analyzed_files.len() >= 2,
        "expected initial and post-analysis analysis.analyzedFiles notifications"
    );
    assert_eq!(client.analyzed_files[0], json!({"directories": []}));
    let last_analyzed = client.analyzed_files.last().unwrap();
    let dirs = last_analyzed["directories"]
        .as_array()
        .expect("analyzedFiles directories");
    assert!(
        dirs.iter()
            .filter_map(Value::as_str)
            .all(|d| !d.ends_with(".yaml")),
        "analysis.analyzedFiles should exclude .yaml files"
    );
    let mut root_analyzed: Vec<String> = dirs
        .iter()
        .filter_map(Value::as_str)
        .filter(|d| d.starts_with(&root_str))
        .map(|d| d.replace(&root_str, "${ROOT}"))
        .collect();
    root_analyzed.sort();
    steps.insert(
        "notification:analysis.analyzedFiles".to_string(),
        json!({"directories": root_analyzed}),
    );

    // 20. completion domain
    let comp_reg_paths = client.request("completion.registerLibraryPaths", json!({"paths": []}));
    steps.insert(
        "completion.registerLibraryPaths:unknown".to_string(),
        comp_reg_paths,
    );
    let comp_set_subs = client.request(
        "completion.setSubscriptions",
        json!({"subscriptions": ["AVAILABLE_SUGGESTION_SETS"]}),
    );
    steps.insert(
        "completion.setSubscriptions:unknown".to_string(),
        comp_set_subs,
    );

    // Dart file completion at `box.` (completes HelperBox<T> members)
    {
        let offset = find_offset(&lib_src, "box.item") + ("box.".len() as i64);
        let mut resp = client.request(
            "completion.getSuggestions2",
            json!({
                "file": lib_file,
                "offset": offset,
                "maxResults": 100,
                "completionCaseMatchingMode": "FIRST_CHAR",
                "completionMode": "BASIC",
                "invocationCount": 1,
                "timeout": 10000
            }),
        );
        if let Some(suggestions) = resp["result"]["suggestions"].as_array_mut() {
            for s in suggestions.iter_mut() {
                if let Some(loc_file) = s["element"]["location"]["file"].as_str()
                    && loc_file.contains("/lib/core/")
                {
                    s["element"]["location"]["file"] = json!("${SDK}/lib/core/object.dart");
                    s["element"]["location"]["offset"] = json!(0);
                    s["element"]["location"]["length"] = json!(0);
                    s["element"]["location"]["startLine"] = json!(1);
                    s["element"]["location"]["startColumn"] = json!(1);
                    s["element"]["location"]["endLine"] = json!(1);
                    s["element"]["location"]["endColumn"] = json!(1);
                }
            }
            suggestions.sort_by(|a, b| {
                let ka = (
                    b["relevance"].as_i64().unwrap_or(0),
                    a["completion"].as_str().unwrap_or(""),
                );
                let kb = (
                    a["relevance"].as_i64().unwrap_or(0),
                    b["completion"].as_str().unwrap_or(""),
                );
                ka.cmp(&kb)
            });
        }
        steps.insert(
            "completion.getSuggestions2:dart_box_dot".to_string(),
            normalize_paths(resp, &root_str),
        );
    }

    // YAML completions: pubspec.yaml, analysis_options.yaml, fix_data.yaml
    {
        let pubspec_file = root.join("pubspec.yaml");
        let mut resp = client.request(
            "completion.getSuggestions2",
            json!({
                "file": pubspec_file,
                "offset": 0,
                "maxResults": 100
            }),
        );
        if let Some(suggestions) = resp["result"]["suggestions"].as_array_mut() {
            suggestions.sort_by(|a, b| {
                a["completion"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["completion"].as_str().unwrap_or(""))
            });
        }
        steps.insert(
            "completion.getSuggestions2:pubspec_yaml".to_string(),
            normalize_paths(resp, &root_str),
        );

        let options_file = root.join("analysis_options.yaml");
        std::fs::write(&options_file, "analyzer:\n  \n").unwrap();
        let mut resp_opts = client.request(
            "completion.getSuggestions2",
            json!({
                "file": options_file,
                "offset": 12,
                "maxResults": 100
            }),
        );
        if let Some(suggestions) = resp_opts["result"]["suggestions"].as_array_mut() {
            suggestions.sort_by(|a, b| {
                a["completion"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["completion"].as_str().unwrap_or(""))
            });
        }
        steps.insert(
            "completion.getSuggestions2:analysis_options_yaml".to_string(),
            normalize_paths(resp_opts, &root_str),
        );
        let _ = std::fs::remove_file(&options_file);

        let fix_data_file = root.join("lib/fix_data.yaml");
        std::fs::write(&fix_data_file, "version: 1\ntransforms:\n  - \n").unwrap();
        let mut resp_fix = client.request(
            "completion.getSuggestions2",
            json!({
                "file": fix_data_file,
                "offset": 27,
                "maxResults": 100
            }),
        );
        if let Some(suggestions) = resp_fix["result"]["suggestions"].as_array_mut() {
            suggestions.sort_by(|a, b| {
                a["completion"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["completion"].as_str().unwrap_or(""))
            });
        }
        steps.insert(
            "completion.getSuggestions2:fix_data_yaml".to_string(),
            normalize_paths(resp_fix, &root_str),
        );
        let _ = std::fs::remove_file(&fix_data_file);
    }

    // completion.getSuggestionDetails2
    {
        let offset = find_offset(&lib_src, "math.max");
        let resp_prefixed = client.request(
            "completion.getSuggestionDetails2",
            json!({
                "file": lib_file,
                "offset": offset,
                "completion": "sin",
                "libraryUri": "dart:math"
            }),
        );
        steps.insert(
            "completion.getSuggestionDetails2:existing_prefixed".to_string(),
            normalize_paths(resp_prefixed, &root_str),
        );

        let resp_new = client.request(
            "completion.getSuggestionDetails2",
            json!({
                "file": lib_file,
                "offset": offset,
                "completion": "Future",
                "libraryUri": "dart:async"
            }),
        );
        steps.insert(
            "completion.getSuggestionDetails2:new_import".to_string(),
            normalize_paths(resp_new, &root_str),
        );
    }

    // 21. edit domain additions
    let dartfix_resp = client.request("edit.dartfix", json!({"included": [root]}));
    steps.insert("edit.dartfix:unknown".to_string(), dartfix_resp);

    let postfix_list = client.request("edit.listPostfixCompletionTemplates", json!({}));
    steps.insert(
        "edit.listPostfixCompletionTemplates".to_string(),
        postfix_list,
    );

    {
        let bool_end = find_offset(&lib_src, "m > 0") + ("m > 0".len() as i64);
        let stmt_end = find_offset(&lib_src, "final m = math.max(1, 2);")
            + ("final m = math.max(1, 2);".len() as i64);
        let app_not = client.request(
            "edit.isPostfixCompletionApplicable",
            json!({"file": lib_file, "key": ".not", "offset": bool_end}),
        );
        steps.insert(
            "edit.isPostfixCompletionApplicable:not_true".to_string(),
            app_not,
        );
        let app_fori_false = client.request(
            "edit.isPostfixCompletionApplicable",
            json!({"file": lib_file, "key": ".fori", "offset": bool_end}),
        );
        steps.insert(
            "edit.isPostfixCompletionApplicable:fori_false".to_string(),
            app_fori_false,
        );
        let app_try = client.request(
            "edit.isPostfixCompletionApplicable",
            json!({"file": lib_file, "key": ".try", "offset": stmt_end}),
        );
        steps.insert(
            "edit.isPostfixCompletionApplicable:try_true".to_string(),
            app_try,
        );

        let expand_par = client.request(
            "edit.getPostfixCompletion",
            json!({"file": lib_file, "key": ".par", "offset": bool_end}),
        );
        steps.insert(
            "edit.getPostfixCompletion:par".to_string(),
            normalize_paths(expand_par, &root_str),
        );
        let expand_try = client.request(
            "edit.getPostfixCompletion",
            json!({"file": lib_file, "key": ".try", "offset": stmt_end}),
        );
        steps.insert(
            "edit.getPostfixCompletion:try".to_string(),
            normalize_paths(expand_try, &root_str),
        );
        let expand_unknown = client.request(
            "edit.getPostfixCompletion",
            json!({"file": lib_file, "key": ".nonexistent", "offset": bool_end}),
        );
        steps.insert(
            "edit.getPostfixCompletion:unknown_key".to_string(),
            expand_unknown,
        );
    }

    {
        let stmt_offset = find_offset(&lib_src, "final m = math.max(1, 2)");
        let stmt_comp = client.request(
            "edit.getStatementCompletion",
            json!({"file": lib_file, "offset": stmt_offset}),
        );
        steps.insert(
            "edit.getStatementCompletion:ok".to_string(),
            normalize_paths(stmt_comp, &root_str),
        );
        let stmt_comp_bad = client.request(
            "edit.getStatementCompletion",
            json!({"file": "relative.dart", "offset": 0}),
        );
        steps.insert(
            "edit.getStatementCompletion:invalid_path".to_string(),
            stmt_comp_bad,
        );
    }

    {
        let imp_existing = client.request(
            "edit.importElements",
            json!({
                "file": lib_file,
                "elements": [{
                    "path": helper_file,
                    "prefix": "",
                    "elements": ["HelperBox"]
                }]
            }),
        );
        steps.insert(
            "edit.importElements:existing".to_string(),
            normalize_paths(imp_existing, &root_str),
        );
        let imp_bad = client.request(
            "edit.importElements",
            json!({
                "file": root.join("pubspec.yaml"),
                "elements": []
            }),
        );
        steps.insert(
            "edit.importElements:invalid_file".to_string(),
            normalize_paths(imp_bad, &root_str),
        );
    }

    {
        let edit_src = std::fs::read_to_string(&edit_file).unwrap();
        let unused_imp_offset = find_offset(&edit_src, "import 'dart:async';");
        let fixes_resp = client.request(
            "edit.getFixes",
            json!({"file": edit_file, "offset": unused_imp_offset}),
        );
        steps.insert(
            "edit.getFixes:unused_import".to_string(),
            normalize_paths(fixes_resp, &root_str),
        );
        let fixes_bad = client.request(
            "edit.getFixes",
            json!({"file": "relative.dart", "offset": 0}),
        );
        steps.insert("edit.getFixes:invalid_path".to_string(), fixes_bad);
    }

    {
        let op_offset = find_offset(&lib_src, "> 0");
        let assists_resp = client.request(
            "edit.getAssists",
            json!({"file": lib_file, "offset": op_offset, "length": 0}),
        );
        steps.insert(
            "edit.getAssists:exchange_operands".to_string(),
            normalize_paths(assists_resp, &root_str),
        );
    }

    {
        for (label, needle, len) in [
            ("local_var_box", "box = HelperBox<T>(seed)", 3),
            ("method_mixed", "mixed() => tag;", 5),
            ("getter_tag", "tag => 1;", 3),
        ] {
            let offset = find_offset(&lib_src, needle);
            let mut resp = client.request(
                "edit.getAvailableRefactorings",
                json!({"file": lib_file, "offset": offset, "length": len}),
            );
            if let Some(kinds) = resp["result"]["kinds"].as_array_mut() {
                kinds.sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));
            }
            steps.insert(format!("edit.getAvailableRefactorings:{label}"), resp);
        }
    }

    // edit.getRefactoring for each kind
    {
        // RENAME local variable
        let box_offset = find_offset(&lib_src, "box = HelperBox<T>(seed)");
        let rename_val = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "RENAME",
                "file": lib_file,
                "offset": box_offset,
                "length": 3,
                "validateOnly": true
            }),
        );
        steps.insert(
            "edit.getRefactoring:RENAME_validate".to_string(),
            normalize_paths(rename_val, &root_str),
        );
        let rename_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "RENAME",
                "file": lib_file,
                "offset": box_offset,
                "length": 3,
                "validateOnly": false,
                "options": {"newName": "renamedBox"}
            }),
        );
        steps.insert(
            "edit.getRefactoring:RENAME_exec".to_string(),
            normalize_paths(rename_exec, &root_str),
        );

        // EXTRACT_LOCAL_VARIABLE
        let max_offset = find_offset(&lib_src, "math.max(1, 2)");
        let max_len = "math.max(1, 2)".len() as i64;
        let extract_local_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "EXTRACT_LOCAL_VARIABLE",
                "file": lib_file,
                "offset": max_offset,
                "length": max_len,
                "validateOnly": false,
                "options": {"name": "maxVal", "extractAll": true}
            }),
        );
        steps.insert(
            "edit.getRefactoring:EXTRACT_LOCAL_VARIABLE".to_string(),
            normalize_paths(extract_local_exec, &root_str),
        );

        // INLINE_LOCAL_VARIABLE
        let m_offset = find_offset(&lib_src, "m = math.max(1, 2)");
        let inline_local_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "INLINE_LOCAL_VARIABLE",
                "file": lib_file,
                "offset": m_offset,
                "length": 1,
                "validateOnly": false
            }),
        );
        steps.insert(
            "edit.getRefactoring:INLINE_LOCAL_VARIABLE".to_string(),
            normalize_paths(inline_local_exec, &root_str),
        );

        // CONVERT_METHOD_TO_GETTER
        let mixed_offset = find_offset(&lib_src, "mixed() => tag;");
        let conv_m2g = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "CONVERT_METHOD_TO_GETTER",
                "file": lib_file,
                "offset": mixed_offset,
                "length": 5,
                "validateOnly": false
            }),
        );
        steps.insert(
            "edit.getRefactoring:CONVERT_METHOD_TO_GETTER".to_string(),
            normalize_paths(conv_m2g, &root_str),
        );

        // CONVERT_GETTER_TO_METHOD
        let tag_offset = find_offset(&lib_src, "tag => 2;");
        let conv_g2m = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "CONVERT_GETTER_TO_METHOD",
                "file": lib_file,
                "offset": tag_offset,
                "length": 3,
                "validateOnly": false
            }),
        );
        steps.insert(
            "edit.getRefactoring:CONVERT_GETTER_TO_METHOD".to_string(),
            normalize_paths(conv_g2m, &root_str),
        );

        // EXTRACT_METHOD
        let extract_method_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "EXTRACT_METHOD",
                "file": lib_file,
                "offset": max_offset,
                "length": max_len,
                "validateOnly": false,
                "options": {
                    "returnType": "int",
                    "createGetter": false,
                    "name": "computeMax",
                    "parameters": [],
                    "extractAll": true
                }
            }),
        );
        steps.insert(
            "edit.getRefactoring:EXTRACT_METHOD".to_string(),
            normalize_paths(extract_method_exec, &root_str),
        );

        // INLINE_METHOD
        let inline_method_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "INLINE_METHOD",
                "file": lib_file,
                "offset": mixed_offset,
                "length": 5,
                "validateOnly": false,
                "options": {
                    "deleteSource": false,
                    "inlineAll": true
                }
            }),
        );
        steps.insert(
            "edit.getRefactoring:INLINE_METHOD".to_string(),
            normalize_paths(inline_method_exec, &root_str),
        );

        // MOVE_FILE
        let move_file_exec = client.request(
            "edit.getRefactoring",
            json!({
                "kind": "MOVE_FILE",
                "file": helper_file,
                "offset": 0,
                "length": 0,
                "validateOnly": false,
                "options": {
                    "newFile": root.join("lib/moved_helper.dart")
                }
            }),
        );
        steps.insert(
            "edit.getRefactoring:MOVE_FILE".to_string(),
            normalize_paths(move_file_exec, &root_str),
        );
    }

    // edit.formatIfEnabled & edit.bulkFixes
    {
        let fmt_enabled = client.request("edit.formatIfEnabled", json!({"directories": [root]}));
        steps.insert(
            "edit.formatIfEnabled".to_string(),
            normalize_paths(fmt_enabled, &root_str),
        );

        let bulk_ok = client.request(
            "edit.bulkFixes",
            json!({"included": [edit_file], "codes": ["unused_import"]}),
        );
        steps.insert(
            "edit.bulkFixes:unused_import".to_string(),
            normalize_paths(bulk_ok, &root_str),
        );

        let bulk_bad = client.request(
            "edit.bulkFixes",
            json!({"included": [root.join("lib/does_not_exist.dart")]}),
        );
        steps.insert(
            "edit.bulkFixes:nonexistent_error".to_string(),
            normalize_paths(bulk_bad, &root_str),
        );
    }

    // 22. lsp.handle: textDocument/completion, completionItem/resolve, textDocument/codeAction
    {
        let (line, col) = find_line_col(&lib_src, "box.item");
        let comp_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 22,
                    "method": "textDocument/completion",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "position": {"line": line, "character": col + ("box.".len() as u32)}
                    }
                }
            }),
        );
        let item_candidate = comp_resp["result"]["lspResponse"]["result"]["items"]
            .as_array()
            .or_else(|| comp_resp["result"]["lspResponse"]["result"].as_array())
            .and_then(|items| items.iter().find(|i| i["label"] == "item"))
            .cloned()
            .unwrap_or_else(|| json!({"label": "item"}));
        let mut norm_comp = normalize_paths(comp_resp, &root_str);
        if let Some(items) = norm_comp
            .pointer_mut("/result/lspResponse/result/items")
            .and_then(Value::as_array_mut)
        {
            items.sort_by(|a, b| {
                a["label"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["label"].as_str().unwrap_or(""))
            });
        }
        steps.insert("lsp.handle:completion".to_string(), norm_comp);

        let resolve_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 23,
                    "method": "completionItem/resolve",
                    "params": item_candidate
                }
            }),
        );
        steps.insert(
            "lsp.handle:completionItem/resolve".to_string(),
            normalize_paths(resolve_resp, &root_str),
        );

        let (op_line, op_col) = find_line_col(&lib_src, "> 0");
        let code_action_resp = client.request(
            "lsp.handle",
            json!({
                "lspMessage": {
                    "jsonrpc": "2.0",
                    "id": 24,
                    "method": "textDocument/codeAction",
                    "params": {
                        "textDocument": {"uri": lib_uri},
                        "range": {
                            "start": {"line": op_line, "character": op_col},
                            "end": {"line": op_line, "character": op_col}
                        },
                        "context": {"diagnostics": []}
                    }
                }
            }),
        );
        steps.insert(
            "lsp.handle:codeAction".to_string(),
            normalize_paths(code_action_resp, &root_str),
        );
    }

    for event in [
        "analysis.navigation",
        "analysis.highlights",
        "analysis.occurrences",
        "analysis.outline",
        "analysis.implemented",
        "analysis.overrides",
        "analysis.closingLabels",
        "flutter.outline",
    ] {
        let map = client.notifications.get(event).cloned().unwrap_or_default();
        let mut normalized = Map::new();
        for (file, payload) in map {
            let key = file.replace(&root_str, "${ROOT}");
            normalized.insert(key, normalize_paths(payload, &root_str));
        }
        steps.insert(format!("notification:{event}"), Value::Object(normalized));
    }

    {
        let map = client
            .notifications
            .get("completion.existingImports")
            .cloned()
            .unwrap_or_default();
        let mut normalized = Map::new();
        for (file, payload) in map {
            let key = file.replace(&root_str, "${ROOT}");
            normalized.insert(key, normalize_paths(payload, &root_str));
        }
        steps.insert(
            "notification:completion.existingImports".to_string(),
            Value::Object(normalized),
        );
    }

    client.request("server.shutdown", json!({}));
    let (_, exit_code) = client.finish();
    assert_eq!(exit_code, 0);

    ExtendedTranscript { steps }
}

#[test]
fn extended_legacy_requests_and_notifications_match_dart_3_13_3() {
    let Some(dart) = dart_binary() else {
        eprintln!("skipped: Dart 3.13.3 is not available");
        return;
    };
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("dartr-legacy-ext-{unique}"));
    let lib_dir = temp_dir.join("lib");
    std::fs::create_dir_all(&lib_dir).unwrap();

    std::fs::write(
        temp_dir.join("pubspec.yaml"),
        "name: legacy_ext\nenvironment:\n  sdk: ^3.5.0\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("helper.dart"),
        "library legacy_ext.helper;\n\nabstract class Iface<T> {\n  T compute(T input);\n  int get tag;\n}\n\nclass HelperBox<E extends num> {\n  final E item;\n  const HelperBox(this.item);\n}\n\n/// Formats a greeting.\nString formatGreeting(String name, {int repeat = 1, required String prefix}) =>\n    '$prefix $name' * repeat;\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("part_a.dart"),
        "part of 'lib.dart';\n\nextension IntExt on int {\n  int triple() => this * 3;\n}\n\nclass PartChild extends Sub<int> {\n  PartChild(super.seed);\n\n  @override\n  int compute(int input) => super.compute(input) + input.triple();\n}\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("lib.dart"),
        "library legacy_ext.lib;\n\nimport 'dart:math' as math;\n\nimport 'helper.dart';\n\npart 'part_a.dart';\n\nabstract class Base<T extends num> implements Iface<T> {\n  final T seed;\n  Base(this.seed);\n\n  @override\n  int get tag => 1;\n\n  T helperValue(T x) => x;\n}\n\nmixin Mix<T extends num> on Base<T> {\n  int mixed() => tag;\n}\n\n/// A generic concrete class.\nclass Sub<T extends num> extends Base<T> with Mix<T> {\n  Sub(super.seed);\n\n  @override\n  int get tag => 2;\n\n  @override\n  T compute(T input) {\n    final box = HelperBox<T>(seed);\n    final m = math.max(1, 2);\n    if (m > 0) {\n      return helperValue(box.item);\n    }\n    return input;\n  }\n}\n\nint runDemo(Sub<int> s, dynamic dynTarget) {\n  final c = PartChild(s.seed);\n  final p = dynTarget.compute(3);\n  final msg = formatGreeting('hi', prefix: 'p');\n  final boxes = <HelperBox<int>>[\n    HelperBox<int>(\n      msg.length,\n    ),\n  ];\n  return c.compute(s.mixed()) + (p as int) + boxes.length;\n}\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("edit_target.dart"),
        "import 'helper.dart';\nimport 'dart:async';\nimport 'dart:math';\n\nclass Messy   {\n  void bMethod() {}\n  int zField = 1;\n  Messy(this.aField);\n  static int sField = 2;\n  int aField;\n  void aMethod(int x) {\n    final v = max(x, aField);\n    print(v);\n  }\n}\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("broken.dart"),
        "void broken( {\n  print('unclosed';\n}\n",
    )
    .unwrap();

    let canonical_root = temp_dir.canonicalize().unwrap();
    let args = [
        "language-server",
        "--protocol=analyzer",
        "--client-id=legacy-parity",
    ];
    let dart_tx = run_extended_session(&dart, &args, &canonical_root);
    let dartr_tx = run_extended_session(
        Path::new(env!("CARGO_BIN_EXE_dartr")),
        &args,
        &canonical_root,
    );
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Both servers must answer every scripted step; a session that lost steps
    // would otherwise compare two short lists and pass.
    let dart_keys: Vec<_> = dart_tx.steps.keys().collect();
    let dartr_keys: Vec<_> = dartr_tx.steps.keys().collect();
    assert_eq!(
        dartr_keys, dart_keys,
        "dart and dartr recorded different steps"
    );
    assert!(
        dart_tx.steps.len() >= 138,
        "dart recorded only {} steps",
        dart_tx.steps.len()
    );

    let mut diffs = Vec::new();
    for (key, dart_val) in &dart_tx.steps {
        let dartr_val = dartr_tx.steps.get(key).expect("missing step in dartr");
        if dart_val == dartr_val {
            println!("PASS {key}: identical");
        } else {
            println!("DIFF {key}:\n  dart:  {dart_val}\n  dartr: {dartr_val}");
            diffs.push(key.clone());
        }
    }
    assert!(diffs.is_empty(), "differences in steps: {diffs:?}");
}

fn find_property<'a>(props: &'a [Value], name: &str) -> &'a Value {
    props
        .iter()
        .find(|p| p["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("property `{name}` not found"))
}

fn run_flutter_session(program: &Path, args: &[&str], root: &Path) -> ExtendedTranscript {
    let root_str = root.to_string_lossy().to_string();
    let main_file = root.join("lib/main.dart");
    let main_src = std::fs::read_to_string(&main_file).unwrap();

    let mut client = LegacyClient::spawn(program, args);
    client.wait_for_event("server.connected");

    client.request(
        "server.setSubscriptions",
        json!({"subscriptions": ["STATUS"]}),
    );
    client.request(
        "analysis.setSubscriptions",
        json!({"subscriptions": {"OUTLINE": [&main_file]}}),
    );
    let flutter_sub_resp = client.request(
        "flutter.setSubscriptions",
        json!({"subscriptions": {"OUTLINE": [&main_file]}}),
    );
    client.request("analysis.setPriorityFiles", json!({"files": [&main_file]}));
    let completed = client.analysis_completed;
    client.request(
        "analysis.setAnalysisRoots",
        json!({"included": [root], "excluded": []}),
    );
    client.settle(completed, true);

    let mut steps = BTreeMap::new();
    steps.insert("flutter.setSubscriptions".to_string(), flutter_sub_resp);

    // 1. flutter.getWidgetDescription on Text('Hello', ...)
    let text_offset = find_offset(&main_src, "Text(\n          'Hello'");
    let text_desc = client.request(
        "flutter.getWidgetDescription",
        json!({"file": main_file, "offset": text_offset}),
    );
    let text_props = text_desc["result"]["properties"]
        .as_array()
        .expect("Text properties")
        .clone();
    steps.insert(
        "flutter.getWidgetDescription:Text".to_string(),
        normalize_paths(text_desc, &root_str),
    );

    // Modify existing intValue property (maxLines)
    let max_lines_id = find_property(&text_props, "maxLines")["id"]
        .as_i64()
        .unwrap();
    let set_max_lines = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": max_lines_id, "value": {"intValue": 3}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:Text_maxLines_int".to_string(),
        normalize_paths(set_max_lines, &root_str),
    );

    // Remove optional property (maxLines -> null)
    let remove_max_lines = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": max_lines_id}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:Text_maxLines_remove".to_string(),
        normalize_paths(remove_max_lines, &root_str),
    );

    // Modify existing boolValue property (softWrap)
    let soft_wrap_id = find_property(&text_props, "softWrap")["id"]
        .as_i64()
        .unwrap();
    let set_soft_wrap = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": soft_wrap_id, "value": {"boolValue": false}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:Text_softWrap_bool".to_string(),
        normalize_paths(set_soft_wrap, &root_str),
    );

    // Set unset optional String property (semanticsLabel)
    let sem_label_id = find_property(&text_props, "semanticsLabel")["id"]
        .as_i64()
        .unwrap();
    let set_sem_label = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": sem_label_id, "value": {"stringValue": "greeting"}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:Text_semanticsLabel_insert".to_string(),
        normalize_paths(set_sem_label, &root_str),
    );

    // Attempt to remove required positional property (data -> null)
    let data_id = find_property(&text_props, "data")["id"].as_i64().unwrap();
    let remove_required = client.request("flutter.setWidgetPropertyValue", json!({"id": data_id}));
    steps.insert(
        "flutter.setWidgetPropertyValue:Text_data_remove_required_error".to_string(),
        remove_required,
    );

    // Attempt invalid expression (unformatted edit fallback)
    let invalid_expr = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": max_lines_id, "value": {"expression": ")bad("}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:invalid_expression_error".to_string(),
        normalize_paths(invalid_expr, &root_str),
    );

    // Materialize virtual Container property (width on Text)
    let container_prop = find_property(&text_props, "Container");
    let container_children = container_prop["children"]
        .as_array()
        .expect("Container children");
    let virt_width_id = find_property(container_children, "width")["id"]
        .as_i64()
        .unwrap();
    let set_virt_width = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": virt_width_id, "value": {"doubleValue": 120.0}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:VirtualContainer_width".to_string(),
        normalize_paths(set_virt_width, &root_str),
    );

    // 2. flutter.getWidgetDescription on Padding(padding: EdgeInsets.all(8.0), ...)
    let padding_offset = find_offset(&main_src, "Padding(");
    let padding_desc = client.request(
        "flutter.getWidgetDescription",
        json!({"file": main_file, "offset": padding_offset}),
    );
    let padding_props = padding_desc["result"]["properties"]
        .as_array()
        .expect("Padding properties")
        .clone();
    steps.insert(
        "flutter.getWidgetDescription:Padding".to_string(),
        normalize_paths(padding_desc, &root_str),
    );

    let padding_prop = find_property(&padding_props, "padding");
    let padding_children = padding_prop["children"]
        .as_array()
        .expect("padding EdgeInsets children");
    let left_id = find_property(padding_children, "left")["id"]
        .as_i64()
        .unwrap();
    let set_left = client.request(
        "flutter.setWidgetPropertyValue",
        json!({"id": left_id, "value": {"doubleValue": 16.0}}),
    );
    steps.insert(
        "flutter.setWidgetPropertyValue:EdgeInsets_left".to_string(),
        normalize_paths(set_left, &root_str),
    );

    // 3. flutter.getWidgetDescription on Container(...)
    let container_offset = find_offset(&main_src, "Container(\n          width: 100.0");
    let container_desc = client.request(
        "flutter.getWidgetDescription",
        json!({"file": main_file, "offset": container_offset}),
    );
    steps.insert(
        "flutter.getWidgetDescription:Container".to_string(),
        normalize_paths(container_desc, &root_str),
    );

    // 4. EXTRACT_WIDGET refactoring on Padding(...)
    let mut avail_widget = client.request(
        "edit.getAvailableRefactorings",
        json!({"file": main_file, "offset": padding_offset, "length": 0}),
    );
    if let Some(kinds) = avail_widget["result"]["kinds"].as_array_mut() {
        kinds.sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));
    }
    steps.insert(
        "edit.getAvailableRefactorings:Padding".to_string(),
        avail_widget,
    );
    let extract_widget_resp = client.request(
        "edit.getRefactoring",
        json!({
            "kind": "EXTRACT_WIDGET",
            "file": main_file,
            "offset": padding_offset,
            "length": 0,
            "validateOnly": false,
            "options": {"name": "ExtractedPadding"}
        }),
    );
    steps.insert(
        "edit.getRefactoring:EXTRACT_WIDGET".to_string(),
        normalize_paths(extract_widget_resp, &root_str),
    );

    for event in ["analysis.outline", "flutter.outline"] {
        let map = client.notifications.get(event).cloned().unwrap_or_default();
        let mut normalized = Map::new();
        for (file, payload) in map {
            let key = file.replace(&root_str, "${ROOT}");
            normalized.insert(key, normalize_paths(payload, &root_str));
        }
        steps.insert(format!("notification:{event}"), Value::Object(normalized));
    }

    client.request("server.shutdown", json!({}));
    let (_, exit_code) = client.finish();
    assert_eq!(exit_code, 0);

    ExtendedTranscript { steps }
}

#[test]
fn flutter_legacy_requests_and_outline_match_dart_3_13_3() {
    let Some(dart) = dart_binary() else {
        eprintln!("skipped: Dart 3.13.3 is not available");
        return;
    };
    let Some(flutter_root) = flutter_sdk_root(&dart) else {
        eprintln!("skipped: Flutter SDK not found next to dart");
        return;
    };

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("dartr-legacy-flutter-{unique}"));
    let lib_dir = temp_dir.join("lib");
    std::fs::create_dir_all(&lib_dir).unwrap();

    std::fs::write(
        temp_dir.join("pubspec.yaml"),
        "name: flutter_fixture\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )
    .unwrap();

    let pub_status = Command::new(&dart)
        .args(["pub", "get", "--offline"])
        .current_dir(&temp_dir)
        .env("FLUTTER_ROOT", &flutter_root)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .expect("failed to run dart pub get");
    assert!(pub_status.success(), "dart pub get --offline failed");

    std::fs::write(
        lib_dir.join("main.dart"),
        r#"import 'package:flutter/widgets.dart';

class DemoWidget extends StatelessWidget {
  final Widget extraChild;

  const DemoWidget({super.key, required this.extraChild});

  @override
  Widget build(BuildContext context) {
    final localLabel = Text('Local');
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: <Widget>[
        Text(
          'Hello',
          maxLines: 2,
          softWrap: true,
          textAlign: TextAlign.center,
          style: TextStyle(fontSize: 14.0),
        ),
        Padding(
          padding: EdgeInsets.all(8.0),
          child: Text('Padded'),
        ),
        Container(
          width: 100.0,
          alignment: Alignment.center,
          child: Icon(const IconData(0xe000)),
        ),
        localLabel,
        extraChild,
      ],
    );
  }
}
"#,
    )
    .unwrap();

    let canonical_root = temp_dir.canonicalize().unwrap();
    let args = [
        "language-server",
        "--protocol=analyzer",
        "--client-id=legacy-parity",
    ];
    let dart_tx = run_flutter_session(&dart, &args, &canonical_root);
    let dartr_tx = run_flutter_session(
        Path::new(env!("CARGO_BIN_EXE_dartr")),
        &args,
        &canonical_root,
    );
    let _ = std::fs::remove_dir_all(&temp_dir);

    let dart_keys: Vec<_> = dart_tx.steps.keys().collect();
    let dartr_keys: Vec<_> = dartr_tx.steps.keys().collect();
    assert_eq!(
        dartr_keys, dart_keys,
        "dart and dartr recorded different flutter steps"
    );
    assert!(
        dart_tx.steps.len() >= 15,
        "dart recorded only {} flutter steps",
        dart_tx.steps.len()
    );

    let mut diffs = Vec::new();
    for (key, dart_val) in &dart_tx.steps {
        let dartr_val = dartr_tx.steps.get(key).expect("missing step in dartr");
        if dart_val == dartr_val {
            println!("PASS {key}: identical");
        } else {
            println!("DIFF {key}:\n  dart:  {dart_val}\n  dartr: {dartr_val}");
            diffs.push(key.clone());
        }
    }
    assert!(diffs.is_empty(), "differences in flutter steps: {diffs:?}");
}
