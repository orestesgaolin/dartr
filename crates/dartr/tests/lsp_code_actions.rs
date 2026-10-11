//! Differential tests of `textDocument/codeAction` and
//! `workspace/executeCommand`: the same requests against
//! `dart language-server` and `dartr language-server`.
//!
//! - Quick fixes: one request per diagnostic that `dart` publishes (range
//!   of the diagnostic, `only: ["quickfix"]`), on the fixture project
//!   `lsp_fixtures/fixes_project` and (with `DARTR_LSP_FIX_CORPUS=<folder>
//!   [:<max files>]`) on a sample of real files, opened with their first
//!   `package:` import removed (so that names are unresolved).
//! - Assists: one request per word start of each file (empty range,
//!   `only: ["refactor"]`); the actions with a `dart.assist.*` id are
//!   compared (the refactorings are not).
//! - Source actions: `only: ["source"]`, then the `Organize Imports`,
//!   `Sort Members` and `Fix All` commands; the `workspace/applyEdit` of
//!   each command is compared.
//!
//! The report has one line per fix kind (the `dart.logAction` id of the
//! action): identical, differing, missing in dartr, extra in dartr.
//! `cargo test --release -p dartr --test lsp_code_actions -- --nocapture`.

mod lsp_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lsp_support::*;
use serde_json::{Value, json};

fn dartr_bin() -> &'static str {
    match std::env::var("DARTR_LSP_NAV_BIN") {
        Ok(bin) => Box::leak(bin.into_boxed_str()),
        Err(_) => env!("CARGO_BIN_EXE_dartr"),
    }
}

fn session_args() -> Vec<&'static str> {
    vec![
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ]
}

/// A file to open: path and content.
struct OpenFile {
    path: PathBuf,
    content: String,
}

/// One code action request.
#[derive(Clone)]
struct FixRequest {
    uri: String,
    /// The label of the diagnostic (code and position).
    label: String,
    range: Value,
    diagnostic: Value,
}

/// The results of one server.
#[derive(Default)]
struct Results {
    fixes: Vec<Value>,
    assists: Vec<Value>,
    source_actions: Vec<Value>,
    commands: Vec<(Value, Option<Value>)>,
    /// The workspace commands (`WORKSPACE_COMMANDS`).
    workspace_commands: Vec<(Value, Option<Value>)>,
}

/// The commands executed once for the workspace.
const WORKSPACE_COMMANDS: [&str; 2] = [
    "dart.edit.fixAllInWorkspace.preview",
    "dart.edit.fixAllInWorkspace",
];

fn start(program: &str, root: &Path, files: &[OpenFile]) -> LspClient {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    c.request("initialize", dart_code_initialize_params(root));
    c.notify("initialized", json!({}));
    c.settle_with_grace(std::time::Duration::from_secs(120));
    for f in files {
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": file_uri(&f.path),
                "languageId": "dart",
                "version": 1,
                "text": f.content,
            }}),
        );
    }
    c.settle_with_grace(std::time::Duration::from_secs(60));
    c
}

/// The requests of the diagnostics that [c] published for [files].
fn fix_requests(c: &LspClient, files: &[OpenFile]) -> Vec<FixRequest> {
    let mut out = Vec::new();
    for f in files {
        let uri = file_uri(&f.path);
        for d in c.state.diagnostics.get(&uri).cloned().unwrap_or_default() {
            let label = format!(
                "{}:{}:{} {}",
                f.path.file_name().unwrap().to_string_lossy(),
                d["range"]["start"]["line"],
                d["range"]["start"]["character"],
                d["code"].as_str().unwrap_or("")
            );
            out.push(FixRequest {
                uri: uri.clone(),
                label,
                range: d["range"].clone(),
                diagnostic: d.clone(),
            });
        }
    }
    out
}

