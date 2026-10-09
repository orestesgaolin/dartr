// Dart source: pkg/linter/lib/src/rules/avoid_bool_literals_in_conditional_expressions.dart

use super::helpers::unparenthesized;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ConditionalExpression, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ConditionalExpression,
        "avoid_bool_literals_in_conditional_expressions",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let Some(ts) = c.type_system() else { return };
    let n = &c.ast[Id::<ConditionalExpression>::from_raw(node)];
    let then_ = unparenthesized(c.ast, n.then_expression);
    let else_ = unparenthesized(c.ast, n.else_expression);
    if !c
        .static_type(then_)
        .is_some_and(|t| ts.dart_eq(t, r.ctx.tp.bool_type()))
        || !c
            .static_type(else_)
            .is_some_and(|t| ts.dart_eq(t, r.ctx.tp.bool_type()))
    {
        return;
    }
    for expression in [then_, else_] {
        if c.ast.kind(expression) == NodeKind::BooleanLiteral {
            c.report_node(
                out,
                &diag::AVOID_BOOL_LITERALS_IN_CONDITIONAL_EXPRESSIONS,
                node,
                &[],
            );
        }
    }
}
