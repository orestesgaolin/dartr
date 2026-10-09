// Dart source: pkg/analyzer/lib/src/error/unused_local_elements_verifier.dart

//! STUB (wd-errors): not ported yet.

use super::UnitVerifier;

/// Dart `UsedLocalElements`.
#[derive(Default)]
pub struct UsedLocalElements {}

impl UsedLocalElements {
    /// Dart `UsedLocalElements.merge(parts)`.
    pub fn merge(parts: Vec<UsedLocalElements>) -> UsedLocalElements {
        let _ = parts;
        UsedLocalElements::default()
    }
}

/// Dart `unit.accept(GatherUsedLocalElementsVisitor(library))`.
pub fn gather_used_local_elements(v: &UnitVerifier<'_>) -> UsedLocalElements {
    let _ = v;
    UsedLocalElements::default()
}

/// Dart `unit.accept(UnusedLocalElementsVerifier(reporter, usedElements,
/// library))`.
pub fn verify(v: &mut UnitVerifier<'_>, used: &UsedLocalElements) {
    let _ = (v, used);
}
