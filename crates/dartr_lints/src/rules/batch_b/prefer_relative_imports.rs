// Dart source: pkg/linter/lib/src/rules/prefer_relative_imports.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;
use dartr_element::DirectiveUri;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_in_lib_dir() {
        return;
    }
    r.add(NodeKind::ImportDirective, "prefer_relative_imports", check);
}

/// The package name of a `package:` URI.
fn package_of(uri: &str) -> Option<&str> {
    uri.strip_prefix("package:")?.split('/').next()
}

/// Dart `isPackageSelfReference`.
fn is_package_self_reference(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(resolved) = c.resolved else { return false };
    // Dart `context.libraryElement?.uri`.
    let source_uri = ctx.library_uri(resolved.library);
    let keyword = c.ast[Id::<ImportDirective>::from_raw(node)].import_keyword;
    let offset = c.ast.tokens.get(keyword).offset as i32;
    let Some(fragment) = c
        .resolved
        .and_then(|r| r.tables.declared_fragment.get(unit_node(c)?).copied())
        .and_then(|f| f.cast::<dartr_element::LibraryFragment>())
    else {
        return false;
    };
    let Some(import) = ctx
        .fragment(fragment)
        .library_imports
        .iter()
        .find(|i| !i.is_synthetic && i.import_keyword_offset == offset)
    else {
        return false;
    };
    let (relative_uri, source) = match &import.directive.uri {
        DirectiveUri::Source {
            relative_uri,
            source,
            ..
        }
        | DirectiveUri::Library {
            relative_uri,
            source,
            ..
        } => (relative_uri.clone(), source.clone()),
        _ => return false,
    };
    let Some(import_package) = package_of(&relative_uri) else {
        return false;
    };
    // Dart `Uri.isSamePackageAs`.
    if package_of(source_uri) != Some(import_package) {
        return false;
    }
    c.package_root()
        .is_some_and(|root| std::path::Path::new(&*source.path).starts_with(root))
}

/// The `CompilationUnit` node of the visited unit.
fn unit_node(c: &LinterContext<'_>) -> Option<NodeId> {
    (0..c.ast.node_count())
        .map(NodeId::from_index)
        .find(|&n| kind(c, n) == NodeKind::CompilationUnit)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if is_package_self_reference(c, node) {
        let uri = c.ast[Id::<ImportDirective>::from_raw(node)].uri;
        c.report_node(out, &diag::PREFER_RELATIVE_IMPORTS, uri, &[]);
    }
}
