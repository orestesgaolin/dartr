//! Differential test of the language server: one scripted LSP session
//! (Dart-Code's `initialize`, open/edit/close of fixture files with syntax
//! errors, requests of the implemented methods, workspace folder changes,
//! shutdown) runs against `dart language-server` (3.13.3, needs `dart` on
//! `PATH`), `dartr language-server`, and `dart tools/shim/dartr_shim.dart
//! --lsp`. The normalized transcripts must be equal.
//!
//! Normalization: request ids, `jsonrpc` and versions are dropped; the
//! notifications are compared as the last value per document after each
//! step (the order of notifications and `$/progress` is not compared);
//! diagnostics are restricted to syntactic diagnostics (`SYNTACTIC_ERROR`
//! codes) until dartr has resolution.
//!
//! `cargo test -p dartr --test lsp_parity -- --nocapture` prints the
//! per-step report.

mod lsp_support;

use std::collections::BTreeMap;
use std::path::Path;

use dartr_diagnostics::{DiagnosticType, codes_by_name};
use lsp_support::*;
use serde_json::{Value, json};

/// A normalized transcript: step name and value.
type Transcript = Vec<(String, Value)>;

fn is_syntactic(code: &str) -> bool {
    codes_by_name(code)
        .iter()
        .any(|c| c.diagnostic_type == DiagnosticType::SyntacticError)
}

/// Whether [uri] is a non-Dart file with diagnostics of the context manager.
fn is_non_dart(uri: &str) -> bool {
    ["analysis_options.yaml", "pubspec.yaml", "AndroidManifest.xml"]
        .iter()
        .any(|n| uri.ends_with(&format!("/{n}")))
}

/// The document state, normalized: syntactic diagnostics (and all
/// diagnostics of non-Dart files) only, empty lists
/// removed, only documents of the fixture project.
fn snapshot(c: &LspClient, root_uri: &str) -> Value {
    snapshot_where(c, root_uri, is_syntactic)
}

/// Like [snapshot], with the codes that [keep] accepts.
fn snapshot_where(c: &LspClient, root_uri: &str, keep: fn(&str) -> bool) -> Value {
    let in_root = |uri: &String| uri.starts_with(root_uri);
    let mut diagnostics = BTreeMap::new();
    for (uri, list) in &c.state.diagnostics {
        let list: Vec<Value> = list
            .iter()
            .filter(|d| is_non_dart(uri) || d["code"].as_str().is_some_and(keep))
            .cloned()
            .collect();
        if in_root(uri) && !list.is_empty() {
            diagnostics.insert(uri.trim_start_matches(root_uri).to_string(), list);
        }
    }
    let strip = |m: &BTreeMap<String, Value>| -> BTreeMap<String, Value> {
        m.iter()
            .filter(|(u, _)| in_root(u))
            .map(|(u, v)| (u.trim_start_matches(root_uri).to_string(), v.clone()))
            .collect()
    };
    json!({
        "diagnostics": diagnostics,
        "closingLabels": strip(&c.state.closing_labels),
        "outlines": strip(&c.state.outlines),
        "flutterOutlines": strip(&c.state.flutter_outlines),
    })
}

fn doc(uri: &str) -> Value {
    json!({"textDocument": {"uri": uri}})
}

fn position(line: u32, character: u32) -> Value {
    json!({"line": line, "character": character})
}

