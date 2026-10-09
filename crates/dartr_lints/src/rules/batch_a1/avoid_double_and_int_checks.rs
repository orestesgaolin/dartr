// Dart source: pkg/linter/lib/src/rules/avoid_double_and_int_checks.dart

use super::helpers::{lexeme, node_type};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, IfStatement, IsExpression, NodeId, NodeKind, SimpleIdentifier};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::IfStatement, "avoid_double_and_int_checks", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<IfStatement>::from_raw(node)];
    let Some(else_if) = n.else_statement.and_then(|s| c.ast.cast::<IfStatement>(s)) else {
        return;
    };
    let Some(first) = c.ast.cast::<IsExpression>(n.expression) else {
        return;
    };
    let Some(second) = c.ast.cast::<IsExpression>(c.ast[else_if].expression) else {
        return;
    };
    let (Some(left), Some(right)) = (
        c.ast.cast::<SimpleIdentifier>(c.ast[first].expression),
        c.ast.cast::<SimpleIdentifier>(c.ast[second].expression),
    ) else {
        return;
    };
    if lexeme(c, c.ast[left].token) != lexeme(c, c.ast[right].token) {
        return;
    }
    if node_type(c, c.ast[first].type_).is_some_and(|t| r.ctx.is_dart_core_double(t))
        && node_type(c, c.ast[second].type_).is_some_and(|t| r.ctx.is_dart_core_int(t))
        && c.element(left).is_some()
        && c.element(left) == c.element(right)
    {
        c.report_node(out, &diag::AVOID_DOUBLE_AND_INT_CHECKS, second, &[]);
    }
}
