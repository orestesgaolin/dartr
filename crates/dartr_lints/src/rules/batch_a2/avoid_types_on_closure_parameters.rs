// Dart source: pkg/linter/lib/src/rules/avoid_types_on_closure_parameters.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, TypeId, TypeKind};
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::FunctionExpression,
        "avoid_types_on_closure_parameters",
        check,
    );
}
fn approximate_context_type(context: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let resolved = context.resolved?;
    if let Some(parameter) = resolved.tables.param_element.get(node).copied() {
        return Some(member::type_(&resolved.ctx, parameter));
    }
    let parent = context.ast.parent(node)?;
    match context.ast.kind(parent) {
        NodeKind::VariableDeclaration => context
            .declared_element(parent)
            .map(|e| member::type_(&resolved.ctx, ElemRef::Base(e))),
        NodeKind::AssignmentExpression => {
            let assignment = &context.ast[context.ast.cast::<AssignmentExpression>(parent)?];
            (assignment.right_hand_side.raw() == node)
                .then(|| context.static_type(assignment.left_hand_side))
                .flatten()
        }
        _ => None,
    }
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let Some(context_type) = approximate_context_type(context, node) else {
        return;
    };
    if !matches!(resolved.ctx.ty(context_type), TypeKind::Function(_)) {
        return;
    }
    let function = &context.ast[context.ast.cast::<FunctionExpression>(node).unwrap()];
    let Some(parameters) = function.parameters else {
        return;
    };
    for &parameter_node in context.ast.list(context.ast[parameters].parameters) {
        let Some(parameter_id) = context.ast.cast::<RegularFormalParameter>(parameter_node) else {
            continue;
        };
        let parameter = &context.ast[parameter_id];
        if parameter.function_typed_suffix.is_some() {
            context.report_node(
                out,
                &diag::AVOID_TYPES_ON_CLOSURE_PARAMETERS,
                parameter_node,
                &[],
            );
        } else if let Some(annotation) = parameter.type_
            && super::helpers::annotation_type(context, annotation.raw())
                .is_some_and(|ty| !matches!(resolved.ctx.ty(ty), TypeKind::Dynamic))
        {
            context.report_node(
                out,
                &diag::AVOID_TYPES_ON_CLOSURE_PARAMETERS,
                annotation,
                &[],
            );
        }
    }
}
