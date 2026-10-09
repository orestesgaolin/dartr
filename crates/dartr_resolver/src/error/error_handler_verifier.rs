// Dart source: pkg/analyzer/lib/src/error/error_handler_verifier.dart

//! Reports invalid functions passed as error handlers (Dart
//! `ErrorHandlerVerifier`).
//!
//! Functions must either accept exactly one positional parameter, or
//! exactly two positional parameters. The one parameter (or the first
//! parameter) must have a type of `dynamic`, `Object`, or `Object?`. If a
//! second parameter is accepted, it must have a type of `StackTrace`.
//!
//! A function is checked if it is passed as the first argument to
//! `Future.catchError`, as the `onError` named argument to `Future.then`,
//! as the first argument to `Stream.handleError`, as the `onError` argument
//! to `Stream.listen`, or as the first argument to
//! `StreamSubscription.onError`. A function passed to `Future<T>.catchError`
//! must return `FutureOr<T>`, and any return statements in a function
//! literal must return a value of type `FutureOr<T>`.
//!
//! The best practices verifier calls [`verify_method_invocation`] (Dart
//! `BestPracticesVerifier.visitMethodInvocation`). The return statements of
//! a function literal are checked here with the context that Dart gives the
//! `ReturnTypeVerifier` (`EnclosingExecutableContext(...,
//! catchErrorOnErrorReturnType:)` / `thenOnErrorReturnType:`).

use dartr_ast::{
    Ast, AstVisitor, Expression, ExpressionFunctionBody, FunctionExpression, Id, MethodInvocation,
    NamedArgument, NodeId, ReturnStatement,
};
use dartr_diagnostics::{LocatableDiagnostic, TypeArg, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::type_ext::is_named;

use super::VerifierHost;
use crate::ast_ext::method_invocation_real_target;

/// Dart `verifyMethodInvocation(node)`.
pub fn verify_method_invocation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<MethodInvocation>) {
    let ast = host.ast();
    let Some(target) = method_invocation_real_target(ast, node) else {
        return;
    };
    let arguments: Vec<NodeId> = ast
        .list_raw(ast[ast[node].argument_list].arguments)
        .to_vec();
    let Some(&first) = arguments.first() else {
        return;
    };
    let Some(target_type) = host.static_type(target) else {
        return;
    };
    let ctx = host.ctx();
    let method_name = ast
        .tokens
        .lexeme(ast[ast[node].method_name].token)
        .to_string();

    // Dart `node.argumentList.arguments.whereType<NamedArgument>()
    // .firstWhereOrNull((a) => a.name.lexeme == 'onError')`.
    let on_error_argument = |ast: &Ast| -> Option<Id<NamedArgument>> {
        arguments.iter().find_map(|&a| {
            let named = ast.cast::<NamedArgument>(a)?;
            (ast.tokens.lexeme(ast[named].name) == "onError").then_some(named)
        })
    };

    if method_name == "catchError" && ctx.is_dart_async_future(target_type) {
        if ast.cast::<NamedArgument>(first).is_some() {
            // This implies that no positional arguments are passed.
            return;
        }
        check_future_catch_error_on_error(host, target, Id::from_raw(first));
        return;
    }

    if method_name == "then" && ctx.is_dart_async_future(target_type) {
        let Some(callback) = on_error_argument(ast) else {
            return;
        };
        let expression = ast[callback].argument_expression;
        check_future_then_on_error(host, node, expression);
        return;
    }

    // Dart: `handleError` on a `Stream`, `listen` on a `Stream` (the
    // `onError` argument), and `onError` on a `StreamSubscription`.
    let callback = if method_name == "handleError"
        && is_dart_core_async_type(host, target_type, "Stream")
        || method_name == "onError"
            && is_dart_core_async_type(host, target_type, "StreamSubscription")
    {
        if ast.cast::<NamedArgument>(first).is_some() {
            // This implies that no positional arguments are passed.
            return;
        }
        Some((first, Id::<Expression>::from_raw(first)))
    } else if method_name == "listen" && is_dart_core_async_type(host, target_type, "Stream") {
        let Some(callback) = on_error_argument(ast) else {
            return;
        };
        Some((callback.raw(), ast[callback].argument_expression))
    } else {
        None
    };
    if let Some((callback, expression)) = callback {
        let Some(callback_type) = host.static_type(expression) else {
            return;
        };
        if matches!(ctx.ty(callback_type), TypeKind::Function(_)) {
            let check_first_parameter_type = ast.cast::<FunctionExpression>(expression).is_some();
            check_error_handler_function_type(
                host,
                callback,
                callback_type,
                ctx.tp.void_type(),
                check_first_parameter_type,
            );
        }
        // [callbackType] might be dart:core's Function, or something not
        // assignable to Function, in which case an error is reported
        // elsewhere.
    }
}

