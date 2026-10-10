// Dart source: pkg/linter/lib/src/rules/only_throw_errors.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_throw_expression("only_throw_errors", check);
}

/// Dart `DartTypeExtension.typeForInterfaceCheck` (linter `extensions.dart`).
fn type_for_interface_check(
    ctx: &LinterContext<'_>,
    ty: dartr_element::TypeId,
) -> dartr_element::TypeId {
    let resolved = ctx.resolved.expect("resolved context");
    match *resolved.ctx.ty(ty) {
        TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } => match promoted_bound {
            Some(promoted) => type_for_interface_check(ctx, promoted),
            None => type_for_interface_check(
                ctx,
                resolved
                    .ctx
                    .get(param)
                    .bound
                    .get()
                    .unwrap_or(resolved.ctx.tp.object_question_type()),
            ),
        },
        _ => ctx.type_system().unwrap().extension_type_erasure(ty),
    }
}

/// Dart `_isThrowable`.
fn throwable(ctx: &LinterContext<'_>, ty: Option<dartr_element::TypeId>) -> bool {
    let (Some(resolved), Some(ty)) = (ctx.resolved, ty) else {
        return true;
    };
    let check = type_for_interface_check(ctx, ty);
    if matches!(*resolved.ctx.ty(check), TypeKind::Dynamic)
        || matches!(*resolved.ctx.ty(ty), TypeKind::Never(_))
    {
        return true;
    }
    // Dart `implementsAnyInterface`.
    let Some(element) = resolved.ctx.interface_element(check) else {
        return false;
    };
    let is_any_interface = |t: dartr_element::TypeId| {
        resolved.ctx.interface_element(t).is_some_and(|e| {
            matches!(
                resolved.ctx.element_name(e.raw()),
                Some("Exception" | "Error")
            ) && resolved.ctx.element_library_name(e.raw()) == Some("dart.core")
        })
    };
    is_any_interface(check)
        || dartr_typesystem::class_hierarchy::implemented_interfaces(&resolved.ctx, element)
            .iter()
            .any(|&t| is_any_interface(t))
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = ctx.ast[Id::<ThrowExpression>::from_raw(node)].expression;
    let literal = matches!(
        ctx.ast.kind(expression),
        NodeKind::BooleanLiteral
            | NodeKind::DoubleLiteral
            | NodeKind::IntegerLiteral
            | NodeKind::NullLiteral
            | NodeKind::SimpleStringLiteral
            | NodeKind::StringInterpolation
            | NodeKind::AdjacentStrings
            | NodeKind::SymbolLiteral
            | NodeKind::ListLiteral
            | NodeKind::SetOrMapLiteral
            | NodeKind::RecordLiteral
    );
    if literal || !throwable(ctx, ctx.static_type(expression)) {
        ctx.report_node(out, &diag::ONLY_THROW_ERRORS, expression, &[]);
    }
}
