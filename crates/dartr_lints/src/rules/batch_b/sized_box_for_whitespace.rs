// Dart source: pkg/linter/lib/src/rules/sized_box_for_whitespace.dart
use super::container_rules::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::InstanceCreationExpression,
        "sized_box_for_whitespace",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    check_container(c, node, out, &diag::SIZED_BOX_FOR_WHITESPACE, sized_box);
}
