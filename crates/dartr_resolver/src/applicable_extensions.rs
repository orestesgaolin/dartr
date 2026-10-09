// Dart source: pkg/analyzer/lib/src/dart/resolver/applicable_extensions.dart

//! Selection and instantiation of extension declarations that apply to a
//! receiver type.

use dartr_element::{EId, ElemRef, ExtensionElement, Nullability, TypeId, TypeKind};
use dartr_typesystem::generic_inferrer::{GenericInferrer, InferenceFlags};
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::{MapSubstitution, get_fresh_type_parameters};
use dartr_typesystem::type_system_operations::TypeSystemOperations;

use crate::resolver::ResolverVisitor;

/// Dart `_NotInstantiatedExtensionWithMember`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NotInstantiatedExtensionWithMember {
    pub extension: EId<ExtensionElement>,
    pub getter: Option<ElemRef>,
    pub setter: Option<ElemRef>,
}

/// Dart `InstantiatedExtensionWithMember`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InstantiatedExtensionWithMember {
    pub extension: EId<ExtensionElement>,
    pub getter: Option<ElemRef>,
    pub setter: Option<ElemRef>,
    pub extended_type: TypeId,
}

/// Dart `_ExtensionWithMemberWithName`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ExtensionWithMemberWithName {
    pub extension: EId<ExtensionElement>,
    pub member: ElemRef,
}

impl NotInstantiatedExtensionWithMember {
    /// Dart `_NotInstantiatedExtensionWithMember.instantiate`.
    fn instantiate(
        self,
        rv: &ResolverVisitor<'_>,
        substitution: &MapSubstitution,
        extended_type: TypeId,
    ) -> InstantiatedExtensionWithMember {
        InstantiatedExtensionWithMember {
            extension: self.extension,
            getter: self
                .getter
                .map(|e| member::substitute(&rv.ctx, e, substitution)),
            setter: self
                .setter
                .map(|e| member::substitute(&rv.ctx, e, substitution)),
            extended_type,
        }
    }
}

/// Dart `ExtensionsExtensions.havingMemberWithBaseName` for one extension.
pub(crate) fn having_member_with_base_name(
    rv: &ResolverVisitor<'_>,
    extension: EId<ExtensionElement>,
    name: &dartr_typesystem::inheritance_manager3::Name,
) -> Option<NotInstantiatedExtensionWithMember> {
    let ctx = rv.ctx;
    let extension_data = ctx.get(extension);
    let extension_library = extension_data.library?;
    if !name.is_accessible_for(&ctx, extension_library) {
        return None;
    }

    let instance = extension.upcast();
    let base_name = name.text(&ctx);
    let (getter, setter) = if base_name == "[]" {
        (
            dartr_typesystem::lookup::get_method(&ctx, instance, "[]")
                .map(|e| ElemRef::Base(e.raw())),
            dartr_typesystem::lookup::get_method(&ctx, instance, "[]=")
                .map(|e| ElemRef::Base(e.raw())),
        )
    } else {
        let getter = dartr_typesystem::lookup::get_getter(&ctx, instance, base_name)
            .map(|e| ElemRef::Base(e.raw()))
            .filter(|&e| !member::is_static(&ctx, e))
            .or_else(|| {
                dartr_typesystem::lookup::get_method(&ctx, instance, base_name)
                    .map(|e| ElemRef::Base(e.raw()))
                    .filter(|&e| !member::is_static(&ctx, e))
            });
        let setter = dartr_typesystem::lookup::get_setter(&ctx, instance, base_name)
            .map(|e| ElemRef::Base(e.raw()))
            .filter(|&e| !member::is_static(&ctx, e));
        (getter, setter)
    };

    (getter.is_some() || setter.is_some()).then_some(NotInstantiatedExtensionWithMember {
        extension,
        getter,
        setter,
    })
}

/// Dart `ExtensionsExtensions.havingStaticMemberWithName` for one extension.
pub(crate) fn having_static_member_with_name(
    rv: &ResolverVisitor<'_>,
    extension: EId<ExtensionElement>,
    name: &dartr_typesystem::inheritance_manager3::Name,
) -> Vec<ExtensionWithMemberWithName> {
    let ctx = rv.ctx;
    let extension_data = ctx.get(extension);
    let Some(extension_library) = extension_data.library else {
        return Vec::new();
    };
    if !name.is_accessible_for(&ctx, extension_library) {
        return Vec::new();
    }

    let instance = extension.upcast();
    let name = name.text(&ctx);
    let mut result = Vec::new();
    let getter = dartr_typesystem::lookup::get_getter(&ctx, instance, name)
        .map(|e| ElemRef::Base(e.raw()))
        .filter(|&e| member::is_static(&ctx, e));
    let setter = dartr_typesystem::lookup::get_setter(&ctx, instance, name)
        .map(|e| ElemRef::Base(e.raw()))
        .filter(|&e| member::is_static(&ctx, e));
    let method = dartr_typesystem::lookup::get_method(&ctx, instance, name)
        .map(|e| ElemRef::Base(e.raw()))
        .filter(|&e| member::is_static(&ctx, e));
    for member in [getter, setter, method].into_iter().flatten() {
        result.push(ExtensionWithMemberWithName { extension, member });
    }
    result
}

/// Dart `NotInstantiatedExtensionsExtensions.applicableTo`.
pub(crate) fn applicable_to(
    rv: &ResolverVisitor<'_>,
    candidates: impl IntoIterator<Item = NotInstantiatedExtensionWithMember>,
    target_type: TypeId,
) -> Vec<InstantiatedExtensionWithMember> {
    if matches!(*rv.ctx.ty(target_type), TypeKind::Never(Nullability::None)) {
        return Vec::new();
    }

    let mut instantiated = Vec::new();
    for candidate in candidates {
        let extension = rv.ctx.get(candidate.extension);
        let Some(extension_type) = extension.extended_type.get() else {
            continue;
        };
        let fresh = get_fresh_type_parameters(&rv.ctx, &extension.type_params);
        let raw_extended_type = fresh.substitution.substitute_type(&rv.ctx, extension_type);
        let flags = InferenceFlags {
            generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
            inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
            strict_inference: false,
        };
        let operations = TypeSystemOperations::new(rv.type_system, false);
        let inferred_types = {
            let mut inferrer = GenericInferrer::new(
                rv.type_system,
                &fresh.fresh_type_parameters,
                None,
                None,
                flags,
                operations,
                None,
            );
            inferrer.constrain_argument(target_type, raw_extended_type, "extendedType", None);
            inferrer.try_choose_final_types(true)
        };
        let Some(inferred_types) = inferred_types else {
            continue;
        };

        let substitution = MapSubstitution::from_pairs(&extension.type_params, &inferred_types);
        let extended_type = substitution.substitute_type(&rv.ctx, extension_type);
        if !rv.type_system.is_subtype_of(target_type, extended_type) {
            continue;
        }
        instantiated.push(candidate.instantiate(rv, &substitution, extended_type));
    }
    instantiated
}
