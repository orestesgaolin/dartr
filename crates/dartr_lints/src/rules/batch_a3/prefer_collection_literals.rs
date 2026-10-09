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

fn is_collection_type(
    resolved: crate::ResolvedLintContext<'_>,
    ty: dartr_element::TypeId,
    name: &str,
) -> bool {
    resolved.ctx.interface_element(ty).is_some_and(|element| {
        resolved.ctx.element_name(element.raw()) == Some(name)
            && resolved.ctx.element_library_name(element.raw()) == Some("dart.collection")
    })
}

fn declared_variable_context_type(
    ctx: &LinterContext<'_>,
    node: NodeId,
) -> Option<dartr_element::TypeId> {
    let variable = ctx
        .ast
        .parent(node)
        .and_then(|parent| ctx.ast.cast::<VariableDeclaration>(parent))?;
    if ctx.ast[variable].initializer?.raw() != node {
        return None;
    }
    let list = ctx
        .ast
        .parent(variable.raw())
        .and_then(|parent| ctx.ast.cast::<VariableDeclarationList>(parent))?;
    let annotation = ctx.ast[list].type_?;
    ctx.resolved?
        .tables
        .annotation_type
        .get(annotation.raw())
        .copied()
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
            if ctx
                .element(ctx.ast[n.constructor_name].type_.raw())
                .is_some_and(|element| {
                    dartr_typesystem::member::base_element(&resolved.ctx, element).kind()
                        == dartr_element::ElementKind::TypeAlias
                })
            {
                return;
            }
            let Some(ty) = ctx.static_type(node) else {
                return;
            };
            let is_hash_map = is_collection_type(resolved, ty, "LinkedHashMap");
            let is_hash_set = is_collection_type(resolved, ty, "LinkedHashSet");
            if let Some(context_type) = declared_variable_context_type(ctx, node)
                && (is_hash_map && is_collection_type(resolved, context_type, "LinkedHashMap")
                    || is_hash_set && is_collection_type(resolved, context_type, "LinkedHashSet"))
            {
                return;
            }
            let is_map = resolved.ctx.is_dart_core_map(ty) || is_hash_map;
            let is_set = resolved.ctx.is_dart_core_set(ty) || is_hash_set;
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
