// Dart source: pkg/linter/lib/src/rules/do_not_use_environment.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::DotShorthandConstructorInvocation,
        "do_not_use_environment",
        check,
    );
    registry.add(
        NodeKind::InstanceCreationExpression,
        "do_not_use_environment",
        check,
    );
}
fn report(
    context: &LinterContext<'_>,
    type_node: NodeId,
    report_node: NodeId,
    name: &str,
    out: &mut Vec<Diagnostic>,
) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let Some(ty) = context.static_type(type_node) else {
        return;
    };
    let applies = (name == "fromEnvironment"
        && (resolved.ctx.is_dart_core_bool(ty)
            || resolved.ctx.is_dart_core_int(ty)
            || resolved.ctx.is_dart_core_string(ty)))
        || (name == "hasEnvironment" && resolved.ctx.is_dart_core_bool(ty));
    if applies {
        context.report_node(out, &diag::DO_NOT_USE_ENVIRONMENT, report_node, &[]);
    }
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match context.ast.kind(node) {
        NodeKind::DotShorthandConstructorInvocation => {
            let n = &context.ast[context
                .ast
                .cast::<DotShorthandConstructorInvocation>(node)
                .unwrap()];
            let name = context
                .ast
                .tokens
                .lexeme(context.ast[n.constructor_name].token);
            report(context, node, n.constructor_name.raw(), name, out);
        }
        NodeKind::InstanceCreationExpression => {
            let n = &context.ast[context
                .ast
                .cast::<InstanceCreationExpression>(node)
                .unwrap()];
            let constructor = &context.ast[n.constructor_name];
            let Some(name) = constructor
                .name
                .map(|id| context.ast.tokens.lexeme(context.ast[id].token))
            else {
                return;
            };
            let Some(element) = context
                .element(n.constructor_name)
                .or_else(|| context.element(node))
            else {
                return;
            };
            let Some(base) = super::helpers::base_element(context, element) else {
                return;
            };
            if base.is::<dartr_element::ConstructorElement>() {
                report(context, node, n.constructor_name.raw(), name, out);
            }
        }
        _ => {}
    }
}
