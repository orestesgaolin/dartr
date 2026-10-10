// Dart source: pkg/analyzer/lib/src/error/member_duplicate_definition_verifier.dart

//! `MemberDuplicateDefinitionVerifier`: members of classes, enums, mixins,
//! extensions and extension types with the same name, duplicate
//! constructors, constructors with the name of a static member, static
//! members with the name of an instance member, and `values` in enums.
//!
//! [`check_library`] is the library-wide step (Dart
//! `MemberDuplicateDefinitionVerifier.checkLibrary`); [`check_unit`] and
//! [`check_unit_static`] are its two passes over one unit, with the
//! [`DuplicationDefinitionContext`] of the library.

use dartr_ast::{
    Ast, ClassDeclaration, ClassMember, CompilationUnit, ConstructorDeclaration, EnumDeclaration,
    ExtensionDeclaration, ExtensionTypeDeclaration, FieldDeclaration, Id, MethodDeclaration,
    MixinDeclaration, NodeId, PrimaryConstructorDeclaration,
};
use dartr_diagnostics::diag;
use dartr_element::{
    ClassFragment, Ctx, EId, ElemRef, ElementId, EnumFragment, ExtensionFragment,
    ExtensionTypeFragment, FId, FieldElement, FormalParameterElement, FragmentFlags, FragmentId,
    InstanceFragmentData, InterfaceElement, MixinFragment, Tag,
};
use dartr_syntax::TokenId;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name as MemberName};
use dartr_typesystem::member;
use dartr_typesystem::type_ext::TypeExt;
use indexmap::{IndexMap, IndexSet};

use super::correct_override::{
    declared_fragment, diagnostic_range, duplicate_definition, first_fragment,
    first_fragment_flags, fragment_element, fragment_flags, is_augmentation,
    is_augmentation_without_augmented_declaration, is_wildcard_variable, lookup_name,
    token_is_synthetic, token_range,
};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;

/// Dart `DuplicationDefinitionContext`: the scopes of each instance
/// element of the library (by its first fragment), shared by the units.
#[derive(Default)]
pub struct DuplicationDefinitionContext {
    /// `_instanceElementContexts`.
    instance_element_contexts: IndexMap<FragmentId, InstanceElementContext>,
}

/// Dart `_InstanceElementContext`.
#[derive(Default)]
struct InstanceElementContext {
    constructor_names: IndexSet<String>,
    instance_scope: IndexMap<String, ScopeEntry>,
    static_scope: IndexMap<String, ScopeEntry>,
}

/// Dart `_ScopeEntry`.
#[derive(Clone, Copy)]
enum ScopeEntry {
    /// `_ScopeEntryElement`.
    Element(ElementId),
    /// `_ScopeEntryGetterSetterPair`.
    GetterSetterPair {
        getter: ElementId,
        setter: ElementId,
    },
}

/// Which scope of an [`InstanceElementContext`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Instance,
    Static,
}

impl InstanceElementContext {
    fn scope(&mut self, kind: ScopeKind) -> &mut IndexMap<String, ScopeEntry> {
        match kind {
            ScopeKind::Instance => &mut self.instance_scope,
            ScopeKind::Static => &mut self.static_scope,
        }
    }
}

/// Dart `MemberDuplicateDefinitionVerifier.checkLibrary(...)`: all units of
/// the library, the instance members first, then the static members.
pub fn check_library(units: &mut [UnitVerifier<'_>]) {
    let mut context = DuplicationDefinitionContext::default();
    // Check all instance members.
    for v in units.iter_mut() {
        let unit = v.unit;
        check_unit(v, &mut context, unit);
    }
    // Check all static members.
    for v in units.iter_mut() {
        let unit = v.unit;
        check_unit_static(v, &mut context, unit);
    }
}

/// Dart `_checkUnit(node)`.
pub fn check_unit<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<CompilationUnit>,
) {
    let declarations = host.ast().list(host.ast()[node].declarations).to_vec();
    for declaration in declarations {
        let ast = host.ast();
        let d = declaration.raw();
        if let Some(n) = ast.cast::<ClassDeclaration>(d) {
            check_class(host, context, n);
        } else if let Some(n) = ast.cast::<ExtensionDeclaration>(d) {
            check_extension(host, context, n);
        } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
            check_enum(host, context, n);
        } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(d) {
            check_extension_type(host, context, n);
        } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
            check_mixin(host, context, n);
        }
    }
}

