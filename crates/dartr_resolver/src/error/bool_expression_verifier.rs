// Dart source: pkg/analyzer/lib/src/error/bool_expression_verifier.dart

//! Checks of expressions that must have the type `bool` (Dart
//! `BoolExpressionVerifier`), which the resolver calls while it resolves.
//!
//! Open point: the why-not-promoted context messages of
//! `unchecked_use_of_nullable_value_as_condition` are not attached.

use dartr_ast::{Expression, Id, MethodInvocation};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::TypeExt;

use super::{VerifierHost, nullable_dereference_verifier};

/// Dart `checkForNonBoolCondition(condition, whyNotPromoted:)`: checks that
/// [condition] has the type `bool`; reports `non_bool_condition` if not.
pub fn check_for_non_bool_condition<'a, H: VerifierHost<'a>>(
    host: &mut H,
    condition: Id<Expression>,
) {
    check_for_non_bool_expression(host, condition, diag::non_bool_condition());
}

/// Dart `checkForNonBoolExpression(expression, locatableDiagnostic:,
/// whyNotPromoted:)`: verifies that [expression] has the type `bool`, and
/// reports [locatable] if not, or a nullability error if it is improperly
/// nullable.
pub fn check_for_non_bool_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    expression: Id<Expression>,
    locatable: LocatableDiagnostic,
) {
    // Dart `expression.typeOrThrow` (no throw: an unresolved expression is
    // `dynamic`, which is assignable to `bool`).
    let ty = host.static_type(expression).unwrap_or(TypeId::DYNAMIC);
    let ctx = host.ctx();
    let bool_type = ctx.tp.bool_type();
    if !check_for_use_of_void_result(host, expression)
        && !host
            .type_system()
            .is_assignable_to(ty, bool_type, host.options().strict_casts)
    {
        if ctx.is_dart_core_bool(ty) {
            nullable_dereference_verifier::report(
                host,
                diag::unchecked_use_of_nullable_value_as_condition(),
                expression.raw(),
                ty,
            );
        } else {
            let d = host.at(locatable, expression);
            host.report(d);
        }
    }
}

/// Dart `checkForNonBoolNegationExpression(expression, whyNotPromoted:)`.
pub fn check_for_non_bool_negation_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    expression: Id<Expression>,
) {
    check_for_non_bool_expression(host, expression, diag::non_bool_negation_expression());
}

/// Dart `_checkForUseOfVoidResult(expression)`: reports `use_of_void_result`
/// if [expression] has the type `void`; returns whether it reported.
fn check_for_use_of_void_result<'a, H: VerifierHost<'a>>(
    host: &mut H,
    expression: Id<Expression>,
) -> bool {
    let Some(ty) = host.static_type(expression) else {
        return false;
    };
    if !matches!(host.ctx().ty(ty), TypeKind::Void) {
        return false;
    }
    let d = if let Some(invocation) = host.ast().cast::<MethodInvocation>(expression) {
        let method_name = host.ast()[invocation].method_name;
        host.at(diag::use_of_void_result(), method_name)
    } else {
        host.at(diag::use_of_void_result(), expression)
    };
    host.report(d);
    true
}
