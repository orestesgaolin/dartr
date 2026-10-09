// Dart source: pkg/analyzer/lib/src/generated/exhaustiveness.dart (STUB: replaced by branch wd-const-exhaustive)

//! STUB: the switch exhaustiveness check. Branch `wd-const-exhaustive`
//! replaces this file with the port of `exhaustiveness.dart` and of
//! `ConstantVerifier._validateSwitchExhaustiveness`.

/// Dart `AnalyzerExhaustivenessCache` (one per library analysis).
#[derive(Default)]
pub struct ExhaustivenessCache {}

/// The verifier state Dart passes to `_validateSwitchExhaustiveness`.
pub struct SwitchExhaustivenessInput<'r> {
    /// SwitchStatement or SwitchExpression.
    pub node: dartr_ast::NodeId,
    pub map_pattern_key_values:
        &'r indexmap::IndexMap<dartr_ast::NodeId, dartr_constant::DartObjectImpl>,
    pub constant_pattern_values:
        &'r indexmap::IndexMap<dartr_ast::NodeId, dartr_constant::DartObjectImpl>,
    pub must_be_exhaustive: bool,
    pub is_switch_expression: bool,
}

/// Dart `ConstantVerifier._validateSwitchExhaustiveness`: reports into `diagnostics`.
pub fn validate_switch_exhaustiveness(
    engine: &crate::constant::evaluation::ConstantEvaluationEngine<'_>,
    cache: &mut ExhaustivenessCache,
    unit: u32,
    input: &SwitchExhaustivenessInput<'_>,
    diagnostics: &mut Vec<dartr_diagnostics::Diagnostic>,
) {
    let _ = (engine, cache, unit, input, diagnostics);
}
