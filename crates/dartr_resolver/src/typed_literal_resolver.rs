// Dart source: pkg/analyzer/lib/src/dart/resolver/typed_literal_resolver.dart

//! `TypedLiteralResolver`: resolves [ListLiteral]s and [SetOrMapLiteral]s
//! (inference of the element, key and value types, set-or-map
//! disambiguation). Also the Dart `ResolverVisitor.visitX` of the collection
//! elements that are not for elements: `visitIfElement`,
//! `visitMapLiteralEntry`, `visitSpreadElement`, `visitNullAwareElement`
//! (`generated/resolver.dart`).
//!
//! Differences to Dart:
//! - `SetOrMapLiteralImpl.becomeMap` / `becomeSet` are not stored: the
//!   kind of a resolved literal follows from its static type (`Map`,
//!   `Set`, or `dynamic` when it is ambiguous).
//! - `SetOrMapLiteralImpl.contextType` (a field that only lives between the
//!   downwards and the upwards inference) is a local value.

use dartr_ast::{
    CollectionElement, Expression, ForElement, Id, IfElement, ListLiteral, MapLiteralEntry, NodeId,
    NullAwareElement, SetOrMapLiteral, SpreadElement, TypeAnnotation,
};
use dartr_diagnostics::{Diagnostic, DiagnosticReporter, diag};
use dartr_element::{ClassElement, EId, Nullability, TypeId, TypeKind};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::TypeExt;
use dartr_typesystem::generic_inferrer::{
    GenericInferrer, InferenceErrorEntity, InferenceErrorEntityKind, InferenceFlags,
};

use crate::ast_ext::in_constant_context;
use crate::pattern_resolver::guarded_pattern_variables;
use crate::resolver::CollectionLiteralContext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitListLiteral(node, contextType: contextType)`.
pub fn visit_list_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ListLiteral>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    resolve_list_literal(rv, node, context_type);
}

/// Dart `ResolverVisitor.visitSetOrMapLiteral(node, contextType:
/// contextType)`.
pub fn visit_set_or_map_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SetOrMapLiteral>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    resolve_set_or_map_literal(rv, node, context_type);
}

/// Dart `ResolverVisitor.visitIfElement(node, context: context)`.
pub fn visit_if_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<IfElement>,
    context: Option<CollectionLiteralContext>,
) {
    let expression = rv.ast[node].expression;
    let then_element = rv.ast[node].then_element;
    let else_element = rv.ast[node].else_element;
    if let Some(case_clause) = rv.ast[node].case_clause {
        let guarded_pattern = rv.ast[case_clause].guarded_pattern;
        let pattern = rv.ast[guarded_pattern].pattern;
        let guard = rv.ast[guarded_pattern]
            .when_clause
            .map(|w| rv.ast[w].expression);
        let variables = guarded_pattern_variables(rv, guarded_pattern);
        rv.analyze_if_case_element(
            node.raw(),
            expression,
            pattern,
            &variables,
            guard,
            then_element.raw(),
            else_element.map(|e| e.raw()),
            context,
        );
        // Stack: (Expression, Guard)
        rv.pop_rewrite(); // guard
        rv.pop_rewrite(); // expression
    } else {
        rv.analyze_if_element(
            node.raw(),
            expression,
            then_element.raw(),
            else_element.map(|e| e.raw()),
            context,
        );
    }
}

/// Dart `ResolverVisitor.visitMapLiteralEntry(node, context: context)`.
pub fn visit_map_literal_entry(
    rv: &mut ResolverVisitor<'_>,
    node: Id<MapLiteralEntry>,
    context: Option<CollectionLiteralContext>,
) {
    rv.check_unreachable_node(node);

    let is_key_null_aware = rv.ast[node].key_question.is_some();
    let is_value_null_aware = rv.ast[node].value_question.is_some();

    // If the key is null-aware, the context of the expression under `?`
    // should be changed to the nullable version of the downwards context.
    let mut key_type_context = context.and_then(|c| c.key_type);
    if let Some(t) = key_type_context
        && is_key_null_aware
    {
        key_type_context = Some(rv.type_system.make_nullable(t));
    }
    let key = rv.ast[node].key;
    let key_type = rv
        .analyze_expression_node(key, key_type_context.unwrap_or(TypeId::UNKNOWN))
        .type_
        .unwrap_type_view();
    rv.pop_rewrite();

    let key = rv.ast[node].key;
    let key_info = rv.flow_analysis.get_expression_info(Some(key));
    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.null_aware_map_entry_value_begin(
            key_info,
            SharedTypeView::new(key_type),
            is_key_null_aware,
        );
    }

    // If the value is null-aware, the context of the expression under `?`
    // should be changed to the nullable version of the downwards context.
    let mut value_type_context = context.and_then(|c| c.value_type);
    if let Some(t) = value_type_context
        && is_value_null_aware
    {
        value_type_context = Some(rv.type_system.make_nullable(t));
    }
    let value = rv.ast[node].value;
    rv.analyze_expression_node(value, value_type_context.unwrap_or(TypeId::UNKNOWN));
    rv.pop_rewrite();

    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.null_aware_map_entry_end(is_key_null_aware);
    }
}

