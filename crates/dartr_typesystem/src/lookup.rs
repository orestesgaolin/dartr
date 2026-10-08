// Dart source: pkg/analyzer/lib/src/dart/element/element.dart
// (InstanceElementImpl.getGetter / getMethod / getSetter / lookUpGetter /
// lookUpMethod / lookUpSetter / _implementationsOfGetter /
// _implementationsOfMethod / _implementationsOfSetter,
// InterfaceElementImpl.lookUpConcreteMethod /
// lookUpInheritedConcreteGetter / lookUpInheritedConcreteMethod /
// lookUpInheritedConcreteSetter / lookUpInheritedMethod /
// lookupStaticGetter / lookupStaticMethod),
// pkg/analyzer/lib/src/dart/element/type.dart (InterfaceTypeImpl.getGetter /
// getMethod / getSetter / lookUpConstructor / lookUpGetter / lookUpMethod /
// lookUpSetter)

//! Member lookup on elements and interface types.
//!
//! - `InstanceElementImpl` / `InterfaceElementImpl` methods take the
//!   element: [`get_getter`], [`look_up_getter`], [`look_up_concrete_method`],
//!   [`look_up_inherited_method`], ... They walk the superclass chain and the
//!   mixins (`_implementationsOf*`), except [`look_up_inherited_method`],
//!   which asks the [`InheritanceManager3`].
//! - `InterfaceTypeImpl` methods take the type and substitute its type
//!   arguments: [`type_get_getter`], [`type_look_up_method`], ...

use dartr_element::{
    ConstructorElement, Ctx, EId, ElemRef, ElementId, GetterElement, InstanceElement,
    InterfaceElement, LibraryElement, MethodElement, Requirement, SetterElement, TypeId,
};
use indexmap::IndexSet;

use crate::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use crate::member;
use crate::type_algebra::MapSubstitution;
use crate::type_ext::TypeExt;

// ------------------------------------------------------------------ InstanceElementImpl

/// `InstanceElementImpl.getGetter(name)`.
pub fn get_getter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<EId<GetterElement>> {
    ctx.req.record(Requirement::InstanceElementGetGetter {
        element: element.raw(),
        name: ctx.name(name),
    });
    ctx.instance(element)
        .getters
        .iter()
        .copied()
        .find(|&e| ctx.element_name(e.raw()) == Some(name))
}

/// `InstanceElementImpl.getMethod(name)` (compares `lookupName`).
pub fn get_method(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<EId<MethodElement>> {
    ctx.req.record(Requirement::InstanceElementGetMethod {
        element: element.raw(),
        name: ctx.name(name),
    });
    ctx.instance(element)
        .methods
        .iter()
        .copied()
        .find(|&e| member::lookup_name(ctx, ElemRef::Base(e.raw())).as_deref() == Some(name))
}

/// `InstanceElementImpl.getSetter(name)` ([name] without `=`).
pub fn get_setter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
) -> Option<EId<SetterElement>> {
    ctx.req.record(Requirement::InstanceElementGetSetter {
        element: element.raw(),
        name: ctx.name(name),
    });
    ctx.instance(element)
        .setters
        .iter()
        .copied()
        .find(|&e| ctx.element_name(e.raw()) == Some(name))
}

/// The kind of member of `_implementationsOf*`.
#[derive(Clone, Copy)]
enum Kind {
    Getter,
    Method,
    Setter,
}

/// `_implementationsOfGetter` / `_implementationsOfMethod` /
/// `_implementationsOfSetter`: the members named [name] of [element], its
/// mixins (last first) and its superclasses.
fn implementations_of(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
    kind: Kind,
) -> Vec<ElementId> {
    let get = |e: EId<InstanceElement>| -> Option<ElementId> {
        match kind {
            Kind::Getter => get_getter(ctx, e, name).map(|g| g.raw()),
            Kind::Method => get_method(ctx, e, name).map(|m| m.raw()),
            Kind::Setter => get_setter(ctx, e, name).map(|s| s.raw()),
        }
    };
    let mut result = Vec::new();
    let mut visited_elements = IndexSet::new();
    let mut element = Some(element);
    while let Some(e) = element {
        if !visited_elements.insert(e) {
            break;
        }
        if let Some(found) = get(e) {
            result.push(found);
        }
        let Some(interface) = e.raw().cast::<InterfaceElement>() else {
            return result;
        };
        for &mixin in ctx.element_mixins(interface).iter().rev() {
            let mixin_element = ctx.interface_element(mixin).unwrap();
            if let Some(found) = get(mixin_element.upcast()) {
                result.push(found);
            }
        }
        element = ctx
            .element_supertype(interface)
            .and_then(|s| ctx.interface_element(s))
            .map(|s| s.upcast());
    }
    result
}

