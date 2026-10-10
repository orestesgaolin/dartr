// Dart source: pkg/analysis_server/lib/src/computer/computer_hover.dart
// Dart source: pkg/analysis_server/lib/src/computer/computer_documentation.dart
// Dart source: pkg/analysis_server/lib/src/computer/computer_overrides.dart (findOverriddenElements)
// Dart source: pkg/analyzer/lib/src/dartdoc/dartdoc_directive_info.dart
// Dart source: pkg/analysis_server/lib/src/lsp/dartdoc.dart (cleanDartdoc)

//! The hover of a resolved unit at an offset (Dart `DartUnitHoverComputer`)
//! and the documentation of elements.

use std::collections::HashMap;

use dartr_ast::*;
use dartr_element::display_string::{
    DisplayOptions, element_display_string_with, type_display_string_with,
};
use dartr_element::{Ctx, ElemRef, ElementId, Tag, TypeId, TypeKind};
use dartr_resolver::error::support;
use dartr_syntax::TokenId;
use dartr_typesystem::member;

use crate::element_locator::{Unit, is_executable};

/// Dart `HoverInformation` (the fields that the LSP hover reads).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct HoverInformation {
    pub offset: u32,
    pub length: u32,
    pub element_description: Option<String>,
    pub is_deprecated: bool,
    pub containing_class_description: Option<String>,
    pub containing_library_name: Option<String>,
    pub dartdoc: Option<String>,
    pub parameter: Option<String>,
    pub static_type: Option<String>,
}

/// The library name of a library file for the hover (Dart
/// `_libraryInfo`): the URI, or for a `file:` URI the path relative to its
/// package root.
pub type LibraryNameFn<'f> = dyn Fn(&str, &str) -> String + 'f;

/// Dart `DartUnitHoverComputer`.
pub struct HoverComputer<'u, 'c, 'a> {
    pub unit: &'u Unit<'c, 'a>,
    pub root: Id<CompilationUnit>,
    pub templates: &'u HashMap<String, String>,
    /// (library path, library URI) → library name.
    pub library_name: &'u LibraryNameFn<'u>,
}

