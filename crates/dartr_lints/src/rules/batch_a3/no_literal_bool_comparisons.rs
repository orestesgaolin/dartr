// Dart source: pkg/linter/lib/src/rules/no_literal_bool_comparisons.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_binary_expression("no_literal_bool_comparisons", check);
}

fn is_non_nullable_bool(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(resolved) = ctx.resolved else {
        return false;
    };
    let Some(ty) = ctx.static_type(node) else {
        return false;
    };
    resolved.ctx.is_dart_core_bool(ty)
        && ctx
            .type_system()
            .is_some_and(|type_system| type_system.is_non_nullable(ty))
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<BinaryExpression>::from_raw(node)];
    if !matches!(
        ctx.ast.tokens.lexeme(n.operator),
        "==" | "!=" | "|" | "||" | "&" | "&&" | "^"
    ) {
        return;
    }
    if ctx.ast.kind(n.right_operand) == NodeKind::BooleanLiteral
        && is_non_nullable_bool(ctx, n.left_operand.raw())
    {
        ctx.report_node(
            out,
            &diag::NO_LITERAL_BOOL_COMPARISONS,
            n.right_operand,
            &[],
        );
    } else if ctx.ast.kind(n.left_operand) == NodeKind::BooleanLiteral
        && is_non_nullable_bool(ctx, n.right_operand.raw())
    {
        ctx.report_node(out, &diag::NO_LITERAL_BOOL_COMPARISONS, n.left_operand, &[]);
    }
}
