// Dart source: pkg/linter/lib/src/rules/unnecessary_await_in_return.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElemRef;
use dartr_typesystem::{TypeExt, member};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ExpressionFunctionBody,
        "unnecessary_await_in_return",
        check,
    );
    r.add(NodeKind::ReturnStatement, "unnecessary_await_in_return", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = match kind(c, node) {
        NodeKind::ExpressionFunctionBody => {
            Some(c.ast[Id::<ExpressionFunctionBody>::from_raw(node)].expression.raw())
        }
        _ => c.ast[Id::<ReturnStatement>::from_raw(node)].expression.map(|e| e.raw()),
    };
    let Some(expression) = expression.map(|e| unparenthesized(c, e)) else {
        return;
    };
    visit(c, node, expression, out);
}

fn visit(c: &LinterContext<'_>, node: NodeId, expression: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(ts)) = (rctx(c), c.type_system()) else {
        return;
    };
    let Some(await_expression) = c.ast.cast::<AwaitExpression>(expression) else {
        return;
    };
    let Some(ty) = c.static_type(c.ast[await_expression].expression) else {
        return;
    };
    if !ctx.is_dart_async_future(ty) {
        return;
    }
    let Some(parent) = this_or_ancestor(c, node, |e| match kind(c, e) {
        NodeKind::FunctionExpression | NodeKind::MethodDeclaration => true,
        NodeKind::Block => c.ast.parent(e).is_some_and(|p| kind(c, p) == NodeKind::TryStatement),
        _ => false,
    }) else {
        return;
    };
    if kind(c, parent) == NodeKind::Block {
        return;
    }
    let Some(return_type) = c
        .declared_element(parent)
        .map(|e| member::return_type(&ctx, ElemRef::Base(e)))
    else {
        return;
    };
    if ctx.is_dart_async_future(return_type) && ts.is_subtype_of(ty, return_type) {
        c.report_token(
            out,
            &diag::UNNECESSARY_AWAIT_IN_RETURN,
            c.ast[await_expression].await_keyword,
            &[],
        );
    }
}
