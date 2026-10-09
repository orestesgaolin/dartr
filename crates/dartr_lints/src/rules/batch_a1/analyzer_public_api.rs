// Dart source: pkg/linter/lib/src/rules/analyzer_public_api.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    ClassDeclaration, CompilationUnit, EnumDeclaration, FunctionDeclaration, GenericTypeAlias, Id,
    MixinDeclaration, NodeId, NodeKind,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, TypeKind};
use indexmap::IndexSet;

use super::helpers::{
    descendants, element_library_uri, element_name, has_resolved_annotation, node_type,
};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::CompilationUnit, "analyzer_public_api", check);
}

fn is_analyzer_public_uri(uri: &str) -> bool {
    uri.starts_with("package:analyzer/") && !uri.starts_with("package:analyzer/src/")
}

fn declaration_name(c: &LinterContext<'_>, node: NodeId) -> Option<dartr_syntax::TokenId> {
    match c.ast.kind(node) {
        NodeKind::ClassDeclaration => {
            super::helpers::class_name_token(c.ast, Id::<ClassDeclaration>::from_raw(node))
        }
        NodeKind::EnumDeclaration => {
            let part = c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part;
            c.ast
                .cast::<dartr_ast::NameWithTypeParameters>(part)
                .map(|p| c.ast[p].type_name)
        }
        NodeKind::MixinDeclaration => Some(c.ast[Id::<MixinDeclaration>::from_raw(node)].name),
        NodeKind::FunctionDeclaration => {
            Some(c.ast[Id::<FunctionDeclaration>::from_raw(node)].name)
        }
        NodeKind::GenericTypeAlias => Some(c.ast[Id::<GenericTypeAlias>::from_raw(node)].name),
        NodeKind::FunctionTypeAlias => {
            Some(c.ast[Id::<dartr_ast::FunctionTypeAlias>::from_raw(node)].name)
        }
        _ => None,
    }
}

fn problematic_type(c: &LinterContext<'_>, node: NodeId) -> Option<String> {
    let r = c.resolved?;
    let ty = node_type(c, node)?;
    let TypeKind::Interface { element, .. } = *r.ctx.ty(ty) else {
        return None;
    };
    let reference = ElemRef::Base(element.raw());
    let uri = element_library_uri(c, reference)?;
    if uri.starts_with("dart:")
        || is_analyzer_public_uri(uri)
        || (uri.starts_with("package:") && !uri.split('/').any(|part| part == "src"))
    {
        return None;
    }
    element_name(c, reference).map(str::to_owned)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if !is_analyzer_public_uri(&c.source_uri()) {
        return;
    }
    let unit = &c.ast[Id::<CompilationUnit>::from_raw(node)];
    for declaration in c.ast.list_raw(unit.declarations) {
        let Some(name_token) = declaration_name(c, *declaration) else {
            continue;
        };
        let name = c.ast.tokens.lexeme(name_token);
        let public =
            !name.starts_with('_') || has_resolved_annotation(c, *declaration, "AnalyzerPublicApi");
        if !public {
            continue;
        }
        if name.ends_with("Impl") {
            c.report_token(
                out,
                &diag::ANALYZER_PUBLIC_API_IMPL_IN_PUBLIC_API,
                name_token,
                &[],
            );
        }
        let mut bad = IndexSet::new();
        for child in descendants(c.ast, *declaration) {
            if matches!(
                c.ast.kind(child),
                NodeKind::NamedType
                    | NodeKind::GenericFunctionType
                    | NodeKind::RecordTypeAnnotation
            ) && let Some(name) = problematic_type(c, child)
            {
                bad.insert(name);
            }
        }
        if !bad.is_empty() {
            let joined = bad.into_iter().collect::<Vec<_>>().join(", ");
            c.report_token(
                out,
                &diag::ANALYZER_PUBLIC_API_BAD_TYPE,
                name_token,
                &[&joined],
            );
        }
    }
}
