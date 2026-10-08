// Dart source: pkg/analyzer/lib/src/dart/resolver/this_lookup.dart

//! `ThisLookup`: resolves an identifier as an implicit property get or set
//! on the type of `this`.

use dartr_ast::{Id, SimpleIdentifier};

use crate::lexical_lookup::LexicalLookupResult;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ThisLookup.lookupGetter`.
pub fn lookup_getter(rv: &mut ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> Option<LexicalLookupResult> {
    let id = rv.lexeme(rv.ast[node].token).to_string();
    let this_type = rv.effective_this_type()?;
    let property_result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: None,
            receiver_type: this_type,
            name: &id,
            has_read: true,
            has_write: false,
            property_error_entity: node.raw(),
            name_error_entity: node.raw(),
            parent_node: None,
        },
    );
    if let Some(call_function_type) = property_result.call_function_type {
        return Some(LexicalLookupResult {
            call_function_type: Some(call_function_type),
            ..Default::default()
        });
    }
    if let Some(record_field) = property_result.record_field {
        return Some(LexicalLookupResult {
            record_field: Some(record_field),
            ..Default::default()
        });
    }
    match property_result.getter {
        Some(getter) => Some(LexicalLookupResult {
            requested: Some(getter),
            ..Default::default()
        }),
        None => Some(LexicalLookupResult {
            recovery: property_result.setter,
            ..Default::default()
        }),
    }
}

/// Dart `ThisLookup.lookupSetter`.
pub fn lookup_setter(rv: &mut ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> Option<LexicalLookupResult> {
    let id = rv.lexeme(rv.ast[node].token).to_string();
    let this_type = rv.effective_this_type()?;
    let property_result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: None,
            receiver_type: this_type,
            name: &id,
            has_read: false,
            has_write: true,
            property_error_entity: node.raw(),
            name_error_entity: node.raw(),
            parent_node: None,
        },
    );
    match property_result.setter {
        Some(setter) => Some(LexicalLookupResult {
            requested: Some(setter),
            ..Default::default()
        }),
        None => Some(LexicalLookupResult {
            recovery: property_result.getter,
            ..Default::default()
        }),
    }
}
