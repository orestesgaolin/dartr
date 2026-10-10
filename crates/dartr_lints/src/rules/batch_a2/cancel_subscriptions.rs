// Dart source: pkg/linter/lib/src/rules/cancel_subscriptions.dart
// Dart source: pkg/linter/lib/src/util/leak_detector_visitor.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::FieldDeclaration, "cancel_subscriptions", check);
    registry.add(
        NodeKind::PrimaryConstructorDeclaration,
        "cancel_subscriptions",
        check,
    );
    registry.add(
        NodeKind::VariableDeclarationStatement,
        "cancel_subscriptions",
        check,
    );
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    super::leak_detector::check_declaration(
        context,
        node,
        out,
        &diag::CANCEL_SUBSCRIPTIONS,
        &[super::leak_detector::LeakKind {
            library: "dart.async",
            interface: "StreamSubscription",
            method: "cancel",
        }],
    );
}