/// Runs the session on [c] and returns the transcript and the exit code.
fn run_session(mut c: LspClient, root: &Path) -> (Transcript, i32) {
    let root_uri = format!("{}/", file_uri(root));
    let uri = |rel: &str| format!("{root_uri}{rel}");
    let errors = uri("lib/errors.dart");
    let shapes = uri("lib/shapes.dart");
    let widgets = uri("lib/widgets.dart");
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    let mut t: Transcript = Vec::new();

    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    t.push(("initial analysis".into(), snapshot(&c, &root_uri)));

    for (u, rel) in [(&errors, "lib/errors.dart"), (&shapes, "lib/shapes.dart"), (&widgets, "lib/widgets.dart")] {
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": u, "languageId": "dart", "version": 1, "text": read(rel)}}),
        );
    }
    c.settle(true);
    t.push(("open 3 files".into(), snapshot(&c, &root_uri)));

    for u in [&errors, &shapes, &widgets] {
        let name = u.trim_start_matches(&root_uri);
        t.push((
            format!("documentSymbol {name}"),
            c.request("textDocument/documentSymbol", doc(u)),
        ));
        t.push((
            format!("foldingRange {name}"),
            c.request("textDocument/foldingRange", doc(u)),
        ));
    }
    let positions: Vec<Value> = [
        (0, 0),
        (12, 20),
        (24, 10),
        (35, 22),
        (43, 25),
        (60, 8),
        (75, 12),
        (90, 6),
        (104, 15),
    ]
    .iter()
    .map(|&(l, ch)| position(l, ch))
    .collect();
    t.push((
        "selectionRange shapes.dart".into(),
        c.request(
            "textDocument/selectionRange",
            json!({"textDocument": {"uri": shapes}, "positions": positions}),
        ),
    ));
    t.push((
        "selectionRange errors.dart".into(),
        c.request(
            "textDocument/selectionRange",
            json!({"textDocument": {"uri": errors}, "positions": [position(2, 10), position(9, 9), position(13, 0)]}),
        ),
    ));
    t.push((
        "selectionRange widgets.dart".into(),
        c.request(
            "textDocument/selectionRange",
            json!({"textDocument": {"uri": widgets}, "positions": [position(26, 15), position(28, 18), position(37, 10)]}),
        ),
    ));
    t.push((
        "selectionRange invalid line".into(),
        c.request(
            "textDocument/selectionRange",
            json!({"textDocument": {"uri": shapes}, "positions": [position(9999, 0)]}),
        ),
    ));
    t.push((
        "documentSymbol of a file outside the roots".into(),
        c.request("textDocument/documentSymbol", doc("file:///tmp/dartr_not_analyzed.dart")),
    ));
    t.push((
        "foldingRange of a file outside the roots".into(),
        c.request("textDocument/foldingRange", doc("file:///tmp/dartr_not_analyzed.dart")),
    ));
    t.push((
        "documentSymbol of a non-file URI".into(),
        c.request("textDocument/documentSymbol", doc("untitled:Untitled-1")),
    ));

    // Fix the missing parenthesis of `print('missing paren';`.
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": errors, "version": 2}, "contentChanges": [
            {"range": {"start": position(2, 25), "end": position(2, 25)}, "text": ")"}
        ]}),
    );
    c.settle(true);
    t.push(("incremental change errors.dart".into(), snapshot(&c, &root_uri)));

    // Two changes in one notification: break the class body of `Shape` and
    // add an unterminated string at the end.
    let shapes_text = read("lib/shapes.dart");
    let shapes_lines = shapes_text.lines().count() as u32;
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": shapes, "version": 2}, "contentChanges": [
            {"range": {"start": position(21, 0), "end": position(22, 0)}, "text": "  /// The ärea 😀.\n  double get area => ;\n"},
            {"range": {"start": position(shapes_lines + 1, 0), "end": position(shapes_lines + 1, 0)}, "text": "var s = 'unterminated\n"}
        ]}),
    );
    c.settle(true);
    t.push(("two changes in shapes.dart".into(), snapshot(&c, &root_uri)));
    t.push((
        "documentSymbol shapes.dart after change".into(),
        c.request("textDocument/documentSymbol", doc(&shapes)),
    ));
    t.push((
        "foldingRange shapes.dart after change".into(),
        c.request("textDocument/foldingRange", doc(&shapes)),
    ));

    // Full content change.
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": shapes, "version": 3}, "contentChanges": [{"text": shapes_text}]}),
    );
    c.settle(true);
    t.push(("full change shapes.dart".into(), snapshot(&c, &root_uri)));

    // An open file that analysis_options.yaml excludes and no file imports:
    // analyzed as a priority file, but no notifications.
    let excluded = uri("lib/generated/excluded.dart");
    c.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": excluded, "languageId": "dart", "version": 1, "text": read("lib/generated/excluded.dart")}}),
    );
    c.settle(true);
    t.push(("open excluded file".into(), snapshot(&c, &root_uri)));
    t.push((
        "documentSymbol excluded file".into(),
        c.request("textDocument/documentSymbol", doc(&excluded)),
    ));

    c.notify("textDocument/didClose", doc(&errors));
    c.settle(true);
    t.push(("close errors.dart".into(), snapshot(&c, &root_uri)));

    // A second workspace folder.
    let root2 = root.parent().unwrap().join("lsp_project2");
    let folder2 = json!({"uri": file_uri(&root2), "name": "lsp_project2"});
    let root2_uri = format!("{}/", file_uri(&root2));
    c.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [folder2], "removed": []}}),
    );
    c.settle(true);
    t.push(("add workspace folder".into(), snapshot(&c, &root2_uri)));
    c.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [], "removed": [folder2]}}),
    );
    // Both servers analyze again after a change of the roots.
    c.settle(true);
    t.push(("remove workspace folder".into(), snapshot(&c, &root2_uri)));
    t.push(("first workspace folder after remove".into(), snapshot(&c, &root_uri)));

    c.notify("workspace/didChangeConfiguration", json!({"settings": null}));
    c.settle(false);
    t.push((
        "didChangeConfiguration: server requests".into(),
        json!(c.server_requests.last().map(|r| r.0.clone())),
    ));

    t.push((
        "unknown request".into(),
        c.request("dart/noSuchMethod", json!({})),
    ));
    t.push((
        "user-visible messages".into(),
        Value::Array(c.messages.clone()),
    ));

    if let Some(dir) = std::env::var_os("DARTR_LSP_TRANSCRIPTS") {
        let path = Path::new(&dir).join(format!("lsp_log_{}.json", c.id()));
        let log: Vec<Value> = c.log.clone();
        std::fs::write(&path, serde_json::to_string_pretty(&log).unwrap()).unwrap();
    }
    let (response, code) = c.shutdown_and_exit();
    t.push(("shutdown".into(), response));
    t.push(("exit code".into(), json!(code)));
    (t, code)
}