/// Dart `_checkErrorHandlerFunctionType(errorNode, expression,
/// expressionType, expectedFunctionReturnType, checkFirstParameterType:)`:
/// checks that a function of type [expression_type] is a valid error
/// handler. Only checks the first parameter type if
/// [check_first_parameter_type].
fn check_error_handler_function_type<'a, H: VerifierHost<'a>>(
    host: &mut H,
    error_node: NodeId,
    expression_type: TypeId,
    expected_function_return_type: TypeId,
    check_first_parameter_type: bool,
) {
    let ctx = host.ctx();
    let report = |host: &mut H| {
        let d = diag::argument_type_not_assignable_to_error_handler(
            type_arg(&ctx, expression_type),
            type_arg(&ctx, expected_function_return_type),
        );
        let d = host.at(d, error_node);
        host.report(d);
    };

    let TypeKind::Function(function) = ctx.ty(expression_type) else {
        return;
    };
    let parameters = ctx.list(function.params);
    let Some(first_parameter) = parameters.first() else {
        return report(host);
    };
    let ts = host.type_system();
    let invalid_first = is_named(first_parameter.kind)
        || check_first_parameter_type
            && !ts.is_subtype_of(ctx.tp.object_type(), first_parameter.ty);
    let invalid_rest = match parameters.len() {
        2 => {
            let second_parameter = &parameters[1];
            is_named(second_parameter.kind)
                || !ts.is_subtype_of(ctx.tp.stack_trace_type(), second_parameter.ty)
        }
        n => n > 2,
    };
    if invalid_first || invalid_rest {
        report(host);
    }
}

/// Dart `_checkFutureCatchErrorOnError(target, callback)`: checks the
/// `onError` argument given to `Future.catchError`.
fn check_future_catch_error_on_error<'a, H: VerifierHost<'a>>(
    host: &mut H,
    target: Id<Expression>,
    callback: Id<Expression>,
) {
    let ctx = host.ctx();
    let Some(target_type) = host.static_type(target) else {
        return;
    };
    let Some(&target_future_type) = ctx.type_arguments(target_type).first() else {
        return;
    };
    check_on_error(host, callback, target_future_type, OnError::CatchError);
}

/// Dart `_checkFutureThenOnError(node, callback)`.
fn check_future_then_on_error<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<MethodInvocation>,
    callback: Id<Expression>,
) {
    let ctx = host.ctx();
    let Some(node_type) = host.static_type(node) else {
        return;
    };
    let Some(&target_future_type) = ctx.type_arguments(node_type).first() else {
        return;
    };
    check_on_error(host, callback, target_future_type, OnError::Then);
}

/// Which error handler is checked: `Future.catchError` or the `onError`
/// argument of `Future.then`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OnError {
    CatchError,
    Then,
}

