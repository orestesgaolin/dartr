// Dart source: pkg/linter/lib/src/rules/omit_obvious_property_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementKind;
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_field_declaration("omit_obvious_property_types", check);
    registry.add_top_level_variable_declaration("omit_obvious_property_types", check);
}

pub(super) fn is_obvious(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    match ctx.ast.kind(node) {
        NodeKind::BooleanLiteral
        | NodeKind::DoubleLiteral
        | NodeKind::NullLiteral
        | NodeKind::SimpleStringLiteral
        | NodeKind::StringInterpolation
        | NodeKind::AdjacentStrings
        | NodeKind::SymbolLiteral
        | NodeKind::ThisExpression
        | NodeKind::TypeLiteral
        | NodeKind::AsExpression
        | NodeKind::IsExpression
        | NodeKind::ThrowExpression => true,
        NodeKind::IntegerLiteral => ctx
            .static_type(node)
            .is_some_and(|t| ctx.resolved.is_some_and(|r| !r.ctx.is_dart_core_double(t))),
        NodeKind::InstanceCreationExpression => {
            let n = &ctx.ast[Id::<InstanceCreationExpression>::from_raw(node)];
            ctx.ast[n.constructor_name]
                .type_
                .pipe(|t| ctx.ast[t].type_arguments.is_some())
                || ctx
                    .resolved
                    .and_then(|r| {
                        r.tables
                            .annotation_type
                            .get(ctx.ast[n.constructor_name].type_.raw())
                            .copied()
                    })
                    .and_then(|t| ctx.resolved.unwrap().ctx.interface_element(t))
                    .is_some_and(|e| {
                        ctx.resolved
                            .unwrap()
                            .ctx
                            .interface_type_parameters(e)
                            .is_empty()
                    })
        }
        NodeKind::ListLiteral => {
            let literal = &ctx.ast[Id::<ListLiteral>::from_raw(node)];
            if literal.type_arguments.is_some() {
                return true;
            }
            let Some(ty) = ctx.static_type(node) else {
                return false;
            };
            let Some(element_type) = ctx
                .resolved
                .map(|r| r.ctx.type_arguments(ty))
                .and_then(|args| args.first().copied())
            else {
                return false;
            };
            let elements = ctx.ast.list_raw(literal.elements);
            !elements.is_empty()
                && elements.iter().all(|element| {
                    collection_element_is_obvious(ctx, *element, Some(element_type), None)
                })
        }
        NodeKind::SetOrMapLiteral => {
            let literal = &ctx.ast[Id::<SetOrMapLiteral>::from_raw(node)];
            if literal.type_arguments.is_some() {
                return true;
            }
            let Some(ty) = ctx.static_type(node) else {
                return false;
            };
            let Some(resolved) = ctx.resolved else {
                return false;
            };
            let args = resolved.ctx.type_arguments(ty);
            let (element, map) = if resolved.ctx.is_dart_core_map(ty) {
                let [key, value] = args else {
                    return false;
                };
                (Some(*key), Some(*value))
            } else {
                (args.first().copied(), None)
            };
            let elements = ctx.ast.list_raw(literal.elements);
            !elements.is_empty()
                && elements
                    .iter()
                    .all(|entry| collection_element_is_obvious(ctx, *entry, element, map))
        }
        NodeKind::RecordLiteral => {
            let literal = &ctx.ast[Id::<RecordLiteral>::from_raw(node)];
            ctx.ast.list_raw(literal.fields).iter().all(|field| {
                ctx.ast.kind(*field) != NodeKind::RecordLiteralNamedField && is_obvious(ctx, *field)
            })
        }
        NodeKind::SimpleIdentifier => {
            let Some(resolved) = ctx.resolved else {
                return false;
            };
            let Some(element) = ctx.element(node) else {
                return false;
            };
            if !matches!(
                member::base_element(&resolved.ctx, element).kind(),
                ElementKind::LocalVariable | ElementKind::Parameter
            ) {
                return false;
            }
            ctx.static_type(node)
                .is_some_and(|ty| ty == member::type_(&resolved.ctx, element))
        }
        NodeKind::PrefixExpression => {
            let n = &ctx.ast[Id::<PrefixExpression>::from_raw(node)];
            ctx.ast.tokens.lexeme(n.operator) == "-"
                && matches!(
                    ctx.ast.kind(n.operand),
                    NodeKind::IntegerLiteral | NodeKind::DoubleLiteral
                )
                && is_obvious(ctx, n.operand.raw())
        }
        NodeKind::ParenthesizedExpression => is_obvious(
            ctx,
            ctx.ast[Id::<ParenthesizedExpression>::from_raw(node)]
                .expression
                .raw(),
        ),
        NodeKind::CascadeExpression => is_obvious(
            ctx,
            ctx.ast[Id::<CascadeExpression>::from_raw(node)]
                .target
                .raw(),
        ),
        NodeKind::ConditionalExpression => {
            let expression = &ctx.ast[Id::<ConditionalExpression>::from_raw(node)];
            is_obvious(ctx, expression.then_expression.raw())
                && is_obvious(ctx, expression.else_expression.raw())
                && ctx.static_type(expression.then_expression)
                    == ctx.static_type(expression.else_expression)
        }
        NodeKind::PropertyAccess => {
            let access = &ctx.ast[Id::<PropertyAccess>::from_raw(node)];
            ctx.ast.tokens.lexeme(ctx.ast[access.property_name].token) == "hashCode"
        }
        NodeKind::PrefixedIdentifier => {
            let access = &ctx.ast[Id::<PrefixedIdentifier>::from_raw(node)];
            ctx.ast.tokens.lexeme(ctx.ast[access.identifier].token) == "hashCode"
        }
        NodeKind::BinaryExpression => matches!(
            ctx.ast
                .tokens
                .lexeme(ctx.ast[Id::<BinaryExpression>::from_raw(node)].operator),
            "==" | "!="
        ),
        NodeKind::MethodInvocation => {
            ctx.ast
                .tokens
                .lexeme(ctx.ast[ctx.ast[Id::<MethodInvocation>::from_raw(node)].method_name].token)
                == "toString"
        }
        _ => false,
    }
}

