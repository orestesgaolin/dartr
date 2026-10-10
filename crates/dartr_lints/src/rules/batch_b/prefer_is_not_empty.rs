// Dart source: pkg/linter/lib/src/rules/prefer_is_not_empty.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::PrefixExpression, "prefer_is_not_empty", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<PrefixExpression>::from_raw(node)];
    if lexeme(c, n.operator) != "!" {
        return;
    }
    let expression = unparenthesized(c, n.operand.raw());
    let identifier = if let Some(access) = c.ast.cast::<PropertyAccess>(expression) {
        c.ast[access].property_name
    } else if let Some(prefixed) = c.ast.cast::<PrefixedIdentifier>(expression) {
        c.ast[prefixed].identifier
    } else {
        return;
    };
    let Some(property) = c.element(identifier).map(|e| base(c, e)) else {
        return;
    };
    if name(c, property) != Some("isEmpty") {
        return;
    }
    let Some(target) = enclosing(c, property).and_then(|e| e.cast::<dartr_element::InterfaceElement>())
    else {
        return;
    };
    if dartr_typesystem::lookup::get_getter(&ctx, target.upcast(), "isNotEmpty").is_none() {
        return;
    }
    c.report_node(out, &diag::PREFER_IS_NOT_EMPTY, node, &[]);
}
