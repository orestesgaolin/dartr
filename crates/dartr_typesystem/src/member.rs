// Dart source: pkg/analyzer/lib/src/dart/element/member.dart
// (SubstitutedElementImpl, SubstitutedExecutableElementImpl and subclasses,
// SubstitutedVariableElementImpl and subclasses, _SubstitutedTypeParameters),
// and the getters of the `Internal*Element` interfaces that both base
// elements and substituted members implement (element.dart:
// ExecutableElementImpl.isAbstract / isStatic / lookupName,
// MethodElementImpl.lookupName, SetterElementImpl.lookupName,
// ElementImpl.isAccessibleIn)

//! Substituted members (`Substituted*ElementImpl`) as [`ElemRef::Member`]
//! values (design §1.3): an interned pair of the base element and a
//! substitution ([`Member`]). The Dart getters of members (`type`,
//! `returnType`, `formalParameters`, `enclosingElement`, `baseElement`,
//! `variable`, ...) are the functions of this module; they also accept a
//! base element ([`ElemRef::Base`]), so callers use one API for the Dart
//! `InternalExecutableElement` / `InternalVariableElement` interfaces.
//!
//! Identity: Dart creates a new member object on each `substitute` call and
//! compares members by identity. Rust interns members, so two members with
//! the same base and the same substitution are equal. Generic methods get
//! fresh type parameters on each substitution (`_SubstitutedTypeParameters`),
//! so their members stay distinct, as in Dart.
//!
//! Lazy member types (Dart `_type`) are cached in the interner
//! ([`Ctx::member_type_cached`]).

use dartr_element::{
    ClassElement, Ctx, EId, ElemRef, ElementFlags, ElementId, ExecutableElement, FieldElement,
    FormalParameterElement, FragmentFlags, GetterElement, InterfaceElement, LibraryElement, Member,
    MethodElement, Nullability, PropertyAccessorElement, SetterElement, SubstPair, Tag, TypeId,
    TypeParameterElement, VariableElement,
};
use indexmap::IndexMap;

use crate::element_type;
use crate::type_algebra::MapSubstitution;
use crate::type_ext::TypeExt;

/// Slots of [`Ctx::member_type_cached`].
const SLOT_TYPE: u8 = 0;

/// `baseElement`: the declaration of a member, or the element itself.
pub fn base_element(ctx: &Ctx<'_>, e: ElemRef) -> ElementId {
    match e {
        ElemRef::Base(b) => b,
        ElemRef::Member(m) => ctx.member(m).base,
    }
}

/// `substitution` of a member (`Substitution.empty` for a base element).
/// The pairs come in the canonical order of the interned substitution.
pub fn substitution(ctx: &Ctx<'_>, e: ElemRef) -> MapSubstitution {
    match e {
        ElemRef::Base(_) => MapSubstitution::empty(),
        ElemRef::Member(m) => {
            let pairs = ctx.subst(ctx.member(m).subst);
            MapSubstitution::from_map(pairs.iter().copied().collect())
        }
    }
}

/// Interns `Member(base, substitution)`; a base element when the
/// substitution is empty.
fn make_member(ctx: &Ctx<'_>, base: ElementId, substitution: &MapSubstitution) -> ElemRef {
    if substitution.is_empty() {
        return ElemRef::Base(base);
    }
    let mut pairs: Vec<SubstPair> = substitution.map.iter().map(|(&k, &v)| (k, v)).collect();
    let subst = ctx.intern_subst(&mut pairs);
    ElemRef::Member(ctx.intern_member(Member { base, subst }))
}

/// `substitute(substitution)` of an `Internal*Element`: a base element
/// becomes a member (`Substituted*ElementImpl(baseElement, substitution)`),
/// a member gets `this.substitution.andThen(substitution)`. An empty
/// [substitution] returns [e] itself.
///
/// Methods get fresh type parameters (`SubstitutedMethodElementImpl`
/// factory, `_SubstitutedTypeParameters`).
pub fn substitute(ctx: &Ctx<'_>, e: ElemRef, substitution: &MapSubstitution) -> ElemRef {
    if substitution.is_empty() {
        return e;
    }
    let base = base_element(ctx, e);
    let combined = match e {
        ElemRef::Base(_) => substitution.clone(),
        ElemRef::Member(_) => self::substitution(ctx, e).and_then(ctx, substitution),
    };
    if base.tag() == Tag::Method {
        let type_parameters = &ctx.get(EId::<MethodElement>::from_raw(base)).type_params;
        let fresh = substituted_type_parameters(ctx, type_parameters, &combined);
        return make_member(ctx, base, &fresh.1);
    }
    make_member(ctx, base, &combined)
}