/// The assist requests: an empty range at each word start of [files].
fn assist_requests(files: &[OpenFile]) -> Vec<FixRequest> {
    let mut out = Vec::new();
    for f in files {
        let uri = file_uri(&f.path);
        let name = f.path.file_name().unwrap().to_string_lossy().to_string();
        for (line, text) in f.content.lines().enumerate() {
            let mut previous: Option<char> = None;
            for (column, ch) in text.encode_utf16().enumerate() {
                let ch = char::from_u32(ch as u32).unwrap_or('?');
                let word = ch.is_alphanumeric() || ch == '_' || ch == '\'' || ch == '"';
                let after_word = previous.is_some_and(|p| p.is_alphanumeric() || p == '_');
                if word && !after_word {
                    let position = json!({"line": line, "character": column});
                    out.push(FixRequest {
                        uri: uri.clone(),
                        label: format!("{name}:{line}:{column} assist"),
                        range: json!({"start": position, "end": position}),
                        diagnostic: Value::Null,
                    });
                }
                previous = Some(ch);
            }
        }
    }
    out
}

fn run(
    c: &mut LspClient,
    requests: &[FixRequest],
    assists: &[FixRequest],
    files: &[OpenFile],
) -> Results {
    let mut results = Results::default();
    for r in assists {
        let response = c.request(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": r.uri},
                "range": r.range,
                "context": {"diagnostics": [], "only": ["refactor"]},
            }),
        );
        let mut response = response;
        if let Some(actions) = response["result"].as_array_mut() {
            actions.retain(|a| action_id(a).starts_with("dart.assist"));
        }
        results.assists.push(response);
    }
    for r in requests {
        let response = c.request(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": r.uri},
                "range": r.range,
                "context": {"diagnostics": [r.diagnostic], "only": ["quickfix"]},
            }),
        );
        results.fixes.push(response);
    }
    for f in files {
        let uri = file_uri(&f.path);
        let response = c.request(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": uri},
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "context": {"diagnostics": [], "only": ["source"]},
            }),
        );
        results.source_actions.push(response);
        for command in COMMANDS {
            let before = c.server_requests.len();
            let response = c.request(
                "workspace/executeCommand",
                json!({"command": command, "arguments": [{"path": f.path.display().to_string()}]}),
            );
            let edit = c.server_requests[before..]
                .iter()
                .find(|(m, _)| m == "workspace/applyEdit")
                .map(|(_, p)| p.clone());
            results.commands.push((response, edit));
        }
    }
    if std::env::var_os("DARTR_LSP_FIX_CORPUS").is_none() {
        for command in WORKSPACE_COMMANDS {
            let before = c.server_requests.len();
            let response = c.request(
                "workspace/executeCommand",
                json!({"command": command, "arguments": []}),
            );
            let edit = c.server_requests[before..]
                .iter()
                .find(|(m, _)| m == "workspace/applyEdit")
                .map(|(_, p)| p.clone());
            results.workspace_commands.push((response, edit));
        }
    }
    results
}

/// The commands executed for each file.
const COMMANDS: [&str; 3] = [
    "dart.edit.organizeImports",
    "dart.edit.sortMembers",
    "dart.edit.fixAll",
];

/// The fix kind of an action (the argument of `dart.logAction`, else the
/// LSP kind).
fn action_id(a: &Value) -> String {
    a.pointer("/command/arguments/0/action")
        .and_then(Value::as_str)
        .or_else(|| a.get("kind").and_then(Value::as_str))
        .unwrap_or("?")
        .to_string()
}

#[derive(Default)]
struct Tally {
    identical: usize,
    differing: usize,
    missing: usize,
    extra: usize,
}

