// Dart source: pkg/analyzer/lib/src/error/getter_setter_types_verifier.dart

//! `GetterSetterTypesVerifier`: the return type of a getter must be a
//! subtype of the parameter type of the setter with the same name
//! (`GETTER_NOT_SUBTYPE_SETTER_TYPES`, only before the language version
//! with the `getter-setter-error` feature).
//!
//! The error verifier calls [`check_static_getters`] (classes, enums,
//! mixins, the unit), [`check_extension`] and [`check_extension_type`];
//! the inheritance override verifier calls [`check_interface`].

use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, ElementId, ExtensionElement, ExtensionTypeElement, GetterElement,
    InterfaceElement, Tag, TypeId,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::inheritance_manager3::Interface;
use dartr_typesystem::member;
use dartr_typesystem::type_ext::TypeExt;

use super::VerifierHost;
use super::correct_override::{diagnostic_range, display_name, library_feature_enabled};

/// `_skipGetterSetterTypesCheck`.
fn skip_getter_setter_types_check<'h, H: VerifierHost<'h>>(host: &H) -> bool {
    library_feature_enabled(
        &host.ctx(),
        host.library(),
        ExperimentalFlag::GetterSetterError,
    )
}

/// Dart `GetterSetterTypesVerifier.checkExtension(element)`.
pub fn check_extension<'h, H: VerifierHost<'h>>(host: &mut H, element: EId<ExtensionElement>) {
    if skip_getter_setter_types_check(host) {
        return;
    }
    let getters = host.ctx().get(element).getters.clone();
    for getter in getters {
        check_local_getter(host, getter);
    }
}

/// Dart `GetterSetterTypesVerifier.checkExtensionType(element, interface)`.
pub fn check_extension_type<'h, H: VerifierHost<'h>>(
    host: &mut H,
    element: EId<ExtensionTypeElement>,
    interface: &Interface,
) {
    if skip_getter_setter_types_check(host) {
        return;
    }
    check_interface(host, element.upcast(), interface);
    let getters = host.ctx().get(element).getters.clone();
    check_static_getters(host, &getters);
}

/// Dart `GetterSetterTypesVerifier.checkInterface(element, interface)`.
pub fn check_interface<'h, H: VerifierHost<'h>>(
    host: &mut H,
    element: EId<InterfaceElement>,
    interface: &Interface,
) {
    if skip_getter_setter_types_check(host) {
        return;
    }
    let ctx = host.ctx();
    let type_system = host.type_system();
    let Some(library) = ctx.element_data(element.raw()).and_then(|d| d.library) else {
        return;
    };
    let element_raw = element.raw();

    let interface_map = &interface.map;
    for (getter_name, &getter) in interface_map {
        if !getter_name.is_accessible_for(&ctx, library) {
            continue;
        }
        if member::base_element(&ctx, getter).tag() != Tag::Getter {
            continue;
        }
        let Some(&setter) = interface_map.get(&getter_name.for_setter(&ctx)) else {
            continue;
        };
        let setter_parameters = member::formal_parameters(&ctx, setter);
        if setter_parameters.len() != 1 {
            continue;
        }
        let getter_type = member::return_type(&ctx, getter);
        let setter_type = member::type_(&ctx, setter_parameters[0]);
        if type_system.is_subtype_of(getter_type, setter_type) {
            continue;
        }
        let getter_enclosing = member::enclosing_element(&ctx, getter);
        let setter_enclosing = member::enclosing_element(&ctx, setter);
        let error_element: ElementId = if getter_enclosing == Some(element_raw) {
            let is_representation_getter = element_raw
                .cast::<ExtensionTypeElement>()
                .and_then(|et| ctx.get(et).fields.first().copied())
                .and_then(|field| ctx.get(field).getter)
                .is_some_and(|g| ElemRef::Base(g.raw()) == getter);
            if is_representation_getter {
                member::base_element(&ctx, setter)
            } else {
                member::base_element(&ctx, getter)
            }
        } else if setter_enclosing == Some(element_raw) {
            member::base_element(&ctx, setter)
        } else {
            element_raw
        };

        let mut getter_name = display_name(&ctx, member::base_element(&ctx, getter));
        if getter_enclosing != Some(element_raw) {
            let class_name = getter_enclosing.map_or_else(String::new, |e| display_name(&ctx, e));
            getter_name = format!("{class_name}.{getter_name}");
        }
        let mut setter_name = display_name(&ctx, member::base_element(&ctx, setter));
        if setter_enclosing != Some(element_raw) {
            let class_name = setter_enclosing.map_or_else(String::new, |e| display_name(&ctx, e));
            setter_name = format!("{class_name}.{setter_name}");
        }

        let (offset, length) = diagnostic_range(&ctx, error_element);
        host.report(
            diag::getter_not_subtype_setter_types(
                &getter_name,
                type_arg(&ctx, getter_type),
                type_arg(&ctx, setter_type),
                &setter_name,
            )
            .at_offset(offset, length),
        );
    }
}

/// Dart `GetterSetterTypesVerifier.checkStaticGetters(getters)`.
pub fn check_static_getters<'h, H: VerifierHost<'h>>(host: &mut H, getters: &[EId<GetterElement>]) {
    if skip_getter_setter_types_check(host) {
        return;
    }
    let ctx = host.ctx();
    for &getter in getters {
        if member::is_static(&ctx, ElemRef::Base(getter.raw())) {
            check_local_getter(host, getter);
        }
    }
}

/// `_checkLocalGetter(getter)`.
fn check_local_getter<'h, H: VerifierHost<'h>>(host: &mut H, getter: EId<GetterElement>) {
    let ctx = host.ctx();
    let Some(name) = ctx.element_name(getter.raw()) else {
        return;
    };
    let Some(variable) = ctx.get(getter).variable.get() else {
        return;
    };
    let Some(setter) = ctx.property_inducing(variable).setter else {
        return;
    };
    let Some(setter_type) = get_setter_type(host, setter.raw()) else {
        return;
    };
    let getter_type = member::return_type(&ctx, ElemRef::Base(getter.raw()));
    if host.type_system().is_subtype_of(getter_type, setter_type) {
        return;
    }
    let (offset, length) = diagnostic_range(&ctx, getter.raw());
    host.report(
        diag::getter_not_subtype_setter_types(
            name,
            type_arg(&ctx, getter_type),
            type_arg(&ctx, setter_type),
            name,
        )
        .at_offset(offset, length),
    );
}

/// `_getSetterType(setter)`: the type of the first parameter of [setter].
fn get_setter_type<'h, H: VerifierHost<'h>>(host: &H, setter: ElementId) -> Option<TypeId> {
    let ctx = host.ctx();
    let parameters = member::formal_parameters(&ctx, ElemRef::Base(setter));
    let &first = parameters.first()?;
    Some(member::type_(&ctx, first))
}
