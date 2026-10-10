// Dart source: pkg/linter/lib/src/rules/provide_deprecation_message.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::Annotation, "provide_deprecation_message", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<Annotation>::from_raw(node)];
    // Dart `node.elementAnnotation?.isDeprecated`.
    let deprecated = c
        .element(node)
        .is_some_and(|e| is_deprecated_annotation_element(c, base(c, e)));
    if deprecated && n.arguments.is_none() {
        c.report_node(out, &diag::PROVIDE_DEPRECATION_MESSAGE, node, &[]);
    }
}
