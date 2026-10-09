// Dart source: pkg/linter/lib/src/rules/prefer_asserts_in_initializer_lists.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::member;
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_constructor_declaration("prefer_asserts_in_initializer_lists", check);
    r.add_primary_constructor_body("prefer_asserts_in_initializer_lists", check);
}
fn needs_instance(ctx: &LinterContext<'_>, root: NodeId) -> bool {
    let mut stack = ctx.ast.children(root);
    while let Some(n) = stack.pop() {
        if ctx.ast.kind(n) == NodeKind::ThisExpression {
            return true;
        }
        if ctx.ast.kind(n) == NodeKind::SimpleIdentifier
            && let (Some(r), Some(e)) = (ctx.resolved, ctx.element(n))
            && matches!(
                member::base_element(&r.ctx, e).kind(),
                dartr_element::ElementKind::Field
                    | dartr_element::ElementKind::Method
                    | dartr_element::ElementKind::Getter
                    | dartr_element::ElementKind::Setter
            )
        {
            return true;
        }
        stack.extend(ctx.ast.children(n));
    }
    false
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let body = match ctx.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if n.factory_keyword.is_some() {
                return;
            }
            n.body
        }
        NodeKind::PrimaryConstructorBody => {
            ctx.ast[Id::<PrimaryConstructorBody>::from_raw(node)].body
        }
        _ => return,
    };
    let Some(block_body) = ctx.ast.cast::<BlockFunctionBody>(body.raw()) else {
        return;
    };
    for statement in ctx.ast.list(ctx.ast[ctx.ast[block_body].block].statements) {
        if ctx.ast.kind(*statement) != NodeKind::AssertStatement {
            break;
        }
        if !needs_instance(ctx, statement.raw()) {
            ctx.report_token(
                out,
                &diag::PREFER_ASSERTS_IN_INITIALIZER_LISTS,
                ctx.ast.begin_token(*statement),
                &[],
            );
        }
    }
}
