//! Differential test of unit C3 (method and function expression
//! invocations): `difftest resolved` on the code snippets of the analyzer's
//! `method_invocation_test.dart` and `function_expression_invocation_test.dart`
//! (`tests/fixtures/resolved_c3`, one library per test) must give the same
//! static type, invoke type and type arguments as the analyzer for every
//! `MethodInvocation` and `FunctionExpressionInvocation`, and the same
//! static types for the `SimpleIdentifier`s (the invoked method names).
//!
//! The snippets whose results depend on unported parts (extension member
//! lookup, top-level inference, instance creation, ...) are not in the
//! fixtures. Needs `dart` on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, run};

#[test]
fn invocations_match_the_analyzer_on_ported_resolution_tests() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resolved_c3");
    let kinds = [
        "MethodInvocation",
        "FunctionExpressionInvocation",
        "SimpleIdentifier",
    ];
    let options = Options {
        mode: "resolved".to_string(),
        inputs: vec![fixtures],
        jobs: 2,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        no_diagnostics: true,
        kinds: kinds.iter().map(|k| k.to_string()).collect(),
        ..Default::default()
    };
    let report = run(&options).unwrap();
    println!("{}{}", report.kind_report(), report.summary());
    for kind in kinds {
        let k = report.kinds.get(kind).copied().unwrap_or_default();
        assert!(k.oracle > 0, "no oracle entries of kind {kind}");
        assert_eq!(k.matched, k.oracle, "{kind}:\n{}", report.kind_report());
    }
    assert!(report.differences.is_empty(), "{:?}", report.differences);
}
