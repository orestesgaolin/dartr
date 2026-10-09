// Dart source: pkg/analyzer/lib/src/generated/error_detection_helpers.dart,
// pkg/analyzer/lib/src/error/bool_expression_verifier.dart (the checks that
// the resolver calls), pkg/analyzer/lib/src/generated/resolver.dart
// (`checkForArgumentTypesNotAssignableInList`,
// `checkForBodyMayCompleteNormally`, `_checkForFutureCatchErrorOnError`)

//! The `ErrorDetectionHelpers` mixin (shared by `ResolverVisitor` and
//! `ErrorVerifier`) and the checks of `BoolExpressionVerifier` that the
//! resolver calls while it resolves.
//!
//! The mixin is the trait [`ErrorDetectionHelpers`] (design §7: a mixin
//! with state is a trait with required accessors and provided methods).
//! `ResolverVisitor` keeps inherent methods with the same names, so that
//! the resolver files call them without importing the trait.
//!
//! Not ported: the why-not-promoted context messages
//! (`computeWhyNotPromotedMessages`); the diagnostics have no context
//! messages.

use dartr_ast::{
    ArgumentList, AssignmentExpression, BinaryExpression, IndexExpression, Ast, BlockFunctionBody, ConstructorDeclaration, Expression,
    FunctionDeclaration, FunctionExpressionInvocation, Id, MethodDeclaration, MethodInvocation,
    NamedArgument, NodeId, ParenthesizedExpression, CascadeExpression,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_diagnostics::{LocatableDiagnostic, LocatedDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{Ctx, ElemRef, ElementId, Nullability, ResolutionTables, Tag, TypeId, TypeKind};
use dartr_syntax::TokenType;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::{TypeExt, TypeSystem, member};

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

impl NonAssignabilityReporter {
    /// Dart `createDiagnostic(expectedStaticType:, actualStaticType:)`.
    pub fn create_diagnostic(
        self,
        ctx: &Ctx<'_>,
        expected_static_type: TypeId,
        actual_static_type: TypeId,
    ) -> LocatableDiagnostic {
        match self {
            NonAssignabilityReporter::ForAssignment => diag::invalid_assignment(
                type_arg(ctx, actual_static_type),
                type_arg(ctx, expected_static_type),
            ),
            NonAssignabilityReporter::ForArgument => {
                let mut additional_info = Vec::<String>::new();
                if let (
                    TypeKind::Record {
                        positional: expected_positional,
                        named: expected_named,
                        ..
                    },
                    TypeKind::Record {
                        positional: actual_positional,
                        named: actual_named,
                        ..
                    },
                ) = (*ctx.ty(expected_static_type), *ctx.ty(actual_static_type))
                {
                    let actual_positional_fields = ctx.list(actual_positional).len();
                    let expected_positional_fields = ctx.list(expected_positional).len();
                    if expected_positional_fields != 0
                        && actual_positional_fields != expected_positional_fields
                    {
                        additional_info.push(format!(
                            "Expected {expected_positional_fields} positional arguments, but got {actual_positional_fields} instead."
                        ));
                    }
                    let actual_named_fields_length = ctx.list(actual_named).len();
                    let expected_named_fields_length = ctx.list(expected_named).len();
                    if expected_named_fields_length != 0
                        && actual_named_fields_length != expected_named_fields_length
                    {
                        additional_info.push(format!(
                            "Expected {expected_named_fields_length} named arguments, but got {actual_named_fields_length} instead."
                        ));
                    }
                    let named_fields = ctx.list(expected_named);
                    if !named_fields.is_empty() {
                        for field in ctx.list(actual_named) {
                            if !named_fields.iter().any(|element| {
                                element.name == field.name
                                    && ctx.dart_eq(field.ty, element.ty)
                            }) {
                                additional_info.push(format!(
                                    "Unexpected named argument `{}` with type `{}`.",
                                    ctx.name_str(field.name),
                                    dartr_element::diagnostics::type_display_string(
                                        ctx, field.ty, false
                                    ),
                                ));
                            }
                        }
                    }
                }
                diag::argument_type_not_assignable(
                    type_arg(ctx, actual_static_type),
                    type_arg(ctx, expected_static_type),
                    &additional_info.join(" "),
                )
            }
        }
    }
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
    /// The resolver-private node data (`correspondingParameter` types).
    fn edh_rt(&self) -> &crate::tables::ResolverTables;

    /// Dart `diagnostic.at(node)`.
    fn edh_at(&self, diagnostic: LocatableDiagnostic, node: NodeId) -> LocatedDiagnostic {
        let ast = self.edh_ast();
        diagnostic.at_offset(ast.offset(node) as usize, ast.length(node) as usize)
    }

    /// Dart `node.staticType`.
    fn edh_static_type(&self, node: NodeId) -> Option<TypeId> {
        self.edh_tables().static_type.get(node).copied()
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
        let ctx = self.edh_ctx();
        if !matches!(ctx.ty(expected_static_type), TypeKind::Void)
            && self.check_for_use_of_void_result(expression)
        {
            return;
        }
        self.check_for_assignable_expression_at_type(
            expression,
            actual_static_type,
            expected_static_type,
            reporter,
        );
    }

    /// Dart `checkForArgumentTypeNotAssignableForArgument(argument,
    /// promoteParameterToNullable:)`. [argument] is an `Argument` of an
    /// argument list, or an expression whose Dart `correspondingParameter`
    /// comes from its parent (the right operand of a binary expression, the
    /// right-hand side of an assignment).
    fn check_for_argument_type_not_assignable_for_argument(
        &mut self,
        argument: NodeId,
        promote_parameter_to_nullable: bool,
    ) {
        let ctx = self.edh_ctx();
        let ast = self.edh_ast();
        // Dart `argument.argumentExpression`.
        let expression: Id<Expression> = match ast.cast::<NamedArgument>(argument) {
            Some(named) => ast[named].argument_expression,
            None => Id::from_raw(argument),
        };
        let corresponding_parameter_type = match self.corresponding_parameter_type(argument) {
            Some(ty) => Some(ty),
            None => {
                // Treat dynamic invocations as having function types where
                // all formal parameter types are 'dynamic'.
                let function_type = ast
                    .parent(argument)
                    .and_then(|p| ast.cast::<ArgumentList>(p))
                    .and_then(|list| ast.parent(list.raw()))
                    .and_then(|p| ast.cast::<FunctionExpressionInvocation>(p))
                    .and_then(|invocation| {
                        let function = ast[invocation].function;
                        self.edh_static_type(function.raw())
                    });
                match function_type {
                    Some(t)
                        if t == TypeId::DYNAMIC
                            || matches!(ctx.ty(t), TypeKind::Never(Nullability::None))
                            || (matches!(ctx.ty(t), TypeKind::Interface { .. })
                                && ctx.is_dart_core_function(t)) =>
                    {
                        Some(TypeId::DYNAMIC)
                    }
                    _ => None,
                }
            }
        };
        if let Some(corresponding_parameter_type) = corresponding_parameter_type {
            self.check_for_argument_type_not_assignable_for_argument_inner(
                expression,
                corresponding_parameter_type,
                promote_parameter_to_nullable,
            );
        }
    }

    /// The type of Dart `argument.correspondingParameter`
    /// (`ArgumentImpl.correspondingParameter` and
    /// `ExpressionImpl.correspondingParameter`) for an argument of an
    /// argument list, the right operand of a binary expression and the
    /// right-hand side of an assignment.
    fn corresponding_parameter_type(&self, argument: NodeId) -> Option<TypeId> {
        let ctx = self.edh_ctx();
        let ast = self.edh_ast();
        let tables = self.edh_tables();
        let parent = ast.parent(argument)?;
        if ast.is::<ArgumentList>(parent) {
            let expression = match ast.cast::<NamedArgument>(argument) {
                Some(named) => ast[named].argument_expression.raw(),
                None => argument,
            };
            if let Some(&ty) = self.edh_rt().corresponding_parameter_type.get(expression) {
                return Some(ty);
            }
            let parameter = tables.param_element.get(expression).copied()?;
            return Some(member::type_(&ctx, parameter));
        }
        if let Some(binary) = ast.cast::<BinaryExpression>(parent) {
            if ast[binary].right_operand.raw() != argument {
                return None;
            }
            let invoke_type = tables.invoke_type.get(binary).copied()?;
            let TypeKind::Function(f) = *ctx.ty(invoke_type) else {
                return None;
            };
            return ctx.list(f.params).first().map(|p| p.ty);
        }
        if let Some(assignment) = ast.cast::<AssignmentExpression>(parent) {
            if ast[assignment].right_hand_side.raw() != argument {
                return None;
            }
            let is_eq = ast.tokens.ty(ast[assignment].operator) == TokenType::EQ;
            let executable = if is_eq {
                tables.write_element.get(assignment).copied()
            } else {
                tables.element.get(assignment).copied()
            }?;
            if !is_executable_element(member::base_element(&ctx, executable)) {
                return None;
            }
            let parameters = member::formal_parameters(&ctx, executable);
            if parameters.is_empty() {
                return None;
            }
            let parameter = if is_eq && ast.is::<IndexExpression>(ast[assignment].left_hand_side) {
                if parameters.len() == 2 {
                    parameters[1]
                } else {
                    return None;
                }
            } else {
                parameters[0]
            };
            return Some(member::type_(&ctx, parameter));
        }
        None
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
        let ctx = self.edh_ctx();
        if !matches!(ctx.ty(expected_static_type), TypeKind::Void)
            && self.check_for_use_of_void_result(expression)
        {
            return;
        }

        let ts = self.edh_type_system();
        let strict_casts = self.edh_strict_casts();
        if ts.is_assignable_to(actual_static_type, expected_static_type, strict_casts) {
            return;
        }

        let ast = self.edh_ast();
        if let TypeKind::Record { positional, .. } = *ctx.ty(expected_static_type)
            && ctx.list(positional).len() == 1
            && !matches!(ctx.ty(actual_static_type), TypeKind::Record { .. })
            && ast.is::<ParenthesizedExpression>(expression)
        {
            let field = ctx.list(positional)[0];
            if ts.is_assignable_to(field, actual_static_type, strict_casts) {
                let d = self.edh_at(
                    diag::record_literal_one_positional_no_trailing_comma_by_type(),
                    expression.raw(),
                );
                self.edh_report(d);
                return;
            }
        }

        // Dart `getErrorNode(node)`.
        let mut error_node = expression.raw();
        loop {
            if let Some(c) = ast.cast::<CascadeExpression>(error_node) {
                error_node = ast[c].target.raw();
            } else if let Some(p) = ast.cast::<ParenthesizedExpression>(error_node) {
                error_node = ast[p].expression.raw();
            } else {
                break;
            }
        }
        let d = reporter.create_diagnostic(&ctx, expected_static_type, actual_static_type);
        let d = self.edh_at(d, error_node);
        self.edh_report(d);
    }

    /// Dart `checkForFieldInitializerNotAssignable(initializer,
    /// fieldElement, isConstConstructor:)`. [field_type] is
    /// `fieldElement.type`; [expression] is `initializer.expression`.
    fn check_for_field_initializer_not_assignable(
        &mut self,
        expression: Id<Expression>,
        field_type: TypeId,
        is_const_constructor: bool,
    ) {
        let ctx = self.edh_ctx();
        let Some(static_type) = self.edh_static_type(expression.raw()) else {
            return;
        };
        let ts = self.edh_type_system();
        if ts.is_assignable_to(static_type, field_type, self.edh_strict_casts()) {
            if !matches!(ctx.ty(field_type), TypeKind::Void) {
                self.check_for_use_of_void_result(expression);
            }
            return;
        }
        let d = if is_const_constructor {
            diag::const_field_initializer_not_assignable(
                type_arg(&ctx, static_type),
                type_arg(&ctx, field_type),
            )
        } else {
            diag::field_initializer_not_assignable(
                type_arg(&ctx, static_type),
                type_arg(&ctx, field_type),
            )
        };
        let d = self.edh_at(d, expression.raw());
        self.edh_report(d);
    }

    /// Dart `checkForUseOfVoidResult(expression)`: whether [expression] has
    /// type `void` in a place where it is not allowed (reports it).
    fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool {
        let ctx = self.edh_ctx();
        let Some(ty) = self.edh_static_type(expression.raw()) else {
            return false;
        };
        if !matches!(ctx.ty(ty), TypeKind::Void) {
            return false;
        }
        let ast = self.edh_ast();
        let d = match ast.cast::<MethodInvocation>(expression) {
            Some(invocation) => {
                let method_name = ast[invocation].method_name;
                self.edh_at(diag::use_of_void_result(), method_name.raw())
            }
            None => self.edh_at(diag::use_of_void_result(), expression.raw()),
        };
        self.edh_report(d);
        true
    }

    /// Dart `checkIndexExpressionIndex(index, readElement:,
    /// writeElement:)`.
    fn check_index_expression_index(
        &mut self,
        index: Id<Expression>,
        read_element: Option<ElemRef>,
        write_element: Option<ElemRef>,
    ) {
        let ctx = self.edh_ctx();
        for element in [read_element, write_element].into_iter().flatten() {
            if member::base_element(&ctx, element).tag() != Tag::Method {
                continue;
            }
            let parameters = member::formal_parameters(&ctx, element);
            if let Some(&first) = parameters.first() {
                let ty = member::type_(&ctx, first);
                self.check_for_argument_type_not_assignable_for_argument_inner(index, ty, false);
            }
        }
    }

    /// Dart `getImplicitCallMethod(type, context, errorNode)`: the `call`
    /// method when an assignment of [ty] to [context] is an implicit `call`
    /// tear-off.
    fn get_implicit_call_method(&mut self, ty: TypeId, context: TypeId) -> Option<ElemRef> {
        let ctx = self.edh_ctx();
        let mut ty = ty;
        let mut visited_types = indexmap::IndexSet::new();
        visited_types.insert(ty);
        while let TypeKind::TypeParameter { nullability, .. } = *ctx.ty(ty) {
            if nullability != Nullability::None {
                // The value might be `null`, so implicit `.call` tearoff is
                // invalid.
                return None;
            }
            ty = ctx.type_parameter_type_bound(ty);
            if !visited_types.insert(ty) {
                // A cycle!
                return None;
            }
        }
        if !self.edh_type_system().accepts_function_type(context) {
            return None;
        }
        let TypeKind::Interface {
            element,
            nullability,
            ..
        } = *ctx.ty(ty)
        else {
            return None;
        };
        if nullability == Nullability::Question {
            return None;
        }
        let library = ctx.element_data(element.raw()).and_then(|d| d.library);
        let name = Name::for_library(&ctx, library, "call");
        InheritanceManager3::new(ctx)
            .get_member3(ty, name, GetMemberOptions::default())
            .filter(|&m| member::base_element(&ctx, m).tag() == Tag::Method)
    }

    /// Dart `getVariableElement(expression)`: the variable element that
    /// [expression] (an identifier) references.
    fn get_variable_element(&self, expression: Option<Id<Expression>>) -> Option<ElemRef> {
        let expression = expression?;
        let ast = self.edh_ast();
        if !ast.is::<dartr_ast::Identifier>(expression) {
            return None;
        }
        let element = self.edh_tables().element.get(expression).copied()?;
        let ctx = self.edh_ctx();
        is_variable_element(member::base_element(&ctx, element)).then_some(element)
    }

    /// Dart `_checkForArgumentTypeNotAssignableForArgument(argument:,
    /// staticParameterType:, promoteParameterToNullable:)`.
    fn check_for_argument_type_not_assignable_for_argument_inner(
        &mut self,
        argument: Id<Expression>,
        static_parameter_type: TypeId,
        promote_parameter_to_nullable: bool,
    ) {
        let mut static_parameter_type = static_parameter_type;
        if promote_parameter_to_nullable {
            static_parameter_type = self.edh_type_system().make_nullable(static_parameter_type);
        }
        // Dart `argument.typeOrThrow`.
        let Some(actual) = self.edh_static_type(argument.raw()) else {
            return;
        };
        self.check_for_argument_type_not_assignable(
            argument,
            static_parameter_type,
            actual,
            NonAssignabilityReporter::ForArgument,
        );
    }
}

/// Dart `element is ExecutableElement`.
pub fn is_executable_element(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::Getter
            | Tag::Setter
            | Tag::Method
            | Tag::Constructor
            | Tag::TopLevelFunction
            | Tag::LocalFunction
    )
}

/// Dart `element is VariableElement`.
pub fn is_variable_element(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::Field
            | Tag::TopLevelVariable
            | Tag::FormalParameter
            | Tag::FieldFormalParameter
            | Tag::SuperFormalParameter
            | Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
    )
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

    fn edh_rt(&self) -> &crate::tables::ResolverTables {
        self.rt
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

    fn edh_rt(&self) -> &crate::tables::ResolverTables {
        self.rt
    }
}

impl<'a> ResolverVisitor<'a> {
    /// Dart `boolExpressionVerifier.checkForNonBoolCondition(condition,
    /// whyNotPromoted: ...)`.
    pub fn check_for_non_bool_condition(&mut self, condition: Id<Expression>) {
        self.check_for_non_bool_expression(condition, diag::non_bool_condition());
    }

    /// Dart `boolExpressionVerifier.checkForNonBoolExpression(expression,
    /// locatableDiagnostic:, whyNotPromoted: ...)`: reports
    /// [locatable] if [expression] is not of type `bool`, or a nullability
    /// error if it is improperly nullable.
    pub fn check_for_non_bool_expression(
        &mut self,
        expression: Id<Expression>,
        locatable: LocatableDiagnostic,
    ) {
        let Some(ty) = self.static_type(expression) else {
            return;
        };
        let bool_type = self.ctx.tp.bool_type();
        // Dart `_checkForUseOfVoidResult` of `BoolExpressionVerifier` (the
        // same as the mixin method).
        if !ErrorDetectionHelpers::check_for_use_of_void_result(self, expression)
            && !self
                .type_system
                .is_assignable_to(ty, bool_type, self.unit.options.strict_casts)
        {
            if self.ctx.is_dart_core_bool(ty) {
                self.report_nullable_dereference(
                    diag::unchecked_use_of_nullable_value_as_condition(),
                    expression.raw(),
                    ty,
                );
            } else {
                let d = self.at(locatable, expression);
                self.report(d);
            }
        }
    }

    /// Dart `boolExpressionVerifier.checkForNonBoolNegationExpression(
    /// expression, whyNotPromoted: ...)`.
    pub fn check_for_non_bool_negation_expression(&mut self, expression: Id<Expression>) {
        self.check_for_non_bool_expression(expression, diag::non_bool_negation_expression());
    }

    /// Dart `checkForArgumentTypesNotAssignableInList(argumentList,
    /// whyNotPromotedArguments)`: verifies that the arguments of
    /// [argument_list] can be assigned to their parameters.
    pub fn check_for_argument_types_not_assignable_in_list(
        &mut self,
        argument_list: Id<ArgumentList>,
    ) {
        let arguments = self.ast.list(self.ast[argument_list].arguments).to_vec();
        for argument in arguments {
            self.check_for_argument_type_not_assignable_for_argument(argument.raw(), false);
        }
    }

    /// Dart `checkForAssignableExpressionAtType` (the mixin method).
    pub fn check_for_assignable_expression_at_type(
        &mut self,
        expression: Id<Expression>,
        actual_static_type: TypeId,
        expected_static_type: TypeId,
        reporter: NonAssignabilityReporter,
    ) {
        ErrorDetectionHelpers::check_for_assignable_expression_at_type(
            self,
            expression,
            actual_static_type,
            expected_static_type,
            reporter,
        );
    }

    /// Dart `checkForFieldInitializerNotAssignable` (the mixin method).
    pub fn check_for_field_initializer_not_assignable(
        &mut self,
        expression: Id<Expression>,
        field_type: TypeId,
        is_const_constructor: bool,
    ) {
        ErrorDetectionHelpers::check_for_field_initializer_not_assignable(
            self,
            expression,
            field_type,
            is_const_constructor,
        );
    }

    /// Dart `checkIndexExpressionIndex` (the mixin method).
    pub fn check_index_expression_index(
        &mut self,
        index: Id<Expression>,
        read_element: Option<ElemRef>,
        write_element: Option<ElemRef>,
    ) {
        ErrorDetectionHelpers::check_index_expression_index(
            self,
            index,
            read_element,
            write_element,
        );
    }

    /// Dart `checkForArgumentTypeNotAssignableForArgument(argument,
    /// promoteParameterToNullable:)` (the mixin method).
    pub fn check_for_argument_type_not_assignable_for_argument(
        &mut self,
        argument: NodeId,
        promote_parameter_to_nullable: bool,
    ) {
        ErrorDetectionHelpers::check_for_argument_type_not_assignable_for_argument(
            self,
            argument,
            promote_parameter_to_nullable,
        );
    }

    /// Dart `checkForBodyMayCompleteNormally(body:, errorNode:)`. For a
    /// function or method declaration [error_node] is the declaration; the
    /// diagnostic is at its name (Dart passes `node.name`).
    pub fn check_for_body_may_complete_normally(&mut self, body: NodeId, error_node: NodeId) {
        let Some(body_context) = self.rt.body_context.get(body).cloned() else {
            return;
        };

        let Some(flow) = self.flow_analysis.flow.as_ref() else {
            return;
        };
        if !flow.is_reachable() {
            // Dart `bodyContext.mayCompleteNormally = false` (nothing reads
            // it).
            return;
        }

        let Some(return_type) = body_context.context_type else {
            if let Some(error_node) = self.ast.cast::<BlockFunctionBody>(error_node) {
                self.check_for_future_catch_error_on_error(error_node);
            }
            return;
        };

        let Some(block_body) = self.ast.cast::<BlockFunctionBody>(body) else {
            return;
        };
        if body_context.is_generator {
            return;
        }

        let ctx = self.ctx;
        if body_context.is_async {
            // Check whether the return type is legal. If not, return rather
            // than reporting a second error.
            let lower_bound = ctx.tp.future_type(&ctx, ctx.tp.never_type());
            if let Some(imposed_type) = body_context.imposed_type
                && !self.type_system.is_subtype_of(lower_bound, imposed_type)
            {
                // [imposedType] is an illegal return type for an
                // asynchronous non-generator function; do not report an
                // additional error here.
                return;
            }
        }

        let locatable = if self.type_system.is_potentially_non_nullable(return_type) {
            diag::body_might_complete_normally(type_arg(&ctx, return_type))
        } else {
            let return_type_base = self.type_system.future_or_base(return_type);
            if matches!(
                ctx.ty(return_type_base),
                TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Unknown | TypeKind::Void
            ) || ctx.is_dart_core_null(return_type_base)
            {
                return;
            }
            diag::body_might_complete_normally_nullable(type_arg(&ctx, return_type))
        };
        let _ = block_body;
        let ast = &*self.ast;
        let d = if let Some(c) = ast.cast::<ConstructorDeclaration>(error_node) {
            // Dart `ConstructorDeclaration.errorRange`.
            let start = ast[c]
                .type_name
                .map(|t| ast.offset(t.raw()))
                .or_else(|| {
                    ast[c]
                        .new_keyword
                        .or(ast[c].factory_keyword)
                        .map(|t| ast.tokens.get(t).offset)
                });
            let Some(start) = start else {
                return;
            };
            let end = match ast[c].name {
                Some(name) => ast.tokens.get(name).end(),
                None => match ast[c].type_name {
                    Some(t) => ast.offset(t.raw()) + ast.length(t.raw()),
                    None => {
                        let t = ast[c].new_keyword.or(ast[c].factory_keyword);
                        t.map(|t| ast.tokens.get(t).end()).unwrap_or(start)
                    }
                },
            };
            locatable.at_offset(start as usize, (end - start) as usize)
        } else if let Some(b) = ast.cast::<BlockFunctionBody>(error_node) {
            let block = ast[b].block;
            self.at_token(locatable, ast[block].left_bracket)
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(error_node) {
            self.at_token(locatable, ast[f].name)
        } else if let Some(m) = ast.cast::<MethodDeclaration>(error_node) {
            self.at_token(locatable, ast[m].name)
        } else {
            self.at(locatable, error_node)
        };
        self.report(d);
    }

    /// Dart `_checkForFutureCatchErrorOnError(errorNode)`: "body might
    /// complete normally" in a `Future.catchError`'s `onError` callback.
    fn check_for_future_catch_error_on_error(&mut self, error_node: Id<BlockFunctionBody>) {
        let ast = &*self.ast;
        let Some(parent) = ast
            .parent(error_node.raw())
            .and_then(|p| ast.parent(p))
            .and_then(|p| ast.cast::<ArgumentList>(p))
        else {
            return;
        };
        let Some(invocation) = ast
            .parent(parent.raw())
            .and_then(|p| ast.cast::<MethodInvocation>(p))
        else {
            return;
        };
        let target_type = crate::ast_ext::method_invocation_real_target(ast, invocation)
            .and_then(|t| self.static_type(t));
        let method_name = ast[invocation].method_name;
        if crate::ast_ext::identifier_name(ast, method_name) != "catchError" {
            return;
        }
        let ctx = self.ctx;
        let Some(target_type) = target_type else {
            return;
        };
        if !matches!(ctx.ty(target_type), TypeKind::Interface { .. }) {
            return;
        }
        let Some(instance_of_future) = ctx.as_instance_of(target_type, ctx.tp.future_element().upcast())
        else {
            return;
        };
        let Some(&target_future_type) = ctx.type_arguments(instance_of_future).first() else {
            return;
        };
        let expected_return_type = ctx.tp.future_or_type(&ctx, target_future_type);
        let return_type_base = self.type_system.future_or_base(expected_return_type);
        if matches!(
            ctx.ty(return_type_base),
            TypeKind::Dynamic | TypeKind::Unknown | TypeKind::Void
        ) || ctx.is_dart_core_null(return_type_base)
        {
            return;
        }
        let block = ast[error_node].block;
        let d = self.at_token(
            diag::body_might_complete_normally_catch_error(type_arg(&ctx, return_type_base)),
            ast[block].left_bracket,
        );
        self.report(d);
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
