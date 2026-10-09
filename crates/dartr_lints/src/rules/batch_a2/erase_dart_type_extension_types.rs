// Dart source: pkg/linter/lib/src/rules/erase_dart_type_extension_types.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::IsExpression,
        "erase_dart_type_extension_types",
        check,
    );
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<IsExpression>(node).unwrap()];
    let Some(ty) = super::helpers::annotation_type(context, n.type_.raw()) else {
        return;
    };
    if super::helpers::implements(context, ty, "kernel.ast", "DartType") {
        context.report_node(out, &diag::ERASE_DART_TYPE_EXTENSION_TYPES, node, &[]);
    }
}
