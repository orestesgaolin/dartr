// Dart source: pkg/analyzer/lib/src/error/super_formal_parameters_verifier.dart

//! `verifySuperFormalParameters` as `ErrorVerifier` calls it (without a
//! diagnostic reporter: it reports nothing). The resolver has its own copy
//! with the reporter (`element_resolver::verify_super_formal_parameters`).
//! STUB (wd-errors): the error/* port (branch `wd-errors`) may replace this
//! file.

use dartr_ast::{FormalParameterList, Id, SuperFormalParameter};

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
/// diagnosticReporter:)` without `hasExplicitPositionalArguments` (so it
/// reports nothing): the count of positional super parameters and the
/// names of the named super parameters.
pub fn verify_super_formal_parameters(
    ev: &mut ErrorVerifier<'_>,
    formal_parameter_list: Option<Id<FormalParameterList>>,
) -> SuperFormalParametersResult {
    let mut result = SuperFormalParametersResult::default();
    let Some(list) = formal_parameter_list else {
        return result;
    };
    let ast = ev.ast;
    for &parameter in ast.list(ast[list].parameters) {
        let Some(parameter) = ast.cast::<SuperFormalParameter>(parameter) else {
            continue;
        };
        if ast[parameter].kind.is_named() {
            // Dart `declaredFragment.name`.
            let name = ast.tokens.lexeme(ast[parameter].name);
            if !name.is_empty() {
                result.named_argument_names.push(name.to_string());
            }
        } else {
            result.positional_argument_count += 1;
        }
    }
    result
}
