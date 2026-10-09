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
const KNOWN_FAILURES: &[(&str, &str)] = &[
    (
        "assignment_of_do_not_store_test.dart::AssignmentOfDoNotStoreTest::test_topLevelVariable_asExpression",
        "@SkippedTest in the analyzer (not implemented there either)",
    ),
    (
        "assignment_of_do_not_store_test.dart::AssignmentOfDoNotStoreTest::test_topLevelVariable_cascadeExpression_target",
        "@SkippedTest in the analyzer (not implemented there either)",
    ),
    (
        "assignment_of_do_not_store_test.dart::AssignmentOfDoNotStoreTest::test_topLevelVariable_dotShorthandPropertyAccess",
        "@SkippedTest in the analyzer (not implemented there either)",
    ),
    (
        "assignment_of_do_not_store_test.dart::AssignmentOfDoNotStoreTest::test_topLevelVariable_nullAssert",
        "@SkippedTest in the analyzer (not implemented there either)",
    ),
    (
        "assignment_of_do_not_store_test.dart::AssignmentOfDoNotStoreTest::test_topLevelVariable_switchExpression_caseBody",
        "@SkippedTest in the analyzer (not implemented there either)",
    ),
    (
        "equal_elements_in_set_test.dart::EqualElementsInSetTest::test_constant_constant",
        "needs computeConstantValue() of non-const expressions; the port compares literal values only",
    ),
    (
        "equal_elements_in_set_test.dart::EqualElementsInSetTest::test_literal_constant",
        "needs computeConstantValue() of non-const expressions; the port compares literal values only",
    ),
    (
        "equal_keys_in_map_test.dart::EqualKeysInMapTest::test_constant_constant",
        "needs computeConstantValue() of non-const expressions; the port compares literal values only",
    ),
    (
        "equal_keys_in_map_test.dart::EqualKeysInMapTest::test_literal_constant",
        "needs computeConstantValue() of non-const expressions; the port compares literal values only",
    ),
    (
        "invalid_use_of_visible_for_testing_member_test.dart::InvalidUseOfVisibleForTestingMemberTest::test_import_show",
        "the names of combinators have no element (library_analyzer resolve_directives is a stub)",
    ),
];

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
