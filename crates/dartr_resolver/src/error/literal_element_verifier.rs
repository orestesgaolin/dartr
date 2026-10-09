// Dart source: pkg/analyzer/lib/src/error/literal_element_verifier.dart

//! STUB (wd-errors): the `LiteralElementVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::{CollectionElement, Id};
use dartr_element::TypeId;

use crate::error_verifier::ErrorVerifier;

/// The configuration of a Dart `LiteralElementVerifier` (`forList`,
/// `forSet`, `elementType`, `forMap`, `mapKeyType`, `mapValueType`).
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteralElementVerifier {
    pub for_list: bool,
    pub for_set: bool,
    pub element_type: Option<TypeId>,
    pub for_map: bool,
    pub map_key_type: Option<TypeId>,
    pub map_value_type: Option<TypeId>,
}

impl LiteralElementVerifier {
    /// Dart `verify(element)`.
    pub fn verify(&self, ev: &mut ErrorVerifier<'_>, element: Id<CollectionElement>) {
        let _ = (ev, element);
    }
}