/// `_SubstitutedTypeParameters(elements, substitution)`: fresh copies of
/// [elements] with substituted bounds, and the substitution that also maps
/// [elements] to the fresh copies.
fn substituted_type_parameters(
    ctx: &Ctx<'_>,
    elements: &[EId<TypeParameterElement>],
    substitution: &MapSubstitution,
) -> (Vec<EId<TypeParameterElement>>, MapSubstitution) {
    if elements.is_empty() {
        return (Vec::new(), substitution.clone());
    }
    // Create type formals with specialized bounds.
    // For example `<U extends T>` where T comes from an outer scope.
    let new_elements: Vec<EId<TypeParameterElement>> =
        elements.iter().map(|&e| ctx.fresh_copy(e)).collect();
    let new_types: Vec<TypeId> = new_elements
        .iter()
        .map(|&e| ctx.type_parameter_type(e, Nullability::None))
        .collect();
    // Update bounds to reference new TypeParameterElement(s).
    let substitution2 = MapSubstitution::from_pairs(elements, &new_types);
    for (i, &element) in elements.iter().enumerate() {
        if let Some(bound) = ctx.type_parameter_bound(element) {
            let new_bound = substitution2.substitute_type(ctx, bound);
            let new_bound = substitution.substitute_type(ctx, new_bound);
            ctx.get(new_elements[i]).bound.set(Some(new_bound));
        }
    }
    if substitution.is_empty() {
        return (new_elements, substitution2);
    }
    let mut map: IndexMap<EId<TypeParameterElement>, TypeId> = substitution.map.clone();
    for (&k, &v) in &substitution2.map {
        map.insert(k, v);
    }
    (new_elements, MapSubstitution::from_map(map))
}

/// `SubstitutedConstructorElementImpl.from2(element, definingType)`.
pub fn constructor_from2(ctx: &Ctx<'_>, element: ElementId, defining_type: TypeId) -> ElemRef {
    if ctx.type_arguments(defining_type).is_empty() {
        return ElemRef::Base(element);
    }
    make_member(
        ctx,
        element,
        &MapSubstitution::from_interface_type(ctx, defining_type),
    )
}

// ------------------------------------------------------------------ getters

/// `enclosingElement` (members: `baseElement.enclosingElement`).
pub fn enclosing_element(ctx: &Ctx<'_>, e: ElemRef) -> Option<ElementId> {
    ctx.element_data(base_element(ctx, e))?.enclosing
}

/// `library`.
pub fn library(ctx: &Ctx<'_>, e: ElemRef) -> Option<EId<LibraryElement>> {
    ctx.element_data(base_element(ctx, e))?.library
}

/// `name`.
pub fn name<'a>(ctx: &Ctx<'a>, e: ElemRef) -> Option<&'a str> {
    ctx.element_name(base_element(ctx, e))
}

/// `lookupName`: `name=` for setters, `unary-` for the unary minus
/// operator, else the name.
pub fn lookup_name(ctx: &Ctx<'_>, e: ElemRef) -> Option<String> {
    let base = base_element(ctx, e);
    let name = ctx.element_name(base)?;
    match base.tag() {
        Tag::Setter => Some(format!("{name}=")),
        Tag::Method => {
            let method = ctx.get(EId::<MethodElement>::from_raw(base));
            if name == "-" && method.formal_params.is_empty() {
                Some("unary-".to_string())
            } else {
                Some(name.to_string())
            }
        }
        _ => Some(name.to_string()),
    }
}

/// `ExecutableElementImpl.isAbstract`: every fragment is abstract.
pub fn is_abstract(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    let base = base_element(ctx, e);
    let Some(data) = ctx.element_data(base) else {
        return false;
    };
    let mut fragment = Some(data.first_fragment);
    while let Some(f) = fragment {
        let Some(fd) = ctx.fragment_data(f) else {
            return false;
        };
        if !fd.flags.has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT) {
            return false;
        }
        fragment = fd.next_fragment;
    }
    true
}

/// `isStatic` of an executable (`_firstFragment.isStatic`) or of a
/// field / top-level variable.
pub fn is_static(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    let base = base_element(ctx, e);
    let Some(data) = ctx.element_data(base) else {
        return false;
    };
    let Some(fd) = ctx.fragment_data(data.first_fragment) else {
        return false;
    };
    match base.tag() {
        Tag::Field | Tag::TopLevelVariable => {
            fd.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
        }
        _ => fd.flags.has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC),
    }
}