/// Dart `ResolverVisitor.visitSpreadElement(node, context: context)`.
pub fn visit_spread_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SpreadElement>,
    context: Option<CollectionLiteralContext>,
) {
    let is_null_aware = spread_is_null_aware(rv, node);
    let mut iterable_type = context.and_then(|c| c.iterable_type);
    if let Some(t) = iterable_type
        && is_null_aware
    {
        iterable_type = Some(rv.type_system.make_nullable(t));
    }
    rv.check_unreachable_node(node);
    let expression = rv.ast[node].expression;
    rv.analyze_expression_node(expression, iterable_type.unwrap_or(TypeId::UNKNOWN));
    rv.pop_rewrite();

    if !is_null_aware {
        let expression = rv.ast[node].expression;
        rv.nullable_dereference_expression(
            diag::unchecked_use_of_nullable_value_in_spread(),
            expression,
            None,
        );
    }
}

/// Dart `ResolverVisitor.visitNullAwareElement(node, context: context)`.
pub fn visit_null_aware_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<NullAwareElement>,
    context: Option<CollectionLiteralContext>,
) {
    let element_type = context
        .and_then(|c| c.element_type)
        .map(|t| rv.type_system.make_nullable(t));
    let value = rv.ast[node].value;
    rv.analyze_expression_node(value, element_type.unwrap_or(TypeId::UNKNOWN));
    rv.pop_rewrite();
}

/// Dart `SpreadElement.isNullAware`.
fn spread_is_null_aware(rv: &ResolverVisitor<'_>, node: Id<SpreadElement>) -> bool {
    rv.lexeme(rv.ast[node].spread_operator) == "...?"
}

/// The types of the type arguments (Dart `typeArguments[i].typeOrThrow`).
fn type_argument_types(
    rv: &ResolverVisitor<'_>,
    type_arguments: &[Id<TypeAnnotation>],
) -> Vec<TypeId> {
    type_arguments
        .iter()
        .map(|&t| {
            rv.tables
                .annotation_type
                .get(t)
                .copied()
                .unwrap_or(TypeId::INVALID)
        })
        .collect()
}

/// Dart `node.typeArguments?.arguments` of a list or set-or-map literal.
fn literal_type_arguments(
    rv: &ResolverVisitor<'_>,
    type_arguments: Option<Id<dartr_ast::TypeArgumentList>>,
) -> Option<Vec<TypeId>> {
    let type_arguments = type_arguments?;
    let arguments = rv.ast.list(rv.ast[type_arguments].arguments).to_vec();
    Some(type_argument_types(rv, &arguments))
}

fn is_dynamic(rv: &ResolverVisitor<'_>, t: TypeId) -> bool {
    matches!(rv.ctx.ty(t), TypeKind::Dynamic)
}

fn inference_flags(rv: &ResolverVisitor<'_>) -> InferenceFlags {
    InferenceFlags {
        generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
        inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
        strict_inference: rv.unit.options.strict_inference,
    }
}

/// Dart `_typeSystem.setupGenericTypeInference(...)` for the class
/// [element] (`List`, `Set` or `Map`) of a collection literal.
fn setup_literal_inference<'a, 'r, 'l>(
    rv: &ResolverVisitor<'a>,
    element: EId<ClassElement>,
    node: NodeId,
    context_type: TypeId,
    diagnostic_reporter: Option<&'r mut DiagnosticReporter<'l>>,
) -> GenericInferrer<'a, 'r, 'l> {
    let element = element.upcast();
    let type_parameters = rv.ctx.interface_type_parameters(element);
    let declared_return_type = rv.ctx.interface_this_type(element);
    let is_const = literal_is_const(rv, node);
    let error_entity = diagnostic_reporter.as_ref().map(|_| InferenceErrorEntity {
        offset: rv.ast.offset(node) as usize,
        length: rv.ast.length(node) as usize,
        is_invocation_in_as_expression: false,
        kind: InferenceErrorEntityKind::Expression {
            static_type: rv.static_type(node),
        },
    });
    rv.type_system.setup_generic_type_inference(
        type_parameters,
        declared_return_type,
        context_type,
        diagnostic_reporter,
        error_entity,
        inference_flags(rv),
        is_const,
        rv.flow_analysis.type_operations,
        None,
        Some(node),
    )
}

/// Dart `TypedLiteral.isConst`: `constKeyword != null ||
/// inConstantContext`.
fn literal_is_const(rv: &ResolverVisitor<'_>, node: NodeId) -> bool {
    let const_keyword = if let Some(list) = rv.ast.cast::<ListLiteral>(node) {
        rv.ast[list].const_keyword
    } else if let Some(set_or_map) = rv.ast.cast::<SetOrMapLiteral>(node) {
        rv.ast[set_or_map].const_keyword
    } else {
        None
    };
    const_keyword.is_some() || in_constant_context(rv.ast, node)
}

/// The type parameter `T` of [element] as a type (Dart
/// `typeParameters[index].instantiate(nullabilitySuffix: none)`).
fn generic_type(rv: &ResolverVisitor<'_>, element: EId<ClassElement>, index: usize) -> TypeId {
    let parameter = rv.ctx.interface_type_parameters(element.upcast())[index];
    rv.ctx.type_parameter_type(parameter, Nullability::None)
}

