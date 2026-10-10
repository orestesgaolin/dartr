// Dart source: pkg/linter/lib/src/rules/unreachable_from_main.dart
use super::util::*;
use crate::{AnnotationRef, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, InterfaceElement, Tag};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use indexmap::{IndexMap, IndexSet};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::CompilationUnit,
        "unreachable_from_main",
        visit_compilation_unit,
    );
}

/// A declaration: the index of its unit and its node.
type Decl = (usize, NodeId);

const WIDGET_PREVIEWS_URI: &str = "package:flutter/src/widget_previews/widget_previews.dart";

/// Dart `Element.isPrivate` (`ElementImpl`: an element without a name is
/// private).
fn is_private(c: &LinterContext<'_>, element: ElementId) -> bool {
    name(c, element).is_none_or(|n| n.starts_with('_'))
}

/// Dart `_DeclarationGatherer.addDeclarations`.
fn add_declarations(
    c: &LinterContext<'_>,
    unit: usize,
    root: NodeId,
    declarations: &mut IndexSet<Decl>,
) {
    for &declaration in c
        .ast
        .list_raw(c.ast[Id::<CompilationUnit>::from_raw(root)].declarations)
    {
        if let Some(variables) = c.ast.cast::<TopLevelVariableDeclaration>(declaration) {
            let list = c.ast[variables].variables;
            for &variable in c.ast.list_raw(c.ast[list].variables) {
                declarations.insert((unit, variable));
            }
            continue;
        }
        declarations.insert((unit, declaration));
        let Some(element) = c.declared_element(declaration) else {
            continue;
        };
        if is_private(c, element) {
            continue;
        }
        let container = match kind(c, declaration) {
            NodeKind::ClassDeclaration | NodeKind::EnumDeclaration | NodeKind::MixinDeclaration => {
                Some(element)
            }
            NodeKind::ExtensionDeclaration | NodeKind::ExtensionTypeDeclaration => None,
            _ => continue,
        };
        let members = super::sort_unnamed_constructors_first::body_members(c, declaration);
        add_members(c, unit, container, members, declarations);
    }
}

/// Dart `_DeclarationGatherer._addMembers`.
fn add_members(
    c: &LinterContext<'_>,
    unit: usize,
    container: Option<ElementId>,
    members: Vec<NodeId>,
    declarations: &mut IndexSet<Decl>,
) {
    let is_override = |element: Option<ElementId>| -> bool {
        let Some(container) = container.and_then(|e| e.cast::<InterfaceElement>()) else {
            return false;
        };
        let Some(element) = element else { return false };
        let Some(ctx) = rctx(c) else { return false };
        let Some(name) = Name::for_element(&ctx, ElemRef::Base(element)) else {
            return false;
        };
        InheritanceManager3::new(ctx)
            .get_overridden(container, name)
            .is_some()
    };
    for member in members {
        match kind(c, member) {
            NodeKind::ConstructorDeclaration => {
                let grandparent = c.ast.parent(member).and_then(|p| c.ast.parent(p));
                if let Some(e) = c.declared_element(member)
                    && !is_private(c, e)
                    && !grandparent.is_some_and(|g| kind(c, g) == NodeKind::EnumDeclaration)
                {
                    declarations.insert((unit, member));
                }
            }
            NodeKind::FieldDeclaration => {
                let fields = c.ast[Id::<FieldDeclaration>::from_raw(member)].fields;
                for &field in c.ast.list_raw(c.ast[fields].variables) {
                    let Some(element) = c.declared_element(field) else {
                        continue;
                    };
                    if element.tag() == Tag::Field && !is_private(c, element) {
                        let getter = rctx(c).and_then(|ctx| match ctx.any(element) {
                            dartr_element::AnyElement::Field(f) => f.getter.map(|g| g.raw()),
                            _ => None,
                        });
                        if !is_override(getter) {
                            declarations.insert((unit, field));
                        }
                    }
                }
            }
            NodeKind::MethodDeclaration => {
                if let Some(element) = c.declared_element(member)
                    && !is_private(c, element)
                {
                    let raw_name = lexeme(c, c.ast[Id::<MethodDeclaration>::from_raw(member)].name);
                    let is_test_method = raw_name.starts_with("test_")
                        || raw_name.starts_with("solo_test_")
                        || matches!(
                            raw_name,
                            "setUp" | "tearDown" | "setUpClass" | "tearDownClass"
                        );
                    if !is_override(Some(element)) && !is_test_method {
                        declarations.insert((unit, member));
                    }
                }
            }
            _ => {}
        }
    }
}

