// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (visitDotShorthand*) and the dot shorthand parts of the invocation resolvers

//! STUB (unit C8): `dot shorthands`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use crate::resolver::ResolverVisitor;
use dartr_ast::{
    DotShorthandConstructorInvocation, DotShorthandInvocation, DotShorthandPropertyAccess, Id,
};
use dartr_element::TypeId;

/// Dart `ResolverVisitor.visitDotShorthandConstructorInvocation(node, contextType: contextType)`.
pub fn visit_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
    context_type: TypeId,
) {
    crate::instance_creation_expression_resolver::visit_dot_shorthand_constructor_invocation(
        rv,
        node,
        context_type,
    );
}

/// Dart `ResolverVisitor.visitDotShorthandInvocation(node, contextType: contextType)`.
pub fn visit_dot_shorthand_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    context_type: TypeId,
) {
    let _ = context_type;
    // STUB (C8): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitDotShorthandPropertyAccess(node, contextType: contextType)`.
pub fn visit_dot_shorthand_property_access(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandPropertyAccess>,
    context_type: TypeId,
) {
    let _ = context_type;
    // STUB (C8): fallback.
    rv.fallback_expression(node.upcast());
}
