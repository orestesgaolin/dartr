// Dart source: pkg/linter/lib/src/rules/no_duplicate_case_values.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_switch_statement("no_duplicate_case_values", check);
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<SwitchStatement>::from_raw(node)];
    let Some(ts) = ctx.constant_type_system() else {
        return;
    };
    let mut values: Vec<(dartr_constant::DartObjectImpl, NodeId)> = Vec::new();
    for member in ctx.ast.list(n.members) {
        let Some(case) = ctx.ast.cast::<SwitchCase>(*member) else {
            continue;
        };
        let expression = ctx.ast[case].expression.raw();
        let Some(value) = ctx.constant_value(expression) else {
            continue;
        };
        if !value.has_known_value() {
            continue;
        }
        if let Some((_, first)) = values.iter().find(|(v, _)| value.dart_eq(v, &ts)) {
            let a = ctx.text(expression);
            let b = ctx.text(*first);
            ctx.report_node(out, &diag::NO_DUPLICATE_CASE_VALUES, expression, &[&a, &b]);
        } else {
            values.push((value, expression));
        }
    }
}
