// Dart source: pkg/analyzer/lib/src/error/super_formal_parameters_verifier.dart

//! `verifySuperFormalParameters`: the positional and named arguments that
//! the super formal parameters of a constructor pass to the super
//! constructor. Called by `ResolverVisitor.resolveArgumentsToParameters`
//! (for `super(...)` invocations, `invocation_inferrer.rs`) and by the
//! error verifier.

use dartr_ast::{FormalParameterList, Id, SuperFormalParameter};
use dartr_diagnostics::diag;

use super::VerifierHost;

/// Dart `VerifySuperFormalParametersResult`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifySuperFormalParametersResult {
    /// The count of positional arguments provided by the super-parameters.
    pub positional_argument_count: usize,
    /// The names of named arguments provided by the super-parameters.
    pub named_argument_names: Vec<String>,
}

/// Dart `verifySuperFormalParameters(formalParameterList:,
/// diagnosticReporter:, hasExplicitPositionalArguments:)`. Reports
/// `positionalSuperFormalParameterWithPositionalArgument` only when
/// [report] is set (Dart: a non-null `diagnosticReporter`).
pub fn verify_super_formal_parameters<'a, H: VerifierHost<'a>>(
    host: &mut H,
    formal_parameter_list: Id<FormalParameterList>,
    report: bool,
    has_explicit_positional_arguments: bool,
) -> VerifySuperFormalParametersResult {
    let mut result = VerifySuperFormalParametersResult::default();
    let ast = host.ast();
    let parameters: Vec<_> = ast
        .list(ast[formal_parameter_list].parameters)
        .iter()
        .filter_map(|&p| ast.cast::<SuperFormalParameter>(p))
        .collect();
    for parameter in parameters {
        let ast = host.ast();
        let is_named = ast[parameter].kind.is_named();
        let name_token = ast[parameter].name;
        if is_named {
            // Dart `declaredFragment.name`.
            let ctx = host.ctx();
            let name = host
                .tables()
                .declared_fragment
                .get(parameter)
                .and_then(|&f| ctx.fragment_data(f))
                .and_then(|f| f.name)
                .map(|n| ctx.name_str(n).to_string());
            if let Some(name) = name {
                result.named_argument_names.push(name);
            }
        } else {
            result.positional_argument_count += 1;
            if has_explicit_positional_arguments && report {
                let d = host.at_token(
                    diag::positional_super_formal_parameter_with_positional_argument(),
                    name_token,
                );
                host.report(d);
            }
        }
    }
    result
}
