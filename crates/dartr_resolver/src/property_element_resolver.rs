// Dart source: pkg/analyzer/lib/src/dart/resolver/property_element_resolver.dart

//! Partly STUB (unit C4): `PropertyElementResolver`. Ported:
//! [`PropertyElementResolverResult`] and [`resolve_simple_identifier`]
//! without the cascade case (needed by the identifier resolution of the
//! core). The rest (property access, index expressions, prefixed
//! identifiers, `_resolve`, `_resolveTargetInterfaceElement`, ...) is a stub.
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Id, IndexExpression, PropertyAccess};
use dartr_element::TypeId;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitPropertyAccess(node, contextType: contextType)`.
pub fn visit_property_access(rv: &mut ResolverVisitor<'_>, node: Id<PropertyAccess>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C4): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `ResolverVisitor.visitIndexExpression(node, contextType: contextType)`.
pub fn visit_index_expression(rv: &mut ResolverVisitor<'_>, node: Id<IndexExpression>, context_type: TypeId) {
    let _ = context_type;
    // STUB (C4): fallback.
    rv.fallback_expression(node.upcast());
}

/// Dart `PropertyElementResolverResult`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PropertyElementResolverResult {
    pub read_element_requested: Option<dartr_element::ElemRef>,
    pub read_element_recovery: Option<dartr_element::ElemRef>,
    pub write_element_requested: Option<dartr_element::ElemRef>,
    pub write_element_recovery: Option<dartr_element::ElemRef>,
    pub at_dynamic_target: bool,
    pub function_type_call_type: Option<TypeId>,
    pub record_field: Option<crate::resolution_result::RecordField>,
    pub get_type: Option<TypeId>,
    /// If an `IndexExpression` is resolved, the context type of the index
    /// (`_` if `[]` or `[]=` are not resolved or invalid). `None` = `_`.
    pub index_context_type: Option<TypeId>,
}

impl PropertyElementResolverResult {
    /// Dart `readElement2`.
    pub fn read_element(&self) -> Option<dartr_element::ElemRef> {
        self.read_element_requested.or(self.read_element_recovery)
    }

    /// Dart `writeElement2`.
    pub fn write_element(&self) -> Option<dartr_element::ElemRef> {
        self.write_element_requested.or(self.write_element_recovery)
    }
}

/// Dart `SimpleIdentifierImpl.ancestorCascade`.
pub fn ancestor_cascade(rv: &ResolverVisitor<'_>, node: Id<dartr_ast::SimpleIdentifier>) -> Option<Id<dartr_ast::CascadeExpression>> {
    let token = rv.ast[node].token;
    let previous = rv.ast.tokens.previous(token);
    let ty = rv.ast.tokens.ty(previous);
    if ty == dartr_syntax::TokenType::PERIOD_PERIOD || ty == dartr_syntax::TokenType::QUESTION_PERIOD_PERIOD {
        return rv.ast.this_or_ancestor_of_type::<dartr_ast::CascadeExpression>(node);
    }
    None
}

/// Dart `PropertyElementResolver.resolveSimpleIdentifier(node:, hasRead:,
/// hasWrite:)`.
pub fn resolve_simple_identifier(
    rv: &mut ResolverVisitor<'_>,
    node: Id<dartr_ast::SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    use dartr_flow::flow_analysis::{FlowAnalysis, PropertyTarget};
    use dartr_flow::shared_type::SharedTypeView;

    if ancestor_cascade(rv, node).is_some() {
        // STUB (C4): `_resolve(node:, target: ancestorCascade.target,
        // isCascaded: true, ...)`.
        return PropertyElementResolverResult::default();
    }

    let scope_lookup_result = rv
        .rt
        .scope_lookup_result
        .get(node)
        .copied()
        .unwrap_or_default();
    // Dart `reportDeprecatedExportUse(...)` (wave D).

    let mut read_element_requested = None;
    let mut get_type = None;
    if has_read {
        let ctx = rv.ctx;
        let read_lookup = match crate::lexical_lookup::resolve_getter(&ctx, scope_lookup_result) {
            Some(r) => Some(r),
            None => crate::this_lookup::lookup_getter(rv, node),
        };
        if let Some(call_function_type) = read_lookup.and_then(|r| r.call_function_type) {
            return PropertyElementResolverResult {
                function_type_call_type: Some(call_function_type),
                ..Default::default()
            };
        }
        if let Some(record_field) = read_lookup.and_then(|r| r.record_field) {
            return PropertyElementResolverResult {
                record_field: Some(record_field),
                ..Default::default()
            };
        }
        read_element_requested = read_lookup.and_then(|r| r.requested);
        if let Some(requested) = read_element_requested
            && dartr_typesystem::member::base_element(&ctx, requested)
                .is::<dartr_element::PropertyAccessorElement>()
            && !dartr_typesystem::member::is_static(&ctx, requested)
        {
            let unpromoted_type = dartr_typesystem::member::return_type(&ctx, requested);
            let name = ctx.name(rv.lexeme(rv.ast[node].token));
            if let Some(flow) = rv.flow_analysis.flow.as_mut() {
                let (promoted, info) = flow.property_get(
                    PropertyTarget::This,
                    name,
                    Some(requested),
                    SharedTypeView::new(unpromoted_type),
                );
                rv.flow_analysis.store_expression_info(node.upcast(), info);
                get_type = promoted.map(|t| t.unwrap_type_view());
            }
            get_type = get_type.or(Some(unpromoted_type));
        }
        rv.check_read_of_not_assigned_local_variable(node, read_element_requested);
    }

    let mut write_element_requested = None;
    let mut write_element_recovery = None;
    if has_write {
        let ctx = rv.ctx;
        let write_lookup = match crate::lexical_lookup::resolve_setter(&ctx, scope_lookup_result) {
            Some(r) => Some(r),
            None => crate::this_lookup::lookup_setter(rv, node),
        };
        write_element_requested = write_lookup.and_then(|r| r.requested);
        write_element_recovery = write_lookup.and_then(|r| r.recovery);
        // Dart `AssignmentVerifier(diagnosticReporter).verify(...)` (C5).
    }

    PropertyElementResolverResult {
        read_element_requested,
        read_element_recovery: None,
        write_element_requested,
        write_element_recovery,
        get_type,
        ..Default::default()
    }
}
