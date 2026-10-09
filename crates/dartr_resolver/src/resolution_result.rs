// Dart source: pkg/analyzer/lib/src/dart/resolver/resolution_result.dart,
// pkg/analyzer/lib/src/dart/element/extensions.dart (RecordTypeExtension
// .fieldByName / namedField / positionalField / positionalFieldIndex)

//! [`ResolutionResult`]: the result of resolving a property (getter /
//! setter / method) of a type.

use dartr_element::{Ctx, ElemRef, Name, TypeId, TypeKind};

/// A field of a record type (Dart `RecordTypeFieldImpl`): a positional
/// field (by index) or a named field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordField {
    /// The name of a named field; `None` for a positional field.
    pub name: Option<Name>,
    /// The index of a positional field.
    pub index: usize,
    pub ty: TypeId,
}

/// Dart `ResolutionResult` (with `SimpleResolutionResult`).
#[derive(Clone, Copy, Debug)]
pub struct ResolutionResult {
    /// Dart `getter2`: the element that is invoked for reading.
    pub getter: Option<ElemRef>,
    /// Dart `setter2`: the element that is invoked for writing.
    pub setter: Option<ElemRef>,
    /// Dart `needsGetterError` (default `true`).
    pub needs_getter_error: bool,
    /// Dart `isGetterInvalid`.
    pub is_getter_invalid: bool,
    /// Dart `needsSetterError` (default `true`).
    pub needs_setter_error: bool,
    /// Dart `callFunctionType`.
    pub call_function_type: Option<TypeId>,
    /// Dart `recordField`.
    pub record_field: Option<RecordField>,
}

impl Default for ResolutionResult {
    fn default() -> Self {
        ResolutionResult {
            getter: None,
            setter: None,
            needs_getter_error: true,
            is_getter_invalid: false,
            needs_setter_error: true,
            call_function_type: None,
            record_field: None,
        }
    }
}

/// Dart `RecordTypeExtension.positionalFieldIndex`: parses `$1`, `$2`, ...
pub fn positional_field_index(name: &str) -> Option<usize> {
    let digits = name.strip_prefix('$')?;
    let mut chars = digits.chars();
    let first = chars.next()?;
    if !('1'..='9').contains(&first) || !chars.all(|c| c.is_ascii_digit()) {
        return None;
    }
    // `int.tryParse` rejects numerals too big to fit in an `int`.
    let position: i64 = digits.parse().ok()?;
    Some((position - 1) as usize)
}

/// Dart `RecordTypeExtension.fieldByName(name)` of the record type [ty].
pub fn record_field_by_name(ctx: &Ctx<'_>, ty: TypeId, name: &str) -> Option<RecordField> {
    let TypeKind::Record {
        positional, named, ..
    } = *ctx.ty(ty)
    else {
        return None;
    };
    for field in ctx.list(named) {
        if ctx.name_str(field.name) == name {
            return Some(RecordField {
                name: Some(field.name),
                index: 0,
                ty: field.ty,
            });
        }
    }
    let index = positional_field_index(name)?;
    let positional = ctx.list(positional);
    positional.get(index).map(|&ty| RecordField {
        name: None,
        index,
        ty,
    })
}
