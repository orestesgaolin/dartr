// Dart source: pkg/linter/lib/src/rules/public_member_api_docs.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, EId, ElementFlags, FragmentFlags, Tag};
use dartr_typesystem::TypeExt;
use indexmap::{IndexMap, IndexSet};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    // `package.canHavePublicApi` is false only for a basic workspace (no
    // pubspec), which `isInLibDir` already excludes.
    if !c.is_in_lib_dir() {
        return;
    }
    const NAME: &str = "public_member_api_docs";
    r.add(NodeKind::ClassDeclaration, NAME, visit_class_declaration);
    r.add(NodeKind::ClassTypeAlias, NAME, visit_named);
    r.add(NodeKind::CompilationUnit, NAME, visit_compilation_unit);
    r.add(NodeKind::ConstructorDeclaration, NAME, visit_constructor_declaration);
    r.add(NodeKind::EnumConstantDeclaration, NAME, visit_enum_constant_declaration);
    r.add(NodeKind::EnumDeclaration, NAME, visit_enum_declaration);
    r.add(NodeKind::ExtensionDeclaration, NAME, visit_extension_declaration);
    r.add(NodeKind::ExtensionTypeDeclaration, NAME, visit_extension_type_declaration);
    r.add(NodeKind::FieldDeclaration, NAME, visit_field_declaration);
    r.add(NodeKind::FunctionTypeAlias, NAME, visit_named);
    r.add(NodeKind::GenericTypeAlias, NAME, visit_named);
    r.add(NodeKind::MixinDeclaration, NAME, visit_mixin_declaration);
    r.add(NodeKind::PrimaryConstructorBody, NAME, visit_primary_constructor_body);
    r.add(NodeKind::PrimaryConstructorDeclaration, NAME, visit_primary_constructor_declaration);
    r.add(NodeKind::TopLevelVariableDeclaration, NAME, visit_top_level_variable_declaration);
}

fn is_private(c: &LinterContext<'_>, token: dartr_syntax::TokenId) -> bool {
    lexeme(c, token).starts_with('_')
}

/// Dart `AnnotatedNode.documentationComment` (with the
/// `VariableDeclaration` fallback to its grandparent).
fn documentation_comment(c: &LinterContext<'_>, node: NodeId) -> Option<NodeId> {
    macro_rules! doc {
        ($($t:ident),*) => {
            match kind(c, node) {
                $(NodeKind::$t => c.ast[Id::<$t>::from_raw(node)].documentation_comment.map(|d| d.raw()),)*
                _ => None,
            }
        };
    }
    let comment = doc!(
        ClassDeclaration,
        ClassTypeAlias,
        ConstructorDeclaration,
        EnumConstantDeclaration,
        EnumDeclaration,
        ExtensionDeclaration,
        ExtensionTypeDeclaration,
        FieldDeclaration,
        FunctionDeclaration,
        FunctionTypeAlias,
        GenericTypeAlias,
        MethodDeclaration,
        MixinDeclaration,
        PrimaryConstructorBody,
        TopLevelVariableDeclaration,
        VariableDeclaration
    );
    if comment.is_none() && kind(c, node) == NodeKind::VariableDeclaration {
        let grandparent = c.ast.parent(node).and_then(|p| c.ast.parent(p))?;
        if matches!(kind(c, grandparent), NodeKind::FieldDeclaration | NodeKind::TopLevelVariableDeclaration) {
            return documentation_comment(c, grandparent);
        }
    }
    comment
}

/// Dart `AstNodeExtension.isInternal`.
fn is_internal(c: &LinterContext<'_>, node: NodeId) -> bool {
    if kind(c, node) == NodeKind::VariableDeclaration
        && let Some(element) = c.declared_element(node)
        && element.tag() == Tag::TopLevelVariable
    {
        return c.has_package_meta_getter(element, "internal");
    }
    let Some(parent) = this_or_ancestor(c, node, |n| CompilationUnitMember::test(kind(c, n))) else {
        return false;
    };
    if kind(c, parent) == NodeKind::TopLevelVariableDeclaration {
        return false;
    }
    c.declared_element(parent).is_some_and(|e| c.has_package_meta_getter(e, "internal"))
}

/// Dart `AstNodeExtension.inPrivateMember`.
fn in_private_member(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(parent) = c.ast.parent(node).and_then(|p| c.ast.parent(p)) else {
        return false;
    };
    match kind(c, parent) {
        NodeKind::ClassDeclaration | NodeKind::EnumDeclaration | NodeKind::ExtensionTypeDeclaration => {
            type_name(c, parent).is_some_and(|t| is_private(c, t))
        }
        NodeKind::ExtensionDeclaration => {
            c.ast[Id::<ExtensionDeclaration>::from_raw(parent)].name.is_none_or(|t| is_private(c, t))
        }
        NodeKind::MixinDeclaration => is_private(c, c.ast[Id::<MixinDeclaration>::from_raw(parent)].name),
        _ => false,
    }
}

