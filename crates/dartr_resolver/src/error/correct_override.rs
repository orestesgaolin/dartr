// Dart source: pkg/analyzer/lib/src/error/correct_override.dart

//! `CorrectOverrideHelper` (is a member a correct override of a super
//! member) and `CovariantParametersVerifier` (the types of covariant
//! parameters against the corresponding parameters of the super members).
//!
//! The module also holds the element helpers that the D8 verifiers share
//! (Dart getters of `ElementImpl` / `FragmentImpl` and the
//! `DiagnosticFactory` methods they use).

use dartr_ast::{Annotation, Id, Identifier, NodeId, NodeList, SimpleIdentifier};
use dartr_diagnostics::{DiagnosticMessage, LocatableDiagnostic, LocatedDiagnostic, TypeArg, diag};
use dartr_element::diagnostics::{library_fragment_of, type_arg};
use dartr_element::{
    AnyElement, Ctx, EId, ElemRef, ElementId, FId, FnParam, FormalParameterElement, FragmentFlags,
    FragmentId, InstanceElement, InterfaceElement, LibraryElement, LibraryFragment, Nullability,
    Tag, TypeId, TypeKind,
};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeSystem;
use dartr_typesystem::class_hierarchy;
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::type_ext::TypeExt;
use indexmap::IndexMap;

use super::VerifierHost;

/// Dart `InvalidOverrideDiagnosticCode`: the codes that
/// [`CorrectOverrideHelper::verify`] reports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvalidOverrideCode {
    /// `diag.invalidOverride`.
    InvalidOverride,
    /// `diag.invalidOverrideSetter`.
    InvalidOverrideSetter,
    /// `diag.invalidImplementationOverride`.
    InvalidImplementationOverride,
    /// `diag.invalidImplementationOverrideSetter`.
    InvalidImplementationOverrideSetter,
}

impl InvalidOverrideCode {
    /// `code.withArguments(...)`.
    fn with_arguments(
        self,
        member_name: &str,
        declaring_interface_name: &str,
        type_in_declaring_interface: TypeArg,
        overridden_interface_name: &str,
        type_in_overridden_interface: TypeArg,
    ) -> LocatableDiagnostic {
        let f = match self {
            InvalidOverrideCode::InvalidOverride => diag::invalid_override,
            InvalidOverrideCode::InvalidOverrideSetter => diag::invalid_override_setter,
            InvalidOverrideCode::InvalidImplementationOverride => {
                diag::invalid_implementation_override
            }
            InvalidOverrideCode::InvalidImplementationOverrideSetter => {
                diag::invalid_implementation_override_setter
            }
        };
        f(
            member_name,
            declaring_interface_name,
            type_in_declaring_interface,
            overridden_interface_name,
            type_in_overridden_interface,
        )
    }
}

/// Dart `CorrectOverrideHelper`.
pub struct CorrectOverrideHelper<'a> {
    type_system: TypeSystem<'a>,
    this_member: ElemRef,
    /// `_thisTypeForSubtype`.
    this_type_for_subtype: TypeId,
}

impl<'a> CorrectOverrideHelper<'a> {
    /// `CorrectOverrideHelper(typeSystem:, thisMember:)`.
    pub fn new(type_system: TypeSystem<'a>, this_member: ElemRef) -> Self {
        let this_type_for_subtype = compute_this_type_for_subtype(&type_system, this_member);
        CorrectOverrideHelper {
            type_system,
            this_member,
            this_type_for_subtype,
        }
    }

    /// `isCorrectOverrideOf(superMember:)`: whether the member is a correct
    /// override of [super_member].
    pub fn is_correct_override_of(&self, super_member: ElemRef) -> bool {
        let ctx = self.type_system.ctx;
        let super_type = member::type_(&ctx, super_member);
        self.type_system
            .is_subtype_of(self.this_type_for_subtype, super_type)
    }

