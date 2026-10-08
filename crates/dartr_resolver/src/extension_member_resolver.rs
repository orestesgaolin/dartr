// Dart source: pkg/analyzer/lib/src/dart/resolver/extension_member_resolver.dart

//! STUB (unit C6): `ExtensionMemberResolver`.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, ExtensionOverride};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitExtensionOverride(node, contextType: contextType)`.
pub fn visit_extension_override(rv: &mut ResolverVisitor<'_>, node: Id<ExtensionOverride>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C6): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ExtensionResolutionResult` / `ExtensionResolutionError`: the getter
/// and setter of the single most specific applicable extension, or
/// whether the extensions are ambiguous.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExtensionResolutionResult {
    pub getter: Option<dartr_element::ElemRef>,
    pub setter: Option<dartr_element::ElemRef>,
    /// Dart `ExtensionResolutionError.ambiguous`.
    pub is_ambiguous: bool,
}

/// Dart `ExtensionMemberResolver.findExtension(type, nameEntity, name)`:
/// the extension member [name] applicable to [ty]. [name_entity] is the
/// node to report an ambiguity on. STUB (C6): no extension is found.
pub fn find_extension(
    rv: &mut ResolverVisitor<'_>,
    ty: TypeId,
    name_entity: dartr_ast::NodeId,
    name: &dartr_typesystem::inheritance_manager3::Name,
) -> ExtensionResolutionResult {
    let _ = (rv, ty, name_entity, name);
    ExtensionResolutionResult::default()
}