/// Compares the results and prints the report; returns the tally by fix
/// kind and the differences.
fn compare(
    label: &str,
    requests: &[FixRequest],
    assists: &[FixRequest],
    dart: &Results,
    dartr: &Results,
) -> (BTreeMap<String, Tally>, Vec<String>) {
    let mut tally: BTreeMap<String, Tally> = BTreeMap::new();
    let mut problems = Vec::new();
    let pairs = requests
        .iter()
        .zip(dart.fixes.iter().zip(&dartr.fixes))
        .chain(assists.iter().zip(dart.assists.iter().zip(&dartr.assists)));
    for (r, (dart_response, dartr_response)) in pairs {
        let d = dart_response["result"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let x = dartr_response["result"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for a in &d {
            let title = a["title"].as_str().unwrap_or("");
            let t = tally.entry(action_id(a)).or_default();
            match x.iter().find(|b| b["title"].as_str() == Some(title)) {
                None => {
                    t.missing += 1;
                    if std::env::var_os("DARTR_LSP_FIX_MISSING").is_some() {
                        problems.push(format!(
                            "{} missing: {title}\n  dart:  {}",
                            r.label,
                            truncate(a)
                        ));
                    }
                }
                Some(b) if b == a => t.identical += 1,
                Some(b) => {
                    t.differing += 1;
                    problems.push(format!(
                        "{} {title}\n  dart:  {}\n  dartr: {}",
                        r.label,
                        truncate(a),
                        truncate(b)
                    ));
                }
            }
        }
        for b in &x {
            let title = b["title"].as_str().unwrap_or("");
            if !d.iter().any(|a| a["title"].as_str() == Some(title)) {
                tally.entry(action_id(b)).or_default().extra += 1;
                problems.push(format!("{} extra: {title}", r.label));
            }
        }
        // The order of the actions that both servers return.
        let dt: Vec<&str> = d
            .iter()
            .filter_map(|a| a["title"].as_str())
            .filter(|t| x.iter().any(|b| b["title"].as_str() == Some(*t)))
            .collect();
        let xt: Vec<&str> = x
            .iter()
            .filter_map(|a| a["title"].as_str())
            .filter(|t| d.iter().any(|a| a["title"].as_str() == Some(*t)))
            .collect();
        if dt != xt {
            problems.push(format!("{} order: dart {dt:?} dartr {xt:?}", r.label));
        }
    }
    for (i, (d, x)) in dart
        .source_actions
        .iter()
        .zip(&dartr.source_actions)
        .enumerate()
    {
        let t = tally.entry("source actions".into()).or_default();
        if d == x {
            t.identical += 1;
        } else {
            t.differing += 1;
            problems.push(format!(
                "source actions {i}: dart {} dartr {}",
                truncate(d),
                truncate(x)
            ));
        }
    }
    for (i, (d, x)) in dart.commands.iter().zip(&dartr.commands).enumerate() {
        let name = COMMANDS[i % COMMANDS.len()].trim_start_matches("dart.edit.");
        let t = tally.entry(format!("command {name}")).or_default();
        if d == x {
            t.identical += 1;
        } else {
            t.differing += 1;
            problems.push(format!(
                "command {name} {i}: dart {} {} dartr {} {}",
                truncate(&d.0),
                d.1.as_ref().map(truncate).unwrap_or_default(),
                truncate(&x.0),
                x.1.as_ref().map(truncate).unwrap_or_default()
            ));
        }
    }
    for (i, (d, x)) in dart
        .workspace_commands
        .iter()
        .zip(&dartr.workspace_commands)
        .enumerate()
    {
        let name = WORKSPACE_COMMANDS[i].trim_start_matches("dart.edit.");
        let t = tally.entry(format!("command {name}")).or_default();
        if d == x {
            t.identical += 1;
        } else {
            t.differing += 1;
            problems.push(format!(
                "command {name}: dart {} {} dartr {} {}",
                truncate(&d.0),
                d.1.as_ref().map(truncate).unwrap_or_default(),
                truncate(&x.0),
                x.1.as_ref().map(truncate).unwrap_or_default()
            ));
        }
    }
    eprintln!(
        "== {label}: {} diagnostics, {} assist positions",
        requests.len(),
        assists.len()
    );
    eprintln!(
        "{:<52} {:>6} {:>6} {:>6} {:>6}",
        "fix kind", "same", "diff", "miss", "extra"
    );
    for (id, t) in &tally {
        eprintln!(
            "{:<52} {:>6} {:>6} {:>6} {:>6}",
            id, t.identical, t.differing, t.missing, t.extra
        );
    }
    if std::env::var_os("DARTR_LSP_FIX_VERBOSE").is_some() {
        for p in &problems {
            eprintln!("{p}");
        }
    }
    (tally, problems)
}

fn truncate(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 1500 {
        format!("{}...", &s[..1500])
    } else {
        s
    }
}

fn run_both(
    label: &str,
    root: &Path,
    files: &[OpenFile],
) -> (BTreeMap<String, Tally>, Vec<String>) {
    let mut dart = start("dart", root, files);
    let requests = fix_requests(&dart, files);
    let assists = if std::env::var_os("DARTR_LSP_FIX_NO_ASSISTS").is_some() {
        Vec::new()
    } else {
        assist_requests(files)
    };
    let dart_results = run(&mut dart, &requests, &assists, files);
    let _ = dart.shutdown_and_exit();
    let mut dartr = start(dartr_bin(), root, files);
    let dartr_results = run(&mut dartr, &requests, &assists, files);
    let _ = dartr.shutdown_and_exit();
    compare(label, &requests, &assists, &dart_results, &dartr_results)
}

#[test]
fn code_actions_match_dart_language_server() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = fixtures().join("fixes_project");
    let mut files = Vec::new();
    for name in [
        "imports.dart",
        "creates.dart",
        "lints.dart",
        "warnings.dart",
        "data_driven.dart",
        "assists.dart",
        "more.dart",
        "ignores.dart",
    ] {
        let path = root.join("lib").join(name);
        let content = std::fs::read_to_string(&path).unwrap();
        files.push(OpenFile { path, content });
    }
    let (tally, problems) = run_both("fixes_project", &root, &files);
    // Every action that dartr returns must be identical (dartr does not
    // implement every producer: missing actions are allowed). The source
    // actions and the commands must be identical.
    let mut failures = Vec::new();
    for (id, t) in &tally {
        if t.differing + t.extra > 0 {
            failures.push(format!(
                "{id}: {} differing, {} extra",
                t.differing, t.extra
            ));
        }
    }
    if !failures.is_empty() {
        for p in &problems {
            eprintln!("{p}");
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The Dart files under [root] (sorted).
fn dart_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                if !name.starts_with('.') && name != "build" {
                    stack.push(p);
                }
            } else if name.ends_with(".dart")
                && !name.ends_with(".g.dart")
                && !name.ends_with(".freezed.dart")
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// [content] without its first `package:` import.
fn without_first_package_import(content: &str) -> Option<String> {
    let mut out = String::new();
    let mut removed = false;
    for line in content.split_inclusive('\n') {
        if !removed
            && line.trim_start().starts_with("import 'package:")
            && line.trim_end().ends_with(';')
        {
            removed = true;
            continue;
        }
        out.push_str(line);
    }
    removed.then_some(out)
}

/// `DARTR_LSP_FIX_CORPUS=<folder>[:<max files>]`: the fixes of a sample of
/// real files with their first `package:` import removed.
#[test]
fn code_actions_corpus() {
    let Some(spec) = std::env::var_os("DARTR_LSP_FIX_CORPUS") else {
        eprintln!("skipped: set DARTR_LSP_FIX_CORPUS=<folder>[:<max files>]");
        return;
    };
    let spec = spec.to_string_lossy().to_string();
    let mut parts = spec.split(':');
    let root = PathBuf::from(parts.next().unwrap()).canonicalize().unwrap();
    let max_files: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let lib = root.join("lib");
    let all = dart_files(if lib.is_dir() { &lib } else { &root });
    let mut files = Vec::new();
    let step = (all.len() / (max_files * 3).max(1)).max(1);
    for path in all.into_iter().step_by(step) {
        if files.len() >= max_files {
            break;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(content) = without_first_package_import(&content) {
            files.push(OpenFile { path, content });
        }
    }
    let label = format!("{} ({} files)", root.display(), files.len());
    let _ = run_both(&label, &root, &files);
}

#[derive(Clone)]
struct ParityStep {
    label: &'static str,
    method: &'static str,
    params: Value,
    expect_server_requests: bool,
}

fn sorted_workspace_edit(v: &Value) -> Value {
    let mut v = v.clone();
    for ptr in [
        "/result/documentChanges",
        "/edit/documentChanges",
        "/documentChanges",
    ] {
        if let Some(changes) = v.pointer_mut(ptr).and_then(Value::as_array_mut) {
            changes.sort_by_key(|c| {
                c.pointer("/textDocument/uri")
                    .or(c.get("uri"))
                    .or(c.get("oldUri"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            });
        }
    }
    v
}

fn run_parity_session(
    program: &str,
    init: Value,
    files: &[OpenFile],
    steps: &[ParityStep],
    out: &mut Vec<Value>,
) {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    let init_resp = c.request("initialize", init);
    assert!(
        init_resp["result"]["capabilities"].is_object(),
        "{init_resp}"
    );
    c.notify("initialized", json!({}));
    c.settle_with_grace(std::time::Duration::from_secs(120));
    for f in files {
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": file_uri(&f.path),
                "languageId": "dart",
                "version": 1,
                "text": f.content,
            }}),
        );
    }
    c.settle_with_grace(std::time::Duration::from_secs(60));

    for step in steps {
        let before = c.server_requests.len();
        let response = c.request(step.method, step.params.clone());
        let server_requests: Vec<Value> = c.server_requests[before..]
            .iter()
            .filter(|(m, p)| {
                !(m == "window/workDoneProgress/create"
                    && p.get("token") == Some(&json!("ANALYZING")))
            })
            .map(|(m, p)| {
                json!({
                    "method": m,
                    "params": sorted_workspace_edit(p),
                })
            })
            .collect();
        out.push(json!({
            "label": step.label,
            "expect_server_requests": step.expect_server_requests,
            "response": sorted_workspace_edit(&response),
            "server_requests": server_requests,
        }));
    }
    let _ = c.shutdown_and_exit();
}

fn utf16_offset_of(text: &str, needle: &str, delta: usize) -> usize {
    let byte = text
        .find(needle)
        .unwrap_or_else(|| panic!("missing {needle:?}"));
    text[..byte].encode_utf16().count() + delta
}

fn run_apply_and_interactive_resolve(program: &str, root: &Path) -> Vec<Value> {
    let warnings_path = root.join("lib").join("warnings.dart");
    let warnings_content = std::fs::read_to_string(&warnings_path).unwrap();
    let warnings_uri = file_uri(&warnings_path);
    let warnings_path_str = warnings_path.display().to_string();

    let assists_path = root.join("lib").join("assists.dart");
    let assists_content = std::fs::read_to_string(&assists_path).unwrap();
    let assists_path_str = assists_path.display().to_string();

    let ctor_offset = utf16_offset_of(&warnings_content, "C(this.name);", 0);
    let import_offset = utf16_offset_of(&warnings_content, "import 'src/helpers.dart';", 0);
    let class_offset = utf16_offset_of(&warnings_content, "class C {", 6);
    let fn_offset = utf16_offset_of(&assists_content, "int compute(int x, int y)", 4);
    let moved_c_uri = file_uri(&root.join("lib").join("moved_c.dart"));

    let warnings_files = vec![OpenFile {
        path: warnings_path.clone(),
        content: warnings_content.clone(),
    }];
    let both_files = vec![
        OpenFile {
            path: warnings_path.clone(),
            content: warnings_content,
        },
        OpenFile {
            path: assists_path,
            content: assists_content,
        },
    ];

    let mut out = Vec::new();

    // 1. Session without codeActionLiteralSupport: produces and executes dart.edit.codeAction.apply.
    let mut non_literal_init = dart_code_initialize_params(root);
    non_literal_init["capabilities"]["textDocument"]["codeAction"]
        .as_object_mut()
        .unwrap()
        .remove("codeActionLiteralSupport");
    let unused_range = json!({
        "start": {"line": 4, "character": 6},
        "end": {"line": 4, "character": 12},
    });
    let assist_range = json!({
        "start": {"line": 4, "character": 6},
        "end": {"line": 4, "character": 6},
    });
    let non_literal_steps = vec![
        ParityStep {
            label: "codeAction:non_literal_quickfix",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": unused_range,
                "context": {"diagnostics": [], "only": ["quickfix.remove.unusedLocalVariable"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "codeAction:non_literal_assist",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": assist_range,
                "context": {"diagnostics": [], "only": ["refactor.add.typeAnnotation"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:applyCodeAction:quickfix",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.edit.codeAction.apply",
                "arguments": [{
                    "textDocument": {"uri": warnings_uri, "version": 1},
                    "range": unused_range,
                    "kind": "quickfix.remove.unusedLocalVariable",
                    "loggedAction": "dart.fix.remove.unusedLocalVariable",
                }],
            }),
            expect_server_requests: true,
        },
        ParityStep {
            label: "executeCommand:applyCodeAction:assist",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.edit.codeAction.apply",
                "arguments": [{
                    "textDocument": {"uri": warnings_uri, "version": 1},
                    "range": assist_range,
                    "kind": "refactor.add.typeAnnotation",
                    "loggedAction": "dart.assist.add.typeAnnotation",
                }],
            }),
            expect_server_requests: true,
        },
        ParityStep {
            label: "executeCommand:applyCodeAction:not_valid",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.edit.codeAction.apply",
                "arguments": [{
                    "textDocument": {"uri": warnings_uri, "version": 1},
                    "range": unused_range,
                    "kind": "quickfix.NotValid",
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:applyCodeAction:invalid_args",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.edit.codeAction.apply",
                "arguments": [{
                    "textDocument": 123,
                    "range": 123,
                    "kind": 123,
                    "loggedAction": 123,
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:applyCodeAction:content_modified",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.edit.codeAction.apply",
                "arguments": [{
                    "textDocument": {"uri": warnings_uri, "version": 99},
                    "range": unused_range,
                    "kind": "quickfix.remove.unusedLocalVariable",
                }],
            }),
            expect_server_requests: false,
        },
    ];
    run_parity_session(
        program,
        non_literal_init,
        &warnings_files,
        &non_literal_steps,
        &mut out,
    );

    // 2. Session without workspace.applyEdit: dart.edit.codeAction.apply returns stateError (-32015).
    let mut no_apply_edit_init = dart_code_initialize_params(root);
    no_apply_edit_init["capabilities"]["workspace"]["applyEdit"] = json!(false);
    let no_apply_edit_steps = vec![ParityStep {
        label: "executeCommand:applyCodeAction:no_apply_edit",
        method: "workspace/executeCommand",
        params: json!({
            "command": "dart.edit.codeAction.apply",
            "arguments": [{
                "textDocument": {"uri": warnings_uri, "version": 1},
                "range": unused_range,
                "kind": "quickfix.remove.unusedLocalVariable",
            }],
        }),
        expect_server_requests: false,
    }];
    run_parity_session(
        program,
        no_apply_edit_init,
        &warnings_files,
        &no_apply_edit_steps,
        &mut out,
    );

    // 3. Session with interactiveResolve enabled: codeAction, command/resolve, and workspace/executeCommand.
    let interactive_init = dart_code_initialize_params(root);
    let class_range = json!({
        "start": {"line": 9, "character": 6},
        "end": {"line": 9, "character": 6},
    });
    let ctor_range = json!({
        "start": {"line": 13, "character": 2},
        "end": {"line": 13, "character": 2},
    });
    let import_range = json!({
        "start": {"line": 1, "character": 0},
        "end": {"line": 1, "character": 0},
    });
    let interactive_steps = vec![
        ParityStep {
            label: "codeAction:interactive_move_top_level",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": class_range,
                "context": {"diagnostics": [], "only": ["refactor.move"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "codeAction:interactive_add_ctor_name",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": ctor_range,
                "context": {"diagnostics": [], "only": ["refactor.add"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "codeAction:interactive_add_import_prefix",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": import_range,
                "context": {"diagnostics": [], "only": ["refactor.add"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_constructor_name:initial",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_constructor_name:invalid_value",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": ["name1"],
                }],
                "formAnswers": [{"id": "name", "value": "invalid name"}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_constructor_name:invalid_type",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": ["name1"],
                }],
                "formAnswers": [{"id": "name", "value": 123}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_constructor_name:valid",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": ["name1"],
                }],
                "formAnswers": [{"id": "name", "value": "named"}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:add_constructor_name:resolved",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "workDoneToken": "progress-ctor",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": ["named"],
                }],
            }),
            expect_server_requests: true,
        },
        ParityStep {
            label: "command/resolve:add_import_prefix:initial",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor_add.import_prefix",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": import_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_import_prefix:invalid_value",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor_add.import_prefix",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": import_offset,
                    "selectionLength": 0,
                    "arguments": ["helpers"],
                }],
                "formAnswers": [{"id": "name", "value": "invalid prefix"}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:add_import_prefix:valid",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor_add.import_prefix",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": import_offset,
                    "selectionLength": 0,
                    "arguments": ["helpers"],
                }],
                "formAnswers": [{"id": "name", "value": "hlp"}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:add_import_prefix:resolved",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.refactor_add.import_prefix",
                "workDoneToken": "progress-import",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": import_offset,
                    "selectionLength": 0,
                    "arguments": ["hlp"],
                }],
            }),
            expect_server_requests: true,
        },
        ParityStep {
            label: "command/resolve:move_top_level_to_file:initial",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.move_top_level_to_file",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": class_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:move_top_level_to_file:invalid_uri",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.move_top_level_to_file",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": class_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
                "formAnswers": [{"id": "destinationUri", "value": "https://example.com/c.dart"}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:move_top_level_to_file:valid",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.move_top_level_to_file",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": class_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
                "formAnswers": [{"id": "destinationUri", "value": moved_c_uri}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "executeCommand:move_top_level_to_file:resolved",
            method: "workspace/executeCommand",
            params: json!({
                "command": "dart.refactor.move_top_level_to_file",
                "workDoneToken": "progress-move",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": class_offset,
                    "selectionLength": 0,
                    "arguments": [moved_c_uri],
                }],
            }),
            expect_server_requests: true,
        },
        ParityStep {
            label: "command/resolve:non_parameterized_refactor",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.convert_all_formal_parameters_to_named",
                "arguments": [{
                    "filePath": assists_path_str,
                    "selectionOffset": fn_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:unknown_command",
            method: "command/resolve",
            params: json!({
                "command": "my_unknown_command",
                "arguments": [{"a": 1}],
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:invalid_location",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": 0,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
    ];
    run_parity_session(
        program,
        interactive_init,
        &both_files,
        &interactive_steps,
        &mut out,
    );

    // 4. Session without interactiveResolve: MoveTopLevelToFile includes data.parameters, and command/resolve auto-fills defaults.
    let mut non_interactive_init = dart_code_initialize_params(root);
    non_interactive_init["capabilities"]["experimental"]
        .as_object_mut()
        .unwrap()
        .remove("interactiveResolve");
    let non_interactive_steps = vec![
        ParityStep {
            label: "codeAction:non_interactive_move_top_level",
            method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": warnings_uri},
                "range": class_range,
                "context": {"diagnostics": [], "only": ["refactor.move"]},
            }),
            expect_server_requests: false,
        },
        ParityStep {
            label: "command/resolve:unsupported_input_type",
            method: "command/resolve",
            params: json!({
                "command": "dart.refactor.add_constructor_name",
                "arguments": [{
                    "filePath": warnings_path_str,
                    "selectionOffset": ctor_offset,
                    "selectionLength": 0,
                    "arguments": [],
                }],
            }),
            expect_server_requests: false,
        },
    ];
    run_parity_session(
        program,
        non_interactive_init,
        &warnings_files,
        &non_interactive_steps,
        &mut out,
    );

    out
}

#[test]
fn apply_code_action_and_interactive_resolve_match_dart_language_server() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = fixtures().join("fixes_project");
    let dart = run_apply_and_interactive_resolve("dart", &root);
    let dartr = run_apply_and_interactive_resolve(dartr_bin(), &root);

    assert!(!dart.is_empty(), "dart steps must not be empty");
    assert_eq!(dart.len(), dartr.len(), "step count mismatch");

    let mut diffs = Vec::new();
    for (d, r) in dart.iter().zip(&dartr) {
        let label = d["label"].as_str().unwrap_or("?");
        assert_eq!(
            d["label"], r["label"],
            "step label mismatch: dart={} dartr={}",
            d["label"], r["label"]
        );
        let expect_reqs = d["expect_server_requests"] == json!(true);
        let server_reqs = d["server_requests"].as_array().unwrap();
        if expect_reqs {
            assert!(
                !server_reqs.is_empty(),
                "Dart returned empty server_requests for {label}: {d}"
            );
        } else {
            let has_result =
                !d["response"]["result"].is_null() && d["response"]["result"] != json!([]);
            let has_error = !d["response"]["error"].is_null();
            assert!(
                has_result || has_error,
                "Dart returned empty answer for {label}: {d}"
            );
        }
        if d != r {
            diffs.push(label.to_string());
            eprintln!("DIFF {label}");
            if d["response"] != r["response"] {
                eprintln!(
                    "  response:\n    dart:  {}\n    dartr: {}",
                    truncate(&d["response"]),
                    truncate(&r["response"])
                );
            }
            if d["server_requests"] != r["server_requests"] {
                eprintln!(
                    "  server_requests:\n    dart:  {}\n    dartr: {}",
                    truncate(&d["server_requests"]),
                    truncate(&r["server_requests"])
                );
            }
        }
    }
    eprintln!(
        "apply_code_action + interactive_resolve parity: {}/{} steps identical",
        dart.len() - diffs.len(),
        dart.len()
    );
    assert!(
        diffs.is_empty(),
        "parity test had {} differing steps: {diffs:?}",
        diffs.len()
    );
}
