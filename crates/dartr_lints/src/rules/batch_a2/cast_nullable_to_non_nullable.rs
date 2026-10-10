// Dart source: pkg/linter/lib/src/rules/cast_nullable_to_non_nullable.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::AsExpression,
        "cast_nullable_to_non_nullable",
        check,
    );
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<AsExpression>(node).unwrap()];
    let (Some(ts), Some(expression_type), Some(cast_type)) = (
        context.type_system(),
        context.static_type(n.expression),
        super::helpers::annotation_type(context, n.type_.raw()),
    ) else {
        return;
    };
    if !super::helpers::is_dynamic(context, expression_type)
        && !super::helpers::is_invalid(context, expression_type)
        && ts.is_nullable(expression_type)
        && ts.is_non_nullable(cast_type)
    {
        context.report_node(out, &diag::CAST_NULLABLE_TO_NON_NULLABLE, node, &[]);
    }
}
