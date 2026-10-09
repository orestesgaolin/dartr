// Dart source: pkg/linter/lib/src/rules/one_member_abstracts.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ClassElement;
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_class_declaration("one_member_abstracts", check);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<ClassDeclaration>::from_raw(node)];
    if n.abstract_keyword.is_none() || n.extends_clause.is_some() || n.augment_keyword.is_some() {
        return;
    }
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let Some(class) = ctx
        .declared_element(node)
        .and_then(|element| element.cast::<ClassElement>())
    else {
        return;
    };
    let class_data = resolved.ctx.get(class);
    if !resolved
        .ctx
        .list(class_data.interfaces.get().unwrap_or_default())
        .is_empty()
        || !resolved
            .ctx
            .list(class_data.mixins.get().unwrap_or_default())
            .is_empty()
        || !class_data.fields.is_empty()
        || class_data.methods.len() != 1
    {
        return;
    }
    let method = class_data.methods[0];
    if !dartr_link::dump::is_abstract(&resolved.ctx, method.raw()) {
        return;
    }
    let Some(method_name) = resolved.ctx.element_name(method.raw()) else {
        return;
    };
    let type_name = ctx.ast.begin_token(n.name_part);
    ctx.report_token(out, &diag::ONE_MEMBER_ABSTRACTS, type_name, &[method_name]);
}
