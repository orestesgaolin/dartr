// Dart sources: pkg/linter/lib/src/extensions.dart; pkg/linter/lib/src/ast.dart

use crate::LinterContext;
use dartr_ast::{
    Ast, BlockFunctionBody, ExpressionFunctionBody, FunctionBody, NodeId, NodeKind, NodeType,
};
use dartr_element::{ElemRef, ElementId, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn ancestors(ast: &Ast, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    std::iter::successors(ast.parent(node), |node| ast.parent(*node))
}

pub fn nearest_function_body(ast: &Ast, node: NodeId) -> Option<NodeId> {
    ancestors(ast, node).find(|node| FunctionBody::test(ast.kind(*node)))
}

pub fn is_synchronous_body(ast: &Ast, body: NodeId) -> bool {
    match ast.kind(body) {
        NodeKind::BlockFunctionBody => ast[ast.cast::<BlockFunctionBody>(body).unwrap()]
            .keyword
            .is_none(),
        NodeKind::ExpressionFunctionBody => ast[ast.cast::<ExpressionFunctionBody>(body).unwrap()]
            .keyword
            .is_none(),
        NodeKind::EmptyFunctionBody | NodeKind::NativeFunctionBody => true,
        _ => false,
    }
}

pub fn base_element(context: &LinterContext<'_>, element: ElemRef) -> Option<ElementId> {
    Some(member::base_element(&context.resolved?.ctx, element))
}

pub fn annotation_type(context: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    context.resolved?.tables.annotation_type.get(node).copied()
}

pub fn is_invalid(context: &LinterContext<'_>, ty: TypeId) -> bool {
    matches!(
        context.resolved.map(|r| *r.ctx.ty(ty)),
        Some(TypeKind::Invalid)
    )
}

pub fn is_dynamic(context: &LinterContext<'_>, ty: TypeId) -> bool {
    matches!(
        context.resolved.map(|r| *r.ctx.ty(ty)),
        Some(TypeKind::Dynamic)
    )
}

pub fn implements(context: &LinterContext<'_>, ty: TypeId, library: &str, name: &str) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let matches = |candidate: TypeId| {
        if library.contains(':') {
            resolved.ctx.interface_element(candidate).is_some_and(|e| {
                resolved.ctx.element_name(e.raw()) == Some(name)
                    && resolved.ctx.element_library_uri(e.raw()) == Some(library)
            })
        } else {
            resolved.ctx.is_interface_of(candidate, library, name)
        }
    };
    if matches(ty) {
        return true;
    }
    if resolved.ctx.interface_element(ty).is_none() {
        return false;
    }
    resolved
        .ctx
        .all_supertypes(ty)
        .iter()
        .any(|&supertype| matches(supertype))
}

pub fn type_text(context: &LinterContext<'_>, ty: TypeId) -> String {
    dartr_element::type_display_string_with(
        &context.resolved.expect("resolved lint context").ctx,
        ty,
        dartr_element::DisplayOptions::default(),
    )
}
