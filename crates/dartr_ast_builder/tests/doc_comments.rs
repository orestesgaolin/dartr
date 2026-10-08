//! Differential test of the `DocCommentBuilder` against the oracle `ast`
//! dump: every `Comment` node of the oracle AST is rebuilt from the same
//! comment token with `DocCommentBuilder` and compared (child entities:
//! references and tokens), and the `doc_directive_*` diagnostics are
//! compared. Needs `dart` on PATH (the oracle is compiled on first use).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_ast::Ast;
use dartr_ast_builder::LibraryLanguageVersion;
use dartr_ast_builder::doc_comment_builder::DocCommentBuilder;
use dartr_ast_builder::error_converter::FastaErrorReporter;
use dartr_parser::ExperimentalFeatures;
use dartr_syntax::{TokenId, scan_for_analyzer, severity_lower_name, strip_bom};
use serde_json::Value;

/// The oracle `ast` output for [files], in order.
fn oracle(files: &[String]) -> Vec<Value> {
    let mut cmd = dartr_difftest::ensure_oracle().expect("oracle");
    let program = cmd.remove(0);
    let mut child = Command::new(program)
        .args(cmd)
        .arg("ast")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run oracle");
    let mut stdin = child.stdin.take().unwrap();
    let input = files.join("\n");
    std::thread::spawn(move || {
        stdin.write_all(input.as_bytes()).unwrap();
    });
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let mut de = serde_json::Deserializer::from_str(l);
            de.disable_recursion_limit();
            serde::Deserialize::deserialize(&mut de).unwrap()
        })
        .collect()
}

fn collect_comments<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
    if node.get("t").and_then(|t| t.as_str()) == Some("Comment") {
        out.push(node);
    }
    if let Some(children) = node.get("c").and_then(|c| c.as_array()) {
        for c in children {
            if c.get("t").is_some() {
                collect_comments(c, out);
            }
        }
    }
}

#[derive(Default, Debug)]
struct Stats {
    files: usize,
    comments: usize,
    identical: usize,
    diagnostic_files_different: usize,
    failures: Vec<String>,
}

fn diagnostic_key(d: &Value) -> String {
    format!(
        "{} {} {} {} {}",
        d["code"].as_str().unwrap(),
        d["severity"].as_str().unwrap(),
        d["o"],
        d["l"],
        d["msg"].as_str().unwrap()
    )
}

