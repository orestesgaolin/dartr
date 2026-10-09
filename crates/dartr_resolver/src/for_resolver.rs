// Dart source: pkg/analyzer/lib/src/dart/resolver/for_resolver.dart

//! `ForResolver`: resolves [ForStatement]s and [ForElement]s (C-style for
//! loops, for-in loops with a declared variable, an identifier or a
//! pattern, and `await for`).

use dartr_ast::{
    CollectionElement, DartPattern, DeclaredIdentifier, Expression, ForEachPartsWithDeclaration,
    ForEachPartsWithIdentifier, ForEachPartsWithPattern, ForElement, ForLoopParts,
    ForPartsWithDeclarations, ForPartsWithExpression, ForPartsWithPattern, ForStatement, Id,
    NodeId, NodeList, SimpleIdentifier, Statement,
};
use dartr_diagnostics::diag;
use dartr_element::{
    ElemRef, ElementId, LocalVariableElement, PromotableElement, TypeId, TypeKind, VariableElement,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::{TypeExt, member};

use crate::element_ext;
use crate::resolver::CollectionLiteralContext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitForStatement(node)` (after
/// `checkUnreachableNode`, which the core calls) =
/// `ForResolver.resolveStatement(node)`.
pub fn visit_for_statement(rv: &mut ResolverVisitor<'_>, node: Id<ForStatement>) {
    resolve_statement(rv, node);
    // Dart `nullSafetyDeadCodeVerifier.flowEnd(node.body)`: wave D.
}

/// Dart `ResolverVisitor.visitForElement(node, context: context)` =
/// `ForResolver.resolveElement(node, context)`.
pub fn visit_for_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ForElement>,
    context: Option<CollectionLiteralContext>,
) {
    resolve_element(rv, node, context);
}

/// The loop parts of a for statement or element, by kind.
enum Parts {
    /// `ForPartsImpl`.
    For(ForParts),
    /// `ForEachPartsWithPatternImpl`.
    EachWithPattern(Id<ForEachPartsWithPattern>),
    /// `ForEachPartsImpl` (with a declaration or an identifier).
    Each(EachParts),
}

/// The fields of `ForPartsImpl`.
struct ForParts {
    /// `ForPartsWithDeclarations.variables`, `ForPartsWithPattern.variables`.
    variables: Option<NodeId>,
    /// `ForPartsWithExpression.initialization`.
    initialization: Option<Id<Expression>>,
    /// The node of the loop parts.
    node: NodeId,
}

/// The fields of `ForEachPartsImpl`.
struct EachParts {
    node: NodeId,
    loop_variable: Option<Id<DeclaredIdentifier>>,
    identifier: Option<Id<SimpleIdentifier>>,
}

fn classify(rv: &ResolverVisitor<'_>, parts: Id<ForLoopParts>) -> Option<Parts> {
    let n = parts.raw();
    let ast = &*rv.ast;
    if let Some(p) = ast.cast::<ForPartsWithDeclarations>(n) {
        Some(Parts::For(ForParts {
            variables: Some(ast[p].variables.raw()),
            initialization: None,
            node: n,
        }))
    } else if let Some(p) = ast.cast::<ForPartsWithExpression>(n) {
        Some(Parts::For(ForParts {
            variables: None,
            initialization: ast[p].initialization,
            node: n,
        }))
    } else if let Some(p) = ast.cast::<ForPartsWithPattern>(n) {
        Some(Parts::For(ForParts {
            variables: Some(ast[p].variables.raw()),
            initialization: None,
            node: n,
        }))
    } else if let Some(p) = ast.cast::<ForEachPartsWithPattern>(n) {
        Some(Parts::EachWithPattern(p))
    } else if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(n) {
        Some(Parts::Each(EachParts {
            node: n,
            loop_variable: Some(ast[p].loop_variable),
            identifier: None,
        }))
    } else {
        ast.cast::<ForEachPartsWithIdentifier>(n).map(|p| {
            Parts::Each(EachParts {
                node: n,
                loop_variable: None,
                identifier: Some(ast[p].identifier),
            })
        })
    }
}

/// Dart `resolveElement`.
fn resolve_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ForElement>,
    context: Option<CollectionLiteralContext>,
) {
    let for_loop_parts = rv.ast[node].for_loop_parts;
    let is_async = rv.ast[node].await_keyword.is_some();
    let mut visit_body = |rv: &mut ResolverVisitor<'_>| {
        let body: Id<CollectionElement> = rv.ast[node].body;
        rv.dispatch_collection_element(body.raw(), context);
    };
    match classify(rv, for_loop_parts) {
        Some(Parts::For(parts)) => for_parts(rv, node.raw(), parts, &mut visit_body),
        Some(Parts::EachWithPattern(parts)) => {
            analyze_pattern_for_in(rv, node.raw(), is_async, parts, &mut visit_body)
        }
        Some(Parts::Each(parts)) => {
            for_each_parts(rv, node.raw(), is_async, parts, &mut visit_body)
        }
        None => {}
    }
}

/// Dart `resolveStatement`.
fn resolve_statement(rv: &mut ResolverVisitor<'_>, node: Id<ForStatement>) {
    let for_loop_parts = rv.ast[node].for_loop_parts;
    let is_async = rv.ast[node].await_keyword.is_some();
    match classify(rv, for_loop_parts) {
        Some(Parts::For(parts)) => {
            let mut visit_body = |rv: &mut ResolverVisitor<'_>| {
                let body = rv.ast[node].body;
                rv.visit_node(body.raw());
            };
            for_parts(rv, node.raw(), parts, &mut visit_body);
        }
        Some(Parts::EachWithPattern(parts)) => {
            let mut dispatch_body = |rv: &mut ResolverVisitor<'_>| {
                let body: Id<Statement> = rv.ast[node].body;
                rv.dispatch_statement(body);
            };
            analyze_pattern_for_in(rv, node.raw(), is_async, parts, &mut dispatch_body);
        }
        Some(Parts::Each(parts)) => {
            let mut visit_body = |rv: &mut ResolverVisitor<'_>| {
                let body = rv.ast[node].body;
                rv.visit_node(body.raw());
            };
            for_each_parts(rv, node.raw(), is_async, parts, &mut visit_body);
        }
        None => {}
    }
}

/// Dart `_analyzePatternForIn`.
fn analyze_pattern_for_in(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    has_await: bool,
    for_loop_parts: Id<ForEachPartsWithPattern>,
    dispatch_body: &mut dyn FnMut(&mut ResolverVisitor<'_>),
) {
    let metadata = rv.ast[for_loop_parts].metadata;
    rv.visit_list(metadata);
    let pattern: Id<DartPattern> = rv.ast[for_loop_parts].pattern;
    let iterable = rv.ast[for_loop_parts].iterable;
    rv.analyze_pattern_for_in(node, has_await, pattern, iterable, &mut |rv| {
        dispatch_body(rv)
    });
    rv.pop_rewrite();
    let iterable = rv.ast[for_loop_parts].iterable;
    rv.nullable_dereference_expression(
        diag::unchecked_use_of_nullable_value_as_iterator(),
        iterable,
        None,
    );
}

/// Dart `_computeForEachElementType`: the type of the elements of
/// [iterable], from the type of the iterable or stream that the for-each
/// loop iterates over.
fn compute_for_each_element_type(
    rv: &ResolverVisitor<'_>,
    iterable: Id<Expression>,
    is_async: bool,
) -> TypeId {
    let Some(iterable_type) = rv.static_type(iterable) else {
        return TypeId::INVALID;
    };

    let iterable_type = rv.type_system.resolve_to_bound(iterable_type);
    if matches!(rv.ctx.ty(iterable_type), TypeKind::Dynamic) {
        return TypeId::DYNAMIC;
    }

    let iterated_element = if is_async {
        rv.ctx.tp.stream_element()
    } else {
        rv.ctx.tp.iterable_element()
    };

    let Some(iterated_type) = rv
        .ctx
        .as_instance_of(iterable_type, iterated_element.upcast())
    else {
        return TypeId::INVALID;
    };

    match rv.ctx.type_arguments(iterated_type) {
        [single] => *single,
        _ => TypeId::INVALID,
    }
}

/// Dart `loopVariable.declaredFragment?.element`.
fn declared_element(rv: &ResolverVisitor<'_>, node: Id<DeclaredIdentifier>) -> Option<ElementId> {
    let fragment = *rv.tables.declared_fragment.get(node)?;
    rv.ctx.fragment_data(fragment)?.element.try_get().copied()
}

/// Dart `_forEachParts`.
fn for_each_parts(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    is_async: bool,
    for_each_parts: EachParts,
    visit_body: &mut dyn FnMut(&mut ResolverVisitor<'_>),
) {
    let iterable_of = |rv: &ResolverVisitor<'_>| -> Id<Expression> {
        let n = for_each_parts.node;
        if let Some(p) = rv.ast.cast::<ForEachPartsWithDeclaration>(n) {
            rv.ast[p].iterable
        } else {
            let p = rv
                .ast
                .cast::<ForEachPartsWithIdentifier>(n)
                .expect("for-each parts");
            rv.ast[p].iterable
        }
    };
    let loop_variable = for_each_parts.loop_variable;
    let mut identifier_element: Option<ElemRef> = None;
    if let Some(identifier) = for_each_parts.identifier {
        // TODO(scheglov): replace with lexical lookup
        rv.visit_node(identifier.raw());
        // Dart `AssignmentExpressionShared.checkFinalAlreadyAssigned(
        // identifier, isForEachIdentifier: true)`: unit C5.
    }
    // The identifier may have been rewritten (it is not, in practice).
    let identifier = for_each_parts.identifier.and_then(|_| {
        rv.ast
            .cast::<ForEachPartsWithIdentifier>(for_each_parts.node)
            .map(|p| rv.ast[p].identifier)
    });

    let mut value_type = None;
    if let Some(loop_variable) = loop_variable {
        value_type = Some(
            rv.ast[loop_variable]
                .type_
                .and_then(|t| rv.tables.annotation_type.get(t).copied())
                .unwrap_or(TypeId::UNKNOWN),
        );
    }
    if let Some(identifier) = identifier {
        identifier_element = rv.element(identifier);
        if let Some(e) = identifier_element {
            let base = member::base_element(&rv.ctx, e);
            if base.cast::<VariableElement>().is_some() {
                let ctx = rv.ctx;
                value_type = Some(rv.flow_analysis.local_variable_type(
                    &ctx,
                    identifier.upcast(),
                    base,
                    false,
                ));
            } else if member::is_setter(&rv.ctx, e) {
                let parameters = member::formal_parameters(&rv.ctx, e);
                if let Some(&parameter) = parameters.first() {
                    value_type = Some(member::type_(&rv.ctx, parameter));
                }
            }
        }
    }
    let target_type = value_type.map(|value_type| {
        if is_async {
            rv.ctx.tp.stream_type(&rv.ctx, value_type)
        } else {
            rv.ctx.tp.iterable_type(&rv.ctx, value_type)
        }
    });

    let iterable = iterable_of(rv);
    rv.analyze_expression_node(iterable, target_type.unwrap_or(TypeId::UNKNOWN));
    let iterable = rv.pop_rewrite().unwrap_or_else(|| iterable_of(rv));

    rv.nullable_dereference_expression(
        diag::unchecked_use_of_nullable_value_as_iterator(),
        iterable,
        None,
    );

    if let Some(loop_variable) = loop_variable {
        rv.visit_node(loop_variable.raw());
    }
    let element_type = compute_for_each_element_type(rv, iterable, is_async);
    if let Some(loop_variable) = loop_variable
        && rv.ast[loop_variable].type_.is_none()
        && let Some(e) = declared_element(rv, loop_variable)
        && let Some(e) = e.cast::<LocalVariableElement>()
    {
        element_ext::set_local_variable_type(&rv.ctx, e, element_type);
    }

    if let Some(loop_variable) = loop_variable
        && let Some(declared_element) = declared_element(rv, loop_variable)
        && let Some(promotable) = declared_element.cast::<PromotableElement>()
    {
        let declared_type = element_ext::variable_type(&rv.ctx, declared_element);
        if let Some(flow) = rv.flow_analysis.flow.as_mut() {
            flow.declare(promotable, SharedTypeView::new(declared_type), true);
        }
    }

    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.for_each_body_begin(node);
    }
    if identifier.is_some()
        && let Some(ElemRef::Base(e)) = identifier_element
        && let Some(promotable) = e.cast::<PromotableElement>()
    {
        let ast = &*rv.ast;
        rv.flow_analysis
            .write(ast, for_each_parts.node, promotable, element_type, None);
    }

    visit_body(rv);

    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.for_each_end();
    }
}

