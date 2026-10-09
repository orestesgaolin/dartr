// Dart source: pkg/linter/lib/src/rules/avoid_dynamic_calls.dart

use super::helpers::{is_dynamic, lexeme, unparenthesized};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::AssignmentExpression,
        NodeKind::BinaryExpression,
        NodeKind::FunctionExpressionInvocation,
        NodeKind::IndexExpression,
        NodeKind::MethodInvocation,
        NodeKind::PostfixExpression,
        NodeKind::PrefixExpression,
        NodeKind::PrefixedIdentifier,
        NodeKind::PropertyAccess,
    ] {
        registry.add(kind, "avoid_dynamic_calls", check);
    }
}

fn explicit_cast(c: &LinterContext<'_>, expression: Id<Expression>) -> bool {
    c.ast.kind(unparenthesized(c.ast, expression)) == NodeKind::AsExpression
}
fn report(c: &LinterContext<'_>, expression: Id<Expression>, out: &mut Vec<Diagnostic>) -> bool {
    if is_dynamic(c, expression) && !explicit_cast(c, expression) {
        c.report_node(out, &diag::AVOID_DYNAMIC_CALLS, expression, &[]);
        true
    } else {
        false
    }
}

fn report_dynamic_or_function(
    c: &LinterContext<'_>,
    expression: Id<Expression>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(r) = c.resolved else { return };
    let Some(ty) = c.static_type(expression) else {
        return;
    };
    if (ty == dartr_element::TypeId::DYNAMIC || r.ctx.is_dart_core_function(ty))
        && !explicit_cast(c, expression)
    {
        c.report_node(out, &diag::AVOID_DYNAMIC_CALLS, expression, &[]);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::AssignmentExpression => {
            let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
            if lexeme(c, n.operator) != "??="
                && c.resolved
                    .and_then(|r| r.tables.read_type.get(node))
                    .is_some_and(|&t| t == dartr_element::TypeId::DYNAMIC)
            {
                c.report_node(out, &diag::AVOID_DYNAMIC_CALLS, node, &[]);
            }
        }
        NodeKind::BinaryExpression => {
            let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
            if !matches!(lexeme(c, n.operator), "==" | "!=" | "&&" | "||" | "??") {
                report(c, n.left_operand, out);
            }
        }
        NodeKind::FunctionExpressionInvocation => {
            report_dynamic_or_function(
                c,
                c.ast[Id::<FunctionExpressionInvocation>::from_raw(node)].function,
                out,
            );
        }
        NodeKind::IndexExpression => {
            if let Some(target) = c.ast[Id::<IndexExpression>::from_raw(node)].target {
                report(c, target, out);
            }
        }
        NodeKind::MethodInvocation => {
            let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
            let name = lexeme(c, c.ast[n.method_name].token);
            if n.target.is_some()
                && ((name == "toString"
                    && c.ast.list_raw(c.ast[n.argument_list].arguments).is_empty())
                    || (name == "noSuchMethod"
                        && c.ast.list_raw(c.ast[n.argument_list].arguments).len() == 1))
            {
                return;
            }
            if let Some(target) = n.target {
                report(c, target, out);
            }
        }
        NodeKind::PostfixExpression => {
            let n = &c.ast[Id::<PostfixExpression>::from_raw(node)];
            if lexeme(c, n.operator) != "!"
                && !report(c, n.operand, out)
                && c.resolved
                    .and_then(|r| r.tables.read_type.get(node))
                    .is_some_and(|&t| t == dartr_element::TypeId::DYNAMIC)
            {
                c.report_node(out, &diag::AVOID_DYNAMIC_CALLS, node, &[]);
            }
        }
        NodeKind::PrefixExpression => {
            let operand = c.ast[Id::<PrefixExpression>::from_raw(node)].operand;
            if !report(c, operand, out)
                && c.resolved
                    .and_then(|r| r.tables.read_type.get(node))
                    .is_some_and(|&t| t == dartr_element::TypeId::DYNAMIC)
            {
                c.report_node(out, &diag::AVOID_DYNAMIC_CALLS, node, &[]);
            }
        }
        NodeKind::PrefixedIdentifier => {
            let n = &c.ast[Id::<PrefixedIdentifier>::from_raw(node)];
            if !matches!(
                lexeme(c, c.ast[n.identifier].token),
                "hashCode" | "noSuchMethod" | "runtimeType" | "toString"
            ) {
                report(c, n.prefix.upcast(), out);
            }
        }
        NodeKind::PropertyAccess => {
            let n = &c.ast[Id::<PropertyAccess>::from_raw(node)];
            if !matches!(
                lexeme(c, c.ast[n.property_name].token),
                "hashCode" | "noSuchMethod" | "runtimeType" | "toString"
            ) && let Some(target) = n.target
            {
                report(c, target, out);
            }
        }
        _ => {}
    }
}
