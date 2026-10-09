//! Differential test of units D1–D2 (constant evaluation): `difftest
//! elements --with-const` on the code snippets of the analyzer's
//! `test/src/dart/constant/evaluation_test.dart` (`ConstantVisitorTest`,
//! `InstanceCreationEvaluatorTest`; one library per test in
//! `tests/fixtures/constants`) must give the same element dump, including
//! the `"const"` value (`DartObjectImpl.toString()`) of every const
//! variable, field and enum constant, as the analyzer.
//!
//! The snippets that need unported resolver units are in
//! `tests/fixtures/constants_pending` (extension members, unit C6; the
//! context type of enum constant arguments with explicit type arguments,
//! `v1<double>.named(10)`, unit C8); their
//! test is ignored. The snippets that use `newFile` or declared variables
//! are not ported. Needs `dart` on PATH (the oracle is compiled on first
//! use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, run};

fn check(dir: &str) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let options = Options {
        mode: "elements".to_string(),
        inputs: vec![fixtures],
        jobs: 4,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        extra_args: vec!["--with-const".to_string()],
        ..Default::default()
    };
    let report = run(&options).unwrap();
    println!("{}", report.summary());
    assert!(report.files > 0, "no fixtures in {dir}");
    assert!(
        report.differences.is_empty(),
        "{}",
        report.difference_report(20)
    );
}

#[test]
fn constant_values_match_the_analyzer_on_ported_evaluation_tests() {
    check("constants");
}

#[test]
#[ignore = "needs unit C6 (extension members) and a C8 fix (enum constant argument context)"]
fn constant_values_match_the_analyzer_pending() {
    check("constants_pending");
}
