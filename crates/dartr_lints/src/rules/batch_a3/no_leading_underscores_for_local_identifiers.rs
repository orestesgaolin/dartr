// Dart source: pkg/linter/lib/src/rules/no_leading_underscores_for_local_identifiers.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_syntax::TokenId;

pub fn register(registry: &mut RuleVisitorRegistry) {
    for kind in [
        NodeKind::CatchClause,
        NodeKind::DeclaredIdentifier,
        NodeKind::DeclaredVariablePattern,
        NodeKind::ForPartsWithDeclarations,
        NodeKind::FormalParameterList,
        NodeKind::FunctionDeclarationStatement,
        NodeKind::VariableDeclarationStatement,
    ] {
        registry.add(kind, "no_leading_underscores_for_local_identifiers", check);
    }
}

fn check_token(ctx: &LinterContext<'_>, token: TokenId, out: &mut Vec<Diagnostic>) {
    let name = ctx.ast.tokens.lexeme(token);
    if name.starts_with('_') && name.bytes().any(|b| b != b'_') {
        ctx.report_token(
            out,
            &diag::NO_LEADING_UNDERSCORES_FOR_LOCAL_IDENTIFIERS,
            token,
            &[name],
        );
    }
}

fn formal_name(ctx: &LinterContext<'_>, node: NodeId) -> Option<TokenId> {
    match ctx.ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            ctx.ast[Id::<RegularFormalParameter>::from_raw(node)].name
        }
        _ => None,
    }
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::CatchClause => {
            let n = &ctx.ast[Id::<CatchClause>::from_raw(node)];
            for p in [n.exception_parameter, n.stack_trace_parameter]
                .into_iter()
                .flatten()
            {
                check_token(ctx, ctx.ast[p].name, out);
            }
        }
        NodeKind::DeclaredIdentifier => check_token(
            ctx,
            ctx.ast[Id::<DeclaredIdentifier>::from_raw(node)].name,
            out,
        ),
        NodeKind::DeclaredVariablePattern => {
            if ctx
                .ast
                .parent(node)
                .and_then(|parent| ctx.ast.cast::<PatternField>(parent))
                .is_some_and(|field| ctx.ast[field].name.is_none())
            {
                return;
            }
            check_token(
                ctx,
                ctx.ast[Id::<DeclaredVariablePattern>::from_raw(node)].name,
                out,
            );
        }
        NodeKind::ForPartsWithDeclarations => {
            let list = &ctx.ast[ctx.ast[Id::<ForPartsWithDeclarations>::from_raw(node)].variables];
            for variable in ctx.ast.list(list.variables) {
                check_token(ctx, ctx.ast[*variable].name, out);
            }
        }
        NodeKind::VariableDeclarationStatement => {
            let list =
                &ctx.ast[ctx.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables];
            for variable in ctx.ast.list(list.variables) {
                check_token(ctx, ctx.ast[*variable].name, out);
            }
        }
        NodeKind::FunctionDeclarationStatement => {
            let declaration =
                ctx.ast[Id::<FunctionDeclarationStatement>::from_raw(node)].function_declaration;
            check_token(ctx, ctx.ast[declaration].name, out);
        }
        NodeKind::FormalParameterList => {
            let list = &ctx.ast[Id::<FormalParameterList>::from_raw(node)];
            if ctx.ast.parent(node).is_some_and(|p| {
                ctx.ast.kind(p) == NodeKind::PrimaryConstructorDeclaration
                    && ctx.ast.parent(p).is_some_and(|owner| {
                        ctx.ast.kind(owner) == NodeKind::ExtensionTypeDeclaration
                    })
            }) {
                return;
            }
            for parameter in ctx.ast.list(list.parameters) {
                if matches!(
                    ctx.ast.kind(*parameter),
                    NodeKind::FieldFormalParameter | NodeKind::SuperFormalParameter
                ) {
                    return;
                }
                if ctx
                    .declared_element(parameter.raw())
                    .is_some_and(|e| e.is::<dartr_element::FieldFormalParameterElement>())
                {
                    return;
                }
                if let Some(name) = formal_name(ctx, parameter.raw())
                    && ctx.ast[Id::<RegularFormalParameter>::from_raw(parameter.raw())].kind
                        != dartr_element::ParameterKind::Named
                {
                    check_token(ctx, name, out);
                }
            }
        }
        _ => {}
    }
}