    /// `verify(superMember:, diagnosticReporter:, errorNode:,
    /// diagnosticCode:)`: reports [code] at [error_range] (offset, length)
    /// if the member is not a correct override of [super_member].
    pub fn verify<'h, H: VerifierHost<'h>>(
        &self,
        host: &mut H,
        super_member: ElemRef,
        error_range: (usize, usize),
        code: InvalidOverrideCode,
    ) {
        if self.is_correct_override_of(super_member) {
            return;
        }
        let ctx = self.type_system.ctx;
        let Some(member_name) = member::name(&ctx, self.this_member) else {
            return;
        };
        let d = invalid_override(&ctx, code, self.this_member, super_member, member_name);
        host.report(d.at_offset(error_range.0, error_range.1));
    }
}

/// `_computeThisTypeForSubtype`: the type of [this_member], with the types
/// of covariant formal parameters replaced with `Object?`.
fn compute_this_type_for_subtype(type_system: &TypeSystem<'_>, this_member: ElemRef) -> TypeId {
    let ctx = type_system.ctx;
    let t = member::type_(&ctx, this_member);
    let TypeKind::Function(f) = *ctx.ty(t) else {
        return t;
    };
    let parameters = ctx.list(f.params);
    if !parameters.iter().any(|p| p.covariant) {
        return t;
    }
    let object_question = type_system.object_question();
    let new_parameters: Vec<FnParam> = parameters
        .iter()
        .map(|p| {
            if p.covariant {
                FnParam {
                    ty: object_question,
                    ..*p
                }
            } else {
                *p
            }
        })
        .collect();
    let type_params = ctx.list(f.type_params).to_vec();
    ctx.function_type(&type_params, &new_parameters, f.ret, f.nullability, None)
}

/// `DiagnosticFactory.invalidOverride(source, code, errorNode, member,
/// superMember, memberName)` without the location.
fn invalid_override(
    ctx: &Ctx<'_>,
    code: InvalidOverrideCode,
    member: ElemRef,
    super_member: ElemRef,
    member_name: &str,
) -> LocatableDiagnostic {
    let declaring = member::enclosing_element(ctx, member)
        .and_then(|e| ctx.element_name(e))
        .unwrap_or("<unknown>");
    let overridden = member::enclosing_element(ctx, super_member)
        .and_then(|e| ctx.element_name(e))
        .unwrap_or("<unknown>");
    let d = code.with_arguments(
        member_name,
        declaring,
        type_arg(ctx, member::type_(ctx, member)),
        overridden,
        type_arg(ctx, member::type_(ctx, super_member)),
    );
    let message = match code {
        InvalidOverrideCode::InvalidOverride => "The member being overridden.",
        InvalidOverrideCode::InvalidOverrideSetter => "The setter being overridden.",
        _ => return d,
    };
    // `superMember.nonSynthetic.baseElement.firstFragmentLocation`.
    let super_element = non_synthetic(ctx, member::base_element(ctx, super_member));
    let location = first_fragment_location(ctx, super_element);
    d.with_context_messages([DiagnosticMessage {
        file_path: location.path,
        offset: location.name_offset.map_or(-1, |o| o as i64),
        length: location.name_length as i64,
        message: message.to_string(),
        url: None,
    }])
}

// ------------------------------------------------------------------ covariant

/// Dart `CovariantParametersVerifier`.
pub struct CovariantParametersVerifier<'a> {
    type_system: TypeSystem<'a>,
    this_member: ElemRef,
}

/// Dart `_SuperMember`.
struct SuperMember {
    interface: TypeId,
    raw_element: ElementId,
}

/// Dart `_SuperParameter`.
struct SuperParameter {
    element: EId<FormalParameterElement>,
    ty: TypeId,
}

impl<'a> CovariantParametersVerifier<'a> {
    /// `CovariantParametersVerifier(thisMember:)`.
    pub fn new(type_system: TypeSystem<'a>, this_member: ElemRef) -> Self {
        CovariantParametersVerifier {
            type_system,
            this_member,
        }
    }

