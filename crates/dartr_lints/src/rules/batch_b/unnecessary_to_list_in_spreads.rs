// Dart source: pkg/linter/lib/src/rules/unnecessary_to_list_in_spreads.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::SpreadElement, "unnecessary_to_list_in_spreads", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = c.ast[Id::<SpreadElement>::from_raw(node)].expression.raw();
    let Some(invocation) = c.ast.cast::<MethodInvocation>(expression) else {
        return;
    };
    let n = &c.ast[invocation];
    if simple_name(c, n.method_name) == "toList"
        && let Some(target) = n.target
        && implements_interface(c, c.static_type(target), "Iterable", "dart.core")
    {
        c.report_node(out, &diag::UNNECESSARY_TO_LIST_IN_SPREADS, n.method_name, &[]);
    }
}
