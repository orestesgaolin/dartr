//! Runs the differential test (`difftest tokens`) against the Dart analyzer
//! oracle. Needs `dart` on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, collect_dart_files, run};

fn options(inputs: Vec<PathBuf>) -> Options {
    Options {
        mode: "tokens".to_string(),
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

/// Small fixtures for the tricky parts of the scanner: string interpolation,
/// raw strings, unterminated strings and comments, `>>`/`>>>` in generics,
/// numbers with separators, unicode, `@dart=` comments, script tags, BOM,
/// bracket and curly bracket recovery, CR/CRLF, control characters.
#[test]
fn tokens_parity_on_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tokens");
    let files = collect_dart_files(std::slice::from_ref(&dir)).unwrap();
    assert!(files.len() >= 40, "fixtures not found in {}", dir.display());

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
fn tokens_parity_on_sdk_corpora() {
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
