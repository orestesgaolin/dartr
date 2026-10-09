// Dart source: pkg/linter/lib/src/rules/implementation_imports.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{diag, Diagnostic};

pub fn register(registry: &mut RuleVisitorRegistry, context: &LinterContext<'_>) {
    if context.source_uri().starts_with("package:") {
        registry.add(NodeKind::ImportDirective, "implementation_imports", check);
    }
}
fn package(uri: &str) -> Option<&str> {
    uri.strip_prefix("package:")?.split('/').next()
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<ImportDirective>(node).unwrap()];
    let uri = match context.ast.kind(n.uri) {
        NodeKind::SimpleStringLiteral => context.ast
            [context.ast.cast::<SimpleStringLiteral>(n.uri).unwrap()]
        .value
        .as_ref(),
        _ => return,
    };
    if uri
        .strip_prefix("package:")
        .is_some_and(|rest| rest.split('/').nth(1) == Some("src"))
        && package(uri) != package(&context.source_uri())
    {
        context.report_node(out, &diag::IMPLEMENTATION_IMPORTS, n.uri, &[]);
    }
}
