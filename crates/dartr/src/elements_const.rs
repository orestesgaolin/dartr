// Dart source: tools/oracle/bin/elements.dart (`--with-const`)

//! The `"const"` key of `dartr dump elements --with-const`: the constant
//! value of a const top-level variable or field (enum constants included),
//! as Dart `element.computeConstantValue()` written with
//! `DartObjectImpl.toString()`, or `null` for an invalid constant
//! (docs/design/semantics.md §5.1).

use dartr_element::{Ctx, ElementId};

/// The JSON text of the `"const"` value of [element] (a const top-level
/// variable or field in the world of [ctx]): a JSON string, or `null` when
/// the constant is invalid. `None`: the key is not written.
///
/// The dump writer calls this only for elements with `isConst`, and only
/// with `--with-const`.
// TODO(D1): evaluate the constant with the constant evaluator: analyze the
// library of [element], look up the value of [element] and return
// `DartObjectImpl.toString()` as a JSON string (`null` for an invalid
// constant). Until then the key is not written.
pub(crate) fn const_value_json(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let _ = (ctx, element);
    None
}
