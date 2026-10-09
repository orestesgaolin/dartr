// Dart source: pkg/linter/lib/src/rules/omit_local_variable_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{LocalFunctionElement, TopLevelFunctionElement, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_for_statement("omit_local_variable_types", check);
    registry.add_variable_declaration_statement("omit_local_variable_types", check);
}

fn depends_on_declared_type(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(invocation) = ctx.ast.cast::<MethodInvocation>(node) else {
        return false;
    };
    if ctx.ast[invocation].type_arguments.is_some() {
        return false;
    }
    let Some(resolved) = ctx.resolved else {
        return false;
    };
    let Some(element) = ctx.element(ctx.ast[invocation].method_name.raw()) else {
        return false;
    };
    let base = member::base_element(&resolved.ctx, element);
    if base.cast::<LocalFunctionElement>().is_none()
        && base.cast::<TopLevelFunctionElement>().is_none()
    {
        return false;
    }
    matches!(
        resolved.ctx.ty(member::return_type(&resolved.ctx, element)),
        TypeKind::TypeParameter { .. }
    )
}

fn check_list(
    ctx: &LinterContext<'_>,
    list: Id<VariableDeclarationList>,
    out: &mut Vec<Diagnostic>,
) {
    let n = &ctx.ast[list];
    let Some(annotation) = n.type_ else {
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
        || resolved.ctx.is_dart_core_null(declared)
    {
        return;
    }
    let ts = ctx.type_system().unwrap();
    for variable in ctx.ast.list(n.variables) {
        let Some(initializer) = ctx.ast[*variable].initializer else {
            return;
        };
        let Some(initializer_type) = ctx.static_type(initializer) else {
            return;
        };
        if !ts.dart_eq(initializer_type, declared) {
            return;
        }
        if ctx.ast.kind(initializer) == NodeKind::IntegerLiteral
            && !resolved.ctx.is_dart_core_int(declared)
        {
            return;
        }
        if depends_on_declared_type(ctx, initializer.raw()) {
            return;
        }
    }
    ctx.report_node(out, &diag::OMIT_LOCAL_VARIABLE_TYPES, annotation, &[]);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::VariableDeclarationStatement => check_list(
            ctx,
            ctx.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables,
            out,
        ),
        NodeKind::ForStatement => {
            let parts = ctx.ast[Id::<ForStatement>::from_raw(node)]
                .for_loop_parts
                .raw();
            if let Some(parts) = ctx.ast.cast::<ForPartsWithDeclarations>(parts) {
                check_list(ctx, ctx.ast[parts].variables, out);
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
                if matches!(resolved.ctx.ty(declared), TypeKind::Dynamic) {
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
                    ctx.report_node(out, &diag::OMIT_LOCAL_VARIABLE_TYPES, annotation, &[]);
                }
            }
        }
        _ => {}
    }
}
