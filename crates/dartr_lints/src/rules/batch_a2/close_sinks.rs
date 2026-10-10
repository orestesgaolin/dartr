// Dart source: pkg/linter/lib/src/rules/close_sinks.dart
// Dart source: pkg/linter/lib/src/util/leak_detector_visitor.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::FieldDeclaration, "close_sinks", check);
    registry.add(
        NodeKind::PrimaryConstructorDeclaration,
        "close_sinks",
        check,
    );
    registry.add(NodeKind::VariableDeclarationStatement, "close_sinks", check);
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    super::leak_detector::check_declaration(
        context,
        node,
        out,
        &diag::CLOSE_SINKS,
        &[
            super::leak_detector::LeakKind {
                library: "dart.core",
                interface: "Sink",
                method: "close",
            },
            super::leak_detector::LeakKind {
                library: "dart.io",
                interface: "Socket",
                method: "destroy",
            },
        ],
    );
}
