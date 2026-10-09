// Dart source: pkg/analyzer/lib/src/dart/resolver/postfix_expression_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitPostfixExpression)

//! `PostfixExpressionResolver`: the increments `e++` / `e--` (the read and
//! write elements of the operand, the operator method, the flow analysis
//! write) and the null check `e!`.

use dartr_ast::{Id, PostfixExpression, SimpleIdentifier, SuperExpression};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{ElemRef, PromotableElement, TypeId};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeSchemaView;
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;

use crate::assignment_expression_resolver::{
    check_final_already_assigned, resolve_for_write, set_read_element, set_write_element,
};
use crate::prefix_expression_resolver::{
    check_for_invalid_assignment_inc_dec, compute_static_return_type,
};
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitPostfixExpression(node, contextType:)` =
/// `checkUnreachableNode(node)`, `PostfixExpressionResolver.resolve`, then
/// the implicit call reference.
pub fn visit_postfix_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PostfixExpression>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.is_dot_shorthand(node.upcast());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SharedTypeSchemaView::new(context_type));
    }

    rv.check_unreachable_node(node);
    resolve(rv, node, context_type);
    let e = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(e, context_type);

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `PostfixExpressionResolver.resolve(node, contextType:)`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<PostfixExpression>, context_type: TypeId) {
    if rv.ast.tokens.ty(rv.ast[node].operator) == TokenType::BANG {
        resolve_null_check(rv, node, context_type);
        return;
    }

    let operand = rv.ast[node].operand;
    let operand_resolution = resolve_for_write(rv, operand, true);

    let read_element = operand_resolution.read_element();
    let write_element = operand_resolution.write_element();

    let operand = rv.ast[node].operand;
    set_read_element(
        rv,
        operand,
        read_element,
        operand_resolution.at_dynamic_target,
    );
    set_write_element(
        rv,
        operand,
        write_element,
        operand_resolution.at_dynamic_target,
    );

    // TODO(scheglov): Use VariableElement and do in resolveForWrite() ?
    check_final_already_assigned(rv, operand, false);

    let receiver_type = rv
        .tables
        .read_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::INVALID);
    resolve1(rv, node, receiver_type);
    resolve2(rv, node, receiver_type);
}

/// Dart `_getPostfixOperator(expression)`: the name of the method invoked
/// by the postfix expression.
fn get_postfix_operator(rv: &ResolverVisitor<'_>, node: Id<PostfixExpression>) -> &'static str {
    match rv.ast.tokens.ty(rv.ast[node].operator) {
        TokenType::PLUS_PLUS => TokenType::PLUS.lexeme(),
        // Dart throws `UnsupportedError` for other operators; the parser
        // only creates `++`, `--` and `!`.
        _ => TokenType::MINUS.lexeme(),
    }
}

/// Dart `_resolve1(node, receiverType)`.
fn resolve1(rv: &mut ResolverVisitor<'_>, node: Id<PostfixExpression>, receiver_type: TypeId) {
    let operand = rv.ast[node].operand;

    if receiver_type == TypeId::NEVER {
        let d = rv.at(diag::receiver_of_type_never(), operand);
        rv.report(d);
        return;
    }

    let method_name = get_postfix_operator(rv, node);
    let operator = rv.ast[node].operator;
    let result = type_property_resolver::resolve_at_token(
        rv,
        PropertyQuery {
            receiver: Some(operand),
            receiver_type,
            name: method_name,
            has_read: true,
            has_write: false,
            property_error_entity: node.raw(),
            name_error_entity: operand.raw(),
            parent_node: None,
        },
        operator,
    );
    rv.set_element(node, result.getter);
    if result.needs_getter_error {
        let t = type_arg(&rv.ctx, receiver_type);
        let d = if rv.ast.is::<SuperExpression>(operand) {
            diag::undefined_super_operator(method_name, t)
        } else {
            diag::undefined_operator(method_name, t)
        };
        let d = rv.at_token(d, operator);
        rv.report(d);
    }
}

/// Dart `_resolve2(node, receiverType)`.
fn resolve2(rv: &mut ResolverVisitor<'_>, node: Id<PostfixExpression>, receiver_type: TypeId) {
    let operand = rv.ast[node].operand;

    if receiver_type == TypeId::NEVER {
        rv.record_static_type(node, TypeId::NEVER);
        return;
    }
    let operator_return_type = if rv.ctx.is_dart_core_int(receiver_type) {
        // No need to check for `intVar++`, the result is `int`.
        receiver_type
    } else {
        let operator_element = rv.element(node);
        let t = compute_static_return_type(rv, operator_element, TypeId::DYNAMIC);
        check_for_invalid_assignment_inc_dec(rv, node.upcast(), t);
        t
    };
    if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(operand)
        && let Some(ElemRef::Base(e)) = rv.element(identifier)
        && let Some(variable) = e.cast::<PromotableElement>()
    {
        let ast = &*rv.ast;
        rv.flow_analysis
            .post_inc_dec(ast, node.raw(), variable, operator_return_type);
    }
    rv.record_static_type(node, receiver_type);
}

/// Dart `_resolveNullCheck(node, contextType:)`.
fn resolve_null_check(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PostfixExpression>,
    context_type: TypeId,
) {
    let operand = rv.ast[node].operand;

    if rv.ast.is::<SuperExpression>(operand) {
        let d = rv.at(diag::missing_assignable_selector(), node);
        rv.report(d);
        rv.set_static_type(operand, TypeId::DYNAMIC);
        rv.record_static_type(node, TypeId::DYNAMIC);
        return;
    }

    let ts = rv.type_system;
    rv.analyze_expression(
        operand,
        SharedTypeSchemaView::new(ts.make_nullable(context_type)),
        true,
        false,
        false,
    );
    let operand = rv.pop_rewrite().expect("rewritten operand");

    let operand_type = rv.static_type(operand).unwrap_or(TypeId::DYNAMIC);

    let ty = ts.promote_to_non_null(operand_type);
    rv.record_static_type(node, ty);

    if rv.flow_analysis.flow.is_some() {
        let info = rv.flow_analysis.get_expression_info(Some(operand));
        rv.flow().non_null_assert_end(info);
    }
}
