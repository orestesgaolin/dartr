//! The analyzer diagnostic tests of the g3 codes, ported mechanically
//! (`wd_g3_fixtures/*.rs`, generated from
//! `pkg/analyzer/test/src/diagnostics/<code>_test.dart`: the tests of the
//! `PubPackageResolutionTest` classes whose body is only `newFile` calls
//! and one `resolveTestCodeWithDiagnostics`, `assertErrorsInCode` or
//! `assertNoErrorsInCode`). Only the diagnostics of the g3 codes are
//! compared. Cases listed in [KNOWN_FAILURES] are run but not asserted
//! (see the reason next to each).

mod support;

#[path = "wd_g3_fixtures/best_practices.rs"]
mod best_practices;

use support::g3::run_ported;

/// Cases that fail for reasons outside this group (name, reason).
const KNOWN_FAILURES: &[(&str, &str)] = &[];

#[test]
fn best_practices_ported() {
    let (failures, count) = run_ported(best_practices::CASES, |_| true);
    let unexpected: Vec<&String> = failures
        .iter()
        .filter(|f| !KNOWN_FAILURES.iter().any(|(n, _)| f.starts_with(&format!("{n}:"))))
        .collect();
    eprintln!("{} of {count} cases pass", count - failures.len());
    assert!(
        unexpected.is_empty(),
        "{} of {count} cases failed:\n{}",
        unexpected.len(),
        unexpected.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
}
