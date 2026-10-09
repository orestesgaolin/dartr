// Dart source: pkg/analyzer/lib/src/summary2/enclosing_type_parameters_flag.dart

//! `EnclosingTypeParameterReferenceFlag`: computes precise values for
//! `hasEnclosingTypeParameterReference`.
//!
//! Executable and field elements are created with this flag set
//! conservatively to `true`. During linking, member signatures can still
//! change due to type inference, overridden member inference, and other
//! resolution steps. This pass runs after those signatures are finalized
//! and replaces the conservative value with an exact one. Member
//! substitution (`dartr_typesystem::member::substitute`) returns the base
//! declaration of a getter, setter, method or field whose type does not
//! reference a type parameter of the enclosing element.

use dartr_element::{
    Ctx, EId, ElementFlags, ElementId, FeatureSet, InstanceElement, TypeId, TypeKind, TypeProvider,
};
use dartr_typesystem::TypeExt;

use crate::link::Linker;
use crate::types_builder::link_ctx;

/// Dart `EnclosingTypeParameterReferenceFlag(linker).perform()`.
pub fn perform(lk: &Linker<'_>, tp: &TypeProvider) {
    let features = FeatureSet::default();
    let ctx = link_ctx(lk, tp, &features);
    for builder in &lk.builders {
        let library = ctx.get(builder.element);
        let instances = library
            .classes
            .iter()
            .map(|e| e.raw())
            .chain(library.enums.iter().map(|e| e.raw()))
            .chain(library.mixins.iter().map(|e| e.raw()))
            .chain(library.extensions.iter().map(|e| e.raw()))
            .chain(library.extension_types.iter().map(|e| e.raw()));
        for instance in instances {
            let Some(instance) = instance.cast::<InstanceElement>() else {
                continue;
            };
            perform_instance(&ctx, instance);
        }
        // Top-level accessors and functions have no enclosing type
        // parameters (the accessors of top-level variables are in the
        // library getters and setters).
        for getter in &library.getters {
            set_flag(
                &ctx,
                getter.raw(),
                ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
                false,
            );
        }
        for setter in &library.setters {
            set_flag(
                &ctx,
                setter.raw(),
                ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
                false,
            );
        }
        for function in &library.top_level_functions {
            set_flag(
                &ctx,
                function.raw(),
                ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
                false,
            );
        }
    }
}

/// The fields and executables of one instance element.
fn perform_instance(ctx: &Ctx<'_>, instance: EId<InstanceElement>) {
    let has_type_parameter_reference =
        |t: TypeId| references_type_parameter(ctx, instance.raw(), t);

    let data = ctx.instance(instance);
    for &field in &data.fields {
        let ty = ctx.get(field).type_.get().unwrap_or(TypeId::INVALID);
        let result = has_type_parameter_reference(ty);
        set_flag(
            ctx,
            field.raw(),
            ElementFlags::FIELD_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
            result,
        );
    }

    // Constructors keep the conservative `true`: their member substitution
    // has no shortcut (`ConstructorElementImpl.substitute`), and computing
    // their type here would cache it before their return type is set.
    let executables = data
        .getters
        .iter()
        .map(|e| e.raw())
        .chain(data.setters.iter().map(|e| e.raw()))
        .chain(data.methods.iter().map(|e| e.raw()));
    for executable in executables {
        let Some(e) = executable.cast::<dartr_element::ExecutableElement>() else {
            continue;
        };
        let result = executable_type_references(ctx, instance.raw(), e);
        set_flag(
            ctx,
            executable,
            ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
            result,
        );
    }
}

/// Whether `executable.type` references a type parameter of [instance]:
/// the parts of the function type (return type, type parameter bounds,
/// parameter types), read without computing (and caching) the type.
fn executable_type_references(
    ctx: &Ctx<'_>,
    instance: ElementId,
    executable: EId<dartr_element::ExecutableElement>,
) -> bool {
    let data = ctx.executable(executable);
    let ret = data.return_type.get().unwrap_or(TypeId::INVALID);
    if references_type_parameter(ctx, instance, ret) {
        return true;
    }
    for &tp in &data.type_params {
        if let Some(bound) = ctx.get(tp).bound.get()
            && references_type_parameter(ctx, instance, bound)
        {
            return true;
        }
    }
    data.formal_params.iter().any(|&p| {
        let ty = ctx.get(p).type_.get().unwrap_or(TypeId::INVALID);
        references_type_parameter(ctx, instance, ty)
    })
}

fn set_flag(ctx: &Ctx<'_>, element: ElementId, flag: ElementFlags, value: bool) {
    if let Some(data) = ctx.element_data(element) {
        data.flags.set(flag, value);
    }
}

/// Dart `_ReferencesTypeParameterVisitor(instanceElement)` over [t]
/// (`RecursiveTypeVisitor(includeTypeAliasArguments: true)`): whether [t]
/// mentions a type parameter whose enclosing element is [instance].
fn references_type_parameter(ctx: &Ctx<'_>, instance: ElementId, t: TypeId) -> bool {
    if let Some(alias) = ctx.type_alias(t) {
        let args = ctx.alias(alias).args;
        if ctx
            .list(args)
            .iter()
            .any(|&a| references_type_parameter(ctx, instance, a))
        {
            return true;
        }
    }
    match *ctx.ty(t) {
        TypeKind::Interface { args, .. } => ctx
            .list(args)
            .iter()
            .any(|&a| references_type_parameter(ctx, instance, a)),
        TypeKind::Function(f) => {
            if references_type_parameter(ctx, instance, f.ret) {
                return true;
            }
            for &tp in ctx.list(f.type_params) {
                if let Some(bound) = ctx.get(tp).bound.get()
                    && references_type_parameter(ctx, instance, bound)
                {
                    return true;
                }
            }
            ctx.list(f.params)
                .iter()
                .any(|p| references_type_parameter(ctx, instance, p.ty))
        }
        TypeKind::Record {
            positional, named, ..
        } => {
            ctx.list(positional)
                .iter()
                .any(|&p| references_type_parameter(ctx, instance, p))
                || ctx
                    .list(named)
                    .iter()
                    .any(|n| references_type_parameter(ctx, instance, n.ty))
        }
        TypeKind::TypeParameter { param, .. } => {
            ctx.element_data(param.raw()).and_then(|d| d.enclosing) == Some(instance)
        }
        _ => false,
    }
}
