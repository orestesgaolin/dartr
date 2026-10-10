// Dart source: pkg/linter/lib/src/rules/void_checks.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::AssignedVariablePattern,
        "void_checks",
        assigned_pattern,
    );
    r.add(NodeKind::AssignmentExpression, "void_checks", assignment);
    r.add(
        NodeKind::InstanceCreationExpression,
        "void_checks",
        creation,
    );
    r.add(NodeKind::MethodInvocation, "void_checks", invocation);
    r.add(NodeKind::ReturnStatement, "void_checks", return_statement);
}

fn first_type_argument(c: &LinterContext<'_>, ty: TypeId) -> Option<TypeId> {
    let ctx = rctx(c)?;
    ctx.interface_element(ty)?;
    ctx.type_arguments(ty).first().copied()
}

/// Dart `isTypeAcceptableWhenExpectingFutureOrVoid`.
fn acceptable_future_or_void(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    if matches!(*ctx.ty(ty), TypeKind::Dynamic) || acceptable_void(c, ty) {
        return true;
    }
    ctx.is_dart_async_future_or(ty)
        || (ctx.is_dart_async_future(ty)
            && first_type_argument(c, ty).is_some_and(|a| acceptable_future_or_void(c, a)))
}

/// Dart `isTypeAcceptableWhenExpectingVoid`.
fn acceptable_void(c: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    matches!(*ctx.ty(ty), TypeKind::Void | TypeKind::Never(_))
        || ctx.is_dart_core_null(ty)
        || (ctx.is_dart_async_future(ty)
            && first_type_argument(c, ty).is_some_and(|a| acceptable_void(c, a)))
}

/// Dart `_check`.
fn check_types(
    c: &LinterContext<'_>,
    expected: Option<TypeId>,
    ty: Option<TypeId>,
    node: NodeId,
    checked_node: Option<NodeId>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = rctx(c) else { return };
    let (Some(expected), Some(ty)) = (expected, ty) else {
        return;
    };
    let expected_void = matches!(*ctx.ty(expected), TypeKind::Void);
    if expected_void
        && !matches!(*ctx.ty(ty), TypeKind::Dynamic)
        && kind(c, node) == NodeKind::ReturnStatement
    {
        return;
    }
    if expected_void && !acceptable_void(c, ty) {
        c.report_node(out, &diag::VOID_CHECKS, node, &[]);
    } else if ctx.is_dart_async_future_or(expected)
        && first_type_argument(c, expected).is_some_and(|a| matches!(*ctx.ty(a), TypeKind::Void))
        && !acceptable_future_or_void(c, ty)
    {
        c.report_node(out, &diag::VOID_CHECKS, node, &[]);
    } else if let Some(checked) = checked_node
        && let Some(function) = c.ast.cast::<FunctionExpression>(checked)
        && kind(c, c.ast[function].body) != NodeKind::ExpressionFunctionBody
        && let (TypeKind::Function(e), TypeKind::Function(t)) = (*ctx.ty(expected), *ctx.ty(ty))
    {
        check_types(c, Some(e.ret), Some(t.ret), node, Some(checked), out);
    }
}

/// Dart `_checkArgs`.
fn check_args(c: &LinterContext<'_>, args: Vec<NodeId>, out: &mut Vec<Diagnostic>) {
    for arg in args {
        let expression = argument_expression(c, arg);
        if let Some(ty) = c.corresponding_parameter_type(expression) {
            check_types(
                c,
                Some(ty),
                c.static_type(expression),
                expression,
                None,
                out,
            );
        }
    }
}

fn assigned_pattern(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    let value_type = resolved
        .tables
        .pattern_info
        .get(node)
        .and_then(|i| i.matched_value_type);
    let Some(element) = c.element(node) else {
        return;
    };
    let tag = base(c, element).tag();
    if !(is_local_variable(base(c, element))
        || matches!(
            tag,
            dartr_element::Tag::FormalParameter
                | dartr_element::Tag::FieldFormalParameter
                | dartr_element::Tag::SuperFormalParameter
                | dartr_element::Tag::Field
                | dartr_element::Tag::TopLevelVariable
        ))
    {
        return;
    }
    check_types(c, element_type(c, element), value_type, node, None, out);
}

fn assignment(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
    let ty = resolved.tables.write_type.get(node).copied();
    let rhs = n.right_hand_side.raw();
    check_types(c, ty, c.static_type(rhs), node, Some(rhs), out);
}

fn creation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    if c.element(n.constructor_name).is_some() {
        check_args(c, arguments(c, n.argument_list), out);
    }
}

fn invocation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(resolved)) = (rctx(c), c.resolved) else {
        return;
    };
    let Some(&ty) = resolved.tables.invoke_type.get(node) else {
        return;
    };
    if matches!(*ctx.ty(ty), TypeKind::Function(_)) {
        let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
        check_args(c, arguments(c, n.argument_list), out);
    }
}

fn return_statement(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let expression = c.ast[Id::<ReturnStatement>::from_raw(node)]
        .expression
        .map(|e| e.raw());
    let ty = expression.and_then(|e| c.static_type(e));
    let Some(parent) = this_or_ancestor(c, node, |e| {
        matches!(
            kind(c, e),
            NodeKind::FunctionExpression
                | NodeKind::MethodDeclaration
                | NodeKind::FunctionDeclaration
        )
    }) else {
        return;
    };
    let expected = match kind(c, parent) {
        NodeKind::FunctionExpression => c.static_type(parent).and_then(|t| match *ctx.ty(t) {
            TypeKind::Function(f) => Some(f.ret),
            _ => None,
        }),
        _ => c
            .declared_element(parent)
            .map(|e| member::return_type(&ctx, ElemRef::Base(e))),
    };
    if kind(c, parent) == NodeKind::FunctionExpression
        && c.static_type(parent)
            .is_none_or(|t| !matches!(*ctx.ty(t), TypeKind::Function(_)))
    {
        return;
    }
    check_types(c, expected, ty, node, expression, out);
}