/// `isExtensionTypeMember`.
pub fn is_extension_type_member(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    let base = base_element(ctx, e);
    ctx.element_data(base).is_some_and(|d| {
        d.flags
            .has(ElementFlags::EXECUTABLE_ELEMENT_IS_EXTENSION_TYPE_MEMBER)
    })
}

/// `FormalParameterElement.isCovariant`.
pub fn is_covariant(ctx: &Ctx<'_>, parameter: ElemRef) -> bool {
    let base = base_element(ctx, parameter);
    ctx.element_data(base).is_some_and(|d| {
        d.flags
            .has(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT)
    })
}

/// Whether [e] is a getter (`InternalGetterElement`).
pub fn is_getter(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    base_element(ctx, e).tag() == Tag::Getter
}

/// Whether [e] is a setter (`InternalSetterElement`).
pub fn is_setter(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    base_element(ctx, e).tag() == Tag::Setter
}

/// Whether [e] is a method (`InternalMethodElement`).
pub fn is_method(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    base_element(ctx, e).tag() == Tag::Method
}

/// `ElementImpl.isAccessibleIn(library)`: a private name is accessible only
/// in its library.
pub fn is_accessible_in(ctx: &Ctx<'_>, e: ElemRef, library: EId<LibraryElement>) -> bool {
    match name(ctx, e) {
        Some(n) if !n.starts_with('_') => true,
        // InstanceElementImpl: `name != null && isPrivate`; ElementImpl:
        // `name == null || isPrivate`. Executables always have a name.
        _ => self::library(ctx, e) == Some(library),
    }
}

/// `isObjectMember` (`ExecutableElement2Extension`): the enclosing element
/// is the class `Object` of `dart:core`.
pub fn is_object_member(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    enclosing_element(ctx, e).is_some_and(|c| is_dart_core_object_element(ctx, c))
}

/// `ClassElement.isDartCoreObject`.
pub fn is_dart_core_object_element(ctx: &Ctx<'_>, element: ElementId) -> bool {
    element.is::<ClassElement>() && ctx.is_element(element, "dart.core", "Object")
}

/// `typeParameters` of an executable. For a method member these are the
/// fresh type parameters of `_SubstitutedTypeParameters` (the substitution
/// maps each declared type parameter to a fresh one).
pub fn type_parameters(ctx: &Ctx<'_>, e: ElemRef) -> Vec<EId<TypeParameterElement>> {
    let base = base_element(ctx, e);
    let Some(executable) = base.cast::<ExecutableElement>() else {
        return Vec::new();
    };
    let declared = &ctx.executable(executable).type_params;
    match e {
        ElemRef::Base(_) => declared.clone(),
        ElemRef::Member(_) => {
            if base.tag() != Tag::Method {
                return Vec::new();
            }
            let substitution = self::substitution(ctx, e);
            declared
                .iter()
                .map(|p| {
                    let t = substitution.map[p];
                    match *ctx.ty(t) {
                        dartr_element::TypeKind::TypeParameter { param, .. } => param,
                        _ => unreachable!("fresh type parameter"),
                    }
                })
                .collect()
        }
    }
}

/// `formalParameters`: for a member, each parameter substituted
/// (`SubstitutedFormalParameterElementImpl`).
pub fn formal_parameters(ctx: &Ctx<'_>, e: ElemRef) -> Vec<ElemRef> {
    let base = base_element(ctx, e);
    let Some(executable) = base.cast::<ExecutableElement>() else {
        return Vec::new();
    };
    let params = &ctx.executable(executable).formal_params;
    let substitution = self::substitution(ctx, e);
    params
        .iter()
        .map(|p| substitute(ctx, ElemRef::Base(p.raw()), &substitution))
        .collect()
}

/// `returnType` of an executable.
pub fn return_type(ctx: &Ctx<'_>, e: ElemRef) -> TypeId {
    let base = base_element(ctx, e);
    let executable = base
        .cast::<ExecutableElement>()
        .expect("returnType of an executable");
    dartr_element::type_inference::ensure_accessor_return_type(ctx, executable);
    let result = ctx
        .executable(executable)
        .return_type
        .get()
        .unwrap_or(TypeId::INVALID);
    match e {
        ElemRef::Base(_) => result,
        ElemRef::Member(_) => self::substitution(ctx, e).substitute_type(ctx, result),
    }
}

