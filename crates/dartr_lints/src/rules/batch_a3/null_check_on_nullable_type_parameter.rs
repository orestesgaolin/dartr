// Dart source: pkg/linter/lib/src/rules/null_check_on_nullable_type_parameter.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::{TypeExt, member};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_null_assert_pattern("null_check_on_nullable_type_parameter", check);
    r.add_postfix_expression("null_check_on_nullable_type_parameter", check);
}
/// Dart `getExpectedType(node, allowPromotable: true)`
/// (`rules/unnecessary_null_checks.dart`).
fn expected(ctx: &LinterContext<'_>, node: NodeId) -> Option<dartr_element::TypeId> {
    get_expected_type(ctx, node, true)
}

/// Dart `getExpectedType(node, allowPromotable:)`
/// (`rules/unnecessary_null_checks.dart`).
pub(crate) fn get_expected_type(
    ctx: &LinterContext<'_>,
    node: NodeId,
    allow_promotable: bool,
) -> Option<dartr_element::TypeId> {
    let resolved = ctx.resolved?;
    let r = &resolved.ctx;
    let mut real_node = node;
    while let Some(parent) = ctx.ast.parent(real_node)
        && ctx.ast.kind(parent) == NodeKind::ParenthesizedExpression
    {
        real_node = parent;
    }
    let mut parent = ctx.ast.parent(real_node)?;
    let with_await = ctx.ast.kind(parent) == NodeKind::AwaitExpression;
    if with_await {
        parent = ctx.ast.parent(parent)?;
    }
    let enclosing_function_expression = |from: NodeId| {
        let mut current = Some(from);
        while let Some(n) = current {
            if let Some(f) = ctx.ast.cast::<FunctionExpression>(n) {
                return Some(f);
            }
            current = ctx.ast.parent(n);
        }
        None
    };
    let function_return_type = |function: Id<FunctionExpression>| {
        let ty = ctx.static_type(function.raw())?;
        match *r.ty(ty) {
            TypeKind::Function(data) => Some(data.ret),
            _ => None,
        }
    };
    let first_type_argument = |ty: dartr_element::TypeId| match *r.ty(ty) {
        TypeKind::Interface { args, .. } => r.list(args).first().copied(),
        _ => None,
    };
    match ctx.ast.kind(parent) {
        // in return value
        NodeKind::ReturnStatement | NodeKind::ExpressionFunctionBody => {
            let function = enclosing_function_expression(parent)?;
            let return_type = function_return_type(function)?;
            let body = ctx.ast[function].body.raw();
            let keyword = match ctx.ast.kind(body) {
                NodeKind::BlockFunctionBody => {
                    ctx.ast[Id::<BlockFunctionBody>::from_raw(body)].keyword
                }
                NodeKind::ExpressionFunctionBody => {
                    ctx.ast[Id::<ExpressionFunctionBody>::from_raw(body)].keyword
                }
                _ => None,
            };
            if with_await || keyword.is_some_and(|k| ctx.ast.tokens.lexeme(k) == "async") {
                if r.is_dart_async_future(return_type) || r.is_dart_async_future_or(return_type) {
                    first_type_argument(return_type)
                } else {
                    None
                }
            } else {
                Some(return_type)
            }
        }
        // in yield value
        NodeKind::YieldStatement => {
            let function = enclosing_function_expression(parent)?;
            let return_type = function_return_type(function)?;
            if r.is_dart_core_iterable(return_type) || r.is_dart_async_stream(return_type) {
                first_type_argument(return_type)
            } else {
                None
            }
        }
        // assignment
        NodeKind::AssignmentExpression => {
            let assignment = &ctx.ast[Id::<AssignmentExpression>::from_raw(parent)];
            if ctx.ast.tokens.lexeme(assignment.operator) != "=" {
                return None;
            }
            let lhs = assignment.left_hand_side.raw();
            let operand = ctx.ast[Id::<PostfixExpression>::from_raw(node)]
                .operand
                .raw();
            // Dart `Identifier.name`.
            let identifier_name = |n: NodeId| match ctx.ast.kind(n) {
                NodeKind::SimpleIdentifier => Some(
                    ctx.ast
                        .tokens
                        .lexeme(ctx.ast[Id::<SimpleIdentifier>::from_raw(n)].token)
                        .to_string(),
                ),
                NodeKind::PrefixedIdentifier => {
                    let p = &ctx.ast[Id::<PrefixedIdentifier>::from_raw(n)];
                    Some(format!(
                        "{}.{}",
                        ctx.ast.tokens.lexeme(ctx.ast[p.prefix].token),
                        ctx.ast.tokens.lexeme(ctx.ast[p.identifier].token)
                    ))
                }
                _ => None,
            };
            let same_names = match (identifier_name(lhs), identifier_name(operand)) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            if same_names {
                return None;
            }
            if !allow_promotable && Identifier::test(ctx.ast.kind(lhs)) {
                // Do not return a type when the left side of an assignment is
                // promotable.
                if let Some(element) = ctx.element(lhs).map(|e| member::base_element(r, e)) {
                    let tag = element.tag();
                    if matches!(
                        tag,
                        dartr_element::Tag::LocalVariable
                            | dartr_element::Tag::PatternVariable
                            | dartr_element::Tag::BindPatternVariable
                            | dartr_element::Tag::JoinPatternVariable
                            | dartr_element::Tag::FormalParameter
                            | dartr_element::Tag::FieldFormalParameter
                            | dartr_element::Tag::SuperFormalParameter
                    ) {
                        return None;
                    }
                    if tag == dartr_element::Tag::Field
                        && r.element_data(element)
                            .and_then(|d| r.fragment_data(d.first_fragment))
                            .is_some_and(|f| {
                                f.flags.has(dartr_element::FragmentFlags::FIELD_FRAGMENT_IS_PROMOTABLE)
                            })
                    {
                        return None;
                    }
                }
            }
            resolved.tables.write_type.get(parent).copied()
        }
        // in variable declaration
        NodeKind::VariableDeclaration => {
            let element = ctx.declared_element(parent)?;
            Some(member::type_(r, dartr_element::ElemRef::Base(element)))
        }
        // as right member of binary operator
        NodeKind::BinaryExpression
            if ctx.ast[Id::<BinaryExpression>::from_raw(parent)]
                .right_operand
                .raw()
                == real_node =>
        {
            let element = ctx.element(parent)?;
            let parameter = *member::formal_parameters(r, element).first()?;
            Some(member::type_(r, parameter))
        }
        // as member of list / set
        NodeKind::ListLiteral => first_type_argument(ctx.static_type(parent)?),
        NodeKind::SetOrMapLiteral => {
            let ty = ctx.static_type(parent)?;
            let is_set = r
                .interface_element(ty)
                .is_some_and(|e| r.element_name(e.raw()) == Some("Set"));
            if is_set {
                first_type_argument(ty)
            } else {
                None
            }
        }
        // as member of map
        NodeKind::MapLiteralEntry => {
            let entry = &ctx.ast[Id::<MapLiteralEntry>::from_raw(parent)];
            let index = if entry.key.raw() == node { 0 } else { 1 };
            let mut grand_parent = ctx.ast.parent(parent)?;
            loop {
                match ctx.ast.kind(grand_parent) {
                    NodeKind::ForElement | NodeKind::IfElement => {
                        grand_parent = ctx.ast.parent(grand_parent)?;
                    }
                    NodeKind::SetOrMapLiteral => {
                        let ty = ctx.static_type(grand_parent)?;
                        return match *r.ty(ty) {
                            TypeKind::Interface { args, .. } => r.list(args).get(index).copied(),
                            _ => None,
                        };
                    }
                    _ => return None,
                }
            }
        }
        // as parameter of function
        NodeKind::NamedArgument | NodeKind::ArgumentList => {
            let (real_node, parent) = if ctx.ast.kind(parent) == NodeKind::NamedArgument {
                (parent, ctx.ast.parent(parent)?)
            } else {
                (real_node, parent)
            };
            if ctx.ast.kind(parent) != NodeKind::ArgumentList {
                return None;
            }
            let grand_parent = ctx.ast.parent(parent)?;
            if let Some(creation) = ctx.ast.cast::<InstanceCreationExpression>(grand_parent) {
                if let Some(constructor) = ctx.element(ctx.ast[creation].constructor_name) {
                    let base = member::base_element(r, constructor);
                    if r.is_dart_async_future(member::return_type(r, constructor))
                        && r.element_name(base) == Some("value")
                    {
                        return None;
                    }
                }
            } else if let Some(invocation) = ctx.ast.cast::<MethodInvocation>(grand_parent) {
                let invocation_node = &ctx.ast[invocation];
                // Dart `realTarget`: the target, or the cascade target.
                let real_target = invocation_node.target.map(|t| t.raw()).or_else(|| {
                    let mut current = ctx.ast.parent(grand_parent);
                    while let Some(n) = current {
                        if let Some(cascade) = ctx.ast.cast::<CascadeExpression>(n) {
                            return Some(ctx.ast[cascade].target.raw());
                        }
                        current = ctx.ast.parent(n);
                    }
                    None
                });
                if let Some(target_type) = real_target.and_then(|t| ctx.static_type(t))
                    && let Some(target_class) = r.interface_element(target_type)
                    && r.element_library_uri(target_class.raw()) == Some("dart:async")
                    && r.element_name(target_class.raw()) == Some("Completer")
                    && ctx
                        .ast
                        .tokens
                        .lexeme(ctx.ast[invocation_node.method_name].token)
                        == "complete"
                {
                    return None;
                }
            }
            ctx.corresponding_parameter_type(real_node)
        }
        _ => None,
    }
}

