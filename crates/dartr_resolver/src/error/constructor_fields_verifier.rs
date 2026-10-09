// Dart source: pkg/analyzer/lib/src/error/constructor_fields_verifier.dart

//! STUB (wd-errors): the `ConstructorFieldsVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::{ClassMember, Id, NodeId};
use dartr_element::ElementId;

/// Dart `ConstructorFieldsVerifier` (one per library, in
/// `LibraryVerificationContext`).
#[derive(Debug, Default)]
pub struct ConstructorFieldsVerifier {}

impl ConstructorFieldsVerifier {
    /// Dart `addConstructors(diagnosticReporter, element, members,
    /// namePart)`: [unit] is the index of the unit in the library (the
    /// diagnostic reporter of the unit).
    pub fn add_constructors(
        &mut self,
        unit: usize,
        element: ElementId,
        members: &[Id<ClassMember>],
        name_part: NodeId,
    ) {
        let _ = (unit, element, members, name_part);
    }
}
