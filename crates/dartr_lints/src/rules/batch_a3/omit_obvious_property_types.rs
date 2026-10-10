// Dart source: pkg/linter/lib/src/rules/omit_obvious_property_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementKind, TypeId};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_field_declaration("omit_obvious_property_types", check);
    registry.add_top_level_variable_declaration("omit_obvious_property_types", check);
}

pub(crate) fn is_obvious(ctx: &LinterContext<'_>, node: NodeId) -> bool {
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
                    // A raw generic interface type is not trivial; any other
                    // type (also an invalid one) is.
                    .is_none_or(|t| {
                        ctx.resolved
                            .unwrap()
                            .ctx
                            .interface_element(t)
                            .is_none_or(|e| {
                                ctx.resolved
                                    .unwrap()
                                    .ctx
                                    .interface_type_parameters(e)
                                    .is_empty()
                            })
                    })
        }
        NodeKind::ListLiteral | NodeKind::SetOrMapLiteral => typed_literal_is_obvious(ctx, node),
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

/// Dart `ExpressionExtensions.hasObviousType` of a `TypedLiteral`.
fn typed_literal_is_obvious(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let (type_arguments, elements) = match ctx.ast.kind(node) {
        NodeKind::ListLiteral => {
            let literal = &ctx.ast[Id::<ListLiteral>::from_raw(node)];
            (literal.type_arguments, ctx.ast.list_raw(literal.elements))
        }
        _ => {
            let literal = &ctx.ast[Id::<SetOrMapLiteral>::from_raw(node)];
            (literal.type_arguments, ctx.ast.list_raw(literal.elements))
        }
    };
    if type_arguments.is_some() {
        return true;
    }
    let same_or_null = |a: Option<TypeId>, b: Option<TypeId>| a.is_none() || b.is_none() || a == b;
    let (mut the_type, mut the_key, mut the_value) = (None, None, None);
    for &element in elements {
        if !element_has_obvious_type(ctx, element) {
            return false;
        }
        let (element_type, key_type, value_type) = (
            element_type(ctx, element),
            key_type(ctx, element),
            value_type(ctx, element),
        );
        the_type = the_type.or(element_type);
        the_key = the_key.or(key_type);
        the_value = the_value.or(value_type);
        if !same_or_null(the_type, element_type)
            || !same_or_null(the_key, key_type)
            || !same_or_null(the_value, value_type)
        {
            return false;
        }
    }
    let self_type = ctx.static_type(node);
    if the_type.is_some() {
        element_type_of_iterable(ctx, self_type) == the_type
    } else if let (Some(key), Some(value)) = (the_key, the_value) {
        key_value_type_of_map(ctx, self_type) == Some((key, value))
    } else {
        false
    }
}

/// Dart `DartTypeExtensions.elementTypeOfIterable`.
fn element_type_of_iterable(ctx: &LinterContext<'_>, ty: Option<TypeId>) -> Option<TypeId> {
    let resolved = ctx.resolved?;
    let ty = ty?;
    resolved.ctx.interface_element(ty)?;
    let iterable = resolved
        .ctx
        .as_instance_of(ty, resolved.ctx.tp.iterable_element().upcast())?;
    resolved.ctx.type_arguments(iterable).first().copied()
}

/// Dart `DartTypeExtensions.keyValueTypeOfMap`.
fn key_value_type_of_map(ctx: &LinterContext<'_>, ty: Option<TypeId>) -> Option<(TypeId, TypeId)> {
    let resolved = ctx.resolved?;
    let ty = ty?;
    resolved.ctx.interface_element(ty)?;
    let map = resolved
        .ctx
        .as_instance_of(ty, resolved.ctx.tp.map_element().upcast())?;
    match resolved.ctx.type_arguments(map) {
        [key, value] => Some((*key, *value)),
        _ => None,
    }
}

/// Dart `CollectionElementExtensions.elementType`.
fn element_type(ctx: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    match ctx.ast.kind(node) {
        NodeKind::MapLiteralEntry | NodeKind::ForElement | NodeKind::NullAwareElement => None,
        NodeKind::IfElement => element_type(
            ctx,
            ctx.ast[Id::<IfElement>::from_raw(node)].then_element.raw(),
        ),
        NodeKind::SpreadElement => {
            let expression = ctx.ast[Id::<SpreadElement>::from_raw(node)].expression;
            element_type_of_iterable(ctx, ctx.static_type(expression))
        }
        kind if Expression::test(kind) => ctx.static_type(node),
        _ => None,
    }
}

/// Dart `CollectionElementExtensions.keyType`.
fn key_type(ctx: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let entry = ctx.ast.cast::<MapLiteralEntry>(node)?;
    element_type(ctx, ctx.ast[entry].key.raw())
}

/// Dart `CollectionElementExtensions.valueType`.
fn value_type(ctx: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let entry = ctx.ast.cast::<MapLiteralEntry>(node)?;
    element_type(ctx, ctx.ast[entry].value.raw())
}

/// Dart `CollectionElementExtensions.hasObviousType`.
fn element_has_obvious_type(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    match ctx.ast.kind(node) {
        NodeKind::MapLiteralEntry => {
            let entry = &ctx.ast[Id::<MapLiteralEntry>::from_raw(node)];
            is_obvious(ctx, entry.key.raw()) && is_obvious(ctx, entry.value.raw())
        }
        NodeKind::IfElement => {
            let element = &ctx.ast[Id::<IfElement>::from_raw(node)];
            element_has_obvious_type(ctx, element.then_element.raw())
                && element
                    .else_element
                    .is_none_or(|e| element_has_obvious_type(ctx, e.raw()))
        }
        NodeKind::SpreadElement => is_obvious(
            ctx,
            ctx.ast[Id::<SpreadElement>::from_raw(node)]
                .expression
                .raw(),
        ),
        NodeKind::NullAwareElement => is_obvious(
            ctx,
            ctx.ast[Id::<NullAwareElement>::from_raw(node)].value.raw(),
        ),
        NodeKind::ForElement => false,
        kind if Expression::test(kind) => is_obvious(ctx, node),
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