/// The first difference of [a] and [b] (a JSON pointer and both values).
fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let (va, vb) = (x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null));
                if let Some(d) = first_difference(va, vb, &format!("{path}/{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let (va, vb) = (x.get(i).unwrap_or(&Value::Null), y.get(i).unwrap_or(&Value::Null));
                if let Some(d) = first_difference(va, vb, &format!("{path}/{i}")) {
                    return Some(d);
                }
            }
            None
        }
        _ if a == b => None,
        _ => Some(format!("{path}: dart={a} dartr={b}")),
    }
}

/// Compares the transcripts and prints the report. Returns the number of
/// differing steps.
fn report(label: &str, expected: &Transcript, actual: &Transcript) -> usize {
    println!("== {label}");
    let mut failures = 0;
    for ((name, e), (name2, a)) in expected.iter().zip(actual) {
        assert_eq!(name, name2);
        match first_difference(e, a, "") {
            None => println!("  PARITY  {name}"),
            Some(d) => {
                failures += 1;
                println!("  DIFF    {name}\n          {d}");
            }
        }
    }
    println!("  {} of {} steps at parity", expected.len() - failures, expected.len());
    failures
}

fn dartr_bin() -> &'static str {
    env!("CARGO_BIN_EXE_dartr")
}

fn session_args() -> Vec<&'static str> {
    vec!["--client-id=VS-Code", "--client-version=3.144.0"]
}

#[test]
fn lsp_session_parity_with_dart_language_server() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = fixtures().join("lsp_project");
    let mut dart_args = vec!["language-server", "--protocol=lsp"];
    dart_args.extend(session_args());
    let (dart, dart_code) = run_session(LspClient::spawn("dart", &dart_args, &[]), &root);
    let mut dartr_args = vec!["language-server", "--protocol=lsp"];
    dartr_args.extend(session_args());
    let (dartr, dartr_code) = run_session(LspClient::spawn(dartr_bin(), &dartr_args, &[]), &root);
    assert_eq!(dart_code, 0);
    assert_eq!(dartr_code, 0);
    let failures = report("dart language-server vs dartr language-server", &dart, &dartr);
    if std::env::var_os("DARTR_LSP_TRANSCRIPTS").is_some() {
        let dir = std::path::PathBuf::from(std::env::var_os("DARTR_LSP_TRANSCRIPTS").unwrap());
        for (name, t) in [("dart", &dart), ("dartr", &dartr)] {
            let path = dir.join(format!("lsp_transcript_{name}.json"));
            std::fs::write(&path, serde_json::to_string_pretty(&json!(t)).unwrap()).unwrap();
            println!("transcript: {}", path.display());
        }
    }
    assert_eq!(failures, 0, "LSP session differs from dart language-server");
}

