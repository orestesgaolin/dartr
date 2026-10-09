// Dart source: pkg/linter/lib/src/rules/discarded_futures.dart
// Dart source: pkg/linter/lib/src/util/unused_futures.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ExpressionStatement,
        "discarded_futures",
        check_statement,
    );
    registry.add(
        NodeKind::CascadeExpression,
        "discarded_futures",
        check_cascade,
    );
    registry.add(
        NodeKind::InterpolationExpression,
        "discarded_futures",
        check_interpolation,
    );
}
fn interesting(context: &LinterContext<'_>, expression: NodeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let Some(ty) = context.static_type(expression) else {
        return false;
    };
    let future = resolved
        .ctx
        .as_instance_of(ty, resolved.ctx.tp.future_element().upcast())
        .is_some();
    let future_or = resolved
        .ctx
        .as_instance_of(ty, resolved.ctx.tp.future_or_element().upcast())
        .is_some();
    (future || future_or)
        && super::helpers::nearest_function_body(context.ast, expression)
            .is_some_and(|body| super::helpers::is_synchronous_body(context.ast, body))
}
fn report(context: &LinterContext<'_>, expression: NodeId, out: &mut Vec<Diagnostic>) {
    let report = match context.ast.kind(expression) {
        NodeKind::MethodInvocation => context.ast
            [context.ast.cast::<MethodInvocation>(expression).unwrap()]
        .method_name
        .raw(),
        NodeKind::InstanceCreationExpression => context.ast[context
            .ast
            .cast::<InstanceCreationExpression>(expression)
            .unwrap()]
        .constructor_name
        .raw(),
        NodeKind::FunctionExpressionInvocation => context.ast[context
            .ast
            .cast::<FunctionExpressionInvocation>(expression)
            .unwrap()]
        .function
        .raw(),
        NodeKind::PrefixedIdentifier => context.ast
            [context.ast.cast::<PrefixedIdentifier>(expression).unwrap()]
        .identifier
        .raw(),
        NodeKind::PropertyAccess => context.ast
            [context.ast.cast::<PropertyAccess>(expression).unwrap()]
        .property_name
        .raw(),
        _ => expression,
    };
    context.report_node(out, &diag::DISCARDED_FUTURES, report, &[]);
}
fn ignored_known_case(context: &LinterContext<'_>, expression: NodeId) -> bool {
    match context.ast.kind(expression) {
        NodeKind::InstanceCreationExpression => {
            let n = &context.ast[context
                .ast
                .cast::<InstanceCreationExpression>(expression)
                .unwrap()];
            let name = context.ast[n.constructor_name]
                .name
                .map(|name| context.ast.tokens.lexeme(context.ast[name].token));
            name == Some("delayed")
                && context
                    .ast
                    .list(context.ast[n.argument_list].arguments)
                    .len()
                    == 2
                && context.static_type(expression).is_some_and(|ty| {
                    context
                        .resolved
                        .is_some_and(|resolved| resolved.ctx.is_dart_async_future(ty))
                })
        }
        NodeKind::MethodInvocation => {
            let n = &context.ast[context.ast.cast::<MethodInvocation>(expression).unwrap()];
            if context.ast.tokens.lexeme(context.ast[n.method_name].token) != "putIfAbsent" {
                return false;
            }
            context
                .element(n.method_name)
                .and_then(|element| super::helpers::base_element(context, element))
                .and_then(|element| context.resolved?.ctx.element_data(element)?.enclosing)
                .is_some_and(|enclosing| {
                    let resolved = context.resolved.unwrap();
                    resolved.ctx.element_name(enclosing) == Some("Map")
                        && resolved.ctx.element_library_uri(enclosing) == Some("dart:core")
                })
        }
        _ => false,
    }
}
fn check_statement(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = context.ast[context.ast.cast::<ExpressionStatement>(node).unwrap()]
        .expression
        .raw();
    if matches!(
        context.ast.kind(expression),
        NodeKind::AssignmentExpression | NodeKind::AwaitExpression
    ) {
        return;
    }
    if interesting(context, expression) && !ignored_known_case(context, expression) {
        report(context, expression, out);
    }
}
fn check_cascade(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<CascadeExpression>(node).unwrap()];
    for &section in context.ast.list(n.cascade_sections) {
        let expression = section.raw();
        if context.ast.kind(expression) != NodeKind::AssignmentExpression
            && interesting(context, expression)
        {
            report(context, expression, out);
        }
    }
}
fn check_interpolation(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = context.ast[context.ast.cast::<InterpolationExpression>(node).unwrap()]
        .expression
        .raw();
    if interesting(context, expression) {
        report(context, expression, out);
    }
}
