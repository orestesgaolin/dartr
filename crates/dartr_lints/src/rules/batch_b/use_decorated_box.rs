// Dart source: pkg/linter/lib/src/rules/use_decorated_box.dart
use super::container_rules::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::InstanceCreationExpression,
        "use_decorated_box",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    check_container(c, node, out, &diag::USE_DECORATED_BOX, decorated_box);
}
