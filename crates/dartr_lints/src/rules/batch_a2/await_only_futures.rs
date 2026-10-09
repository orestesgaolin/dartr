// Dart source: pkg/linter/lib/src/rules/await_only_futures.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ExtensionTypeElement, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::AwaitExpression, "await_only_futures", check);
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<AwaitExpression>(node).unwrap()];
    if context.ast.kind(n.expression) == NodeKind::NullLiteral {
        return;
    }
    let (Some(resolved), Some(ts), Some(mut ty)) = (
        context.resolved,
        context.type_system(),
        context.static_type(n.expression),
    ) else {
        return;
    };
    if matches!(resolved.ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid) {
        return;
    }
    ty = ts.promote_to_non_null(ty);
    if resolved.ctx.is_dart_async_future_or(ty)
        || resolved
            .ctx
            .interface_element(ty)
            .is_some_and(|e| e.raw().is::<ExtensionTypeElement>())
        || ts.is_assignable_to(ty, resolved.ctx.tp.future_dynamic_type(), false)
    {
        return;
    }
    let text = super::helpers::type_text(context, ty);
    context.report_token(out, &diag::AWAIT_ONLY_FUTURES, n.await_keyword, &[&text]);
}
