// Dart source: pkg/analyzer/lib/src/dart/resolver/record_literal_resolver.dart

//! STUB (unit C7): `RecordLiteralResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, RecordLiteral};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitRecordLiteral(node, contextType: contextType)`.
pub fn visit_record_literal(rv: &mut ResolverVisitor<'_>, node: Id<RecordLiteral>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C7): fallback.
    rv.fallback_expression(node.upcast());
}