impl HoverComputer<'_, '_, '_> {
    fn ast(&self) -> &Ast {
        self.unit.ast
    }

    fn ctx(&self) -> &Ctx<'_> {
        self.unit.ctx
    }

    /// Dart `compute()`.
    pub fn compute(&self, offset: u32) -> Option<HoverInformation> {
        let ast = self.ast();
        let node = ast.node_covering(self.root, offset, 0)?;
        let location = self.location_entity(node, offset);
        let node = self.target_node(node)?;
        let location = location?;
        let kind_ok = ast.is::<CompilationUnitMember>(node)
            || ast.is::<CatchClauseParameter>(node)
            || ast.is::<EnumConstantDeclaration>(node)
            || ast.is::<Expression>(node)
            || ast.is::<FormalParameter>(node)
            || ast.is::<MethodDeclaration>(node)
            || ast.is::<NamedArgument>(node)
            || ast.is::<NamedType>(node)
            || ast.is::<ConstructorDeclaration>(node)
            || ast.is::<DeclaredIdentifier>(node)
            || ast.is::<RecordLiteralNamedField>(node)
            || ast.is::<VariableDeclaration>(node)
            || ast.is::<DeclaredVariablePattern>(node)
            || ast.is::<AssignedVariablePattern>(node)
            || ast.is::<PatternFieldName>(node)
            || ast.is::<PrimaryConstructorBody>(node)
            || ast.is::<PrimaryConstructorDeclaration>(node)
            || ast.is::<PrimaryConstructorName>(node)
            || ast.is::<DartPattern>(node)
            || ast
                .cast::<LibraryDirective>(node)
                .is_some_and(|l| ast[l].name.is_none())
            || (ast.is::<SimpleIdentifier>(node)
                && ast
                    .parent(node)
                    .is_some_and(|p| ast.is::<ImportDirective>(p)))
            || ast.is::<ImportPrefixReference>(node);
        if !kind_ok {
            return None;
        }
        let (offset, length) = self.hover_range(node, location);
        let mut hover = HoverInformation {
            offset,
            length,
            ..Default::default()
        };
        let element_ref = self.locate_ref(node);
        let element = element_ref.map(|e| member::base_element(self.ctx(), e));
        if let (Some(element), Some(element_ref)) = (element, element_ref) {
            hover.element_description = self.element_display_string(node, element_ref);
            hover.is_deprecated = dartr_resolver::element_metadata::is_deprecated_with_kind(
                self.ctx(),
                element,
                "use",
                None,
            );
            let enclosing = self.ctx().element_data(element).and_then(|d| d.enclosing);
            let local = enclosing.is_some_and(is_executable) || is_local_element(element);
            if !local {
                hover.containing_class_description = self.containing_class(element);
                hover.containing_library_name = self.library_name_of(element);
            }
            hover.dartdoc = documentation(self.ctx(), element, self.templates);
        }
        hover.parameter = self.parameter_display_string(node);
        hover.static_type = self.type_display_string(node, element);
        Some(hover)
    }

    /// The element of [node] with its member substitution, if any.
    fn locate_ref(&self, node: NodeId) -> Option<ElemRef> {
        let base = self.unit.locate(node)?;
        // Keep the member when the locator returns `node.element` and the
        // table has a member for it.
        let ast = self.ast();
        let candidates: Vec<NodeId> = {
            let mut v = vec![node];
            if let Some(m) = ast.cast::<MethodInvocation>(node) {
                v.push(ast[m].method_name.raw());
            }
            if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
                v.push(ast[p].identifier.raw());
            }
            if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
                v.push(ast[i].constructor_name.raw());
            }
            v
        };
        for n in candidates {
            for table in [
                &self.unit.tables.element,
                &self.unit.tables.write_element,
                &self.unit.tables.read_element,
            ] {
                if let Some(&r) = table.get(n)
                    && member::base_element(self.ctx(), r) == base
                {
                    return Some(r);
                }
            }
        }
        Some(ElemRef::Base(base))
    }

    /// Dart `_containingClass`.
    fn containing_class(&self, element: ElementId) -> Option<String> {
        let mut current = Some(element);
        while let Some(e) = current {
            if e.cast::<dartr_element::InterfaceElement>().is_some() {
                if e == element {
                    return None;
                }
                return Some(support::display_name(self.ctx(), e));
            }
            current = self.ctx().element_data(e).and_then(|d| d.enclosing);
        }
        None
    }

    /// Dart `_libraryInfo(element)?.libraryName`.
    fn library_name_of(&self, element: ElementId) -> Option<String> {
        let library = support::library_of(self.ctx(), element)?;
        let first = self.ctx().get(library).first_fragment();
        let source = &self.ctx().fragment(first).source;
        Some((self.library_name)(&source.path, &source.uri))
    }

    /// Dart `_elementDisplayString`.
    fn element_display_string(&self, node: NodeId, element: ElemRef) -> Option<String> {
        let mut text = display_string(self.ctx(), element, true)?;
        let ast = self.ast();
        if let Some(i) = ast.cast::<InstanceCreationExpression>(node)
            && ast[i].keyword.is_none()
        {
            let prefix = if dartr_resolver::ast_ext::instance_creation_is_const(ast, i) {
                "(const) "
            } else {
                "(new) "
            };
            text = format!("{prefix}{text}");
        } else if let Some(d) = ast.cast::<DotShorthandConstructorInvocation>(node) {
            let is_const = ast[d].const_keyword.is_some()
                || dartr_resolver::ast_ext::in_constant_context(ast, node);
            let prefix = if is_const { "(const) " } else { "(new) " };
            text = format!("{prefix}{text}");
        }
        Some(text)
    }

    /// Dart `_hoverRange`.
    fn hover_range(&self, node: NodeId, entity: (u32, u32)) -> (u32, u32) {
        let ast = self.ast();
        let token = |t: TokenId| ast.tokens.get(t);
        if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
            let c = ast[i].constructor_name;
            return (ast.offset(c), ast.length(c));
        }
        if ast.is::<DotShorthandConstructorInvocation>(node) {
            return (ast.offset(node), ast.length(node));
        }
        if let Some(c) = ast.cast::<ConstructorDeclaration>(node) {
            let start = ast[c]
                .type_name
                .map(|t| ast.offset(t))
                .or(ast[c].new_keyword.map(|t| token(t).offset))
                .or(ast[c].factory_keyword.map(|t| token(t).offset))
                .unwrap_or(0);
            let end = ast[c]
                .name
                .map(|t| token(t).end())
                .or(ast[c].type_name.map(|t| ast.end(t)))
                .or(ast[c].new_keyword.map(|t| token(t).end()))
                .or(ast[c].factory_keyword.map(|t| token(t).end()))
                .unwrap_or(start);
            return (start, end - start);
        }
        if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(node) {
            if ast[p].constructor_name.is_some() {
                return entity;
            }
            let t = token(ast[p].type_name);
            return (t.offset, t.end() - t.offset);
        }
        entity
    }

    /// Dart `_locationEntity`.
    fn location_entity(&self, node: NodeId, offset: u32) -> Option<(u32, u32)> {
        let ast = self.ast();
        let token = |t: TokenId| {
            let t = ast.tokens.get(t);
            (t.offset, t.end() - t.offset)
        };
        let node_range = |n: NodeId| (ast.offset(n), ast.length(n));
        if let Some(n) = ast.cast::<BinaryExpression>(node) {
            return Some(token(ast[n].operator));
        }
        if let Some(n) = ast.cast::<ConditionalExpression>(node) {
            let q = ast.tokens.get(ast[n].question);
            if offset >= q.offset && offset <= q.end() {
                return Some(token(ast[n].question));
            }
            let c = ast.tokens.get(ast[n].colon);
            if offset >= c.offset && offset <= c.end() {
                return Some(token(ast[n].colon));
            }
            return Some(node_range(node));
        }
        if let Some(n) = ast.cast::<AssignmentExpression>(node) {
            return Some(token(ast[n].operator));
        }
        if let Some(n) = ast.cast::<PrefixExpression>(node) {
            return Some(token(ast[n].operator));
        }
        if let Some(n) = ast.cast::<PostfixExpression>(node) {
            return Some(token(ast[n].operator));
        }
        if let Some(n) = ast.cast::<CatchClauseParameter>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<ClassDeclaration>(node) {
            return Some(token(support::class_name_token(ast, ast[n].name_part)));
        }
        if let Some(n) = ast.cast::<ConstructorDeclaration>(node) {
            return ast[n]
                .name
                .map(token)
                .or(ast[n].type_name.map(|t| node_range(t.raw())))
                .or(ast[n].new_keyword.map(token))
                .or(ast[n].factory_keyword.map(token));
        }
        if let Some(n) = ast.cast::<DeclaredIdentifier>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<EnumConstantDeclaration>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<EnumDeclaration>(node) {
            return Some(token(support::class_name_token(ast, ast[n].name_part)));
        }
        if ast.is::<Expression>(node) {
            return Some(node_range(node));
        }
        if let Some(n) = ast.cast::<ExtensionDeclaration>(node) {
            return ast[n].name.map(token);
        }
        if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(node) {
            return Some(token(support::class_name_token(ast, ast[n].name_part)));
        }
        if ast.is::<FormalParameter>(node) {
            return dartr_resolver::ast_ext::formal_parameter_parts(ast, node)
                .name
                .map(token);
        }
        if let Some(n) = ast.cast::<FunctionDeclaration>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<ImportPrefixReference>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<LibraryDirective>(node) {
            return Some(token(ast[n].library_keyword));
        }
        if let Some(n) = ast.cast::<MethodDeclaration>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<NamedArgument>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<MixinDeclaration>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<NameWithTypeParameters>(node) {
            return Some(token(ast[n].type_name));
        }
        if let Some(n) = ast.cast::<NamedType>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<PatternFieldName>(node) {
            return ast[n].name.map(token);
        }
        if let Some(n) = ast.cast::<PrimaryConstructorBody>(node) {
            let declaration =
                dartr_resolver::element_binding_visitor::primary_constructor_body_declaration(
                    ast, n,
                )?;
            return ast[declaration]
                .constructor_name
                .map(|c| token(ast[c].name))
                .or(Some(token(ast[declaration].type_name)));
        }
        if let Some(n) = ast.cast::<PrimaryConstructorDeclaration>(node) {
            return ast[n]
                .constructor_name
                .map(|c| node_range(c.raw()))
                .or(Some(token(ast[n].type_name)));
        }
        if let Some(n) = ast.cast::<PrimaryConstructorName>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<RecordLiteralNamedField>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<GenericTypeAlias>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<FunctionTypeAlias>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<ClassTypeAlias>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<VariableDeclaration>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<DeclaredVariablePattern>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<AssignedVariablePattern>(node) {
            return Some(token(ast[n].name));
        }
        if let Some(n) = ast.cast::<WildcardPattern>(node) {
            return Some(token(ast[n].name));
        }
        None
    }

    /// Dart `_targetNode`.
    fn target_node(&self, node: NodeId) -> Option<NodeId> {
        let ast = self.ast();
        let parent = ast.parent(node);
        let parent2 = parent.and_then(|p| ast.parent(p));
        if ast.is::<ClassNamePart>(node) {
            return parent;
        }
        if let (Some(p), Some(p2)) = (parent, parent2)
            && ast.is::<NamedType>(p)
            && ast.is::<ConstructorName>(p2)
            && ast
                .parent(p2)
                .is_some_and(|p3| ast.is::<InstanceCreationExpression>(p3))
        {
            return ast.parent(p2);
        }
        if let (Some(p), Some(p2)) = (parent, parent2)
            && ast.is::<ConstructorName>(p)
            && ast.is::<InstanceCreationExpression>(p2)
        {
            return Some(p2);
        }
        if ast.is::<SimpleIdentifier>(node)
            && let Some(p) = parent
            && let Some(c) = ast.cast::<ConstructorDeclaration>(p)
            && ast[c].name.is_some()
        {
            return Some(p);
        }
        if ast.is::<SimpleIdentifier>(node)
            && let Some(p) = parent
            && ast.is::<DotShorthandConstructorInvocation>(p)
        {
            return Some(p);
        }
        Some(node)
    }

    /// Dart `_parameterDisplayString`.
    fn parameter_display_string(&self, node: NodeId) -> Option<String> {
        let ast = self.ast();
        if !ast.is::<Expression>(node) && !ast.is::<NamedArgument>(node) {
            return None;
        }
        let parameter = self
            .unit
            .tables
            .param_element
            .get(node)
            .copied()
            .or_else(|| {
                let named = ast.cast::<NamedArgument>(node)?;
                self.unit
                    .tables
                    .param_element
                    .get(ast[named].argument_expression.raw())
                    .copied()
            })?;
        let base = member::base_element(self.ctx(), parameter);
        let enclosing = self.ctx().element_data(base).and_then(|d| d.enclosing);
        if let Some(e) = enclosing {
            if e.tag() == Tag::Setter {
                return None;
            }
            if e.tag() == Tag::Method && is_operator_name(self.ctx(), e) {
                return None;
            }
        }
        self.element_display_string(node, parameter)
    }

    /// Dart `_typeDisplayString`.
    fn type_display_string(&self, node: NodeId, element: Option<ElementId>) -> Option<String> {
        let ast = self.ast();
        let ctx = self.ctx();
        let tables = self.unit.tables;
        let parent = ast.parent(node);
        let is_variable = |e: ElementId| e.cast::<dartr_element::VariableElement>().is_some();
        let static_type: Option<TypeId> = if let Some(n) = ast.cast::<NamedArgument>(node) {
            support::corresponding_parameter(ctx, ast, tables, n.raw())
                .map(|p| dartr_resolver::element_ext::variable_type(ctx, p))
        } else if ast.is::<Expression>(node)
            && element
                .is_none_or(|e| is_variable(e) || matches!(e.tag(), Tag::Getter | Tag::Setter))
        {
            self.type_of_declaration_or_reference(node)
        } else if let Some(e) = element.filter(|e| is_variable(*e)) {
            Some(dartr_resolver::element_ext::variable_type(ctx, e))
        } else if let Some(p) = parent
            && let Some(m) = ast.cast::<MethodInvocation>(p)
            && ast[m].method_name.raw() == node
        {
            tables
                .invoke_type
                .get(p)
                .copied()
                .filter(|t| !matches!(ctx.ty(*t), TypeKind::Dynamic))
        } else if let Some(p) = parent
            && let Some(pi) = ast.cast::<PrefixedIdentifier>(p)
            && ast[pi].identifier.raw() == node
        {
            tables
                .static_type
                .get(node)
                .copied()
                .filter(|t| !matches!(ctx.ty(*t), TypeKind::Dynamic))
        } else if let Some(p) = parent
            && let Some(d) = ast.cast::<DotShorthandInvocation>(p)
            && ast[d].member_name.raw() == node
        {
            tables
                .invoke_type
                .get(p)
                .copied()
                .filter(|t| !matches!(ctx.ty(*t), TypeKind::Dynamic))
        } else if ast.is::<PatternFieldName>(node)
            && let Some(p) = parent
            && let Some(field) = ast.cast::<PatternField>(p)
        {
            tables
                .pattern_info
                .get(ast[field].pattern.raw())
                .and_then(|i| i.matched_value_type)
        } else if ast.is::<DartPattern>(node) {
            tables
                .pattern_info
                .get(node)
                .and_then(|i| i.matched_value_type)
        } else {
            None
        };
        static_type.map(|t| type_display_string_with(ctx, t, DisplayOptions::default()))
    }

    /// Dart `_getTypeOfDeclarationOrReference`.
    fn type_of_declaration_or_reference(&self, node: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let ctx = self.ctx();
        let tables = self.unit.tables;
        if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
            if let Some(e) = self.unit.element(s)
                && e.cast::<dartr_element::VariableElement>().is_some()
                && support::in_declaration_context(ast, s)
            {
                return Some(dartr_resolver::element_ext::variable_type(ctx, e));
            }
            let parent = ast.parent(node);
            let parent2 = parent.and_then(|p| ast.parent(p));
            if let Some(p) = parent
                && let Some(a) = ast.cast::<AssignmentExpression>(p)
                && ast[a].left_hand_side.raw() == node
            {
                return tables.write_type.get(p).copied();
            }
            if let (Some(p), Some(p2)) = (parent, parent2)
                && let Some(a) = ast.cast::<AssignmentExpression>(p2)
                && ast[a].left_hand_side.raw() == p
            {
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(p)
                    && ast[pi].identifier.raw() == node
                {
                    return tables.write_type.get(p2).copied();
                }
                if let Some(pa) = ast.cast::<PropertyAccess>(p)
                    && ast[pa].property_name.raw() == node
                {
                    return tables.write_type.get(p2).copied();
                }
            }
        }
        tables.static_type.get(node).copied()
    }
}

