// Dart source: pkg/analyzer/lib/src/dart/resolver/binary_expression_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitBinaryExpression)

//! `BinaryExpressionResolver`: user-definable binary operators (the
//! operator method, the static invoke type, the numeric refinement of
//! `int + int`), `==` / `!=` (with flow analysis of null checks), `&&` /
//! `||` (with flow analysis), if-null `??`, and the recovery of unsupported
//! operators.

use dartr_ast::{
    BinaryExpression, Expression, ExtensionOverride, Id, NullLiteral, SimpleIdentifier,
    SuperExpression,
};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, InstanceElement, MethodElement, PromotableElement, TypeId, TypeKind,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::TypeExt;
use dartr_typesystem::lookup;
use dartr_typesystem::member;

use crate::method_invocation_resolver;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitBinaryExpression(node, contextType:)` (after
/// `checkUnreachableNode`) = `BinaryExpressionResolver.resolve`, then the
/// implicit call reference.
pub fn visit_binary_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<BinaryExpression>,
    context_type: TypeId,
) {
    resolve(rv, node, context_type);
    let e = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(e, context_type);
}

/// Dart `BinaryExpressionResolver.resolve(node, contextType:)`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>, context_type: TypeId) {
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);

    if operator == TokenType::AMPERSAND_AMPERSAND {
        resolve_logical_binary(rv, node, true);
        return;
    }

    if operator == TokenType::BANG_EQ || operator == TokenType::EQ_EQ {
        resolve_equal(rv, node, operator == TokenType::BANG_EQ);
        return;
    }

    if operator == TokenType::BAR_BAR {
        resolve_logical_binary(rv, node, false);
        return;
    }

    if operator == TokenType::QUESTION_QUESTION {
        resolve_if_null(rv, node, context_type);
        return;
    }

    if operator.is_user_definable_operator() && operator.is_binary_operator() {
        resolve_user_definable(rv, node, context_type);
        return;
    }

    // Report an error if not already reported by the parser.
    if operator != TokenType::BANG_EQ_EQ && operator != TokenType::EQ_EQ_EQ {
        let token = rv.ast[node].operator;
        let d = rv.at_token(diag::not_binary_operator(operator.lexeme()), token);
        rv.report(d);
    }

    resolve_unsupported_operator(rv, node);
}

/// The static type of [e], `dynamic` if it has none (Dart
/// `e.typeOrThrow`, without the throw).
fn type_of(rv: &ResolverVisitor<'_>, e: Id<Expression>) -> TypeId {
    rv.static_type(e).unwrap_or(TypeId::DYNAMIC)
}

/// Dart `_checkNonBoolOperand(operand, operator, whyNotPromoted:)` =
/// `boolExpressionVerifier.checkForNonBoolExpression(operand,
/// locatableDiagnostic: diag.nonBoolOperand, ...)` (wave D hook).
fn check_non_bool_operand(rv: &mut ResolverVisitor<'_>, operand: Id<Expression>, operator: &str) {
    crate::error::bool_expression_verifier::check_for_non_bool_expression(
        rv,
        operand,
        dartr_diagnostics::diag::non_bool_operand(operator),
    );
}

/// Dart `_resolveEqual(node, notEqual:)`.
fn resolve_equal(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>, not_equal: bool) {
    let left = rv.ast[node].left_operand;
    let left = rv.resolve_expression(left, TypeId::UNKNOWN);

    let left_extension_override = rv.ast.is::<ExtensionOverride>(left);
    let left_info = if left_extension_override {
        None
    } else {
        rv.flow_analysis.get_expression_info(Some(left))
    };

    // When evaluating exactly a dot shorthand in the RHS, we save the LHS
    // type to provide the context type for the shorthand.
    let right = rv.ast[node].right_operand;
    if rv.is_dot_shorthand(right) {
        let left_type = type_of(rv, left);
        rv.push_dot_shorthand_context(right.raw(), SharedTypeSchemaView::new(left_type));
    }

    let right = rv.resolve_expression(right, TypeId::UNKNOWN);
    // Dart `flow?.whyNotPromoted(...)`: not ported yet.

    if !left_extension_override && rv.flow_analysis.flow.is_some() {
        let left_type = type_of(rv, left);
        let right_type = type_of(rv, right);
        let right_info = rv.flow_analysis.get_expression_info(Some(right));
        let info = rv.flow().equality_operation_end(
            left_info,
            SharedTypeView::new(left_type),
            right_info,
            SharedTypeView::new(right_type),
            not_equal,
        );
        rv.flow_analysis.store_expression_info(node.upcast(), info);
    }

    resolve_user_definable_element(rv, node, "==", true);
    resolve_user_definable_type(rv, node);
    // Dart `checkForArgumentTypeNotAssignableForArgument(node.rightOperand,
    // promoteParameterToNullable: true, whyNotPromoted:)` (wave D).

    let operator = rv.ast[node].operator;
    let report_null_comparison = |rv: &mut ResolverVisitor<'_>, start: usize, end: usize| {
        let d = if not_equal {
            diag::unnecessary_null_comparison_always_null_false()
        } else {
            diag::unnecessary_null_comparison_always_null_true()
        };
        rv.report(d.at_offset(start, end - start));
    };
    let operator_offset = rv.ast.tokens.get(operator).offset as usize;
    let operator_end = rv.ast.tokens.get(operator).end() as usize;

    if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(left)
        && rv.ast.is::<NullLiteral>(right)
    {
        if is_definitely_unassigned(rv, identifier) {
            let start = rv.ast.offset(left) as usize;
            report_null_comparison(rv, start, operator_end);
        }
    } else if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(right)
        && rv.ast.is::<NullLiteral>(left)
        && is_definitely_unassigned(rv, identifier)
    {
        let end = rv.ast.end(right) as usize;
        report_null_comparison(rv, operator_offset, end);
    }
}

