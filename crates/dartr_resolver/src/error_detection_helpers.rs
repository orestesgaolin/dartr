// Dart source: pkg/analyzer/lib/src/generated/error_detection_helpers.dart,
// pkg/analyzer/lib/src/error/bool_expression_verifier.dart (the checks that
// the resolver calls)

//! The checks of `ErrorDetectionHelpers` and `BoolExpressionVerifier` that
//! the resolver calls while it resolves.
//!
//! STUB (wave D, D5 statement/expression checks): the checks report
//! nothing yet. Their call sites are in place, so that porting a check
//! makes its diagnostics appear without touching the resolver.

use dartr_ast::{Expression, Id};

use crate::resolver::ResolverVisitor;

impl<'a> ResolverVisitor<'a> {
    /// Dart `boolExpressionVerifier.checkForNonBoolCondition(condition,
    /// whyNotPromoted: ...)`.
    pub fn check_for_non_bool_condition(&mut self, condition: Id<Expression>) {
        let _ = condition;
    }

    /// Dart `boolExpressionVerifier.checkForNonBoolExpression(expression,
    /// locatableDiagnostic: diag.nonBoolExpression, whyNotPromoted: ...)`.
    pub fn check_for_non_bool_expression(&mut self, expression: Id<Expression>) {
        let _ = expression;
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
        // Dart: `receiverType == typeProvider.nullType` (Dart `==`).
        let locatable = if dartr_typesystem::TypeSystem::new(self.ctx)
            .dart_eq(receiver_type, self.ctx.tp.null_type())
        {
            dartr_diagnostics::diag::invalid_use_of_null_value()
        } else {
            locatable
        };
        let offset = self.ast.offset(error_entity) as usize;
        let length = self.ast.length(error_entity) as usize;
        self.report(locatable.at_offset(offset, length));
    }

    /// Dart `checkForUseOfVoidResult(expression)`: whether the expression has
    /// type `void` in a place where it is not allowed (reports it).
    pub fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool {
        let _ = expression;
        false
    }
}
