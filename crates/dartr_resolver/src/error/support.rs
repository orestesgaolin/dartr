// Dart source: pkg/analyzer/lib/src/dart/ast/extensions.dart
// (writeOrReadElement), pkg/analyzer/lib/src/utilities/extensions/ast.dart
// (inCommentReference), pkg/analyzer/lib/src/dart/ast/ast.dart
// (SimpleIdentifier.inDeclarationContext, DartPattern.variablePattern,
// ImportDirective.libraryImport, ExportDirective.libraryExport,
// Argument.correspondingParameter), pkg/analyzer/lib/src/dart/element/element.dart
// (displayName)

//! Small AST and element helpers that the verifiers of
//! `best_practices_verifier.dart` and its helpers share (Dart getters on
//! AST nodes and elements).

use dartr_ast::{
    AssignedVariablePattern, Ast, CastPattern, ClassNamePart, CommentReference, CompilationUnit,
    DeclaredVariablePattern, ExportDirective, Id, ImportDirective, Label, NameWithTypeParameters,
    NamedArgument, NodeId, NullAssertPattern, NullCheckPattern, ParenthesizedPattern,
    PostfixExpression, PrefixExpression, PrefixedIdentifier, PrimaryConstructorDeclaration,
    PropertyAccess, SimpleIdentifier, Statement, SwitchMember,
};
use dartr_element::{
    Ctx, DirectiveUri, EId, ElemRef, ElementId, LibraryElement, LibraryExport, LibraryImport,
    ResolutionTables, Tag,
};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;

use super::VerifierHost;

/// An offset and a length (Dart `SyntacticEntity` of a report).
pub type Range = (u32, u32);

pub fn node_range(ast: &Ast, node: impl Into<NodeId>) -> Range {
    let node = node.into();
    (ast.offset(node), ast.length(node))
}

pub fn token_range(ast: &Ast, token: TokenId) -> Range {
    let t = ast.tokens.get(token);
    (t.offset, t.end() - t.offset)
}

/// The base element of `node.element`.
pub fn element_of(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: impl Into<NodeId>,
) -> Option<ElementId> {
    tables
        .element
        .get(node.into())
        .map(|&e| dartr_typesystem::member::base_element(ctx, e))
}

/// `node.element` (with the member, if any).
pub fn elem_ref_of(tables: &ResolutionTables, node: impl Into<NodeId>) -> Option<ElemRef> {
    tables.element.get(node.into()).copied()
}

/// `node.declaredFragment?.element`.
pub fn declared_element(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: impl Into<NodeId>,
) -> Option<ElementId> {
    let fragment = *tables.declared_fragment.get(node.into())?;
    ctx.fragment_data(fragment)?.element.try_get().copied()
}

/// `node.readElement` (base element).
pub fn read_element(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: impl Into<NodeId>,
) -> Option<ElementId> {
    tables
        .read_element
        .get(node.into())
        .map(|&e| dartr_typesystem::member::base_element(ctx, e))
}

/// `node.writeElement` (base element).
pub fn write_element(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: impl Into<NodeId>,
) -> Option<ElementId> {
    tables
        .write_element
        .get(node.into())
        .map(|&e| dartr_typesystem::member::base_element(ctx, e))
}

/// Dart `_writeElement(node)` of `dart/ast/extensions.dart`.
fn write_element_of_parent(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    node: NodeId,
) -> Option<ElementId> {
    let parent = ast.parent(node)?;
    if let Some(p) = ast.cast::<dartr_ast::AssignmentExpression>(parent)
        && ast[p].left_hand_side.raw() == node
    {
        return write_element(ctx, tables, p);
    }
    if let Some(p) = ast.cast::<PostfixExpression>(parent)
        && ast[p].operand.raw() == node
    {
        return write_element(ctx, tables, p);
    }
    if let Some(p) = ast.cast::<PrefixExpression>(parent)
        && ast[p].operand.raw() == node
    {
        return write_element(ctx, tables, p);
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent)
        && ast[p].identifier.raw() == node
    {
        return write_element_of_parent(ctx, ast, tables, parent);
    }
    if let Some(p) = ast.cast::<PropertyAccess>(parent)
        && ast[p].property_name.raw() == node
    {
        return write_element_of_parent(ctx, ast, tables, parent);
    }
    None
}

/// Dart `SimpleIdentifier.writeOrReadElement`.
pub fn write_or_read_element(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    node: Id<SimpleIdentifier>,
) -> Option<ElementId> {
    write_element_of_parent(ctx, ast, tables, node.raw()).or_else(|| element_of(ctx, tables, node))
}

/// Dart `Argument.correspondingParameter` (base element).
pub fn corresponding_parameter(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    argument: NodeId,
) -> Option<ElementId> {
    let found = tables.param_element.get(argument).copied().or_else(|| {
        let named = ast.cast::<NamedArgument>(argument)?;
        tables
            .param_element
            .get(ast[named].argument_expression.raw())
            .copied()
    });
    found.map(|e| dartr_typesystem::member::base_element(ctx, e))
}