/// Whether the element of [identifier] is a promotable element that is
/// definitely unassigned (Dart `element is PromotableElementImpl &&
/// flowAnalysis.isDefinitelyUnassigned(identifier, element)`).
fn is_definitely_unassigned(
    rv: &mut ResolverVisitor<'_>,
    identifier: Id<SimpleIdentifier>,
) -> bool {
    if rv.flow_analysis.flow.is_none() {
        return false;
    }
    match rv.element(identifier) {
        Some(ElemRef::Base(e)) => match e.cast::<PromotableElement>() {
            Some(p) => rv.flow_analysis.is_definitely_unassigned(p),
            None => false,
        },
        _ => false,
    }
}

/// Dart `_resolveIfNull(node, contextType:)`.
fn resolve_if_null(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>, context_type: TypeId) {
    let ts = rv.type_system;
    let ctx = rv.ctx;
    let flow_active = rv.flow_analysis.flow.is_some();

    // An if-null expression `E` of the form `e1 ?? e2` with context type `K`
    // is analyzed as follows:
    //
    // - Let `T1` be the type of `e1` inferred with context type `K?`.
    let left = rv.ast[node].left_operand;
    let left = rv.resolve_expression(left, ts.make_nullable(context_type));
    let t1 = type_of(rv, left);

    // - Let `T2` be the type of `e2` inferred with context type `J`, where:
    //   - If `K` is `_`, `J = T1`.
    //   - Otherwise, `J = K`.
    let j = match ctx.ty(context_type) {
        TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Unknown => t1,
        _ => context_type,
    };
    if flow_active {
        let info = rv.flow_analysis.get_expression_info(Some(left));
        rv.flow()
            .if_null_expression_right_begin(info, SharedTypeView::new(t1));
    }
    let right = rv.ast[node].right_operand;
    let right = rv.resolve_expression(right, j);
    if flow_active {
        rv.flow().if_null_expression_end();
    }
    let t2 = type_of(rv, right);

    // - Let `T` be `UP(NonNull(T1), T2)`.
    let non_null_t1 = ts.promote_to_non_null(t1);
    let t = ts.least_upper_bound(non_null_t1, t2);

    // - Let `S` be the greatest closure of `K`.
    let s = ts.greatest_closure_of_schema(context_type);

    // If `inferenceUpdate3` is not enabled, then the type of `E` is `T`.
    let static_type = if !rv.is_enabled(ExperimentalFlag::InferenceUpdate3) {
        t
    } else if ts.is_subtype_of(t, s) {
        // - If `T <: S`, then the type of `E` is `T`.
        t
    } else if ts.is_subtype_of(non_null_t1, s) && ts.is_subtype_of(t2, s) {
        // - Otherwise, if `NonNull(T1) <: S` and `T2 <: S`, then the type
        //   of `E` is `S`.
        s
    } else {
        // - Otherwise, the type of `E` is `T`.
        t
    };

    rv.record_static_type(node, static_type);
    // Dart `checkForArgumentTypeNotAssignableForArgument(right)` (wave D).
}

