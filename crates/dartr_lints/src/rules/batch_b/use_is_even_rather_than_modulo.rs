// Dart source: pkg/linter/lib/src/rules/use_is_even_rather_than_modulo.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::BinaryExpression,
        "use_is_even_rather_than_modulo",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    if lexeme(c, n.operator) != "==" || in_constant_context(c, node) {
        return;
    }
    let is_int = |e: NodeId| c.static_type(e).is_some_and(|t| ctx.is_dart_core_int(t));
    let (left, right) = (n.left_operand.raw(), n.right_operand.raw());
    let Some(right_literal) = c.ast.cast::<IntegerLiteral>(right) else {
        return;
    };
    if !(is_int(left) && is_int(right)) {
        return;
    }
    let Some(left) = c.ast.cast::<BinaryExpression>(left) else {
        return;
    };
    let l = &c.ast[left];
    if lexeme(c, l.operator) != "%" {
        return;
    }
    let Some(right_child) = c.ast.cast::<IntegerLiteral>(l.right_operand.raw()) else {
        return;
    };
    if c.ast[right_child].value != Some(2) {
        return;
    }
    if !is_int(right_child.raw()) {
        return;
    }
    let Some(value) = c.ast[right_literal].value else {
        return;
    };
    if let Some(assert) = this_or_ancestor_kind(c, node, NodeKind::AssertInitializer)
        && let Some(constructor) = c
            .ast
            .parent(assert)
            .and_then(|p| c.ast.cast::<ConstructorDeclaration>(p))
        && c.ast[constructor].const_keyword.is_some()
    {
        return;
    }
    let name = if value == 0 { "isEven" } else { "isOdd" };
    c.report_node(out, &diag::USE_IS_EVEN_RATHER_THAN_MODULO, node, &[name]);
}
