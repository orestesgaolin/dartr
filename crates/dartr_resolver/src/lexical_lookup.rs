// Dart source: pkg/analyzer/lib/src/dart/resolver/lexical_lookup.dart

//! `LexicalLookup`: interprets the scope lookup result of an identifier for
//! a getter or a setter.

use dartr_element::{Ctx, ElemRef, ElementId, InstanceElement, VariableElement};

use crate::resolution_result::RecordField;
use crate::scope::ScopeLookupResult;

/// Dart `LexicalLookupResult`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LexicalLookupResult {
    pub requested: Option<ElemRef>,
    pub recovery: Option<ElemRef>,
    /// The type, usually a `FunctionType` referenced with `call`.
    pub call_function_type: Option<dartr_element::TypeId>,
    /// The field referenced in a record type.
    pub record_field: Option<RecordField>,
}

/// Dart `Element.isInstanceMember`: an element declared in an instance
/// element (class, mixin, enum, extension, extension type) that is not
/// static.
pub fn is_instance_member(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let Some(data) = ctx.element_data(e) else {
        return false;
    };
    let Some(enclosing) = data.enclosing else {
        return false;
    };
    enclosing.is::<InstanceElement>() && !dartr_typesystem::member::is_static(ctx, ElemRef::Base(e))
}

/// Dart `LexicalLookup.resolveGetter`.
pub fn resolve_getter(ctx: &Ctx<'_>, scope_result: ScopeLookupResult) -> Option<LexicalLookupResult> {
    let scope_getter = scope_result.getter;
    let scope_setter = scope_result.setter;
    if scope_getter.is_some() || scope_setter.is_some() {
        if let Some(getter) = scope_getter {
            return Some(LexicalLookupResult {
                requested: Some(ElemRef::Base(getter)),
                ..Default::default()
            });
        }
        if let Some(setter) = scope_setter
            && !is_instance_member(ctx, setter)
        {
            return Some(LexicalLookupResult {
                recovery: Some(ElemRef::Base(setter)),
                ..Default::default()
            });
        }
    }
    None
}

/// Dart `LexicalLookup.resolveSetter`.
pub fn resolve_setter(ctx: &Ctx<'_>, scope_result: ScopeLookupResult) -> Option<LexicalLookupResult> {
    let scope_getter = scope_result.getter;
    let scope_setter = scope_result.setter;
    if scope_getter.is_some() || scope_setter.is_some() {
        if let Some(getter) = scope_getter
            && getter.is::<VariableElement>()
        {
            return Some(LexicalLookupResult {
                requested: Some(ElemRef::Base(getter)),
                ..Default::default()
            });
        }
        if let Some(setter) = scope_setter {
            return Some(LexicalLookupResult {
                requested: Some(ElemRef::Base(setter)),
                ..Default::default()
            });
        }
        if let Some(getter) = scope_getter
            && !is_instance_member(ctx, getter)
        {
            return Some(LexicalLookupResult {
                recovery: Some(ElemRef::Base(getter)),
                ..Default::default()
            });
        }
    }
    None
}