/// Dart `_checkUnitStatic(node)`.
pub fn check_unit_static<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<CompilationUnit>,
) {
    let declarations = host.ast().list(host.ast()[node].declarations).to_vec();
    for declaration in declarations {
        let ast = host.ast();
        let d = declaration.raw();
        if let Some(n) = ast.cast::<ClassDeclaration>(d) {
            let members = class_body_members(ast, ast[n].body.raw());
            if let Some(fragment) = declared_fragment(host, n) {
                check_class_static(host, context, fragment, members);
            }
        } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
            check_enum_static(host, n);
        } else if let Some(n) = ast.cast::<ExtensionDeclaration>(d) {
            check_extension_static(host, context, n);
        } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(d) {
            let members = class_body_members(ast, ast[n].body.raw());
            if let Some(fragment) = declared_fragment(host, n) {
                check_class_static(host, context, fragment, members);
            }
        } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
            let members = class_body_members(ast, ast[n].body.raw());
            if let Some(fragment) = declared_fragment(host, n) {
                check_class_static(host, context, fragment, members);
            }
        }
    }
}

/// The members of a `ClassBody` / `EnumBody` (empty for an empty body).
pub(crate) fn class_body_members(ast: &Ast, body: NodeId) -> Vec<Id<ClassMember>> {
    if let Some(b) = ast.cast::<dartr_ast::BlockClassBody>(body) {
        return ast.list(ast[b].members).to_vec();
    }
    if let Some(b) = ast.cast::<dartr_ast::BlockEnumBody>(body) {
        return ast.list(ast[b].members).to_vec();
    }
    Vec::new()
}

/// The constants of an `EnumBody`.
fn enum_body_constants(ast: &Ast, body: NodeId) -> Vec<Id<dartr_ast::EnumConstantDeclaration>> {
    match ast.cast::<dartr_ast::BlockEnumBody>(body) {
        Some(b) => ast.list(ast[b].constants).to_vec(),
        None => Vec::new(),
    }
}

/// Dart `namePart.tryCast<PrimaryConstructorDeclaration>()`.
fn primary_constructor(ast: &Ast, name_part: NodeId) -> Option<Id<PrimaryConstructorDeclaration>> {
    ast.cast::<PrimaryConstructorDeclaration>(name_part)
}

/// The element of the first fragment of [fragment]'s element (Dart
/// `fragment.element.firstFragment`).
fn element_first_fragment(ctx: &Ctx<'_>, fragment: FragmentId) -> FragmentId {
    fragment_element(ctx, fragment)
        .and_then(|e| first_fragment(ctx, e))
        .unwrap_or(fragment)
}

/// The data of an instance fragment (class, enum, mixin, extension,
/// extension type).
fn instance_fragment<'a>(ctx: &Ctx<'a>, f: FragmentId) -> Option<&'a InstanceFragmentData> {
    Some(match f.tag() {
        Tag::Class => {
            &ctx.fragment(FId::<ClassFragment>::from_raw(f))
                .interface
                .instance
        }
        Tag::Enum => {
            &ctx.fragment(FId::<EnumFragment>::from_raw(f))
                .interface
                .instance
        }
        Tag::Mixin => {
            &ctx.fragment(FId::<MixinFragment>::from_raw(f))
                .interface
                .instance
        }
        Tag::ExtensionType => {
            &ctx.fragment(FId::<ExtensionTypeFragment>::from_raw(f))
                .interface
                .instance
        }
        Tag::Extension => &ctx.fragment(FId::<ExtensionFragment>::from_raw(f)).instance,
        _ => return None,
    })
}

/// Dart `firstFragment is InterfaceFragmentImpl`.
fn is_interface_fragment(f: FragmentId) -> bool {
    matches!(
        f.tag(),
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
    )
}

/// `_checkClass(node)`.
fn check_class<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<ClassDeclaration>,
) {
    let ast = host.ast();
    let members = class_body_members(ast, ast[node].body.raw());
    let primary = primary_constructor(ast, ast[node].name_part.raw());
    if let Some(fragment) = declared_fragment(host, node) {
        check_class_members(host, context, fragment, &members, primary);
    }
}

