//! Round trip of the AST model against the Dart analyzer, without a parser:
//! the oracle (`tools/oracle`, mode `ast`) parses each file, the loader
//! (`dartr_ast::testing`) builds the Rust AST from the dump, and the Rust
//! `ast` dump and `to_source` must be the same as the oracle output
//! (`ast`, and `toSource()` from `tools/oracle/bin/tosource.dart`).
//!
//! Needs `dart` on PATH (the oracles are compiled on first use). Run with
//! `cargo test -p dartr_ast --features testing`.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_ast::testing;
use dartr_difftest::{collect_dart_files, ensure_oracle, repo_root};

/// Compiles `tools/oracle/bin/tosource.dart` to `target/oracle/tosource`
/// when it is missing or older than its source.
fn ensure_tosource_oracle() -> PathBuf {
    ensure_oracle().expect("oracle"); // runs `dart pub get` when needed
    let root = repo_root();
    let source = root.join("tools/oracle/bin/tosource.dart");
    let exe = root.join("target/oracle/tosource");
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if modified(&exe).is_none_or(|t| modified(&source).is_some_and(|s| s > t)) {
        let tmp = exe.with_extension(format!("tmp{}", std::process::id()));
        let status = Command::new("dart")
            .args(["compile", "exe"])
            .arg(&source)
            .arg("-o")
            .arg(&tmp)
            .stdout(Stdio::null())
            .status()
            .expect("dart compile exe");
        assert!(status.success(), "compiling tosource.dart failed");
        std::fs::rename(&tmp, &exe).unwrap();
    }
    exe
}

/// Runs an oracle program on [files] (paths on stdin), returns path -> line.
fn run_oracle(program: &[String], files: &[String]) -> HashMap<String, String> {
    let mut child = Command::new(&program[0])
        .args(&program[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("oracle");
    let input: String = files.iter().map(|f| format!("{f}\n")).collect();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap().unwrap();
    let mut lines = HashMap::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let mut de = serde_json::Deserializer::from_str(line);
        de.disable_recursion_limit();
        let v: serde_json::Value = serde::Deserialize::deserialize(&mut de).unwrap();
        let path = v["path"].as_str().unwrap().to_string();
        lines.insert(path, line.to_string());
    }
    lines
}

/// The raw JSON text of the `"ast"` value of an oracle line.
fn raw_ast(line: &str) -> &str {
    let start = line.find(",\"ast\":").expect("ast") + ",\"ast\":".len();
    let end = line.rfind(",\"diagnostics\":").expect("diagnostics");
    &line[start..end]
}

struct Summary {
    files: usize,
    failures: Vec<String>,
}

fn round_trip(inputs: &[PathBuf]) -> Summary {
    let files = collect_dart_files(inputs).unwrap();
    let mut oracle = ensure_oracle().unwrap();
    oracle.push("ast".to_string());
    let asts = run_oracle(&oracle, &files);
    let tosource = vec![ensure_tosource_oracle().to_string_lossy().into_owned()];
    let sources = run_oracle(&tosource, &files);
    let mut failures = Vec::new();
    for f in &files {
        let Some(line) = asts.get(f) else { continue };
        if !line.contains(",\"ast\":") {
            continue; // the file could not be read
        }
        let source = testing::read_source(f).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&sources[f]).unwrap();
        let expected = expected["source"].as_str().unwrap();
        match testing::check(raw_ast(line), &source, Some(expected)) {
            Ok(c) if c.is_ok() => {}
            Ok(c) => failures.push(format!(
                "{f}\n  dump: {}\n  to_source: {}",
                c.dump_diff.unwrap_or_default(),
                c.source_diff.unwrap_or_default()
            )),
            Err(e) => failures.push(format!("{f}\n  load: {e}")),
        }
    }
    Summary {
        files: files.len(),
        failures,
    }
}

/// Fixtures with most node kinds: declarations of all kinds, expressions,
/// statements, patterns, and recovery (a comment after the annotations,
/// modifiers out of order, missing tokens, a left delimiter without
/// parameters).
#[test]
fn round_trip_on_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let summary = round_trip(&[dir]);
    assert_eq!(summary.files, 3);
    assert!(
        summary.failures.is_empty(),
        "{}",
        summary.failures.join("\n")
    );
}

/// The corpora of the pinned SDK and of Flutter. Slow; run with
/// `cargo test --release -p dartr_ast --features testing -- --ignored`.
/// Known differences (limits of the loader, not of the model): see
/// `dartr_ast::testing`.
#[test]
#[ignore]
fn round_trip_on_corpora() {
    let sdk = repo_root().join("third_party/dart-sdk");
    let mut inputs = vec![sdk.join("tests/language"), sdk.join("sdk/lib")];
    if let Some(home) = std::env::var_os("HOME") {
        let flutter = PathBuf::from(home).join("fvm/default/packages/flutter/lib");
        if flutter.exists() {
            inputs.push(flutter);
        }
    }
    let summary = round_trip(&inputs);
    let ok = summary.files - summary.failures.len();
    println!("{ok}/{} files identical", summary.files);
    // Three files of tests/language are ambiguous for the loader.
    assert!(
        summary.failures.len() <= 3,
        "{}",
        summary.failures.join("\n")
    );
}
