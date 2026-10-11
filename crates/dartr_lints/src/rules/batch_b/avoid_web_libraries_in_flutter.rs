// Dart source: pkg/linter/lib/src/rules/avoid_web_libraries_in_flutter.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if c.has_flutter_dep() {
        r.add(
            NodeKind::ImportDirective,
            "avoid_web_libraries_in_flutter",
            check,
        );
    }
}

fn is_web_uri(uri: Option<&str>) -> bool {
    matches!(uri, Some("dart:html" | "dart:js" | "dart:js_util"))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let uri = c.ast[Id::<ImportDirective>::from_raw(node)].uri.raw();
    let uri_string = super::valid_regexps::string_value(c, uri);
    if is_web_uri(uri_string.as_deref()) {
        c.report_node(out, &diag::AVOID_WEB_LIBRARIES_IN_FLUTTER, node, &[]);
    }
}
