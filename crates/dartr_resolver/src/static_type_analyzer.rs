// Dart source: pkg/analyzer/lib/src/generated/static_type_analyzer.dart

//! `StaticTypeAnalyzer`: the static types of the simple expressions. Each
//! Dart method `visitX(node)` is a function `visit_x(rv, node)`.

use dartr_ast::{
    AdjacentStrings, AsExpression, BooleanLiteral, CascadeExpression, ConditionalExpression,
    DoubleLiteral, Expression, ExtensionDeclaration, Id, IntegerLiteral, IsExpression, NullLiteral,
    ParenthesizedExpression, RethrowExpression, SimpleStringLiteral, StringInterpolation,
    SuperExpression, SymbolLiteral, ThisExpression, ThrowExpression, TypeAnnotation,
};
use dartr_element::TypeId;
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeView;
use dartr_parser::experimental_flags::ExperimentalFlag;

use crate::resolver::ResolverVisitor;

pub fn visit_adjacent_strings(rv: &mut ResolverVisitor<'_>, node: Id<AdjacentStrings>) {
    let t = rv.ctx.tp.string_type();
    rv.record_static_type(node, t);
}

pub fn visit_as_expression(rv: &mut ResolverVisitor<'_>, node: Id<AsExpression>) {
    let t = get_type(rv, rv.ast[node].type_);
    rv.record_static_type(node, t);
}

pub fn visit_boolean_literal(rv: &mut ResolverVisitor<'_>, node: Id<BooleanLiteral>) {
    let t = rv.ctx.tp.bool_type();
    rv.record_static_type(node, t);
}

pub fn visit_cascade_expression(rv: &mut ResolverVisitor<'_>, node: Id<CascadeExpression>) {
    let t = rv.type_or_throw(rv.ast[node].target);
    rv.record_static_type(node, t);
}

pub fn visit_conditional_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ConditionalExpression>,
    context_type: TypeId,
) {
    let ts = rv.type_system;
    let t1 = rv.type_or_throw(rv.ast[node].then_expression);
    let t2 = rv.type_or_throw(rv.ast[node].else_expression);
    let t = ts.least_upper_bound(t1, t2);
    let s = ts.greatest_closure_of_schema(context_type);
    // Dart `_resolver.definingLibrary.featureSet`: the library features are
    // the features of its defining unit.
    #[allow(clippy::if_same_then_else)]
    let static_type = if !rv.is_enabled(ExperimentalFlag::InferenceUpdate3) {
        t
    } else if ts.is_subtype_of(t, s) {
        t
    } else if ts.is_subtype_of(t1, s) && ts.is_subtype_of(t2, s) {
        s
    } else {
        t
    };
    rv.record_static_type(node, static_type);
}

pub fn visit_double_literal(rv: &mut ResolverVisitor<'_>, node: Id<DoubleLiteral>) {
    let t = rv.ctx.tp.double_type();
    rv.record_static_type(node, t);
}

pub fn visit_integer_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<IntegerLiteral>,
    context_type: TypeId,
) {
    let ts = rv.type_system;
    let tp = rv.ctx.tp;
    let strict_casts = rv.unit.options.strict_casts;
    if ts.is_assignable_to(tp.int_type(), context_type, strict_casts)
        || !ts.is_assignable_to(tp.double_type(), context_type, strict_casts)
    {
        rv.record_static_type(node, tp.int_type());
    } else {
        rv.record_static_type(node, tp.double_type());
    }
}

pub fn visit_is_expression(rv: &mut ResolverVisitor<'_>, node: Id<IsExpression>) {
    let t = rv.ctx.tp.bool_type();
    rv.record_static_type(node, t);
}

pub fn visit_null_literal(rv: &mut ResolverVisitor<'_>, node: Id<NullLiteral>) {
    let t = rv.ctx.tp.null_type();
    rv.record_static_type(node, t);
}

pub fn visit_parenthesized_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ParenthesizedExpression>,
) {
    let t = rv.type_or_throw(rv.ast[node].expression);
    rv.record_static_type(node, t);
}

pub fn visit_rethrow_expression(rv: &mut ResolverVisitor<'_>, node: Id<RethrowExpression>) {
    let t = rv.ctx.tp.bottom_type();
    rv.record_static_type(node, t);
}

pub fn visit_simple_string_literal(rv: &mut ResolverVisitor<'_>, node: Id<SimpleStringLiteral>) {
    let t = rv.ctx.tp.string_type();
    rv.record_static_type(node, t);
}

pub fn visit_string_interpolation(rv: &mut ResolverVisitor<'_>, node: Id<StringInterpolation>) {
    let t = rv.ctx.tp.string_type();
    rv.record_static_type(node, t);
}

#[allow(clippy::unnecessary_unwrap)] // Keeps the condition of the Dart code.
pub fn visit_super_expression(rv: &mut ResolverVisitor<'_>, node: Id<SuperExpression>) {
    let this_type = rv.this_type();
    let info = rv.flow_analysis.flow.as_mut().map(|flow| {
        flow.this_or_super(
            SharedTypeView::new(this_type.unwrap_or(TypeId::DYNAMIC)),
            true,
        )
    });
    rv.flow_analysis
        .store_expression_info(node.upcast::<Expression>(), info);
    match this_type {
        Some(this_type)
            if rv
                .ast
                .this_or_ancestor_of_type::<ExtensionDeclaration>(node)
                .is_none() =>
        {
            rv.record_static_type(node, this_type);
        }
        _ => rv.record_static_type(node, TypeId::INVALID),
    }
}

pub fn visit_symbol_literal(rv: &mut ResolverVisitor<'_>, node: Id<SymbolLiteral>) {
    let t = rv.ctx.tp.symbol_type();
    rv.record_static_type(node, t);
}

pub fn visit_this_expression(rv: &mut ResolverVisitor<'_>, node: Id<ThisExpression>) {
    let static_type = rv.effective_this_type().unwrap_or(TypeId::DYNAMIC);
    let info = rv
        .flow_analysis
        .flow
        .as_mut()
        .map(|flow| flow.this_or_super(SharedTypeView::new(static_type), false));
    rv.flow_analysis
        .store_expression_info(node.upcast::<Expression>(), info);
    rv.record_static_type(node, static_type);
}

pub fn visit_throw_expression(rv: &mut ResolverVisitor<'_>, node: Id<ThrowExpression>) {
    let t = rv.ctx.tp.bottom_type();
    rv.record_static_type(node, t);
}

/// Dart `_getType`: the type of the annotation, `dynamic` if it is not
/// resolved.
fn get_type(rv: &ResolverVisitor<'_>, annotation: Id<TypeAnnotation>) -> TypeId {
    rv.tables
        .annotation_type
        .get(annotation)
        .copied()
        .unwrap_or(TypeId::DYNAMIC)
}
