// Dart source: pkg/linter/lib/src/rules/sized_box_shrink_expand.dart
use super::flutter::*;
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::InstanceCreationExpression, "sized_box_shrink_expand", check);
}

/// Dart `argumentValue`.
fn argument_value(c: &LinterContext<'_>, e: NodeId) -> Option<f64> {
    match kind(c, e) {
        NodeKind::IntegerLiteral => c.ast[Id::<IntegerLiteral>::from_raw(e)].value.map(|v| v as f64),
        NodeKind::DoubleLiteral => Some(c.ast[Id::<DoubleLiteral>::from_raw(e)].value),
        NodeKind::PrefixedIdentifier => {
            let p = &c.ast[Id::<PrefixedIdentifier>::from_raw(e)];
            (simple_name(c, p.identifier) == "infinity" && simple_name(c, p.prefix) == "double")
                .then_some(f64::INFINITY)
        }
        _ => None,
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    if !is_sized_box(c, c.static_type(node)) || c.ast[n.constructor_name].name.is_some() {
        return;
    }
    let (mut height, mut width) = (None, None);
    for argument in arguments(c, n.argument_list) {
        match named_argument_name(c, argument) {
            None => return,
            Some("width") => width = argument_value(c, argument_expression(c, argument)),
            Some("height") => height = argument_value(c, argument_expression(c, argument)),
            Some(_) => {}
        }
    }
    if width == Some(0.0) && height == Some(0.0) {
        c.report_node(out, &diag::SIZED_BOX_SHRINK_EXPAND, n.constructor_name, &["shrink"]);
    } else if width == Some(f64::INFINITY) && height == Some(f64::INFINITY) {
        c.report_node(out, &diag::SIZED_BOX_SHRINK_EXPAND, n.constructor_name, &["expand"]);
    }
}
