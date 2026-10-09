// Dart source: pkg/analyzer/lib/src/error/correct_override.dart

//! STUB (wd-errors): the `CorrectOverrideHelper` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_element::ElemRef;
use dartr_typesystem::TypeSystem;

/// Dart `CorrectOverrideHelper(typeSystem:, thisMember:)
/// .isCorrectOverrideOf(superMember:)`. The stub accepts every override.
pub fn is_correct_override_of(
    type_system: &TypeSystem<'_>,
    this_member: ElemRef,
    super_member: ElemRef,
) -> bool {
    let _ = (type_system, this_member, super_member);
    true
}
