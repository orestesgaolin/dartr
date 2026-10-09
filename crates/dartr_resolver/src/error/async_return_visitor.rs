// Dart source: pkg/analyzer/lib/src/error/async_return_visitor.dart

//! Finds `return` statements and `=>` bodies in `async` functions that
//! return a `Future` without awaiting it (Dart `AsyncReturnVisitor`).
//!
//! The error verifier uses it with `withinTryBlock: true`
//! ([`report_missing_await_in_try_block`]); the `async_return_with_no_await`
//! lint uses it with `withinTryBlock: false`. The Dart visitor reports
//! through a callback (`reportAtToken`); here the `visit_*` methods return
//! the token to report at.

use dartr_ast::{
    Ast, Block, BlockFunctionBody, ExpressionFunctionBody, FunctionBody, FunctionDeclaration,
    FunctionExpression, Id, MethodDeclaration, NodeId, ReturnStatement, TryStatement,
};
use dartr_diagnostics::diag;
use dartr_element::{ElemRef, TypeId, TypeKind};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;

use super::VerifierHost;
use crate::ast_ext::{function_body_is_asynchronous, function_body_is_generator};

/// Dart `AsyncReturnVisitor` (a `SimpleAstVisitor`).
#[derive(Clone, Copy, Debug, Default)]
pub struct AsyncReturnVisitor {
    /// Dart `_withinTryBlock`.
    pub within_try_block: bool,
}

impl AsyncReturnVisitor {
    /// Dart `visitExpressionFunctionBody(node)`: the token to report at
    /// (`=>`), if any.
    pub fn visit_expression_function_body<'a, H: VerifierHost<'a>>(
        &self,
        host: &H,
        node: Id<ExpressionFunctionBody>,
    ) -> Option<TokenId> {
        if self.within_try_block {
            return None;
        }
        let ast = host.ast();
        let expression = ast[node].expression;
        let expression_type = host.static_type(expression).unwrap_or(TypeId::DYNAMIC);
        let body = enclosing_function_body(ast, node.raw());
        report(host, body, expression_type, ast[node].function_definition)
    }

    /// Dart `visitReturnStatement(node)`: the token to report at
    /// (`return`), if any.
    pub fn visit_return_statement<'a, H: VerifierHost<'a>>(
        &self,
        host: &H,
        node: Id<ReturnStatement>,
    ) -> Option<TokenId> {
        let ast = host.ast();
        let expression = ast[node].expression?;
        if self.within_try_block != is_within_try_block(ast, node) {
            return None;
        }
        let expression_type = host.static_type(expression).unwrap_or(TypeId::DYNAMIC);
        let body = enclosing_function_body(ast, node.raw());
        report(host, body, expression_type, ast[node].return_keyword)
    }
}

/// Dart `ErrorVerifier._reportMissingAwaitInTryBlock(node)`:
/// `node.accept(AsyncReturnVisitor(withinTryBlock: true, reportAtToken:
/// _reportUnawaitedReturnInTryBlock))`.
pub fn report_missing_await_in_try_block<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<ReturnStatement>,
) {
    let visitor = AsyncReturnVisitor {
        within_try_block: true,
    };
    if let Some(token) = visitor.visit_return_statement(host, node) {
        let d = host.at_token(diag::unawaited_return_in_try_block(), token);
        host.report(d);
    }
}

/// Dart `node.withAncestors.whereType<FunctionBody>().firstOrNull`.
fn enclosing_function_body(ast: &Ast, node: NodeId) -> Option<NodeId> {
    let mut current = Some(node);
    while let Some(n) = current {
        if ast.cast::<FunctionBody>(n).is_some() {
            return Some(n);
        }
        current = ast.parent(n);
    }
    None
}

/// Dart `_report(body, expressionType, reportAt)`: the token to report at,
/// if the expression of type [expression_type] is a future that an `async`
/// body returns without awaiting it.
fn report<'a, H: VerifierHost<'a>>(
    host: &H,
    body: Option<NodeId>,
    expression_type: TypeId,
    report_at: TokenId,
) -> Option<TokenId> {
    let body = body?;
    let ctx = host.ctx();
    if matches!(
        ctx.ty(expression_type),
        TypeKind::Dynamic | TypeKind::Invalid
    ) {
        return None;
    }
    if ctx
        .as_instance_of(expression_type, ctx.tp.future_element().upcast())
        .is_none()
        && ctx
            .as_instance_of(expression_type, ctx.tp.future_or_element().upcast())
            .is_none()
    {
        return None;
    }
    let ast = host.ast();
    if function_body_is_generator(ast, body) {
        return None;
    }
    if function_body_is_asynchronous(ast, body) {
        let return_type = body_return_type(host, body)?;
        let ts = host.type_system();
        let expected = ctx
            .tp
            .future_or_type(&ctx, ts.future_value_type(return_type));
        // Dart `TypeSystem.isAssignableTo` (public API: `strictCasts:
        // false`).
        if ts.is_assignable_to(expression_type, expected, false) {
            return Some(report_at);
        }
    }
    None
}

/// Dart `ReturnStatement.isWithinTryBlock` (extension).
fn is_within_try_block(ast: &Ast, node: Id<ReturnStatement>) -> bool {
    let mut current = Some(node.raw());
    while let Some(ancestor) = current {
        if ast.cast::<Block>(ancestor).is_some()
            && let Some(parent) = ast.parent(ancestor)
        {
            if ast.cast::<BlockFunctionBody>(parent).is_some() {
                return false;
            }
            if let Some(t) = ast.cast::<TryStatement>(parent)
                && ast[t].body.raw() == ancestor
            {
                return true;
            }
        }
        current = ast.parent(ancestor);
    }
    false
}

/// Dart `FunctionBody.returnType` (extension): the return type of the
/// element of the method, function, or function expression of [body].
fn body_return_type<'a, H: VerifierHost<'a>>(host: &H, body: NodeId) -> Option<TypeId> {
    let ast = host.ast();
    let parent = ast.parent(body)?;
    if ast.cast::<MethodDeclaration>(parent).is_none()
        && ast.cast::<FunctionDeclaration>(parent).is_none()
        && ast.cast::<FunctionExpression>(parent).is_none()
    {
        return None;
    }
    let ctx = host.ctx();
    let fragment = *host.tables().declared_fragment.get(parent)?;
    let element = *ctx.fragment_data(fragment)?.element.try_get()?;
    Some(dartr_typesystem::member::return_type(
        &ctx,
        ElemRef::Base(element),
    ))
}
