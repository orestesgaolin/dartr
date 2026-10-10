// Dart source: pkg/linter/lib/src/rules/unnecessary_async.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, Nullability, TypeId, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::FunctionDeclaration, "unnecessary_async", visit_function_declaration);
    r.add(NodeKind::FunctionExpression, "unnecessary_async", visit_function_expression);
    r.add(NodeKind::MethodDeclaration, "unnecessary_async", visit_method_declaration);
}

fn declared_return_type(c: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let ctx = rctx(c)?;
    let element = c.declared_element(node)?;
    Some(dartr_typesystem::member::return_type(&ctx, ElemRef::Base(element)))
}

fn visit_function_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(return_type) = declared_return_type(c, node) else { return };
    let function = c.ast[Id::<FunctionDeclaration>::from_raw(node)].function_expression;
    check_body(c, c.ast[function].body.raw(), Some(return_type), out);
}

fn visit_function_expression(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.ast.parent(node).is_some_and(|p| kind(c, p) == NodeKind::FunctionDeclaration) {
        return;
    }
    let body = c.ast[Id::<FunctionExpression>::from_raw(node)].body.raw();
    let imposed_type = rctx_body_context(c, body).and_then(|b| b.imposed_type);
    check_body(c, body, imposed_type, out);
}

fn visit_method_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(return_type) = declared_return_type(c, node) else { return };
    check_body(c, c.ast[Id::<MethodDeclaration>::from_raw(node)].body.raw(), Some(return_type), out);
}

fn rctx_body_context(c: &LinterContext<'_>, body: NodeId) -> Option<crate::BodyContext> {
    c.resolved.as_ref()?.body_context.get(body).copied()
}

/// Dart `_HasAwaitVisitor`.
struct HasAwait {
    has_await: bool,
    every_return_has_value: bool,
    returns_only_future: bool,
}

impl HasAwait {
    fn visit(&mut self, c: &LinterContext<'_>, node: NodeId) {
        match kind(c, node) {
            NodeKind::AwaitExpression => self.has_await = true,
            NodeKind::ExpressionFunctionBody => {
                self.update_with_expression(c, c.ast[Id::<ExpressionFunctionBody>::from_raw(node)].expression.raw());
            }
            NodeKind::ForElement => self.has_await |= c.ast[Id::<ForElement>::from_raw(node)].await_keyword.is_some(),
            NodeKind::ForStatement => {
                self.has_await |= c.ast[Id::<ForStatement>::from_raw(node)].await_keyword.is_some()
            }
            NodeKind::FunctionExpression => return,
            NodeKind::ReturnStatement => match c.ast[Id::<ReturnStatement>::from_raw(node)].expression {
                Some(expression) => self.update_with_expression(c, expression.raw()),
                None => self.every_return_has_value = false,
            },
            _ => {}
        }
        for child in c.ast.children(node) {
            self.visit(c, child);
        }
    }

    fn update_with_expression(&mut self, c: &LinterContext<'_>, expression: NodeId) {
        let Some(ctx) = rctx(c) else { return };
        let ty = c.static_type(expression);
        let ok = ty.is_some_and(|t| {
            matches!(*ctx.ty(t), TypeKind::Interface { .. })
                && is_dart_async_future_or_subtype(c, t)
                && ctx.nullability_suffix(t) == Nullability::None
        });
        if !ok {
            self.returns_only_future = false;
        }
    }
}

/// Dart `InterfaceType.isDartAsyncFutureOrSubtype`.
fn is_dart_async_future_or_subtype(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let future = ctx.tp.future_element();
    ctx.as_instance_of(ty, future.upcast()).is_some()
}

/// Dart `_checkBody`.
fn check_body(c: &LinterContext<'_>, body: NodeId, return_type: Option<TypeId>, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let (keyword_token, star) = match kind(c, body) {
        NodeKind::BlockFunctionBody => {
            let b = &c.ast[Id::<BlockFunctionBody>::from_raw(body)];
            (b.keyword, b.star)
        }
        NodeKind::ExpressionFunctionBody => {
            let b = &c.ast[Id::<ExpressionFunctionBody>::from_raw(body)];
            (b.keyword, b.star)
        }
        _ => (None, None),
    };
    let Some(async_keyword) = keyword_token.filter(|&k| lexeme(c, k) == "async") else { return };
    if star.is_some() {
        return;
    }
    let Some(body_context) = rctx_body_context(c, body) else { return };
    let mut visitor = HasAwait { has_await: false, every_return_has_value: true, returns_only_future: true };
    visitor.visit(c, body);
    if visitor.has_await {
        return;
    }
    let report = |out: &mut Vec<Diagnostic>| c.report_token(out, &diag::UNNECESSARY_ASYNC, async_keyword, &[]);
    let Some(return_type) = return_type else {
        report(out);
        return;
    };
    match *ctx.ty(return_type) {
        TypeKind::Void | TypeKind::Dynamic => {
            report(out);
            return;
        }
        _ => {}
    }
    if ctx.is_dart_async_future_or(return_type) {
        report(out);
        return;
    }
    if matches!(*ctx.ty(return_type), TypeKind::Interface { .. }) && ctx.is_dart_core_object(return_type) {
        report(out);
        return;
    }
    if !ctx.is_dart_async_future(return_type) {
        return;
    }
    if body_context.may_complete_normally {
        return;
    }
    if visitor.every_return_has_value && visitor.returns_only_future {
        report(out);
    }
}
