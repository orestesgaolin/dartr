// Dart source: pkg/linter/lib/src/rules/prefer_mixin.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FragmentFlags, Tag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::WithClause, "prefer_mixin", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    for &mixin in c
        .ast
        .list(c.ast[Id::<WithClause>::from_raw(node)].mixin_types)
    {
        let Some(ty) = c
            .resolved
            .unwrap()
            .tables
            .annotation_type
            .get(mixin.raw())
            .copied()
        else {
            continue;
        };
        let Some(element) = ctx.interface_element(ty) else {
            continue;
        };
        let element = element.raw();
        if element.tag() == Tag::Mixin {
            continue;
        }
        if element.tag() == Tag::Class
            && !flags(c, element).contains(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS)
        {
            let name = lexeme(c, c.ast[mixin].name);
            c.report_node(out, &diag::PREFER_MIXIN, mixin, &[name]);
        }
    }
}
