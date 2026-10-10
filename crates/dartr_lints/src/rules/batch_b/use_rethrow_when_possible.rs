// Dart source: pkg/linter/lib/src/rules/use_rethrow_when_possible.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ThrowExpression, "use_rethrow_when_possible", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.ast.parent(node).is_none_or(|p| kind(c, p) != NodeKind::ExpressionStatement) {
        return;
    }
    let expression = c.ast[Id::<ThrowExpression>::from_raw(node)].expression.raw();
    let Some(element) = canonical_element(c, expression) else {
        return;
    };
    let exception_parameter = this_or_ancestor_kind(c, node, NodeKind::CatchClause)
        .and_then(|clause| c.ast[Id::<CatchClause>::from_raw(clause)].exception_parameter)
        .and_then(|p| c.declared_element(p));
    if Some(base(c, element)) == exception_parameter {
        c.report_node(out, &diag::USE_RETHROW_WHEN_POSSIBLE, node, &[]);
    }
}