/// Dart `_resolveElements`.
fn resolve_elements(
    rv: &mut ResolverVisitor<'_>,
    elements: &[Id<CollectionElement>],
    context: Option<CollectionLiteralContext>,
) {
    for &element in elements {
        rv.dispatch_collection_element(element.raw(), context);
    }
}

// ------------------------------------------------------------ list literals

/// Dart `resolveListLiteral`.
fn resolve_list_literal(rv: &mut ResolverVisitor<'_>, node: Id<ListLiteral>, context_type: TypeId) {
    let mut reported: Vec<Diagnostic> = Vec::new();
    {
        let mut listener = |d: Diagnostic| reported.push(d);
        let mut reporter = DiagnosticReporter::new(&mut listener);

        let mut element_type = None;
        let mut inferrer = None;

        let type_arguments = literal_type_arguments(rv, rv.ast[node].type_arguments);
        if let Some(type_arguments) = &type_arguments {
            if type_arguments.len() == 1 {
                let ty = type_arguments[0];
                if !is_dynamic(rv, ty) {
                    element_type = Some(ty);
                }
            }
        } else {
            let mut i = infer_list_type_downwards(rv, node, context_type, &mut reporter);
            if context_type != TypeId::UNKNOWN {
                let type_arguments = i.choose_preliminary_types();
                element_type = Some(type_arguments[0]);
            }
            inferrer = Some(i);
        }
        let context = element_type.map(|element_type| CollectionLiteralContext {
            element_type: Some(element_type),
            iterable_type: Some(rv.ctx.tp.iterable_type(&rv.ctx, element_type)),
            key_type: None,
            value_type: None,
        });

        if let Some(type_arguments) = rv.ast[node].type_arguments {
            rv.visit_node(type_arguments.raw());
        }
        let elements = rv.ast.list(rv.ast[node].elements).to_vec();
        resolve_elements(rv, &elements, context);
        let static_type = resolve_list_literal2(rv, inferrer, node, context_type);
        rv.record_static_type(node, static_type);
    }
    rv.flush_type_analyzer_errors();
    if rv.lock_level == 0 {
        rv.diagnostics.extend(reported);
    }
}

/// Dart `_inferListTypeDownwards`.
fn infer_list_type_downwards<'a, 'r, 'l>(
    rv: &ResolverVisitor<'a>,
    node: Id<ListLiteral>,
    context_type: TypeId,
    reporter: &'r mut DiagnosticReporter<'l>,
) -> GenericInferrer<'a, 'r, 'l> {
    let element = rv.ctx.tp.list_element();
    setup_literal_inference(rv, element, node.raw(), context_type, Some(reporter))
}

/// Dart `_inferListTypeUpwards`.
fn infer_list_type_upwards(
    rv: &mut ResolverVisitor<'_>,
    inferrer: &mut GenericInferrer<'_, '_, '_>,
    node: Id<ListLiteral>,
    context_type: TypeId,
) -> TypeId {
    let element = rv.ctx.tp.list_element();
    let generic_element_type = generic_type(rv, element, 0);

    // Also use upwards information to infer the type.
    let elements = rv.ast.list(rv.ast[node].elements).to_vec();
    let element_types: Vec<TypeId> = elements
        .iter()
        .map(|&e| compute_element_type(rv, e))
        .collect();
    if rv.unit.options.strict_inference
        && element_types.is_empty()
        && context_type == TypeId::UNKNOWN
    {
        // We cannot infer the type of a collection literal with no elements,
        // and no context type. If there are any elements, inference has not
        // failed, as the types of those elements are considered resolved.
        let d = rv.at(diag::inference_failure_on_collection_literal("List"), node);
        rv.report(d);
    }

    for &element_type in &element_types {
        inferrer.constrain_argument(
            element_type,
            generic_element_type,
            "element",
            Some(node.raw()),
        );
    }
    let type_arguments = inferrer.choose_final_types();
    rv.ctx
        .instantiate_interface(element.upcast(), &type_arguments, Nullability::None)
}

/// Dart `_resolveListLiteral2`.
fn resolve_list_literal2(
    rv: &mut ResolverVisitor<'_>,
    inferrer: Option<GenericInferrer<'_, '_, '_>>,
    node: Id<ListLiteral>,
    context_type: TypeId,
) -> TypeId {
    // If we have explicit arguments, use them.
    if let Some(type_arguments) = literal_type_arguments(rv, rv.ast[node].type_arguments) {
        let element_type = if type_arguments.len() == 1 {
            type_arguments[0]
        } else {
            TypeId::DYNAMIC
        };
        return rv.ctx.tp.list_type(&rv.ctx, element_type);
    }

    // If there are no type arguments, try to infer some arguments.
    let Some(mut inferrer) = inferrer else {
        return rv.ctx.tp.list_type(&rv.ctx, TypeId::DYNAMIC);
    };
    infer_list_type_upwards(rv, &mut inferrer, node, context_type)
}

