// Dart source: pkg/linter/lib/src/rules/use_truncating_division.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::BinaryExpression, "use_truncating_division", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    if lexeme(c, n.operator) != "/" {
        return;
    }
    let is_int = |e: Id<Expression>| c.static_type(e).is_some_and(|t| ctx.is_dart_core_int(t));
    if !is_int(n.left_operand) || !is_int(n.right_operand) {
        return;
    }
    let Some(method) = c.element(node) else { return };
    if library_uri(c, base(c, method)) != Some("dart:core") {
        return;
    }
    let Some(parent) = c.ast.parent(node) else { return };
    if kind(c, parent) != NodeKind::ParenthesizedExpression {
        return;
    }
    let outermost = this_or_ancestor(c, parent, |n| {
        c.ast
            .parent(n)
            .is_none_or(|p| kind(c, p) != NodeKind::ParenthesizedExpression)
    })
    .unwrap();
    let Some(grand_parent) = c.ast.parent(outermost) else {
        return;
    };
    if let Some(invocation) = c.ast.cast::<MethodInvocation>(grand_parent)
        && simple_name(c, c.ast[invocation].method_name) == "toInt"
        && c.ast
            .list_raw(c.ast[c.ast[invocation].argument_list].arguments)
            .is_empty()
    {
        c.report_node(out, &diag::USE_TRUNCATING_DIVISION, grand_parent, &[]);
    }
}
