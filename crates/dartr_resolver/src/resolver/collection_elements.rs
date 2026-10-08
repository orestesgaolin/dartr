// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (resolveElement of
// ExpressionImpl, ForElementImpl, IfElementImpl, MapLiteralEntryImpl,
// NullAwareElementImpl, SpreadElementImpl)

//! The collection element dispatch targets of [`ResolverVisitor`]. Each one
//! leaves one entry on the rewrite stack (the expression, or `None`), which
//! `dispatch_collection_element` pops.

use dartr_ast::*;
use dartr_element::TypeId;

use crate::resolver::{CollectionLiteralContext, ResolverVisitor};
use crate::{for_resolver, typed_literal_resolver};

impl<'a> ResolverVisitor<'a> {
    /// Dart `ExpressionImpl.resolveElement(resolver, context)`.
    pub(crate) fn resolve_element_expression(
        &mut self,
        node: Id<Expression>,
        context: Option<CollectionLiteralContext>,
    ) {
        // While typing `{key^}` in a `Map<K, V>` context, recover by using
        // `K`.
        let context_type = context
            .and_then(|c| c.element_type.or(c.key_type))
            .unwrap_or(TypeId::UNKNOWN);
        self.analyze_expression_node(node, context_type);
    }

    pub(crate) fn resolve_element_for_element(&mut self, node: Id<ForElement>, context: Option<CollectionLiteralContext>) {
        for_resolver::visit_for_element(self, node, context);
        self.push_rewrite(None);
    }

    pub(crate) fn resolve_element_if_element(&mut self, node: Id<IfElement>, context: Option<CollectionLiteralContext>) {
        typed_literal_resolver::visit_if_element(self, node, context);
        self.push_rewrite(None);
    }

    pub(crate) fn resolve_element_map_literal_entry(
        &mut self,
        node: Id<MapLiteralEntry>,
        context: Option<CollectionLiteralContext>,
    ) {
        typed_literal_resolver::visit_map_literal_entry(self, node, context);
        self.push_rewrite(None);
    }

    pub(crate) fn resolve_element_null_aware_element(
        &mut self,
        node: Id<NullAwareElement>,
        context: Option<CollectionLiteralContext>,
    ) {
        typed_literal_resolver::visit_null_aware_element(self, node, context);
        self.push_rewrite(None);
    }

    pub(crate) fn resolve_element_spread_element(
        &mut self,
        node: Id<SpreadElement>,
        context: Option<CollectionLiteralContext>,
    ) {
        typed_literal_resolver::visit_spread_element(self, node, context);
        self.push_rewrite(None);
    }
}
