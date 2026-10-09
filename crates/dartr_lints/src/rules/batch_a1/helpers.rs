// Dart source: pkg/linter/lib/src/extensions.dart

use crate::LinterContext;
use dartr_ast::*;
use dartr_element::{
    AnyElement, DisplayOptions, ElemRef, ElementId, InterfaceElement, TypeId, TypeKind,
    type_display_string_with,
};
use dartr_typesystem::{TypeExt, member};

pub fn lexeme<'a>(c: &'a LinterContext<'_>, token: dartr_syntax::TokenId) -> &'a str {
    c.ast.tokens.lexeme(token)
}

pub fn unparenthesized(ast: &Ast, mut node: Id<Expression>) -> Id<Expression> {
    while let Some(parenthesized) = ast.cast::<ParenthesizedExpression>(node) {
        node = ast[parenthesized].expression;
    }
    node
}

pub fn descendants(ast: &Ast, root: impl Into<NodeId>) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending = ast.children(root).into_iter().rev().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        result.push(node);
        pending.extend(ast.children(node).into_iter().rev());
    }
    result
}

pub fn ancestor(ast: &Ast, node: impl Into<NodeId>, kind: NodeKind) -> Option<NodeId> {
    let mut current = ast.parent(node);
    while let Some(node) = current {
        if ast.kind(node) == kind {
            return Some(node);
        }
        current = ast.parent(node);
    }
    None
}

pub fn base_element(c: &LinterContext<'_>, element: ElemRef) -> Option<ElementId> {
    let resolved = c.resolved?;
    Some(member::base_element(&resolved.ctx, element))
}

pub fn element_name<'a>(c: &'a LinterContext<'_>, element: ElemRef) -> Option<&'a str> {
    let resolved = c.resolved?;
    member::name(&resolved.ctx, element)
}

pub fn element_library_uri<'a>(c: &'a LinterContext<'_>, element: ElemRef) -> Option<&'a str> {
    let resolved = c.resolved?;
    member::library(&resolved.ctx, element).map(|library| resolved.ctx.library_uri(library))
}

/// Whether semantic metadata flags for this element are required but are not
/// available yet. Unit C9 fills `metadata_flags`; until then, treating an
/// annotated element as unannotated can create false-positive diagnostics.
pub fn element_has_unresolved_metadata(c: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(resolved) = c.resolved else {
        return false;
    };
    resolved
        .ctx
        .element_data(element)
        .and_then(|data| resolved.ctx.fragment_data(data.first_fragment))
        .is_some_and(|fragment| {
            !fragment.metadata.annotations.is_empty()
                && fragment.metadata.metadata_flags.try_get().is_none()
        })
}

pub fn declared_type(c: &LinterContext<'_>, node: impl Into<NodeId>) -> Option<TypeId> {
    let resolved = c.resolved?;
    let element = c.declared_element(node)?;
    match resolved.ctx.any(element) {
        AnyElement::Field(e) => e.type_.get(),
        AnyElement::TopLevelVariable(e) => e.type_.get(),
        AnyElement::FormalParameter(e) => e.type_.get(),
        AnyElement::LocalVariable(e) => e.type_.get(),
        AnyElement::Method(e) => e.return_type.get(),
        AnyElement::Getter(e) => e.return_type.get(),
        AnyElement::Setter(e) => e.return_type.get(),
        AnyElement::TopLevelFunction(e) => e.return_type.get(),
        AnyElement::LocalFunction(e) => e.return_type.get(),
        AnyElement::Constructor(e) => e.return_type.get(),
        _ => None,
    }
}

pub fn display_type(c: &LinterContext<'_>, ty: TypeId) -> Option<String> {
    let resolved = c.resolved?;
    Some(type_display_string_with(
        &resolved.ctx,
        ty,
        DisplayOptions::default(),
    ))
}

pub fn node_type(c: &LinterContext<'_>, node: impl Into<NodeId>) -> Option<TypeId> {
    let node = node.into();
    let resolved = c.resolved?;
    resolved
        .tables
        .static_type
        .get(node)
        .copied()
        .or_else(|| resolved.tables.annotation_type.get(node).copied())
        .or_else(|| {
            resolved
                .tables
                .pattern_info
                .get(node)
                .and_then(|info| info.matched_value_type)
        })
}

pub fn is_dynamic(c: &LinterContext<'_>, node: impl Into<NodeId>) -> bool {
    c.static_type(node).is_some_and(|ty| ty == TypeId::DYNAMIC)
}

pub fn is_void(c: &LinterContext<'_>, ty: TypeId) -> bool {
    c.resolved
        .is_some_and(|r| matches!(r.ctx.ty(ty), TypeKind::Void))
}

pub fn is_future_void(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(r) = c.resolved else { return false };
    if !r.ctx.is_dart_async_future(ty) {
        return false;
    }
    match *r.ctx.ty(ty) {
        TypeKind::Interface { args, .. } => {
            r.ctx.list(args).first().is_some_and(|&arg| is_void(c, arg))
        }
        _ => false,
    }
}

pub fn interface_element(
    c: &LinterContext<'_>,
    ty: TypeId,
) -> Option<dartr_element::EId<InterfaceElement>> {
    let r = c.resolved?;
    r.ctx.interface_element(ty)
}

pub fn is_null_literal(ast: &Ast, node: impl Into<NodeId>) -> bool {
    ast.kind(node.into()) == NodeKind::NullLiteral
}

pub fn class_name_token(ast: &Ast, node: Id<ClassDeclaration>) -> Option<dartr_syntax::TokenId> {
    let part = ast[node].name_part;
    if let Some(name) = ast.cast::<NameWithTypeParameters>(part) {
        Some(ast[name].type_name)
    } else {
        ast.cast::<PrimaryConstructorDeclaration>(part)
            .map(|primary| ast[primary].type_name)
    }
}

pub fn metadata(ast: &Ast, node: NodeId) -> Vec<Id<Annotation>> {
    match ast.kind(node) {
        NodeKind::ClassDeclaration => ast
            .list(ast[Id::<ClassDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::FieldDeclaration => ast
            .list(ast[Id::<FieldDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::MethodDeclaration => ast
            .list(ast[Id::<MethodDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::FunctionDeclaration => ast
            .list(ast[Id::<FunctionDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::VariableDeclaration => ast
            .list(ast[Id::<VariableDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::RegularFormalParameter => ast
            .list(ast[Id::<RegularFormalParameter>::from_raw(node)].metadata)
            .to_vec(),
        _ => Vec::new(),
    }
}

pub fn has_resolved_annotation(c: &LinterContext<'_>, node: NodeId, name: &str) -> bool {
    metadata(c.ast, node).into_iter().any(|annotation| {
        c.element(annotation)
            .is_some_and(|element| element_name(c, element) == Some(name))
    })
}
