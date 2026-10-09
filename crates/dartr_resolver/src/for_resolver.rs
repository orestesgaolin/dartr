// Dart source: pkg/analyzer/lib/src/dart/resolver/for_resolver.dart

//! STUB (unit C7): `ForResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, ForElement, ForStatement};
use crate::resolver::CollectionLiteralContext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitForStatement(node)`.
pub fn visit_for_statement(rv: &mut ResolverVisitor<'_>, node: Id<ForStatement>) {
    // STUB (C7): the statement is not resolved.
    let _ = (rv, node);
}

/// Dart `ResolverVisitor.visitForElement(node, context: context)`.
pub fn visit_for_element(rv: &mut ResolverVisitor<'_>, node: Id<ForElement>, context: Option<CollectionLiteralContext>) {
    // STUB (C7): fallback.
    let _ = context;
    rv.fallback_visit_expressions_below(node.raw());
}