/// Dart `_computeElementType`.
fn compute_element_type(rv: &ResolverVisitor<'_>, element: Id<CollectionElement>) -> TypeId {
    let n = element.raw();
    let ts = rv.type_system;
    if let Some(e) = rv.ast.cast::<Expression>(n) {
        rv.type_or_throw(e)
    } else if let Some(e) = rv.ast.cast::<ForElement>(n) {
        compute_element_type(rv, rv.ast[e].body)
    } else if let Some(e) = rv.ast.cast::<IfElement>(n) {
        let then_type = compute_element_type(rv, rv.ast[e].then_element);
        let Some(else_element) = rv.ast[e].else_element else {
            return then_type;
        };
        let else_type = compute_element_type(rv, else_element);
        ts.least_upper_bound(then_type, else_type)
    } else if rv.ast.cast::<MapLiteralEntry>(n).is_some() {
        // This error will be reported elsewhere.
        TypeId::DYNAMIC
    } else if let Some(e) = rv.ast.cast::<SpreadElement>(n) {
        let expression_type = rv.type_or_throw(rv.ast[e].expression);

        let iterable_element = rv.ctx.tp.iterable_element().upcast();
        if let Some(iterable_type) = rv.ctx.as_instance_of(expression_type, iterable_element) {
            return rv.ctx.type_arguments(iterable_type)[0];
        }

        if is_dynamic(rv, expression_type) {
            return TypeId::DYNAMIC;
        }

        let never = rv.ctx.tp.never_type();
        if ts.is_subtype_of(expression_type, never) {
            return never;
        }

        if ts.is_subtype_of(expression_type, ts.null_none()) {
            if spread_is_null_aware(rv, e) {
                return never;
            }
            return TypeId::DYNAMIC;
        }

        TypeId::DYNAMIC
    } else if let Some(e) = rv.ast.cast::<NullAwareElement>(n) {
        ts.promote_to_non_null(rv.type_or_throw(rv.ast[e].value))
    } else {
        // Dart throws `UnimplementedError`.
        TypeId::DYNAMIC
    }
}

// ------------------------------------------------------------ set or map literals

/// Dart `_LiteralResolutionKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiteralResolutionKind {
    Ambiguous,
    Map,
    Set,
}

/// Dart `_LiteralResolution`.
#[derive(Clone, Copy, Debug)]
struct LiteralResolution {
    kind: LiteralResolutionKind,
    context_type: Option<TypeId>,
}

impl LiteralResolution {
    fn new(kind: LiteralResolutionKind, context_type: Option<TypeId>) -> Self {
        LiteralResolution { kind, context_type }
    }
}

/// Dart `resolveSetOrMapLiteral`.
fn resolve_set_or_map_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SetOrMapLiteral>,
    context_type: TypeId,
) {
    let type_arguments = literal_type_arguments(rv, rv.ast[node].type_arguments);

    let mut literal_type = None;
    let mut inferrer = None;
    let literal_resolution = compute_set_or_map_resolution(rv, node, context_type);
    match literal_resolution.kind {
        LiteralResolutionKind::Set => {
            if let Some(type_arguments) = type_arguments.as_ref().filter(|a| a.len() == 1) {
                let element_type = type_arguments[0];
                literal_type = Some(rv.ctx.tp.set_type(&rv.ctx, element_type));
            } else {
                let mut i = setup_literal_inference(
                    rv,
                    rv.ctx.tp.set_element(),
                    node.raw(),
                    literal_resolution.context_type.unwrap_or(TypeId::UNKNOWN),
                    None,
                );
                if literal_resolution.context_type.is_some() {
                    let type_arguments = i.choose_preliminary_types();
                    literal_type = Some(rv.ctx.instantiate_interface(
                        rv.ctx.tp.set_element().upcast(),
                        &type_arguments,
                        Nullability::None,
                    ));
                }
                inferrer = Some(i);
            }
        }
        LiteralResolutionKind::Map => {
            if let Some(type_arguments) = type_arguments.as_ref().filter(|a| a.len() == 2) {
                let key_type = type_arguments[0];
                let value_type = type_arguments[1];
                literal_type = Some(rv.ctx.tp.map_type(&rv.ctx, key_type, value_type));
            } else {
                let mut i = setup_literal_inference(
                    rv,
                    rv.ctx.tp.map_element(),
                    node.raw(),
                    literal_resolution.context_type.unwrap_or(TypeId::UNKNOWN),
                    None,
                );
                if literal_resolution.context_type.is_some() {
                    let type_arguments = i.choose_preliminary_types();
                    literal_type = Some(rv.ctx.instantiate_interface(
                        rv.ctx.tp.map_element().upcast(),
                        &type_arguments,
                        Nullability::None,
                    ));
                }
                inferrer = Some(i);
            }
        }
        LiteralResolutionKind::Ambiguous => {}
    }
    let mut context = None;
    if let Some(literal_type) = literal_type {
        let type_arguments = rv.ctx.type_arguments(literal_type);
        if type_arguments.len() == 1 {
            let element_type = type_arguments[0];
            context = Some(CollectionLiteralContext {
                element_type: Some(element_type),
                iterable_type: Some(rv.ctx.tp.iterable_type(&rv.ctx, element_type)),
                key_type: None,
                value_type: None,
            });
        } else if type_arguments.len() == 2 {
            context = Some(CollectionLiteralContext {
                element_type: None,
                iterable_type: Some(literal_type),
                key_type: Some(type_arguments[0]),
                value_type: Some(type_arguments[1]),
            });
        }
    }

    if let Some(type_arguments) = rv.ast[node].type_arguments {
        rv.visit_node(type_arguments.raw());
    }
    let elements = rv.ast.list(rv.ast[node].elements).to_vec();
    resolve_elements(rv, &elements, context);
    resolve_set_or_map_literal2(
        rv,
        inferrer,
        literal_resolution,
        node,
        literal_type,
        context_type,
    );
}

