// Dart source: pkg/analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitPrefixExpression)

//! `PrefixExpressionResolver`: `!e` (with flow analysis), `-e`, `~e`
//! (user-definable operators) and the increments `++e` / `--e` (the read
//! and write elements of the operand, the operator method, the flow
//! analysis write).

use dartr_ast::{
    Expression, ExtensionOverride, Id, IntegerLiteral, PrefixExpression, SimpleIdentifier,
    SuperExpression,
};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{ElemRef, InstanceElement, PromotableElement, TypeId, TypeKind};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;
use dartr_typesystem::lookup;
use dartr_typesystem::member;

use crate::assignment_expression_resolver::{
    check_final_already_assigned, resolve_for_write, set_read_element, set_write_element,
};
use crate::ast_ext::is_increment_operator;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitPrefixExpression(node, contextType:)` =
/// `checkUnreachableNode(node)`, `PrefixExpressionResolver.resolve`, then
/// the implicit call reference.
pub fn visit_prefix_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PrefixExpression>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    resolve(rv, node, context_type);
    let e = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(e, context_type);
}

/// Dart `PrefixExpressionResolver.resolve(node, contextType:)`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<PrefixExpression>, context_type: TypeId) {
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);

    if operator == TokenType::BANG {
        resolve_negation(rv, node);
        return;
    }

    let operand = rv.ast[node].operand;
    if is_increment_operator(operator) {
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

        check_final_already_assigned(rv, operand, false);
    } else {
        let inner_context_type =
            if operator == TokenType::MINUS && rv.ast.is::<IntegerLiteral>(operand) {
                // Negated integer literals should undergo int->double conversion
                // in the same circumstances as non-negated integer literals, so
                // pass the context type through.
                context_type
            } else {
                TypeId::UNKNOWN
            };
        rv.resolve_expression(operand, inner_context_type);
    }

    resolve1(rv, node);
    resolve2(rv, node);
}

/// Dart `_checkForInvalidAssignmentIncDec(node, type)`: the result [ty] of
/// a prefix or postfix `++` or `--` must be assignable to the write type of
/// the operand.
pub fn check_for_invalid_assignment_inc_dec(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    ty: TypeId,
) {
    let operand_write_type = rv
        .tables
        .write_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::INVALID);
    let strict_casts = rv.unit.options.strict_casts;
    if !rv
        .type_system
        .is_assignable_to(ty, operand_write_type, strict_casts)
    {
        let ctx = rv.ctx;
        let d = diag::invalid_assignment(type_arg(&ctx, ty), type_arg(&ctx, operand_write_type));
        let d = rv.at(d, node);
        rv.report(d);
    }
}

/// Dart `InvocationInferrer.computeInvokeReturnType(type)`.
pub fn compute_invoke_return_type(rv: &ResolverVisitor<'_>, ty: TypeId) -> TypeId {
    match rv.ctx.ty(ty) {
        TypeKind::Function(f) => f.ret,
        _ => TypeId::DYNAMIC,
    }
}

/// Dart `_computeStaticReturnType(element)` of the prefix and postfix
/// resolvers; [fallback] is the type for no executable element (`invalid`
/// for a prefix expression, `dynamic` for a postfix expression).
pub fn compute_static_return_type(
    rv: &ResolverVisitor<'_>,
    element: Option<ElemRef>,
    fallback: TypeId,
) -> TypeId {
    let ctx = rv.ctx;
    let Some(element) = element else {
        return fallback;
    };
    let base = member::base_element(&ctx, element);
    if base.is::<dartr_element::PropertyAccessorElement>() {
        // This is a function invocation expression disguised as something
        // else. We are invoking a getter and then invoking the returned
        // function.
        let property_type = member::type_(&ctx, element);
        let return_type = compute_invoke_return_type(rv, property_type);
        return compute_invoke_return_type(rv, return_type);
    } else if base.is::<dartr_element::ExecutableElement>() {
        return compute_invoke_return_type(rv, member::type_(&ctx, element));
    }
    fallback
}

/// Dart `_getPrefixOperator(expression)`: the name of the method invoked by
/// the prefix expression [node].
fn get_prefix_operator(rv: &ResolverVisitor<'_>, node: Id<PrefixExpression>) -> String {
    let operator = rv.ast[node].operator;
    let operator_type = rv.ast.tokens.ty(operator);
    if operator_type == TokenType::PLUS_PLUS {
        TokenType::PLUS.lexeme().to_string()
    } else if operator_type == TokenType::MINUS_MINUS {
        TokenType::MINUS.lexeme().to_string()
    } else if operator_type == TokenType::MINUS {
        "unary-".to_string()
    } else {
        rv.lexeme(operator).to_string()
    }
}

/// Dart `_resolve1(node)`.
fn resolve1(rv: &mut ResolverVisitor<'_>, node: Id<PrefixExpression>) {
    let ctx = rv.ctx;
    let operator = rv.ast[node].operator;
    let operator_type = rv.ast.tokens.ty(operator);
    if !(operator_type.is_user_definable_operator() || is_increment_operator(operator_type)) {
        return;
    }
    let operand = rv.ast[node].operand;
    let method_name = get_prefix_operator(rv, node);
    if let Some(override_) = rv.ast.cast::<ExtensionOverride>(operand) {
        let element = rv.base_element(override_);
        let member = element
            .and_then(|e| e.cast::<InstanceElement>())
            .and_then(|e| lookup::get_method(&ctx, e, &method_name))
            .map(|m| ElemRef::Base(m.raw()));
        if member.is_none() {
            // Extension overrides always refer to named extensions, so we
            // can safely assume `element.name` is non-`null`.
            let extension_name = element
                .and_then(|e| ctx.element_name(e))
                .unwrap_or("")
                .to_string();
            let d = rv.at_token(
                diag::undefined_extension_operator(&method_name, &extension_name),
                operator,
            );
            rv.report(d);
        }
        rv.set_element(node, member);
        return;
    }

    let read_type = rv
        .tables
        .read_type
        .get(node)
        .copied()
        .unwrap_or_else(|| rv.static_type(operand).unwrap_or(TypeId::DYNAMIC));
    if matches!(ctx.ty(read_type), TypeKind::Invalid) {
        return;
    }
    if read_type == TypeId::NEVER {
        let d = rv.at(diag::receiver_of_type_never(), operand);
        rv.report(d);
        return;
    }

    let result = type_property_resolver::resolve_at_token(
        rv,
        PropertyQuery {
            receiver: Some(operand),
            receiver_type: read_type,
            name: &method_name,
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
        let t = type_arg(&ctx, read_type);
        let d = if rv.ast.is::<SuperExpression>(operand) {
            diag::undefined_super_operator(&method_name, t)
        } else {
            diag::undefined_operator(&method_name, t)
        };
        let d = rv.at_token(d, operator);
        rv.report(d);
    }
}

/// Dart `_resolve2(node)`.
fn resolve2(rv: &mut ResolverVisitor<'_>, node: Id<PrefixExpression>) {
    let ctx = rv.ctx;
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);
    let operand = rv.ast[node].operand;
    let read_type = rv
        .tables
        .read_type
        .get(node)
        .copied()
        .or_else(|| rv.static_type(operand));
    if read_type == Some(TypeId::NEVER) {
        rv.record_static_type(node, TypeId::NEVER);
        return;
    }
    // The other cases are equivalent to invoking a method.
    let mut static_type = match read_type.map(|t| ctx.ty(t)) {
        Some(TypeKind::Dynamic) => TypeId::DYNAMIC,
        Some(TypeKind::Invalid) => TypeId::INVALID,
        _ => {
            let element = rv.element(node);
            compute_static_return_type(rv, element, TypeId::INVALID)
        }
    };
    if rv.ast.is::<ExtensionOverride>(operand) {
        // No special handling for incremental operators.
    } else if is_increment_operator(operator) {
        if read_type.is_some_and(|t| ctx.is_dart_core_int(t)) {
            static_type = ctx.tp.int_type();
        } else {
            check_for_invalid_assignment_inc_dec(rv, node.upcast(), static_type);
        }
        if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(operand)
            && let Some(ElemRef::Base(e)) = rv.element(identifier)
            && let Some(variable) = e.cast::<PromotableElement>()
        {
            let info = {
                let ast = &*rv.ast;
                rv.flow_analysis
                    .write(ast, node.raw(), variable, static_type, None)
            };
            rv.flow_analysis.store_expression_info(node.upcast(), info);
        }
    }
    rv.record_static_type(node, static_type);
}

/// Dart `_resolveNegation(node)`.
fn resolve_negation(rv: &mut ResolverVisitor<'_>, node: Id<PrefixExpression>) {
    let bool_type = rv.ctx.tp.bool_type();
    let operand = rv.ast[node].operand;
    let operand = rv.resolve_expression(operand, bool_type);
    // Dart `flow?.whyNotPromoted(...)` and
    // `boolExpressionVerifier.checkForNonBoolNegationExpression(operand,
    // whyNotPromoted:)` (wave D).

    rv.record_static_type(node, bool_type);

    if rv.flow_analysis.flow.is_some() {
        let info = rv.flow_analysis.get_expression_info(Some(operand));
        let result = rv.flow().logical_not_end(info);
        rv.flow_analysis
            .store_expression_info(node.upcast(), result);
    }
}
