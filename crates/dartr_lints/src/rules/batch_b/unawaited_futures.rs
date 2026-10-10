// Dart source: pkg/linter/lib/src/rules/unawaited_futures.dart
use super::util::*;
use crate::rules::batch_a2::discarded_futures::{
    unused_futures_cascade, unused_futures_interpolation, unused_futures_statement,
};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ExpressionStatement,
        "unawaited_futures",
        statement,
    );
    r.add(NodeKind::CascadeExpression, "unawaited_futures", cascade);
    r.add(
        NodeKind::InterpolationExpression,
        "unawaited_futures",
        interpolation,
    );
}

fn statement(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    unused_futures_statement(c, node, out, is_interesting, &diag::UNAWAITED_FUTURES);
}
fn cascade(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    unused_futures_cascade(c, node, out, is_interesting, &diag::UNAWAITED_FUTURES);
}
fn interpolation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    unused_futures_interpolation(c, node, out, is_interesting, &diag::UNAWAITED_FUTURES);
}

fn is_interesting(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(ty) = c.static_type(node) else {
        return false;
    };
    // Dart `isOrImplementsFuture`.
    let Some(element) = ctx.interface_element(ty) else {
        return false;
    };
    if !(ctx.is_dart_async_future(ty)
        || ctx
            .element_all_supertypes(element)
            .iter()
            .any(|&t| ctx.is_dart_async_future(t)))
    {
        return false;
    }
    ancestors(c, node)
        .find(|&n| FunctionBody::test(kind(c, n)))
        .is_some_and(|body| is_asynchronous(c, body))
}