/// Dart `_computeSetOrMapResolution`: the resolution suggested by the type
/// arguments, the context type and the elements of [literal].
fn compute_set_or_map_resolution(
    rv: &ResolverVisitor<'_>,
    literal: Id<SetOrMapLiteral>,
    context_type: TypeId,
) -> LiteralResolution {
    let type_arguments_resolution = from_type_arguments(
        rv,
        literal_type_arguments(rv, rv.ast[literal].type_arguments),
    );
    let context_resolution = from_context_type(rv, Some(context_type));
    let elements = rv.ast.list(rv.ast[literal].elements).to_vec();
    let element_counts = LeafElements::new(rv, &elements);
    let element_resolution = element_counts.resolution();

    let mut unambiguous_resolutions = Vec::new();
    let mut kinds = Vec::new();
    for resolution in [
        type_arguments_resolution,
        context_resolution,
        element_resolution,
    ] {
        if resolution.kind != LiteralResolutionKind::Ambiguous {
            unambiguous_resolutions.push(resolution);
            if !kinds.contains(&resolution.kind) {
                kinds.push(resolution.kind);
            }
        }
    }

    if kinds.len() == 2 {
        // It looks like it needs to be both a map and a set. Attempt to
        // recover.
        if element_resolution.kind == LiteralResolutionKind::Ambiguous
            && element_resolution.context_type.is_some()
        {
            return element_resolution;
        } else if type_arguments_resolution.kind != LiteralResolutionKind::Ambiguous
            && type_arguments_resolution.context_type.is_some()
        {
            return type_arguments_resolution;
        } else if context_resolution.kind != LiteralResolutionKind::Ambiguous
            && context_resolution.context_type.is_some()
        {
            return context_resolution;
        }
    } else if unambiguous_resolutions.len() >= 2 {
        // If there are three resolutions, the last resolution is guaranteed
        // to be from the elements, which always has a context type of `null`
        // (when it is not ambiguous). So, whether there are 2 or 3
        // resolutions only the first two are potentially interesting.
        return if unambiguous_resolutions[0].context_type.is_none() {
            unambiguous_resolutions[1]
        } else {
            unambiguous_resolutions[0]
        };
    } else if unambiguous_resolutions.len() == 1 {
        return unambiguous_resolutions[0];
    } else if elements.is_empty() {
        return LiteralResolution::new(
            LiteralResolutionKind::Map,
            Some(
                rv.ctx
                    .tp
                    .map_type(&rv.ctx, TypeId::DYNAMIC, TypeId::DYNAMIC),
            ),
        );
    }
    LiteralResolution::new(LiteralResolutionKind::Ambiguous, None)
}

/// Dart `_fromContextType`: a set literal when [context_type] implements
/// `Iterable` but not `Map`, a map literal when it implements `Map` but
/// not `Iterable`.
fn from_context_type(rv: &ResolverVisitor<'_>, context_type: Option<TypeId>) -> LiteralResolution {
    if let Some(context_type) = context_type {
        let unwrapped_context_type = rv.type_system.future_or_base(context_type);
        let iterable_type = rv.ctx.as_instance_of(
            unwrapped_context_type,
            rv.ctx.tp.iterable_element().upcast(),
        );
        let map_type = rv
            .ctx
            .as_instance_of(unwrapped_context_type, rv.ctx.tp.map_element().upcast());
        let is_iterable = iterable_type.is_some();
        let is_map = map_type.is_some();

        // When `S` implements `Iterable` but not `Map`, `e` is a set literal.
        if is_iterable && !is_map {
            return LiteralResolution::new(
                LiteralResolutionKind::Set,
                Some(unwrapped_context_type),
            );
        }

        // When `S` implements `Map` but not `Iterable`, `e` is a map literal.
        if is_map && !is_iterable {
            return LiteralResolution::new(
                LiteralResolutionKind::Map,
                Some(unwrapped_context_type),
            );
        }
    }

    LiteralResolution::new(LiteralResolutionKind::Ambiguous, None)
}

/// Dart `_fromTypeArguments`.
fn from_type_arguments(
    rv: &ResolverVisitor<'_>,
    arguments: Option<Vec<TypeId>>,
) -> LiteralResolution {
    if let Some(arguments) = arguments {
        if arguments.len() == 1 {
            return LiteralResolution::new(
                LiteralResolutionKind::Set,
                Some(rv.ctx.tp.set_type(&rv.ctx, arguments[0])),
            );
        } else if arguments.len() == 2 {
            return LiteralResolution::new(
                LiteralResolutionKind::Map,
                Some(rv.ctx.tp.map_type(&rv.ctx, arguments[0], arguments[1])),
            );
        }
    }
    LiteralResolution::new(LiteralResolutionKind::Ambiguous, None)
}