    /// `verify(diagnosticReporter:, errorEntity:)`.
    pub fn verify<'h, H: VerifierHost<'h>>(&self, host: &mut H, error_range: (usize, usize)) {
        let ctx = self.type_system.ctx;
        let super_parameters = self.super_parameters();
        for (parameter, super_parameters) in super_parameters {
            for super_parameter in super_parameters {
                let this_type = member::type_(&ctx, parameter);
                let super_type = super_parameter.ty;
                if self.type_system.is_subtype_of(super_type, this_type)
                    || self.type_system.is_subtype_of(this_type, super_type)
                {
                    continue;
                }
                // `superParameter.member`: the enclosing executable.
                let Some(super_member) = ctx
                    .element_data(super_parameter.element.raw())
                    .and_then(|d| d.enclosing)
                else {
                    continue;
                };
                let super_member = ElemRef::Base(super_member);
                let enclosing_name = |e: ElemRef| {
                    member::enclosing_element(&ctx, e)
                        .and_then(|e| ctx.element_name(e))
                        .unwrap_or("")
                        .to_string()
                };
                let d = diag::invalid_override(
                    member::name(&ctx, self.this_member).unwrap_or(""),
                    &enclosing_name(self.this_member),
                    type_arg(&ctx, member::type_(&ctx, self.this_member)),
                    &enclosing_name(super_member),
                    type_arg(&ctx, member::type_(&ctx, super_member)),
                );
                host.report(d.at_offset(error_range.0, error_range.1));
            }
        }
    }

    /// `_superMembers()`.
    fn super_members(&self) -> Vec<SuperMember> {
        let ctx = self.type_system.ctx;
        let Some(class_element) = member::enclosing_interface(&ctx, self.this_member) else {
            return Vec::new();
        };
        let interfaces = class_hierarchy::implemented_interfaces(&ctx, class_element);
        let mut super_members = Vec::new();
        for &interface in interfaces {
            let Some(element) = ctx.interface_element(interface) else {
                continue;
            };
            if let Some(super_member) = corresponding_member(&ctx, element, self.this_member) {
                super_members.push(SuperMember {
                    interface,
                    raw_element: super_member,
                });
            }
        }
        super_members
    }

    /// `_superParameters()`.
    fn super_parameters(&self) -> IndexMap<ElemRef, Vec<SuperParameter>> {
        let ctx = self.type_system.ctx;
        let mut result: IndexMap<ElemRef, Vec<SuperParameter>> = IndexMap::new();
        let mut super_members: Option<Vec<SuperMember>> = None;
        let parameters = member::formal_parameters(&ctx, self.this_member);
        for (i, &parameter) in parameters.iter().enumerate() {
            if !member::is_covariant(&ctx, parameter) {
                continue;
            }
            let super_members = super_members.get_or_insert_with(|| self.super_members());
            for super_member in super_members.iter() {
                let super_formals = executable_formal_parameters(&ctx, super_member.raw_element);
                let Some(super_parameter) =
                    corresponding_parameter(&ctx, &super_formals, parameter, i)
                else {
                    continue;
                };
                let raw_type = member::type_(&ctx, ElemRef::Base(super_parameter.raw()));
                let super_type = self
                    .super_substitution(super_member)
                    .substitute_type(&ctx, raw_type);
                result.entry(parameter).or_default().push(SuperParameter {
                    element: super_parameter,
                    ty: super_type,
                });
            }
        }
        result
    }

