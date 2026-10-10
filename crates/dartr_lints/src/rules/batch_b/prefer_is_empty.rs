// Dart source: pkg/linter/lib/src/rules/prefer_is_empty.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::BinaryExpression, "prefer_is_empty", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    if let Some(value) = get_int_value_with(c, n.right_operand.raw(), false) {
        if is_length_access(c, n.left_operand.raw()) {
            check_value(c, node, value, true, out);
        }
    } else if let Some(value) = get_int_value_with(c, n.left_operand.raw(), false)
        && is_length_access(c, n.right_operand.raw())
    {
        check_value(c, node, value, false, out);
    }
}

/// Dart `_check`.
fn check_value(
    c: &LinterContext<'_>,
    node: NodeId,
    value: i64,
    constant_on_right: bool,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(initializer) =
        this_or_ancestor(c, node, |n| ConstructorInitializer::test(kind(c, n)))
    {
        let declaration = c
            .ast
            .parent(initializer)
            .and_then(|p| c.ast.cast::<ConstructorDeclaration>(p));
        match declaration {
            Some(d) if c.ast[d].const_keyword.is_none() => {}
            _ => return,
        }
    }
    if in_constant_context(c, node) {
        return;
    }
    let operator = lexeme(c, c.ast[Id::<BinaryExpression>::from_raw(node)].operator);
    let code: Option<&'static DiagnosticCode> = if value == 0 {
        match operator {
            "==" | "<=" => Some(&diag::PREFER_IS_EMPTY_USE_IS_EMPTY),
            ">" | "!=" => Some(&diag::PREFER_IS_EMPTY_USE_IS_NOT_EMPTY),
            "<" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_FALSE),
            ">=" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_TRUE),
            _ => None,
        }
    } else if value == 1 {
        match (constant_on_right, operator) {
            (true, ">=") => Some(&diag::PREFER_IS_EMPTY_USE_IS_NOT_EMPTY),
            (true, "<") => Some(&diag::PREFER_IS_EMPTY_USE_IS_EMPTY),
            (false, "<=") => Some(&diag::PREFER_IS_EMPTY_USE_IS_NOT_EMPTY),
            (false, ">") => Some(&diag::PREFER_IS_EMPTY_USE_IS_EMPTY),
            _ => None,
        }
    } else if value < 0 {
        if constant_on_right {
            match operator {
                "==" | "<=" | "<" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_FALSE),
                "!=" | ">=" | ">" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_TRUE),
                _ => None,
            }
        } else {
            match operator {
                "==" | ">=" | ">" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_FALSE),
                "!=" | "<=" | "<" => Some(&diag::PREFER_IS_EMPTY_ALWAYS_TRUE),
                _ => None,
            }
        }
    } else {
        None
    };
    if let Some(code) = code {
        c.report_node(out, code, node, &[]);
    }
}

/// Dart `_isLengthAccess`.
fn is_length_access(c: &LinterContext<'_>, operand: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let mut node = operand;
    loop {
        if let Some(p) = c.ast.cast::<ParenthesizedExpression>(node) {
            node = c.ast[p].expression.raw();
        } else if let Some(a) = c.ast.cast::<AsExpression>(node) {
            node = c.ast[a].expression.raw();
        } else {
            break;
        }
    }
    let (identifier, ty) = if let Some(p) = c.ast.cast::<PrefixedIdentifier>(node) {
        (c.ast[p].identifier, c.static_type(c.ast[p].prefix))
    } else if let Some(p) = c.ast.cast::<PropertyAccess>(node) {
        (
            c.ast[p].property_name,
            c.ast[p].target.and_then(|t| c.static_type(t)),
        )
    } else {
        return false;
    };
    if simple_name(c, identifier) != "length" {
        return false;
    }
    let Some(ty) = ty.filter(|&t| ctx.interface_element(t).is_some()) else {
        return false;
    };
    implements_interface(c, Some(ty), "Iterable", "dart.core")
        || implements_interface(c, Some(ty), "Map", "dart.core")
        || ctx.is_dart_core_string(ty)
}
