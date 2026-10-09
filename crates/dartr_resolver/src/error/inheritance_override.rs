// Dart source: pkg/analyzer/lib/src/error/inheritance_override.dart

//! `InheritanceOverrideVerifier`: the inheritance checks of classes, enums,
//! mixins and class type aliases: disallowed supertypes, recursive
//! interface inheritance, conflicts between superinterfaces, invalid
//! overrides, missing concrete implementations, enum restrictions,
//! `@mustBeOverridden`.
//!
//! # Differences to Dart
//!
//! - Dart creates one verifier per library and reports the conflicts of an
//!   interface through the reporter of the unit that declares the target
//!   element (`_diagnosticReportersByFragment`). The library analyzer runs
//!   [`verify_unit`] per unit, so a target in another unit of the library
//!   (only possible with augmentations) is not reported.
//! - `_interfaceElementStates` (the mixin index and the reported recursive
//!   inheritance across the fragments of one element) is per unit.
//! - `metadata.hasMustBeOverridden` is read from the annotations in this
//!   unit (the linker does not keep resolved annotations of other units and
//!   libraries yet).
//! - `missingOverrides` / `missingMustBeOverridden` (the `Expando`s for
//!   quick fixes) are not kept.

use dartr_ast::{
    Ast, ClassDeclaration, ClassMember, ClassTypeAlias, EnumDeclaration, FieldDeclaration, Id,
    MethodDeclaration, MixinDeclaration, NamedType, NodeId, PrimaryConstructorDeclaration,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::{library_fragment_of, type_arg, type_display_string};
use dartr_element::{
    AnyElement, Ctx, EId, ElemRef, ElementFlags, ElementId, FieldElement, FormalParameterElement,
    FragmentFlags, FragmentId, InterfaceElement, LibraryElement, MethodElement, Nullability, Tag,
    TopLevelInferenceError, TypeId, TypeKind, TypeProvider,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenId;
use dartr_typesystem::inheritance_manager3::{
    Conflict, GetMemberOptions, InheritanceManager3, Interface, Name as MemberName,
};
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::type_ext::TypeExt;
use indexmap::{IndexMap, IndexSet};

use super::correct_override::{
    CorrectOverrideHelper, CovariantParametersVerifier, InvalidOverrideCode,
    annotation_is_top_getter, declared_element, declared_fragment, display_name, first_fragment,
    first_fragment_flags, fragment_element, get_getter, get_method, get_setter,
    is_augmentation_without_augmented_declaration, library_feature_enabled, node_range,
    non_synthetic, token_is_synthetic, token_range, utf16_len,
};
use super::duplicate_definition_verifier::class_name_part_type_name;
use super::getter_setter_types_verifier;
use super::member_duplicate_definition_verifier::class_body_members;
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;

/// Dart `InheritanceOverrideVerifier(...).verifyUnit(unit, reporter)`.
pub fn verify_unit(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let mut verifier = InheritanceOverrideVerifier::default();
    let declarations = ast.list(ast[v.unit].declarations).to_vec();
    for declaration in declarations {
        verifier.verify_declaration(v, declaration.raw());
    }
}

/// Dart `InheritanceOverrideVerifier`: the state across the declarations.
#[derive(Default)]
pub struct InheritanceOverrideVerifier {
    /// `_interfaceElementStates`.
    interface_element_states: IndexMap<EId<InterfaceElement>, InterfaceElementState>,
    /// The fragments of each element that the annotations in the unit
    /// belong to (`ResolverTables.element_annotation`, reversed); built on
    /// first use.
    annotations_by_fragment: Option<IndexMap<FragmentId, Vec<NodeId>>>,
}

/// Dart `_InterfaceElementState`: an element's state across fragments.
#[derive(Default, Clone, Copy)]
struct InterfaceElementState {
    has_reported_recursive_interface_inheritance: bool,
    mixin_index: i32,
}

/// The syntax of one declaration that `_ClassVerifier` checks.
struct ClassNode {
    class_name_token: TokenId,
    implements_clause: Option<Id<dartr_ast::ImplementsClause>>,
    members: Vec<Id<ClassMember>>,
    on_clause: Option<Id<dartr_ast::MixinOnClause>>,
    primary_constructor: Option<Id<PrimaryConstructorDeclaration>>,
    superclass: Option<Id<NamedType>>,
    with_clause: Option<Id<dartr_ast::WithClause>>,
}

impl InheritanceOverrideVerifier {
    /// One iteration of the loop of Dart `verifyUnit`.
    pub fn verify_declaration<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        declaration: NodeId,
    ) {
        let ast = host.ast();
        let class_node = if let Some(n) = ast.cast::<ClassDeclaration>(declaration) {
            let d = &ast[n];
            ClassNode {
                class_name_token: class_name_part_type_name(ast, d.name_part.raw()),
                implements_clause: d.implements_clause,
                members: class_body_members(ast, d.body.raw()),
                on_clause: None,
                primary_constructor: ast.cast::<PrimaryConstructorDeclaration>(d.name_part.raw()),
                superclass: d.extends_clause.map(|e| ast[e].superclass),
                with_clause: d.with_clause,
            }
        } else if let Some(n) = ast.cast::<ClassTypeAlias>(declaration) {
            let d = &ast[n];
            ClassNode {
                class_name_token: d.name,
                implements_clause: d.implements_clause,
                members: Vec::new(),
                on_clause: None,
                primary_constructor: None,
                superclass: Some(d.superclass),
                with_clause: Some(d.with_clause),
            }
        } else if let Some(n) = ast.cast::<EnumDeclaration>(declaration) {
            let d = &ast[n];
            ClassNode {
                class_name_token: class_name_part_type_name(ast, d.name_part.raw()),
                implements_clause: d.implements_clause,
                members: class_body_members(ast, d.body.raw()),
                on_clause: None,
                primary_constructor: ast.cast::<PrimaryConstructorDeclaration>(d.name_part.raw()),
                superclass: None,
                with_clause: d.with_clause,
            }
        } else if let Some(n) = ast.cast::<MixinDeclaration>(declaration) {
            let d = &ast[n];
            ClassNode {
                class_name_token: d.name,
                implements_clause: d.implements_clause,
                members: class_body_members(ast, d.body.raw()),
                on_clause: d.on_clause,
                primary_constructor: None,
                superclass: None,
                with_clause: None,
            }
        } else {
            return;
        };
        let Some(class_fragment) = declared_fragment(host, declaration) else {
            return;
        };
        let ctx = host.ctx();
        let Some(class_element) =
            fragment_element(&ctx, class_fragment).and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        let state = *self
            .interface_element_states
            .entry(class_element)
            .or_default();
        let library = host.library();
        let implements_dart_core_enum = ctx
            .element_all_supertypes(class_element)
            .iter()
            .any(|&t| ctx.is_dart_core_enum(t));
        let mut verifier = ClassVerifier {
            ctx,
            library,
            class_element,
            class_fragment,
            n: class_node,
            state,
            direct_super_interfaces: Vec::new(),
            implements_dart_core_enum,
            annotations_by_fragment: &mut self.annotations_by_fragment,
        };
        let stop = verifier.verify(host);
        if !stop {
            verifier.verify_must_be_overridden(host);
        }
        let state = verifier.state;
        self.interface_element_states.insert(class_element, state);
    }
}

/// Dart `_reportInterfaceConflicts(element, interface)`.
fn report_interface_conflicts<'h, H: VerifierHost<'h>>(
    host: &mut H,
    element: EId<InterfaceElement>,
    interface: &Interface,
) {
    let ctx = host.ctx();
    for conflict in &interface.conflicts {
        let Some(interface_target) = target_for_element(host, element.raw()) else {
            continue;
        };
        let member_name = conflict.name().text(&ctx).to_string();
        match conflict {
            Conflict::GetterMethod { getter, method, .. } => {
                let mut target = interface_target;
                // Try to use a local declaration related to the conflict.
                if let Some(&declared) = interface.declared.get(&conflict.name()) {
                    target = target_for_element(host, member::base_element(&ctx, declared))
                        .unwrap_or(target);
                }
                let getter_interface = enclosing_name(&ctx, *getter);
                let method_interface = enclosing_name(&ctx, *method);
                host.report(
                    diag::inconsistent_inheritance_getter_and_method(
                        &member_name,
                        &getter_interface,
                        &method_interface,
                    )
                    .at_offset(target.0, target.1),
                );
            }
            Conflict::Candidates { candidates, .. } => {
                let inherited_signatures = candidates
                    .iter()
                    .map(|&candidate| {
                        let class_name = enclosing_name(&ctx, candidate);
                        let type_str =
                            type_display_string(&ctx, member::type_(&ctx, candidate), false);
                        format!("{class_name}.{member_name} ({type_str})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                host.report(
                    diag::inconsistent_inheritance(&member_name, &inherited_signatures)
                        .at_offset(interface_target.0, interface_target.1),
                );
            }
            // Dart throws a `StateError`; these conflicts are only computed
            // for extensions and extension types.
            _ => {}
        }
    }
}

/// `member.enclosingElement!.name!`.
fn enclosing_name(ctx: &Ctx<'_>, e: ElemRef) -> String {
    member::enclosing_element(ctx, e)
        .and_then(|e| ctx.element_name(e))
        .unwrap_or("")
        .to_string()
}

/// Dart `_targetForElement(element)`: the name of the first fragment of the
/// non-synthetic element, if it is in this unit (see the module
/// documentation).
fn target_for_element<'h, H: VerifierHost<'h>>(
    host: &H,
    element: ElementId,
) -> Option<(usize, usize)> {
    let ctx = host.ctx();
    let non_synthetic = non_synthetic(&ctx, element);
    let fragment = first_fragment(&ctx, non_synthetic)?;
    target_for_fragment(host, fragment)
}

/// Dart `_targetForFragment(fragment)`.
fn target_for_fragment<'h, H: VerifierHost<'h>>(
    host: &H,
    fragment: FragmentId,
) -> Option<(usize, usize)> {
    let ctx = host.ctx();
    let library_fragment = library_fragment_of(&ctx, fragment)?;
    if library_fragment != host.fragment() {
        return None;
    }
    let data = ctx.fragment_data(fragment)?;
    let offset = data.name_offset?;
    let length = utf16_len(ctx.name_str(data.name?));
    Some((offset as usize, length))
}

/// Dart `isInterfaceTypeInterface` (`summary2/types_builder.dart`).
fn is_interface_type_interface(ctx: &Ctx<'_>, t: TypeId) -> bool {
    let TypeKind::Interface {
        element,
        nullability,
        ..
    } = *ctx.ty(t)
    else {
        return false;
    };
    if matches!(element.raw().tag(), Tag::Enum | Tag::ExtensionType) {
        return false;
    }
    if ctx.is_dart_core_function(t) || ctx.is_dart_core_null(t) {
        return false;
    }
    nullability != Nullability::Question
}

/// The diagnostic codes of `_checkDirectSuperTypeNode`
/// (`DisallowedClassDiagnosticCode`).
type DisallowedClassCode = fn(dartr_diagnostics::TypeArg) -> LocatableDiagnostic;

/// Dart `_ClassVerifier`.
struct ClassVerifier<'a, 's> {
    ctx: Ctx<'a>,
    library: EId<LibraryElement>,
    class_element: EId<InterfaceElement>,
    class_fragment: FragmentId,
    n: ClassNode,
    state: InterfaceElementState,
    direct_super_interfaces: Vec<TypeId>,
    implements_dart_core_enum: bool,
    annotations_by_fragment: &'s mut Option<IndexMap<FragmentId, Vec<NodeId>>>,
}

impl<'a> ClassVerifier<'a, '_> {
    fn inheritance(&self) -> InheritanceManager3<'a> {
        InheritanceManager3::new(self.ctx)
    }

    fn is_class(&self) -> bool {
        self.class_element.raw().tag() == Tag::Class
    }

    fn is_enum(&self) -> bool {
        self.class_element.raw().tag() == Tag::Enum
    }

    fn is_mixin(&self) -> bool {
        self.class_element.raw().tag() == Tag::Mixin
    }

    /// `ClassElementImpl.isAbstract`.
    fn is_abstract(&self) -> bool {
        self.ctx
            .element_data(self.class_element.raw())
            .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT))
    }

    fn class_name(&self) -> String {
        self.ctx
            .element_name(self.class_element.raw())
            .unwrap_or("")
            .to_string()
    }

    fn report_at_class_name<'h, H: VerifierHost<'h>>(&self, host: &mut H, d: LocatableDiagnostic) {
        let (offset, length) = token_range(host.ast(), self.n.class_name_token);
        host.report(d.at_offset(offset, length));
    }

    /// `verify()`: returns `true` if an error was reported that should
    /// prevent follow-on diagnostics.
    fn verify<'h, H: VerifierHost<'h>>(&mut self, host: &mut H) -> bool {
        if self.check_direct_super_types(host) {
            return true;
        }

        let ctx = self.ctx;
        let element = self.class_element;
        if self.is_class() && !self.is_abstract() && self.implements_dart_core_enum {
            self.report_at_class_name(host, diag::concrete_class_has_enum_superinterface());
            return true;
        }

        if self.check_for_recursive_interface_inheritance(host) {
            return true;
        }

        // Compute the interface of the class.
        let interface = self.inheritance().get_interface(element);

        if first_fragment(&ctx, element.raw()) == Some(self.class_fragment) {
            report_interface_conflicts(host, element, &interface);
        }

        if let Some(supertype) = ctx.element_supertype(element) {
            self.direct_super_interfaces.push(supertype);
        }
        if self.is_mixin() {
            self.direct_super_interfaces
                .extend_from_slice(ctx.element_superclass_constraints(element));
        }

        // Each mixin in `class C extends S with M0, M1, M2 {}` is equivalent
        // to:
        //   class S&M0 extends S { ...members of M0... }
        //   class S&M1 extends S&M0 { ...members of M1... }
        //   class S&M2 extends S&M1 { ...members of M2... }
        //   class C extends S&M2 { ...members of C... }
        // So, we need to check members of each mixin against superinterfaces
        // of `S`, and superinterfaces of all previous mixins.
        let mixin_nodes: Vec<Id<NamedType>> = match self.n.with_clause {
            Some(w) => host.ast().list(host.ast()[w].mixin_types).to_vec(),
            None => Vec::new(),
        };
        for node in mixin_nodes {
            let mixin_type = named_type_type(host, node);
            // When building the element model, we skip incorrect types. So,
            // here we skip corresponding nodes to keep the index in sync.
            if let Some(mixin_type) = mixin_type
                && is_interface_type_interface(&ctx, mixin_type)
            {
                let index = self.state.mixin_index;
                self.state.mixin_index += 1;
                self.check_declared_members(host, node.raw(), mixin_type, index);
                self.direct_super_interfaces.push(mixin_type);
            }
        }

        self.direct_super_interfaces
            .extend_from_slice(ctx.element_interfaces(element));

        self.check_declaring_formal_parameter_fields(host);

        // Check the members of the class itself, against all the previously
        // collected superinterfaces of the supertype, mixins, and interfaces.
        let members = self.n.members.clone();
        let library = self.library;
        for member in members {
            let ast = host.ast();
            if let Some(f) = ast.cast::<FieldDeclaration>(member.raw()) {
                let is_static = ast[f].static_keyword.is_some();
                let fields = ast[f].fields;
                let variables = ast.list(ast[fields].variables).to_vec();
                for field in variables {
                    let name = host.ast()[field].name;
                    if let Some(field_element) =
                        declared_element(host, field).and_then(|e| e.cast::<FieldElement>())
                    {
                        self.check_declared_field(
                            host,
                            token_range(host.ast(), name),
                            field_element,
                        );
                    }
                    if !is_static && !self.is_enum() {
                        self.check_illegal_enum_values_declaration(host, name);
                    }
                    if !is_static {
                        self.check_illegal_concrete_enum_member_declaration(host, name);
                    }
                }
            } else if let Some(m) = ast.cast::<MethodDeclaration>(member.raw()) {
                let has_error = self.report_no_combined_super_signature(host, m);
                if has_error {
                    continue;
                }
                let ast = host.ast();
                let name = ast[m].name;
                let is_static = is_keyword(ast, ast[m].modifier_keyword, "static");
                let is_setter = is_keyword(ast, ast[m].property_keyword, "set");
                let is_complete = ast[m].external_keyword.is_some()
                    || !ast.is::<dartr_ast::EmptyFunctionBody>(ast[m].body);
                if let Some(element) = declared_element(host, m) {
                    let name_range = token_range(host.ast(), name);
                    self.check_declared_member(
                        host,
                        name_range,
                        library,
                        Some(ElemRef::Base(element)),
                        -1,
                    );
                }
                if !(is_static || !is_complete || is_setter) {
                    self.check_illegal_concrete_enum_member_declaration(host, name);
                }
                if !is_static && !self.is_enum() {
                    self.check_illegal_enum_values_declaration(host, name);
                }
            }
        }

        self.check_illegal_concrete_enum_member_inheritance(host);
        self.check_illegal_enum_values_inheritance(host);

        getter_setter_types_verifier::check_interface(host, element, &interface);

        if self.is_class() && !self.is_abstract() || self.is_enum() {
            let mut inherited_abstract: Option<Vec<ElemRef>> = None;

            for (name, &interface_element) in &interface.map {
                if !name.is_accessible_for(&ctx, library) {
                    continue;
                }

                let concrete_element = interface.implemented.get(name).copied();

                // No concrete implementation of the name.
                let Some(concrete_element) = concrete_element else {
                    if is_augmentation_without_augmented_declaration(
                        &ctx,
                        member::base_element(&ctx, interface_element),
                    ) {
                        continue;
                    }
                    if self.report_concrete_class_with_abstract_member(host, name.text(&ctx)) {
                        continue;
                    }
                    if member::enclosing_element(&ctx, interface_element) == Some(element.raw()) {
                        continue;
                    }
                    if self.is_not_implemented_in_concrete_super_class(*name) {
                        continue;
                    }
                    // We already reported ILLEGAL_ENUM_VALUES_INHERITANCE.
                    if self.is_enum() && matches!(name.text(&ctx), "values" | "values=") {
                        continue;
                    }
                    inherited_abstract
                        .get_or_insert_with(Vec::new)
                        .push(interface_element);
                    continue;
                };

                // The case when members have different kinds is reported in
                // verifier.
                if member::base_element(&ctx, concrete_element).tag()
                    != member::base_element(&ctx, interface_element).tag()
                {
                    continue;
                }

                // If a class declaration is not abstract, and the interface
                // has a member declaration named `m`, then:
                // 1. if the class contains a non-overridden member whose
                //    signature is not a valid override of the interface
                //    member signature for `m`, then it's a compile-time
                //    error.
                // 2. if the class contains no member named `m`, and the class
                //    member for `noSuchMethod` is the one declared in
                //    `Object`, then it's a compile-time error.
                let code = if member::is_setter(&ctx, concrete_element) {
                    InvalidOverrideCode::InvalidImplementationOverrideSetter
                } else {
                    InvalidOverrideCode::InvalidImplementationOverride
                };
                let error_range = token_range(host.ast(), self.n.class_name_token);
                CorrectOverrideHelper::new(host.type_system(), concrete_element).verify(
                    host,
                    interface_element,
                    error_range,
                    code,
                );
            }

            if first_fragment(&ctx, element.raw()) == Some(self.class_fragment) {
                self.report_inherited_abstract_members(host, inherited_abstract);
            }
        }

        false
    }

    /// `_checkDeclaredField(name, field)`.
    fn check_declared_field<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        name: (usize, usize),
        field: EId<FieldElement>,
    ) {
        let data = self.ctx.get(field);
        let getter = data.getter.map(|g| ElemRef::Base(g.raw()));
        let setter = data.setter.map(|s| ElemRef::Base(s.raw()));
        let library = self.library;
        self.check_declared_member(host, name, library, getter, -1);
        self.check_declared_member(host, name, library, setter, -1);
    }

    /// `_checkDeclaredMember(node, libraryUri, member, mixinIndex:)`: the
    /// member is a valid override of the corresponding instance members in
    /// each of the direct superinterfaces.
    fn check_declared_member<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: (usize, usize),
        _library: EId<LibraryElement>,
        member: Option<ElemRef>,
        mixin_index: i32,
    ) {
        let ctx = self.ctx;
        let Some(member) = member else {
            return;
        };
        if member::is_static(&ctx, member) {
            return;
        }
        let Some(name) = MemberName::for_element(&ctx, member) else {
            return;
        };

        let correct_override_helper = CorrectOverrideHelper::new(host.type_system(), member);
        let member_kind = member::base_element(&ctx, member).tag();

        for &super_type in &self.direct_super_interfaces.clone() {
            let Some(super_member) = self.inheritance().get_member3(
                super_type,
                name,
                GetMemberOptions {
                    for_mixin_index: mixin_index,
                    ..GetMemberOptions::default()
                },
            ) else {
                continue;
            };

            // The case when members have different kinds is reported in
            // verifier.
            if member_kind != member::base_element(&ctx, super_member).tag() {
                continue;
            }

            let code = if member_kind == Tag::Setter {
                InvalidOverrideCode::InvalidOverrideSetter
            } else {
                InvalidOverrideCode::InvalidOverride
            };
            correct_override_helper.verify(host, super_member, node, code);
        }

        if mixin_index == -1 {
            CovariantParametersVerifier::new(host.type_system(), member).verify(host, node);
        }
    }

    /// `_checkDeclaredMembers(node, type, mixinIndex:)`: the instance members
    /// of [t] are valid overrides of the corresponding instance members in
    /// each of the direct superinterfaces.
    fn check_declared_members<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: NodeId,
        t: TypeId,
        mixin_index: i32,
    ) {
        let ctx = self.ctx;
        let Some(element) = ctx.interface_element(t) else {
            return;
        };
        let Some(library) = ctx.element_data(element.raw()).and_then(|d| d.library) else {
            return;
        };
        let substitution = MapSubstitution::from_interface_type(&ctx, t);
        let data = ctx.instance(element.upcast());
        let members: Vec<ElementId> = data
            .methods
            .iter()
            .map(|m| m.raw())
            .chain(data.getters.iter().map(|g| g.raw()))
            .chain(data.setters.iter().map(|s| s.raw()))
            .collect();
        let range = node_range(host.ast(), node);
        for m in members {
            let m = member::substitute(&ctx, ElemRef::Base(m), &substitution);
            self.check_declared_member(host, range, library, Some(m), mixin_index);
        }
    }

    /// `_checkDeclaringFormalParameterFields()`.
    fn check_declaring_formal_parameter_fields<'h, H: VerifierHost<'h>>(&mut self, host: &mut H) {
        let Some(primary_constructor) = self.n.primary_constructor else {
            return;
        };
        let ctx = self.ctx;
        let ast = host.ast();
        let parameters = ast
            .list(ast[ast[primary_constructor].formal_parameters].parameters)
            .to_vec();
        for formal_parameter in parameters {
            let Some(element) = declared_element(host, formal_parameter) else {
                continue;
            };
            if element.tag() != Tag::FieldFormalParameter
                || !first_fragment_flags(&ctx, element)
                    .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
            {
                continue;
            }
            let name = formal_parameter_parts(host.ast(), formal_parameter.raw()).name;
            let field = element
                .cast::<FormalParameterElement>()
                .and_then(|p| ctx.get(p).field.get());
            if let (Some(name), Some(field)) = (name, field) {
                let range = token_range(host.ast(), name);
                self.check_declared_field(host, range, field);
            }
        }
    }

    /// `_checkDirectSuperType(type:, hasEnum:, notSubtypable:)`: whether
    /// [t] cannot be subtyped; `Some(true)` for `Enum`, `Some(false)` for a
    /// non-subtypable class.
    fn check_direct_super_type(&self, t: TypeId) -> Option<bool> {
        let ctx = self.ctx;
        // The SDK implementation may implement disallowed types. For example,
        // JSNumber in dart2js and _Smi in Dart VM both implement int.
        if ctx.library_uri(self.library).starts_with("dart:") {
            return None;
        }
        let type_element = ctx.interface_element(t)?;

        if type_element.raw().tag() == Tag::Class
            && ctx.is_element(type_element.raw(), "dart.core", "Enum")
            && library_feature_enabled(&ctx, self.library, ExperimentalFlag::EnhancedEnums)
        {
            if self.is_class() && self.is_abstract() || self.is_enum() || self.is_mixin() {
                return None;
            }
            return Some(true);
        }

        let name = ctx.element_name(type_element.raw()).unwrap_or("");
        let uri = ctx.element_library_uri(type_element.raw()).unwrap_or("");
        if TypeProvider::is_non_subtypable_class(name, uri) {
            return Some(false);
        }
        None
    }

    /// `_checkDirectSuperTypeNode(namedType, diagnosticCode)`: the named
    /// type does not extend, implement, or mix in types such as `num` or
    /// `String`.
    fn check_direct_super_type_node<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        named_type: Id<NamedType>,
        code: DisallowedClassCode,
    ) -> bool {
        let ast = host.ast();
        if token_is_synthetic(ast, ast[named_type].name) && ast[named_type].type_arguments.is_none()
        {
            return false;
        }
        let Some(t) = named_type_type(host, named_type) else {
            return false;
        };
        let range = node_range(host.ast(), named_type);
        match self.check_direct_super_type(t) {
            None => false,
            Some(true) => {
                host.report(
                    diag::concrete_class_has_enum_superinterface().at_offset(range.0, range.1),
                );
                true
            }
            Some(false) => {
                host.report(code(type_arg(&self.ctx, t)).at_offset(range.0, range.1));
                true
            }
        }
    }

    /// `_checkDirectSuperTypes()`: reports invalid direct supertypes and
    /// returns `true` if there are any.
    fn check_direct_super_types<'h, H: VerifierHost<'h>>(&self, host: &mut H) -> bool {
        let mut has_error = false;
        if let Some(implements_clause) = self.n.implements_clause {
            let interfaces = host
                .ast()
                .list(host.ast()[implements_clause].interfaces)
                .to_vec();
            for named_type in interfaces {
                if self.check_direct_super_type_node(
                    host,
                    named_type,
                    diag::implements_disallowed_class,
                ) {
                    has_error = true;
                }
            }
        }
        if let Some(on_clause) = self.n.on_clause {
            let constraints = host
                .ast()
                .list(host.ast()[on_clause].superclass_constraints)
                .to_vec();
            for named_type in constraints {
                if self.check_direct_super_type_node(
                    host,
                    named_type,
                    diag::mixin_super_class_constraint_disallowed_class,
                ) {
                    has_error = true;
                }
            }
        }
        if let Some(superclass) = self.n.superclass
            && self.check_direct_super_type_node(host, superclass, diag::extends_disallowed_class)
        {
            has_error = true;
        }
        if let Some(with_clause) = self.n.with_clause {
            let mixins = host
                .ast()
                .list(host.ast()[with_clause].mixin_types)
                .to_vec();
            for named_type in mixins {
                if self.check_direct_super_type_node(
                    host,
                    named_type,
                    diag::mixin_of_disallowed_class,
                ) {
                    has_error = true;
                }
                if self.is_enum() && self.check_mixin_of_enum(host, named_type) {
                    has_error = true;
                }
            }
        }
        has_error
    }

    /// `_checkForRecursiveInterfaceInheritance(element)`: the class is not a
    /// superinterface to itself.
    fn check_for_recursive_interface_inheritance<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
    ) -> bool {
        if self.state.has_reported_recursive_interface_inheritance {
            return true;
        }
        let ctx = self.ctx;
        let element = self.class_element;
        let Some(cycle) = ctx.interface(element).interface_cycle.try_get() else {
            return false;
        };
        let display = display_name(&ctx, element.raw());
        let refers_to_element = |host: &H, t: Id<NamedType>| {
            host.element(t)
                .is_some_and(|e| member::base_element(&ctx, e) == element.raw())
        };

        if let Some(superclass) = self.n.superclass
            && refers_to_element(host, superclass)
        {
            let range = node_range(host.ast(), superclass);
            host.report(
                diag::recursive_interface_inheritance_extends(&display).at_offset(range.0, range.1),
            );
            self.state.has_reported_recursive_interface_inheritance = true;
            return true;
        }

        if let Some(on_clause) = self.n.on_clause {
            let constraints = host
                .ast()
                .list(host.ast()[on_clause].superclass_constraints)
                .to_vec();
            for type_annotation in constraints {
                if refers_to_element(host, type_annotation) {
                    let range = node_range(host.ast(), type_annotation);
                    host.report(
                        diag::recursive_interface_inheritance_on(&display)
                            .at_offset(range.0, range.1),
                    );
                    self.state.has_reported_recursive_interface_inheritance = true;
                    return true;
                }
            }
        }

        if let Some(with_clause) = self.n.with_clause {
            let mixins = host
                .ast()
                .list(host.ast()[with_clause].mixin_types)
                .to_vec();
            for type_annotation in mixins {
                if refers_to_element(host, type_annotation) {
                    let range = node_range(host.ast(), type_annotation);
                    host.report(
                        diag::recursive_interface_inheritance_with(&display)
                            .at_offset(range.0, range.1),
                    );
                    self.state.has_reported_recursive_interface_inheritance = true;
                    return true;
                }
            }
        }

        if let Some(implements_clause) = self.n.implements_clause {
            let interfaces = host
                .ast()
                .list(host.ast()[implements_clause].interfaces)
                .to_vec();
            for type_annotation in interfaces {
                if refers_to_element(host, type_annotation) {
                    let range = node_range(host.ast(), type_annotation);
                    host.report(
                        diag::recursive_interface_inheritance_implements(&display)
                            .at_offset(range.0, range.1),
                    );
                    self.state.has_reported_recursive_interface_inheritance = true;
                    return true;
                }
            }
        }

        // Earlier fragments can see cycles from clauses in later
        // augmentations. Wait for those clauses before reporting the generic
        // cycle.
        if ctx
            .fragment_data(self.class_fragment)
            .is_some_and(|f| f.next_fragment.is_some())
        {
            return true;
        }

        let loop_ = cycle
            .iter()
            .map(|e| display_name(&ctx, e.raw()))
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(target) = target_for_element(host, element.raw()) {
            host.report(
                diag::recursive_interface_inheritance(&display, &loop_)
                    .at_offset(target.0, target.1),
            );
        }
        self.state.has_reported_recursive_interface_inheritance = true;
        true
    }

    /// `_checkIllegalConcreteEnumMemberDeclaration(name)`.
    fn check_illegal_concrete_enum_member_declaration<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        name: TokenId,
    ) {
        if !self.implements_dart_core_enum {
            return;
        }
        let ctx = self.ctx;
        let is_dart_core_enum_impl = ctx.is_element(self.class_element.raw(), "dart.core", "_Enum");
        if self.is_class() && !is_dart_core_enum_impl || self.is_enum() || self.is_mixin() {
            let lexeme = host.ast().tokens.lexeme(name).to_string();
            if matches!(lexeme.as_str(), "index" | "hashCode" | "==") {
                let range = token_range(host.ast(), name);
                host.report(
                    diag::illegal_concrete_enum_member_declaration(&lexeme)
                        .at_offset(range.0, range.1),
                );
            }
        }
    }

    /// `_checkIllegalConcreteEnumMemberInheritance()`.
    fn check_illegal_concrete_enum_member_inheritance<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
    ) {
        // We ignore mixins because they don't inherit and members. But to
        // support `super.foo()` invocations we put members from superclass
        // constraints into the `superImplemented` bucket, the same we look
        // below.
        if self.is_mixin() {
            return;
        }
        if !self.implements_dart_core_enum {
            return;
        }
        let ctx = self.ctx;
        let concrete = self
            .inheritance()
            .get_inherited_concrete_map(self.class_element);
        let check_single = |host: &mut H, member_name: &str, filter: &dyn Fn(ElementId) -> bool| {
            let name = MemberName::new(&ctx, Some(self.library), member_name);
            let Some(&member) = concrete.get(&name) else {
                return;
            };
            let Some(enclosing_class) = member::enclosing_element(&ctx, member) else {
                return;
            };
            if enclosing_class.tag() != Tag::Class || filter(enclosing_class) {
                let class_name = ctx.element_name(enclosing_class).unwrap_or("").to_string();
                self.report_at_class_name(
                    host,
                    diag::illegal_concrete_enum_member_inheritance(member_name, &class_name),
                );
            }
        };
        let not_object = |e: ElementId| !member::is_dart_core_object_element(&ctx, e);
        let not_enum = |e: ElementId| !ctx.is_element(e, "dart.core", "Enum");
        check_single(host, "hashCode", &not_object);
        check_single(host, "==", &not_object);
        check_single(host, "index", &not_enum);
    }

    /// `_checkIllegalEnumValuesDeclaration(name)`.
    fn check_illegal_enum_values_declaration<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        name: TokenId,
    ) {
        if self.implements_dart_core_enum && host.ast().tokens.lexeme(name) == "values" {
            let range = token_range(host.ast(), name);
            host.report(diag::illegal_enum_values_declaration().at_offset(range.0, range.1));
        }
    }

    /// `_checkIllegalEnumValuesInheritance()`.
    fn check_illegal_enum_values_inheritance<'h, H: VerifierHost<'h>>(&self, host: &mut H) {
        if !self.implements_dart_core_enum {
            return;
        }
        let ctx = self.ctx;
        let getter = self.inheritance().get_inherited(
            self.class_element,
            MemberName::new(&ctx, Some(self.library), "values"),
        );
        let setter = self.inheritance().get_inherited(
            self.class_element,
            MemberName::new(&ctx, Some(self.library), "values="),
        );
        if let Some(inherited) = getter.or(setter) {
            let class_name = enclosing_name(&ctx, inherited);
            self.report_at_class_name(host, diag::illegal_enum_values_inheritance(&class_name));
        }
    }

    /// `_checkMixinOfEnum(namedType)`.
    fn check_mixin_of_enum<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        named_type: Id<NamedType>,
    ) -> bool {
        let ctx = self.ctx;
        let Some(t) = named_type_type(host, named_type) else {
            return false;
        };
        let Some(interface_element) = ctx.interface_element(t) else {
            return false;
        };
        if matches!(
            interface_element.raw().tag(),
            Tag::Enum | Tag::ExtensionType
        ) {
            return false;
        }
        let all_ok = ctx
            .instance(interface_element.upcast())
            .fields
            .iter()
            .all(|&f| {
                let flags = first_fragment_flags(&ctx, f.raw());
                flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
                    || flags
                        .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
                    || flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT)
                    || flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL)
            });
        if all_ok {
            return false;
        }
        let range = node_range(host.ast(), named_type);
        host.report(diag::enum_mixin_with_instance_variable().at_offset(range.0, range.1));
        true
    }

    /// `_isNotImplementedInConcreteSuperClass(name)`: if [name] is not
    /// implemented in the extended concrete class, the issue should be
    /// fixed there.
    fn is_not_implemented_in_concrete_super_class(&self, name: MemberName) -> bool {
        let ctx = self.ctx;
        let Some(super_element) = ctx
            .element_supertype(self.class_element)
            .and_then(|t| ctx.interface_element(t))
        else {
            return false;
        };
        if super_element.raw().tag() == Tag::Class
            && !ctx
                .element_data(super_element.raw())
                .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT))
        {
            let super_interface = self.inheritance().get_interface(super_element);
            return super_interface.map.contains_key(&name);
        }
        false
    }

    /// `_reportConcreteClassWithAbstractMember(name)`: if the class itself
    /// declares an abstract member [name], reports the more specific
    /// diagnostic and returns `true`.
    fn report_concrete_class_with_abstract_member<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        name: &str,
    ) -> bool {
        let class_name = self.class_name();
        let is_enum = self.is_enum();
        let check_member_name_combo =
            |host: &mut H, member: NodeId, member_name: &str, display_name: &str| -> bool {
                if member_name != name {
                    return false;
                }
                let d = if is_enum {
                    diag::enum_with_abstract_member(display_name, &class_name)
                } else {
                    diag::concrete_class_with_abstract_member(display_name, &class_name)
                };
                let range = node_range(host.ast(), member);
                host.report(d.at_offset(range.0, range.1));
                true
            };

        for &member in &self.n.members {
            let ast = host.ast();
            if let Some(m) = ast.cast::<MethodDeclaration>(member.raw()) {
                let display_name = ast.tokens.lexeme(ast[m].name).to_string();
                let mut member_name = display_name.clone();
                if is_keyword(ast, ast[m].property_keyword, "set") {
                    member_name.push('=');
                }
                if check_member_name_combo(host, member.raw(), &member_name, &display_name) {
                    return true;
                }
            } else if let Some(f) = ast.cast::<FieldDeclaration>(member.raw()) {
                let fields = ast[f].fields;
                let is_final = is_keyword(ast, ast[fields].keyword, "final");
                let names: Vec<String> = ast
                    .list(ast[fields].variables)
                    .iter()
                    .map(|&v| ast.tokens.lexeme(ast[v].name).to_string())
                    .collect();
                for name in names {
                    if check_member_name_combo(host, member.raw(), &name, &name) {
                        return true;
                    }
                    if !is_final
                        && check_member_name_combo(host, member.raw(), &format!("{name}="), &name)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `_reportInheritedAbstractMembers(elements)`.
    fn report_inherited_abstract_members<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        elements: Option<Vec<ElemRef>>,
    ) {
        let Some(elements) = elements else {
            return;
        };
        let ctx = self.ctx;
        let mut descriptions: Vec<String> = elements
            .iter()
            .map(|&element| {
                let base = member::base_element(&ctx, element);
                let prefix = match base.tag() {
                    Tag::Getter => "getter ",
                    Tag::Setter => "setter ",
                    _ => "",
                };
                let element_name = display_name(&ctx, base);
                let enclosing_name = member::enclosing_element(&ctx, element)
                    .map_or_else(String::new, |e| display_name(&ctx, e));
                format!("{prefix}{enclosing_name}.{element_name}")
            })
            .collect();
        // Dart sorts by UTF-16 code units.
        descriptions.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));

        let d = match descriptions.len() {
            1 => diag::non_abstract_class_inherits_abstract_member_one(&descriptions[0]),
            2 => diag::non_abstract_class_inherits_abstract_member_two(
                &descriptions[0],
                &descriptions[1],
            ),
            3 => diag::non_abstract_class_inherits_abstract_member_three(
                &descriptions[0],
                &descriptions[1],
                &descriptions[2],
            ),
            4 => diag::non_abstract_class_inherits_abstract_member_four(
                &descriptions[0],
                &descriptions[1],
                &descriptions[2],
                &descriptions[3],
            ),
            _ => diag::non_abstract_class_inherits_abstract_member_five_plus(
                &descriptions[0],
                &descriptions[1],
                &descriptions[2],
                &descriptions[3],
                descriptions.len() as i64 - 4,
            ),
        };
        self.report_at_class_name(host, d);
    }

    /// `_reportNoCombinedSuperSignature(node)`.
    fn report_no_combined_super_signature<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        node: Id<MethodDeclaration>,
    ) -> bool {
        let ctx = self.ctx;
        let Some(element) = declared_element(host, node).and_then(|e| e.cast::<MethodElement>())
        else {
            return false;
        };
        if let Some(TopLevelInferenceError::OverrideNoCombinedSuperSignature {
            candidate_signatures,
        }) = ctx.get(element).type_inference_error.try_get()
        {
            let range = token_range(host.ast(), host.ast()[node].name);
            host.report(
                diag::no_combined_super_signature(&self.class_name(), candidate_signatures)
                    .at_offset(range.0, range.1),
            );
            return true;
        }
        false
    }

    /// `_verifyMustBeOverridden()`: the class complies with all
    /// `@mustBeOverridden` members of its supertypes.
    fn verify_must_be_overridden<'h, H: VerifierHost<'h>>(&mut self, host: &mut H) {
        let ctx = self.ctx;
        let class_element = self.class_element;
        let is_sealed = first_fragment_flags(&ctx, class_element.raw())
            .contains(FragmentFlags::CLASS_FRAGMENT_IS_SEALED);
        if !self.is_class() || self.is_abstract() || is_sealed {
            // We only care about concrete classes.
            return;
        }

        if let Some(no_such_method) = get_method(&ctx, class_element.upcast(), "noSuchMethod")
            && !member::is_abstract(&ctx, ElemRef::Base(no_such_method))
        {
            return;
        }
        let class_library = ctx
            .element_data(class_element.raw())
            .and_then(|d| d.library);
        let mut not_overridden: Vec<ElemRef> = Vec::new();
        for &supertype in ctx.element_all_supertypes(class_element) {
            let Some(element) = ctx.interface_element(supertype) else {
                continue;
            };
            let substitution = MapSubstitution::from_interface_type(&ctx, supertype);
            let data = ctx.instance(element.upcast());
            let skip = |e: ElementId| {
                let private = ctx.element_name(e).is_none_or(|n| n.starts_with('_'));
                (private && ctx.element_data(e).and_then(|d| d.library) != class_library)
                    || member::is_static(&ctx, ElemRef::Base(e))
            };
            for &method in &data.methods {
                let method = method.raw();
                if skip(method) {
                    continue;
                }
                if self.element_has_must_be_overridden(host, method) {
                    let lookup =
                        super::correct_override::lookup_name(&ctx, method).unwrap_or_default();
                    let declaration = get_method(&ctx, class_element.upcast(), &lookup);
                    if declaration.is_none_or(|d| member::is_abstract(&ctx, ElemRef::Base(d))) {
                        not_overridden.push(ElemRef::Base(method));
                    }
                }
            }
            for &getter in &data.getters {
                let getter = getter.raw();
                if skip(getter) {
                    continue;
                }
                if self.accessor_has_must_be_overridden(host, getter) {
                    let name = ctx.element_name(getter).unwrap_or("");
                    let declaration = get_getter(&ctx, class_element.upcast(), name);
                    if declaration.is_none_or(|d| member::is_abstract(&ctx, ElemRef::Base(d))) {
                        not_overridden.push(member::substitute(
                            &ctx,
                            ElemRef::Base(getter),
                            &substitution,
                        ));
                    }
                }
            }
            for &setter in &data.setters {
                let setter = setter.raw();
                if skip(setter) {
                    continue;
                }
                if self.accessor_has_must_be_overridden(host, setter) {
                    let name = ctx.element_name(setter).unwrap_or("");
                    let declaration = get_setter(&ctx, class_element.upcast(), name);
                    if declaration.is_none_or(|d| member::is_abstract(&ctx, ElemRef::Base(d))) {
                        not_overridden.push(member::substitute(
                            &ctx,
                            ElemRef::Base(setter),
                            &substitution,
                        ));
                    }
                }
            }
        }
        if not_overridden.is_empty() {
            return;
        }

        let names_for_error: IndexSet<String> = not_overridden
            .iter()
            .map(|&e| {
                let name = member::name(&ctx, e).unwrap_or("");
                name.strip_suffix('=').unwrap_or(name).to_string()
            })
            .collect();
        let names: Vec<&String> = names_for_error.iter().collect();
        let d = match names.as_slice() {
            [member] => diag::missing_override_of_must_be_overridden_one(member),
            [first, second] => diag::missing_override_of_must_be_overridden_two(first, second),
            [first, second, rest @ ..] => diag::missing_override_of_must_be_overridden_three_plus(
                first,
                second,
                rest.len() as i64,
            ),
            [] => return,
        };
        self.report_at_class_name(host, d);
    }

    /// `getter.metadata.hasMustBeOverridden ||
    /// getter.variable.metadata.hasMustBeOverridden`.
    fn accessor_has_must_be_overridden<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &H,
        accessor: ElementId,
    ) -> bool {
        if self.element_has_must_be_overridden(host, accessor) {
            return true;
        }
        let variable = match self.ctx.any(accessor) {
            AnyElement::Getter(g) => g.variable.get(),
            AnyElement::Setter(s) => s.variable.get(),
            _ => None,
        };
        variable.is_some_and(|v| self.element_has_must_be_overridden(host, v.raw()))
    }

    /// `element.metadata.hasMustBeOverridden`, from the annotations in this
    /// unit of the fragments of [element].
    fn element_has_must_be_overridden<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &H,
        element: ElementId,
    ) -> bool {
        let ctx = self.ctx;
        let index = self.annotations_by_fragment.get_or_insert_with(|| {
            let mut index: IndexMap<FragmentId, Vec<NodeId>> = IndexMap::new();
            let rt = host.rt();
            for i in 1..host.ast().node_count() {
                let node = NodeId::from_index(i);
                if let Some(&fragment) = rt.element_annotation.get(node) {
                    index.entry(fragment).or_default().push(node);
                }
            }
            index
        });
        let mut fragment = first_fragment(&ctx, element);
        while let Some(f) = fragment {
            if let Some(annotations) = index.get(&f) {
                for &a in annotations {
                    if let Some(a) = host.ast().cast::<dartr_ast::Annotation>(a)
                        && annotation_is_top_getter(host, a, "meta", "mustBeOverridden")
                    {
                        return true;
                    }
                }
            }
            fragment = ctx.fragment_data(f).and_then(|d| d.next_fragment);
        }
        false
    }
}

/// Dart `namedType.type` (`typeOrThrow`).
fn named_type_type<'h, H: VerifierHost<'h>>(host: &H, named_type: Id<NamedType>) -> Option<TypeId> {
    host.tables()
        .annotation_type
        .get(named_type)
        .copied()
        .or_else(|| host.static_type(named_type))
}

/// Whether [token] is the keyword [keyword].
fn is_keyword(ast: &Ast, token: Option<TokenId>, keyword: &str) -> bool {
    token.is_some_and(|t| ast.tokens.lexeme(t) == keyword)
}
