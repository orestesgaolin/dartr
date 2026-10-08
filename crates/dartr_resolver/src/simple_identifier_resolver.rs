// Dart source: pkg/analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart

//! `SimpleIdentifierResolver`: the element and the static type of a
//! `SimpleIdentifier` expression (local variables with flow analysis
//! promotion, formal parameters, top-level and instance members, types,
//! prefixes).
//!
//! STUB (unit C2, identifiers): [`visit_simple_identifier`] uses the
//! fallback.

use dartr_ast::{Id, SimpleIdentifier};
use dartr_element::TypeId;

use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitSimpleIdentifier(node, contextType:)` +
/// `SimpleIdentifierResolver.resolve(node, contextType:)`.
pub fn visit_simple_identifier(rv: &mut ResolverVisitor<'_>, node: Id<SimpleIdentifier>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C2 identifiers): fallback.
    rv.fallback_expression(node.upcast());
}