fn is_operator_name(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let Some(name) = ctx.element_data(e).and_then(|d| d.name) else {
        return false;
    };
    let name = ctx.name_str(name);
    !name.is_empty() && !name.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '$')
}

/// Whether [e] is an element in a local store (a local variable, local
/// function, label, or parameter of a local function).
fn is_local_element(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::Label
    )
}

/// `element.displayString(multiline:)` for an element or a member.
pub fn display_string(ctx: &Ctx<'_>, element: ElemRef, multiline: bool) -> Option<String> {
    let options = DisplayOptions {
        multiline,
        prefer_type_alias: false,
    };
    match element {
        ElemRef::Base(e) => Some(element_display_string_with(ctx, e, options)),
        ElemRef::Member(m) => Some(dartr_element::display_string::member_display_string_with(
            ctx,
            ctx.member(m).base,
            &member_types(ctx, element),
            options,
        )),
    }
}

/// The substituted types of a member, for its display string.
fn member_types(ctx: &Ctx<'_>, element: ElemRef) -> dartr_element::display_string::MemberTypes {
    let base = member::base_element(ctx, element);
    let mut types = dartr_element::display_string::MemberTypes::default();
    if base.cast::<dartr_element::VariableElement>().is_some() {
        types.variable_type = Some(member::type_(ctx, element));
    }
    if is_executable(base) {
        types.return_type = Some(member::return_type(ctx, element));
        types.parameter_types = member::formal_parameters(ctx, element)
            .into_iter()
            .map(|p| member::type_(ctx, p))
            .collect();
    }
    types
}