/// `InstanceElementImpl.lookUpGetter(name:, library:)`.
pub fn look_up_getter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<GetterElement>> {
    ctx.req.record(Requirement::OpaqueApiUse {
        target: element.raw(),
        method: "lookUpGetter",
    });
    implementations_of(ctx, element, name, Kind::Getter)
        .into_iter()
        .find(|&g| member::is_accessible_in(ctx, ElemRef::Base(g), library))
        .and_then(|g| g.cast())
}

/// `InstanceElementImpl.lookUpMethod(name:, library:)`.
pub fn look_up_method(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<MethodElement>> {
    ctx.req.record(Requirement::OpaqueApiUse {
        target: element.raw(),
        method: "lookUpMethod",
    });
    implementations_of(ctx, element, name, Kind::Method)
        .into_iter()
        .find(|&m| member::is_accessible_in(ctx, ElemRef::Base(m), library))
        .and_then(|m| m.cast())
}

/// `InstanceElementImpl.lookUpSetter(name:, library:)`.
pub fn look_up_setter(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<SetterElement>> {
    ctx.req.record(Requirement::OpaqueApiUse {
        target: element.raw(),
        method: "lookUpSetter",
    });
    implementations_of(ctx, element, name, Kind::Setter)
        .into_iter()
        .find(|&s| member::is_accessible_in(ctx, ElemRef::Base(s), library))
        .and_then(|s| s.cast())
}

// ------------------------------------------------------------------ InterfaceElementImpl

/// `InterfaceElementImpl.lookUpConcreteMethod(methodName, library)`.
pub fn look_up_concrete_method(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    method_name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<MethodElement>> {
    implementations_of(ctx, element.upcast(), method_name, Kind::Method)
        .into_iter()
        .find(|&m| {
            let m = ElemRef::Base(m);
            !member::is_abstract(ctx, m) && member::is_accessible_in(ctx, m, library)
        })
        .and_then(|m| m.cast())
}

/// The filter of `lookUpInheritedConcrete*`.
fn is_inherited_concrete(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    e: ElementId,
    library: EId<LibraryElement>,
) -> bool {
    let e = ElemRef::Base(e);
    !member::is_abstract(ctx, e)
        && !member::is_static(ctx, e)
        && member::is_accessible_in(ctx, e, library)
        && member::enclosing_element(ctx, e) != Some(element.raw())
}

/// `InterfaceElementImpl.lookUpInheritedConcreteGetter(getterName, library)`.
pub fn look_up_inherited_concrete_getter(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    getter_name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<GetterElement>> {
    implementations_of(ctx, element.upcast(), getter_name, Kind::Getter)
        .into_iter()
        .find(|&g| is_inherited_concrete(ctx, element, g, library))
        .and_then(|g| g.cast())
}

/// `InterfaceElementImpl.lookUpInheritedConcreteMethod(methodName, library)`.
pub fn look_up_inherited_concrete_method(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    method_name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<MethodElement>> {
    implementations_of(ctx, element.upcast(), method_name, Kind::Method)
        .into_iter()
        .find(|&m| is_inherited_concrete(ctx, element, m, library))
        .and_then(|m| m.cast())
}

/// `InterfaceElementImpl.lookUpInheritedConcreteSetter(setterName, library)`.
pub fn look_up_inherited_concrete_setter(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    setter_name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<SetterElement>> {
    implementations_of(ctx, element.upcast(), setter_name, Kind::Setter)
        .into_iter()
        .find(|&s| is_inherited_concrete(ctx, element, s, library))
        .and_then(|s| s.cast())
}

/// `InterfaceElementImpl.lookUpInheritedMethod(methodName:, library:)`:
/// the inherited member [method_name] (from the inheritance manager), if
/// it is a method.
pub fn look_up_inherited_method(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    method_name: &str,
    library: EId<LibraryElement>,
) -> Option<ElemRef> {
    ctx.req.record(Requirement::OpaqueApiUse {
        target: element.raw(),
        method: "lookUpInheritedMethod",
    });
    let name = Name::for_library(ctx, Some(library), method_name);
    InheritanceManager3::new(*ctx)
        .get_inherited(element, name)
        .filter(|&e| member::is_method(ctx, e))
}

/// `InterfaceElementImpl.lookupStaticGetter(name, library)`.
pub fn look_up_static_getter(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<GetterElement>> {
    implementations_of(ctx, element.upcast(), name, Kind::Getter)
        .into_iter()
        .find(|&g| {
            let g = ElemRef::Base(g);
            member::is_static(ctx, g) && member::is_accessible_in(ctx, g, library)
        })
        .and_then(|g| g.cast())
}

/// `InterfaceElementImpl.lookupStaticMethod(name, library)`.
pub fn look_up_static_method(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<MethodElement>> {
    implementations_of(ctx, element.upcast(), name, Kind::Method)
        .into_iter()
        .find(|&m| {
            let m = ElemRef::Base(m);
            member::is_static(ctx, m) && member::is_accessible_in(ctx, m, library)
        })
        .and_then(|m| m.cast())
}

/// `InterfaceElementImpl.lookupStaticSetter(name, library)`.
pub fn look_up_static_setter(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    name: &str,
    library: EId<LibraryElement>,
) -> Option<EId<SetterElement>> {
    implementations_of(ctx, element.upcast(), name, Kind::Setter)
        .into_iter()
        .find(|&s| {
            let s = ElemRef::Base(s);
            member::is_static(ctx, s) && member::is_accessible_in(ctx, s, library)
        })
        .and_then(|s| s.cast())
}

/// `InterfaceElementImpl.getNamedConstructor(name)`.
pub fn get_named_constructor(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    name: &str,
) -> Option<EId<ConstructorElement>> {
    ctx.req
        .record(Requirement::InterfaceElementGetNamedConstructor {
            element: element.raw(),
            name: ctx.name(name),
        });
    ctx.interface(element)
        .constructors
        .iter()
        .copied()
        .find(|&c| ctx.element_name(c.raw()) == Some(name))
}

// ------------------------------------------------------------------ InterfaceTypeImpl

/// The named parameters of `InterfaceTypeImpl.lookUp{Getter,Method,Setter}`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LookUpOptions {
    pub concrete: bool,
    pub inherited: bool,
    pub recovery_static: bool,
}

fn type_element(ctx: &Ctx<'_>, t: TypeId) -> EId<InterfaceElement> {
    ctx.interface_element(t).expect("an interface type")
}

/// `InterfaceTypeImpl.getGetter(getterName)`.
pub fn type_get_getter(ctx: &Ctx<'_>, t: TypeId, getter_name: &str) -> Option<ElemRef> {
    let element = get_getter(ctx, type_element(ctx, t).upcast(), getter_name)?;
    Some(member::substitute(
        ctx,
        ElemRef::Base(element.raw()),
        &MapSubstitution::from_interface_type(ctx, t),
    ))
}

/// `InterfaceTypeImpl.getMethod(methodName)`.
pub fn type_get_method(ctx: &Ctx<'_>, t: TypeId, method_name: &str) -> Option<ElemRef> {
    let element = get_method(ctx, type_element(ctx, t).upcast(), method_name)?;
    Some(member::substitute(
        ctx,
        ElemRef::Base(element.raw()),
        &MapSubstitution::from_interface_type(ctx, t),
    ))
}

/// `InterfaceTypeImpl.getSetter(setterName)`.
pub fn type_get_setter(ctx: &Ctx<'_>, t: TypeId, setter_name: &str) -> Option<ElemRef> {
    let element = get_setter(ctx, type_element(ctx, t).upcast(), setter_name)?;
    Some(member::substitute(
        ctx,
        ElemRef::Base(element.raw()),
        &MapSubstitution::from_interface_type(ctx, t),
    ))
}

/// `InterfaceTypeImpl.lookUpConstructor(constructorName, library)`:
/// `None` for the unnamed constructor name is `new`.
pub fn type_look_up_constructor(
    ctx: &Ctx<'_>,
    t: TypeId,
    constructor_name: Option<&str>,
    library: EId<LibraryElement>,
) -> Option<ElemRef> {
    let element = type_element(ctx, t);
    // prepare base ConstructorElement (`unnamedConstructor` is
    // `getNamedConstructor('new')`)
    let constructor = get_named_constructor(ctx, element, constructor_name.unwrap_or("new"))?;
    // not found or not accessible
    if !member::is_accessible_in(ctx, ElemRef::Base(constructor.raw()), library) {
        return None;
    }
    Some(member::constructor_from2(ctx, constructor.raw(), t))
}

/// The common body of `InterfaceTypeImpl.lookUp{Getter,Method,Setter}`.
fn type_look_up(
    ctx: &Ctx<'_>,
    t: TypeId,
    name_obj: Name,
    options: LookUpOptions,
    is_kind: fn(&Ctx<'_>, ElemRef) -> bool,
) -> Option<ElemRef> {
    let inheritance = InheritanceManager3::new(*ctx);
    let element = type_element(ctx, t);

    if options.inherited {
        if options.concrete {
            let result = inheritance.get_member3(
                t,
                name_obj,
                GetMemberOptions {
                    for_super: true,
                    ..GetMemberOptions::default()
                },
            );
            if let Some(result) = result
                && is_kind(ctx, result)
            {
                return Some(result);
            }
        } else {
            let raw_element = inheritance.get_inherited(element, name_obj);
            if let Some(raw_element) = raw_element
                && is_kind(ctx, raw_element)
            {
                return Some(member::substitute(
                    ctx,
                    raw_element,
                    &MapSubstitution::from_interface_type(ctx, t),
                ));
            }
        }
        return None;
    }

    let result = inheritance.get_member3(
        t,
        name_obj,
        GetMemberOptions {
            concrete: options.concrete,
            ..GetMemberOptions::default()
        },
    );
    if let Some(result) = result
        && is_kind(ctx, result)
    {
        return Some(result);
    }
    None
}

/// `InterfaceTypeImpl.lookUpGetter(name, library, concrete:, inherited:,
/// recoveryStatic:)`.
pub fn type_look_up_getter(
    ctx: &Ctx<'_>,
    t: TypeId,
    name: &str,
    library: EId<LibraryElement>,
    options: LookUpOptions,
) -> Option<ElemRef> {
    let name_obj = Name::new(ctx, Some(library), name);
    if let Some(result) = type_look_up(ctx, t, name_obj, options, member::is_getter) {
        return Some(result);
    }
    if !options.inherited && options.recovery_static {
        return look_up_static_getter(ctx, type_element(ctx, t), name, library)
            .map(|g| ElemRef::Base(g.raw()));
    }
    None
}

/// `InterfaceTypeImpl.lookUpMethod(name, library, concrete:, inherited:,
/// recoveryStatic:)`.
pub fn type_look_up_method(
    ctx: &Ctx<'_>,
    t: TypeId,
    name: &str,
    library: EId<LibraryElement>,
    options: LookUpOptions,
) -> Option<ElemRef> {
    let name_obj = Name::new(ctx, Some(library), name);
    if let Some(result) = type_look_up(ctx, t, name_obj, options, member::is_method) {
        return Some(result);
    }
    if !options.inherited && options.recovery_static {
        return look_up_static_method(ctx, type_element(ctx, t), name, library)
            .map(|m| ElemRef::Base(m.raw()));
    }
    None
}

/// `InterfaceTypeImpl.lookUpSetter(name, library, concrete:, inherited:,
/// recoveryStatic:)`; [name] is without `=`.
pub fn type_look_up_setter(
    ctx: &Ctx<'_>,
    t: TypeId,
    name: &str,
    library: EId<LibraryElement>,
    options: LookUpOptions,
) -> Option<ElemRef> {
    let name_obj = Name::new(ctx, Some(library), &format!("{name}="));
    if let Some(result) = type_look_up(ctx, t, name_obj, options, member::is_setter) {
        return Some(result);
    }
    if !options.inherited && options.recovery_static {
        return look_up_static_setter(ctx, type_element(ctx, t), name, library)
            .map(|s| ElemRef::Base(s.raw()));
    }
    None
}
