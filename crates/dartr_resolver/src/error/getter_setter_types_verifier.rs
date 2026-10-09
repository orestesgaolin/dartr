// Dart source: pkg/analyzer/lib/src/error/getter_setter_types_verifier.dart

//! STUB (wd-errors): the `GetterSetterTypesVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_element::ElementId;

use crate::error_verifier::ErrorVerifier;

/// Dart `GetterSetterTypesVerifier(...).checkExtension(element)`.
pub fn check_extension(ev: &mut ErrorVerifier<'_>, element: ElementId) {
    let _ = (ev, element);
}

/// Dart `GetterSetterTypesVerifier(...).checkExtensionType(element,
/// interface)`.
pub fn check_extension_type(ev: &mut ErrorVerifier<'_>, element: ElementId) {
    let _ = (ev, element);
}

/// Dart `GetterSetterTypesVerifier(...).checkInterface(element,
/// interface)`.
pub fn check_interface(ev: &mut ErrorVerifier<'_>, element: ElementId) {
    let _ = (ev, element);
}

/// Dart `GetterSetterTypesVerifier(...).checkStaticGetters(getters)`.
pub fn check_static_getters(ev: &mut ErrorVerifier<'_>, getters: &[ElementId]) {
    let _ = (ev, getters);
}