fn check_file(path: &str, oracle: &Value, stats: &mut Stats, all_diagnostics: bool) {
    let Some(ast_json) = oracle.get("ast") else {
        return;
    };
    stats.files += 1;
    let bytes = std::fs::read(path).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let source = strip_bom(&text);
    let scan = scan_for_analyzer(source);
    let mut ast = Ast::new(scan.scan.tokens.clone());

    // Comment tokens by offset.
    let mut comment_at = std::collections::HashMap::new();
    for id in ast.tokens.iter_from(scan.first) {
        for c in ast.tokens.comments(id) {
            comment_at.entry(ast.tokens.offset(c)).or_insert(c);
        }
    }

    let mut oracle_comments = Vec::new();
    collect_comments(ast_json, &mut oracle_comments);
    let mut reporter = FastaErrorReporter::new();
    let version = LibraryLanguageVersion {
        package: (3, 13),
        override_: scan.override_version.map(|(a, b)| (a as u32, b as u32)),
    };
    let features = dartr_parser::analyzer::features_for_file(scan.override_version);
    let mut seen = std::collections::HashSet::new();
    for expected in oracle_comments {
        let offset = expected["o"].as_u64().unwrap() as u32;
        let start: TokenId = *comment_at.get(&offset).expect("comment token");
        stats.comments += 1;
        let first = seen.insert(offset);
        let mut scratch = FastaErrorReporter::new();
        let reporter = if first { &mut reporter } else { &mut scratch };
        let comment = DocCommentBuilder::new(
            &mut ast,
            reporter,
            "file:///test.dart",
            features,
            version,
            start,
        )
        .build();
        let actual: Value =
            serde_json::from_str(&dartr_ast::dump::node_json(&ast, comment)).unwrap();
        if &actual == expected {
            stats.identical += 1;
        } else if stats.failures.len() < 20 {
            stats.failures.push(format!(
                "{path} @{offset}\n  oracle: {expected}\n  dartr:  {actual}"
            ));
        }
    }
    let _ = ExperimentalFeatures::latest();

    // `doc_directive_*` diagnostics.
    let mut expected: Vec<String> = oracle["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| all_diagnostics || d["code"].as_str().unwrap().starts_with("doc_directive"))
        .map(diagnostic_key)
        .collect();
    let mut actual: Vec<String> = reporter
        .diagnostic_reporter
        .diagnostics
        .iter()
        .filter(|d| all_diagnostics || d.code.lower_case_name().starts_with("doc_directive"))
        .map(|d| {
            diagnostic_key(&serde_json::json!({
                "code": d.code.lower_case_name(),
                "severity": severity_lower_name(d.severity),
                "o": d.offset,
                "l": d.length,
                "msg": d.message,
            }))
        })
        .collect();
    expected.sort();
    actual.sort();
    if expected != actual {
        stats.diagnostic_files_different += 1;
        if stats.failures.len() < 20 {
            stats.failures.push(format!(
                "{path} diagnostics\n  oracle: {expected:?}\n  dartr:  {actual:?}"
            ));
        }
    }
}

fn run(files: Vec<String>) -> Stats {
    run_with(files, false)
}

fn run_with(files: Vec<String>, all_diagnostics: bool) -> Stats {
    let mut stats = Stats::default();
    for chunk in files.chunks(200) {
        let outputs = oracle(chunk);
        assert_eq!(outputs.len(), chunk.len());
        for (path, out) in chunk.iter().zip(&outputs) {
            check_file(path, out, &mut stats, all_diagnostics);
        }
    }
    stats
}

fn fixtures() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/doc_comments");
    dartr_difftest::collect_dart_files(&[dir]).unwrap()
}

fn assert_parity(stats: &Stats) {
    assert!(
        stats.failures.is_empty() && stats.identical == stats.comments,
        "{} of {} comments identical, {} files with different diagnostics\n{}",
        stats.identical,
        stats.comments,
        stats.diagnostic_files_different,
        stats.failures.join("\n")
    );
}

#[test]
fn doc_comments_match_oracle_on_fixtures() {
    // The doc import fixture needs the full builder for the nested parse;
    // its comment nodes are checked here, its diagnostics in the
    // `doc_imports` test.
    let files = fixtures();
    assert!(files.len() >= 4);
    let stats = run(files);
    eprintln!("{stats:?}");
    assert!(stats.comments >= 25);
    assert_parity(&stats);
}

/// The diagnostics of `@docImport`s (parse errors of the nested unit)
/// need the full AST builder for the nested parse.
#[test]
#[ignore]
fn doc_imports_match_oracle() {
    let files: Vec<String> = fixtures()
        .into_iter()
        .filter(|f| f.ends_with("doc_imports.dart"))
        .collect();
    let stats = run_with(files, true);
    assert_parity(&stats);
    assert_eq!(stats.diagnostic_files_different, 0);
}

/// All doc comments of the SDK libraries and Flutter. Slow; run with
/// `cargo test --release -p dartr_ast_builder --test doc_comments -- --ignored`.
#[test]
#[ignore]
fn doc_comments_match_oracle_on_corpora() {
    let sdk = dartr_difftest::repo_root().join("third_party/dart-sdk/sdk/lib");
    let mut inputs: Vec<PathBuf> = vec![sdk];
    if let Ok(home) = std::env::var("HOME") {
        inputs.push(PathBuf::from(home).join("fvm/default/packages/flutter/lib"));
    }
    let inputs: Vec<PathBuf> = inputs.into_iter().filter(|p| p.exists()).collect();
    let files = dartr_difftest::collect_dart_files(&inputs).unwrap();
    // Deeply nested files need a large stack (JSON and AST recursion).
    let stats = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || run(files))
        .unwrap()
        .join()
        .unwrap();
    eprintln!(
        "files {} comments {} identical {} diagnostic files different {}",
        stats.files, stats.comments, stats.identical, stats.diagnostic_files_different
    );
    assert_parity(&stats);
}
