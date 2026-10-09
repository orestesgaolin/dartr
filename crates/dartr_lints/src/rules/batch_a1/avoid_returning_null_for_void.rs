// Dart source: pkg/linter/lib/src/rules/avoid_returning_null_for_void.dart

use super::helpers::{ancestor, declared_type, is_future_void, is_null_literal, is_void};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    ExpressionFunctionBody, FunctionExpression, Id, MethodDeclaration, NodeId, NodeKind,
    ReturnStatement,
};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ExpressionFunctionBody,
        "avoid_returning_null_for_void",
        check,
    );
    registry.add(
        NodeKind::ReturnStatement,
        "avoid_returning_null_for_void",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = match c.ast.kind(node) {
        NodeKind::ExpressionFunctionBody => {
            c.ast[Id::<ExpressionFunctionBody>::from_raw(node)].expression
        }
        NodeKind::ReturnStatement => {
            match c.ast[Id::<ReturnStatement>::from_raw(node)].expression {
                Some(e) => e,
                None => return,
            }
        }
        _ => return,
    };
    if !is_null_literal(c.ast, expression) {
        return;
    }
    let (owner, code) = if let Some(method) = ancestor(c.ast, node, NodeKind::MethodDeclaration) {
        (method, &diag::AVOID_RETURNING_NULL_FOR_VOID_FROM_METHOD)
    } else if let Some(function) = ancestor(c.ast, node, NodeKind::FunctionExpression) {
        (function, &diag::AVOID_RETURNING_NULL_FOR_VOID_FROM_FUNCTION)
    } else {
        return;
    };
    let Some(ty) = declared_type(c, owner) else {
        return;
    };
    let body = if c.ast.kind(owner) == NodeKind::MethodDeclaration {
        c.ast[Id::<MethodDeclaration>::from_raw(owner)].body
    } else {
        c.ast[Id::<FunctionExpression>::from_raw(owner)].body
    };
    let asynchronous = match c.ast.kind(body) {
        NodeKind::BlockFunctionBody => c.ast
            [c.ast.cast::<dartr_ast::BlockFunctionBody>(body).unwrap()]
        .keyword
        .is_some(),
        NodeKind::ExpressionFunctionBody => c.ast
            [c.ast.cast::<ExpressionFunctionBody>(body).unwrap()]
        .keyword
        .is_some(),
        _ => false,
    };
    if (!asynchronous && is_void(c, ty)) || (asynchronous && is_future_void(c, ty)) {
        c.report_node(out, code, node, &[]);
    }
}
