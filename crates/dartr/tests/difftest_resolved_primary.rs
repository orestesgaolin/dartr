//! Differential tests of primary constructors (bodies, initializer lists,
//! declaring parameters) on `tests/fixtures/resolved_primary`: the code
//! snippets of the analyzer's own tests (`pkg/analyzer/test/src/dart/
//! resolution/class_test.dart`, `enum_test.dart`, `extension_type_test.dart`;
//! one file per test method whose code declares a primary constructor or a
//! primary constructor body, without the tests that need other files or
//! experiments).
//!
//! Needs `dart` on PATH (the oracle is compiled on first use).

use std::path::{Path, PathBuf};

use dartr_difftest::{Options, run};

fn options(mode: &str) -> Options {
    Options {
        mode: mode.to_string(),
        inputs: vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resolved_primary")],
        jobs: 4,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        no_diagnostics: true,
        ..Default::default()
    }
}

/// One test for both modes: the oracle is compiled on first use, and two
/// tests that start at the same time would both compile it.
#[test]
fn primary_constructors_match_the_analyzer_on_its_own_test_code() {
    for mode in ["resolved", "resolved-el"] {
        let report = run(&options(mode)).unwrap();
        println!("{}{}", report.kind_report(), report.summary());
        let mut oracle = 0;
        for (kind, k) in &report.kinds {
            oracle += k.oracle;
            assert_eq!(
                k.matched,
                k.oracle,
                "{mode} {kind}: {} of {}\n{}",
                k.matched,
                k.oracle,
                report.kind_report()
            );
        }
        assert!(oracle > 0, "{mode}: no oracle entries");
    }
}