#[test]
fn lsp_session_through_dart_shim() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = fixtures().join("lsp_project");
    let shim = repo_root().join("tools/shim/dartr_shim.dart");
    let shim = shim.to_str().unwrap();
    // Dart-Code: `dart <analyzerPath> --lsp --client-id=... --client-version=...`.
    let mut shim_args = vec![shim, "--lsp"];
    shim_args.extend(session_args());
    let (via_shim, shim_code) = run_session(
        LspClient::spawn("dart", &shim_args, &[("DARTR_BIN", dartr_bin())]),
        &root,
    );
    let mut direct_args = vec!["language-server", "--lsp"];
    direct_args.extend(session_args());
    let (direct, direct_code) =
        run_session(LspClient::spawn(dartr_bin(), &direct_args, &[]), &root);
    assert_eq!(shim_code, direct_code);
    let failures = report("dartr language-server vs dart dartr_shim.dart --lsp", &direct, &via_shim);
    assert_eq!(failures, 0);

    // Exit codes are forwarded: `exit` without `shutdown` is 1.
    let mut c = LspClient::spawn("dart", &shim_args, &[("DARTR_BIN", dartr_bin())]);
    c.request("initialize", dart_code_initialize_params(&root));
    c.notify("exit", Value::Null);
    assert_eq!(c.close(), 1);
    // A usage error is 64, with the message on stderr.
    let out = std::process::Command::new("dart")
        .args([shim, "--no-such-option"])
        .env("DARTR_BIN", dartr_bin())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Could not find an option named"));
}

const FIXTURES: &str = "../dartr_project/tests/fixtures/diagnostics";

/// A package with a package language version of 2.19 (records are an error
/// there) and broken `analysis_options.yaml`, `pubspec.yaml` and
/// `AndroidManifest.xml` (from the `dartr_project` fixtures).
fn write_non_dart_project() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lsp_non_dart");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("android")).unwrap();
    let root = root.canonicalize().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES);
    let copy = |from: &str, to: &str| {
        std::fs::copy(fixtures.join(from), root.join(to)).unwrap();
    };
    std::fs::write(
        root.join("analysis_options.yaml"),
        "analyzer:\n  mystery: true\n  errors:\n    no_such_diagnostic: warning\n  language:\n    strict-casts: maybe\n  optional-checks:\n    chrome-os-manifest-checks: true\nlinter:\n  rules:\n    - no_such_lint\n    - avoid_print\n    - avoid_print\n",
    )
    .unwrap();
    copy("pubspec_fields/pubspec.yaml", "pubspec.yaml");
    copy("manifest_required/AndroidManifest.xml", "android/AndroidManifest.xml");
    std::fs::write(
        root.join(".dart_tool/package_config.json"),
        "{\"configVersion\":2,\"packages\":[{\"name\":\"p\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"2.19\"}]}",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/old.dart"),
        "var r = (1, 2);\nsealed class S {}\nvar t = 1 >>> 2;\nvoid main() { print(1) }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/new.dart"),
        "// @dart = 3.0\nvar r = (1, 2);\nsealed class S {}\nvoid main() { print(1) }\n",
    )
    .unwrap();
    // A byte order mark in the text of an open document is ignored.
    std::fs::write(
        root.join("lib/bom.dart"),
        "\u{feff}void main() { var x = }\n",
    )
    .unwrap();
    root
}

fn run_non_dart_session(mut c: LspClient, root: &Path) -> (Transcript, i32) {
    let root_uri = format!("{}/", file_uri(root));
    let uri = |rel: &str| format!("{root_uri}{rel}");
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    let mut t: Transcript = Vec::new();
    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    t.push(("initial analysis".into(), snapshot(&c, &root_uri)));

    for (rel, language) in [
        ("analysis_options.yaml", "yaml"),
        ("pubspec.yaml", "yaml"),
        ("android/AndroidManifest.xml", "xml"),
        ("lib/old.dart", "dart"),
        ("lib/new.dart", "dart"),
        ("lib/bom.dart", "dart"),
    ] {
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri(rel), "languageId": language, "version": 1, "text": read(rel)}}),
        );
    }
    c.settle(true);
    t.push(("open all files".into(), snapshot(&c, &root_uri)));

    // Change the options in the editor: the overlay is analyzed.
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri("analysis_options.yaml"), "version": 2}, "contentChanges": [
            {"text": "analyzer:\n  errors:\n    no_such_diagnostic: ignore\n  language:\n    strict-raw-types: sometimes\n"}
        ]}),
    );
    c.settle(true);
    t.push(("change analysis_options.yaml".into(), snapshot(&c, &root_uri)));

    // Change the pubspec and the manifest.
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri("pubspec.yaml"), "version": 2}, "contentChanges": [
            {"text": "name: p\nauthor: someone\n"}
        ]}),
    );
    c.settle(false);
    t.push(("change pubspec.yaml".into(), snapshot(&c, &root_uri)));
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri("android/AndroidManifest.xml"), "version": 2}, "contentChanges": [
            {"text": "<manifest><uses-feature android:name=\"android.hardware.camera\" /></manifest>"}
        ]}),
    );
    c.settle(false);
    t.push(("change AndroidManifest.xml".into(), snapshot(&c, &root_uri)));

    for rel in ["analysis_options.yaml", "pubspec.yaml", "android/AndroidManifest.xml"] {
        c.notify("textDocument/didClose", doc(&uri(rel)));
    }
    c.settle(false);
    t.push(("close non-Dart files".into(), snapshot(&c, &root_uri)));
    if let Some(dir) = std::env::var_os("DARTR_LSP_TRANSCRIPTS") {
        let path = Path::new(&dir).join(format!("lsp_log_{}.json", c.id()));
        std::fs::write(&path, serde_json::to_string_pretty(&c.log).unwrap()).unwrap();
    }
    let (response, code) = c.shutdown_and_exit();
    t.push(("shutdown".into(), response));
    (t, code)
}