/// Dart `_checkClassMembers(fragment, members, primaryConstructor:)`:
/// there are no members with the same name.
pub fn check_class_members<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    fragment: FragmentId,
    members: &[Id<ClassMember>],
    primary_constructor: Option<Id<PrimaryConstructorDeclaration>>,
) {
    let ctx = host.ctx();
    let first_fragment = element_first_fragment(&ctx, fragment);
    let first_fragment_name = ctx
        .fragment_data(first_fragment)
        .and_then(|f| f.name)
        .map(|n| ctx.name_str(n).to_string());
    context
        .instance_element_contexts
        .entry(first_fragment)
        .or_default();

    if let Some(primary_constructor) = primary_constructor {
        if let Some(element) =
            declared_fragment(host, primary_constructor).and_then(|f| fragment_element(&ctx, f))
            && let Some(name) = ctx.element_name(element)
        {
            element_context(context, first_fragment)
                .constructor_names
                .insert(name.to_string());
        }
        let ast = host.ast();
        let formals = ast
            .list(ast[ast[primary_constructor].formal_parameters].parameters)
            .to_vec();
        for formal_node in formals {
            let Some(formal_fragment) = declared_fragment(host, formal_node) else {
                continue;
            };
            if formal_fragment.tag() != Tag::FieldFormalParameter
                || !fragment_flags(&ctx, formal_fragment)
                    .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
            {
                continue;
            }
            let field_name = formal_parameter_parts(host.ast(), formal_node.raw()).name;
            let field_element = fragment_element(&ctx, formal_fragment)
                .and_then(|e| e.cast::<FormalParameterElement>())
                .and_then(|p| ctx.get(p).field.get());
            let (Some(field_name), Some(field_element)) = (field_name, field_element) else {
                continue;
            };
            let field = ctx.get(field_element);
            if let Some(getter) = field.getter {
                let getter_fragment = ctx.get(getter).first_fragment().raw();
                check_duplicate_identifier(
                    host,
                    context,
                    first_fragment,
                    ScopeKind::Instance,
                    field_name,
                    getter_fragment,
                    Some(formal_fragment),
                );
            }
            if let Some(setter) = field.setter {
                let setter_fragment = ctx.get(setter).first_fragment().raw();
                check_duplicate_identifier(
                    host,
                    context,
                    first_fragment,
                    ScopeKind::Instance,
                    field_name,
                    setter_fragment,
                    Some(formal_fragment),
                );
            }
        }
    }

    for &member in members {
        let ast = host.ast();
        let m = member.raw();
        if let Some(c) = ast.cast::<ConstructorDeclaration>(m) {
            // Augmentations are not declarations, can have multiple.
            if ast[c].augment_keyword.is_some() {
                continue;
            }
            // Skip if the typeName is wrong.
            if let Some(type_name) = ast[c].type_name {
                let type_name = ast.tokens.lexeme(ast[type_name].token);
                if Some(type_name) != first_fragment_name.as_deref() {
                    continue;
                }
            }
            let name = match ast[c].name {
                Some(name) => ast.tokens.lexeme(name).to_string(),
                None => "new".to_string(),
            };
            let (offset, length) = constructor_error_range(ast, c);
            if !element_context(context, first_fragment)
                .constructor_names
                .insert(name.clone())
            {
                let d = if name == "new" {
                    diag::duplicate_constructor_default()
                } else {
                    diag::duplicate_constructor_name(&name)
                };
                host.report(d.at_offset(offset, length));
            }
        } else if let Some(f) = ast.cast::<FieldDeclaration>(m) {
            let is_static = ast[f].static_keyword.is_some();
            let scope = if is_static {
                ScopeKind::Static
            } else {
                ScopeKind::Instance
            };
            let fields = ast[f].fields;
            let variables = ast.list(ast[fields].variables).to_vec();
            for field in variables {
                let Some(field_fragment) = declared_fragment(host, field) else {
                    continue;
                };
                if is_augmentation(&ctx, field_fragment) {
                    continue;
                }
                let Some(field_element) =
                    fragment_element(&ctx, field_fragment).and_then(|e| e.cast::<FieldElement>())
                else {
                    continue;
                };
                let name = host.ast()[field].name;
                let data = ctx.get(field_element);
                if let Some(getter) = data.getter {
                    let getter_fragment = ctx.get(getter).first_fragment().raw();
                    check_duplicate_identifier(
                        host,
                        context,
                        first_fragment,
                        scope,
                        name,
                        getter_fragment,
                        Some(field_fragment),
                    );
                }
                if let Some(setter) = data.setter {
                    let setter_fragment = ctx.get(setter).first_fragment().raw();
                    check_duplicate_identifier(
                        host,
                        context,
                        first_fragment,
                        scope,
                        name,
                        setter_fragment,
                        Some(field_fragment),
                    );
                }
                if fragment.tag() == Tag::Enum {
                    check_values_declaration_in_enum(host, name);
                }
            }
        } else if let Some(md) = ast.cast::<MethodDeclaration>(m) {
            let is_static = is_keyword(ast, ast[md].modifier_keyword, "static");
            let is_setter = is_keyword(ast, ast[md].property_keyword, "set");
            let name = ast[md].name;
            let Some(method_fragment) = declared_fragment(host, md) else {
                continue;
            };
            let scope = if is_static {
                ScopeKind::Static
            } else {
                ScopeKind::Instance
            };
            check_duplicate_identifier(
                host,
                context,
                first_fragment,
                scope,
                name,
                method_fragment,
                None,
            );
            if fragment.tag() == Tag::Enum && !(is_static && is_setter) {
                check_values_declaration_in_enum(host, name);
            }
        }
        // `PrimaryConstructorBody`: not an actual declaration.
    }

    if is_interface_fragment(first_fragment) {
        check_conflicting_constructor_and_static(host, context, first_fragment);
    }
}