// ---- documentation ----

/// Dart `elementWithDocumentation`.
fn element_with_documentation(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    match element.tag() {
        Tag::FieldFormalParameter => match ctx.any(element) {
            dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
            _ => None,
        },
        Tag::SuperFormalParameter => {
            let mut super_parameter =
                dartr_resolver::constant::evaluation::super_constructor_parameter(ctx, element)
                    .map(|p| member::base_element(ctx, p));
            while let Some(p) = super_parameter
                && p.tag() == Tag::SuperFormalParameter
            {
                super_parameter =
                    dartr_resolver::constant::evaluation::super_constructor_parameter(ctx, p)
                        .map(|p| member::base_element(ctx, p));
            }
            if let Some(p) = super_parameter
                && p.tag() == Tag::FieldFormalParameter
            {
                return match ctx.any(p) {
                    dartr_element::AnyElement::FormalParameter(fp) => {
                        fp.field.get().map(|f| f.raw())
                    }
                    _ => None,
                };
            }
            ctx.element_data(element).and_then(|d| d.enclosing)
        }
        Tag::FormalParameter => ctx.element_data(element).and_then(|d| d.enclosing),
        _ => Some(element),
    }
}

/// Dart `Element.documentationComment`: of the first fragment with one.
pub fn documentation_comment(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    if let Some(library) = element.cast::<dartr_element::LibraryElement>() {
        return ctx
            .get(library)
            .documentation_comment
            .as_ref()
            .map(|c| c.to_string());
    }
    let mut f = ctx.element_data(element).map(|d| d.first_fragment);
    while let Some(id) = f {
        let data = ctx.fragment_data(id)?;
        if let Some(c) = &data.documentation_comment {
            return Some(c.to_string());
        }
        f = data.next_fragment;
    }
    None
}

