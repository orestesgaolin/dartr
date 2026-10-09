// Dart source: pkg/linter/lib/src/rules/missing_whitespace_between_adjacent_strings.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_adjacent_strings("missing_whitespace_between_adjacent_strings", check);
}

fn pieces<'a>(ctx: &'a LinterContext<'_>, node: NodeId) -> Vec<&'a str> {
    match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => {
            vec![&ctx.ast[Id::<SimpleStringLiteral>::from_raw(node)].value]
        }
        NodeKind::StringInterpolation => ctx
            .ast
            .list(ctx.ast[Id::<StringInterpolation>::from_raw(node)].elements)
            .iter()
            .filter_map(|e| {
                ctx.ast
                    .cast::<InterpolationString>(*e)
                    .map(|s| &*ctx.ast[s].value)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn starts_ws(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    pieces(ctx, node)
        .first()
        .is_some_and(|s| s.is_empty() || s.starts_with([' ', '\n', '\r', '\t']))
}
fn ends_ws(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    pieces(ctx, node)
        .last()
        .is_some_and(|s| s.is_empty() || s.ends_with([' ', '\n', '\r', '\t']))
}
fn has_ws(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    pieces(ctx, node)
        .iter()
        .any(|s| s.contains([' ', '\n', '\r', '\t']))
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(arguments) = ctx
        .ast
        .parent(node)
        .and_then(|p| ctx.ast.cast::<ArgumentList>(p))
        && let Some(invocation) = ctx.ast.parent(arguments)
    {
        let skip = match ctx.ast.kind(invocation) {
            NodeKind::MethodInvocation => {
                let n = &ctx.ast[Id::<MethodInvocation>::from_raw(invocation)];
                n.target.is_none()
                    && matches!(
                        ctx.ast.tokens.lexeme(ctx.ast[n.method_name].token),
                        "RegExp" | "matches"
                    )
            }
            NodeKind::InstanceCreationExpression => {
                let n = &ctx.ast[Id::<InstanceCreationExpression>::from_raw(invocation)];
                ctx.resolved
                    .zip(ctx.element(n.constructor_name.raw()))
                    .and_then(|(r, e)| {
                        r.ctx
                            .element_data(member::base_element(&r.ctx, e))?
                            .enclosing
                            .and_then(|owner| r.ctx.element_name(owner))
                    })
                    == Some("RegExp")
            }
            _ => false,
        };
        if skip {
            return;
        }
    }
    let strings = ctx
        .ast
        .list(ctx.ast[Id::<AdjacentStrings>::from_raw(node)].strings);
    for pair in strings.windows(2) {
        let current = pair[0].raw();
        let next = pair[1].raw();
        if !ends_ws(ctx, current) && !starts_ws(ctx, next) && has_ws(ctx, current) {
            ctx.report_node(
                out,
                &diag::MISSING_WHITESPACE_BETWEEN_ADJACENT_STRINGS,
                current,
                &[],
            );
        }
    }
}
