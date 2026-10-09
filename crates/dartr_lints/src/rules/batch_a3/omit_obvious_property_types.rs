// Dart source: pkg/linter/lib/src/rules/omit_obvious_property_types.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

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
        | NodeKind::RecordLiteral
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