/// Dart `DartDocumentationComputer.compute(element)?.full`.
pub fn documentation(
    ctx: &Ctx<'_>,
    element: ElementId,
    templates: &HashMap<String, String>,
) -> Option<String> {
    let element = element_with_documentation(ctx, element)?;
    let (supers, interfaces) = find_overridden_elements(ctx, element);
    let mut candidates = vec![element];
    candidates.extend(supers);
    candidates.extend(interfaces);
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && let Some(v) = dartr_resolver::element_metadata::accessor_variable_any(ctx, element)
    {
        candidates.push(v);
    }
    let mut documented = None;
    let mut documented_getter = None;
    for candidate in candidates {
        if documentation_comment(ctx, candidate).is_some() {
            documented = Some(candidate);
            break;
        }
        if documented_getter.is_none()
            && candidate.tag() == Tag::Setter
            && let Some(getter) = corresponding_getter(ctx, candidate)
            && documentation_comment(ctx, getter).is_some()
        {
            documented_getter = Some(getter);
        }
    }
    let documented = documented.or(documented_getter)?;
    let raw = documentation_comment(ctx, documented)?;
    let mut full = process_dartdoc(&raw, templates);
    let documented_class = ctx.element_data(documented).and_then(|d| d.enclosing);
    let element_class = ctx.element_data(element).and_then(|d| d.enclosing);
    if let Some(c) = documented_class
        && Some(c) != element_class
    {
        full = format!("{full}\n\nCopied from `{}`.", support::display_name(ctx, c));
    }
    Some(full)
}

/// Dart `SetterElement.correspondingGetter`.
fn corresponding_getter(ctx: &Ctx<'_>, setter: ElementId) -> Option<ElementId> {
    let variable = dartr_resolver::element_metadata::accessor_variable_any(ctx, setter)?;
    match ctx.any(variable) {
        dartr_element::AnyElement::Field(f) => f.getter.map(|g| g.raw()),
        dartr_element::AnyElement::TopLevelVariable(v) => v.getter.map(|g| g.raw()),
        _ => None,
    }
}

/// Dart `findOverriddenElements(element)`: (super elements, interface
/// elements).
pub fn find_overridden_elements(
    ctx: &Ctx<'_>,
    seed: ElementId,
) -> (Vec<ElementId>, Vec<ElementId>) {
    let Some(class) = ctx
        .element_data(seed)
        .and_then(|d| d.enclosing)
        .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
    else {
        return (Vec::new(), Vec::new());
    };
    let mut finder = OverriddenFinder::new(ctx, seed, class);
    finder.add_super_overrides(Some(class), false);
    finder.visited.clear();
    finder.add_interface_overrides(Some(class), false);
    let supers = finder.super_elements.clone();
    finder.interface_elements.retain(|e| !supers.contains(e));
    (finder.super_elements, finder.interface_elements)
}

struct OverriddenFinder<'c, 'a> {
    ctx: &'c Ctx<'a>,
    library: Option<dartr_element::EId<dartr_element::LibraryElement>>,
    name: String,
    methods: bool,
    getters: bool,
    setters: bool,
    super_elements: Vec<ElementId>,
    interface_elements: Vec<ElementId>,
    visited: Vec<dartr_element::EId<dartr_element::InterfaceElement>>,
}

impl<'c, 'a> OverriddenFinder<'c, 'a> {
    fn new(
        ctx: &'c Ctx<'a>,
        seed: ElementId,
        class: dartr_element::EId<dartr_element::InterfaceElement>,
    ) -> Self {
        let name = support::display_name(ctx, seed);
        let (methods, getters, setters) = match seed.tag() {
            Tag::Field => (
                false,
                true,
                !dartr_resolver::element_ext::is_final(ctx, seed),
            ),
            Tag::Method => (true, false, false),
            Tag::Getter => (false, true, false),
            Tag::Setter => (false, false, true),
            _ => (false, false, false),
        };
        OverriddenFinder {
            ctx,
            library: support::library_of(ctx, class.raw()),
            name,
            methods,
            getters,
            setters,
            super_elements: Vec::new(),
            interface_elements: Vec::new(),
            visited: Vec::new(),
        }
    }

