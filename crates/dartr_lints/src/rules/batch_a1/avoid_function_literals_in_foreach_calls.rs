// Dart source: pkg/linter/lib/src/rules/avoid_function_literals_in_foreach_calls.dart

use super::helpers::lexeme;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodInvocation, NodeId, NodeKind, NodeType};
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

fn contains_null_aware(
    c: &LinterContext<'_>,
    mut expression: dartr_ast::Id<dartr_ast::Expression>,
) -> bool {
    loop {
        if let Some(invocation) = c.ast.cast::<dartr_ast::MethodInvocation>(expression) {
            if c.ast[invocation]
                .operator
                .is_some_and(|operator| lexeme(c, operator).contains('?'))
            {
                return true;
            }
            let Some(target) = c.ast[invocation].target else {
                return false;
            };
            expression = target;
        } else if let Some(access) = c.ast.cast::<dartr_ast::PropertyAccess>(expression) {
            if lexeme(c, c.ast[access].operator).contains('?') {
                return true;
            }
            let Some(target) = c.ast[access].target else {
                return false;
            };
            expression = target;
        } else if let Some(index) = c.ast.cast::<dartr_ast::IndexExpression>(expression) {
            if c.ast[index].question.is_some() {
                return true;
            }
            let Some(target) = c.ast[index].target else {
                return false;
            };
            expression = target;
        } else if let Some(prefixed) = c.ast.cast::<dartr_ast::PrefixedIdentifier>(expression) {
            expression = c.ast[prefixed].prefix.upcast();
        } else {
            return false;
        }
    }
}

fn inside_cascade(c: &LinterContext<'_>, node: NodeId) -> bool {
    let mut current = c.ast.parent(node);
    while let Some(node) = current {
        if dartr_ast::Statement::test(c.ast.kind(node)) {
            return false;
        }
        if c.ast.kind(node) == NodeKind::CascadeExpression {
            return true;
        }
        current = c.ast.parent(node);
    }
    false
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    let Some(target) = n.target else { return };
    if lexeme(c, c.ast[n.method_name].token) != "forEach"
        || n.operator.is_some_and(|op| lexeme(c, op).contains('?'))
        || contains_null_aware(c, target)
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
    if inside_cascade(c, node) {
        return;
    }
    c.report_node(
        out,
        &diag::AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS,
        n.method_name,
        &[],
    );
}