#[test]
fn lsp_non_dart_files_and_package_language_version() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_non_dart_project();
    let mut dart_args = vec!["language-server", "--protocol=lsp"];
    dart_args.extend(session_args());
    let (dart, dart_code) = run_non_dart_session(LspClient::spawn("dart", &dart_args, &[]), &root);
    let (dartr, dartr_code) =
        run_non_dart_session(LspClient::spawn(dartr_bin(), &dart_args, &[]), &root);
    assert_eq!(dart_code, 0);
    assert_eq!(dartr_code, 0);
    let failures = report("non-Dart files, language version", &dart, &dartr);
    assert_eq!(failures, 0, "LSP session differs from dart language-server");
}

/// Syntactic diagnostics, lints (AST-only rules are implemented) and the
/// ignore-comment diagnostics.
fn is_syntactic_or_lint(code: &str) -> bool {
    is_syntactic(code)
        || matches!(code, "duplicate_ignore" | "unignorable_ignore")
        || codes_by_name(code)
            .iter()
            .any(|c| c.diagnostic_type == DiagnosticType::Lint)
}

fn write_lint_project() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lsp_lints");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    let root = root.canonicalize().unwrap();
    let write = |rel: &str, content: &str| std::fs::write(root.join(rel), content).unwrap();
    write("pubspec.yaml", "name: p\nenvironment:\n  sdk: ^3.9.0\n");
    write(
        ".dart_tool/package_config.json",
        "{\"configVersion\":2,\"packages\":[{\"name\":\"p\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"3.13\"}]}",
    );
    write(
        "analysis_options.yaml",
        "analyzer:\n  errors:\n    camel_case_types: error\n    empty_statements: ignore\n    always_declare_return_types: warning\n  cannot-ignore:\n    - unnecessary_new\nlinter:\n  rules:\n    - always_declare_return_types\n    - camel_case_types\n    - empty_statements\n    - unnecessary_new\n    - constant_identifier_names\n    - unawaited_futures\n",
    );
    write(
        "lib/main_lib.dart",
        "part 'main_part.dart';\n\nclass lib_class {}\nclass ignored_class {} // ignore: camel_case_types\nf() {;}\nObject o() => new Object(); // ignore: unnecessary_new\nint x() => 1 // ignore: expected_token\n",
    );
    write(
        "lib/main_part.dart",
        "part of 'main_lib.dart';\n\nclass part_class {}\nconst my_const = 1;\n// ignore: type=lint\nconst other_const = 2;\n",
    );
    write("lib/plain.dart", "int ok() => 1;\n");
    root
}

fn run_lint_session(mut c: LspClient, root: &Path) -> (Transcript, i32) {
    let root_uri = format!("{}/", file_uri(root));
    let uri = |rel: &str| format!("{root_uri}{rel}");
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    let snap = |c: &LspClient| snapshot_where(c, &root_uri, is_syntactic_or_lint);
    let mut t: Transcript = Vec::new();
    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    t.push(("initial analysis".into(), snap(&c)));

    for rel in ["lib/main_lib.dart", "lib/main_part.dart", "lib/plain.dart"] {
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri(rel), "languageId": "dart", "version": 1, "text": read(rel)}}),
        );
    }
    c.settle(true);
    t.push(("open files".into(), snap(&c)));

    // A new lint and a new ignore comment in the part (an overlay).
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri("lib/main_part.dart"), "version": 2}, "contentChanges": [
            {"text": "part of 'main_lib.dart';\n\nclass part_class {}\nclass second_part {}\ng() {}\n// ignore: always_declare_return_types, always_declare_return_types\nh() {}\nObject n() => new Object();\n"}
        ]}),
    );
    c.settle(true);
    t.push(("change part".into(), snap(&c)));

    // A change of the defining unit: the lints of the part still run on
    // the library.
    c.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri("lib/main_lib.dart"), "version": 2}, "contentChanges": [
            {"text": "part 'main_part.dart';\n\nclass lib_class {}\nclass Fine {}\nf() {}\n"}
        ]}),
    );
    c.settle(true);
    t.push(("change library".into(), snap(&c)));

    c.notify("textDocument/didClose", doc(&uri("lib/main_part.dart")));
    c.settle(true);
    t.push(("close part".into(), snap(&c)));
    let (response, code) = c.shutdown_and_exit();
    t.push(("shutdown".into(), response));
    (t, code)
}

