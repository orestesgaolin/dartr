//! The analyzer diagnostic tests of the g2 codes (imports, unused
//! elements, dead code, TODOs; unit D9), ported mechanically into
//! `wd_g2_fixtures/cases.rs` from `pkg/analyzer/test/src/diagnostics/`
//! (`dead_code`, `duplicate_{import,hidden_name,shown_name}`,
//! `undefined_{hidden,shown}_name`, `unnecessary_import`, `unused_*`,
//! `todo` tests: the tests whose body is `newFile` calls and one
//! `resolveTestCodeWithDiagnostics`, `assertErrorsInCode` or
//! `assertNoErrorsInCode`; `@SkippedTest` and `@FailingTest` cases are left
//! out). Only the diagnostics of the codes of
//! `tools/difftest/error_codes_g2.txt` are compared.

mod support;

#[path = "wd_g2_fixtures/cases.rs"]
mod cases;

use support::g3::run_ported_with_codes;

const G2_CODES: &str = include_str!("../../../tools/difftest/error_codes_g2.txt");

/// Cases that fail for reasons outside this group (name, reason).
const KNOWN_FAILURES: &[(&str, &str)] = &[
    (
        "dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_nullaware_deadCode",
        "anonymous methods experiment (not enabled by the test support)",
    ),
    (
        "dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_nullaware_deadCode",
        "anonymous methods experiment (not enabled by the test support)",
    ),
    (
        "dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_plain_deadCode",
        "anonymous methods experiment (not enabled by the test support)",
    ),
];

#[test]
fn g2_verifiers_report_the_diagnostics_of_the_analyzer_tests() {
    let (failures, count) = run_ported_with_codes(cases::CASES, G2_CODES, |_| true);
    let unexpected: Vec<&String> = failures
        .iter()
        .filter(|f| {
            !KNOWN_FAILURES
                .iter()
                .any(|(n, _)| f.starts_with(&format!("{n}:")))
        })
        .collect();
    eprintln!("{} of {count} cases pass", count - failures.len());
    assert!(
        unexpected.is_empty(),
        "{} of {count} cases failed:\n{}",
        unexpected.len(),
        unexpected
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
