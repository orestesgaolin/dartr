// Dart source: pkg/linter/lib/src/rules/no_dynamic_casts.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry) {
    for kind in [
        NodeKind::ArgumentList,
        NodeKind::AssignmentExpression,
        NodeKind::BinaryExpression,
        NodeKind::ConditionalExpression,
        NodeKind::DoStatement,
        NodeKind::ExpressionFunctionBody,
        NodeKind::ForEachPartsWithDeclaration,
        NodeKind::ForEachPartsWithIdentifier,
        NodeKind::ForEachPartsWithPattern,
        NodeKind::ForStatement,
        NodeKind::IfElement,
        NodeKind::IfStatement,
        NodeKind::ListLiteral,
        NodeKind::PrefixExpression,
        NodeKind::ReturnStatement,
        NodeKind::SetOrMapLiteral,
        NodeKind::VariableDeclaration,
        NodeKind::WhenClause,
        NodeKind::WhileStatement,
        NodeKind::YieldStatement,
    ] {
        registry.add(kind, "no_dynamic_casts", check_node);
    }
}

fn unparenthesized(ctx: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(p) = ctx.ast.cast::<ParenthesizedExpression>(node) {
        node = ctx.ast[p].expression.raw();
    }
    node
}

fn check(ctx: &LinterContext<'_>, expression: NodeId, target: TypeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = ctx.resolved else {
        return;
    };
    if !matches!(
        ctx.static_type(expression).map(|t| *resolved.ctx.ty(t)),
        Some(TypeKind::Dynamic)
    ) {
        return;
    }
    if matches!(resolved.ctx.ty(target), TypeKind::Dynamic)
        || target == resolved.ctx.tp.object_question_type()
    {
        return;
    }
    if ctx.ast.kind(unparenthesized(ctx, expression)) == NodeKind::AsExpression {
        return;
    }
    ctx.report_node(out, &diag::NO_DYNAMIC_CASTS, expression, &[]);
}

fn bool_check(ctx: &LinterContext<'_>, expression: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(r) = ctx.resolved {
        check(ctx, expression, r.ctx.tp.bool_type(), out);
    }
}

fn check_for_each(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (iterable, loop_type) = match ctx.ast.kind(node) {
        NodeKind::ForEachPartsWithDeclaration => {
            let parts = &ctx.ast[Id::<ForEachPartsWithDeclaration>::from_raw(node)];
            let loop_type = ctx.ast[parts.loop_variable].type_.and_then(|annotation| {
                ctx.resolved?
                    .tables
                    .annotation_type
                    .get(annotation.raw())
                    .copied()
            });
            (parts.iterable.raw(), loop_type)
        }
        NodeKind::ForEachPartsWithIdentifier => {
            let parts = &ctx.ast[Id::<ForEachPartsWithIdentifier>::from_raw(node)];
            (parts.iterable.raw(), ctx.static_type(parts.identifier))
        }
        NodeKind::ForEachPartsWithPattern => {
            let parts = &ctx.ast[Id::<ForEachPartsWithPattern>::from_raw(node)];
            (parts.iterable.raw(), None)
        }
        _ => return,
    };
    let Some(statement) = ctx.ast.parent(node) else {
        return;
    };
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let Some(statement) = ctx.ast.cast::<ForStatement>(statement) else {
        return;
    };
    let is_async = ctx.ast[statement].await_keyword.is_some();
    let dynamic = resolved.ctx.tp.dynamic_type();
    let iterable_target = if is_async {
        resolved.ctx.tp.stream_type(&resolved.ctx, dynamic)
    } else {
        resolved.ctx.tp.iterable_type(&resolved.ctx, dynamic)
    };
    check(ctx, iterable, iterable_target, out);

    let Some(loop_type) = loop_type else {
        return;
    };
    if matches!(
        resolved.ctx.ty(loop_type),
        TypeKind::Dynamic | TypeKind::Void
    ) || loop_type == resolved.ctx.tp.object_question_type()
    {
        return;
    }
    let Some(iterable_type) = ctx.static_type(iterable) else {
        return;
    };
    let element_type = match *resolved.ctx.ty(iterable_type) {
        TypeKind::Dynamic => TypeId::DYNAMIC,
        TypeKind::Interface { args, .. } => resolved
            .ctx
            .list(args)
            .first()
            .copied()
            .unwrap_or(TypeId::INVALID),
        _ => return,
    };
    if matches!(resolved.ctx.ty(element_type), TypeKind::Dynamic) {
        ctx.report_node(out, &diag::NO_DYNAMIC_CASTS, iterable, &[]);
    }
}

