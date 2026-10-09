// Dart source: pkg/linter/lib/src/extensions.dart

use crate::LinterContext;
use dartr_ast::*;
use dartr_element::{
    AnyElement, ConstructorElement, DisplayOptions, ElemRef, ElementId, InterfaceElement, TypeId,
    TypeKind, type_display_string_with,
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

#[derive(Clone, Copy)]
pub enum KnownAnnotation {
    AnalyzerPublicApi,
    Immutable,
    OptionalTypeArgs,
    Override,
    Redeclare,
    Required,
}

fn resolved_annotation_matches(
    c: &LinterContext<'_>,
    annotation: Id<Annotation>,
    expected: KnownAnnotation,
) -> Option<bool> {
    let resolved = c.resolved?;
    let element = c.element(annotation)?;
    let base = member::base_element(&resolved.ctx, element);
    let name = resolved.ctx.element_name(base);
    let library_uri = resolved.ctx.element_library_uri(base);
    let constructor_class = base
        .cast::<ConstructorElement>()
        .and_then(|_| resolved.ctx.element_data(base)?.enclosing)
        .and_then(|enclosing| resolved.ctx.element_name(enclosing));
    Some(match expected {
        KnownAnnotation::AnalyzerPublicApi => {
            constructor_class == Some("AnalyzerPublicApi")
                || match resolved.ctx.any(base) {
                    AnyElement::Getter(getter) => getter.return_type.get().is_some_and(|ty| {
                        matches!(
                            *resolved.ctx.ty(ty),
                            TypeKind::Interface { element, .. }
                                if resolved.ctx.element_name(element.raw())
                                    == Some("AnalyzerPublicApi")
                        )
                    }),
                    _ => false,
                }
        }
        KnownAnnotation::Immutable => {
            (matches!(resolved.ctx.any(base), AnyElement::Getter(_))
                && name == Some("immutable")
                && library_uri == Some("package:meta/meta.dart"))
                || (constructor_class == Some("Immutable")
                    && library_uri == Some("package:meta/meta.dart"))
        }
        KnownAnnotation::OptionalTypeArgs => {
            matches!(resolved.ctx.any(base), AnyElement::Getter(_))
                && name == Some("optionalTypeArgs")
                && library_uri == Some("package:meta/meta.dart")
        }
        KnownAnnotation::Override => {
            matches!(resolved.ctx.any(base), AnyElement::Getter(_))
                && name == Some("override")
                && library_uri == Some("dart:core")
        }
        KnownAnnotation::Redeclare => {
            matches!(resolved.ctx.any(base), AnyElement::Getter(_))
                && name == Some("redeclare")
                && library_uri == Some("package:meta/meta.dart")
        }
        KnownAnnotation::Required => {
            (matches!(resolved.ctx.any(base), AnyElement::Getter(_))
                && name == Some("required")
                && library_uri == Some("package:meta/meta.dart"))
                || (constructor_class == Some("Required")
                    && library_uri == Some("package:meta/meta.dart"))
        }
    })
}

/// Returns `None` only when an annotation on the declaration did not resolve.
/// C9 resolves annotation AST elements, but it does not evaluate annotations
/// or populate element `metadata_flags`; that remains Wave D work.
pub fn annotation_status(
    c: &LinterContext<'_>,
    node: NodeId,
    expected: KnownAnnotation,
) -> Option<bool> {
    let mut unresolved = false;
    for annotation in metadata(c.ast, node) {
        match resolved_annotation_matches(c, annotation, expected) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => unresolved = true,
        }
    }
    (!unresolved).then_some(false)
}

/// Reads annotations from a declaration in one of the resolved library units.
/// Imported element annotations cannot be inspected until Wave D evaluates the
/// copied metadata and fills `metadata_flags`.
pub fn element_annotation_status(
    c: &LinterContext<'_>,
    element: ElementId,
    expected: KnownAnnotation,
) -> Option<bool> {
    let mut found = false;
    let mut unresolved = false;
    for unit in std::iter::once(*c).chain(
        (0..c.resolved_units.len())
            .filter(|&index| index != c.current_unit)
            .filter_map(|index| c.resolved_unit(index)),
    ) {
        for node in (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .filter(|&node| unit.declared_element(node) == Some(element))
        {
            found = true;
            match annotation_status(&unit, node, expected) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => unresolved = true,
            }
        }
    }
    if found {
        return (!unresolved).then_some(false);
    }

    let resolved = c.resolved?;
    let metadata = &resolved
        .ctx
        .fragment_data(resolved.ctx.element_data(element)?.first_fragment)?
        .metadata;
    if metadata.annotations.is_empty() {
        Some(false)
    } else {
        // Metadata copied from another library has no resolved annotation AST
        // identity, and C9 does not populate this lazy flag cache.
        None
    }
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
        NodeKind::ClassTypeAlias => ast
            .list(ast[Id::<ClassTypeAlias>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::ConstructorDeclaration => ast
            .list(ast[Id::<ConstructorDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::EnumDeclaration => ast
            .list(ast[Id::<EnumDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::ExtensionDeclaration => ast
            .list(ast[Id::<ExtensionDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::ExtensionTypeDeclaration => ast
            .list(ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::FieldFormalParameter => ast
            .list(ast[Id::<FieldFormalParameter>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::MethodDeclaration => ast
            .list(ast[Id::<MethodDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::FunctionDeclaration => ast
            .list(ast[Id::<FunctionDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::FunctionTypeAlias => ast
            .list(ast[Id::<FunctionTypeAlias>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::GenericTypeAlias => ast
            .list(ast[Id::<GenericTypeAlias>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::MixinDeclaration => ast
            .list(ast[Id::<MixinDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::VariableDeclaration => ast
            .list(ast[Id::<VariableDeclaration>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::RegularFormalParameter => ast
            .list(ast[Id::<RegularFormalParameter>::from_raw(node)].metadata)
            .to_vec(),
        NodeKind::SuperFormalParameter => ast
            .list(ast[Id::<SuperFormalParameter>::from_raw(node)].metadata)
            .to_vec(),
        _ => Vec::new(),
    }
}
