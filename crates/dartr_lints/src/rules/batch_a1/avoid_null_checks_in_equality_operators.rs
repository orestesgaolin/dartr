// Dart source: pkg/linter/lib/src/rules/avoid_null_checks_in_equality_operators.dart

use super::helpers::{base_element, descendants, is_null_literal, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    BinaryExpression, Id, MethodDeclaration, MethodInvocation, NodeId, NodeKind, PropertyAccess,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{Nullability, TypeKind};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodDeclaration,
        "avoid_null_checks_in_equality_operators",
        check,
    );
}

fn is_parameter(c: &LinterContext<'_>, node: NodeId, parameter: dartr_element::ElementId) -> bool {
    c.element(node).and_then(|e| base_element(c, e)) == Some(parameter)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if lexeme(c, n.name) != "==" {
        return;
    }
    let Some(parameters) = n.parameters else {
        return;
    };
    let parameters = c.ast.list_raw(c.ast[parameters].parameters);
    if parameters.len() != 1 {
        return;
    }
    let Some(parameter) = c.declared_element(parameters[0]) else {
        return;
    };
    if super::helpers::declared_type(c, parameters[0]).is_some_and(|ty| match *r.ctx.ty(ty) {
        TypeKind::Interface { nullability, .. }
        | TypeKind::Never(nullability)
        | TypeKind::Record { nullability, .. }
        | TypeKind::TypeParameter { nullability, .. } => nullability != Nullability::Question,
        _ => false,
    }) {
        return;
    }
    for child in descendants(c.ast, n.body) {
        if let Some(binary) = c.ast.cast::<BinaryExpression>(child) {
            let binary = &c.ast[binary];
            let op = lexeme(c, binary.operator);
            let hit = (matches!(op, "==" | "!=")
                && ((is_null_literal(c.ast, binary.left_operand)
                    && is_parameter(c, binary.right_operand.raw(), parameter))
                    || (is_null_literal(c.ast, binary.right_operand)
                        && is_parameter(c, binary.left_operand.raw(), parameter))))
                || (op == "??" && is_parameter(c, binary.left_operand.raw(), parameter));
            if hit {
                c.report_node(
                    out,
                    &diag::AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS,
                    child,
                    &[],
                );
            }
        } else if let Some(invocation) = c.ast.cast::<MethodInvocation>(child) {
            let invocation = &c.ast[invocation];
            if invocation
                .operator
                .is_some_and(|operator| lexeme(c, operator) == "?.")
                && invocation
                    .target
                    .is_some_and(|target| is_parameter(c, target.raw(), parameter))
            {
                c.report_node(
                    out,
                    &diag::AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS,
                    child,
                    &[],
                );
            }
        } else if let Some(access) = c.ast.cast::<PropertyAccess>(child) {
            let access = &c.ast[access];
            if lexeme(c, access.operator) == "?."
                && access
                    .target
                    .is_some_and(|target| is_parameter(c, target.raw(), parameter))
            {
                c.report_node(
                    out,
                    &diag::AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS,
                    child,
                    &[],
                );
            }
        }
    }
}