fn enclosing_return_type(ctx: &LinterContext<'_>, mut node: NodeId) -> Option<TypeId> {
    let flatten_if_async = |return_type: TypeId, body: Id<FunctionBody>| {
        let keyword = match ctx.ast.kind(body) {
            NodeKind::BlockFunctionBody => {
                ctx.ast[Id::<BlockFunctionBody>::from_raw(body.raw())].keyword
            }
            NodeKind::ExpressionFunctionBody => {
                ctx.ast[Id::<ExpressionFunctionBody>::from_raw(body.raw())].keyword
            }
            _ => None,
        };
        if keyword.is_some_and(|keyword| ctx.ast.tokens.lexeme(keyword) == "async") {
            Some(ctx.type_system()?.flatten(return_type))
        } else {
            Some(return_type)
        }
    };
    while let Some(parent) = ctx.ast.parent(node) {
        if let Some(function) = ctx.ast.cast::<FunctionExpression>(parent) {
            let ty = ctx.static_type(function.raw())?;
            let TypeKind::Function(data) = *ctx.resolved?.ctx.ty(ty) else {
                return None;
            };
            return flatten_if_async(data.ret, ctx.ast[function].body);
        }
        let return_type = || {
            Some(member::return_type(
                &ctx.resolved?.ctx,
                dartr_element::ElemRef::Base(ctx.declared_element(parent)?),
            ))
        };
        match ctx.ast.kind(parent) {
            NodeKind::MethodDeclaration => {
                let method = &ctx.ast[Id::<MethodDeclaration>::from_raw(parent)];
                return flatten_if_async(return_type()?, method.body);
            }
            NodeKind::ConstructorDeclaration | NodeKind::PrimaryConstructorDeclaration => {
                return return_type();
            }
            NodeKind::FunctionDeclaration => {
                let function = &ctx.ast
                    [ctx.ast[Id::<FunctionDeclaration>::from_raw(parent)].function_expression];
                return flatten_if_async(return_type()?, function.body);
            }
            _ => {}
        }
        node = parent;
    }
    None
}

fn check_collection_element(
    ctx: &LinterContext<'_>,
    node: NodeId,
    target: TypeId,
    out: &mut Vec<Diagnostic>,
) {
    match ctx.ast.kind(node) {
        NodeKind::ForEachPartsWithDeclaration
        | NodeKind::ForEachPartsWithIdentifier
        | NodeKind::ForEachPartsWithPattern => check_for_each(ctx, node, out),
        kind if Expression::test(kind) => check(ctx, node, target, out),
        NodeKind::IfElement => {
            let element = &ctx.ast[Id::<IfElement>::from_raw(node)];
            check_collection_element(ctx, element.then_element.raw(), target, out);
            if let Some(else_element) = element.else_element {
                check_collection_element(ctx, else_element.raw(), target, out);
            }
        }
        NodeKind::ForElement => check_collection_element(
            ctx,
            ctx.ast[Id::<ForElement>::from_raw(node)].body.raw(),
            target,
            out,
        ),
        NodeKind::SpreadElement => {
            let Some(resolved) = ctx.resolved else {
                return;
            };
            let iterable = resolved.ctx.tp.iterable_type(&resolved.ctx, target);
            check(
                ctx,
                ctx.ast[Id::<SpreadElement>::from_raw(node)]
                    .expression
                    .raw(),
                iterable,
                out,
            );
        }
        _ => {}
    }
}