fn nullable_parameter(ctx: &LinterContext<'_>, ty: dartr_element::TypeId) -> bool {
    matches!(
        ctx.resolved.unwrap().ctx.ty(ty),
        TypeKind::TypeParameter { .. }
    ) && ctx.type_system().unwrap().is_nullable(ty)
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = ctx.resolved else {
        return;
    };
    match ctx.ast.kind(node) {
        NodeKind::NullAssertPattern => {
            let n = &ctx.ast[Id::<NullAssertPattern>::from_raw(node)];
            if r.tables
                .pattern_info
                .get(node)
                .and_then(|i| i.matched_value_type)
                .is_some_and(|t| nullable_parameter(ctx, t))
            {
                ctx.report_token(
                    out,
                    &diag::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER,
                    n.operator,
                    &[],
                );
            }
        }
        NodeKind::PostfixExpression => {
            let n = &ctx.ast[Id::<PostfixExpression>::from_raw(node)];
            if ctx.ast.tokens.lexeme(n.operator) != "!" {
                return;
            }
            let (Some(ty), Some(exp)) = (ctx.static_type(n.operand), expected(ctx, node)) else {
                return;
            };
            let ts = ctx.type_system().unwrap();
            if nullable_parameter(ctx, ty)
                && ts.is_potentially_nullable(exp)
                && ts.dart_eq(ts.promote_to_non_null(ty), ts.promote_to_non_null(exp))
            {
                ctx.report_token(
                    out,
                    &diag::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER,
                    n.operator,
                    &[],
                );
            }
        }
        _ => {}
    }
}
