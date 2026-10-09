// Dart source: pkg/analyzer/lib/src/dart/resolver/yield_statement_resolver.dart

//! `YieldStatementResolver`: a `yield` / `yield*` statement.
//!
//! This file is the example of a fully ported resolver file in the crate
//! `README.md`: it resolves its expression through the shared type
//! analyzer (`analyzeYieldStatement`) or the core
//! ([`ResolverVisitor::resolve_expression`]), reads the body context,
//! reports diagnostics, and keeps the Dart method names.

use dartr_ast::{Expression, Id, MethodInvocation, YieldStatement};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{TypeId, TypeKind};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::TypeExt;

use crate::body_inference_context::BodyInferenceContext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitYieldStatement(node)` (after
/// `checkUnreachableNode`) = `YieldStatementResolver.resolve(node)`.
pub fn visit_yield_statement(rv: &mut ResolverVisitor<'_>, node: Id<YieldStatement>) {
    match rv.body_context() {
        Some(body_context) if body_context.is_generator => resolve_generator(rv, node),
        _ => resolve_not_generator(rv, node),
    }
}

/// Dart `_checkForUseOfVoidResult`: reports `useOfVoidResult` when the
/// expression has the type `void`.
fn check_for_use_of_void_result(rv: &mut ResolverVisitor<'_>, expression: Id<Expression>) -> bool {
    if !rv
        .static_type(expression)
        .is_some_and(|t| matches!(rv.ctx.ty(t), TypeKind::Void))
    {
        return false;
    }
    let d = match rv.ast.cast::<MethodInvocation>(expression) {
        Some(invocation) => {
            let method_name = rv.ast[invocation].method_name;
            rv.at(diag::use_of_void_result(), method_name)
        }
        None => rv.at(diag::use_of_void_result(), expression),
    };
    rv.report(d);
    true
}

/// Dart `_checkForYieldOfInvalidType`: a type mismatch between the yielded
/// type and the declared return type of a generator function.
fn check_for_yield_of_invalid_type(
    rv: &mut ResolverVisitor<'_>,
    body_context: &BodyInferenceContext,
    node: Id<YieldStatement>,
    is_yield_each: bool,
) {
    let ctx = rv.ctx;
    let tp = ctx.tp;
    let ts = rv.type_system;
    let strict_casts = rv.unit.options.strict_casts;
    let expression = rv.ast[node].expression;
    let expression_type = rv.type_or_throw(expression);

    let implied_return_type = if is_yield_each {
        expression_type
    } else if body_context.is_sync() {
        tp.iterable_type(&ctx, expression_type)
    } else {
        tp.stream_type(&ctx, expression_type)
    };

    if let Some(imposed_return_type) = body_context.imposed_type {
        let imposed_return_type = ts.union_free_type(imposed_return_type);
        if is_yield_each {
            if !ts.is_assignable_to(implied_return_type, imposed_return_type, strict_casts) {
                let d = diag::yield_each_of_invalid_type(
                    type_arg(&ctx, implied_return_type),
                    type_arg(&ctx, imposed_return_type),
                );
                let d = rv.at(d, expression);
                rv.report(d);
                return;
            }
        } else {
            let sequence_element = if body_context.is_sync() {
                tp.iterable_element()
            } else {
                tp.stream_element()
            };
            if let Some(imposed_sequence_type) =
                ctx.as_instance_of(imposed_return_type, sequence_element.upcast())
            {
                let imposed_value_type = ctx.type_arguments(imposed_sequence_type)[0];
                if !ts.is_assignable_to(expression_type, imposed_value_type, strict_casts) {
                    let d = diag::yield_of_invalid_type(
                        type_arg(&ctx, expression_type),
                        type_arg(&ctx, imposed_value_type),
                    );
                    let d = rv.at(d, expression);
                    rv.report(d);
                    return;
                }
            }
        }
    }

    if is_yield_each {
        // Since the declared return type might have been "dynamic", we need
        // to also check that the implied return type is assignable to
        // generic Iterable/Stream.
        let required_return_type: TypeId = if body_context.is_sync() {
            tp.iterable_dynamic_type()
        } else {
            tp.stream_dynamic_type()
        };
        if !ts.is_assignable_to(implied_return_type, required_return_type, strict_casts) {
            let d = diag::yield_each_of_invalid_type(
                type_arg(&ctx, implied_return_type),
                type_arg(&ctx, required_return_type),
            );
            let d = rv.at(d, expression);
            rv.report(d);
        }
    }
}

/// Dart `_resolve_generator`.
fn resolve_generator(rv: &mut ResolverVisitor<'_>, node: Id<YieldStatement>) {
    let expression = rv.ast[node].expression;
    let is_yield_star = rv.ast[node].star.is_some();
    rv.analyze_yield_statement(node.upcast(), expression, is_yield_star);
    // The expression may have been rewritten: read it again from the node.
    rv.pop_rewrite();
    let expression = rv.ast[node].expression;

    if is_yield_star {
        rv.nullable_dereference_expression(
            diag::unchecked_use_of_nullable_value_in_yield_each(),
            expression,
            None,
        );
    }

    // Dart `bodyContext.addYield(node)`.
    let expression_type = rv.type_or_throw(expression);
    let ts = rv.type_system;
    if let Some(body_context) = rv.body_context_mut() {
        body_context.add_yield(&ts, expression_type, is_yield_star);
    }
    let body_context = rv.body_context().cloned().expect("body context");

    check_for_yield_of_invalid_type(rv, &body_context, node, is_yield_star);
    check_for_use_of_void_result(rv, expression);
}

/// Dart `_resolve_notGenerator`.
fn resolve_not_generator(rv: &mut ResolverVisitor<'_>, node: Id<YieldStatement>) {
    let expression = rv.ast[node].expression;
    let expression = rv.resolve_expression(expression, TypeId::UNKNOWN);

    let d = if rv.ast[node].star.is_some() {
        diag::yield_each_in_non_generator()
    } else {
        diag::yield_in_non_generator()
    };
    let d = rv.at(d, node);
    rv.report(d);

    check_for_use_of_void_result(rv, expression);
}