fn check_map_element(
    ctx: &LinterContext<'_>,
    node: NodeId,
    key_type: TypeId,
    value_type: TypeId,
    out: &mut Vec<Diagnostic>,
) {
    match ctx.ast.kind(node) {
        NodeKind::MapLiteralEntry => {
            let entry = &ctx.ast[Id::<MapLiteralEntry>::from_raw(node)];
            check(ctx, entry.key.raw(), key_type, out);
            check(ctx, entry.value.raw(), value_type, out);
        }
        NodeKind::IfElement => {
            let element = &ctx.ast[Id::<IfElement>::from_raw(node)];
            check_map_element(ctx, element.then_element.raw(), key_type, value_type, out);
            if let Some(else_element) = element.else_element {
                check_map_element(ctx, else_element.raw(), key_type, value_type, out);
            }
        }
        NodeKind::ForElement => check_map_element(
            ctx,
            ctx.ast[Id::<ForElement>::from_raw(node)].body.raw(),
            key_type,
            value_type,
            out,
        ),
        NodeKind::SpreadElement => {
            let Some(resolved) = ctx.resolved else {
                return;
            };
            let map = resolved
                .ctx
                .tp
                .map_type(&resolved.ctx, key_type, value_type);
            check(
                ctx,
                ctx.ast[Id::<SpreadElement>::from_raw(node)]
                    .expression
                    .raw(),
                map,
                out,
            );
        }
        _ => {}
    }
}

