// Dart source: pkg/linter/lib/src/rules/prefer_interpolation_to_compose_strings.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::BinaryExpression,
        "prefer_interpolation_to_compose_strings",
        check,
    );
}

/// Dart `chainedAdditions`.
fn chained_additions(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<NodeId>) -> bool {
    let Some(binary) = c.ast.cast::<BinaryExpression>(node) else {
        out.push(node);
        return true;
    };
    let b = &c.ast[binary];
    if lexeme(c, b.operator) != "+" {
        // `const []`: the whole chain is empty.
        return false;
    }
    if !chained_additions(c, b.left_operand.raw(), out) {
        out.clear();
    }
    out.push(b.right_operand.raw());
    true
}

/// Dart `isToStringInvocationWithArguments`.
fn is_to_string_with_arguments(c: &LinterContext<'_>, node: NodeId) -> bool {
    c.ast.cast::<MethodInvocation>(node).is_some_and(|m| {
        simple_name(c, c.ast[m].method_name) == "toString"
            && !arguments(c, c.ast[m].argument_list).is_empty()
    })
}

fn is_raw(c: &LinterContext<'_>, node: NodeId) -> bool {
    c.ast
        .cast::<SimpleStringLiteral>(node)
        .is_some_and(|s| lexeme(c, c.ast[s].literal).starts_with('r'))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    if lexeme(c, n.operator) != "+" {
        return;
    }
    let mut operands = Vec::new();
    chained_additions(c, node, &mut operands);
    let mut i = 0;
    while i + 1 < operands.len() {
        let (left, right) = (operands[i], operands[i + 1]);
        let is_string = |n: NodeId| StringLiteral::test(kind(c, n));
        if (!is_string(left) && !is_string(right))
            || is_raw(c, left)
            || is_raw(c, right)
            || (is_string(left) && is_string(right))
            || is_to_string_with_arguments(c, left)
            || is_to_string_with_arguments(c, right)
        {
            i += 1;
            continue;
        }
        if c.static_type(left).is_some_and(|t| ctx.is_dart_core_string(t)) {
            let offset = c.ast.offset(left) as usize;
            let end = c.ast.end(right) as usize;
            c.report_offset(out, &diag::PREFER_INTERPOLATION_TO_COMPOSE_STRINGS, offset, end - offset, &[]);
            i += 1;
        }
        i += 1;
    }
}
