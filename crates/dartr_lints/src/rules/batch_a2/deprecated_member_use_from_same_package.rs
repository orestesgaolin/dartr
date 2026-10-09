// Dart source: pkg/linter/lib/src/rules/deprecated_member_use_from_same_package.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementId;
use dartr_typesystem::TypeExt;
use indexmap::IndexSet;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::CompilationUnit,
        "deprecated_member_use_from_same_package",
        check,
    );
}
fn metadata(context: &LinterContext<'_>, node: NodeId) -> Option<NodeList<Annotation>> {
    match context.ast.kind(node) {
        NodeKind::ClassDeclaration => {
            Some(context.ast[context.ast.cast::<ClassDeclaration>(node)?].metadata)
        }
        NodeKind::MixinDeclaration => {
            Some(context.ast[context.ast.cast::<MixinDeclaration>(node)?].metadata)
        }
        NodeKind::EnumDeclaration => {
            Some(context.ast[context.ast.cast::<EnumDeclaration>(node)?].metadata)
        }
        NodeKind::ExtensionDeclaration => {
            Some(context.ast[context.ast.cast::<ExtensionDeclaration>(node)?].metadata)
        }
        NodeKind::ExtensionTypeDeclaration => {
            Some(context.ast[context.ast.cast::<ExtensionTypeDeclaration>(node)?].metadata)
        }
        NodeKind::ConstructorDeclaration => {
            Some(context.ast[context.ast.cast::<ConstructorDeclaration>(node)?].metadata)
        }
        NodeKind::MethodDeclaration => {
            Some(context.ast[context.ast.cast::<MethodDeclaration>(node)?].metadata)
        }
        NodeKind::FunctionDeclaration => {
            Some(context.ast[context.ast.cast::<FunctionDeclaration>(node)?].metadata)
        }
        NodeKind::FieldDeclaration => {
            Some(context.ast[context.ast.cast::<FieldDeclaration>(node)?].metadata)
        }
        NodeKind::VariableDeclaration => {
            Some(context.ast[context.ast.cast::<VariableDeclaration>(node)?].metadata)
        }
        NodeKind::RegularFormalParameter => {
            Some(context.ast[context.ast.cast::<RegularFormalParameter>(node)?].metadata)
        }
        NodeKind::FieldFormalParameter => {
            Some(context.ast[context.ast.cast::<FieldFormalParameter>(node)?].metadata)
        }
        NodeKind::SuperFormalParameter => {
            Some(context.ast[context.ast.cast::<SuperFormalParameter>(node)?].metadata)
        }
        _ => None,
    }
}
fn has_deprecated(context: &LinterContext<'_>, node: NodeId) -> bool {
    metadata(context, node)
        .is_some_and(|m| super::deprecated_consistency::has_deprecated(context, m))
}
fn in_deprecated_declaration(context: &LinterContext<'_>, node: NodeId) -> bool {
    std::iter::successors(context.ast.parent(node), |n| context.ast.parent(*n))
        .any(|n| has_deprecated(context, n))
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if context.package_root().is_none() {
        return;
    }
    let mut deprecated = IndexSet::<ElementId>::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if has_deprecated(context, current)
            && let Some(element) = context.declared_element(current)
        {
            deprecated.insert(element);
        }
        pending.extend(context.ast.children(current));
    }
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if !in_deprecated_declaration(context, current)
            && let Some(element) = context
                .element(current)
                .and_then(|e| super::helpers::base_element(context, e))
            && deprecated.contains(&element)
        {
            let name = context
                .resolved
                .and_then(|r| r.ctx.element_name(element))
                .unwrap_or("");
            context.report_node(
                out,
                &diag::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITHOUT_MESSAGE,
                current,
                &[name],
            );
        }
        pending.extend(context.ast.children(current));
    }
}