/// Dart `_inferCollectionElementType`.
fn infer_collection_element_type(
    rv: &ResolverVisitor<'_>,
    element: Id<CollectionElement>,
) -> InferredCollectionElementTypeInformation {
    let n = element.raw();
    let ts = rv.type_system;
    if let Some(e) = rv.ast.cast::<Expression>(n) {
        InferredCollectionElementTypeInformation {
            element_type: Some(rv.type_or_throw(e)),
            ..Default::default()
        }
    } else if let Some(e) = rv.ast.cast::<ForElement>(n) {
        infer_collection_element_type(rv, rv.ast[e].body)
    } else if let Some(e) = rv.ast.cast::<IfElement>(n) {
        let then_type = infer_collection_element_type(rv, rv.ast[e].then_element);
        let Some(else_element) = rv.ast[e].else_element else {
            return then_type;
        };
        let else_type = infer_collection_element_type(rv, else_element);
        InferredCollectionElementTypeInformation::for_if_element(rv, then_type, else_type)
    } else if let Some(e) = rv.ast.cast::<MapLiteralEntry>(n) {
        let mut key_type = rv.static_type(rv.ast[e].key);
        if let Some(t) = key_type
            && rv.ast[e].key_question.is_some()
        {
            key_type = Some(ts.promote_to_non_null(t));
        }
        let mut value_type = rv.static_type(rv.ast[e].value);
        if let Some(t) = value_type
            && rv.ast[e].value_question.is_some()
        {
            value_type = Some(ts.promote_to_non_null(t));
        }
        InferredCollectionElementTypeInformation {
            element_type: None,
            key_type,
            value_type,
        }
    } else if let Some(e) = rv.ast.cast::<SpreadElement>(n) {
        let expression_type = rv.type_or_throw(rv.ast[e].expression);

        let iterable_element = rv.ctx.tp.iterable_element().upcast();
        if let Some(iterable_type) = rv.ctx.as_instance_of(expression_type, iterable_element) {
            return InferredCollectionElementTypeInformation {
                element_type: Some(rv.ctx.type_arguments(iterable_type)[0]),
                ..Default::default()
            };
        }

        let map_element = rv.ctx.tp.map_element().upcast();
        if let Some(map_type) = rv.ctx.as_instance_of(expression_type, map_element) {
            let args = rv.ctx.type_arguments(map_type);
            return InferredCollectionElementTypeInformation {
                element_type: None,
                key_type: Some(args[0]),
                value_type: Some(args[1]),
            };
        }

        if is_dynamic(rv, expression_type) {
            return InferredCollectionElementTypeInformation::all(expression_type);
        }

        let never = rv.ctx.tp.never_type();
        if ts.is_subtype_of(expression_type, never) {
            return InferredCollectionElementTypeInformation::all(never);
        }

        if ts.is_subtype_of(expression_type, ts.null_none()) && spread_is_null_aware(rv, e) {
            return InferredCollectionElementTypeInformation::all(never);
        }

        InferredCollectionElementTypeInformation::default()
    } else if let Some(e) = rv.ast.cast::<NullAwareElement>(n) {
        InferredCollectionElementTypeInformation {
            element_type: Some(ts.promote_to_non_null(rv.type_or_throw(rv.ast[e].value))),
            ..Default::default()
        }
    } else {
        // Dart throws `UnimplementedError`.
        InferredCollectionElementTypeInformation::default()
    }
}

/// Dart `_inferSetOrMapLiteralType`. Ends generic inference if it is in
/// progress. [context_type] is Dart `literal.contextType` (the type of the
/// literal from the downwards inference).
fn infer_set_or_map_literal_type(
    rv: &mut ResolverVisitor<'_>,
    inferrer: Option<GenericInferrer<'_, '_, '_>>,
    literal_resolution: LiteralResolution,
    literal: Id<SetOrMapLiteral>,
    context_type: Option<TypeId>,
) -> TypeId {
    let elements = rv.ast.list(rv.ast[literal].elements).to_vec();
    let mut inferred_types = Vec::new();
    let mut can_be_a_map = true;
    let mut must_be_a_map = false;
    let mut can_be_a_set = true;
    let mut must_be_a_set = false;
    for &element in &elements {
        let inferred_type = infer_collection_element_type(rv, element);
        can_be_a_map = can_be_a_map && inferred_type.can_be_map();
        must_be_a_map = must_be_a_map || inferred_type.must_be_map();
        can_be_a_set = can_be_a_set && inferred_type.can_be_set();
        must_be_a_set = must_be_a_set || inferred_type.must_be_set();
        inferred_types.push(inferred_type);
    }
    if literal_resolution.kind == LiteralResolutionKind::Map
        && literal_resolution.context_type.is_some()
    {
        return to_map_type(rv, inferrer, literal_resolution, literal, &inferred_types);
    }
    if can_be_a_set && must_be_a_set {
        return to_set_type(rv, inferrer, literal_resolution, literal, &inferred_types);
    } else if can_be_a_map && must_be_a_map {
        return to_map_type(rv, inferrer, literal_resolution, literal, &inferred_types);
    }

    // Note: according to the spec, the following computations should be
    // based on the greatest closure of the context type (unless the context
    // type is `_`). In practice, we can just use the context type directly,
    // because the only way the greatest closure of the context type could
    // possibly have a different subtype relationship to `Iterable<Object>`
    // and `Map<Object, Object>` is if the context type is `_`.
    if let Some(context_type) = context_type {
        let context_iterable_type = rv
            .ctx
            .as_instance_of(context_type, rv.ctx.tp.iterable_element().upcast());
        let context_map_type = rv
            .ctx
            .as_instance_of(context_type, rv.ctx.tp.map_element().upcast());
        let context_is_iterable = context_iterable_type.is_some();
        let context_is_map = context_map_type.is_some();

        // When `S` implements `Iterable` but not `Map`, `e` is a set literal.
        if context_is_iterable && !context_is_map {
            return to_set_type(rv, inferrer, literal_resolution, literal, &inferred_types);
        }

        // When `S` implements `Map` but not `Iterable`, `e` is a map literal.
        if context_is_map && !context_is_iterable {
            return to_map_type(rv, inferrer, literal_resolution, literal, &inferred_types);
        }
    }

    // When `e` is of the form `{}` and `S` is undefined, `e` is a map
    // literal.
    if elements.is_empty() && context_type.is_none() {
        return rv
            .ctx
            .tp
            .map_type(&rv.ctx, TypeId::DYNAMIC, TypeId::DYNAMIC);
    }

    // Ambiguous. We're not going to get any more information to resolve the
    // ambiguity. We don't want to make an arbitrary decision at this point
    // because it will interfere with future type inference (see
    // dartbug.com/36210), so we return a type of `dynamic`.
    let d = if must_be_a_map && must_be_a_set {
        diag::ambiguous_set_or_map_literal_both()
    } else {
        diag::ambiguous_set_or_map_literal_either()
    };
    let d = rv.at(d, literal);
    rv.report(d);
    TypeId::DYNAMIC
}

