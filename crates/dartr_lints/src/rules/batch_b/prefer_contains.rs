// Dart source: pkg/linter/lib/src/rules/prefer_contains.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::BinaryExpression, "prefer_contains", check);
}

/// Dart `TokenTypeExtension.inverted`.
fn inverted(operator: &str) -> &str {
    match operator {
        "<=" => ">=",
        "<" => ">",
        ">" => "<",
        ">=" => "<=",
        other => other,
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    let operator = lexeme(c, n.operator);
    if !matches!(operator, "==" | "!=" | ">" | ">=" | "<" | "<=") {
        return;
    }
    if let Some(value) = get_int_value(c, n.right_operand.raw()) {
        if value <= 0 && is_unassigned_index_of(c, n.left_operand.raw()) {
            check_constant(c, node, value, operator, out);
        }
    } else if let Some(value) = get_int_value(c, n.left_operand.raw())
        && value <= 0
        && is_unassigned_index_of(c, n.right_operand.raw())
    {
        check_constant(c, node, value, inverted(operator), out);
    }
}

/// Dart `_checkConstant`.
fn check_constant(c: &LinterContext<'_>, node: NodeId, value: i64, operator: &str, out: &mut Vec<Diagnostic>) {
    let code = if value == -1 {
        match operator {
            "==" | "!=" | "<=" | ">" => Some(&diag::PREFER_CONTAINS_USE_CONTAINS),
            "<" => Some(&diag::PREFER_CONTAINS_ALWAYS_FALSE),
            ">=" => Some(&diag::PREFER_CONTAINS_ALWAYS_TRUE),
            _ => None,
        }
    } else if value == 0 {
        matches!(operator, ">=" | "<").then_some(&diag::PREFER_CONTAINS_USE_CONTAINS)
    } else if value < -1 {
        match operator {
            "==" | "<=" | "<" => Some(&diag::PREFER_CONTAINS_ALWAYS_FALSE),
            "!=" | ">=" | ">" => Some(&diag::PREFER_CONTAINS_ALWAYS_TRUE),
            _ => None,
        }
    } else {
        None
    };
    if let Some(code) = code {
        c.report_node(out, code, node, &[]);
    }
}

/// Dart `_isUnassignedIndexOf`.
fn is_unassigned_index_of(c: &LinterContext<'_>, expression: NodeId) -> bool {
    let mut invocation = unparenthesized(c, expression);
    while let Some(a) = c.ast.cast::<AsExpression>(invocation) {
        invocation = c.ast[a].expression.raw();
    }
    let invocation = unparenthesized(c, invocation);
    let Some(m) = c.ast.cast::<MethodInvocation>(invocation) else {
        return false;
    };
    if c.ast.parent(invocation).is_some_and(|p| kind(c, p) == NodeKind::AssignmentExpression) {
        return false;
    }
    let n = &c.ast[m];
    if simple_name(c, n.method_name) != "indexOf" {
        return false;
    }
    let Some(parent_type) = n.target.and_then(|t| c.static_type(t)) else {
        return false;
    };
    if !implements_any_interface(c, Some(parent_type), &[("Iterable", "dart.core"), ("String", "dart.core")]) {
        return false;
    }
    let args = arguments(c, n.argument_list);
    if args.len() == 2 {
        let start = argument_expression(c, args[1]);
        if get_int_value(c, start) != Some(0) {
            return false;
        }
    }
    true
}
