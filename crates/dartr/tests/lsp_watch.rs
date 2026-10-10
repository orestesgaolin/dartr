//! Differential test of the file watcher of the language server: files
//! change on disk outside the editor (a new Dart file that an import
//! needs, a fix in a Dart file, a removed Dart file, a change of an open
//! file on disk, a new `analysis_options.yaml` with a lint, a change of
//! `pubspec.yaml`), and the published diagnostics after each change are
//! compared with `dart language-server` (all codes, last list per file).

mod lsp_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn write_project(name: &str) -> PathBuf {
    // A new folder for each run: FSEvents replays the recent events of a
    // removed folder with the same path, and the watcher of Dart stops
    // when it sees its root removed.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}_{stamp}"));
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    write(
        &root,
        "pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.9.0\n",
    );
    write(
        &root,
        "lib/a.dart",
        "import 'b.dart';\n\nvoid main() {\n  var x = B();\n  print(x);\n}\n",
    );
    write(&root, "lib/c.dart", "int f() => 'not an int';\n");
    write(&root, "lib/open.dart", "int g() => 1;\n");
    // Let FSEvents deliver the creation before the server starts watching.
    std::thread::sleep(Duration::from_secs(2));
    root
}

/// The diagnostics of the files of [root] (relative path → list), without
/// empty lists.
fn snapshot(c: &LspClient, root: &Path) -> Value {
    let prefix = format!("{}/", file_uri(root));
    let mut out = BTreeMap::new();
    for (uri, list) in &c.state.diagnostics {
        if let Some(rel) = uri.strip_prefix(&prefix)
            && !list.is_empty()
        {
            out.insert(rel.to_string(), Value::Array(list.clone()));
        }
    }
    json!(out)
}

/// Waits for the analysis after a change on disk.
fn wait(c: &mut LspClient) {
    c.settle_with_grace(Duration::from_secs(8));
    // Separate the changes for FSEvents (it coalesces close events).
    std::thread::sleep(Duration::from_secs(2));
    c.settle(false);
}

fn run(program: &str, name: &str) -> Vec<(String, Value)> {
    let root = write_project(name);
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    let init = c.request("initialize", dart_code_initialize_params(&root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    let mut t: Vec<(String, Value)> = Vec::new();
    t.push(("initial".into(), snapshot(&c, &root)));

    let open = root.join("lib/open.dart");
    c.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": file_uri(&open), "languageId": "dart", "version": 1, "text": "int g() => 1;\n"}}),
    );
    c.settle(false);

    write(&root, "lib/b.dart", "class B {}\n");
    wait(&mut c);
    t.push(("add b.dart".into(), snapshot(&c, &root)));

    write(&root, "lib/c.dart", "int f() => 1;\nString h() => 2;\n");
    wait(&mut c);
    t.push(("modify c.dart".into(), snapshot(&c, &root)));

    std::fs::remove_file(root.join("lib/c.dart")).unwrap();
    wait(&mut c);
    t.push(("remove c.dart".into(), snapshot(&c, &root)));

    // The overlay of an open file wins over the file on disk.
    write(&root, "lib/open.dart", "int g() => 'x';\n");
    wait(&mut c);
    t.push(("modify open file on disk".into(), snapshot(&c, &root)));

    write(&root, "lib/b.dart", "class C {}\n");
    wait(&mut c);
    t.push(("modify b.dart".into(), snapshot(&c, &root)));

    write(
        &root,
        "analysis_options.yaml",
        "linter:\n  rules:\n    - prefer_final_locals\n",
    );
    wait(&mut c);
    t.push(("add analysis_options.yaml".into(), snapshot(&c, &root)));

    write(
        &root,
        "pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.9.0\nunknown_key: 1\n",
    );
    wait(&mut c);
    t.push(("modify pubspec.yaml".into(), snapshot(&c, &root)));

    std::fs::remove_file(root.join("lib/b.dart")).unwrap();
    wait(&mut c);
    t.push(("remove b.dart".into(), snapshot(&c, &root)));

    let _ = c.shutdown_and_exit();
    let _ = std::fs::remove_dir_all(&root);
    t
}

#[test]
fn lsp_watch_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let dart = run("dart", "lsp_watch_dart");
    let dartr = run(dartr_bin(), "lsp_watch_dartr");
    let mut identical = 0;
    for ((step, d), (_, r)) in dart.iter().zip(&dartr) {
        // The roots differ (two projects): compare the relative values.
        let ok = d == r;
        identical += ok as usize;
        eprintln!("  {step}: {}", if ok { "identical" } else { "DIFFERENT" });
        if !ok {
            eprintln!("    dart:  {d}\n    dartr: {r}");
        }
    }
    eprintln!("file watching: {identical}/{} steps identical", dart.len());
    // The steps whose result in Dart does not depend on the timing of its
    // watcher: Dart publishes stale diagnostics for a removed file (the
    // content when the server started) and processes some later events
    // only at the next change, so those steps are reported, not asserted.
    for step in [
        "initial",
        "add b.dart",
        "modify c.dart",
        "modify open file on disk",
    ] {
        let d = &dart.iter().find(|(s, _)| s == step).unwrap().1;
        let r = &dartr.iter().find(|(s, _)| s == step).unwrap().1;
        assert_eq!(d, r, "step {step}");
    }
}