/// Dart `AstNodeExtension.isEffectivelyPrivate`.
fn is_effectively_private(c: &LinterContext<'_>, node: NodeId) -> bool {
    if is_internal(c, node) {
        return true;
    }
    if kind(c, node) == NodeKind::ClassDeclaration
        && let Some(element) = c.declared_element(node)
        && let Some(ctx) = rctx(c)
    {
        let class = ctx.get(EId::<ClassElement>::from_raw(element));
        let element_flags = class.flags.get();
        if flags(c, element).contains(FragmentFlags::CLASS_FRAGMENT_IS_SEALED) {
            return true;
        }
        if element_flags.contains(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
            && (element_flags.contains(ElementFlags::CLASS_ELEMENT_IS_FINAL)
                || element_flags.contains(ElementFlags::CLASS_ELEMENT_IS_INTERFACE))
        {
            return true;
        }
    }
    false
}

/// `namePart.typeName` of a class, enum or extension type declaration.
fn type_name(c: &LinterContext<'_>, node: NodeId) -> Option<dartr_syntax::TokenId> {
    let name_part = match kind(c, node) {
        NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(node)].name_part.raw(),
        NodeKind::EnumDeclaration => c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part.raw(),
        NodeKind::ExtensionTypeDeclaration => c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].name_part.raw(),
        _ => return None,
    };
    if let Some(n) = c.ast.cast::<NameWithTypeParameters>(name_part) {
        Some(c.ast[n].type_name)
    } else {
        c.ast.cast::<PrimaryConstructorDeclaration>(name_part).map(|p| c.ast[p].type_name)
    }
}

/// Dart `_Visitor.isOverridingMember`.
fn is_overriding_member(c: &LinterContext<'_>, node: NodeId) -> bool {
    c.declared_element(node).is_some_and(|e| overridden_member(c, e).is_some())
}

/// Dart `_Visitor.check`.
fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) -> bool {
    if is_internal(c, node) {
        return false;
    }
    if documentation_comment(c, node).is_none() && !is_overriding_member(c, node) {
        let (offset, length) = node_to_annotate(c, node);
        c.report_offset(out, &diag::PUBLIC_MEMBER_API_DOCS, offset, length, &[]);
        return true;
    }
    false
}

fn property_keyword<'a>(c: &'a LinterContext<'a>, node: NodeId) -> Option<&'a str> {
    match kind(c, node) {
        NodeKind::MethodDeclaration => c.ast[Id::<MethodDeclaration>::from_raw(node)].property_keyword,
        NodeKind::FunctionDeclaration => c.ast[Id::<FunctionDeclaration>::from_raw(node)].property_keyword,
        _ => None,
    }
    .map(|k| lexeme(c, k))
}

fn member_name(c: &LinterContext<'_>, node: NodeId) -> dartr_syntax::TokenId {
    match kind(c, node) {
        NodeKind::MethodDeclaration => c.ast[Id::<MethodDeclaration>::from_raw(node)].name,
        _ => c.ast[Id::<FunctionDeclaration>::from_raw(node)].name,
    }
}

/// Dart `_Visitor.checkMethods` and the function part of
/// `visitCompilationUnit`: getters, then setters of undocumented getters,
/// then the remaining members.
fn check_accessor_groups(c: &LinterContext<'_>, members: Vec<NodeId>, out: &mut Vec<Diagnostic>) {
    let mut getters: IndexMap<&str, NodeId> = IndexMap::new();
    let mut setters = Vec::new();
    let mut methods = Vec::new();
    for member in members {
        let name = lexeme(c, member_name(c, member));
        match property_keyword(c, member) {
            Some("get") => {
                getters.insert(name, member);
            }
            Some("set") => setters.push(member),
            _ => methods.push(member),
        }
    }
    let mut missing_docs = IndexSet::new();
    for &getter in getters.values() {
        if check(c, getter, out) {
            missing_docs.insert(getter);
        }
    }
    for setter in setters {
        let name = lexeme(c, member_name(c, setter));
        if let Some(getter) = getters.get(name)
            && missing_docs.contains(getter)
        {
            check(c, setter, out);
        }
    }
    for method in methods {
        check(c, method, out);
    }
}

fn check_methods(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let members = super::sort_unnamed_constructors_first::body_members(c, node)
        .into_iter()
        .filter(|&m| {
            c.ast.cast::<MethodDeclaration>(m).is_some_and(|m| !is_private(c, c.ast[m].name))
        })
        .collect();
    check_accessor_groups(c, members, out);
}

