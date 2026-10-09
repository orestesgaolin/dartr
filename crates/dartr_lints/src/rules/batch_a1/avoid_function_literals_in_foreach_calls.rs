// Dart source: pkg/linter/lib/src/rules/avoid_function_literals_in_foreach_calls.dart

use super::helpers::lexeme;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodInvocation, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodInvocation,
        "avoid_function_literals_in_foreach_calls",
        check,
    );
}

fn chained(c: &LinterContext<'_>, mut expression: dartr_ast::Id<dartr_ast::Expression>) -> bool {
    loop {
        if let Some(prefixed) = c.ast.cast::<dartr_ast::PrefixedIdentifier>(expression) {
            expression = c.ast[prefixed].prefix.upcast();
        } else if c.ast.kind(expression) == NodeKind::MethodInvocation {
            return true;
        } else if let Some(property) = c.ast.cast::<dartr_ast::PropertyAccess>(expression) {
            let Some(target) = c.ast[property].target else {
                return false;
            };
            expression = target;
        } else {
            return false;
        }
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    let Some(target) = n.target else { return };
    if lexeme(c, c.ast[n.method_name].token) != "forEach"
        || n.operator.is_some_and(|op| lexeme(c, op).contains('?'))
    {
        return;
    }
    let args = c.ast.list_raw(c.ast[n.argument_list].arguments);
    if args
        .first()
        .is_none_or(|arg| c.ast.kind(*arg) != NodeKind::FunctionExpression)
    {
        return;
    }
    if c.static_type(target).is_none_or(|ty| {
        r.ctx
            .as_instance_of(ty, r.ctx.tp.iterable_element().upcast())
            .is_none()
    }) {
        return;
    }
    if chained(c, target) {
        return;
    }
    if super::helpers::ancestor(c.ast, node, NodeKind::CascadeExpression).is_some() {
        return;
    }
    c.report_node(
        out,
        &diag::AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS,
        n.method_name,
        &[],
    );
}
