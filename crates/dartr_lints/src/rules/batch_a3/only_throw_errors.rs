// Dart source: pkg/linter/lib/src/rules/only_throw_errors.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_throw_expression("only_throw_errors", check);
}

fn throwable(ctx: &LinterContext<'_>, ty: dartr_element::TypeId) -> bool {
    let Some(resolved) = ctx.resolved else {
        return true;
    };
    let mut ty = ty;
    loop {
        ty = ctx.type_system().unwrap().extension_type_erasure(ty);
        ty = match *resolved.ctx.ty(ty) {
            TypeKind::Dynamic | TypeKind::Never(_) => return true,
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => promoted_bound
                .or_else(|| resolved.ctx.get(param).bound.get())
                .unwrap_or(resolved.ctx.tp.object_question_type()),
            TypeKind::Interface { .. } => break,
            _ => return true,
        };
    }
    std::iter::once(ty)
        .chain(resolved.ctx.all_supertypes(ty))
        .any(|t| {
            resolved.ctx.interface_element(t).is_some_and(|e| {
                matches!(
                    resolved.ctx.element_name(e.raw()),
                    Some("Exception" | "Error")
                ) && resolved.ctx.element_library_name(e.raw()) == Some("dart.core")
            })
        })
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
    if literal
        || ctx
            .static_type(expression)
            .is_some_and(|t| !throwable(ctx, t))
    {
        ctx.report_node(out, &diag::ONLY_THROW_ERRORS, expression, &[]);
    }
}
