// Dart source: pkg/linter/lib/src/util/flutter_utils.dart
//! Flutter helpers of the linter.

use super::util::*;
use crate::LinterContext;
use dartr_element::{EId, InterfaceElement, Nullability, TypeId};
use dartr_typesystem::TypeExt;

const URI_BASIC: &str = "package:flutter/src/widgets/basic.dart";
const URI_CONTAINER: &str = "package:flutter/src/widgets/container.dart";
const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";

/// Dart `InterfaceElementExtension._isExactly(type, uri)`.
pub fn is_exactly(
    c: &LinterContext<'_>,
    element: EId<InterfaceElement>,
    ty: &str,
    uri: &str,
) -> bool {
    name(c, element.raw()) == Some(ty) && library_uri(c, element.raw()) == Some(uri)
}

/// Dart `InterfaceElementExtension.isExactlyWidget`.
pub fn is_exactly_widget(c: &LinterContext<'_>, element: EId<InterfaceElement>) -> bool {
    is_exactly(c, element, "Widget", URI_FRAMEWORK)
}

fn any_supertype(
    c: &LinterContext<'_>,
    element: EId<InterfaceElement>,
    ty: &str,
    uri: &str,
) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    ctx.element_all_supertypes(element).iter().any(|&t| {
        ctx.interface_element(t)
            .is_some_and(|e| is_exactly(c, e, ty, uri))
    })
}

/// Dart `InterfaceElementExtension.isWidget`.
pub fn is_widget(c: &LinterContext<'_>, element: EId<InterfaceElement>) -> bool {
    is_exactly_widget(c, element) || any_supertype(c, element, "Widget", URI_FRAMEWORK)
}

/// Dart `InterfaceElementExtension.isState`.
pub fn is_state(c: &LinterContext<'_>, element: EId<InterfaceElement>) -> bool {
    is_exactly(c, element, "State", URI_FRAMEWORK)
        || any_supertype(c, element, "State", URI_FRAMEWORK)
}

/// Dart `InterfaceElementExtension.extendsWidget`.
pub fn extends_widget(c: &LinterContext<'_>, element: EId<InterfaceElement>) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let mut seen = indexmap::IndexSet::new();
    let mut current = Some(element);
    while let Some(e) = current {
        if is_exactly_widget(c, e) {
            return true;
        }
        if !seen.insert(e) {
            return false;
        }
        current = ctx
            .element_supertype(e)
            .and_then(|t| ctx.interface_element(t));
    }
    false
}

/// Dart `FlutterDartTypeExtension.isWidgetType`.
pub fn is_widget_type(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    ty.and_then(|t| interface_element(c, t))
        .is_some_and(|e| is_widget(c, e))
}

/// Dart `FlutterDartTypeExtension.isWidgetProperty`.
pub fn is_widget_property(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    if is_widget_type(c, ty) {
        return true;
    }
    let (Some(ty), Some(ctx)) = (ty, rctx(c)) else {
        return false;
    };
    let Some(element) = ctx.interface_element(ty) else {
        return false;
    };
    if implements_any_interface(
        c,
        Some(ty),
        &[
            ("List", "dart.core"),
            ("Map", "dart.core"),
            ("LinkedHashMap", "dart.collection"),
            ("Set", "dart.core"),
            ("LinkedHashSet", "dart.collection"),
        ],
    ) {
        return ctx.interface_type_parameters(element).len() == 1
            && is_widget_property(c, ctx.type_arguments(ty).first().copied());
    }
    false
}

/// Dart `FlutterDartTypeExtension.isBuildContext(skipNullable:)`.
pub fn is_build_context(c: &LinterContext<'_>, ty: Option<TypeId>, skip_nullable: bool) -> bool {
    let (Some(ty), Some(ctx)) = (ty, rctx(c)) else {
        return false;
    };
    let Some(element) = ctx.interface_element(ty) else {
        return false;
    };
    if skip_nullable && ctx.nullability_suffix(ty) == Nullability::Question {
        return false;
    }
    is_exactly(c, element, "BuildContext", URI_FRAMEWORK)
}

/// Dart `isWidgetTypeContainer` of an instance creation expression type.
pub fn is_container(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    ty.and_then(|t| interface_element(c, t))
        .is_some_and(|e| is_exactly(c, e, "Container", URI_CONTAINER))
}

/// Dart `isWidgetTypeSizedBox` of an instance creation expression type.
pub fn is_sized_box(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    ty.and_then(|t| interface_element(c, t))
        .is_some_and(|e| is_exactly(c, e, "SizedBox", URI_BASIC))
}
