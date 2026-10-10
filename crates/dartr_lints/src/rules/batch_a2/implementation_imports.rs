// Dart source: pkg/linter/lib/src/rules/implementation_imports.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{DirectiveUri, LibraryFragment};

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
    let Some(uri) = resolved_import_uri(context, node, n.import_keyword) else {
        return;
    };
    if uri
        .strip_prefix("package:")
        .is_some_and(|rest| rest.split('/').nth(1) == Some("src"))
        && package(&uri) != package(&context.source_uri())
    {
        context.report_node(out, &diag::IMPLEMENTATION_IMPORTS, n.uri, &[]);
    }
}

fn resolved_import_uri(
    context: &LinterContext<'_>,
    node: NodeId,
    import_keyword: dartr_syntax::TokenId,
) -> Option<String> {
    let resolved = context.resolved?;
    let unit = std::iter::successors(Some(node), |node| context.ast.parent(*node))
        .find(|node| context.ast.kind(*node) == NodeKind::CompilationUnit)?;
    let fragment = resolved
        .tables
        .declared_fragment
        .get(unit)
        .copied()?
        .cast::<LibraryFragment>()?;
    let offset = context.ast.tokens.get(import_keyword).offset as i32;
    let import = resolved
        .ctx
        .fragment(fragment)
        .library_imports
        .iter()
        .find(|import| import.import_keyword_offset == offset)?;
    match &import.directive.uri {
        DirectiveUri::Source { source, .. } | DirectiveUri::Library { source, .. } => {
            Some(source.uri.to_string())
        }
        _ => None,
    }
}
