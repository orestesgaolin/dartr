// Dart source: pkg/analyzer/lib/src/dart/ast/element_locator.dart

//! Dart `ElementLocator.locate(node)`: the element of a node of a resolved
//! unit.

use dartr_ast::*;
use dartr_element::{Ctx, ElementId, ResolutionTables, Tag, TypeKind};
use dartr_resolver::error::support;
use dartr_typesystem::member;

/// The resolved data of a unit.
pub struct Unit<'c, 'a> {
    pub ctx: &'c Ctx<'a>,
    pub ast: &'c Ast,
    pub tables: &'c ResolutionTables,
}

impl Unit<'_, '_> {
    pub fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        support::element_of(self.ctx, self.tables, node)
    }

    pub fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        support::declared_element(self.ctx, self.tables, node)
    }

    /// Dart `SimpleIdentifier.writeOrReadElement`.
    pub fn write_or_read_element(&self, node: Id<SimpleIdentifier>) -> Option<ElementId> {
        support::write_or_read_element(self.ctx, self.ast, self.tables, node)
            .or_else(|| support::read_element(self.ctx, self.tables, node))
    }

    /// The library of an import or export directive (Dart
    /// `libraryImport?.importedLibrary`).
    pub fn directive_library(&self, node: NodeId) -> Option<ElementId> {
        let ast = self.ast;
        let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
        let fragment = self
            .tables
            .declared_fragment
            .get(unit.raw())?
            .cast::<dartr_element::LibraryFragment>()?;
        let data = self.ctx.fragment(fragment);
        let directives = ast.list_raw(ast[unit].directives);
        let uri = if ast.is::<ImportDirective>(node) {
            let index = directives
                .iter()
                .filter(|d| ast.is::<ImportDirective>(**d))
                .position(|d| *d == node)?;
            data.library_imports
                .iter()
                .filter(|i| !i.is_synthetic)
                .nth(index)?
                .directive
                .uri
                .clone()
        } else {
            let index = directives
                .iter()
                .filter(|d| ast.is::<ExportDirective>(**d))
                .position(|d| *d == node)?;
            data.library_exports.get(index)?.directive.uri.clone()
        };
        support::uri_library(&uri).map(|l| l.raw())
    }

    /// Dart `InterfaceElement.unnamedConstructor`.
    fn unnamed_constructor(&self, element: ElementId) -> Option<ElementId> {
        let interface = element.cast::<dartr_element::InterfaceElement>()?;
        self.ctx
            .interface(interface)
            .constructors
            .iter()
            .copied()
            .find(|&c| {
                matches!(
                    self.ctx
                        .element_data(c.raw())
                        .and_then(|d| d.name)
                        .map(|n| self.ctx.name_str(n)),
                    Some("new") | None
                )
            })
            .map(|c| c.raw())
    }

    /// Dart `_ElementMapper2.visitIdentifier`.
    fn identifier_element(&self, node: NodeId) -> Option<ElementId> {
        let ast = self.ast;
        let parent = ast.parent(node);
        if let Some(parent) = parent {
            if let Some(a) = ast.cast::<Annotation>(parent) {
                if ast[a].name.raw() == node && ast[a].constructor_name.is_none() {
                    return self.element(a);
                }
            } else if let Some(c) = ast.cast::<ConstructorDeclaration>(parent) {
                if ast[c].type_name.map(|t| t.raw()) == Some(node) {
                    if ast[c].name.is_some() {
                        return self.declared_element(c);
                    }
                    let element = self.element(node);
                    if let Some(e) = element
                        && e.cast::<dartr_element::InterfaceElement>().is_some()
                    {
                        return self.unnamed_constructor(e);
                    }
                } else if ast[c].name == Some(ast.end_token(node)) {
                    return self.declared_element(c);
                }
            } else if let Some(s) = ast.cast::<ConstructorSelector>(parent) {
                let enum_constant = ast
                    .parent(s)
                    .filter(|p| ast.is::<EnumConstantArguments>(*p))
                    .and_then(|p| ast.parent(p))
                    .and_then(|p| ast.cast::<EnumConstantDeclaration>(p));
                if let Some(d) = enum_constant {
                    return self.element(d);
                }
            } else if ast.is::<DottedName>(parent) {
                let grand = ast.parent(parent);
                if let Some(l) = grand.and_then(|g| ast.cast::<LibraryDirective>(g)) {
                    return self.element(l);
                }
                return None;
            } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
                if ast[m].method_name.raw() == node
                    && ast.tokens.lexeme(ast[ast[m].method_name].token) == "call"
                {
                    let target = dartr_resolver::ast_ext::method_invocation_real_target(ast, m)
                        .map(|t| t.raw());
                    if let Some(t) = target
                        && ast.is::<Identifier>(t)
                        && self
                            .tables
                            .static_type
                            .get(t)
                            .is_some_and(|ty| matches!(self.ctx.ty(*ty), TypeKind::Function(_)))
                    {
                        return self.element(t);
                    }
                }
            } else if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
                let prefix = ast[p].prefix;
                if ast[p].identifier.raw() == node
                    && ast.tokens.lexeme(ast[ast[p].identifier].token) == "call"
                    && self
                        .tables
                        .static_type
                        .get(prefix.raw())
                        .is_some_and(|ty| matches!(self.ctx.ty(*ty), TypeKind::Function(_)))
                {
                    return self.element(prefix);
                }
            }
        }
        if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
            return self.write_or_read_element(s);
        }
        // A prefixed identifier: `element` (write or read of the
        // identifier).
        self.element(node)
    }

    /// Dart `ElementLocator.locate(node)`.
    pub fn locate(&self, node: NodeId) -> Option<ElementId> {
        let ast = self.ast;
        match ast.kind(node) {
            NodeKind::Annotation
            | NodeKind::AssignedVariablePattern
            | NodeKind::AssignmentExpression
            | NodeKind::BinaryExpression
            | NodeKind::ExtensionOverride
            | NodeKind::ImportPrefixReference
            | NodeKind::IndexExpression
            | NodeKind::LabelReference
            | NodeKind::LibraryDirective
            | NodeKind::NamedType
            | NodeKind::PatternField
            | NodeKind::PostfixExpression
            | NodeKind::PrefixExpression => {
                if ast.is::<IndexExpression>(node) {
                    return self
                        .tables
                        .element
                        .get(node)
                        .map(|&e| member::base_element(self.ctx, e))
                        .or_else(|| support::write_element(self.ctx, self.tables, node))
                        .or_else(|| support::read_element(self.ctx, self.tables, node));
                }
                if ast.is::<AssignedVariablePattern>(node) {
                    return self.element(node);
                }
                if ast.is::<LibraryDirective>(node) {
                    // Dart `LibraryDirective.element`: the library of the
                    // unit.
                    return self.element(node).or_else(|| {
                        let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
                        let fragment = self
                            .tables
                            .declared_fragment
                            .get(unit.raw())?
                            .cast::<dartr_element::LibraryFragment>()?;
                        Some(self.ctx.fragment(fragment).library.raw())
                    });
                }
                self.element(node)
            }
            NodeKind::CatchClauseParameter
            | NodeKind::ClassDeclaration
            | NodeKind::ClassTypeAlias
            | NodeKind::CompilationUnit
            | NodeKind::ConstructorDeclaration
            | NodeKind::DeclaredIdentifier
            | NodeKind::DeclaredVariablePattern
            | NodeKind::EnumConstantDeclaration
            | NodeKind::EnumDeclaration
            | NodeKind::ExtensionDeclaration
            | NodeKind::ExtensionTypeDeclaration
            | NodeKind::FunctionDeclaration
            | NodeKind::FunctionTypeAlias
            | NodeKind::GenericTypeAlias
            | NodeKind::Label
            | NodeKind::MethodDeclaration
            | NodeKind::MixinDeclaration
            | NodeKind::TypeParameter
            | NodeKind::VariableDeclaration => self.declared_element(node),
            NodeKind::ConstructorSelector => {
                let enum_constant = ast
                    .parent(node)
                    .filter(|p| ast.is::<EnumConstantArguments>(*p))
                    .and_then(|p| ast.parent(p))
                    .and_then(|p| ast.cast::<EnumConstantDeclaration>(p));
                enum_constant.and_then(|d| self.element(d))
            }
            NodeKind::DotShorthandConstructorInvocation => {
                let n = ast.cast::<DotShorthandConstructorInvocation>(node)?;
                self.element(ast[n].constructor_name)
            }
            NodeKind::DotShorthandInvocation => {
                let n = ast.cast::<DotShorthandInvocation>(node)?;
                self.element(ast[n].member_name)
            }
            NodeKind::DotShorthandPropertyAccess => {
                let n = ast.cast::<DotShorthandPropertyAccess>(node)?;
                self.element(ast[n].property_name)
            }
            NodeKind::DottedName => {
                let parent = ast.parent(node)?;
                ast.cast::<LibraryDirective>(parent)
                    .and_then(|l| self.element(l))
            }
            NodeKind::ExportDirective | NodeKind::ImportDirective => self.directive_library(node),
            NodeKind::InstanceCreationExpression => {
                let n = ast.cast::<InstanceCreationExpression>(node)?;
                self.element(ast[n].constructor_name)
            }
            NodeKind::MethodInvocation => {
                let n = ast.cast::<MethodInvocation>(node)?;
                let name = ast[n].method_name;
                self.element(name)
                    .or_else(|| self.identifier_element(name.raw()))
            }
            NodeKind::NamedArgument => {
                support::corresponding_parameter(self.ctx, ast, self.tables, node)
            }
            NodeKind::NameWithTypeParameters => self.locate(ast.parent(node)?),
            NodeKind::PartOfDirective => None,
            NodeKind::PatternFieldName => {
                let parent = ast.parent(node)?;
                if ast.is::<PatternField>(parent) {
                    self.element(parent)
                } else {
                    None
                }
            }
            NodeKind::PrefixedIdentifier => {
                let n = ast.cast::<PrefixedIdentifier>(node)?;
                self.element(n)
                    .or_else(|| self.identifier_element(ast[n].identifier.raw()))
            }
            NodeKind::SimpleIdentifier => self.identifier_element(node),
            NodeKind::PrimaryConstructorBody => {
                let declaration =
                    dartr_resolver::element_binding_visitor::primary_constructor_body_declaration(
                        ast,
                        ast.cast::<PrimaryConstructorBody>(node)?,
                    )?;
                self.declared_element(declaration)
            }
            NodeKind::PrimaryConstructorDeclaration => {
                let parent = ast.parent(node)?;
                if ast.is::<Declaration>(parent) {
                    self.declared_element(parent)
                } else {
                    None
                }
            }
            NodeKind::PrimaryConstructorName => {
                let parent = ast.parent(node)?;
                if ast.is::<PrimaryConstructorDeclaration>(parent) {
                    // Dart `declaration.declaredFragment?.element`: the
                    // fragment of the primary constructor.
                    self.declared_element(parent)
                } else {
                    self.locate(parent)
                }
            }
            NodeKind::SimpleStringLiteral
            | NodeKind::AdjacentStrings
            | NodeKind::StringInterpolation => {
                let parent = ast.parent(node)?;
                if ast.is::<ExportDirective>(parent) || ast.is::<ImportDirective>(parent) {
                    self.directive_library(parent)
                } else {
                    None
                }
            }
            _ => {
                if ast.is::<FormalParameter>(node) {
                    return self.declared_element(node);
                }
                None
            }
        }
    }
}

/// Dart `AstNode.getElement()` (analysis_server `utilities/extensions/ast.dart`).
pub fn get_element(unit: &Unit<'_, '_>, node: NodeId) -> Option<ElementId> {
    let ast = unit.ast;
    let mut node = node;
    if ast.is::<DottedName>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<StringLiteral>(node)
        && ast
            .parent(node)
            .is_some_and(|p| ast.is::<UriBasedDirective>(p))
    {
        return None;
    }
    unit.locate(node)
}

/// Whether [e] is a Dart `ExecutableElement`.
pub fn is_executable(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::Method
            | Tag::Constructor
            | Tag::Getter
            | Tag::Setter
            | Tag::TopLevelFunction
            | Tag::LocalFunction
    )
}