#[test]
fn lsp_lints_and_error_processors() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_lint_project();
    let mut args = vec!["language-server", "--protocol=lsp"];
    args.extend(session_args());
    let (dart, dart_code) = run_lint_session(LspClient::spawn("dart", &args, &[]), &root);
    let (dartr, dartr_code) = run_lint_session(LspClient::spawn(dartr_bin(), &args, &[]), &root);
    assert_eq!(dart_code, 0);
    assert_eq!(dartr_code, 0);
    // The comparison is not empty: the real server reports lints.
    let opened = &dart.iter().find(|(n, _)| n == "change part").unwrap().1;
    let count: usize = opened["diagnostics"]
        .as_object()
        .unwrap()
        .values()
        .map(|l| l.as_array().unwrap().len())
        .sum();
    assert!(count >= 5, "expected lint diagnostics: {opened}");
    let failures = report("lints and errors: processors", &dart, &dartr);
    assert_eq!(failures, 0, "LSP session differs from dart language-server");
}

/// The environment variable that enables the formatting parity steps that
/// need real formatter output. The formatting styles of `dartr_format`
/// (phase 10 "p10-format") are stubs until they are merged: until then
/// dartr returns `null` for every file that needs changes. Without the
/// variable, only the steps whose result does not depend on the formatter
/// output run (syntax errors, unknown and non-Dart files, disabled
/// formatter, trigger checks of on-type formatting, already formatted
/// files).
const FORMAT_PARITY_ENV: &str = "DARTR_FORMAT_PARITY";

fn format_parity_enabled() -> bool {
    // The formatting styles are ported: always compare formatted output.
    // `DARTR_FORMAT_PARITY=0` skips these steps.
    std::env::var_os(FORMAT_PARITY_ENV).is_none_or(|v| v != "0")
}

/// A package with files that need formatting, a nested package whose
/// `analysis_options.yaml` sets `formatter: page_width` and
/// `trailing_commas`, a file at language version 3.6 (short style) and a
/// file with syntax errors.
fn write_format_project() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lsp_format");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join("wide/lib")).unwrap();
    let root = root.canonicalize().unwrap();
    let write = |rel: &str, content: &str| std::fs::write(root.join(rel), content).unwrap();
    let needs = "import 'dart:math';\nclass  A{\nint x=1  ;\n  void f( int a,int b ){ if(a>b){print( 'a' );} else {print('b');}\n  }\n  List<int> get items => [1,2,3,];\n}\nvoid main(){var a=A();a.f(1,2);\n  var veryLongVariableNameNumberOne = max(1000000, 2000000) + max(3000000, 4000000) + 5;\n// comment   \n  print(veryLongVariableNameNumberOne);}\n";
    write("pubspec.yaml", "name: p\nenvironment:\n  sdk: ^3.13.0\n");
    write("lib/needs.dart", needs);
    write("lib/formatted.dart", "class A {\n  int x = 1;\n}\n");
    write("lib/broken.dart", "void main() {\n  var x = ;\n  print(x)\n}\n");
    write(
        "lib/short_style.dart",
        "// @dart = 3.6\nvoid f(int a,int b,{int c=1}){var list=[a,b,c,];print(list);}\n",
    );
    write(
        "lib/unicode_crlf.dart",
        "// \u{1F600} \u{e9}moji\r\nvar  s = '\u{1F600}';\r\nvoid f( ){print( s );   /* \u{1F600} */ print(s);}\r\n",
    );
    write("wide/pubspec.yaml", "name: wide\nenvironment:\n  sdk: ^3.13.0\n");
    write(
        "wide/analysis_options.yaml",
        "formatter:\n  page_width: 120\n  trailing_commas: preserve\n",
    );
    write("wide/lib/needs.dart", needs);
    root
}

const FORMATTING_OPTIONS: &str = r#"{"tabSize": 2, "insertSpaces": true}"#;