    /// `_superSubstitution(superMember)`: converts types of the super member
    /// to the types of the member.
    fn super_substitution(&self, super_member: &SuperMember) -> MapSubstitution {
        let ctx = self.type_system.ctx;
        let mut result = MapSubstitution::from_interface_type(&ctx, super_member.interface);
        // If the executable has type parameters, ensure that super uses the
        // same.
        let this_type_parameters = member::type_parameters(&ctx, self.this_member);
        if !this_type_parameters.is_empty() {
            let super_type_parameters =
                member::type_parameters(&ctx, ElemRef::Base(super_member.raw_element));
            if this_type_parameters.len() == super_type_parameters.len() {
                // `Substitution.combine(result, fromPairs2(...))`: the type
                // parameters of the class and of the member are distinct.
                for (&s, &t) in super_type_parameters.iter().zip(&this_type_parameters) {
                    let t = ctx.type_parameter_type(t, Nullability::None);
                    result.map.entry(s).or_insert(t);
                }
            }
        }
        result
    }
}

/// The formal parameters of an executable element.
fn executable_formal_parameters(ctx: &Ctx<'_>, e: ElementId) -> Vec<EId<FormalParameterElement>> {
    match e.cast::<dartr_element::ExecutableElement>() {
        Some(e) => ctx.executable(e).formal_params.clone(),
        None => Vec::new(),
    }
}

/// `_correspondingMember(classElement, proto)`.
fn corresponding_member(
    ctx: &Ctx<'_>,
    class_element: EId<InterfaceElement>,
    proto: ElemRef,
) -> Option<ElementId> {
    let base = member::base_element(ctx, proto);
    let display = display_name(ctx, base);
    match base.tag() {
        Tag::Method => get_method(ctx, class_element.upcast(), &display),
        Tag::Getter => get_getter(ctx, class_element.upcast(), &display),
        Tag::Setter => get_setter(ctx, class_element.upcast(), &display),
        _ => None,
    }
}

/// `_correspondingParameter(parameters, proto, protoIndex)`.
fn corresponding_parameter(
    ctx: &Ctx<'_>,
    parameters: &[EId<FormalParameterElement>],
    proto: ElemRef,
    proto_index: usize,
) -> Option<EId<FormalParameterElement>> {
    let proto_base = member::base_element(ctx, proto).cast::<FormalParameterElement>()?;
    let proto_data = ctx.get(proto_base);
    if proto_data.kind.is_positional() {
        let &parameter = parameters.get(proto_index)?;
        if ctx.get(parameter).kind.is_positional() {
            return Some(parameter);
        }
    } else {
        for &parameter in parameters {
            let p = ctx.get(parameter);
            if p.kind.is_named() && p.name == proto_data.name {
                return Some(parameter);
            }
        }
    }
    None
}

// ------------------------------------------------------------------ helpers

/// The element of [fragment] (Dart `fragment.element`).
pub(crate) fn fragment_element(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<ElementId> {
    ctx.fragment_data(fragment)?.element.try_get().copied()
}

/// Dart `node.declaredFragment`.
pub(crate) fn declared_fragment<'h, H: VerifierHost<'h>>(
    host: &H,
    node: impl Into<NodeId>,
) -> Option<FragmentId> {
    host.tables().declared_fragment.get(node.into()).copied()
}

/// Dart `node.declaredFragment?.element`.
pub(crate) fn declared_element<'h, H: VerifierHost<'h>>(
    host: &H,
    node: impl Into<NodeId>,
) -> Option<ElementId> {
    let fragment = declared_fragment(host, node)?;
    fragment_element(&host.ctx(), fragment)
}

/// The flags of [fragment].
pub(crate) fn fragment_flags(ctx: &Ctx<'_>, fragment: FragmentId) -> FragmentFlags {
    ctx.fragment_data(fragment)
        .map(|f| f.flags.get())
        .unwrap_or_default()
}

/// The flags of the first fragment of [e].
pub(crate) fn first_fragment_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    ctx.element_data(e)
        .map(|d| fragment_flags(ctx, d.first_fragment))
        .unwrap_or_default()
}

/// The first fragment of [e].
pub(crate) fn first_fragment(ctx: &Ctx<'_>, e: ElementId) -> Option<FragmentId> {
    ctx.element_data(e).map(|d| d.first_fragment)
}

