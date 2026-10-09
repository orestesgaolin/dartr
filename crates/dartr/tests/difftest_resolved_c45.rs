//! Differential tests of units C4 and C5 (property accesses, prefixed
//! identifiers, index expressions, assignments, binary, prefix and postfix
//! expressions) on `tests/fixtures/resolved_c45`: the code snippets of the
//! analyzer's own tests (`pkg/analyzer/test/src/dart/resolution/`
//! `assignment_test.dart`, `binary_expression_test.dart`,
//! `index_expression_test.dart`, `postfix_expression_test.dart`,
//! `prefix_expression_test.dart`, `prefixed_identifier_test.dart`,
//! `property_access_test.dart`; one file per test method, without the
//! tests that need other files or experiments).
//!
//! Needs `dart` on PATH (the oracle is compiled on first use).
//!
//! The remaining differences come from node kinds of other units (method
//! invocations, instance creations, extension members, top-level
//! inference), so the tests check a parity floor per node kind; raise it
//! when those units land.

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, run};

fn options(mode: &str, kinds: &[&str]) -> Options {
    Options {
        mode: mode.to_string(),
        inputs: vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resolved_c45")],
        jobs: 4,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        no_diagnostics: true,
        kinds: kinds.iter().map(|k| k.to_string()).collect(),
        ..Default::default()
    }
}

/// Runs [mode] on the fixtures and checks that each kind of [floors] has
/// oracle entries and at least the given parity (percent).
fn assert_parity(mode: &str, floors: &[(&str, f64)]) {
    let kinds: Vec<&str> = floors.iter().map(|(k, _)| *k).collect();
    let report = run(&options(mode, &kinds)).unwrap();
    println!("{}{}", report.kind_report(), report.summary());
    for (kind, floor) in floors {
        let k = report.kinds.get(*kind).copied().unwrap_or_default();
        assert!(k.oracle > 0, "no oracle entries of {kind}");
        let parity = 100.0 * k.matched as f64 / k.oracle as f64;
        assert!(
            parity >= *floor,
            "{kind}: parity {parity:.2}% < {floor}% ({} of {})\n{}",
            k.matched,
            k.oracle,
            report.kind_report()
        );
    }
}

/// One test for both modes: the oracle is compiled on first use, and two
/// tests that start at the same time would both compile it.
#[test]
fn c45_kinds_match_the_analyzer_on_its_own_test_code() {
    assert_parity(
        "resolved",
        &[
            ("AssignmentExpression", 95.0),
            ("BinaryExpression", 90.0),
            ("IndexExpression", 80.0),
            ("PostfixExpression", 90.0),
            ("PrefixExpression", 90.0),
            ("PrefixedIdentifier", 100.0),
            ("PropertyAccess", 85.0),
        ],
    );
    assert_parity("resolved-el", &[("SimpleIdentifier", 96.0)]);
}