fn unit_context<'a>(c: &LinterContext<'a>, unit: usize) -> Option<LinterContext<'a>> {
    c.resolved_unit(unit)
}

fn visit_compilation_unit(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.resolved.is_none() {
        return;
    }
    let units: Vec<LinterContext<'_>> = (0..c.resolved_units.len())
        .filter_map(|i| unit_context(c, i))
        .collect();
    if units.len() != c.resolved_units.len() {
        return;
    }
    let mut declarations = IndexSet::new();
    for (index, unit) in units.iter().enumerate() {
        add_declarations(unit, index, c.resolved_units[index].unit, &mut declarations);
    }
    let entry_points: Vec<Decl> = declarations
        .iter()
        .copied()
        .filter(|&(u, d)| is_entry_point(&units[u], u, d))
        .collect();
    if entry_points.is_empty() {
        return;
    }

    let mut declaration_by_element: IndexMap<ElementId, Decl> = IndexMap::new();
    for &(u, d) in &declarations {
        let uc = &units[u];
        let Some(element) = uc.declared_element(d) else {
            continue;
        };
        declaration_by_element.insert(element, (u, d));
        if let Some(ctx) = rctx(uc) {
            let (getter, setter) = match ctx.any(element) {
                dartr_element::AnyElement::TopLevelVariable(v) => {
                    (v.getter.map(|g| g.raw()), v.setter.map(|s| s.raw()))
                }
                dartr_element::AnyElement::Field(f) => {
                    (f.getter.map(|g| g.raw()), f.setter.map(|s| s.raw()))
                }
                _ => (None, None),
            };
            if let Some(getter) = getter {
                declaration_by_element.insert(getter, (u, d));
            }
            if let Some(setter) = setter {
                declaration_by_element.insert(setter, (u, d));
            }
        }
    }

    let mut dependencies: IndexMap<Decl, IndexSet<Decl>> = IndexMap::new();
    for &(u, d) in &declarations {
        let mut visitor = ReferenceVisitor {
            c: &units[u],
            declaration_map: &declaration_by_element,
            declarations: IndexSet::new(),
            pattern_level: 0,
        };
        visitor.visit(d);
        dependencies.insert((u, d), visitor.declarations);
    }

    let mut used_members: IndexSet<Decl> = entry_points.iter().copied().collect();
    let mut to_check: Vec<Decl> = used_members.iter().copied().collect();
    while let Some(declaration) = to_check.pop() {
        for &dep in &dependencies[&declaration] {
            if used_members.insert(dep) {
                to_check.push(dep);
            }
        }
    }

    let mut unit_declarations = IndexSet::new();
    add_declarations(c, c.current_unit, node, &mut unit_declarations);
    let unused_declarations: IndexSet<Decl> = unit_declarations
        .iter()
        .copied()
        .filter(|d| !used_members.contains(d))
        .collect();
    for &(_, member) in &unused_declarations {
        let Some(element) = c.declared_element(member) else {
            continue;
        };
        if is_private(c, element)
            || has_visible_for_testing(c, element)
            || has_widget_preview(c, element)
        {
            continue;
        }
        let enclosing_declaration = c
            .ast
            .parent(member)
            .and_then(|p| this_or_ancestor(c, p, |n| Declaration::test(kind(c, n))));
        if let Some(enclosing) = enclosing_declaration
            && unused_declarations.contains(&(c.current_unit, enclosing))
        {
            continue;
        }
        let (offset, length) = node_to_annotate(c, member);
        let name = name_for_error(c, member);
        c.report_offset(out, &diag::UNREACHABLE_FROM_MAIN, offset, length, &[&name]);
    }
}