/// Dart `fragment.isAugmentation`.
pub(crate) fn is_augmentation(ctx: &Ctx<'_>, fragment: FragmentId) -> bool {
    fragment_flags(ctx, fragment).contains(FragmentFlags::FRAGMENT_IS_AUGMENTATION)
}

/// Dart `ElementImpl.isAugmentationWithoutAugmentedDeclaration`.
pub(crate) fn is_augmentation_without_augmented_declaration(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let Some(data) = ctx.element_data(e) else {
        return false;
    };
    let Some(first) = ctx.fragment_data(data.first_fragment) else {
        return false;
    };
    first.flags.has(FragmentFlags::FRAGMENT_IS_AUGMENTATION) && first.previous_fragment.is_none()
}

/// Dart `element.displayName`: the name (the name without `=` for a
/// setter), `<unnamed>` without a name.
pub(crate) fn display_name(ctx: &Ctx<'_>, e: ElementId) -> String {
    match ctx.element_name(e) {
        Some(name) => name.to_string(),
        None => "<unnamed>".to_string(),
    }
}

/// Dart `element.lookupName`.
pub(crate) fn lookup_name(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
    member::lookup_name(ctx, ElemRef::Base(e))
}

/// The UTF-16 length of [text] (Dart `String.length`).
pub(crate) fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Dart `element.nonSynthetic`, without the panic of
/// `dartr_element::diagnostics::non_synthetic` for a field declared by a
/// formal parameter of a primary constructor (Dart
/// `FieldElementImpl.declaringFormalParameter`).
pub(crate) fn non_synthetic(ctx: &Ctx<'_>, element: ElementId) -> ElementId {
    let origin_variable = |e: ElementId| {
        first_fragment_flags(ctx, e)
            .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
    };
    let variable = match ctx.any(element) {
        AnyElement::Getter(g) if origin_variable(element) => g.variable.get().map(|v| v.raw()),
        AnyElement::Setter(s) if origin_variable(element) => s.variable.get().map(|v| v.raw()),
        _ => None,
    };
    let field = variable.unwrap_or(element);
    if field.tag() == Tag::Field {
        let flags = first_fragment_flags(ctx, field);
        if !flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
            && flags.contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER)
        {
            return declaring_formal_parameter(ctx, field).unwrap_or(field);
        }
    }
    dartr_element::diagnostics::non_synthetic(ctx, element)
}

/// Dart `FieldElementImpl.declaringFormalParameter`: the declaring formal
/// parameter of a primary constructor that declares [field].
fn declaring_formal_parameter(ctx: &Ctx<'_>, field: ElementId) -> Option<ElementId> {
    let enclosing = ctx
        .element_data(field)?
        .enclosing?
        .cast::<InterfaceElement>()?;
    for &constructor in &ctx.interface(enclosing).constructors {
        for &parameter in &ctx.get(constructor).formal_params {
            if ctx.get(parameter).field.get().map(|f| f.raw()) == Some(field) {
                return Some(parameter.raw());
            }
        }
    }
    None
}

/// Dart `FirstFragmentLocation` of an element.
pub(crate) struct FirstFragmentLocation {
    /// `libraryFragment.source.fullName`.
    pub path: String,
    pub name_offset: Option<u32>,
    /// The UTF-16 length of `name` (0 without a name).
    pub name_length: usize,
}

/// Dart `ElementImpl.firstFragmentLocation`.
pub(crate) fn first_fragment_location(ctx: &Ctx<'_>, e: ElementId) -> FirstFragmentLocation {
    let first = first_fragment(ctx, e);
    let fragment = first.and_then(|f| ctx.fragment_data(f));
    let library_fragment = first.and_then(|f| library_fragment_of(ctx, f));
    FirstFragmentLocation {
        path: library_fragment
            .map(|lf| ctx.fragment(lf).source.path.to_string())
            .unwrap_or_default(),
        name_offset: fragment.and_then(|f| f.name_offset),
        name_length: fragment
            .and_then(|f| f.name)
            .map_or(0, |n| utf16_len(ctx.name_str(n))),
    }
}

