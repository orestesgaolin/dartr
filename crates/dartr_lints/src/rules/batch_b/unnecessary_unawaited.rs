// Dart source: pkg/linter/lib/src/rules/unnecessary_unawaited.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, Tag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::MethodInvocation, "unnecessary_unawaited", check);
}

/// Dart `ElementExtension.hasAwaitNotRequired` (linter `extensions.dart`).
pub(crate) fn has_await_not_required(c: &LinterContext<'_>, element: Option<ElemRef>) -> bool {
    let Some(element) = element else { return false };
    let element = base(c, element);
    if c.has_package_meta_getter(element, "awaitNotRequired") {
        return true;
    }
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && let Some(variable) = rctx(c).and_then(|ctx| {
            ctx.property_accessor(dartr_element::EId::from_raw(element))
                .variable
                .get()
        })
    {
        return c.has_package_meta_getter(variable.raw(), "awaitNotRequired");
    }
    false
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    // Dart `isUnawaitedFunction`.
    let is_unawaited = simple_name(c, n.method_name) == "unawaited"
        && c.element(n.method_name)
            .is_some_and(|e| library_name(c, base(c, e)) == Some("dart.async"));
    if !is_unawaited || n.target.is_some() {
        return;
    }
    let arguments = c.ast.list_raw(c.ast[n.argument_list].arguments);
    if arguments.len() != 1 {
        return;
    }
    let argument = c
        .ast
        .cast::<NamedArgument>(arguments[0])
        .map_or(arguments[0], |named| c.ast[named].argument_expression.raw());
    let expression = unparenthesized(c, argument);
    let element = match kind(c, expression) {
        NodeKind::BinaryExpression | NodeKind::PrefixExpression | NodeKind::SimpleIdentifier => {
            c.element(expression)
        }
        NodeKind::MethodInvocation => {
            c.element(c.ast[Id::<MethodInvocation>::from_raw(expression)].method_name)
        }
        NodeKind::PrefixedIdentifier => {
            c.element(c.ast[Id::<PrefixedIdentifier>::from_raw(expression)].identifier)
        }
        NodeKind::PropertyAccess => {
            c.element(c.ast[Id::<PropertyAccess>::from_raw(expression)].property_name)
        }
        _ => None,
    };
    if has_await_not_required(c, element) {
        c.report_node(out, &diag::UNNECESSARY_UNAWAITED, n.method_name, &[]);
    }
}