fn check_node(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::ArgumentList => {
            for argument in ctx
                .ast
                .list(ctx.ast[Id::<ArgumentList>::from_raw(node)].arguments)
            {
                let expression = ctx
                    .ast
                    .cast::<NamedArgument>(*argument)
                    .map_or(argument.raw(), |n| ctx.ast[n].argument_expression.raw());
                let Some(parameter) = ctx
                    .resolved
                    .and_then(|r| r.tables.param_element.get(expression).copied())
                else {
                    continue;
                };
                check(
                    ctx,
                    expression,
                    member::type_(&ctx.resolved.unwrap().ctx, parameter),
                    out,
                );
            }
        }
        NodeKind::AssignmentExpression => {
            let n = &ctx.ast[Id::<AssignmentExpression>::from_raw(node)];
            if let Some(target) = ctx
                .resolved
                .and_then(|r| r.tables.write_type.get(node).copied())
            {
                check(ctx, n.right_hand_side.raw(), target, out);
            }
        }
        NodeKind::BinaryExpression => {
            let n = &ctx.ast[Id::<BinaryExpression>::from_raw(node)];
            if matches!(ctx.ast.tokens.lexeme(n.operator), "&&" | "||") {
                bool_check(ctx, n.left_operand.raw(), out);
                bool_check(ctx, n.right_operand.raw(), out);
            }
        }
        NodeKind::ConditionalExpression => bool_check(
            ctx,
            ctx.ast[Id::<ConditionalExpression>::from_raw(node)]
                .condition
                .raw(),
            out,
        ),
        NodeKind::DoStatement => bool_check(
            ctx,
            ctx.ast[Id::<DoStatement>::from_raw(node)].condition.raw(),
            out,
        ),
        NodeKind::IfStatement => bool_check(
            ctx,
            ctx.ast[Id::<IfStatement>::from_raw(node)].expression.raw(),
            out,
        ),
        NodeKind::IfElement => bool_check(
            ctx,
            ctx.ast[Id::<IfElement>::from_raw(node)].expression.raw(),
            out,
        ),
        NodeKind::WhenClause => bool_check(
            ctx,
            ctx.ast[Id::<WhenClause>::from_raw(node)].expression.raw(),
            out,
        ),
        NodeKind::WhileStatement => bool_check(
            ctx,
            ctx.ast[Id::<WhileStatement>::from_raw(node)]
                .condition
                .raw(),
            out,
        ),
        NodeKind::PrefixExpression => {
            let n = &ctx.ast[Id::<PrefixExpression>::from_raw(node)];
            if ctx.ast.tokens.lexeme(n.operator) == "!" {
                bool_check(ctx, n.operand.raw(), out);
            }
        }
        NodeKind::ExpressionFunctionBody => {
            let n = &ctx.ast[Id::<ExpressionFunctionBody>::from_raw(node)];
            if let Some(target) = enclosing_return_type(ctx, node) {
                check(ctx, n.expression.raw(), target, out);
            }
        }
        NodeKind::ReturnStatement => {
            let n = &ctx.ast[Id::<ReturnStatement>::from_raw(node)];
            if let (Some(expression), Some(target)) =
                (n.expression, enclosing_return_type(ctx, node))
            {
                check(ctx, expression.raw(), target, out);
            }
        }
        NodeKind::YieldStatement => {
            let n = &ctx.ast[Id::<YieldStatement>::from_raw(node)];
            if let Some(mut target) = enclosing_return_type(ctx, node) {
                if n.star.is_none()
                    && let TypeKind::Interface { args, .. } = *ctx.resolved.unwrap().ctx.ty(target)
                    && let Some(first) = ctx.resolved.unwrap().ctx.list(args).first()
                {
                    target = *first;
                }
                check(ctx, n.expression.raw(), target, out);
            }
        }
        NodeKind::ListLiteral => {
            let n = &ctx.ast[Id::<ListLiteral>::from_raw(node)];
            let Some(ty) = ctx.static_type(node) else {
                return;
            };
            let TypeKind::Interface { args, .. } = *ctx.resolved.unwrap().ctx.ty(ty) else {
                return;
            };
            let Some(element_type) = ctx.resolved.unwrap().ctx.list(args).first().copied() else {
                return;
            };
            for element in ctx.ast.list_raw(n.elements) {
                check_collection_element(ctx, *element, element_type, out);
            }
        }
        NodeKind::SetOrMapLiteral => {
            let n = &ctx.ast[Id::<SetOrMapLiteral>::from_raw(node)];
            let Some(ty) = ctx.static_type(node) else {
                return;
            };
            let TypeKind::Interface { element, args, .. } = *ctx.resolved.unwrap().ctx.ty(ty)
            else {
                return;
            };
            let args = ctx.resolved.unwrap().ctx.list(args);
            if ctx
                .resolved
                .unwrap()
                .ctx
                .is_element(element.raw(), "dart.core", "Map")
            {
                let [key_type, value_type] = args else {
                    return;
                };
                for entry in ctx.ast.list_raw(n.elements) {
                    check_map_element(ctx, *entry, *key_type, *value_type, out);
                }
            } else if let Some(element_type) = args.first().copied() {
                for element in ctx.ast.list_raw(n.elements) {
                    check_collection_element(ctx, *element, element_type, out);
                }
            }
        }
        NodeKind::ForStatement => {
            let parts = ctx.ast[Id::<ForStatement>::from_raw(node)]
                .for_loop_parts
                .raw();
            let condition = match ctx.ast.kind(parts) {
                NodeKind::ForPartsWithDeclarations => {
                    ctx.ast[Id::<ForPartsWithDeclarations>::from_raw(parts)].condition
                }
                NodeKind::ForPartsWithExpression => {
                    ctx.ast[Id::<ForPartsWithExpression>::from_raw(parts)].condition
                }
                NodeKind::ForPartsWithPattern => {
                    ctx.ast[Id::<ForPartsWithPattern>::from_raw(parts)].condition
                }
                _ => None,
            };
            if let Some(condition) = condition {
                bool_check(ctx, condition.raw(), out);
            }
        }
        NodeKind::VariableDeclaration => {
            let n = &ctx.ast[Id::<VariableDeclaration>::from_raw(node)];
            let Some(initializer) = n.initializer else {
                return;
            };
            let Some(parent) = ctx
                .ast
                .parent(node)
                .and_then(|p| ctx.ast.cast::<VariableDeclarationList>(p))
            else {
                return;
            };
            let Some(annotation) = ctx.ast[parent].type_ else {
                return;
            };
            if let Some(target) = ctx
                .resolved
                .and_then(|r| r.tables.annotation_type.get(annotation.raw()).copied())
            {
                check(ctx, initializer.raw(), target, out);
            }
        }
        _ => {}
    }
}