/// Dart `SimpleIdentifier.inDeclarationContext()`.
pub fn in_declaration_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(import) = ast.cast::<ImportDirective>(parent) {
        return ast[import].prefix == Some(node);
    }
    if ast.is::<Label>(parent) {
        let Some(parent2) = ast.parent(parent) else {
            return false;
        };
        return ast.is::<Statement>(parent2) || ast.is::<SwitchMember>(parent2);
    }
    false
}

/// Dart `AstNode.inCommentReference`.
pub fn in_comment_reference(ast: &Ast, node: NodeId) -> bool {
    let mut current = ast.parent(node);
    for _ in 0..3 {
        let Some(p) = current else {
            return false;
        };
        if ast.is::<CommentReference>(p) {
            return true;
        }
        current = ast.parent(p);
    }
    false
}

/// Dart `DartPattern.variablePattern`: the name of the variable pattern
/// inside casts, null checks, null asserts and parentheses.
pub fn variable_pattern_name(ast: &Ast, mut pattern: NodeId) -> Option<TokenId> {
    loop {
        if let Some(p) = ast.cast::<DeclaredVariablePattern>(pattern) {
            return Some(ast[p].name);
        }
        if let Some(p) = ast.cast::<AssignedVariablePattern>(pattern) {
            return Some(ast[p].name);
        }
        pattern = if let Some(p) = ast.cast::<CastPattern>(pattern) {
            ast[p].pattern.raw()
        } else if let Some(p) = ast.cast::<NullAssertPattern>(pattern) {
            ast[p].pattern.raw()
        } else if let Some(p) = ast.cast::<NullCheckPattern>(pattern) {
            ast[p].pattern.raw()
        } else if let Some(p) = ast.cast::<ParenthesizedPattern>(pattern) {
            ast[p].pattern.raw()
        } else {
            return None;
        };
    }
}

/// The name token of a class declaration (Dart `namePart.typeName`).
pub fn class_name_token(ast: &Ast, part: Id<ClassNamePart>) -> TokenId {
    if let Some(p) = ast.cast::<NameWithTypeParameters>(part) {
        return ast[p].type_name;
    }
    if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(part) {
        return ast[p].type_name;
    }
    unreachable!("ClassNamePart")
}

/// The index of the directive [node] among the directives of the same
/// kind [T] of its unit.
fn directive_index<T: dartr_ast::NodeType + ?Sized>(ast: &Ast, node: NodeId) -> Option<usize> {
    let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
    ast.list(ast[unit].directives)
        .iter()
        .filter(|d| ast.is::<T>(**d))
        .position(|d| d.raw() == node)
}

/// Dart `ImportDirective.libraryImport`.
pub fn library_import<'a, 'h, H: VerifierHost<'a>>(
    host: &'h H,
    node: Id<ImportDirective>,
) -> Option<&'a LibraryImport> {
    let ctx = host.ctx();
    let index = directive_index::<ImportDirective>(host.ast(), node.raw())?;
    let imports = &ctx.fragment(host.fragment()).library_imports;
    // Dart: the synthetic `dart:core` import is not a directive.
    imports.iter().filter(|i| !i.is_synthetic).nth(index)
}

/// Dart `ExportDirective.libraryExport`.
pub fn library_export<'a, 'h, H: VerifierHost<'a>>(
    host: &'h H,
    node: Id<ExportDirective>,
) -> Option<&'a LibraryExport> {
    let ctx = host.ctx();
    let index = directive_index::<ExportDirective>(host.ast(), node.raw())?;
    ctx.fragment(host.fragment()).library_exports.get(index)
}

/// The library of a directive URI.
pub fn uri_library(uri: &DirectiveUri) -> Option<EId<LibraryElement>> {
    match uri {
        DirectiveUri::Library { library, .. } => Some(*library),
        _ => None,
    }
}

/// Dart `Element.displayName` (`ConstructorElement.displayName`: `C` or
/// `C.name`).
pub fn display_name(ctx: &Ctx<'_>, element: ElementId) -> String {
    let name = ctx.element_name(element);
    match element.tag() {
        Tag::Constructor => {
            let class_name = ctx
                .element_data(element)
                .and_then(|d| d.enclosing)
                .and_then(|c| ctx.element_name(c))
                .unwrap_or("<null>");
            match name {
                Some("new") | None => class_name.to_string(),
                Some(n) => format!("{class_name}.{n}"),
            }
        }
        Tag::Extension | Tag::Library => name.unwrap_or("").to_string(),
        _ => name.unwrap_or("<unnamed>").to_string(),
    }
}

/// Dart `element.library`.
pub fn library_of(ctx: &Ctx<'_>, element: ElementId) -> Option<EId<LibraryElement>> {
    if let Some(l) = element.cast::<LibraryElement>() {
        return Some(l);
    }
    ctx.element_data(element)?.library
}

/// Dart `element.enclosingElement`.
pub fn enclosing_of(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    ctx.element_data(element)?.enclosing
}

/// Dart `Identifier.isPrivateName(name)`.
pub fn is_private_name(name: &str) -> bool {
    name.starts_with('_')
}