/// Dart `element.diagnosticRange(source)`: the name of the first fragment
/// of the non-synthetic element, as (offset, length).
pub(crate) fn diagnostic_range(ctx: &Ctx<'_>, e: ElementId) -> (usize, usize) {
    let element = non_synthetic(ctx, e);
    let location = first_fragment_location(ctx, element);
    (
        location.name_offset.map_or(0, |o| o as usize),
        location.name_length,
    )
}

/// Dart `FragmentImpl.offset`: the name offset, else the first token.
pub(crate) fn fragment_offset(ctx: &Ctx<'_>, fragment: FragmentId) -> i64 {
    match ctx.fragment_data(fragment) {
        Some(f) => f
            .name_offset
            .or(f.first_token_offset)
            .map_or(0, |o| o as i64),
        None => -1,
    }
}

/// `DiagnosticFactory.duplicateDefinition(diagnostic, duplicateFragment,
/// originalElement)`.
pub(crate) fn duplicate_definition(
    ctx: &Ctx<'_>,
    diagnostic: LocatableDiagnostic,
    duplicate_fragment: FragmentId,
    original_element: ElementId,
) -> LocatedDiagnostic {
    let original = non_synthetic(ctx, original_element);
    let original_fragment = first_fragment(ctx, original);
    let path = original_fragment
        .and_then(|f| library_fragment_of(ctx, f))
        .map(|lf| ctx.fragment(lf).source.path.to_string())
        .unwrap_or_default();
    let context = DiagnosticMessage {
        file_path: path,
        offset: original_fragment.map_or(-1, |f| fragment_offset(ctx, f)),
        length: ctx.element_name(original).map_or(0, utf16_len) as i64,
        message: "The first definition of this name.".to_string(),
        url: None,
    };
    let duplicate = ctx.fragment_data(duplicate_fragment);
    let offset = duplicate
        .and_then(|f| f.name_offset)
        .map_or(0, |o| o as usize);
    let length = duplicate
        .and_then(|f| f.name)
        .map_or(0, |n| utf16_len(ctx.name_str(n)));
    diagnostic
        .with_context_messages([context])
        .at_offset(offset, length)
}

/// `DiagnosticFactory.duplicateDefinitionForNodes(source, diagnostic,
/// duplicateNode, originalNode)`, with the ranges of the nodes.
pub(crate) fn duplicate_definition_for_nodes(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    diagnostic: LocatableDiagnostic,
    duplicate: (usize, usize),
    original: (usize, usize),
) -> LocatedDiagnostic {
    let path = ctx.fragment(fragment).source.path.to_string();
    diagnostic
        .with_context_messages([DiagnosticMessage {
            file_path: path,
            offset: original.0 as i64,
            length: original.1 as i64,
            message: "The first definition of this name.".to_string(),
            url: None,
        }])
        .at_offset(duplicate.0, duplicate.1)
}

/// The range of [token] (Dart `SyntacticEntity` offset and length).
pub(crate) fn token_range(ast: &dartr_ast::Ast, token: TokenId) -> (usize, usize) {
    let t = ast.tokens.get(token);
    (t.offset as usize, (t.end() - t.offset) as usize)
}

/// The range of [node].
pub(crate) fn node_range(ast: &dartr_ast::Ast, node: impl Into<NodeId>) -> (usize, usize) {
    let node = node.into();
    (ast.offset(node) as usize, ast.length(node) as usize)
}

/// Dart `token.isSynthetic`.
pub(crate) fn token_is_synthetic(ast: &dartr_ast::Ast, token: TokenId) -> bool {
    ast.tokens.get(token).is_synthetic()
}

