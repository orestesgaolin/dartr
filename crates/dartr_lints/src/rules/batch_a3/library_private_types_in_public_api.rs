// Dart source: pkg/linter/lib/src/rules/library_private_types_in_public_api.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::{TypeExt, member};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_named_type("library_private_types_in_public_api", check);
}
fn private_token(ctx: &LinterContext<'_>, t: dartr_syntax::TokenId) -> bool {
    ctx.ast.tokens.lexeme(t).starts_with('_')
}
fn public_api_position(ctx: &LinterContext<'_>, mut node: NodeId) -> bool {
    while let Some(p) = ctx.ast.parent(node) {
        match ctx.ast.kind(p) {
            NodeKind::BlockFunctionBody
            | NodeKind::ExpressionFunctionBody
            | NodeKind::VariableDeclarationStatement
            | NodeKind::ForStatement => return false,
            NodeKind::MethodDeclaration => {
                return !private_token(ctx, ctx.ast[Id::<MethodDeclaration>::from_raw(p)].name);
            }
            NodeKind::FunctionDeclaration => {
                return !private_token(ctx, ctx.ast[Id::<FunctionDeclaration>::from_raw(p)].name);
            }
            NodeKind::ConstructorDeclaration => {
                return ctx.ast[Id::<ConstructorDeclaration>::from_raw(p)]
                    .name
                    .is_none_or(|n| !private_token(ctx, n));
            }
            NodeKind::FieldDeclaration => {
                let list = &ctx.ast[ctx.ast[Id::<FieldDeclaration>::from_raw(p)].fields];
                return ctx
                    .ast
                    .list(list.variables)
                    .iter()
                    .any(|v| !private_token(ctx, ctx.ast[*v].name));
            }
            NodeKind::TopLevelVariableDeclaration => {
                let list =
                    &ctx.ast[ctx.ast[Id::<TopLevelVariableDeclaration>::from_raw(p)].variables];
                return ctx
                    .ast
                    .list(list.variables)
                    .iter()
                    .any(|v| !private_token(ctx, ctx.ast[*v].name));
            }
            NodeKind::ClassDeclaration => {
                return !private_token(
                    ctx,
                    ctx.ast
                        .begin_token(ctx.ast[Id::<ClassDeclaration>::from_raw(p)].name_part),
                );
            }
            NodeKind::MixinDeclaration => {
                return !private_token(ctx, ctx.ast[Id::<MixinDeclaration>::from_raw(p)].name);
            }
            NodeKind::ExtensionDeclaration => {
                return ctx.ast[Id::<ExtensionDeclaration>::from_raw(p)]
                    .name
                    .is_some_and(|n| !private_token(ctx, n));
            }
            NodeKind::RegularFormalParameter => {
                let n = &ctx.ast[Id::<RegularFormalParameter>::from_raw(p)];
                if n.kind == dartr_element::ParameterKind::Named
                    && n.name.is_some_and(|t| private_token(ctx, t))
                {
                    return false;
                }
            }
            _ => {}
        }
        node = p;
    }
    false
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = ctx.resolved else {
        return;
    };
    let Some(e) = ctx.element(node) else {
        return;
    };
    let base = member::base_element(&r.ctx, e);
    if r.ctx.element_name(base).is_some_and(|n| n.starts_with('_'))
        && public_api_position(ctx, node)
    {
        ctx.report_token(
            out,
            &diag::LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API,
            ctx.ast[Id::<NamedType>::from_raw(node)].name,
            &[],
        );
    }
}
