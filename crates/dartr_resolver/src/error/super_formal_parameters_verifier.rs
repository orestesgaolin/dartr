// Dart source: pkg/analyzer/lib/src/error/super_formal_parameters_verifier.dart

//! STUB (wd-errors): the `verifySuperFormalParameters` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::{FormalParameterList, Id};

use crate::error_verifier::ErrorVerifier;

/// Dart `SuperFormalParametersResult`.
#[derive(Clone, Debug, Default)]
pub struct SuperFormalParametersResult {
    /// Dart `namedArgumentNames`.
    pub named_argument_names: Vec<String>,
    /// Dart `positionalArgumentCount`.
    pub positional_argument_count: usize,
}

/// Dart `verifySuperFormalParameters(formalParameterList:,
/// diagnosticReporter:)`. The stub counts no super parameters.
pub fn verify_super_formal_parameters(
    ev: &mut ErrorVerifier<'_>,
    formal_parameter_list: Option<Id<FormalParameterList>>,
) -> SuperFormalParametersResult {
    let _ = (ev, formal_parameter_list);
    SuperFormalParametersResult::default()
}