fn collection_element_is_obvious(
    ctx: &LinterContext<'_>,
    node: NodeId,
    element_or_key_type: Option<dartr_element::TypeId>,
    value_type: Option<dartr_element::TypeId>,
) -> bool {
    match ctx.ast.kind(node) {
        NodeKind::MapLiteralEntry => {
            let entry = &ctx.ast[Id::<MapLiteralEntry>::from_raw(node)];
            is_obvious(ctx, entry.key.raw())
                && is_obvious(ctx, entry.value.raw())
                && element_or_key_type.is_none_or(|ty| ctx.static_type(entry.key) == Some(ty))
                && value_type.is_none_or(|ty| ctx.static_type(entry.value) == Some(ty))
        }
        NodeKind::IfElement => {
            let element = &ctx.ast[Id::<IfElement>::from_raw(node)];
            collection_element_is_obvious(
                ctx,
                element.then_element.raw(),
                element_or_key_type,
                value_type,
            ) && element.else_element.is_none_or(|else_element| {
                collection_element_is_obvious(
                    ctx,
                    else_element.raw(),
                    element_or_key_type,
                    value_type,
                )
            })
        }
        NodeKind::SpreadElement => {
            let expression = ctx.ast[Id::<SpreadElement>::from_raw(node)].expression;
            if !is_obvious(ctx, expression.raw()) {
                return false;
            }
            let Some(resolved) = ctx.resolved else {
                return false;
            };
            let Some(ty) = ctx.static_type(expression) else {
                return false;
            };
            let target = if value_type.is_some() {
                resolved.ctx.tp.map_element().upcast()
            } else {
                resolved.ctx.tp.iterable_element().upcast()
            };
            let Some(as_target) = resolved.ctx.as_instance_of(ty, target) else {
                return false;
            };
            let args = resolved.ctx.type_arguments(as_target);
            args.first().copied() == element_or_key_type
                && value_type.is_none_or(|value| args.get(1) == Some(&value))
        }
        NodeKind::NullAwareElement => is_obvious(
            ctx,
            ctx.ast[Id::<NullAwareElement>::from_raw(node)].value.raw(),
        ),
        NodeKind::ForElement => false,
        kind if Expression::test(kind) => {
            is_obvious(ctx, node)
                && element_or_key_type.is_none_or(|ty| ctx.static_type(node) == Some(ty))
        }
        _ => false,
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

pub(super) fn check_list(
    ctx: &LinterContext<'_>,
    list: Id<VariableDeclarationList>,
    code: &'static dartr_diagnostics::DiagnosticCode,
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
    if resolved.ctx.is_dart_core_null(declared) {
        return;
    }
    let ts = ctx.type_system().unwrap();
    for variable in ctx.ast.list(n.variables) {
        let Some(initializer) = ctx.ast[*variable].initializer else {
            return;
        };
        if !is_obvious(ctx, initializer.raw()) {
            return;
        }
        let Some(initializer_type) = ctx.static_type(initializer) else {
            return;
        };
        if !ts.dart_eq(initializer_type, declared) {
            return;
        }
    }
    ctx.report_node(out, code, annotation, &[]);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = match ctx.ast.kind(node) {
        NodeKind::FieldDeclaration => ctx.ast[Id::<FieldDeclaration>::from_raw(node)].fields,
        NodeKind::TopLevelVariableDeclaration => {
            ctx.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables
        }
        _ => return,
    };
    check_list(ctx, list, &diag::OMIT_OBVIOUS_PROPERTY_TYPES, out);
}
