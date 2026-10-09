// Dart source: pkg/analyzer/lib/src/error/inference_error.dart

//! The top-level type inference errors (Dart `TopLevelInferenceError`,
//! `TopLevelInferenceErrorKind`).
//!
//! The data type is in `dartr_element` (the linker writes it into
//! elements: Dart `typeInferenceError`); the binary read and write of the
//! Dart file are not needed (no summaries). The error verifier reads it to
//! report `top_level_cycle` and `no_combined_super_signature`.

pub use dartr_element::TopLevelInferenceError;

/// Dart `TopLevelInferenceErrorKind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TopLevelInferenceErrorKind {
    DependencyCycle,
    OverrideNoCombinedSuperSignature,
}

/// The kind of [error] (Dart: the class of the error, and the enum value
/// that `write` writes).
pub fn kind(error: &TopLevelInferenceError) -> TopLevelInferenceErrorKind {
    match error {
        TopLevelInferenceError::DependencyCycle { .. } => {
            TopLevelInferenceErrorKind::DependencyCycle
        }
        TopLevelInferenceError::OverrideNoCombinedSuperSignature { .. } => {
            TopLevelInferenceErrorKind::OverrideNoCombinedSuperSignature
        }
    }
}