/// The common part of Dart `_checkFutureCatchErrorOnError` and
/// `_checkFutureThenOnError`.
fn check_on_error<'a, H: VerifierHost<'a>>(
    host: &mut H,
    callback: Id<Expression>,
    target_future_type: TypeId,
    kind: OnError,
) {
    let ctx = host.ctx();
    let expected_return_type = ctx.tp.future_or_type(&ctx, target_future_type);
    let Some(callback_type) = host.static_type(callback) else {
        return;
    };
    let TypeKind::Function(function) = ctx.ty(callback_type) else {
        // If [callback] is not even a Function, then ErrorVerifier will have
        // reported this.
        return;
    };
    if let Some(function_expression) = host.ast().cast::<FunctionExpression>(callback) {
        check_error_handler_function_type(
            host,
            callback.raw(),
            callback_type,
            expected_return_type,
            true,
        );

        if matches!(ctx.ty(target_future_type), TypeKind::Void) {
            return;
        }

        if matches!(ctx.ty(function.ret), TypeKind::Void)
            && is_void_or_dynamic(host, target_future_type)
        {
            // Special case for `void`: A function returning `void` is allowed
            // for the cases where the expected type is `void`, `dynamic`, or
            // `Null`.
            return;
        }

        // Dart `callback.body.accept(_ReturnStatementVerifier(...))` with
        // `EnclosingExecutableContext(element, isAsynchronous: true,
        // isGenerator: false, catchErrorOnErrorReturnType: /
        // thenOnErrorReturnType: expectedReturnType)`.
        let ast = host.ast();
        let body = ast[function_expression].body;
        let mut visitor = ReturnStatementVisitor::default();
        ast.accept(body, &mut visitor);
        for found in visitor.found {
            match found {
                Found::ExpressionFunctionBody(node) => {
                    verify_expression_function_body(host, node, expected_return_type, kind);
                }
                Found::ReturnStatement(node) => {
                    verify_return_statement(host, node, expected_return_type, kind);
                }
            }
        }
    } else {
        let d = match kind {
            OnError::CatchError => diag::return_type_invalid_for_catch_error,
            OnError::Then => diag::return_type_invalid_for_then,
        };
        check_return_type(host, target_future_type, function.ret, callback, d);
        check_error_handler_function_type(
            host,
            callback.raw(),
            callback_type,
            expected_return_type,
            kind == OnError::CatchError,
        );
    }
}

/// Dart `_checkReturnType(targetType, functionReturnType, callback,
/// diagnostic)`.
fn check_return_type<'a, H: VerifierHost<'a>>(
    host: &mut H,
    target_type: TypeId,
    function_return_type: TypeId,
    callback: Id<Expression>,
    diagnostic: fn(TypeArg, TypeArg) -> LocatableDiagnostic,
) {
    let ctx = host.ctx();
    let ts = host.type_system();
    if matches!(ctx.ty(function_return_type), TypeKind::Void)
        && (is_void_or_dynamic(host, target_type) || ts.is_nullable(target_type))
    {
        // Special case for `void`: A function returning `void` is allowed
        // for the cases where the expected type is `void`, `dynamic`, or a
        // nullable type.
        return;
    }
    let expected_type = ctx.tp.future_or_type(&ctx, target_type);
    if !ts.is_assignable_to(
        function_return_type,
        expected_type,
        host.options().strict_casts,
    ) {
        let d = diagnostic(
            type_arg(&ctx, function_return_type),
            type_arg(&ctx, expected_type),
        );
        let d = host.at(d, callback);
        host.report(d);
    }
}

/// Dart `_isDartCoreAsyncType(type, typeName)`: whether [ty] is the
/// interface type named [type_name] declared in `dart:async`.
fn is_dart_core_async_type<'a, H: VerifierHost<'a>>(host: &H, ty: TypeId, type_name: &str) -> bool {
    host.ctx().is_interface_of(ty, "dart.async", type_name)
}

/// Dart `_isVoidOrDynamic(type)`.
fn is_void_or_dynamic<'a, H: VerifierHost<'a>>(host: &H, ty: TypeId) -> bool {
    let ctx = host.ctx();
    matches!(
        ctx.ty(ty),
        TypeKind::Void | TypeKind::Dynamic | TypeKind::Invalid
    ) || ctx.is_dart_core_null(ty)
}

/// A node that Dart `_ReturnStatementVerifier` gives to the
/// `ReturnTypeVerifier`.
enum Found {
    ExpressionFunctionBody(Id<ExpressionFunctionBody>),
    ReturnStatement(Id<ReturnStatement>),
}

/// Dart `_ReturnStatementVerifier` (a `RecursiveAstVisitor`): visits a
/// function body, looking for return statements; collects them in visit
/// order.
#[derive(Default)]
struct ReturnStatementVisitor {
    found: Vec<Found>,
}