    fn interface_of(
        &self,
        ty: TypeId,
    ) -> Option<dartr_element::EId<dartr_element::InterfaceElement>> {
        match self.ctx.ty(ty) {
            TypeKind::Interface { element, .. } => Some(*element),
            _ => None,
        }
    }

    fn supertype(
        &self,
        class: dartr_element::EId<dartr_element::InterfaceElement>,
    ) -> Option<dartr_element::EId<dartr_element::InterfaceElement>> {
        self.ctx
            .interface(class)
            .supertype
            .get()
            .and_then(|t| self.interface_of(t))
    }

    fn list(
        &self,
        list: Option<dartr_element::TypeList>,
    ) -> Vec<dartr_element::EId<dartr_element::InterfaceElement>> {
        list.map(|l| {
            self.ctx
                .list(l)
                .iter()
                .filter_map(|&t| self.interface_of(t))
                .collect()
        })
        .unwrap_or_default()
    }

    fn constraints(
        &self,
        class: dartr_element::EId<dartr_element::InterfaceElement>,
    ) -> Vec<dartr_element::EId<dartr_element::InterfaceElement>> {
        match self.ctx.any(class.raw()) {
            dartr_element::AnyElement::Mixin(m) => self.list(m.superclass_constraints.get()),
            _ => Vec::new(),
        }
    }

    fn add_interface_overrides(
        &mut self,
        class: Option<dartr_element::EId<dartr_element::InterfaceElement>>,
        check_type: bool,
    ) {
        let Some(class) = class else { return };
        if self.visited.contains(&class) {
            return;
        }
        self.visited.push(class);
        if check_type
            && let Some(e) = self.lookup_member(class)
            && !self.interface_elements.contains(&e)
        {
            self.interface_elements.push(e);
        }
        for i in self.list(self.ctx.interface(class).interfaces.get()) {
            self.add_interface_overrides(Some(i), true);
        }
        let supertype = self.supertype(class);
        self.add_interface_overrides(supertype, check_type);
        for c in self.constraints(class) {
            self.add_interface_overrides(Some(c), true);
        }
    }

    fn add_super_overrides(
        &mut self,
        class: Option<dartr_element::EId<dartr_element::InterfaceElement>>,
        with_this_type: bool,
    ) {
        let Some(class) = class else { return };
        if self.visited.contains(&class) {
            return;
        }
        self.visited.push(class);
        if with_this_type
            && let Some(e) = self.lookup_member(class)
            && !self.super_elements.contains(&e)
        {
            self.super_elements.push(e);
        }
        let supertype = self.supertype(class);
        self.add_super_overrides(supertype, true);
        for m in self.list(self.ctx.interface(class).mixins.get()) {
            self.add_super_overrides(Some(m), true);
        }
        for c in self.constraints(class) {
            self.add_super_overrides(Some(c), true);
        }
    }

    fn lookup_member(
        &self,
        class: dartr_element::EId<dartr_element::InterfaceElement>,
    ) -> Option<ElementId> {
        let instance = self.ctx.instance(class.upcast());
        let matches = |e: ElementId| {
            let library = support::library_of(self.ctx, e);
            if library != self.library && self.name.starts_with('_') {
                return false;
            }
            support::display_name(self.ctx, e) == self.name
        };
        if self.methods
            && let Some(m) = instance
                .methods
                .iter()
                .map(|m| m.raw())
                .find(|&m| matches(m))
        {
            return Some(m);
        }
        if self.getters
            && let Some(g) = instance
                .getters
                .iter()
                .map(|g| g.raw())
                .find(|&g| matches(g))
        {
            return Some(g);
        }
        if self.setters
            && let Some(s) = instance
                .setters
                .iter()
                .map(|s| s.raw())
                .find(|&s| matches(s))
        {
            return Some(s);
        }
        None
    }
}

// ---- dartdoc ----

