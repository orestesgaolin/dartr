// Dart source: pkg/analyzer/lib/src/dart/element/element.dart
// (PropertyInducingElementImpl.type, PropertyInducingElementImpl.typeInference,
// ExecutableElementImpl.returnType)

//! On-demand type inference of fields and top-level variables while a
//! cycle is linked (Dart `PropertyInducingElementImpl.typeInference`).
//!
//! In Dart, the getter `PropertyInducingElementImpl.type` calls
//! `typeInference.perform()` when the type is not set yet: the linker
//! infers the type of a variable from its initializer when the first
//! reader (override inference, the initializer of another variable, a
//! field formal parameter) asks for it. The getter
//! `ExecutableElementImpl.returnType` of a getter or setter of such a
//! variable reads `variable.type` first.
//!
//! The element model of this port keeps no closures in elements. The
//! linker installs a [`PropertyTypeInference`] for the thread that links
//! the cycle ([`with_property_type_inference`]); the readers of variable
//! types and accessor return types call [`ensure_property_type`] /
//! [`ensure_accessor_return_type`] when the slot is not set. Outside of
//! linking no hook is installed and the calls do nothing.

use std::cell::Cell;

use crate::{
    Ctx, EId, ElementId, ExecutableElement, FragmentFlags, PropertyAccessorElement,
    PropertyInducingElement, Tag,
};

/// Dart `PropertyInducingElementTypeInference`: infers the type of a
/// variable of the cycle that is linked and sets it (Dart `type = ...`).
pub trait PropertyTypeInference {
    /// Infers and sets the type of [element] when it has none yet. Does
    /// nothing for an element that is not in the cycle that is linked.
    fn infer(&self, element: EId<PropertyInducingElement>);
}

type HookPtr = *const (dyn PropertyTypeInference + 'static);

thread_local! {
    static HOOK: Cell<Option<HookPtr>> = const { Cell::new(None) };
}

/// Restores the previous hook when the scope ends (also on a panic).
struct Restore(Option<HookPtr>);

impl Drop for Restore {
    fn drop(&mut self) {
        HOOK.with(|h| h.set(self.0));
    }
}

/// Runs [f] with [hook] installed for the current thread.
pub fn with_property_type_inference<R>(
    hook: &dyn PropertyTypeInference,
    f: impl FnOnce() -> R,
) -> R {
    // SAFETY: the pointer is only dereferenced while `f` runs (the hook is
    // removed by `Restore` before this function returns or unwinds), and
    // `hook` outlives this call.
    let ptr: HookPtr = unsafe {
        std::mem::transmute::<*const (dyn PropertyTypeInference + '_), HookPtr>(hook as *const _)
    };
    let previous = HOOK.with(|h| h.replace(Some(ptr)));
    let _restore = Restore(previous);
    f()
}

fn call_hook(element: EId<PropertyInducingElement>) {
    let Some(ptr) = HOOK.with(|h| h.get()) else {
        return;
    };
    // SAFETY: see `with_property_type_inference`.
    let hook = unsafe { &*ptr };
    hook.infer(element);
}

/// Dart `PropertyInducingElementImpl.type` (the inference part): when
/// [element] has no type yet and a linker hook is installed, infers it.
pub fn ensure_property_type(ctx: &Ctx<'_>, element: EId<PropertyInducingElement>) {
    if ctx.property_inducing(element).type_.get().is_some() {
        return;
    }
    call_hook(element);
}

/// [ensure_property_type] for a variable element of any kind (fields and
/// top-level variables; other variables are ignored).
pub fn ensure_variable_type(ctx: &Ctx<'_>, element: ElementId) {
    if matches!(element.tag(), Tag::Field | Tag::TopLevelVariable) {
        ensure_property_type(ctx, EId::from_raw(element));
    }
}

/// Dart `ExecutableElementImpl.returnType` (the inference part): the
/// return type of a getter or setter that a variable induces is set when
/// the type of the variable is; infers the variable first.
pub fn ensure_accessor_return_type(ctx: &Ctx<'_>, element: EId<ExecutableElement>) {
    if !matches!(element.raw().tag(), Tag::Getter | Tag::Setter) {
        return;
    }
    if ctx.executable(element).return_type.get().is_some() {
        return;
    }
    if HOOK.with(|h| h.get()).is_none() {
        return;
    }
    let accessor: EId<PropertyAccessorElement> = EId::from_raw(element.raw());
    let data = ctx.property_accessor(accessor);
    let is_origin_variable = ctx.fragment_data(data.first_fragment).is_some_and(|f| {
        f.flags
            .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
    });
    if !is_origin_variable {
        return;
    }
    if let Some(variable) = data.variable.get() {
        ensure_property_type(ctx, variable);
    }
}

/// The type of the value parameter of a setter that a variable induces is
/// set with the type of the variable: infers the variable first.
pub fn ensure_formal_parameter_type(ctx: &Ctx<'_>, parameter: EId<crate::FormalParameterElement>) {
    let data = ctx.get(parameter);
    if data.type_.get().is_some() || HOOK.with(|h| h.get()).is_none() {
        return;
    }
    if let Some(enclosing) = data.enclosing
        && let Some(setter) = enclosing.cast::<ExecutableElement>()
    {
        ensure_accessor_return_type(ctx, setter);
    }
}
