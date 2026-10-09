// Dart source: pkg/analyzer/lib/src/error/async_return_visitor.dart

use super::helpers::{ancestor, declared_type};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{BlockFunctionBody, ExpressionFunctionBody, Id, NodeId, NodeKind, ReturnStatement};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ReturnStatement,
        "async_return_with_no_await",
        check,
    );
    registry.add(
        NodeKind::ExpressionFunctionBody,
        "async_return_with_no_await",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let (expression, token) = match c.ast.kind(node) {
        NodeKind::ReturnStatement => {
            let n = &c.ast[Id::<ReturnStatement>::from_raw(node)];
            let Some(expression) = n.expression else {
                return;
            };
            (expression, n.return_keyword)
        }
        NodeKind::ExpressionFunctionBody => {
            let n = &c.ast[Id::<ExpressionFunctionBody>::from_raw(node)];
            (n.expression, n.function_definition)
        }
        _ => return,
    };
    let Some(body) = (c.ast.kind(node) == NodeKind::ExpressionFunctionBody)
        .then_some(node)
        .or_else(|| ancestor(c.ast, node, NodeKind::BlockFunctionBody))
        .or_else(|| ancestor(c.ast, node, NodeKind::ExpressionFunctionBody))
    else {
        return;
    };
    let (asynchronous, generator) = match c.ast.kind(body) {
        NodeKind::BlockFunctionBody => {
            let body = &c.ast[Id::<BlockFunctionBody>::from_raw(body)];
            (body.keyword.is_some(), body.star.is_some())
        }
        NodeKind::ExpressionFunctionBody => (
            c.ast[Id::<ExpressionFunctionBody>::from_raw(body)]
                .keyword
                .is_some(),
            false,
        ),
        _ => (false, false),
    };
    if !asynchronous || generator {
        return;
    }
    if c.ast.kind(node) == NodeKind::ReturnStatement {
        let mut child = node;
        while let Some(parent) = c.ast.parent(child) {
            if parent == body {
                break;
            }
            if let Some(try_) = c.ast.cast::<dartr_ast::TryStatement>(parent)
                && c.ast[try_].body.raw() == child
            {
                return;
            }
            child = parent;
        }
    }
    let Some(expression_type) = c.static_type(expression) else {
        return;
    };
    if expression_type == dartr_element::TypeId::DYNAMIC
        || !(r.ctx.is_dart_async_future(expression_type)
            || r.ctx.is_dart_async_future_or(expression_type))
    {
        return;
    }
    let owner = c.ast.parent(body).unwrap_or(body);
    let Some(return_type) = declared_type(c, owner) else {
        return;
    };
    let expected = r.ctx.tp.future_or_type(
        &r.ctx,
        c.type_system().unwrap().future_value_type(return_type),
    );
    if c.type_system()
        .is_some_and(|ts| ts.is_assignable_to(expression_type, expected, false))
    {
        c.report_token(out, &diag::ASYNC_RETURN_WITH_NO_AWAIT, token, &[]);
    }
}