/// Whether [token] is the keyword [keyword].
fn is_keyword(ast: &Ast, token: Option<TokenId>, keyword: &str) -> bool {
    token.is_some_and(|t| ast.tokens.lexeme(t) == keyword)
}

/// Dart `ConstructorDeclaration.errorRange`.
fn constructor_error_range(ast: &Ast, node: Id<ConstructorDeclaration>) -> (usize, usize) {
    let c = &ast[node];
    let start = match c.type_name {
        Some(t) => ast.offset(t) as usize,
        None => match c.new_keyword.or(c.factory_keyword) {
            Some(t) => ast.tokens.get(t).offset as usize,
            None => ast.offset(node) as usize,
        },
    };
    let end = match c.name {
        Some(name) => ast.tokens.get(name).end() as usize,
        None => match c.type_name {
            Some(t) => ast.end(t) as usize,
            None => match c.new_keyword.or(c.factory_keyword) {
                Some(t) => ast.tokens.get(t).end() as usize,
                None => start,
            },
        },
    };
    (start, end.saturating_sub(start))
}

fn element_context(
    context: &mut DuplicationDefinitionContext,
    first_fragment: FragmentId,
) -> &mut InstanceElementContext {
    context
        .instance_element_contexts
        .entry(first_fragment)
        .or_default()
}

/// Dart `_checkClassStatic(fragment, members)`: local static members
/// conflicting with local instance members.
pub fn check_class_static<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    fragment: FragmentId,
    members: Vec<Id<ClassMember>>,
) {
    let ctx = host.ctx();
    let first_fragment = element_first_fragment(&ctx, fragment);
    let class_name = ctx
        .fragment_data(first_fragment)
        .and_then(|f| f.name)
        .map_or_else(String::new, |n| ctx.name_str(n).to_string());
    let mut conflicts = Vec::new();
    {
        let ast = host.ast();
        let element_context = element_context(context, first_fragment);
        let instance_scope = &element_context.instance_scope;
        for member in members {
            let m = member.raw();
            if let Some(f) = ast.cast::<FieldDeclaration>(m) {
                if ast[f].static_keyword.is_some() {
                    for &field in ast.list(ast[ast[f].fields].variables) {
                        let identifier = ast[field].name;
                        let name = ast.tokens.lexeme(identifier);
                        if instance_scope.contains_key(name) {
                            conflicts.push((identifier, name.to_string()));
                        }
                    }
                }
            } else if let Some(md) = ast.cast::<MethodDeclaration>(m)
                && is_keyword(ast, ast[md].modifier_keyword, "static")
            {
                let identifier = ast[md].name;
                let name = ast.tokens.lexeme(identifier);
                if instance_scope.contains_key(name) {
                    conflicts.push((identifier, name.to_string()));
                }
            }
        }
    }
    if !is_interface_fragment(first_fragment) {
        return;
    }
    for (identifier, name) in conflicts {
        let (offset, length) = token_range(host.ast(), identifier);
        host.report(
            diag::conflicting_static_and_instance(&class_name, &name, &class_name)
                .at_offset(offset, length),
        );
    }
}

