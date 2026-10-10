// Dart source: pkg/linter/lib/src/rules/unnecessary_string_interpolations.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::Nullability;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::StringInterpolation,
        "unnecessary_string_interpolations",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    if c.ast
        .parent(node)
        .is_some_and(|p| kind(c, p) == NodeKind::AdjacentStrings)
    {
        return;
    }
    let elements = c
        .ast
        .list_raw(c.ast[Id::<StringInterpolation>::from_raw(node)].elements);
    if elements.len() != 3 {
        return;
    }
    let (Some(start), Some(interpolation), Some(end)) = (
        c.ast.cast::<InterpolationString>(elements[0]),
        c.ast.cast::<InterpolationExpression>(elements[1]),
        c.ast.cast::<InterpolationString>(elements[2]),
    ) else {
        return;
    };
    if c.ast[start].value.is_empty() && c.ast[end].value.is_empty() {
        let expression = c.ast[interpolation].expression;
        if let Some(ty) = c.static_type(expression)
            && ctx.is_dart_core_string(ty)
            && ctx.nullability_suffix(ty) != Nullability::Question
        {
            c.report_node(out, &diag::UNNECESSARY_STRING_INTERPOLATIONS, node, &[]);
        }
    }
}