/// Dart `_Visitor._isEntryPoint`.
fn is_entry_point(c: &LinterContext<'_>, unit: usize, node: NodeId) -> bool {
    let Some(function) = c.ast.cast::<FunctionDeclaration>(node) else {
        return false;
    };
    let f = &c.ast[function];
    lexeme(c, f.name) == "main"
        || c.ast
            .list(f.metadata)
            .iter()
            .any(|&a| is_exempting_annotation(c, unit, a))
}

/// Dart `Annotation._elementType`.
fn annotation_element_type(
    c: &LinterContext<'_>,
    annotation: Id<Annotation>,
) -> Option<dartr_element::TypeId> {
    let ctx = rctx(c)?;
    let element = c.element(annotation)?;
    match base(c, element).tag() {
        Tag::Constructor | Tag::Getter => {
            Some(dartr_typesystem::member::return_type(&ctx, element))
        }
        _ => None,
    }
}

fn annotation_type_is(
    c: &LinterContext<'_>,
    annotation: Id<Annotation>,
    library: &str,
    type_name: &str,
) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(ty) = annotation_element_type(c, annotation) else {
        return false;
    };
    let Some(element) = ctx.interface_element(ty) else {
        return false;
    };
    name(c, element.raw()) == Some(type_name) && library_uri(c, element.raw()) == Some(library)
}

/// Dart `_Visitor._isExemptingAnnotation`.
fn is_exempting_annotation(c: &LinterContext<'_>, unit: usize, annotation: Id<Annotation>) -> bool {
    if annotation_type_is(c, annotation, "dart:core", "pragma") {
        // Dart `_isValidVmEntryPoint`.
        let Some(metadata) = c.resolved.and_then(|r| r.metadata) else {
            return false;
        };
        let Some(value) = metadata.annotation_value(AnnotationRef {
            unit: unit as u32,
            node: annotation.raw(),
        }) else {
            return false;
        };
        let Some(name) = value.get_field("name") else {
            return false;
        };
        return name.has_known_value() && name.to_string_value() == Some("vm:entry-point");
    }
    annotation_type_is(c, annotation, WIDGET_PREVIEWS_URI, "Preview")
}

/// Dart `Element.hasWidgetPreview`.
fn has_widget_preview(c: &LinterContext<'_>, element: ElementId) -> bool {
    let is_candidate = match element.tag() {
        Tag::Constructor | Tag::TopLevelFunction => true,
        _ => element.cast::<dartr_element::ExecutableElement>().is_some() && is_static(c, element),
    };
    is_candidate
        && !is_private(c, element)
        && c.has_annotation_where(element, |ctx, a| {
            a.tag() == Tag::Constructor
                && ctx
                    .element_data(a)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    == Some("Preview")
                && dartr_typesystem::member::library(ctx, ElemRef::Base(a))
                    .is_some_and(|l| ctx.library_uri(l) == WIDGET_PREVIEWS_URI)
        })
}

/// Dart `Declaration.nameForError`.
fn name_for_error(c: &LinterContext<'_>, node: NodeId) -> String {
    let token = |t| lexeme(c, t).to_string();
    match kind(c, node) {
        NodeKind::ClassDeclaration
        | NodeKind::EnumDeclaration
        | NodeKind::ExtensionTypeDeclaration => {
            let name_part = match kind(c, node) {
                NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(node)]
                    .name_part
                    .raw(),
                NodeKind::EnumDeclaration => {
                    c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part.raw()
                }
                _ => c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)]
                    .name_part
                    .raw(),
            };
            if let Some(n) = c.ast.cast::<NameWithTypeParameters>(name_part) {
                token(c.ast[n].type_name)
            } else if let Some(p) = c.ast.cast::<PrimaryConstructorDeclaration>(name_part) {
                token(c.ast[p].type_name)
            } else {
                String::new()
            }
        }
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            let name = n.name.map_or("new".to_string(), token);
            let type_name = n
                .type_name
                .map(|t| simple_name(c, t).to_string())
                .or_else(|| {
                    c.declared_element(node)
                        .and_then(|e| enclosing(c, e))
                        .and_then(|e| self::name(c, e))
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "unknown".to_string());
            format!("{type_name}.{name}")
        }
        NodeKind::EnumConstantDeclaration => {
            token(c.ast[Id::<EnumConstantDeclaration>::from_raw(node)].name)
        }
        NodeKind::ExtensionDeclaration => c.ast[Id::<ExtensionDeclaration>::from_raw(node)]
            .name
            .map_or("the unnamed extension".to_string(), token),
        NodeKind::FunctionDeclaration => {
            token(c.ast[Id::<FunctionDeclaration>::from_raw(node)].name)
        }
        NodeKind::MethodDeclaration => token(c.ast[Id::<MethodDeclaration>::from_raw(node)].name),
        NodeKind::MixinDeclaration => token(c.ast[Id::<MixinDeclaration>::from_raw(node)].name),
        NodeKind::ClassTypeAlias => token(c.ast[Id::<ClassTypeAlias>::from_raw(node)].name),
        NodeKind::FunctionTypeAlias => token(c.ast[Id::<FunctionTypeAlias>::from_raw(node)].name),
        NodeKind::GenericTypeAlias => token(c.ast[Id::<GenericTypeAlias>::from_raw(node)].name),
        NodeKind::VariableDeclaration => {
            token(c.ast[Id::<VariableDeclaration>::from_raw(node)].name)
        }
        _ => String::new(),
    }
}