/// Dart `_checkConflictingConstructorAndStatic(interfaceFragment:,
/// staticScope:)`.
fn check_conflicting_constructor_and_static<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    interface_fragment: FragmentId,
) {
    let ctx = host.ctx();
    let Some(element) =
        fragment_element(&ctx, interface_fragment).and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let static_scope = element_context(context, interface_fragment)
        .static_scope
        .clone();
    for &constructor in &ctx.interface(element).constructors {
        // `interfaceFragment.constructors`: the constructors of this
        // fragment.
        let constructor_fragment = ctx.get(constructor).first_fragment().raw();
        if ctx
            .fragment_data(constructor_fragment)
            .and_then(|f| f.enclosing_fragment)
            != Some(interface_fragment)
        {
            continue;
        }
        let Some(name) = ctx
            .fragment_data(constructor_fragment)
            .and_then(|f| f.name)
            .map(|n| ctx.name_str(n))
        else {
            continue;
        };
        // It is already an error to declare a member named 'new'.
        if name == "new" {
            continue;
        }
        let d = match static_scope.get(name) {
            None => continue,
            Some(ScopeEntry::Element(static_member)) => match static_member.tag() {
                Tag::Getter | Tag::Setter => {
                    if is_origin_variable(&ctx, *static_member) {
                        diag::conflicting_constructor_and_static_field(name)
                    } else if static_member.tag() == Tag::Getter {
                        diag::conflicting_constructor_and_static_getter(name)
                    } else {
                        diag::conflicting_constructor_and_static_setter(name)
                    }
                }
                Tag::Method => diag::conflicting_constructor_and_static_method(name),
                // Dart throws a `StateError`.
                _ => continue,
            },
            Some(ScopeEntry::GetterSetterPair { getter, .. }) => {
                if is_origin_variable(&ctx, *getter) {
                    diag::conflicting_constructor_and_static_field(name)
                } else {
                    diag::conflicting_constructor_and_static_getter(name)
                }
            }
        };
        let (offset, length) = diagnostic_range(&ctx, constructor.raw());
        host.report(d.at_offset(offset, length));
    }
}

/// Dart `PropertyAccessorElementImpl.isOriginVariable`.
fn is_origin_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e)
        .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
}

/// Dart `_checkDuplicateIdentifier(scope, identifier, fragment:,
/// originFragment:)`: whether [fragment] defined by [identifier] conflicts
/// with an element already in the scope.
fn check_duplicate_identifier<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    first_fragment: FragmentId,
    scope_kind: ScopeKind,
    identifier: TokenId,
    fragment: FragmentId,
    origin_fragment: Option<FragmentId>,
) {
    let ctx = host.ctx();
    let Some(element) = fragment_element(&ctx, fragment) else {
        return;
    };
    if token_is_synthetic(host.ast(), identifier) || is_wildcard_variable(&ctx, element) {
        return;
    }
    if is_augmentation(&ctx, fragment) {
        return;
    }
    let name = if fragment.tag() == Tag::Method {
        lookup_name(&ctx, element).unwrap_or_default()
    } else {
        host.ast().tokens.lexeme(identifier).to_string()
    };
    let scope = element_context(context, first_fragment).scope(scope_kind);
    let previous = match scope.get(&name).copied() {
        None => {
            scope.insert(name, ScopeEntry::Element(element));
            return;
        }
        Some(ScopeEntry::Element(previous))
            if previous.tag() == Tag::Getter && fragment.tag() == Tag::Setter =>
        {
            scope.insert(
                name,
                ScopeEntry::GetterSetterPair {
                    getter: previous,
                    setter: element,
                },
            );
            return;
        }
        Some(ScopeEntry::Element(previous))
            if previous.tag() == Tag::Setter && fragment.tag() == Tag::Getter =>
        {
            scope.insert(
                name,
                ScopeEntry::GetterSetterPair {
                    getter: element,
                    setter: previous,
                },
            );
            return;
        }
        Some(ScopeEntry::GetterSetterPair { setter, .. }) if fragment.tag() == Tag::Setter => {
            setter
        }
        Some(ScopeEntry::GetterSetterPair { getter, .. }) => getter,
        Some(ScopeEntry::Element(previous)) => previous,
    };
    if previous != element {
        let d = duplicate_definition(
            &ctx,
            diag::duplicate_definition(&name),
            origin_fragment.unwrap_or(fragment),
            previous,
        );
        host.report(d);
    }
}

