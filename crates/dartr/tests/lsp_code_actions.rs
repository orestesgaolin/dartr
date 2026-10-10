//! Differential tests of `textDocument/codeAction` and
//! `workspace/executeCommand`: the same requests against
//! `dart language-server` and `dartr language-server`.
//!
//! - Quick fixes: one request per diagnostic that `dart` publishes (range
//!   of the diagnostic, `only: ["quickfix"]`), on the fixture project
//!   `lsp_fixtures/fixes_project` and (with `DARTR_LSP_FIX_CORPUS=<folder>
//!   [:<max files>]`) on a sample of real files, opened with their first
//!   `package:` import removed (so that names are unresolved).
//! - Source actions: `only: ["source"]`, then the `Organize Imports` and
//!   `Sort Members` commands; the `workspace/applyEdit` of each command is
//!   compared.
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
    source_actions: Vec<Value>,
    commands: Vec<(Value, Option<Value>)>,
}

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

fn run(c: &mut LspClient, requests: &[FixRequest], files: &[OpenFile]) -> Results {
    let mut results = Results::default();
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
        for command in ["dart.edit.organizeImports", "dart.edit.sortMembers"] {
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
    results
}

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
    dart: &Results,
    dartr: &Results,
) -> (BTreeMap<String, Tally>, Vec<String>) {
    let mut tally: BTreeMap<String, Tally> = BTreeMap::new();
    let mut problems = Vec::new();
    for (i, r) in requests.iter().enumerate() {
        let d = dart.fixes[i]["result"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let x = dartr.fixes[i]["result"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for a in &d {
            let title = a["title"].as_str().unwrap_or("");
            let t = tally.entry(action_id(a)).or_default();
            match x.iter().find(|b| b["title"].as_str() == Some(title)) {
                None => t.missing += 1,
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
        let name = if i % 2 == 0 {
            "organizeImports"
        } else {
            "sortMembers"
        };
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
    eprintln!("== {label}: {} diagnostics", requests.len());
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
    let dart_results = run(&mut dart, &requests, files);
    let _ = dart.shutdown_and_exit();
    let mut dartr = start(dartr_bin(), root, files);
    let dartr_results = run(&mut dartr, &requests, files);
    let _ = dartr.shutdown_and_exit();
    compare(label, &requests, &dart_results, &dartr_results)
}

/// The fix kinds whose actions must be identical on the fixture project.
const IDENTICAL: &[&str] = &[
    "dart.fix.ignore.analysis",
    "dart.fix.ignore.file",
    "dart.fix.ignore.line",
];

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
    ] {
        let path = root.join("lib").join(name);
        let content = std::fs::read_to_string(&path).unwrap();
        files.push(OpenFile { path, content });
    }
    let (tally, problems) = run_both("fixes_project", &root, &files);
    let mut failures = Vec::new();
    for id in IDENTICAL {
        if let Some(t) = tally.get(*id) {
            if t.differing + t.missing + t.extra > 0 {
                failures.push(format!(
                    "{id}: {} differing, {} missing, {} extra",
                    t.differing, t.missing, t.extra
                ));
            }
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