/// Dart `_ReferenceVisitor`.
struct ReferenceVisitor<'c, 'a> {
    c: &'c LinterContext<'a>,
    declaration_map: &'c IndexMap<ElementId, Decl>,
    declarations: IndexSet<Decl>,
    pattern_level: usize,
}

impl ReferenceVisitor<'_, '_> {
    fn element(&self, node: NodeId) -> Option<ElemRef> {
        self.c.element(node)
    }

    fn visit_children(&mut self, node: NodeId) {
        for child in self.c.ast.children(node) {
            self.visit(child);
        }
    }

    fn visit(&mut self, node: NodeId) {
        let c = self.c;
        match kind(c, node) {
            NodeKind::Annotation
            | NodeKind::BinaryExpression
            | NodeKind::IndexExpression
            | NodeKind::PatternField
            | NodeKind::RedirectingConstructorInvocation
            | NodeKind::SuperConstructorInvocation => {
                if let Some(e) = self.element(node) {
                    self.add_declaration(e);
                }
            }
            NodeKind::AssignmentExpression => self.visit_compound_assignment_expression(node),
            NodeKind::PostfixExpression | NodeKind::PrefixExpression => {
                if let Some(e) = self.element(node) {
                    self.add_declaration(e);
                }
                self.visit_compound_assignment_expression(node);
            }
            NodeKind::ClassDeclaration => self.visit_class_declaration(node),
            NodeKind::ConstantPattern => {
                self.pattern_level += 1;
                self.visit_children(node);
                self.pattern_level -= 1;
                return;
            }
            NodeKind::ConstructorDeclaration => {
                let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
                let has_super_initializer = c
                    .ast
                    .list_raw(n.initializers)
                    .iter()
                    .any(|&i| kind(c, i) == NodeKind::SuperConstructorInvocation);
                if !has_super_initializer
                    && let Some(class) = c.ast.parent(node).and_then(|p| c.ast.parent(p))
                    && kind(c, class) == NodeKind::ClassDeclaration
                {
                    self.add_default_super_constructor_declaration(class);
                }
            }
            NodeKind::ConstructorName => {
                if let Some(e) = self.element(node)
                    && self.pattern_level == 0
                {
                    self.add_declaration(e);
                    let named_type = c.ast[Id::<ConstructorName>::from_raw(node)].type_;
                    if let Some(t) = self.element(named_type.raw()) {
                        self.add_declaration(t);
                    }
                }
            }
            NodeKind::ExtensionTypeDeclaration => {
                if let Some(clause) =
                    c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].implements_clause
                {
                    for &t in c.ast.list(c.ast[clause].interfaces) {
                        self.add_named_type(t.raw());
                    }
                }
            }
            NodeKind::MethodDeclaration => {
                let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
                if lexeme(c, n.name) == "toJson"
                    && !n.modifier_keyword.is_some_and(|k| lexeme(c, k) == "static")
                    && let Some(element) = c.declared_element(node)
                {
                    self.add_declaration(ElemRef::Base(element));
                }
            }
            NodeKind::NamedType => {
                self.visit_named_type(node);
                return;
            }
            NodeKind::SimpleIdentifier => {
                if !in_declaration_context(c, node)
                    && let Some(e) = self.element(node)
                {
                    self.add_declaration(e);
                }
            }
            NodeKind::VariableDeclaration => {
                if let Some(list) = c
                    .ast
                    .parent(node)
                    .and_then(|p| c.ast.cast::<VariableDeclarationList>(p))
                    && let Some(ty) = c.ast[list].type_
                {
                    self.visit(ty.raw());
                }
            }
            _ => {}
        }
        self.visit_children(node);
    }

    fn visit_class_declaration(&mut self, node: NodeId) {
        let c = self.c;
        let n = &c.ast[Id::<ClassDeclaration>::from_raw(node)];
        if let Some(clause) = n.extends_clause {
            self.add_named_type(c.ast[clause].superclass.raw());
        }
        if let Some(clause) = n.with_clause {
            for &t in c.ast.list(c.ast[clause].mixin_types) {
                self.add_named_type(t.raw());
            }
        }
        if let Some(clause) = n.implements_clause {
            for &t in c.ast.list(c.ast[clause].interfaces) {
                self.add_named_type(t.raw());
            }
        }
        let Some(element) = c.declared_element(node) else {
            return;
        };
        let has_constructors = c.ast.cast::<BlockClassBody>(n.body.raw()).is_some_and(|b| {
            c.ast
                .list_raw(c.ast[b].members)
                .iter()
                .any(|&m| kind(c, m) == NodeKind::ConstructorDeclaration)
        });
        if !has_constructors {
            self.add_default_super_constructor_declaration(node);
        }
        let Some(ctx) = rctx(c) else { return };
        let Some(metadata) = c.resolved.and_then(|r| r.metadata) else {
            return;
        };
        for annotation in metadata.annotations(element) {
            let is_reflective_test = metadata.annotation_element(annotation).is_some_and(|a| {
                a.tag() == Tag::Getter
                    && ctx.element_name(a) == Some("reflectiveTest")
                    && dartr_typesystem::member::library(&ctx, ElemRef::Base(a)).is_some_and(|l| {
                        ctx.library_uri(l)
                            == "package:test_reflective_loader/test_reflective_loader.dart"
                    })
            });
            if is_reflective_test
                && let Some(interface) = element.cast::<InterfaceElement>()
                && let Some(&constructor) = ctx
                    .interface(interface)
                    .constructors
                    .iter()
                    .find(|k| name(c, k.raw()) == Some("new"))
            {
                self.add_declaration(ElemRef::Base(constructor.raw()));
            }
        }
    }

    fn visit_named_type(&mut self, node: NodeId) {
        let c = self.c;
        let Some(element) = self.element(node) else {
            return;
        };
        let n = &c.ast[Id::<NamedType>::from_raw(node)];
        let ty = annotation_type(c, node);
        let ctx = rctx(c);
        let has_alias = ty
            .zip(ctx)
            .is_some_and(|(t, ctx)| ctx.type_alias(t).is_some());
        let is_extension_type = ty
            .zip(ctx)
            .and_then(|(t, ctx)| ctx.interface_element(t))
            .is_some_and(|e| e.raw().tag() == Tag::ExtensionType);
        let in_type_argument =
            this_or_ancestor(c, node, |a| kind(c, a) == NodeKind::TypeArgumentList).is_some();
        if has_alias
            || is_extension_type
            || c.ast
                .parent(node)
                .is_some_and(|p| kind(c, p) == NodeKind::TypeLiteral)
            || in_type_argument
            || is_in_external_variable_type_or_function_return_type(c, node)
        {
            self.add_declaration(element);
        }
        if let Some(arguments) = n.type_arguments {
            for &argument in c.ast.list_raw(c.ast[arguments].arguments) {
                self.visit(argument);
            }
        }
    }

    /// Dart `_addDeclaration`.
    fn add_declaration(&mut self, element: ElemRef) {
        let c = self.c;
        let element = base(c, element);
        let mut top = Some(element);
        while let Some(e) = top {
            let parent = enclosing(c, e);
            if parent.is_none_or(|p| p.tag() == Tag::Library) {
                break;
            }
            top = parent;
        }
        if let Some(top) = top
            && let Some(&declaration) = self.declaration_map.get(&top)
        {
            self.declarations.insert(declaration);
        }
        if is_private(c, element) {
            return;
        }
        let Some(enclosing_element) = enclosing(c, element) else {
            return;
        };
        if is_private(c, enclosing_element) {
            return;
        }
        if (enclosing_element.cast::<InterfaceElement>().is_some()
            || matches!(enclosing_element.tag(), Tag::Extension | Tag::ExtensionType))
            && let Some(&declaration) = self.declaration_map.get(&element)
        {
            self.declarations.insert(declaration);
        }
    }

    /// Dart `_addDefaultSuperConstructorDeclaration`.
    fn add_default_super_constructor_declaration(&mut self, class: NodeId) {
        let c = self.c;
        let Some(ctx) = rctx(c) else { return };
        let Some(element) = c
            .declared_element(class)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        let Some(supertype) = ctx.element_supertype(element) else {
            return;
        };
        let Some(super_element) = ctx.interface_element(supertype) else {
            return;
        };
        let constructor = ctx
            .interface(super_element)
            .constructors
            .iter()
            .find(|k| name(c, k.raw()) == Some("new"))
            .copied();
        if let Some(constructor) = constructor {
            self.add_declaration(ElemRef::Base(constructor.raw()));
        }
    }

    /// Dart `_addNamedType`.
    fn add_named_type(&mut self, node: NodeId) {
        let Some(element) = self.element(node) else {
            return;
        };
        if let Some(&declaration) = self.declaration_map.get(&base(self.c, element)) {
            self.declarations.insert(declaration);
        }
    }

    /// Dart `_visitCompoundAssignmentExpression`.
    fn visit_compound_assignment_expression(&mut self, node: NodeId) {
        let Some(resolved) = self.c.resolved else {
            return;
        };
        if let Some(&read) = resolved.tables.read_element.get(node) {
            self.add_declaration(read);
        }
        if let Some(&write) = resolved.tables.write_element.get(node) {
            self.add_declaration(write);
        }
    }
}