impl AstVisitor for ReturnStatementVisitor {
    fn visit_expression_function_body(&mut self, ast: &Ast, node: Id<ExpressionFunctionBody>) {
        self.found.push(Found::ExpressionFunctionBody(node));
        ast.visit_children(node, self);
    }

    fn visit_function_expression(&mut self, _ast: &Ast, _node: Id<FunctionExpression>) {
        // Do not visit within [node]. We have no interest in return
        // statements within.
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        self.found.push(Found::ReturnStatement(node));
        ast.visit_children(node, self);
    }
}

/// Dart `ReturnTypeVerifier.verifyExpressionFunctionBody(node)` in the
/// `onError` context (asynchronous, not a generator, return type
/// [return_type]).
fn verify_expression_function_body<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<ExpressionFunctionBody>,
    return_type: TypeId,
    kind: OnError,
) {
    // This enables concise declarations of void functions.
    let flattened = host.type_system().flatten(return_type);
    if matches!(host.ctx().ty(flattened), TypeKind::Void) {
        return;
    }
    let expression = host.ast()[node].expression;
    check_return_expression(host, expression, return_type, kind);
}

/// Dart `ReturnTypeVerifier.verifyReturnStatement(statement)` in the
/// `onError` context.
fn verify_return_statement<'a, H: VerifierHost<'a>>(
    host: &mut H,
    statement: Id<ReturnStatement>,
    return_type: TypeId,
    kind: OnError,
) {
    match host.ast()[statement].expression {
        // Dart `_checkReturnWithoutValue(statement)` (asynchronous).
        None => {
            let t_v = host.type_system().future_value_type(return_type);
            if !is_void_dynamic_or_null(host, t_v) {
                let token = host.ast()[statement].return_keyword;
                let d = host.at_token(diag::return_without_value(), token);
                host.report(d);
            }
        }
        Some(expression) => check_return_expression(host, expression, return_type, kind),
    }
}

/// Dart `ReturnTypeVerifier._checkReturnExpression(expression)` in the
/// `onError` context (the asynchronous case).
fn check_return_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    expression: Id<Expression>,
    t: TypeId,
    kind: OnError,
) {
    let ctx = host.ctx();
    let ts = host.type_system();
    let s = host.static_type(expression).unwrap_or(TypeId::DYNAMIC);
    let report_type_error = |host: &mut H| {
        let (actual, expected) = (type_arg(&ctx, s), type_arg(&ctx, t));
        let d = match kind {
            OnError::CatchError => diag::return_of_invalid_type_from_catch_error(actual, expected),
            OnError::Then => diag::return_of_invalid_type_from_then(actual, expected),
        };
        let d = host.at(d, expression);
        host.report(d);
    };

    let t_v = ts.future_value_type(t);
    let flatten_s = ts.flatten(s);
    // It is a compile-time error if `flatten(T)` is `void`, and
    // `flatten(S)` is neither `void`, `dynamic`, nor `Null`.
    if matches!(ctx.ty(t_v), TypeKind::Void) && !is_void_dynamic_or_null(host, flatten_s) {
        return report_type_error(host);
    }
    // It is a compile-time error if `flatten(S)` is `void`, and `flatten(T)`
    // is neither `void`, `dynamic`.
    if matches!(ctx.ty(flatten_s), TypeKind::Void) {
        if !matches!(
            ctx.ty(t_v),
            TypeKind::Void | TypeKind::Dynamic | TypeKind::Invalid
        ) {
            report_type_error(host);
        }
        return;
    }
    // It is a compile-time error if `flatten(S)` is not `void`, and
    // `Future<flatten(S)>` is not assignable to `T`.
    if !ts.is_assignable_to(s, t_v, host.options().strict_casts)
        && !ts.is_subtype_of(flatten_s, t_v)
    {
        report_type_error(host);
    }
}

/// Dart `ReturnTypeVerifier._isVoidDynamicOrNull(type)`.
fn is_void_dynamic_or_null<'a, H: VerifierHost<'a>>(host: &H, ty: TypeId) -> bool {
    let ctx = host.ctx();
    matches!(
        ctx.ty(ty),
        TypeKind::Void | TypeKind::Dynamic | TypeKind::Invalid
    ) || ctx.is_dart_core_null(ty)
}
