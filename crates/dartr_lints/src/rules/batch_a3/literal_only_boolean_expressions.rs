// Dart source: pkg/linter/lib/src/rules/literal_only_boolean_expressions.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry) {
    for kind in [
        NodeKind::DoStatement,
        NodeKind::ForStatement,
        NodeKind::IfStatement,
        NodeKind::WhenClause,
        NodeKind::WhileStatement,
    ] {
        registry.add(kind, "literal_only_boolean_expressions", check);
    }
}

fn unparenthesized(ctx: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(p) = ctx.ast.cast::<ParenthesizedExpression>(node) {
        node = ctx.ast[p].expression.raw();
    }
    node
}

fn only_literals(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let node = unparenthesized(ctx, node);
    match ctx.ast.kind(node) {
        NodeKind::BooleanLiteral
        | NodeKind::DoubleLiteral
        | NodeKind::IntegerLiteral
        | NodeKind::ListLiteral
        | NodeKind::NullLiteral
        | NodeKind::RecordLiteral
        | NodeKind::SetOrMapLiteral
        | NodeKind::SymbolLiteral
        | NodeKind::SimpleStringLiteral => true,
        NodeKind::AdjacentStrings | NodeKind::StringInterpolation => ctx
            .constant_value(node)
            .is_some_and(|value| value.to_string_value().is_some()),
        NodeKind::PrefixExpression => only_literals(
            ctx,
            ctx.ast[Id::<PrefixExpression>::from_raw(node)]
                .operand
                .raw(),
        ),
        NodeKind::BinaryExpression => {
            let n = &ctx.ast[Id::<BinaryExpression>::from_raw(node)];
            if ctx.ast.tokens.lexeme(n.operator) == "??" {
                only_literals(ctx, n.left_operand.raw())
            } else {
                only_literals(ctx, n.left_operand.raw())
                    && only_literals(ctx, n.right_operand.raw())
            }
        }
        NodeKind::IsExpression => {
            let n = &ctx.ast[Id::<IsExpression>::from_raw(node)];
            // A type-parameter test requires resolution. Suppress when the annotation resolves
            // to a type parameter; the normal literal case remains exact.
            if ctx.resolved.is_some_and(|resolved| {
                resolved
                    .tables
                    .annotation_type
                    .get(n.type_.raw())
                    .is_some_and(|ty| {
                        matches!(
                            resolved.ctx.ty(*ty),
                            dartr_element::TypeKind::TypeParameter { .. }
                        )
                    })
            }) {
                false
            } else {
                only_literals(ctx, n.expression.raw())
            }
        }
        _ => false,
    }
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = match ctx.ast.kind(node) {
        NodeKind::DoStatement => Some(ctx.ast[Id::<DoStatement>::from_raw(node)].condition.raw()),
        NodeKind::IfStatement => {
            let n = &ctx.ast[Id::<IfStatement>::from_raw(node)];
            n.case_clause.is_none().then_some(n.expression.raw())
        }
        NodeKind::WhenClause => Some(ctx.ast[Id::<WhenClause>::from_raw(node)].expression.raw()),
        NodeKind::WhileStatement => {
            let n = &ctx.ast[Id::<WhileStatement>::from_raw(node)];
            if ctx
                .ast
                .cast::<BooleanLiteral>(n.condition.raw())
                .is_some_and(|b| ctx.ast[b].value)
            {
                return;
            }
            Some(n.condition.raw())
        }
        NodeKind::ForStatement => {
            let parts = ctx.ast[Id::<ForStatement>::from_raw(node)]
                .for_loop_parts
                .raw();
            match ctx.ast.kind(parts) {
                NodeKind::ForPartsWithDeclarations => ctx.ast
                    [Id::<ForPartsWithDeclarations>::from_raw(parts)]
                .condition
                .map(Id::raw),
                NodeKind::ForPartsWithExpression => ctx.ast
                    [Id::<ForPartsWithExpression>::from_raw(parts)]
                .condition
                .map(Id::raw),
                NodeKind::ForPartsWithPattern => ctx.ast
                    [Id::<ForPartsWithPattern>::from_raw(parts)]
                .condition
                .map(Id::raw),
                _ => None,
            }
        }
        _ => None,
    };
    if expression.is_some_and(|e| only_literals(ctx, e)) {
        ctx.report_node(out, &diag::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, node, &[]);
    }
}
