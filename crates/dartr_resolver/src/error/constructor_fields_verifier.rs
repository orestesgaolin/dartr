// Dart source: pkg/analyzer/lib/src/error/constructor_fields_verifier.dart

//! STUB (wd-errors): not ported yet.

use super::UnitVerifier;

/// Dart `ConstructorFieldsVerifier` (one per library, in
/// `LibraryVerificationContext`; the error verifier adds the constructors).
#[derive(Default)]
pub struct ConstructorFieldsVerifier {}

impl ConstructorFieldsVerifier {
    /// Dart `report()`: all units of the library.
    pub fn report(&mut self, units: &mut [UnitVerifier<'_>]) {
        let _ = units;
    }
}
