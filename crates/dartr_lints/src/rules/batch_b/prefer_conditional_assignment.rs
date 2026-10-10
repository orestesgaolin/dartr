// Dart source: pkg/linter/lib/src/rules/prefer_conditional_assignment.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::IfStatement,
        "prefer_conditional_assignment",
        check,
    );
}

fn check_statement(c: &LinterContext<'_>, statement: NodeId, condition: NodeId) -> bool {
    if let Some(s) = c.ast.cast::<ExpressionStatement>(statement) {
        let expression = c.ast[s].expression.raw();
        return c
            .ast
            .cast::<AssignmentExpression>(expression)
            .is_some_and(|a| {
                canonical_elements_from_identifiers_are_equal(
                    c,
                    Some(c.ast[a].left_hand_side.raw()),
                    Some(condition),
                )
            });
    }
    if let Some(block) = c.ast.cast::<Block>(statement) {
        let statements = c.ast.list_raw(c.ast[block].statements);
        if statements.len() == 1 {
            return check_statement(c, statements[0], condition);
        }
    }
    false
}

fn expression_condition(c: &LinterContext<'_>, expression: NodeId) -> Option<NodeId> {
    let expression = unparenthesized(c, expression);
    let binary = c.ast.cast::<BinaryExpression>(expression)?;
    let b = &c.ast[binary];
    if lexeme(c, b.operator) != "==" {
        return None;
    }
    if is_null_literal(c, b.right_operand.raw()) {
        return Some(b.left_operand.raw());
    }
    if is_null_literal(c, b.left_operand.raw()) {
        return Some(b.right_operand.raw());
    }
    None
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<IfStatement>::from_raw(node)];
    if n.else_statement.is_some() {
        return;
    }
    if let Some(condition) = expression_condition(c, n.expression.raw())
        && check_statement(c, n.then_statement.raw(), condition)
    {
        c.report_node(out, &diag::PREFER_CONDITIONAL_ASSIGNMENT, node, &[]);
    }
}
