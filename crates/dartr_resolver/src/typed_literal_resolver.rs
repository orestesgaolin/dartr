// Dart source: pkg/analyzer/lib/src/dart/resolver/typed_literal_resolver.dart

//! STUB (unit C7): `TypedLiteralResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, IfElement, ListLiteral, MapLiteralEntry, NullAwareElement, SetOrMapLiteral, SpreadElement};
use dartr_element::TypeId;
use crate::resolver::CollectionLiteralContext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitListLiteral(node, contextType: contextType)`.
pub fn visit_list_literal(rv: &mut ResolverVisitor<'_>, node: Id<ListLiteral>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C7): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitSetOrMapLiteral(node, contextType: contextType)`.
pub fn visit_set_or_map_literal(rv: &mut ResolverVisitor<'_>, node: Id<SetOrMapLiteral>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C7): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitIfElement(node, context: context)`.
pub fn visit_if_element(rv: &mut ResolverVisitor<'_>, node: Id<IfElement>, context: Option<CollectionLiteralContext>) {
    // STUB (C7): fallback.
    let _ = context;
    rv.fallback_visit_expressions_below(node.raw());
}

/// Dart `ResolverVisitor.visitMapLiteralEntry(node, context: context)`.
pub fn visit_map_literal_entry(rv: &mut ResolverVisitor<'_>, node: Id<MapLiteralEntry>, context: Option<CollectionLiteralContext>) {
    // STUB (C7): fallback.
    let _ = context;
    rv.fallback_visit_expressions_below(node.raw());
}

/// Dart `ResolverVisitor.visitSpreadElement(node, context: context)`.
pub fn visit_spread_element(rv: &mut ResolverVisitor<'_>, node: Id<SpreadElement>, context: Option<CollectionLiteralContext>) {
    // STUB (C7): fallback.
    let _ = context;
    rv.fallback_visit_expressions_below(node.raw());
}

/// Dart `ResolverVisitor.visitNullAwareElement(node, context: context)`.
pub fn visit_null_aware_element(rv: &mut ResolverVisitor<'_>, node: Id<NullAwareElement>, context: Option<CollectionLiteralContext>) {
    // STUB (C7): fallback.
    let _ = context;
    rv.fallback_visit_expressions_below(node.raw());
}