fn formatting_params(uri: &str) -> Value {
    json!({"textDocument": {"uri": uri}, "options": serde_json::from_str::<Value>(FORMATTING_OPTIONS).unwrap()})
}

fn range_formatting_params(uri: &str, start: (u32, u32), end: (u32, u32)) -> Value {
    let mut p = formatting_params(uri);
    p["range"] = json!({"start": position(start.0, start.1), "end": position(end.0, end.1)});
    p
}

fn on_type_params(uri: &str, at: (u32, u32), ch: &str) -> Value {
    let mut p = formatting_params(uri);
    p["position"] = position(at.0, at.1);
    p["ch"] = json!(ch);
    p
}

/// The requests of the server since index [from]: the method, and for
/// (un)registrations the formatting entries without ids (the Dart server
/// registers more features, so the ids differ).
fn server_requests_since(c: &LspClient, from: usize) -> Value {
    let formatting = |list: &Value| -> Value {
        list.as_array()
            .unwrap()
            .iter()
            .filter(|r| r["method"].as_str().unwrap().contains("ormatting"))
            .map(|r| {
                let mut r = r.clone();
                r.as_object_mut().unwrap().remove("id");
                r
            })
            .collect()
    };
    Value::Array(
        c.server_requests[from..]
            .iter()
            .map(|(m, p)| match m.as_str() {
                "client/registerCapability" => json!([m, formatting(&p["registrations"])]),
                "client/unregisterCapability" => json!([m, formatting(&p["unregisterations"])]),
                _ => json!([m]),
            })
            .collect(),
    )
}

fn run_format_session(mut c: LspClient, root: &Path, with_formatter: bool) -> (Transcript, i32) {
    let root_uri = format!("{}/", file_uri(root));
    let uri = |rel: &str| format!("{root_uri}{rel}");
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    let mut t: Transcript = Vec::new();
    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    t.push((
        "registrations after initialized".into(),
        server_requests_since(&c, 0)
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r[0] == json!("client/registerCapability"))
            .cloned()
            .collect(),
    ));
    let needs = uri("lib/needs.dart");
    let broken = uri("lib/broken.dart");
    let formatted = uri("lib/formatted.dart");
    // An open document: requests use the overlay.
    c.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": broken, "languageId": "dart", "version": 1, "text": read("lib/broken.dart")}}),
    );
    c.settle(true);

    // Results that do not depend on the formatter output.
    t.push(("formatting: syntax errors".into(), c.request("textDocument/formatting", formatting_params(&broken))));
    t.push((
        "rangeFormatting: syntax errors".into(),
        c.request("textDocument/rangeFormatting", range_formatting_params(&broken, (1, 0), (2, 5))),
    ));
    t.push((
        "onTypeFormatting: syntax errors".into(),
        c.request("textDocument/onTypeFormatting", on_type_params(&broken, (1, 11), ";")),
    ));
    t.push((
        "formatting: already formatted".into(),
        c.request("textDocument/formatting", formatting_params(&formatted)),
    ));
    t.push((
        "formatting: missing file".into(),
        c.request("textDocument/formatting", formatting_params(&uri("lib/missing.dart"))),
    ));
    t.push((
        "formatting: non-Dart file".into(),
        c.request("textDocument/formatting", formatting_params(&uri("pubspec.yaml"))),
    ));
    t.push((
        "formatting: non-file URI".into(),
        c.request("textDocument/formatting", formatting_params("untitled:Untitled-1")),
    ));
    // Not a trigger: `;` and `}` that do not end a statement or block, and
    // another character.
    t.push((
        "onTypeFormatting: ';' not at a statement end".into(),
        c.request("textDocument/onTypeFormatting", on_type_params(&needs, (2, 4), ";")),
    ));
    t.push((
        "onTypeFormatting: '}' inside a string".into(),
        c.request("textDocument/onTypeFormatting", on_type_params(&needs, (3, 41), "}")),
    ));
    t.push((
        "onTypeFormatting: other character".into(),
        c.request("textDocument/onTypeFormatting", on_type_params(&needs, (2, 10), "x")),
    ));
    t.push((
        "onTypeFormatting: line after the end".into(),
        c.request("textDocument/onTypeFormatting", on_type_params(&needs, (900, 0), ";")),
    ));

    if with_formatter {
        let wide = uri("wide/lib/needs.dart");
        for (name, u) in [
            ("needs.dart", &needs),
            ("wide/lib/needs.dart (analysis options)", &wide),
            ("short_style.dart (language version 3.6)", &uri("lib/short_style.dart")),
            ("unicode_crlf.dart", &uri("lib/unicode_crlf.dart")),
        ] {
            t.push((format!("formatting {name}"), c.request("textDocument/formatting", formatting_params(u))));
        }
        t.push((
            "rangeFormatting in the middle".into(),
            c.request("textDocument/rangeFormatting", range_formatting_params(&needs, (2, 0), (4, 3))),
        ));
        t.push((
            "rangeFormatting: invalid line".into(),
            c.request("textDocument/rangeFormatting", range_formatting_params(&needs, (200, 0), (400, 0))),
        ));
        t.push((
            "onTypeFormatting after '}'".into(),
            c.request("textDocument/onTypeFormatting", on_type_params(&needs, (4, 3), "}")),
        ));
        t.push((
            "onTypeFormatting after ';'".into(),
            c.request("textDocument/onTypeFormatting", on_type_params(&needs, (2, 10), ";")),
        ));
        // An edited open document is formatted from the overlay.
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": needs, "languageId": "dart", "version": 1, "text": read("lib/needs.dart")}}),
        );
        c.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": needs, "version": 2}, "contentChanges": [
                {"range": {"start": position(11, 0), "end": position(11, 0)}, "text": "int   g( )=>1;\n"}
            ]}),
        );
        c.settle(true);
        t.push((
            "formatting an edited overlay".into(),
            c.request("textDocument/formatting", formatting_params(&needs)),
        ));
        // `dart.lineLength` from `workspace/configuration` (global and
        // workspace folder); the analysis options of `wide` win.
        c.configuration = json!({"lineLength": 40});
        c.notify("workspace/didChangeConfiguration", json!({"settings": null}));
        c.settle(false);
        t.push((
            "formatting with dart.lineLength 40".into(),
            c.request("textDocument/formatting", formatting_params(&needs)),
        ));
        t.push((
            "formatting with dart.lineLength 40, page_width in analysis options".into(),
            c.request("textDocument/formatting", formatting_params(&wide)),
        ));
    }

    // `dart.enableSdkFormatter: false`: the formatter is unregistered and
    // requests return `null`, also for missing files.
    let before = c.server_requests.len();
    c.configuration = json!({"enableSdkFormatter": false});
    c.notify("workspace/didChangeConfiguration", json!({"settings": null}));
    c.settle(false);
    t.push(("enableSdkFormatter false: server requests".into(), server_requests_since(&c, before)));
    t.push(("formatting: disabled".into(), c.request("textDocument/formatting", formatting_params(&needs))));
    t.push((
        "formatting: disabled, missing file".into(),
        c.request("textDocument/formatting", formatting_params(&uri("lib/missing.dart"))),
    ));
    let before = c.server_requests.len();
    c.configuration = json!({});
    c.notify("workspace/didChangeConfiguration", json!({"settings": null}));
    c.settle(false);
    t.push(("enableSdkFormatter default: server requests".into(), server_requests_since(&c, before)));

    let (response, code) = c.shutdown_and_exit();
    t.push(("shutdown".into(), response));
    (t, code)
}

