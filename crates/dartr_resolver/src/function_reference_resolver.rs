// Dart source: pkg/analyzer/lib/src/dart/resolver/function_reference_resolver.dart

//! STUB (unit C8): `FunctionReferenceResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, FunctionReference, ImplicitCallReference};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitFunctionReference(node, contextType: contextType)`.
pub fn visit_function_reference(rv: &mut ResolverVisitor<'_>, node: Id<FunctionReference>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C8): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitImplicitCallReference(node, contextType: contextType)`.
pub fn visit_implicit_call_reference(rv: &mut ResolverVisitor<'_>, node: Id<ImplicitCallReference>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C8): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor._insertImplicitCallReference` after the form and
/// context checks (`getImplicitCallMethod`, the `ImplicitCallReference`
/// node). STUB (C8): no call reference is inserted.
pub fn insert_implicit_call_reference(
    rv: &mut ResolverVisitor<'_>,
    expression: dartr_ast::Id<dartr_ast::Expression>,
    context_type: TypeId,
) {
    let _ = (rv, expression, context_type);
}
