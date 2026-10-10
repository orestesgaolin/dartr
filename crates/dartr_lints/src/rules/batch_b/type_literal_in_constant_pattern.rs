// Dart source: pkg/linter/lib/src/rules/type_literal_in_constant_pattern.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ConstantPattern,
        "type_literal_in_constant_pattern",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(resolved), Some(ts)) = (c.resolved, c.type_system()) else {
        return;
    };
    let n = &c.ast[Id::<ConstantPattern>::from_raw(node)];
    if n.const_keyword.is_some() {
        return;
    }
    let matched = resolved
        .tables
        .pattern_info
        .get(node)
        .and_then(|i| i.matched_value_type);
    if let Some(matched) = matched
        && ts.is_subtype_of(matched, resolved.ctx.tp.type_type())
    {
        return;
    }
    if c.static_type(n.expression)
        .is_some_and(|t| resolved.ctx.is_dart_core_type(t))
    {
        c.report_node(out, &diag::TYPE_LITERAL_IN_CONSTANT_PATTERN, node, &[]);
    }
}