/// Dart `SimpleIdentifier.inDeclarationContext()`.
fn in_declaration_context(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(parent) = c.ast.parent(node) else {
        return false;
    };
    match kind(c, parent) {
        NodeKind::ImportDirective => c.ast[Id::<ImportDirective>::from_raw(parent)]
            .prefix
            .is_some_and(|p| p.raw() == node),
        NodeKind::Label => c.ast.parent(parent).is_some_and(|g| {
            Statement::test(kind(c, g))
                || matches!(
                    kind(c, g),
                    NodeKind::SwitchCase | NodeKind::SwitchDefault | NodeKind::SwitchPatternCase
                )
        }),
        _ => false,
    }
}

/// Dart `NamedType.isInExternalVariableTypeOrFunctionReturnType`.
fn is_in_external_variable_type_or_function_return_type(
    c: &LinterContext<'_>,
    node: NodeId,
) -> bool {
    let mut top = node;
    let mut parent = c.ast.parent(node);
    while let Some(p) = parent
        && TypeAnnotation::test(kind(c, p))
    {
        top = p;
        parent = c.ast.parent(p);
    }
    let Some(parent) = parent else { return false };
    if let Some(method) = c.ast.cast::<MethodDeclaration>(parent) {
        let m = &c.ast[method];
        return m.external_keyword.is_some() && m.return_type.map(|t| t.raw()) == Some(top);
    }
    if kind(c, parent) == NodeKind::VariableDeclarationList
        && let Some(grandparent) = c.ast.parent(parent)
    {
        if let Some(field) = c.ast.cast::<FieldDeclaration>(grandparent) {
            return c.ast[field].external_keyword.is_some();
        }
        if let Some(variable) = c.ast.cast::<TopLevelVariableDeclaration>(grandparent) {
            return c.ast[variable].external_keyword.is_some();
        }
    }
    false
}