/// Dart `_forParts`.
fn for_parts(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    for_parts: ForParts,
    visit_body: &mut dyn FnMut(&mut ResolverVisitor<'_>),
) {
    if let Some(variables) = for_parts.variables {
        rv.visit_node(variables);
    } else if let Some(initialization) = for_parts.initialization {
        rv.analyze_expression_node(initialization, TypeId::UNKNOWN);
        rv.pop_rewrite();
    }

    rv.flow_analysis.for_condition_begin(node);

    let (condition, _) = for_parts_condition_and_updaters(rv, for_parts.node);
    let mut condition = condition;
    if let Some(c) = condition {
        let bool_type = rv.ctx.tp.bool_type();
        rv.analyze_expression_node(c, bool_type);
        let c = rv.pop_rewrite().unwrap_or(c);
        rv.check_for_non_bool_condition(c);
        condition = Some(c);
    }

    // Dart `nullSafetyDeadCodeVerifier.for_conditionEnd()`: wave D.
    {
        let ast = &*rv.ast;
        rv.flow_analysis.for_body_begin(ast, node, condition);
    }
    visit_body(rv);

    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.for_updater_begin();
    }
    // Dart `nullSafetyDeadCodeVerifier.for_updaterBegin(...)`: wave D.
    let (_, updaters) = for_parts_condition_and_updaters(rv, for_parts.node);
    let updaters = rv.ast.list(updaters).to_vec();
    for updater in updaters {
        rv.analyze_expression_node(updater, TypeId::UNKNOWN);
        rv.pop_rewrite();
    }

    if let Some(flow) = rv.flow_analysis.flow.as_mut() {
        flow.for_end();
    }
}

/// `ForParts.condition` and `ForParts.updaters` of [parts].
fn for_parts_condition_and_updaters(
    rv: &ResolverVisitor<'_>,
    parts: NodeId,
) -> (Option<Id<Expression>>, NodeList<Expression>) {
    let ast = &*rv.ast;
    if let Some(p) = ast.cast::<ForPartsWithDeclarations>(parts) {
        (ast[p].condition, ast[p].updaters)
    } else if let Some(p) = ast.cast::<ForPartsWithExpression>(parts) {
        (ast[p].condition, ast[p].updaters)
    } else {
        let p = ast.cast::<ForPartsWithPattern>(parts).expect("for parts");
        (ast[p].condition, ast[p].updaters)
    }
}
