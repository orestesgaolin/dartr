// Dart source: pkg/linter/lib/src/rules/omit_obvious_local_variable_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_for_statement("omit_obvious_local_variable_types", check);
    registry.add_variable_declaration_statement("omit_obvious_local_variable_types", check);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = match ctx.ast.kind(node) {
        NodeKind::VariableDeclarationStatement => {
            Some(ctx.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables)
        }
        NodeKind::ForStatement => {
            let parts = ctx.ast[Id::<ForStatement>::from_raw(node)]
                .for_loop_parts
                .raw();
            if let Some(parts) = ctx.ast.cast::<ForPartsWithDeclarations>(parts) {
                Some(ctx.ast[parts].variables)
            } else if let Some(parts) = ctx.ast.cast::<ForEachPartsWithDeclaration>(parts) {
                let parts = &ctx.ast[parts];
                let Some(annotation) = ctx.ast[parts.loop_variable].type_ else {
                    return;
                };
                let Some(resolved) = ctx.resolved else {
                    return;
                };
                let Some(declared) = resolved
                    .tables
                    .annotation_type
                    .get(annotation.raw())
                    .copied()
                else {
                    return;
                };
                if matches!(resolved.ctx.ty(declared), TypeKind::Dynamic)
                    || !super::omit_obvious_property_types::is_obvious(ctx, parts.iterable.raw())
                {
                    return;
                }
                let Some(iterable) = ctx.static_type(parts.iterable) else {
                    return;
                };
                let Some(iterable) = resolved
                    .ctx
                    .as_instance_of(iterable, resolved.ctx.tp.iterable_element().upcast())
                else {
                    return;
                };
                if resolved.ctx.type_arguments(iterable).first() == Some(&declared) {
                    ctx.report_node(
                        out,
                        &diag::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES,
                        annotation,
                        &[],
                    );
                }
                return;
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(list) = list {
        super::omit_obvious_property_types::check_list(
            ctx,
            list,
            &diag::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES,
            out,
        );
    }
}
