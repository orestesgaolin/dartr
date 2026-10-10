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
            analysis_started: 0,
            analysis_completed: 0,
            analyzing: false,
            flushed: BTreeSet::new(),
        }
    }

    fn send_request(&mut self, method: &str, params: Value) -> String {
        self.next_id += 1;
        let id = self.next_id.to_string();
        let message = json!({"id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().expect("server stdin");
        serde_json::to_writer(&mut *stdin, &message).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
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
                | "analysis.folding"),
            ) => {
                let params = &message["params"];
                if let Some(file) = params["file"].as_str() {
                    self.notifications
                        .entry(event.to_string())
                        .or_default()
                        .insert(file.to_string(), params.clone());
                }
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
            }
        }),
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

    // 3. analysis.getReachableSources
    let reachable_resp = client.request("analysis.getReachableSources", json!({"file": lib_file}));
    steps.insert(
        "analysis.getReachableSources".to_string(),
        normalize_paths(reachable_resp, &root_str),
    );

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

    for event in [
        "analysis.navigation",
        "analysis.highlights",
        "analysis.occurrences",
        "analysis.outline",
        "analysis.implemented",
        "analysis.overrides",
    ] {
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
        "library legacy_ext.helper;\n\nabstract class Iface<T> {\n  T compute(T input);\n  int get tag;\n}\n\nclass HelperBox<E extends num> {\n  final E item;\n  const HelperBox(this.item);\n}\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("part_a.dart"),
        "part of 'lib.dart';\n\nextension IntExt on int {\n  int triple() => this * 3;\n}\n\nclass PartChild extends Sub<int> {\n  PartChild(super.seed);\n\n  @override\n  int compute(int input) => super.compute(input) + input.triple();\n}\n",
    )
    .unwrap();

    std::fs::write(
        lib_dir.join("lib.dart"),
        "library legacy_ext.lib;\n\nimport 'dart:math' as math;\n\nimport 'helper.dart';\n\npart 'part_a.dart';\n\nabstract class Base<T extends num> implements Iface<T> {\n  final T seed;\n  Base(this.seed);\n\n  @override\n  int get tag => 1;\n\n  T helperValue(T x) => x;\n}\n\nmixin Mix<T extends num> on Base<T> {\n  int mixed() => tag;\n}\n\n/// A generic concrete class.\nclass Sub<T extends num> extends Base<T> with Mix<T> {\n  Sub(super.seed);\n\n  @override\n  int get tag => 2;\n\n  @override\n  T compute(T input) {\n    final box = HelperBox<T>(seed);\n    final m = math.max(1, 2);\n    if (m > 0) {\n      return helperValue(box.item);\n    }\n    return input;\n  }\n}\n\nint runDemo(Sub<int> s, dynamic dynTarget) {\n  final c = PartChild(s.seed);\n  final p = dynTarget.compute(3);\n  return c.compute(s.mixed()) + (p as int);\n}\n",
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
