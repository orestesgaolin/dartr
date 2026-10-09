// Dart source: pkg/analyzer/lib/src/generated/error_detection_helpers.dart,
// pkg/analyzer/lib/src/error/bool_expression_verifier.dart (the checks that
// the resolver calls)

//! The checks of `ErrorDetectionHelpers` and `BoolExpressionVerifier` that
//! the resolver calls while it resolves.
//!
//! STUB (wave D, D5 statement/expression checks): the checks report
//! nothing yet. Their call sites are in place, so that porting a check
//! makes its diagnostics appear without touching the resolver.

use dartr_ast::{ArgumentList, Expression, Id};

use crate::resolver::ResolverVisitor;

impl<'a> ResolverVisitor<'a> {
    /// Dart `checkForArgumentTypesNotAssignableInList(argumentList,
    /// whyNotPromotedArguments)` (`generated/resolver.dart`).
    pub fn check_for_argument_types_not_assignable_in_list(
        &mut self,
        argument_list: Id<ArgumentList>,
    ) {
        let _ = argument_list;
    }

    /// Dart `boolExpressionVerifier.checkForNonBoolCondition(condition,
    /// whyNotPromoted: ...)`.
    pub fn check_for_non_bool_condition(&mut self, condition: Id<Expression>) {
        crate::error::bool_expression_verifier::check_for_non_bool_condition(self, condition);
    }

    /// Dart `boolExpressionVerifier.checkForNonBoolExpression(expression,
    /// locatableDiagnostic: diag.nonBoolExpression, whyNotPromoted: ...)`.
    pub fn check_for_non_bool_expression(&mut self, expression: Id<Expression>) {
        crate::error::bool_expression_verifier::check_for_non_bool_expression(
            self,
            expression,
            dartr_diagnostics::diag::non_bool_expression(),
        );
    }

    /// Dart `checkForBodyMayCompleteNormally(body:, errorNode:)`.
    pub fn check_for_body_may_complete_normally(&mut self, body: dartr_ast::NodeId, error_node: dartr_ast::NodeId) {
        let _ = (body, error_node);
    }

    /// Dart `nullableDereferenceVerifier.report(locatableDiagnostic,
    /// errorEntity, receiverType, messages: ...)`
    /// (error/nullable_dereference_verifier.dart). The why-not-promoted
    /// context messages are not ported yet.
    pub fn report_nullable_dereference(
        &mut self,
        locatable: dartr_diagnostics::LocatableDiagnostic,
        error_entity: dartr_ast::NodeId,
        receiver_type: dartr_element::TypeId,
    ) {
        crate::error::nullable_dereference_verifier::report(
            self,
            locatable,
            error_entity,
            receiver_type,
        );
    }

    /// Dart `nullableDereferenceVerifier.expression(locatableDiagnostic,
    /// expression, type: type)`: reports [locatable] at [expression] if
    /// its type (or [ty]) is potentially nullable. Returns whether it
    /// reported.
    pub fn nullable_dereference_expression(
        &mut self,
        locatable: dartr_diagnostics::LocatableDiagnostic,
        expression: Id<Expression>,
        ty: Option<dartr_element::TypeId>,
    ) -> bool {
        crate::error::nullable_dereference_verifier::expression(self, locatable, expression, ty)
    }

    /// Dart `checkForUseOfVoidResult(expression)`: whether the expression has
    /// type `void` in a place where it is not allowed (reports it).
    pub fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool {
        let _ = expression;
        false
    }
}