/// `type`: the function type of an executable
/// (`substitution.mapFunctionType(baseElement.type)` for a member), or the
/// type of a variable (`substitution.substituteType(baseElement.type)`).
pub fn type_(ctx: &Ctx<'_>, e: ElemRef) -> TypeId {
    match e {
        ElemRef::Base(base) => base_type(ctx, base),
        ElemRef::Member(m) => {
            let compute_ctx = if m.is_local() {
                *ctx
            } else {
                crate::type_ext::cache_ctx(ctx, |store| crate::type_ext::member_mentions_store(ctx, m, store))
            };
            ctx.member_type_cached(m, SLOT_TYPE, || {
                let base = ctx.member(m).base;
                let t = base_type(&compute_ctx, base);
                self::substitution(&compute_ctx, e).substitute_type(&compute_ctx, t)
            })
        }
    }
}

/// `type` of a base element.
fn base_type(ctx: &Ctx<'_>, base: ElementId) -> TypeId {
    if let Some(executable) = base.cast::<ExecutableElement>() {
        return element_type::executable_type(ctx, executable);
    }
    if let Some(variable) = base.cast::<VariableElement>() {
        return variable_type(ctx, variable);
    }
    panic!("type of {base:?}")
}

/// `VariableElementImpl.type` of a base variable.
fn variable_type(ctx: &Ctx<'_>, variable: EId<VariableElement>) -> TypeId {
    let raw = variable.raw();
    if let Some(p) = raw.cast::<FormalParameterElement>() {
        dartr_element::type_inference::ensure_formal_parameter_type(ctx, p);
        return ctx.get(p).type_.get().unwrap_or(TypeId::INVALID);
    }
    // Dart `PropertyInducingElementImpl.type`: infers on demand while
    // linking.
    dartr_element::type_inference::ensure_variable_type(ctx, raw);
    if let Some(f) = raw.cast::<FieldElement>() {
        return ctx.get(f).type_.get().unwrap_or(TypeId::INVALID);
    }
    match ctx.any(raw) {
        dartr_element::AnyElement::TopLevelVariable(v) => v.type_.get().unwrap_or(TypeId::INVALID),
        dartr_element::AnyElement::LocalVariable(v) => v.type_.get().unwrap_or(TypeId::INVALID),
        _ => TypeId::INVALID,
    }
}

/// `PropertyAccessorElement.variable`: for a member whose variable is a
/// field, `SubstitutedFieldElementImpl(variable, substitution)`.
pub fn variable(ctx: &Ctx<'_>, accessor: ElemRef) -> Option<ElemRef> {
    let base = base_element(ctx, accessor);
    let accessor_id = base.cast::<PropertyAccessorElement>()?;
    let variable = ctx.property_accessor(accessor_id).variable.get()?;
    let variable = variable.raw();
    match accessor {
        ElemRef::Member(_) if variable.is::<FieldElement>() => {
            Some(make_member(ctx, variable, &substitution(ctx, accessor)))
        }
        _ => Some(ElemRef::Base(variable)),
    }
}

/// `correspondingSetter` of a getter (`baseElement.variable.setter`,
/// substituted).
pub fn corresponding_setter(ctx: &Ctx<'_>, getter: ElemRef) -> Option<ElemRef> {
    let base = base_element(ctx, getter).cast::<GetterElement>()?;
    let variable = ctx.get(base).variable.get()?;
    let setter = ctx.property_inducing(variable).setter?;
    Some(substitute(
        ctx,
        ElemRef::Base(setter.raw()),
        &substitution(ctx, getter),
    ))
}

/// `correspondingGetter` of a setter (`baseElement.variable.getter`,
/// substituted).
pub fn corresponding_getter(ctx: &Ctx<'_>, setter: ElemRef) -> Option<ElemRef> {
    let base = base_element(ctx, setter).cast::<SetterElement>()?;
    let variable = ctx.get(base).variable.get()?;
    let getter = ctx.property_inducing(variable).getter?;
    Some(substitute(
        ctx,
        ElemRef::Base(getter.raw()),
        &substitution(ctx, setter),
    ))
}

/// The interface element that declares [e] (`enclosingElement` as an
/// `InterfaceElement`), if any.
pub fn enclosing_interface(ctx: &Ctx<'_>, e: ElemRef) -> Option<EId<InterfaceElement>> {
    enclosing_element(ctx, e)?.cast::<InterfaceElement>()
}