/// Dart `_resolveSetOrMapLiteral2`. Ends generic inference if [inferrer]
/// is not `None`.
fn resolve_set_or_map_literal2(
    rv: &mut ResolverVisitor<'_>,
    inferrer: Option<GenericInferrer<'_, '_, '_>>,
    literal_resolution: LiteralResolution,
    node: Id<SetOrMapLiteral>,
    literal_context_type: Option<TypeId>,
    context_type: TypeId,
) {
    // If we have type arguments, use them.
    if let Some(type_arguments) = literal_type_arguments(rv, rv.ast[node].type_arguments) {
        if type_arguments.len() == 1 {
            let element_type = type_arguments[0];
            let t = rv.ctx.tp.set_type(&rv.ctx, element_type);
            rv.record_static_type(node, t);
            return;
        } else if type_arguments.len() == 2 {
            let t = rv
                .ctx
                .tp
                .map_type(&rv.ctx, type_arguments[0], type_arguments[1]);
            rv.record_static_type(node, t);
            return;
        }
        // If we get here, then a nonsense number of type arguments were
        // provided, so treat it as though no type arguments were provided.
    }
    let literal_type =
        infer_set_or_map_literal_type(rv, inferrer, literal_resolution, node, literal_context_type);
    if rv.unit.options.strict_inference
        && rv.ast.list(rv.ast[node].elements).is_empty()
        && context_type == TypeId::UNKNOWN
    {
        // We cannot infer the type of a collection literal with no elements,
        // and no context type. If there are any elements, inference has not
        // failed, as the types of those elements are considered resolved.
        let is_map = rv
            .ctx
            .interface_element(literal_type)
            .is_some_and(|e| e == rv.ctx.tp.map_element().upcast());
        let d = rv.at(
            diag::inference_failure_on_collection_literal(if is_map { "Map" } else { "Set" }),
            node,
        );
        rv.report(d);
    }
    rv.record_static_type(node, literal_type);
}

/// Dart `_toMapType`. Ends generic inference if it is in progress.
fn to_map_type(
    rv: &mut ResolverVisitor<'_>,
    inferrer: Option<GenericInferrer<'_, '_, '_>>,
    literal_resolution: LiteralResolution,
    node: Id<SetOrMapLiteral>,
    inferred_types: &[InferredCollectionElementTypeInformation],
) -> TypeId {
    let element = rv.ctx.tp.map_element();
    let generic_key_type = generic_type(rv, element, 0);
    let generic_value_type = generic_type(rv, element, 1);

    let mut inferrer = match inferrer {
        Some(inferrer) if literal_resolution.kind != LiteralResolutionKind::Set => inferrer,
        _ => setup_literal_inference(rv, element, node.raw(), TypeId::UNKNOWN, None),
    };
    for inferred_type in inferred_types {
        inferrer.constrain_argument(
            inferred_type.key_type.unwrap_or(TypeId::DYNAMIC),
            generic_key_type,
            "key",
            Some(node.raw()),
        );
        inferrer.constrain_argument(
            inferred_type.value_type.unwrap_or(TypeId::DYNAMIC),
            generic_value_type,
            "value",
            Some(node.raw()),
        );
    }
    let type_arguments = inferrer.choose_final_types();
    rv.ctx
        .instantiate_interface(element.upcast(), &type_arguments, Nullability::None)
}

/// Dart `_toSetType`. Ends generic inference if it is in progress.
fn to_set_type(
    rv: &mut ResolverVisitor<'_>,
    inferrer: Option<GenericInferrer<'_, '_, '_>>,
    literal_resolution: LiteralResolution,
    node: Id<SetOrMapLiteral>,
    inferred_types: &[InferredCollectionElementTypeInformation],
) -> TypeId {
    let element = rv.ctx.tp.set_element();
    let generic_element_type = generic_type(rv, element, 0);

    let mut inferrer = match inferrer {
        Some(inferrer) if literal_resolution.kind != LiteralResolutionKind::Map => inferrer,
        _ => setup_literal_inference(rv, element, node.raw(), TypeId::UNKNOWN, None),
    };
    for inferred_type in inferred_types {
        inferrer.constrain_argument(
            inferred_type.element_type.unwrap_or(TypeId::DYNAMIC),
            generic_element_type,
            "element",
            Some(node.raw()),
        );
    }
    let type_arguments = inferrer.choose_final_types();
    rv.ctx
        .instantiate_interface(element.upcast(), &type_arguments, Nullability::None)
}

