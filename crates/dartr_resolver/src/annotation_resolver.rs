// Dart source: pkg/analyzer/lib/src/dart/resolver/annotation_resolver.dart

//! STUB (unit C9): `AnnotationResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, Annotation};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitAnnotation(node)`.
pub fn visit_annotation(rv: &mut ResolverVisitor<'_>, node: Id<Annotation>) {
    // STUB (C9): not resolved.
    let _ = (rv, node);
}
