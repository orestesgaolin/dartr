// Dart source: pkg/analyzer/lib/src/generated/error_detection_helpers.dart,
// pkg/analyzer/lib/src/error/bool_expression_verifier.dart (the checks that
// the resolver calls)

//! The `ErrorDetectionHelpers` mixin (shared by `ResolverVisitor` and
//! `ErrorVerifier`) and the checks of `BoolExpressionVerifier` that the
//! resolver calls while it resolves.
//!
//! The mixin is the trait [`ErrorDetectionHelpers`] (design §7: a mixin
//! with state is a trait with required accessors and provided methods).
//! `ResolverVisitor` keeps inherent methods with the same names, so that
//! the resolver files call them without importing the trait.
//!
//! STUB (wave D, D5 statement/expression checks): the checks report
//! nothing yet. Their call sites are in place, so that porting a check
//! makes its diagnostics appear without touching the resolver.

use dartr_ast::{Ast, Expression, Id, NodeId};
use dartr_diagnostics::{LocatableDiagnostic, LocatedDiagnostic};
use dartr_element::{Ctx, ElemRef, ResolutionTables, TypeId};
use dartr_typesystem::TypeSystem;

use crate::error_verifier::ErrorVerifier;
use crate::resolver::ResolverVisitor;

/// Dart `NonAssignabilityReporter` and its subclasses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonAssignabilityReporter {
    /// Dart `NonAssignabilityReporterForArgument`
    /// (`argument_type_not_assignable`).
    ForArgument,
    /// Dart `NonAssignabilityReporterForAssignment` (`invalid_assignment`).
    ForAssignment,
}

/// Dart `mixin ErrorDetectionHelpers`. The required methods are the
/// abstract getters of the mixin (and the AST and resolution results, which
/// Dart reads from the nodes).
pub trait ErrorDetectionHelpers<'a> {
    /// The lookup context (with the unit's local arena).
    fn edh_ctx(&self) -> Ctx<'a>;
    /// Dart `typeSystem`.
    fn edh_type_system(&self) -> TypeSystem<'a>;
    /// The AST of the unit.
    fn edh_ast(&self) -> &Ast;
    /// The resolution results (`staticType`, `element`, ...).
    fn edh_tables(&self) -> &ResolutionTables;
    /// Dart `diagnosticReporter.report(diagnostic)`.
    fn edh_report(&mut self, diagnostic: LocatedDiagnostic);
    /// Dart `strictCasts`.
    fn edh_strict_casts(&self) -> bool;

    /// Dart `diagnostic.at(node)`.
    fn edh_at(&self, diagnostic: LocatableDiagnostic, node: NodeId) -> LocatedDiagnostic {
        let ast = self.edh_ast();
        diagnostic.at_offset(ast.offset(node) as usize, ast.length(node) as usize)
    }

    /// Dart `checkForUseOfVoidResult(expression)`: whether [expression] has
    /// type `void` in a place where it is not allowed (reports it).
    fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool {
        let _ = expression;
        false
    }

    /// Dart `checkForAssignableExpressionAtType(expression,
    /// actualStaticType, expectedStaticType, nonAssignabilityReporter)`.
    fn check_for_assignable_expression_at_type(
        &mut self,
        expression: Id<Expression>,
        actual_static_type: TypeId,
        expected_static_type: TypeId,
        reporter: NonAssignabilityReporter,
    ) {
        let _ = (
            expression,
            actual_static_type,
            expected_static_type,
            reporter,
        );
    }

    /// Dart `checkForArgumentTypeNotAssignable(expression,
    /// expectedStaticType, actualStaticType, nonAssignabilityReporter)`.
    fn check_for_argument_type_not_assignable(
        &mut self,
        expression: Id<Expression>,
        expected_static_type: TypeId,
        actual_static_type: TypeId,
        reporter: NonAssignabilityReporter,
    ) {
        let _ = (
            expression,
            expected_static_type,
            actual_static_type,
            reporter,
        );
    }

    /// Dart `getImplicitCallMethod(type, context, errorNode)`: the `call`
    /// method when an assignment of [ty] to [context] is an implicit `call`
    /// tear-off.
    fn get_implicit_call_method(
        &mut self,
        ty: TypeId,
        context: TypeId,
        error_node: NodeId,
    ) -> Option<ElemRef> {
        let _ = (ty, context, error_node);
        None
    }
}

impl<'a> ErrorDetectionHelpers<'a> for ResolverVisitor<'a> {
    fn edh_ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn edh_type_system(&self) -> TypeSystem<'a> {
        self.type_system
    }

    fn edh_ast(&self) -> &Ast {
        self.ast
    }

    fn edh_tables(&self) -> &ResolutionTables {
        self.tables
    }

    fn edh_report(&mut self, diagnostic: LocatedDiagnostic) {
        self.report(diagnostic);
    }

    fn edh_strict_casts(&self) -> bool {
        self.unit.options.strict_casts
    }
}

impl<'a> ErrorDetectionHelpers<'a> for ErrorVerifier<'a> {
    fn edh_ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn edh_type_system(&self) -> TypeSystem<'a> {
        self.type_system
    }

    fn edh_ast(&self) -> &Ast {
        self.ast
    }

    fn edh_tables(&self) -> &ResolutionTables {
        self.tables
    }

    fn edh_report(&mut self, diagnostic: LocatedDiagnostic) {
        self.report(diagnostic);
    }

    fn edh_strict_casts(&self) -> bool {
        self.unit.options.strict_casts
    }
}

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
    pub fn check_for_body_may_complete_normally(
        &mut self,
        body: dartr_ast::NodeId,
        error_node: dartr_ast::NodeId,
    ) {
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
        let receiver_type = ty.unwrap_or_else(|| self.type_or_throw(expression));
        if matches!(
            self.ctx.ty(receiver_type),
            dartr_element::TypeKind::Dynamic | dartr_element::TypeKind::Invalid
        ) || !self.type_system.is_potentially_nullable(receiver_type)
        {
            return false;
        }
        self.report_nullable_dereference(locatable, expression.raw(), receiver_type);
        true
    }

    /// Dart `checkForUseOfVoidResult(expression)` (the mixin method).
    pub fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool {
        ErrorDetectionHelpers::check_for_use_of_void_result(self, expression)
    }
}