/// Dart `_InferredCollectionElementTypeInformation`.
#[derive(Clone, Copy, Debug, Default)]
struct InferredCollectionElementTypeInformation {
    element_type: Option<TypeId>,
    key_type: Option<TypeId>,
    value_type: Option<TypeId>,
}

impl InferredCollectionElementTypeInformation {
    /// The same type for the element, the key and the value.
    fn all(t: TypeId) -> Self {
        InferredCollectionElementTypeInformation {
            element_type: Some(t),
            key_type: Some(t),
            value_type: Some(t),
        }
    }

    /// Dart `_InferredCollectionElementTypeInformation.forIfElement`.
    fn for_if_element(rv: &ResolverVisitor<'_>, then_info: Self, else_info: Self) -> Self {
        let dynamic_or_null = |t: Option<TypeId>, dynamic: TypeId| t.map(|_| dynamic);
        if then_info.is_dynamic(rv) {
            let dynamic = then_info.element_type.expect("dynamic");
            return InferredCollectionElementTypeInformation {
                element_type: dynamic_or_null(else_info.element_type, dynamic),
                key_type: dynamic_or_null(else_info.key_type, dynamic),
                value_type: dynamic_or_null(else_info.value_type, dynamic),
            };
        } else if else_info.is_dynamic(rv) {
            let dynamic = else_info.element_type.expect("dynamic");
            return InferredCollectionElementTypeInformation {
                element_type: dynamic_or_null(then_info.element_type, dynamic),
                key_type: dynamic_or_null(then_info.key_type, dynamic),
                value_type: dynamic_or_null(then_info.value_type, dynamic),
            };
        }
        let lub = |first: Option<TypeId>, second: Option<TypeId>| match (first, second) {
            (None, second) => second,
            (first, None) => first,
            (Some(first), Some(second)) => Some(rv.type_system.least_upper_bound(first, second)),
        };
        InferredCollectionElementTypeInformation {
            element_type: lub(then_info.element_type, else_info.element_type),
            key_type: lub(then_info.key_type, else_info.key_type),
            value_type: lub(then_info.value_type, else_info.value_type),
        }
    }

    fn can_be_map(&self) -> bool {
        self.key_type.is_some() || self.value_type.is_some()
    }

    fn can_be_set(&self) -> bool {
        self.element_type.is_some()
    }

    fn is_dynamic(&self, rv: &ResolverVisitor<'_>) -> bool {
        let d = |t: Option<TypeId>| t.is_some_and(|t| is_dynamic(rv, t));
        d(self.element_type) && d(self.key_type) && d(self.value_type)
    }

    fn must_be_map(&self) -> bool {
        self.can_be_map() && self.element_type.is_none()
    }

    fn must_be_set(&self) -> bool {
        self.can_be_set() && self.key_type.is_none() && self.value_type.is_none()
    }
}

/// Dart `_LeafElements`: the counts of the kinds of leaf elements in a
/// collection, used to help disambiguate map and set literals.
struct LeafElements {
    /// The number of expressions found in the collection.
    expression_count: usize,
    /// The number of map entries found in the collection.
    map_entry_count: usize,
}

impl LeafElements {
    fn new(rv: &ResolverVisitor<'_>, elements: &[Id<CollectionElement>]) -> Self {
        let mut counts = LeafElements {
            expression_count: 0,
            map_entry_count: 0,
        };
        for &element in elements {
            counts.count(rv, Some(element));
        }
        counts
    }

    /// Dart `resolution`.
    fn resolution(&self) -> LiteralResolution {
        if self.expression_count > 0 && self.map_entry_count == 0 {
            LiteralResolution::new(LiteralResolutionKind::Set, None)
        } else if self.map_entry_count > 0 && self.expression_count == 0 {
            LiteralResolution::new(LiteralResolutionKind::Map, None)
        } else {
            LiteralResolution::new(LiteralResolutionKind::Ambiguous, None)
        }
    }

    /// Dart `_count`. Dart `_isComplete` always answers `true`.
    fn count(&mut self, rv: &ResolverVisitor<'_>, element: Option<Id<CollectionElement>>) {
        let Some(element) = element else {
            return;
        };
        let n = element.raw();
        if rv.ast.cast::<Expression>(n).is_some() {
            self.expression_count += 1;
        } else if let Some(e) = rv.ast.cast::<ForElement>(n) {
            self.count(rv, Some(rv.ast[e].body));
        } else if let Some(e) = rv.ast.cast::<IfElement>(n) {
            self.count(rv, Some(rv.ast[e].then_element));
            self.count(rv, rv.ast[e].else_element);
        } else if rv.ast.cast::<MapLiteralEntry>(n).is_some() {
            self.map_entry_count += 1;
        }
    }
}
