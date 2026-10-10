// Dart source: pkg/linter/lib/src/rules/remove_deprecations_in_breaking_version.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if c.has_breaking_version() {
        r.add(
            NodeKind::Annotation,
            "remove_deprecations_in_breaking_versions",
            check,
        );
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<Annotation>::from_raw(node)];
    // Dart `node.elementAnnotation?.isDeprecated`.
    let deprecated = c
        .element(node)
        .is_some_and(|e| is_deprecated_annotation_element(c, base(c, e)));
    if deprecated {
        c.report_node(
            out,
            &diag::REMOVE_DEPRECATIONS_IN_BREAKING_VERSIONS,
            n.name,
            &[],
        );
    }
}