#[test]
fn lsp_formatting_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let with_formatter = format_parity_enabled();
    if !with_formatter {
        println!(
            "formatting steps with formatter output skipped: they need the ported formatter \
             styles (p10-format); set {FORMAT_PARITY_ENV}=1 to run them"
        );
    }
    let root = write_format_project();
    let mut args = vec!["language-server", "--protocol=lsp"];
    args.extend(session_args());
    let (dart, dart_code) =
        run_format_session(LspClient::spawn("dart", &args, &[]), &root, with_formatter);
    let (dartr, dartr_code) =
        run_format_session(LspClient::spawn(dartr_bin(), &args, &[]), &root, with_formatter);
    assert_eq!(dart_code, 0);
    assert_eq!(dartr_code, 0);
    if let Some(dir) = std::env::var_os("DARTR_LSP_TRANSCRIPTS") {
        for (name, t) in [("dart", &dart), ("dartr", &dartr)] {
            let path = Path::new(&dir).join(format!("lsp_format_transcript_{name}.json"));
            std::fs::write(&path, serde_json::to_string_pretty(&json!(t)).unwrap()).unwrap();
            println!("transcript: {}", path.display());
        }
    }
    // The comparison is not empty: the real server formats.
    if with_formatter {
        let full = &dart.iter().find(|(n, _)| n == "formatting needs.dart").unwrap().1;
        assert!(full["result"].as_array().is_some_and(|e| e.len() > 10), "{full}");
    }
    let failures = report("formatting", &dart, &dartr);
    assert_eq!(failures, 0, "LSP session differs from dart language-server");
}
