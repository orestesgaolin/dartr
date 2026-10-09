//! Runs the differential test of the linker (`difftest elements
//! --mask-inferred`: the element model of each library, without the types
//! that top-level inference computes) against the Dart oracle. Needs `dart`
//! on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, collect_dart_files, run};

fn options(inputs: Vec<PathBuf>) -> Options {
    Options {
        mode: "elements".to_string(),
        inputs,
        jobs: 4,
        batch_size: 0,
        write_failures: None,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        oracle: Vec::new(),
        mask_inferred: true,
        ..Default::default()
    }
}

fn assert_parity(inputs: Vec<PathBuf>, expected_files: usize) {
    let report = run(&options(inputs)).unwrap();
    assert_eq!(report.files, expected_files);
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// Checked-in fixtures: every declaration kind, parts (also nested),
/// exports with combinators, import prefixes, a cycle between libraries,
/// bounds and defaults, mixin inference (`fixtures/linking`), and the
/// fixtures of the `elements` dump (`fixtures/elements`).
#[test]
fn elements_parity_on_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dirs = vec![fixtures.join("linking"), fixtures.join("elements")];
    let files = collect_dart_files(&dirs).unwrap();
    assert!(
        files.len() >= 12,
        "fixtures not found in {}",
        fixtures.display()
    );
    assert_parity(dirs, files.len());
}

/// Override inference (unit B5), mixin inference and default types
/// (`fixtures/override_inference`), without masking the inferred types:
/// fields, getters, setters, methods and parameters with types from the
/// overridden members, combined signatures and conflicts
/// (`typeInferenceError`), inherited covariance, generic methods, mixins,
/// initializing formal parameters, private names across libraries.
#[test]
fn elements_parity_on_override_inference_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/override_inference");
    let files = collect_dart_files(std::slice::from_ref(&dir)).unwrap();
    assert!(files.len() >= 6, "fixtures not found in {}", dir.display());
    let report = run(&Options {
        mask_inferred: false,
        ..options(vec![dir])
    })
    .unwrap();
    assert_eq!(report.files, files.len());
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// Top-level inference (unit C10, `fixtures/top_level_inference`), without
/// masking the inferred types: initializers of top-level variables and
/// fields inferred on demand (also from override inference and field
/// formal parameters), dependency cycles (`typeInferenceError`), function
/// expressions with local parameters and type parameters.
#[test]
fn elements_parity_on_top_level_inference_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/top_level_inference");
    let report = run(&Options {
        mask_inferred: false,
        ..options(vec![dir.join("variables.dart")])
    })
    .unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// Enum constants: Dart infers their types from the synthetic instance
/// creation of each constant.
#[test]
fn elements_parity_on_top_level_inference_enum_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/top_level_inference");
    let report = run(&Options {
        mask_inferred: false,
        ..options(vec![dir.join("enums.dart")])
    })
    .unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

fn assert_unmasked_parity(dir: PathBuf, min_files: usize) {
    let files = collect_dart_files(std::slice::from_ref(&dir)).unwrap();
    assert!(
        files.len() >= min_files,
        "fixtures not found in {}",
        dir.display()
    );
    let report = run(&Options {
        mask_inferred: false,
        ..options(vec![dir])
    })
    .unwrap();
    assert_eq!(report.files, files.len());
    assert_eq!(
        report.different(),
        0,
        "{}{}",
        report.difference_report(20),
        report.summary()
    );
}

/// The cases of the analyzer's `top_level_inference_test.dart` (unit C10,
/// one file per test, `fixtures/top_level_inference/analyzer`) whose
/// initializers the resolver of this unit resolves, without masking.
#[test]
fn elements_parity_on_analyzer_top_level_inference_tests() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/top_level_inference/analyzer");
    assert_unmasked_parity(dir, 94);
}

/// The other cases of `top_level_inference_test.dart`: their initializers
/// are invocations, property accesses, operators and instance creations.
/// They are identical with the resolvers of units C3–C8 merged (checked on
/// a merge of the wave C branches); then move them to
/// `fixtures/top_level_inference/analyzer`.
#[test]
#[ignore = "needs the property and operator resolvers (units C4-C6)"]
fn elements_parity_on_analyzer_top_level_inference_tests_wave_c() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/top_level_inference/analyzer_wave_c");
    assert_unmasked_parity(dir, 50);
}

/// The `dart:` libraries of the SDK of the `dart` on PATH, linked from
/// source (the `dart:core` cycle and every other SDK cycle).
#[test]
fn elements_parity_on_dart_core_and_async() {
    let inputs = vec![PathBuf::from("dart:core"), PathBuf::from("dart:async")];
    assert_parity(inputs, 2);
}

/// The corpora: every `dart:` library, `pkg/analyzer/lib`,
/// `tests/language`, Flutter and the benchmark app. Corpora that are not
/// on this machine are skipped. Slow; run with
/// `cargo test --release -p dartr --test difftest_elements -- --ignored`.
#[test]
#[ignore]
fn elements_parity_on_corpora() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let uris: Vec<PathBuf> = std::fs::read_to_string(fixtures.join("elements/dart_libraries.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    let count = uris.len();
    assert_parity(uris, count);

    let sdk = dartr_difftest::repo_root().join("third_party/dart-sdk");
    let home = std::env::var("HOME").unwrap_or_default();
    let corpus = std::env::var_os("DARTR_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dartr_difftest::repo_root().join("bench/corpus"));
    let candidates = vec![
        sdk.join("pkg/analyzer/lib"),
        PathBuf::from(format!("{home}/fvm/versions/stable/packages/flutter/lib")),
        corpus.join("visible-app/lib"),
    ];
    for dir in candidates.into_iter().filter(|d| d.is_dir()) {
        let files = collect_dart_files(std::slice::from_ref(&dir)).unwrap();
        assert_parity(vec![dir], files.len());
    }
}