/// Dart `DartdocDirectiveInfo._stripDelimiters`.
pub fn strip_delimiters(comment: &str) -> Vec<String> {
    let bytes: Vec<char> = comment.chars().collect();
    let is_ws = |i: usize, eol: bool| -> bool {
        matches!(bytes.get(i), Some(' ') | Some('\t')) || (eol && bytes.get(i) == Some(&'\n'))
    };
    let starts_with = |i: usize, s: &str| -> bool {
        let s: Vec<char> = s.chars().collect();
        i + s.len() <= bytes.len() && bytes[i..i + s.len()] == s[..]
    };
    let skip_back = |start: usize, mut end: isize, eol: bool| -> isize {
        while (start as isize) < end && is_ws(end as usize, eol) {
            end -= 1;
        }
        end
    };
    let skip_fwd = |mut start: usize, end: usize, eol: bool| -> usize {
        while start < end && is_ws(start, eol) {
            start += 1;
        }
        start
    };
    let mut start = 0usize;
    let mut end = bytes.len();
    if starts_with(0, "/**") {
        start = skip_fwd(3, end, true);
        if comment.ends_with("*/") {
            end = skip_back(start, end as isize - 2, true).max(0) as usize;
        }
    }
    let mut line: isize = -1;
    let mut first_non_empty: isize = -1;
    let mut last_non_empty: isize = -1;
    let mut lines: Vec<String> = Vec::new();
    while start < end {
        line += 1;
        let eol = (start..end).find(|&i| bytes[i] == '\n').or_else(|| {
            // Dart `indexOf('\n', start)` searches the whole comment.
            (end..bytes.len()).find(|&i| bytes[i] == '\n')
        });
        let eol_index = match eol {
            Some(i) => i,
            None => end,
        };
        let mut line_start = skip_fwd(start, eol_index, false);
        if starts_with(line_start, "///") {
            line_start += 3;
            if is_ws(line_start, false) {
                line_start += 1;
            }
        } else if starts_with(line_start, "*") {
            line_start += 1;
            if is_ws(line_start, false) {
                line_start += 1;
            }
        }
        let line_end = (skip_back(line_start, eol_index as isize - 1, false) + 1).max(0) as usize;
        if line_start < line_end {
            if first_non_empty < 0 {
                first_non_empty = line;
            }
            if line > last_non_empty {
                last_non_empty = line;
            }
            lines.push(bytes[line_start..line_end].iter().collect());
        } else {
            lines.push(String::new());
        }
        start = eol_index + 1;
    }
    if first_non_empty < 0 || last_non_empty < first_non_empty {
        return Vec::new();
    }
    lines[first_non_empty as usize..=last_non_empty as usize].to_vec()
}

/// Dart `DartdocDirectiveInfo.processDartdoc(comment).full`.
pub fn process_dartdoc(comment: &str, templates: &HashMap<String, String>) -> String {
    let mut lines = strip_delimiters(comment);
    for line in lines.iter_mut().rev() {
        if line.is_empty() {
            continue;
        }
        if let Some(name) = macro_name(line) {
            if let Some(value) = templates.get(&name) {
                *line = value.clone();
            }
            continue;
        }
        if let Some(uri) = video_uri(line)
            && !uri.is_empty()
        {
            let label = uri.strip_prefix("https://").unwrap_or(&uri).to_string();
            *line = format!("[{label}]({uri})");
        }
    }
    lines.join("\n")
}

/// `{@macro\s+([^}]+)}`: the name of the first macro directive of [line].
fn macro_name(line: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = line[from..].find("{@macro") {
        let start = from + i + "{@macro".len();
        let rest = &line[start..];
        let ws = rest.len() - rest.trim_start_matches(|c: char| c.is_whitespace()).len();
        if ws > 0 {
            let body = &rest[ws..];
            if let Some(close) = body.find('}')
                && close > 0
            {
                // `[^}]+` is greedy from the first non-space, without
                // backtracking over spaces (they are part of the name).
                return Some(body[..close].to_string());
            }
        }
        from = start;
    }
    None
}

/// `{@(youtube|animation)\s+[^}]+\s+[^}]+\s+([^}]+)}`: the URI group.
fn video_uri(line: &str) -> Option<String> {
    for keyword in ["{@youtube", "{@animation"] {
        let mut from = 0;
        while let Some(i) = line[from..].find(keyword) {
            let start = from + i + keyword.len();
            if let Some(close) = line[start..].find('}') {
                let body = &line[start..start + close];
                if body.starts_with(|c: char| c.is_whitespace()) {
                    // Three space-separated parts; the last is the URI (the
                    // regex is greedy: the URI is after the last space).
                    let parts: Vec<&str> = body.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let last_space = body.rfind(|c: char| c.is_whitespace()).unwrap();
                        return Some(body[last_space + 1..].to_string());
                    }
                }
            }
            from = start;
        }
    }
    None
}

