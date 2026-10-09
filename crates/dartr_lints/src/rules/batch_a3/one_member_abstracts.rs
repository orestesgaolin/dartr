// Dart source: pkg/linter/lib/src/rules/one_member_abstracts.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_class_declaration("one_member_abstracts", check);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<ClassDeclaration>::from_raw(node)];
    if n.abstract_keyword.is_none()
        || n.extends_clause.is_some()
        || n.augment_keyword.is_some()
        || n.implements_clause.is_some()
        || n.with_clause.is_some()
    {
        return;
    }
    let Some(body) = ctx.ast.cast::<BlockClassBody>(n.body.raw()) else {
        return;
    };
    let members = ctx.ast.list(ctx.ast[body].members);
    if members.len() != 1 {
        return;
    }
    let Some(method) = ctx.ast.cast::<MethodDeclaration>(members[0]) else {
        return;
    };
    if ctx.ast.kind(ctx.ast[method].body) != NodeKind::EmptyFunctionBody {
        return;
    }
    let method_name = ctx.ast.tokens.lexeme(ctx.ast[method].name);
    let type_name = ctx.ast.begin_token(n.name_part);
    ctx.report_token(out, &diag::ONE_MEMBER_ABSTRACTS, type_name, &[method_name]);
}
