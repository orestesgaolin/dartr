// Dart source: pkg/analyzer/lib/src/dart/resolver/list_pattern_resolver.dart

//! `ListPatternResolver`: resolves a list pattern (the type argument, the
//! shared analysis, the required type).

use dartr_ast::{Id, ListPattern};
use dartr_diagnostics::diag;
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzer;

use crate::pattern_resolver::{check_pattern_never_matches_value_type, set_required_type, type_argument_types};
use crate::resolver::{PatternResultOf, ResolverVisitor, SharedMatchContext};

/// Dart `ListPatternResolver.resolve(node:, context:)`.
pub fn resolve(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ListPattern>,
    context: &SharedMatchContext,
) -> PatternResultOf {
    let type_arguments = rv.ast[node].type_arguments;
    if let Some(type_arguments) = type_arguments {
        rv.visit_node(type_arguments.raw());
        // Check that we have exactly one type argument.
        let length = rv.ast.list_raw(rv.ast[type_arguments].arguments).len();
        if length != 1 {
            let d = rv.at(
                diag::expected_one_list_pattern_type_arguments(length as i64),
                type_arguments,
            );
            rv.report(d);
        }
    }

    // Dart `typeArguments?.arguments.first.typeOrThrow`.
    let element_type = type_arguments
        .and_then(|t| type_argument_types(rv, t).first().copied())
        .map(SharedTypeView::new);
    let elements = rv.ast.list_raw(rv.ast[node].elements).to_vec();
    let result = rv.analyze_list_pattern(context, node.upcast(), element_type, &elements);
    let required_type = result.required_type.unwrap_type_view();
    set_required_type(rv, node.raw(), required_type);

    check_pattern_never_matches_value_type(
        rv,
        context,
        node.upcast(),
        required_type,
        result.matched_value_type.unwrap_type_view(),
    );

    result.into()
}
