// Dart source: pkg/linter/lib/src/rules/join_return_with_assignment.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_block("join_return_with_assignment", check);
}

fn unparenthesized(ctx: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(p) = ctx.ast.cast::<ParenthesizedExpression>(node) {
        node = ctx.ast[p].expression.raw();
    }
    node
}

fn assignment_target(ctx: &LinterContext<'_>, statement: NodeId) -> Option<NodeId> {
    let statement = ctx.ast.cast::<ExpressionStatement>(statement)?;
    let expression = unparenthesized(ctx, ctx.ast[statement].expression.raw());
    match ctx.ast.kind(expression) {
        NodeKind::AssignmentExpression => Some(
            ctx.ast[Id::<AssignmentExpression>::from_raw(expression)]
                .left_hand_side
                .raw(),
        ),
        NodeKind::PostfixExpression => Some(
            ctx.ast[Id::<PostfixExpression>::from_raw(expression)]
                .operand
                .raw(),
        ),
        NodeKind::PrefixExpression => Some(
            ctx.ast[Id::<PrefixExpression>::from_raw(expression)]
                .operand
                .raw(),
        ),
        _ => None,
    }
}

fn return_expression(ctx: &LinterContext<'_>, statement: NodeId) -> Option<NodeId> {
    ctx.ast
        .cast::<ReturnStatement>(statement)
        .and_then(|r| ctx.ast[r].expression.map(Id::raw))
}

fn canonical_element(
    ctx: &LinterContext<'_>,
    expression: NodeId,
) -> Option<dartr_element::ElementId> {
    let mut expression = unparenthesized(ctx, expression);
    expression = match ctx.ast.kind(expression) {
        NodeKind::PropertyAccess => ctx.ast[Id::<PropertyAccess>::from_raw(expression)]
            .property_name
            .raw(),
        NodeKind::PrefixedIdentifier => ctx.ast[Id::<PrefixedIdentifier>::from_raw(expression)]
            .identifier
            .raw(),
        _ => expression,
    };
    let resolved = ctx.resolved?;
    ctx.element(expression)
        .map(|e| member::base_element(&resolved.ctx, e))
}

fn same_target(ctx: &LinterContext<'_>, a: Option<NodeId>, b: Option<NodeId>) -> bool {
    match (
        a.and_then(|n| canonical_element(ctx, n)),
        b.and_then(|n| canonical_element(ctx, n)),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let statements = ctx
        .ast
        .list(ctx.ast[Id::<Block>::from_raw(node)].statements);
    if statements.len() < 2 {
        return;
    }
    let Some(last) = return_expression(ctx, statements[statements.len() - 1].raw()) else {
        return;
    };
    let second_statement = statements[statements.len() - 2].raw();
    let Some(second) = assignment_target(ctx, second_statement) else {
        return;
    };
    let third = statements
        .get(statements.len().wrapping_sub(3))
        .and_then(|s| assignment_target(ctx, s.raw()));
    if !same_target(ctx, Some(second), third) && same_target(ctx, Some(last), Some(second)) {
        ctx.report_node(
            out,
            &diag::JOIN_RETURN_WITH_ASSIGNMENT,
            second_statement,
            &[],
        );
    }
}
