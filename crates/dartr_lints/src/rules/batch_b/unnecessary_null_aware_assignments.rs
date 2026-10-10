// Dart source: pkg/linter/lib/src/rules/unnecessary_null_aware_assignments.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::Tag;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::AssignmentExpression,
        "unnecessary_null_aware_assignments",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    if resolved
        .tables
        .write_element
        .get(node)
        .is_some_and(|&e| tag(c, e) == Tag::Setter)
    {
        return;
    }
    let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
    if lexeme(c, n.operator) == "??=" && is_null_literal(c, n.right_hand_side.raw()) {
        c.report_node(out, &diag::UNNECESSARY_NULL_AWARE_ASSIGNMENTS, node, &[]);
    }
}