fn visit_members(c: &LinterContext<'_>, node: NodeId, name: dartr_syntax::TokenId, out: &mut Vec<Diagnostic>) {
    if is_private(c, name) || is_internal(c, node) {
        return;
    }
    check(c, node, out);
    check_methods(c, node, out);
}

fn visit_class_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.declared_element(node).is_none() {
        return;
    }
    let Some(name) = type_name(c, node) else { return };
    visit_members(c, node, name, out);
}

fn visit_extension_type_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    visit_class_declaration(c, node, out);
}

fn visit_mixin_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let name = c.ast[Id::<MixinDeclaration>::from_raw(node)].name;
    visit_members(c, node, name, out);
}

/// Dart `visitClassTypeAlias`, `visitFunctionTypeAlias`, `visitGenericTypeAlias`.
fn visit_named(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let name = match kind(c, node) {
        NodeKind::ClassTypeAlias => c.ast[Id::<ClassTypeAlias>::from_raw(node)].name,
        NodeKind::FunctionTypeAlias => c.ast[Id::<FunctionTypeAlias>::from_raw(node)].name,
        _ => c.ast[Id::<GenericTypeAlias>::from_raw(node)].name,
    };
    if !is_private(c, name) {
        check(c, node, out);
    }
}

fn visit_compilation_unit(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let members = c
        .ast
        .list_raw(c.ast[Id::<CompilationUnit>::from_raw(node)].declarations)
        .iter()
        .copied()
        .filter(|&m| {
            c.ast.cast::<FunctionDeclaration>(m).is_some_and(|f| {
                let name = c.ast[f].name;
                !is_private(c, name) && lexeme(c, name) != "main"
            })
        })
        .collect();
    check_accessor_groups(c, members, out);
}

fn visit_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
    if in_private_member(c, node) || n.name.is_some_and(|t| is_private(c, t)) {
        return;
    }
    let parent = c.ast.parent(node).and_then(|p| c.ast.parent(p));
    if parent.is_some_and(|p| kind(c, p) == NodeKind::EnumDeclaration) {
        return;
    }
    if parent.is_some_and(|p| is_effectively_private(c, p)) {
        return;
    }
    check(c, node, out);
}

fn visit_enum_constant_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let name = c.ast[Id::<EnumConstantDeclaration>::from_raw(node)].name;
    if !in_private_member(c, node) && !is_private(c, name) {
        check(c, node, out);
    }
}

fn visit_enum_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if type_name(c, node).is_none_or(|t| is_private(c, t)) {
        return;
    }
    check(c, node, out);
    check_methods(c, node, out);
}

fn visit_extension_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<ExtensionDeclaration>::from_raw(node)];
    if n.name.is_none_or(|t| is_private(c, t)) || is_internal(c, node) {
        return;
    }
    check(c, node, out);
    check_methods(c, node, out);
}

fn visit_field_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
    if is_internal(c, node) || in_private_member(c, node) {
        return;
    }
    // Dart `FieldDeclarationExtension.isInvalidExtensionTypeField`.
    let grandparent = c.ast.parent(node).and_then(|p| c.ast.parent(p));
    if n.static_keyword.is_none()
        && grandparent.is_some_and(|g| kind(c, g) == NodeKind::ExtensionTypeDeclaration)
    {
        return;
    }
    for &field in c.ast.list(c.ast[n.fields].variables) {
        if !is_private(c, c.ast[field].name) {
            check(c, field.raw(), out);
        }
    }
}

fn visit_primary_constructor_body(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if in_private_member(c, node) {
        return;
    }
    let grandparent = c.ast.parent(node).and_then(|p| c.ast.parent(p));
    if !grandparent.is_some_and(|g| kind(c, g) == NodeKind::ClassDeclaration) {
        return;
    }
    check(c, node, out);
}

fn visit_primary_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if super::tighten_type_of_initializing_formals::primary_constructor_body(c, node).is_some() {
        return;
    }
    let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
    if is_private(c, n.type_name) {
        return;
    }
    let constructor_name = n.constructor_name.map(|cn| c.ast[cn].name);
    if constructor_name.is_some_and(|t| is_private(c, t)) {
        return;
    }
    if !c.ast.parent(node).is_some_and(|p| kind(c, p) == NodeKind::ClassDeclaration) {
        return;
    }
    c.report_token(out, &diag::PUBLIC_MEMBER_API_DOCS, constructor_name.unwrap_or(n.type_name), &[]);
}

fn visit_top_level_variable_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let variables = c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables;
    for &variable in c.ast.list(c.ast[variables].variables) {
        if !is_private(c, c.ast[variable].name) {
            check(c, variable.raw(), out);
        }
    }
}
