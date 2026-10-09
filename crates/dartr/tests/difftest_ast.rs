//! Runs the differential test of the AST builder (`difftest ast`: the AST
//! as child entities and the parse diagnostics of `parseString`) against the
//! Dart oracle. Needs `dart` on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, collect_dart_files, run};

fn options(inputs: Vec<PathBuf>) -> Options {
    Options {
        mode: "ast".to_string(),
        inputs,
        jobs: 4,
        batch_size: 0,
        write_failures: None,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        oracle: Vec::new(),
        mask_inferred: false,
        ..Default::default()
    }
}

/// Checked-in fixtures: every declaration kind, statements, expressions,
/// strings, doc comments (references, code blocks, doc directives,
/// `@docImport`), `// @dart = x.y` versions (feature errors of the AST
/// builder), heavy recovery; and the parser event fixtures.
#[test]
fn ast_parity_on_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dirs = vec![fixtures.join("ast"), fixtures.join("events")];
    let files = collect_dart_files(&dirs).unwrap();
    assert!(files.len() >= 40, "fixtures not found in {}", fixtures.display());

    let report = run(&options(dirs)).unwrap();
    assert_eq!(report.files, files.len());
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// The corpora: the pinned SDK (language tests, front end parser test
/// cases, `sdk/lib`, `pkg/analyzer/lib`), Flutter and the benchmark app.
/// Corpora that are not on this machine are skipped. Slow; run with
/// `cargo test --release -p dartr --test difftest_ast -- --ignored`.
#[test]
#[ignore]
fn ast_parity_on_corpora() {
    let sdk = dartr_difftest::repo_root().join("third_party/dart-sdk");
    let home = std::env::var("HOME").unwrap_or_default();
    // `bench/corpus` (gitignored, see `bench/`), or `DARTR_CORPUS_DIR`.
    let corpus = std::env::var_os("DARTR_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dartr_difftest::repo_root().join("bench/corpus"));
    let candidates = vec![
        sdk.join("tests/language"),
        sdk.join("pkg/front_end/parser_testcases"),
        sdk.join("sdk/lib"),
        sdk.join("pkg/analyzer/lib"),
        PathBuf::from(&home).join("fvm/default/packages/flutter/lib"),
        corpus.join("visible-app/lib"),
        corpus.join("visible-app/packages"),
    ];
    let inputs: Vec<PathBuf> = candidates.into_iter().filter(|p| p.exists()).collect();
    let report = run(&options(inputs)).unwrap();
    assert!(report.files > 5000);
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}
