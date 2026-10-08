// Dart source: pkg/analyzer/lib/src/dart/resolver/property_element_resolver.dart

//! STUB (unit C4): `PropertyElementResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, IndexExpression, PropertyAccess};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitPropertyAccess(node, contextType: contextType)`.
pub fn visit_property_access(rv: &mut ResolverVisitor<'_>, node: Id<PropertyAccess>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C4): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitIndexExpression(node, contextType: contextType)`.
pub fn visit_index_expression(rv: &mut ResolverVisitor<'_>, node: Id<IndexExpression>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C4): fallback.
    rv.fallback_expression(node.upcast());
}
