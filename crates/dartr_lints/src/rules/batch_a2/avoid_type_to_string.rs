// Dart source: pkg/linter/lib/src/rules/avoid_type_to_string.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodInvocation,
        "avoid_type_to_string",
        check_invocation,
    );
    registry.add(
        NodeKind::ArgumentList,
        "avoid_type_to_string",
        check_arguments,
    );
}
fn is_bad(context: &LinterContext<'_>, target: NodeId, method: Id<SimpleIdentifier>) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    if context.ast.tokens.lexeme(context.ast[method].token) != "toString" {
        return false;
    }
    let Some(element) = context.element(method) else {
        return false;
    };
    if !member::is_object_member(&resolved.ctx, element) {
        return false;
    }
    let Some(ty) = context.static_type(target) else {
        return false;
    };
    context
        .type_system()
        .is_some_and(|ts| ts.is_subtype_of(ty, resolved.ctx.tp.type_type()))
}
fn check_invocation(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<MethodInvocation>(node).unwrap()];
    let Some(target) = n.target else {
        return;
    };
    if is_bad(context, target.raw(), n.method_name) {
        context.report_node(out, &diag::AVOID_TYPE_TO_STRING, n.method_name, &[]);
    }
}
fn check_arguments(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<ArgumentList>(node).unwrap()];
    for &argument in context.ast.list(n.arguments) {
        let expression = argument.raw();
        match context.ast.kind(expression) {
            NodeKind::PropertyAccess => {
                let p = &context.ast[context.ast.cast::<PropertyAccess>(expression).unwrap()];
                if let Some(target) = p.target
                    && is_bad(context, target.raw(), p.property_name)
                {
                    context.report_node(out, &diag::AVOID_TYPE_TO_STRING, p.property_name, &[]);
                }
            }
            NodeKind::PrefixedIdentifier => {
                let p = &context.ast[context.ast.cast::<PrefixedIdentifier>(expression).unwrap()];
                if is_bad(context, p.prefix.raw(), p.identifier) {
                    context.report_node(out, &diag::AVOID_TYPE_TO_STRING, p.identifier, &[]);
                }
            }
            _ => {}
        }
    }
}
