// Dart source: pkg/linter/lib/src/rules/implicit_call_tearoffs.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ImplicitCallReference,
        "implicit_call_tearoffs",
        check,
    );
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    context.report_node(out, &diag::IMPLICIT_CALL_TEAROFFS, node, &[]);
}
