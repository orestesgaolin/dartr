// Dart source: pkg/linter/lib/src/rules/use_if_null_to_convert_nulls_to_bools.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::BinaryExpression,
        "use_if_null_to_convert_nulls_to_bools",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(ts)) = (rctx(c), c.type_system()) else {
        return;
    };
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    let is_nullable_bool = c
        .static_type(n.left_operand)
        .is_some_and(|t| ctx.is_dart_core_bool(t) && ts.is_nullable(t));
    let right = c.ast.cast::<BooleanLiteral>(n.right_operand.raw()).map(|b| c.ast[b].value);
    let operator = lexeme(c, n.operator);
    if operator == "==" && is_nullable_bool && right == Some(true) {
        c.report_node(out, &diag::USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS, node, &[]);
    }
    if operator == "!=" && is_nullable_bool && right == Some(false) {
        c.report_node(out, &diag::USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS, node, &[]);
    }
}