/// Dart `_checkEnum(node)`: there are no members with the same name.
fn check_enum<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<EnumDeclaration>,
) {
    let ctx = host.ctx();
    let Some(fragment) = declared_fragment(host, node) else {
        return;
    };
    let first_fragment = element_first_fragment(&ctx, fragment);
    context
        .instance_element_contexts
        .entry(first_fragment)
        .or_default();

    let ast = host.ast();
    let body = ast[node].body.raw();
    for constant in enum_body_constants(ast, body) {
        let Some(constant_fragment) = declared_fragment(host, constant) else {
            continue;
        };
        let Some(getter) = fragment_element(&ctx, constant_fragment)
            .and_then(|e| e.cast::<FieldElement>())
            .and_then(|f| ctx.get(f).getter)
        else {
            continue;
        };
        let name = host.ast()[constant].name;
        let getter_fragment = ctx.get(getter).first_fragment().raw();
        check_duplicate_identifier(
            host,
            context,
            first_fragment,
            ScopeKind::Static,
            name,
            getter_fragment,
            Some(constant_fragment),
        );
        check_values_declaration_in_enum(host, name);
    }

    let ast = host.ast();
    let members = class_body_members(ast, body);
    let primary = primary_constructor(ast, ast[node].name_part.raw());
    check_class_members(host, context, fragment, &members, primary);

    let Some(element) = fragment_element(&ctx, fragment).and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let class_display_name = first_fragment_display_name(&ctx, first_fragment);
    let Some(data) = instance_fragment(&ctx, fragment) else {
        return;
    };
    let accessors: Vec<FragmentId> = data
        .getters
        .iter()
        .map(|f| f.raw())
        .chain(data.setters.iter().map(|f| f.raw()))
        .collect();
    for accessor in accessors {
        if fragment_is_static(&ctx, accessor) || !in_current_unit(host, accessor) {
            continue;
        }
        let base_name = fragment_display_name(&ctx, accessor);
        let inherited = get_inherited_member(host, element, &base_name);
        if let Some(inherited) = inherited
            && member::base_element(&ctx, inherited).tag() == Tag::Method
        {
            let Some(accessor_element) = fragment_element(&ctx, accessor) else {
                continue;
            };
            let conflicting = enclosing_name(&ctx, inherited);
            let (offset, length) = diagnostic_range(&ctx, accessor_element);
            host.report(
                diag::conflicting_field_and_method(&class_display_name, &base_name, &conflicting)
                    .at_offset(offset, length),
            );
        }
    }

    let methods: Vec<FragmentId> = data.methods.iter().map(|f| f.raw()).collect();
    for method in methods {
        if fragment_is_static(&ctx, method) || !in_current_unit(host, method) {
            continue;
        }
        let base_name = fragment_display_name(&ctx, method);
        let inherited = get_inherited_member(host, element, &base_name);
        if let Some(inherited) = inherited
            && matches!(
                member::base_element(&ctx, inherited).tag(),
                Tag::Getter | Tag::Setter
            )
        {
            let Some(method_element) = fragment_element(&ctx, method) else {
                continue;
            };
            let conflicting = enclosing_name(&ctx, inherited);
            let (offset, length) = diagnostic_range(&ctx, method_element);
            host.report(
                diag::conflicting_method_and_field(&class_display_name, &base_name, &conflicting)
                    .at_offset(offset, length),
            );
        }
    }
}

