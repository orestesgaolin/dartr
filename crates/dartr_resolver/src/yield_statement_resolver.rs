// Dart source: pkg/analyzer/lib/src/dart/resolver/yield_statement_resolver.dart

//! STUB (unit C7): `YieldStatementResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, YieldStatement};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitYieldStatement(node)`.
pub fn visit_yield_statement(rv: &mut ResolverVisitor<'_>, node: Id<YieldStatement>) {
    // STUB (C7): the statement is not resolved.
    let _ = (rv, node);
}
