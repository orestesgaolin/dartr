// Dart source: pkg/linter/lib/src/rules/prefer_collection_literals.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_instance_creation_expression("prefer_collection_literals", check);
    registry.add_method_invocation("prefer_collection_literals", check);
}

fn name<'a>(ctx: &'a LinterContext<'_>, id: Id<SimpleIdentifier>) -> &'a str {
    ctx.ast.tokens.lexeme(ctx.ast[id].token)
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::MethodInvocation => {
            let n = &ctx.ast[Id::<MethodInvocation>::from_raw(node)];
            if name(ctx, n.method_name) == "toSet"
                && n.target
                    .is_some_and(|t| ctx.ast.kind(t) == NodeKind::ListLiteral)
            {
                ctx.report_node(out, &diag::PREFER_COLLECTION_LITERALS, node, &[]);
            }
        }
        NodeKind::InstanceCreationExpression => {
            let n = &ctx.ast[Id::<InstanceCreationExpression>::from_raw(node)];
            let Some(resolved) = ctx.resolved else {
                return;
            };
            let Some(ty) = ctx.static_type(node) else {
                return;
            };
            let is_map = resolved.ctx.is_dart_core_map(ty);
            let is_set = resolved.ctx.is_dart_core_set(ty);
            if !is_map && !is_set {
                return;
            }
            let constructor = &ctx.ast[n.constructor_name];
            let constructor_name = constructor.name.map(|id| name(ctx, id));
            let args = ctx.ast.list(ctx.ast[n.argument_list].arguments);
            let should_report = constructor_name.is_none() && args.is_empty()
                || is_set
                    && matches!(constructor_name, Some("from" | "of"))
                    && args.len() == 1
                    && ctx.ast.kind(args[0]) == NodeKind::ListLiteral;
            if should_report {
                ctx.report_node(out, &diag::PREFER_COLLECTION_LITERALS, node, &[]);
            }
        }
        _ => {}
    }
}
