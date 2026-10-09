// Dart source: pkg/analyzer/lib/src/dart/resolver/prefixed_identifier_resolver.dart

//! STUB (unit C4): `PrefixedIdentifierResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, PrefixedIdentifier};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitPrefixedIdentifier(node, contextType: contextType)`.
pub fn visit_prefixed_identifier(rv: &mut ResolverVisitor<'_>, node: Id<PrefixedIdentifier>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C4): fallback.
    rv.fallback_expression(node.upcast());
}