/// Dart `DartdocDirectiveInfo.extractTemplate(comment)`: adds the templates
/// of [comment] to [templates].
pub fn extract_templates(comment: &str, templates: &mut HashMap<String, String>) {
    let chars: Vec<char> = comment.chars().collect();
    let end = chars.len();
    if end < 28 {
        return;
    }
    let find = |pattern: &str, from: usize| -> Option<usize> {
        let p: Vec<char> = pattern.chars().collect();
        (from..=end.saturating_sub(p.len()))
            .find(|&i| chars[i..i + p.len()] == p[..])
            .map(|i| i + p.len())
    };
    let mut from = 0;
    loop {
        let Some(after) = find("{@template", from) else {
            return;
        };
        from = after;
        if from + 17 > end {
            return;
        }
        let c = chars[from];
        from += 1;
        if !matches!(c, ' ' | '\t' | '\n' | '\r') {
            continue;
        }
        while from < end && matches!(chars[from], ' ' | '\t' | '\n' | '\r') {
            from += 1;
        }
        let name_start = from;
        let mut i = from + 1;
        let mut c = ' ';
        while i < end {
            c = chars[i];
            if c == '\n' || c == '\r' || c == '}' {
                break;
            }
            i += 1;
        }
        if i == name_start {
            continue;
        }
        if c != '}' {
            continue;
        }
        let name_end = i;
        i += 1;
        let Some(end_index) = find("{@endtemplate}", i) else {
            return;
        };
        from = end_index;
        let end_index = end_index - 14;
        let name: String = chars[name_start..name_end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        let body: String = chars[name_end + 1..end_index]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        templates.insert(name, strip_delimiters(&body).join("\n"));
    }
}

/// Dart `cleanDartdoc(doc)`.
pub fn clean_dartdoc(doc: &str) -> String {
    // `(\n *{@.*?}$)|(^{@.*?}\n)` (multiline): directive lines are removed.
    let is_directive = |l: &str| {
        let t = l.trim_start_matches(' ');
        t.starts_with("{@") && t.len() >= 3 && t.ends_with('}')
    };
    let lines: Vec<&str> = doc.split('\n').collect();
    let mut i = 0;
    while i + 1 < lines.len() && lines[i].starts_with("{@") && is_directive(lines[i]) {
        i += 1;
    }
    let mut kept: Vec<&str> = Vec::new();
    if i < lines.len() {
        kept.push(lines[i]);
    }
    for line in lines.iter().skip(i + 1) {
        if !is_directive(line) {
            kept.push(line);
        }
    }
    let doc = kept.join("\n");
    // `(```\w+) +\w+` → the first group.
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let chars: Vec<char> = doc.chars().collect();
    let mut out = String::new();
    let mut k = 0;
    while k < chars.len() {
        if k + 3 <= chars.len() && chars[k..k + 3] == ['`', '`', '`'] {
            let mut j = k + 3;
            while j < chars.len() && is_word(chars[j]) {
                j += 1;
            }
            let word_end = j;
            let mut m = j;
            while m < chars.len() && chars[m] == ' ' {
                m += 1;
            }
            let mut n = m;
            while n < chars.len() && is_word(chars[n]) {
                n += 1;
            }
            if word_end > k + 3 && m > word_end && n > m {
                out.extend(&chars[k..word_end]);
                k = n;
                continue;
            }
        }
        out.push(chars[k]);
        k += 1;
    }
    out
}

/// Dart `DartdocDirectiveInfo.extractFromUnit(unit)` into [templates].
pub fn extract_templates_from_unit(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    templates: &mut HashMap<String, String>,
) {
    let comment_text = |node: NodeId| -> Option<String> {
        let comment = match ast.child_entities(node).first() {
            Some(Entity::Node(n)) => ast.cast::<Comment>(*n)?,
            _ => return None,
        };
        let tokens = ast.token_list(ast[comment].tokens);
        if tokens.len() == 1 {
            return Some(ast.tokens.lexeme(tokens[0]).replace("\r\n", "\n"));
        }
        Some(
            tokens
                .iter()
                .map(|&t| ast.tokens.lexeme(t))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };
    for &d in ast.list_raw(ast[unit].directives) {
        if let Some(text) = comment_text(d) {
            extract_templates(&text, templates);
        }
    }
    for &d in ast.list_raw(ast[unit].declarations) {
        if let Some(text) = comment_text(d) {
            extract_templates(&text, templates);
        }
        let mut members: Vec<NodeId> = Vec::new();
        if let Some(c) = ast.cast::<ClassDeclaration>(d) {
            members.extend(class_body_members(ast, ast[c].body.raw()));
        } else if let Some(e) = ast.cast::<EnumDeclaration>(d) {
            if let Some(body) = ast.cast::<BlockEnumBody>(ast[e].body) {
                members.extend(ast.list_raw(ast[body].constants).iter().copied());
                members.extend(ast.list_raw(ast[body].members).iter().copied());
            }
        } else if let Some(m) = ast.cast::<MixinDeclaration>(d) {
            members.extend(class_body_members(ast, ast[m].body.raw()));
        } else if let Some(x) = ast.cast::<ExtensionDeclaration>(d) {
            members.extend(class_body_members(ast, ast[x].body.raw()));
        } else if let Some(x) = ast.cast::<ExtensionTypeDeclaration>(d) {
            members.extend(class_body_members(ast, ast[x].body.raw()));
        }
        for m in members {
            if let Some(text) = comment_text(m) {
                extract_templates(&text, templates);
            }
        }
    }
}

fn class_body_members(ast: &Ast, body: NodeId) -> Vec<NodeId> {
    match ast.cast::<BlockClassBody>(body) {
        Some(b) => ast.list_raw(ast[b].members).to_vec(),
        None => Vec::new(),
    }
}

/// Dart `HoverHandler.toHover`: the markdown of [hover].
pub fn hover_markdown(hover: &HoverInformation) -> String {
    let mut content = String::new();
    if let Some(description) = &hover.element_description {
        content.push_str("```dart\n");
        if hover.is_deprecated {
            content.push_str("(deprecated) ");
        }
        content.push_str(description);
        content.push_str("\n```\n");
    }
    if let Some(t) = &hover.static_type {
        content.push_str(&format!("Type: `{t}`\n\n"));
    }
    let mut declared_in = String::new();
    if let Some(c) = hover
        .containing_class_description
        .as_ref()
        .filter(|c| !c.is_empty())
    {
        declared_in.push_str(&format!(" in `{c}`"));
    }
    if let Some(l) = hover
        .containing_library_name
        .as_ref()
        .filter(|l| !l.is_empty())
    {
        declared_in.push_str(&format!(" in *{l}*"));
    }
    if !declared_in.is_empty() {
        content.push_str(&format!("Declared{declared_in}.\n\n"));
    }
    if let Some(doc) = &hover.dartdoc {
        if !content.is_empty() {
            content.push_str("---\n");
        }
        content.push_str(&clean_dartdoc(doc));
        content.push('\n');
    }
    content.trim_end().to_string()
}