/// Dart `libraryElement.featureSet.isEnabled(flag)`.
pub(crate) fn library_feature_enabled(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    flag: dartr_parser::experimental_flags::ExperimentalFlag,
) -> bool {
    crate::scope::library_feature_enabled(ctx, library, flag)
}

/// Dart `LibraryElementImpl.hasWildcardVariablesFeatureEnabled`.
pub(crate) fn has_wildcard_variables_feature_enabled(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
) -> bool {
    library_feature_enabled(
        ctx,
        library,
        dartr_parser::experimental_flags::ExperimentalFlag::WildcardVariables,
    )
}

/// Dart `Element.isWildcardVariable` (`ElementExtension`).
pub(crate) fn is_wildcard_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let local_kind = matches!(
        e.tag(),
        Tag::LocalFunction
            | Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::Prefix
            | Tag::TypeParameter
            | Tag::FormalParameter
    );
    if !local_kind || ctx.element_name(e) != Some("_") {
        return false;
    }
    match ctx.element_data(e).and_then(|d| d.library) {
        Some(library) => has_wildcard_variables_feature_enabled(ctx, library),
        None => false,
    }
}

/// Dart `InstanceElementImpl.getMethod(name)`: by `lookupName`.
pub(crate) fn get_method(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<ElementId> {
    ctx.instance(element)
        .methods
        .iter()
        .map(|m| m.raw())
        .find(|&m| lookup_name(ctx, m).as_deref() == Some(name))
}

/// Dart `InstanceElementImpl.getGetter(name)`.
pub(crate) fn get_getter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<ElementId> {
    ctx.instance(element)
        .getters
        .iter()
        .map(|m| m.raw())
        .find(|&m| ctx.element_name(m) == Some(name))
}

/// Dart `InstanceElementImpl.getSetter(name)` (the name without `=`).
pub(crate) fn get_setter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<ElementId> {
    ctx.instance(element)
        .setters
        .iter()
        .map(|m| m.raw())
        .find(|&m| ctx.element_name(m) == Some(name))
}

/// Dart `ElementAnnotation.element` of [annotation]: the resolved element
/// of the annotation, else the scope lookup result of its name (when the
/// annotation resolver has not recorded it).
pub(crate) fn annotation_element<'h, H: VerifierHost<'h>>(
    host: &H,
    annotation: Id<Annotation>,
) -> Option<ElementId> {
    let ctx = host.ctx();
    if let Some(e) = host.element(annotation) {
        return Some(member::base_element(&ctx, e));
    }
    let ast = host.ast();
    let name: Id<Identifier> = ast[annotation].name;
    if let Some(e) = host.element(name) {
        return Some(member::base_element(&ctx, e));
    }
    if ast[annotation].constructor_name.is_some() {
        return None;
    }
    let simple = ast.cast::<SimpleIdentifier>(name.raw())?;
    host.rt().scope_lookup_result.get(simple)?.getter
}

/// Dart `ElementAnnotationImpl._isTopGetter(libraryName:, name:)`.
pub(crate) fn annotation_is_top_getter<'h, H: VerifierHost<'h>>(
    host: &H,
    annotation: Id<Annotation>,
    library_name: &str,
    name: &str,
) -> bool {
    let ctx = host.ctx();
    match annotation_element(host, annotation) {
        Some(e) => e.tag() == Tag::Getter && ctx.is_element(e, library_name, name),
        None => false,
    }
}

/// Whether one of [annotations] is the top-level getter [name] of the
/// library named [library_name] (Dart `metadata.hasOverride`,
/// `hasRedeclare`, `hasMustBeOverridden` of the element that the node with
/// these annotations declares).
pub(crate) fn has_annotation<'h, H: VerifierHost<'h>>(
    host: &H,
    annotations: NodeList<Annotation>,
    library_name: &str,
    name: &str,
) -> bool {
    host.ast()
        .list(annotations)
        .iter()
        .any(|&a| annotation_is_top_getter(host, a, library_name, name))
}
