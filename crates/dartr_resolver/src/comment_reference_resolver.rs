// Dart source: pkg/analyzer/lib/src/dart/resolver/comment_reference_resolver.dart

//! STUB (unit C9): `CommentReferenceResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, CommentReference};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitCommentReference(node)`.
pub fn visit_comment_reference(rv: &mut ResolverVisitor<'_>, node: Id<CommentReference>) {
    // STUB (C9): not resolved.
    let _ = (rv, node);
}
