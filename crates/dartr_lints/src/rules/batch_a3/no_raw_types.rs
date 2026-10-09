// Dart source: pkg/linter/lib/src/rules/no_raw_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_named_type("no_raw_types", check);
}

fn escaping_parent(ctx: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(parent) = ctx.ast.parent(node) {
        if !matches!(
            ctx.ast.kind(parent),
            NodeKind::TypeArgumentList | NodeKind::NamedType
        ) {
            return parent;
        }
        node = parent;
    }
    node
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<NamedType>::from_raw(node)];
    if n.type_arguments.is_some() {
        return;
    }
    if ctx.ast.parent(node).is_some_and(|p| {
        ctx.ast.kind(p) == NodeKind::ConstructorName
            && ctx
                .ast
                .parent(p)
                .is_some_and(|g| ctx.ast.kind(g) == NodeKind::InstanceCreationExpression)
    }) {
        return;
    }
    if matches!(
        ctx.ast.kind(escaping_parent(ctx, node)),
        NodeKind::AsExpression
            | NodeKind::CastPattern
            | NodeKind::IsExpression
            | NodeKind::ObjectPattern
            | NodeKind::TypeLiteral
    ) {
        return;
    }
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let Some(ty) = resolved.tables.annotation_type.get(node).copied() else {
        return;
    };
    let args = match *resolved.ctx.ty(ty) {
        TypeKind::Interface { args, .. } => resolved.ctx.list(args),
        _ => return,
    };
    if args.contains(&dartr_element::TypeId::DYNAMIC) {
        let text = ctx.text(node);
        ctx.report_node(out, &diag::NO_RAW_TYPES, node, &[&text]);
    }
}