/// Dart `_resolveLogicalAnd(node)` ([is_and]) and `_resolveLogicalOr(node)`.
fn resolve_logical_binary(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>, is_and: bool) {
    let flow_active = rv.flow_analysis.flow.is_some();
    let bool_type = rv.ctx.tp.bool_type();

    if flow_active {
        rv.flow().logical_binary_op_begin();
    }
    let left = rv.ast[node].left_operand;
    let left = rv.resolve_expression(left, bool_type);
    // Dart `flow?.whyNotPromoted(...)`: not ported yet.

    if flow_active {
        let info = rv.flow_analysis.get_expression_info(Some(left));
        rv.flow()
            .logical_binary_op_right_begin(info, node.raw(), is_and);
    }
    let right = rv.ast[node].right_operand;
    rv.check_unreachable_node(right);

    let right = rv.resolve_expression(right, bool_type);

    crate::error::dead_code_verifier::flow_end(rv, right);
    let info = if flow_active {
        let right_info = rv.flow_analysis.get_expression_info(Some(right));
        Some(rv.flow().logical_binary_op_end(right_info, is_and))
    } else {
        None
    };
    rv.flow_analysis.store_expression_info(node.upcast(), info);

    let operator = if is_and { "&&" } else { "||" };
    check_non_bool_operand(rv, left, operator);
    check_non_bool_operand(rv, right, operator);

    rv.record_static_type(node, bool_type);
}

/// Dart `_resolveRightOperand(node, contextType)`.
fn resolve_right_operand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<BinaryExpression>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let left = rv.ast[node].left_operand;

    let invoke_type = rv.tables.invoke_type.get(node).copied();
    let first_parameter_type = invoke_type.and_then(|t| match ctx.ty(t) {
        TypeKind::Function(f) => ctx.list(f.params).first().map(|p| p.ty),
        _ => None,
    });
    let right_context_type = match first_parameter_type {
        // If this is a user-defined operator, set the right operand context
        // using the operator method's parameter type.
        Some(parameter_type) => {
            let method = rv
                .element(node)
                .and_then(|e| member::base_element(&ctx, e).cast::<MethodElement>());
            let left_type = rv.static_type(left);
            rv.type_system.refine_numeric_invocation_context(
                left_type,
                method,
                context_type,
                parameter_type,
            )
        }
        None => TypeId::UNKNOWN,
    };

    let right = rv.ast[node].right_operand;
    rv.resolve_expression(right, right_context_type);
    // Dart `flow?.whyNotPromoted(...)`: not ported yet.

    resolve_user_definable_type(rv, node);
    // Dart `checkForArgumentTypeNotAssignableForArgument(right,
    // whyNotPromoted:)` (wave D).
}

/// Dart `_resolveUnsupportedOperator(node)`.
fn resolve_unsupported_operator(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>) {
    let left = rv.ast[node].left_operand;
    rv.resolve_expression(left, TypeId::UNKNOWN);
    let right = rv.ast[node].right_operand;
    rv.resolve_expression(right, TypeId::UNKNOWN);
    rv.record_static_type(node, TypeId::INVALID);
}

/// Dart `_resolveUserDefinable(node, contextType:)`.
fn resolve_user_definable(
    rv: &mut ResolverVisitor<'_>,
    node: Id<BinaryExpression>,
    context_type: TypeId,
) {
    let left = rv.ast[node].left_operand;
    let left = rv.resolve_expression(left, TypeId::UNKNOWN);

    if let Some(super_) = rv.ast.cast::<SuperExpression>(left)
        && method_invocation_resolver::super_context_of(rv, super_)
            != method_invocation_resolver::SuperContext::Valid
    {
        let right = rv.ast[node].right_operand;
        rv.resolve_expression(right, TypeId::INVALID);
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }

    let operator = rv.ast[node].operator;
    let method_name = rv.lexeme(operator).to_string();
    resolve_user_definable_element(rv, node, &method_name, false);

    resolve_right_operand(rv, node, context_type);
}

