// Dart source: pkg/linter/lib/src/rules/null_check_on_nullable_type_parameter.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::member;
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_null_assert_pattern("null_check_on_nullable_type_parameter", check);
    r.add_postfix_expression("null_check_on_nullable_type_parameter", check);
}
fn expected(ctx: &LinterContext<'_>, node: NodeId) -> Option<dartr_element::TypeId> {
    let p = ctx.ast.parent(node)?;
    match ctx.ast.kind(p) {
        NodeKind::AssignmentExpression => ctx.resolved?.tables.write_type.get(p).copied(),
        NodeKind::NamedArgument | NodeKind::ArgumentList => ctx
            .resolved?
            .tables
            .param_element
            .get(node)
            .copied()
            .map(|e| member::type_(&ctx.resolved.unwrap().ctx, e)),
        NodeKind::VariableDeclaration => {
            let list = ctx
                .ast
                .parent(p)
                .and_then(|x| ctx.ast.cast::<VariableDeclarationList>(x))?;
            let t = ctx.ast[list].type_?;
            ctx.resolved?.tables.annotation_type.get(t.raw()).copied()
        }
        NodeKind::ExpressionFunctionBody | NodeKind::ReturnStatement => {
            enclosing_return_type(ctx, node)
        }
        NodeKind::YieldStatement => {
            let return_type = enclosing_return_type(ctx, node)?;
            interface_type_argument(ctx, return_type, 0)
        }
        NodeKind::ListLiteral => interface_type_argument(ctx, ctx.static_type(p)?, 0),
        NodeKind::SetOrMapLiteral => interface_type_argument(ctx, ctx.static_type(p)?, 0),
        NodeKind::MapLiteralEntry => {
            let entry = &ctx.ast[Id::<MapLiteralEntry>::from_raw(p)];
            let literal = ctx.ast.parent(p)?;
            let index = usize::from(entry.value.raw() == node);
            interface_type_argument(ctx, ctx.static_type(literal)?, index)
        }
        _ => None,
    }
}

fn interface_type_argument(
    ctx: &LinterContext<'_>,
    ty: dartr_element::TypeId,
    index: usize,
) -> Option<dartr_element::TypeId> {
    let resolved = ctx.resolved?;
    let TypeKind::Interface { args, .. } = *resolved.ctx.ty(ty) else {
        return None;
    };
    resolved.ctx.list(args).get(index).copied()
}

fn enclosing_return_type(
    ctx: &LinterContext<'_>,
    mut node: NodeId,
) -> Option<dartr_element::TypeId> {
    while let Some(parent) = ctx.ast.parent(node) {
        if let Some(function) = ctx.ast.cast::<FunctionExpression>(parent) {
            let ty = ctx.static_type(function.raw())?;
            let TypeKind::Function(data) = *ctx.resolved?.ctx.ty(ty) else {
                return None;
            };
            return Some(data.ret);
        }
        if matches!(
            ctx.ast.kind(parent),
            NodeKind::FunctionDeclaration
                | NodeKind::MethodDeclaration
                | NodeKind::ConstructorDeclaration
                | NodeKind::PrimaryConstructorDeclaration
        ) {
            return Some(member::return_type(
                &ctx.resolved?.ctx,
                dartr_element::ElemRef::Base(ctx.declared_element(parent)?),
            ));
        }
        node = parent;
    }
    None
}
fn nullable_parameter(ctx: &LinterContext<'_>, ty: dartr_element::TypeId) -> bool {
    matches!(
        ctx.resolved.unwrap().ctx.ty(ty),
        TypeKind::TypeParameter { .. }
    ) && ctx.type_system().unwrap().is_nullable(ty)
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = ctx.resolved else {
        return;
    };
    match ctx.ast.kind(node) {
        NodeKind::NullAssertPattern => {
            let n = &ctx.ast[Id::<NullAssertPattern>::from_raw(node)];
            if r.tables
                .pattern_info
                .get(node)
                .and_then(|i| i.matched_value_type)
                .is_some_and(|t| nullable_parameter(ctx, t))
            {
                ctx.report_token(
                    out,
                    &diag::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER,
                    n.operator,
                    &[],
                );
            }
        }
        NodeKind::PostfixExpression => {
            let n = &ctx.ast[Id::<PostfixExpression>::from_raw(node)];
            if ctx.ast.tokens.lexeme(n.operator) != "!" {
                return;
            }
            let (Some(ty), Some(exp)) = (ctx.static_type(n.operand), expected(ctx, node)) else {
                return;
            };
            let ts = ctx.type_system().unwrap();
            if nullable_parameter(ctx, ty)
                && ts.is_potentially_nullable(exp)
                && ts.dart_eq(ts.promote_to_non_null(ty), ts.promote_to_non_null(exp))
            {
                ctx.report_token(
                    out,
                    &diag::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER,
                    n.operator,
                    &[],
                );
            }
        }
        _ => {}
    }
}
