// Dart source: pkg/linter/lib/src/rules/discarded_futures.dart
// Dart source: pkg/linter/lib/src/util/unused_futures.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, GetterElement, InterfaceElement, SetterElement};
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

fn metadata(context: &LinterContext<'_>, node: NodeId) -> Option<NodeList<Annotation>> {
    match context.ast.kind(node) {
        NodeKind::MethodDeclaration => {
            Some(context.ast[context.ast.cast::<MethodDeclaration>(node)?].metadata)
        }
        NodeKind::FunctionDeclaration => {
            Some(context.ast[context.ast.cast::<FunctionDeclaration>(node)?].metadata)
        }
        NodeKind::VariableDeclaration => {
            Some(context.ast[context.ast.cast::<VariableDeclaration>(node)?].metadata)
        }
        NodeKind::FieldDeclaration => {
            Some(context.ast[context.ast.cast::<FieldDeclaration>(node)?].metadata)
        }
        NodeKind::ClassTypeAlias => {
            Some(context.ast[context.ast.cast::<ClassTypeAlias>(node)?].metadata)
        }
        NodeKind::FunctionTypeAlias => {
            Some(context.ast[context.ast.cast::<FunctionTypeAlias>(node)?].metadata)
        }
        NodeKind::GenericTypeAlias => {
            Some(context.ast[context.ast.cast::<GenericTypeAlias>(node)?].metadata)
        }
        _ => None,
    }
}

fn local_element_is_await_not_required(context: &LinterContext<'_>, element: ElementId) -> bool {
    for unit in std::iter::once(*context).chain(
        (0..context.resolved_units.len())
            .filter(|&index| index != context.current_unit)
            .filter_map(|index| context.resolved_unit(index)),
    ) {
        for declaration in (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .filter(|&node| unit.declared_element(node) == Some(element))
        {
            let direct = metadata(&unit, declaration).is_some_and(|metadata| {
                super::deprecated_consistency::has_annotation(
                    &unit,
                    metadata,
                    "package:meta/meta.dart",
                    &["awaitNotRequired"],
                )
            });
            let enclosing_field = super::helpers::ancestors(unit.ast, declaration)
                .find_map(|ancestor| unit.ast.cast::<FieldDeclaration>(ancestor))
                .is_some_and(|field| {
                    super::deprecated_consistency::has_annotation(
                        &unit,
                        unit.ast[field].metadata,
                        "package:meta/meta.dart",
                        &["awaitNotRequired"],
                    )
                });
            if direct || enclosing_field {
                return true;
            }
        }
    }
    false
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

fn is_await_not_required(context: &LinterContext<'_>, expression: NodeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    if let Some(invocation) = context.ast.cast::<FunctionExpressionInvocation>(expression)
        && context
            .static_type(context.ast[invocation].function)
            .and_then(|ty| resolved.ctx.type_alias(ty))
            .is_some_and(|alias| {
                local_element_is_await_not_required(
                    context,
                    resolved.ctx.alias(alias).element.raw(),
                )
            })
    {
        return true;
    }
    let Some(element) = expression_element(context, expression) else {
        return false;
    };
    let base = dartr_typesystem::member::base_element(&resolved.ctx, element);
    if local_element_is_await_not_required(context, base)
        || accessor_variable(context, base)
            .is_some_and(|variable| local_element_is_await_not_required(context, variable))
    {
        return true;
    }
    let Some(enclosing) = resolved
        .ctx
        .element_data(base)
        .and_then(|data| data.enclosing)
        .and_then(|element| element.cast::<InterfaceElement>())
    else {
        return false;
    };
    let Some(name) = resolved.ctx.element_name(base) else {
        return false;
    };
    let inheritance =
        dartr_typesystem::inheritance_manager3::InheritanceManager3::new(resolved.ctx);
    let lookup_name = dartr_typesystem::inheritance_manager3::Name::for_library(
        &resolved.ctx,
        resolved
            .ctx
            .element_data(base)
            .and_then(|data| data.library),
        name,
    );
    inheritance
        .get_overridden(enclosing, lookup_name)
        .is_some_and(|members| {
            members.into_iter().any(|member| {
                let member = dartr_typesystem::member::base_element(&resolved.ctx, member);
                local_element_is_await_not_required(context, member)
                    || accessor_variable(context, member).is_some_and(|variable| {
                        local_element_is_await_not_required(context, variable)
                    })
            })
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
    let expression = context.ast[context.ast.cast::<ExpressionStatement>(node).unwrap()]
        .expression
        .raw();
    if matches!(
        context.ast.kind(expression),
        NodeKind::AssignmentExpression | NodeKind::AwaitExpression
    ) {
        return;
    }
    if interesting(context, expression)
        && !is_await_not_required(context, expression)
        && !ignored_known_case(context, expression)
    {
        report(context, expression, out);
    }
}
fn check_cascade(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<CascadeExpression>(node).unwrap()];
    for &section in context.ast.list(n.cascade_sections) {
        let expression = section.raw();
        if context.ast.kind(expression) != NodeKind::AssignmentExpression
            && interesting(context, expression)
            && !is_await_not_required(context, expression)
        {
            report(context, expression, out);
        }
    }
}
fn check_interpolation(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = context.ast[context.ast.cast::<InterpolationExpression>(node).unwrap()]
        .expression
        .raw();
    if interesting(context, expression) && !is_await_not_required(context, expression) {
        report(context, expression, out);
    }
}