/// Dart `_checkEnumStatic(node)`.
fn check_enum_static<'h, H: VerifierHost<'h>>(host: &mut H, node: Id<EnumDeclaration>) {
    let ctx = host.ctx();
    let Some(fragment) = declared_fragment(host, node) else {
        return;
    };
    let first_fragment = element_first_fragment(&ctx, fragment);
    let Some(declaration_name) = ctx
        .fragment_data(first_fragment)
        .and_then(|f| f.name)
        .map(|n| ctx.name_str(n).to_string())
    else {
        return;
    };
    let Some(element) = fragment_element(&ctx, fragment).and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let Some(data) = instance_fragment(&ctx, fragment) else {
        return;
    };
    let accessors: Vec<FragmentId> = data
        .getters
        .iter()
        .map(|f| f.raw())
        .chain(data.setters.iter().map(|f| f.raw()))
        .collect();
    for accessor in accessors {
        if !in_current_unit(host, accessor) {
            continue;
        }
        let base_name = fragment_display_name(&ctx, accessor);
        if !fragment_is_static(&ctx, accessor) {
            continue;
        }
        let instance = get_interface_member(host, element, &base_name);
        if let Some(instance) = instance
            && !is_augmentation_without_augmented_declaration(
                &ctx,
                member::base_element(&ctx, instance),
            )
            && base_name != "values"
        {
            let Some(accessor_element) = fragment_element(&ctx, accessor) else {
                continue;
            };
            let (offset, length) = diagnostic_range(&ctx, accessor_element);
            host.report(
                diag::conflicting_static_and_instance(
                    &declaration_name,
                    &base_name,
                    &declaration_name,
                )
                .at_offset(offset, length),
            );
        }
    }

    let methods: Vec<FragmentId> = data.methods.iter().map(|f| f.raw()).collect();
    for method in methods {
        if !in_current_unit(host, method) {
            continue;
        }
        let base_name = fragment_display_name(&ctx, method);
        if !fragment_is_static(&ctx, method) {
            continue;
        }
        let instance = get_interface_member(host, element, &base_name);
        if let Some(instance) = instance
            && !is_augmentation_without_augmented_declaration(
                &ctx,
                member::base_element(&ctx, instance),
            )
        {
            let Some(method_element) = fragment_element(&ctx, method) else {
                continue;
            };
            let (offset, length) = diagnostic_range(&ctx, method_element);
            host.report(
                diag::conflicting_static_and_instance(
                    &declaration_name,
                    &base_name,
                    &declaration_name,
                )
                .at_offset(offset, length),
            );
        }
    }
}

/// `_checkExtension(node)`: there are no members with the same name.
fn check_extension<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<ExtensionDeclaration>,
) {
    let ast = host.ast();
    let members = class_body_members(ast, ast[node].body.raw());
    if let Some(fragment) = declared_fragment(host, node) {
        check_class_members(host, context, fragment, &members, None);
    }
}

/// Dart `_checkExtensionStatic(node)`.
fn check_extension_static<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<ExtensionDeclaration>,
) {
    let ctx = host.ctx();
    let Some(fragment) = declared_fragment(host, node) else {
        return;
    };
    let first_fragment = element_first_fragment(&ctx, fragment);
    let mut conflicts = Vec::new();
    {
        let ast = host.ast();
        let instance_scope = &element_context(context, first_fragment).instance_scope;
        for member in class_body_members(ast, ast[node].body.raw()) {
            let m = member.raw();
            if let Some(f) = ast.cast::<FieldDeclaration>(m) {
                if ast[f].static_keyword.is_some() {
                    for &field in ast.list(ast[ast[f].fields].variables) {
                        let identifier = ast[field].name;
                        let name = ast.tokens.lexeme(identifier);
                        if instance_scope.contains_key(name) {
                            conflicts.push((identifier, name.to_string()));
                        }
                    }
                }
            } else if let Some(md) = ast.cast::<MethodDeclaration>(m)
                && is_keyword(ast, ast[md].modifier_keyword, "static")
            {
                let identifier = ast[md].name;
                let name = ast.tokens.lexeme(identifier);
                if instance_scope.contains_key(name) {
                    conflicts.push((identifier, name.to_string()));
                }
            }
        }
    }
    for (identifier, name) in conflicts {
        let (offset, length) = token_range(host.ast(), identifier);
        host.report(
            diag::extension_conflicting_static_and_instance(&name).at_offset(offset, length),
        );
    }
}

