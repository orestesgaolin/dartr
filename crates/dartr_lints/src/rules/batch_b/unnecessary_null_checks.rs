// Dart source: pkg/linter/lib/src/rules/unnecessary_null_checks.dart
use super::util::*;
use crate::rules::batch_a3::null_check_on_nullable_type_parameter::get_expected_type;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::NullAssertPattern,
        "unnecessary_null_checks",
        pattern,
    );
    r.add(
        NodeKind::PostfixExpression,
        "unnecessary_null_checks",
        postfix,
    );
}

fn pattern(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(resolved), Some(ts)) = (c.resolved, c.type_system()) else {
        return;
    };
    let Some(expected) = resolved
        .tables
        .pattern_info
        .get(node)
        .and_then(|i| i.matched_value_type)
    else {
        return;
    };
    if !ts.is_nullable(expected) {
        return;
    }
    for parent in ancestors(c, node) {
        match kind(c, parent) {
            NodeKind::PatternVariableDeclaration
            | NodeKind::ForEachPartsWithPattern
            | NodeKind::GuardedPattern => {
                return;
            }
            NodeKind::PatternAssignment => break,
            _ => {}
        }
    }
    let operator = c.ast[Id::<NullAssertPattern>::from_raw(node)].operator;
    c.report_token(out, &diag::UNNECESSARY_NULL_CHECKS, operator, &[]);
}

fn postfix(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ts) = c.type_system() else { return };
    let operator = c.ast[Id::<PostfixExpression>::from_raw(node)].operator;
    if lexeme(c, operator) != "!" {
        return;
    }
    if get_expected_type(c, node, false).is_some_and(|t| ts.is_nullable(t)) {
        c.report_token(out, &diag::UNNECESSARY_NULL_CHECKS, operator, &[]);
    }
}
