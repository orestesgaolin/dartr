// Dart source: pkg/linter/lib/src/rules/avoid_types_on_closure_parameters.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::FunctionExpression,
        "avoid_types_on_closure_parameters",
        check,
    );
}
fn approximate_context_type(context: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let resolved = context.resolved?;
    let mut child = node;
    let mut parent = context.ast.parent(child);
    loop {
        match parent.map(|parent| context.ast.kind(parent)) {
            Some(NodeKind::ParenthesizedExpression) => {
                child = parent?;
                parent = context.ast.parent(child);
            }
            Some(NodeKind::CascadeExpression)
                if context
                    .ast
                    .cast::<CascadeExpression>(parent?)
                    .is_some_and(|cascade| context.ast[cascade].target.raw() == child) =>
            {
                child = parent?;
                parent = context.ast.parent(child);
            }
            _ => break,
        }
    }
    let parent = parent?;
    match context.ast.kind(parent) {
        // Dart `correspondingParameter?.type ?? InvalidTypeImpl.instance`.
        NodeKind::ArgumentList => Some(
            context
                .corresponding_parameter_type(node)
                .unwrap_or(TypeId::INVALID),
        ),
        NodeKind::NamedArgument => Some(
            context
                .corresponding_parameter_type(parent)
                .unwrap_or(TypeId::INVALID),
        ),
        NodeKind::VariableDeclaration => {
            let list = context
                .ast
                .parent(parent)
                .and_then(|parent| context.ast.cast::<VariableDeclarationList>(parent))?;
            context.ast[list]
                .type_
                .and_then(|annotation| super::helpers::annotation_type(context, annotation.raw()))
        }
        NodeKind::AssignmentExpression | NodeKind::ConditionalExpression => {
            context.static_type(parent)
        }
        NodeKind::ConstructorFieldInitializer => {
            let initializer =
                &context.ast[context.ast.cast::<ConstructorFieldInitializer>(parent)?];
            context
                .element(initializer.field_name)
                .map(|element| member::type_(&resolved.ctx, element))
        }
        NodeKind::ExpressionFunctionBody => expression_body_return_type(context, parent),
        NodeKind::ReturnStatement | NodeKind::YieldStatement => {
            let body = super::helpers::nearest_function_body(context.ast, parent)?;
            expected_body_value_type(context, body)
        }
        _ => None,
    }
}

fn expression_body_return_type(context: &LinterContext<'_>, body: NodeId) -> Option<TypeId> {
    let owner = context.ast.parent(body)?;
    match context.ast.kind(owner) {
        NodeKind::FunctionExpression => {
            let owner_parent = context.ast.parent(owner)?;
            if context.ast.kind(owner_parent) == NodeKind::FunctionDeclaration {
                declared_return_type(context, owner_parent)
            } else {
                let context_type = approximate_context_type(context, owner)?;
                let TypeKind::Function(function) = context.resolved?.ctx.ty(context_type) else {
                    return None;
                };
                Some(function.ret)
            }
        }
        NodeKind::ConstructorDeclaration
        | NodeKind::PrimaryConstructorDeclaration
        | NodeKind::FunctionDeclaration
        | NodeKind::MethodDeclaration => declared_return_type(context, owner),
        _ => None,
    }
}

fn declared_return_type(context: &LinterContext<'_>, declaration: NodeId) -> Option<TypeId> {
    Some(member::return_type(
        &context.resolved?.ctx,
        ElemRef::Base(context.declared_element(declaration)?),
    ))
}

fn expected_body_value_type(context: &LinterContext<'_>, body: NodeId) -> Option<TypeId> {
    let owner = context.ast.parent(body)?;
    let return_type = match context.ast.kind(owner) {
        NodeKind::FunctionExpression => {
            let owner_parent = context.ast.parent(owner)?;
            if context.ast.kind(owner_parent) == NodeKind::FunctionDeclaration {
                declared_return_type(context, owner_parent)?
            } else {
                let context_type = approximate_context_type(context, owner)?;
                let TypeKind::Function(function) = context.resolved?.ctx.ty(context_type) else {
                    return None;
                };
                function.ret
            }
        }
        NodeKind::MethodDeclaration => declared_return_type(context, owner)?,
        _ => return None,
    };
    let resolved = context.resolved?;
    if !matches!(resolved.ctx.ty(return_type), TypeKind::Interface { .. }) {
        return None;
    }
    let (keyword, star) = match context.ast.kind(body) {
        NodeKind::BlockFunctionBody => {
            let body = &context.ast[context.ast.cast::<BlockFunctionBody>(body)?];
            (body.keyword, body.star)
        }
        NodeKind::ExpressionFunctionBody => {
            let body = &context.ast[context.ast.cast::<ExpressionFunctionBody>(body)?];
            (body.keyword, body.star)
        }
        _ => return Some(return_type),
    };
    let is_async = keyword.is_some_and(|token| context.ast.tokens.lexeme(token) == "async");
    let expected_element = match (is_async, star.is_some()) {
        (true, false) => resolved.ctx.tp.future_element().upcast(),
        (true, true) => resolved.ctx.tp.stream_element().upcast(),
        (false, true) => resolved.ctx.tp.iterable_element().upcast(),
        (false, false) => return Some(return_type),
    };
    if resolved.ctx.interface_element(return_type) != Some(expected_element) {
        return Some(return_type);
    }
    resolved.ctx.type_arguments(return_type).first().copied()
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
        } else if let Some(annotation) = parameter
            .type_
            .filter(|annotation| context.ast.kind(*annotation) == NodeKind::NamedType)
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