/// `_checkExtensionType(node)`.
fn check_extension_type<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<ExtensionTypeDeclaration>,
) {
    let ast = host.ast();
    let members = class_body_members(ast, ast[node].body.raw());
    let primary = primary_constructor(ast, ast[node].name_part.raw());
    if let Some(fragment) = declared_fragment(host, node) {
        check_class_members(host, context, fragment, &members, primary);
    }
}

/// `_checkMixin(node)`.
fn check_mixin<'h, H: VerifierHost<'h>>(
    host: &mut H,
    context: &mut DuplicationDefinitionContext,
    node: Id<MixinDeclaration>,
) {
    let ast = host.ast();
    let members = class_body_members(ast, ast[node].body.raw());
    if let Some(fragment) = declared_fragment(host, node) {
        check_class_members(host, context, fragment, &members, None);
    }
}

/// `_checkValuesDeclarationInEnum(name)`.
fn check_values_declaration_in_enum<'h, H: VerifierHost<'h>>(host: &mut H, name: TokenId) {
    if host.ast().tokens.lexeme(name) == "values" {
        let (offset, length) = token_range(host.ast(), name);
        host.report(diag::values_declaration_in_enum().at_offset(offset, length));
    }
}

/// `_getInheritedMember(element, baseName)`.
fn get_inherited_member<'h, H: VerifierHost<'h>>(
    host: &H,
    element: EId<InterfaceElement>,
    base_name: &str,
) -> Option<ElemRef> {
    let ctx = host.ctx();
    let library = Some(host.library());
    let inheritance = InheritanceManager3::new(ctx);
    let getter_name = MemberName::new(&ctx, library, base_name);
    if let Some(getter) = inheritance.get_inherited(element, getter_name) {
        return Some(getter);
    }
    let setter_name = MemberName::new(&ctx, library, &format!("{base_name}="));
    inheritance.get_inherited(element, setter_name)
}

/// `_getInterfaceMember(element, baseName)`.
fn get_interface_member<'h, H: VerifierHost<'h>>(
    host: &H,
    element: EId<InterfaceElement>,
    base_name: &str,
) -> Option<ElemRef> {
    let ctx = host.ctx();
    let library = Some(host.library());
    let inheritance = InheritanceManager3::new(ctx);
    let getter_name = MemberName::new(&ctx, library, base_name);
    if let Some(getter) = inheritance.get_member(element, getter_name) {
        return Some(getter);
    }
    let setter_name = MemberName::new(&ctx, library, &format!("{base_name}="));
    inheritance.get_member(element, setter_name)
}

/// `fragment.isStatic` of an executable fragment.
fn fragment_is_static(ctx: &Ctx<'_>, fragment: FragmentId) -> bool {
    fragment_flags(ctx, fragment).contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
}

/// `fragment.libraryFragment.source == _currentUnit.source`.
fn in_current_unit<'h, H: VerifierHost<'h>>(host: &H, fragment: FragmentId) -> bool {
    dartr_element::diagnostics::library_fragment_of(&host.ctx(), fragment) == Some(host.fragment())
}

/// `fragment.displayName`: the name (without `=` for a setter).
fn fragment_display_name(ctx: &Ctx<'_>, fragment: FragmentId) -> String {
    ctx.fragment_data(fragment)
        .and_then(|f| f.name)
        .map_or_else(String::new, |n| ctx.name_str(n).to_string())
}

/// `firstFragment.displayName` of an interface fragment.
fn first_fragment_display_name(ctx: &Ctx<'_>, fragment: FragmentId) -> String {
    fragment_display_name(ctx, fragment)
}

/// `member.enclosingElement!.name!`.
fn enclosing_name(ctx: &Ctx<'_>, e: ElemRef) -> String {
    member::enclosing_element(ctx, e)
        .and_then(|e| ctx.element_data(e))
        .and_then(|d| d.name)
        .map_or_else(String::new, |n| ctx.name_str(n).to_string())
}
