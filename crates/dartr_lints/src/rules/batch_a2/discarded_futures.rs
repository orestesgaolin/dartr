// Dart source: pkg/linter/lib/src/rules/discarded_futures.dart
// Dart source: pkg/linter/lib/src/util/unused_futures.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, GetterElement, InterfaceElement, SetterElement, Tag};
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
fn report(
    context: &LinterContext<'_>,
    expression: NodeId,
    out: &mut Vec<Diagnostic>,
    code: &'static dartr_diagnostics::DiagnosticCode,
) {
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
    context.report_node(out, code, report, &[]);
}

/// Dart `ElementAnnotation.isAwaitNotRequired` (`_isPackageMetaGetter`).
fn is_await_not_required_annotation(context: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let ctx = &resolved.ctx;
    element.tag() == Tag::Getter
        && ctx.element_name(element) == Some("awaitNotRequired")
        && dartr_typesystem::member::library(ctx, ElemRef::Base(element))
            .is_some_and(|library| ctx.element_name(library.raw()) == Some("meta"))
}

/// Dart `ElementExtension.hasAwaitNotRequired` (linter `extensions.dart`).
fn has_await_not_required(context: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(metadata) = context.resolved.and_then(|r| r.metadata) else {
        return false;
    };
    let annotated = |e: ElementId| {
        metadata.annotations(e).into_iter().any(|annotation| {
            metadata
                .annotation_element(annotation)
                .is_some_and(|a| is_await_not_required_annotation(context, a))
        })
    };
    annotated(element) || accessor_variable(context, element).is_some_and(annotated)
}

fn accessor_variable(context: &LinterContext<'_>, element: ElementId) -> Option<ElementId> {
    let resolved = context.resolved?;
    if let Some(getter) = element.cast::<GetterElement>() {
        return resolved.ctx.get(getter).variable.get().map(|id| id.raw());
    }
    if let Some(setter) = element.cast::<SetterElement>() {
        return resolved.ctx.get(setter).variable.get().map(|id| id.raw());
    }
    None
}

fn expression_element(context: &LinterContext<'_>, expression: NodeId) -> Option<ElemRef> {
    match context.ast.kind(expression) {
        NodeKind::MethodInvocation => context
            .element(context.ast[context.ast.cast::<MethodInvocation>(expression)?].method_name),
        NodeKind::PrefixedIdentifier => context
            .element(context.ast[context.ast.cast::<PrefixedIdentifier>(expression)?].identifier),
        NodeKind::PropertyAccess => context
            .element(context.ast[context.ast.cast::<PropertyAccess>(expression)?].property_name),
        NodeKind::BinaryExpression | NodeKind::PrefixExpression => context.element(expression),
        _ => None,
    }
}

/// Dart `ExpressionExtension.isAwaitNotRequired` (linter `extensions.dart`).
fn is_await_not_required(context: &LinterContext<'_>, expression: NodeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let ctx = &resolved.ctx;
    // `p();` where `p` is typed with an annotated typedef.
    if let Some(invocation) = context.ast.cast::<FunctionExpressionInvocation>(expression)
        && context
            .static_type(context.ast[invocation].function)
            .and_then(|ty| ctx.type_alias(ty))
            .is_some_and(|alias| has_await_not_required(context, ctx.alias(alias).element.raw()))
    {
        return true;
    }
    let Some(element) = expression_element(context, expression) else {
        return false;
    };
    let base = dartr_typesystem::member::base_element(ctx, element);
    if has_await_not_required(context, base) {
        return true;
    }
    let Some(name) = ctx.element_name(base) else {
        return false;
    };
    let Some(enclosing) = ctx
        .element_data(base)
        .and_then(|data| data.enclosing)
        .and_then(|element| element.cast::<InterfaceElement>())
    else {
        return false;
    };
    let is_method = base.tag() == Tag::Method;
    dartr_typesystem::class_hierarchy::implemented_interfaces(ctx, enclosing)
        .iter()
        .filter_map(|&t| {
            if is_method {
                dartr_typesystem::lookup::type_get_method(ctx, t, name)
            } else {
                dartr_typesystem::lookup::type_get_getter(ctx, t, name)
            }
        })
        .any(|member| {
            has_await_not_required(context, dartr_typesystem::member::base_element(ctx, member))
        })
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
    unused_futures_statement(context, node, out, interesting, &diag::DISCARDED_FUTURES);
}
fn check_cascade(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    unused_futures_cascade(context, node, out, interesting, &diag::DISCARDED_FUTURES);
}
fn check_interpolation(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    unused_futures_interpolation(context, node, out, interesting, &diag::DISCARDED_FUTURES);
}

/// Dart `IsInterestingFilter`.
pub(crate) type IsInteresting = fn(&LinterContext<'_>, NodeId) -> bool;

/// Dart `UnusedFuturesVisitor.visitExpressionStatement`.
pub(crate) fn unused_futures_statement(
    context: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
    is_interesting: IsInteresting,
    code: &'static dartr_diagnostics::DiagnosticCode,
) {
    let expression = context.ast[context.ast.cast::<ExpressionStatement>(node).unwrap()]
        .expression
        .raw();
    if matches!(
        context.ast.kind(expression),
        NodeKind::AssignmentExpression | NodeKind::AwaitExpression
    ) {
        return;
    }
    if is_interesting(context, expression)
        && !is_await_not_required(context, expression)
        && !ignored_known_case(context, expression)
    {
        report(context, expression, out, code);
    }
}

/// Dart `UnusedFuturesVisitor.visitCascadeExpression`.
pub(crate) fn unused_futures_cascade(
    context: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
    is_interesting: IsInteresting,
    code: &'static dartr_diagnostics::DiagnosticCode,
) {
    let n = &context.ast[context.ast.cast::<CascadeExpression>(node).unwrap()];
    for &section in context.ast.list(n.cascade_sections) {
        visit(context, section.raw(), out, is_interesting, code);
    }
}

/// Dart `UnusedFuturesVisitor.visitInterpolationExpression`.
pub(crate) fn unused_futures_interpolation(
    context: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
    is_interesting: IsInteresting,
    code: &'static dartr_diagnostics::DiagnosticCode,
) {
    let expression = context.ast[context.ast.cast::<InterpolationExpression>(node).unwrap()]
        .expression
        .raw();
    visit(context, expression, out, is_interesting, code);
}

/// Dart `UnusedFuturesVisitor._visit`.
fn visit(
    context: &LinterContext<'_>,
    expression: NodeId,
    out: &mut Vec<Diagnostic>,
    is_interesting: IsInteresting,
    code: &'static dartr_diagnostics::DiagnosticCode,
) {
    if is_await_not_required(context, expression) {
        return;
    }
    let Some(resolved) = context.resolved else {
        return;
    };
    let is_future = context.static_type(expression).is_some_and(|ty| {
        resolved
            .ctx
            .as_instance_of(ty, resolved.ctx.tp.future_element().upcast())
            .is_some()
            || resolved
                .ctx
                .as_instance_of(ty, resolved.ctx.tp.future_or_element().upcast())
                .is_some()
    });
    if is_future
        && is_interesting(context, expression)
        && context.ast.kind(expression) != NodeKind::AssignmentExpression
    {
        report(context, expression, out, code);
    }
}
