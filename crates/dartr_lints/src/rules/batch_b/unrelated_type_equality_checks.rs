// Dart source: pkg/linter/lib/src/rules/unrelated_type_equality_checks.dart
use super::util::*;
use crate::rules::batch_a2::collection_methods_unrelated_type::types_are_unrelated;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeId;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::BinaryExpression,
        "unrelated_type_equality_checks",
        binary,
    );
    r.add(
        NodeKind::RelationalPattern,
        "unrelated_type_equality_checks",
        relational,
    );
}

/// Dart `isFixnumIntX`.
fn is_fixnum_int_x(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(element) = interface_element(c, ty) else {
        return false;
    };
    if !matches!(name(c, element.raw()), Some("Int32" | "Int64")) {
        return false;
    }
    library_uri(c, element.raw())
        .and_then(|u| u.strip_prefix("package:"))
        .and_then(|p| p.split('/').next())
        == Some("fixnum")
}

fn comparable(c: &LinterContext<'_>, left: TypeId, right: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return true };
    (is_fixnum_int_x(c, left) && ctx.is_dart_core_int(right))
        || !types_are_unrelated(c, left, right)
}

fn display(c: &LinterContext<'_>, ty: TypeId) -> String {
    dartr_element::type_display_string_with(&rctx(c).unwrap(), ty, Default::default())
}

fn binary(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
    let is_bool = c
        .static_type(node)
        .is_some_and(|t| ctx.is_dart_core_bool(t));
    if !is_bool || !matches!(lexeme(c, n.operator), "==" | "!=") {
        return;
    }
    if kind(c, n.left_operand) == NodeKind::NullLiteral
        || kind(c, n.right_operand) == NodeKind::NullLiteral
    {
        return;
    }
    let (Some(left), Some(right)) = (
        c.static_type(n.left_operand),
        c.static_type(n.right_operand),
    ) else {
        return;
    };
    if comparable(c, left, right) {
        return;
    }
    let (r, l) = (display(c, right), display(c, left));
    c.report_token(
        out,
        &diag::UNRELATED_TYPE_EQUALITY_CHECKS_IN_EXPRESSION,
        n.operator,
        &[&r, &l],
    );
}

fn relational(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    let Some(value_type) = resolved
        .tables
        .pattern_info
        .get(node)
        .and_then(|i| i.matched_value_type)
    else {
        return;
    };
    let n = &c.ast[Id::<RelationalPattern>::from_raw(node)];
    if !matches!(lexeme(c, n.operator), "==" | "!=") {
        return;
    }
    let Some(operand_type) = c.static_type(n.operand) else {
        return;
    };
    if comparable(c, value_type, operand_type) {
        return;
    }
    let (o, v) = (display(c, operand_type), display(c, value_type));
    c.report_node(
        out,
        &diag::UNRELATED_TYPE_EQUALITY_CHECKS_IN_PATTERN,
        node,
        &[&o, &v],
    );
}
