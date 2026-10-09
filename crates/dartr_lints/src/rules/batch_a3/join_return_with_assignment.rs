// Dart source: pkg/linter/lib/src/rules/join_return_with_assignment.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

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

/// Dart `canonicalElementsAreEqual` operand: `writeOrReadElement?.canonicalElement2`.
fn canonical_element(
    ctx: &LinterContext<'_>,
    expression: NodeId,
) -> Option<dartr_element::ElementId> {
    ctx.write_or_read_element(expression)
        .and_then(|e| ctx.canonical_element2(e))
}

/// `.element?.canonicalElement2` (the prefix of a prefixed identifier).
fn canonical_element_of_element(
    ctx: &LinterContext<'_>,
    expression: NodeId,
) -> Option<dartr_element::ElementId> {
    ctx.element(expression)
        .and_then(|e| ctx.canonical_element2(e))
}

fn same_target(ctx: &LinterContext<'_>, a: Option<NodeId>, b: Option<NodeId>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let a = unparenthesized(ctx, a);
    let b = unparenthesized(ctx, b);
    match (ctx.ast.kind(a), ctx.ast.kind(b)) {
        (NodeKind::SimpleIdentifier, NodeKind::SimpleIdentifier) => {
            canonical_element(ctx, a) == canonical_element(ctx, b)
        }
        (NodeKind::PrefixedIdentifier, NodeKind::PrefixedIdentifier) => {
            let a = &ctx.ast[Id::<PrefixedIdentifier>::from_raw(a)];
            let b = &ctx.ast[Id::<PrefixedIdentifier>::from_raw(b)];
            canonical_element_of_element(ctx, a.prefix.raw())
                == canonical_element_of_element(ctx, b.prefix.raw())
                && canonical_element(ctx, a.identifier.raw())
                    == canonical_element(ctx, b.identifier.raw())
        }
        (NodeKind::PropertyAccess, NodeKind::PropertyAccess) => {
            let a = &ctx.ast[Id::<PropertyAccess>::from_raw(a)];
            let b = &ctx.ast[Id::<PropertyAccess>::from_raw(b)];
            same_target(ctx, a.target.map(Id::raw), b.target.map(Id::raw))
                && canonical_element(ctx, a.property_name.raw())
                    == canonical_element(ctx, b.property_name.raw())
        }
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
