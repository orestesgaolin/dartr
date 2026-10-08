//! Runs the differential test of the parser events (`difftest events`)
//! against the Dart oracle (`tools/oracle/bin/events.dart`). Needs `dart`
//! on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, collect_dart_files, run};

fn options(inputs: Vec<PathBuf>) -> Options {
    Options {
        mode: "events".to_string(),
        inputs,
        jobs: 4,
        batch_size: 0,
        write_failures: None,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        oracle: Vec::new(),
        mask_inferred: false,
    }
}

/// Small fixtures for parser recovery: out-of-order import and class header
/// clauses, missing semicolons and brackets, `>>` in type arguments,
/// conditional expressions vs. nullable types, records and patterns,
/// language version comments (feature sets), primary constructors, enums,
/// class member recovery, top level recovery, strings, statements,
/// expressions, deep nesting, `await`/`yield` as identifiers, metadata,
/// extension types, function types, `native`, script tags, `part of`,
/// scanner error tokens, constructors, switch recovery, operators.
#[test]
fn events_parity_on_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/events");
    let files = collect_dart_files(std::slice::from_ref(&dir)).unwrap();
    assert!(files.len() >= 30, "fixtures not found in {}", dir.display());

    let report = run(&options(vec![dir])).unwrap();
    assert_eq!(report.files, files.len());
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// The full corpora of the pinned SDK. Slow; run with
/// `cargo test --release -p dartr -- --ignored`.
#[test]
#[ignore]
fn events_parity_on_sdk_corpora() {
    let sdk = dartr_difftest::repo_root().join("third_party/dart-sdk");
    let inputs = vec![
        sdk.join("tests/language"),
        sdk.join("sdk/lib"),
        sdk.join("pkg/analyzer/lib"),
    ];
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
