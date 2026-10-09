// Dart source: pkg/linter/lib/src/rules/noop_primitive_operations.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{Nullability, TypeKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_adjacent_strings("noop_primitive_operations", check);
    registry.add_interpolation_expression("noop_primitive_operations", check);
    registry.add_method_invocation("noop_primitive_operations", check);
}

fn method_name<'a>(ctx: &'a LinterContext<'_>, n: &MethodInvocation) -> &'a str {
    ctx.ast.tokens.lexeme(ctx.ast[n.method_name].token)
}

fn check_to_string(ctx: &LinterContext<'_>, expression: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(invocation) = ctx.ast.cast::<MethodInvocation>(expression) else {
        return;
    };
    let n = &ctx.ast[invocation];
    if n.target.is_some()
        && ctx.ast.kind(n.target.unwrap()) != NodeKind::SuperExpression
        && method_name(ctx, n) == "toString"
        && ctx.ast.list(ctx.ast[n.argument_list].arguments).is_empty()
    {
        ctx.report_node(out, &diag::NOOP_PRIMITIVE_OPERATIONS, n.method_name, &[]);
    }
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::AdjacentStrings => {
            let strings = ctx
                .ast
                .list(ctx.ast[Id::<AdjacentStrings>::from_raw(node)].strings);
            for string in strings.iter().skip(1).take(strings.len().saturating_sub(2)) {
                if ctx
                    .ast
                    .cast::<SimpleStringLiteral>(*string)
                    .is_some_and(|s| ctx.ast[s].value.is_empty())
                {
                    ctx.report_node(out, &diag::NOOP_PRIMITIVE_OPERATIONS, *string, &[]);
                }
            }
        }
        NodeKind::InterpolationExpression => check_to_string(
            ctx,
            ctx.ast[Id::<InterpolationExpression>::from_raw(node)]
                .expression
                .raw(),
            out,
        ),
        NodeKind::MethodInvocation => {
            let n = &ctx.ast[Id::<MethodInvocation>::from_raw(node)];
            if n.target.is_none() {
                if let (Some(r), Some(e)) = (ctx.resolved, ctx.element(n.method_name.raw())) {
                    let base = member::base_element(&r.ctx, e);
                    let arguments = ctx.ast.list(ctx.ast[n.argument_list].arguments);
                    if r.ctx.is_element(base, "dart.core", "print") && arguments.len() == 1 {
                        let expression = ctx
                            .ast
                            .cast::<NamedArgument>(arguments[0])
                            .map_or(arguments[0].raw(), |a| ctx.ast[a].argument_expression.raw());
                        check_to_string(ctx, expression, out);
                    }
                }
                return;
            }
            let Some(target) = n.target else {
                return;
            };
            let Some(resolved) = ctx.resolved else {
                return;
            };
            let Some(ty) = ctx.static_type(target) else {
                return;
            };
            let name = method_name(ctx, n);
            if resolved.ctx.is_dart_core_string(ty)
                && matches!(
                    resolved.ctx.ty(ty),
                    TypeKind::Interface {
                        nullability: Nullability::None,
                        ..
                    }
                )
                && name == "toString"
                || resolved.ctx.is_dart_core_int(ty)
                    && matches!(name, "toInt" | "round" | "ceil" | "floor" | "truncate")
                || resolved.ctx.is_dart_core_double(ty) && name == "toDouble"
            {
                ctx.report_node(out, &diag::NOOP_PRIMITIVE_OPERATIONS, n.method_name, &[]);
            }
        }
        _ => {}
    }
}