/// Dart `_resolveUserDefinableElement(node, methodName,
/// promoteLeftTypeToNonNull:)`.
fn resolve_user_definable_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<BinaryExpression>,
    method_name: &str,
    promote_left_type_to_non_null: bool,
) {
    let ctx = rv.ctx;
    let left_operand = rv.ast[node].left_operand;
    let operator: TokenId = rv.ast[node].operator;

    if let Some(override_) = rv.ast.cast::<ExtensionOverride>(left_operand) {
        let extension = rv.base_element(override_);
        let member = extension
            .and_then(|e| e.cast::<InstanceElement>())
            .and_then(|e: EId<InstanceElement>| lookup::get_method(&ctx, e, method_name))
            .map(|m| ElemRef::Base(m.raw()));
        if member.is_none() {
            // Extension overrides can only be used with named extensions so
            // it is safe to assume `extension.name` is non-`null`.
            let extension_name = extension
                .and_then(|e| ctx.element_name(e))
                .unwrap_or("")
                .to_string();
            let d = rv.at_token(
                diag::undefined_extension_operator(method_name, &extension_name),
                operator,
            );
            rv.report(d);
        }
        rv.set_element(node, member);
        match member {
            Some(m) => {
                let t = member::type_(&ctx, m);
                rv.tables.invoke_type.insert(node, t);
            }
            None => {
                rv.tables.invoke_type.remove(node);
            }
        }
        return;
    }

    let mut left_type = type_of(rv, left_operand);

    if left_type == TypeId::NEVER {
        let d = rv.at(diag::receiver_of_type_never(), left_operand);
        rv.report(d);
        return;
    }

    if promote_left_type_to_non_null {
        left_type = rv.type_system.promote_to_non_null(left_type);
    }

    let result = type_property_resolver::resolve_at_token(
        rv,
        PropertyQuery {
            receiver: Some(left_operand),
            receiver_type: left_type,
            name: method_name,
            has_read: true,
            has_write: false,
            property_error_entity: node.raw(),
            name_error_entity: node.raw(),
            parent_node: None,
        },
        operator,
    );

    rv.set_element(node, result.getter);
    match result.getter {
        Some(g) => {
            let t = member::type_(&ctx, g);
            rv.tables.invoke_type.insert(node, t);
        }
        None => {
            rv.tables.invoke_type.remove(node);
        }
    }
    if result.needs_getter_error {
        let t = type_arg(&ctx, left_type);
        let d = if rv.ast.is::<SuperExpression>(left_operand) {
            diag::undefined_super_operator(method_name, t)
        } else {
            diag::undefined_operator(method_name, t)
        };
        let d = rv.at_token(d, operator);
        rv.report(d);
    }
}

/// Dart `_resolveUserDefinableType(node)`.
fn resolve_user_definable_type(rv: &mut ResolverVisitor<'_>, node: Id<BinaryExpression>) {
    let ctx = rv.ctx;
    let left_operand = rv.ast[node].left_operand;
    let is_extension_override = rv.ast.is::<ExtensionOverride>(left_operand);

    let left_type = if is_extension_override {
        rv.tables
            .extended_type
            .get(left_operand)
            .copied()
            .unwrap_or(TypeId::INVALID)
    } else {
        let t = type_of(rv, left_operand);
        rv.type_system.resolve_to_bound(t)
    };

    if left_type == TypeId::NEVER {
        rv.record_static_type(node, TypeId::NEVER);
        return;
    }

    let mut static_type = rv
        .tables
        .invoke_type
        .get(node)
        .and_then(|&t| match ctx.ty(t) {
            TypeKind::Function(f) => Some(f.ret),
            _ => None,
        });
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);
    let mut static_type = if operator == TokenType::EQ_EQ {
        ctx.tp.bool_type()
    } else if matches!(ctx.ty(left_type), TypeKind::Dynamic) {
        static_type.take().unwrap_or(TypeId::DYNAMIC)
    } else {
        static_type.take().unwrap_or(TypeId::INVALID)
    };
    if !is_extension_override {
        let right_type = type_of(rv, rv.ast[node].right_operand);
        let method = rv
            .element(node)
            .and_then(|e| member::base_element(&ctx, e).cast::<MethodElement>());
        static_type = rv.type_system.refine_binary_expression_type(
            left_type,
            right_type,
            static_type,
            method,
        );
    }
    rv.record_static_type(node, static_type);
}
