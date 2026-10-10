// Dart source: pkg/linter/lib/src/rules/prefer_void_to_null.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::NamedType, "prefer_void_to_null", check);
}

/// Dart `isFutureOrVoid`.
fn is_future_or_void(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    ctx.is_dart_async_future_or(ty)
        && ctx.interface_element(ty).is_some()
        && ctx
            .type_arguments(ty)
            .first()
            .is_some_and(|&t| matches!(*ctx.ty(t), TypeKind::Void))
}

/// Dart `isVoidIncompatibleOverride`.
fn is_void_incompatible_override(c: &LinterContext<'_>, parent: NodeId, node: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let return_type = c.ast[Id::<MethodDeclaration>::from_raw(parent)].return_type;
    if return_type.map(|t| c.ast.offset(t)) != Some(c.ast.offset(node)) {
        return false;
    }
    let Some(member_element) = c
        .declared_element(parent)
        .and_then(|e| overridden_member(c, e))
    else {
        return false;
    };
    let return_type = member::return_type(&ctx, member_element);
    !(matches!(*ctx.ty(return_type), TypeKind::Void) || is_future_or_void(c, return_type))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let Some(ty) = annotation_type(c, node) else {
        return;
    };
    if !ctx.is_dart_core_null(ty) {
        return;
    }
    let Some(parent) = c.ast.parent(node) else {
        return;
    };
    match kind(c, parent) {
        NodeKind::TypeLiteral
        | NodeKind::GenericFunctionType
        | NodeKind::CastPattern
        | NodeKind::AsExpression
        | NodeKind::ExtensionOnClause => return,
        _ => {}
    }
    let grand_parent = c.ast.parent(parent);
    if kind(c, parent) == NodeKind::RegularFormalParameter
        && let Some(list) = grand_parent.filter(|&l| kind(c, l) == NodeKind::FormalParameterList)
    {
        let owner = c.ast.parent(list);
        if owner.is_some_and(|o| kind(c, o) == NodeKind::GenericFunctionType) {
            return;
        }
        if let Some(primary) =
            owner.filter(|&o| kind(c, o) == NodeKind::PrimaryConstructorDeclaration)
            && c.ast
                .parent(primary)
                .is_some_and(|p| kind(c, p) == NodeKind::ExtensionTypeDeclaration)
        {
            return;
        }
    }
    if let Some(list) = c.ast.cast::<VariableDeclarationList>(parent)
        && c.ast[list].type_.map(|t| t.raw()) == Some(node)
        && grand_parent.is_none_or(|g| kind(c, g) != NodeKind::FieldDeclaration)
    {
        return;
    }
    if kind(c, parent) == NodeKind::TypeArgumentList
        && let Some(literal) = grand_parent
    {
        if let Some(l) = c.ast.cast::<ListLiteral>(literal)
            && c.ast.list_raw(c.ast[l].elements).is_empty()
        {
            return;
        }
        if let Some(l) = c.ast.cast::<SetOrMapLiteral>(literal)
            && c.ast.list_raw(c.ast[l].elements).is_empty()
        {
            return;
        }
    }
    if kind(c, parent) == NodeKind::MethodDeclaration
        && is_void_incompatible_override(c, parent, node)
    {
        return;
    }
    let declaration = this_or_ancestor(c, parent, |n| ClassMember::test(kind(c, n)))
        .or_else(|| this_or_ancestor(c, parent, |n| CompilationUnitMember::test(kind(c, n))));
    if declaration.is_some_and(|d| is_augmentation(c, d)) {
        return;
    }
    c.report_token(
        out,
        &diag::PREFER_VOID_TO_NULL,
        c.ast[Id::<NamedType>::from_raw(node)].name,
        &[],
    );
}
