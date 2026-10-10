// Dart source: pkg/linter/lib/src/rules/use_named_constants.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;
use dartr_element::{ElemRef, FragmentFlags, Tag};
use dartr_typesystem::member;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::InstanceCreationExpression, "use_named_constants", check);
    r.add(
        NodeKind::DotShorthandConstructorInvocation,
        "use_named_constants",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let const_keyword = match kind(c, node) {
        NodeKind::InstanceCreationExpression => c.ast[Id::<InstanceCreationExpression>::from_raw(node)]
            .keyword
            .filter(|&k| lexeme(c, k) == "const"),
        _ => c.ast[Id::<DotShorthandConstructorInvocation>::from_raw(node)].const_keyword,
    };
    if const_keyword.is_none() && !in_constant_context(c, node) {
        return;
    }
    report_when_matching_constant(c, node, out);
}

fn report_when_matching_constant(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(resolved)) = (rctx(c), c.resolved) else {
        return;
    };
    let Some(ty) = c.static_type(node) else { return };
    let Some(element) = ctx.interface_element(ty) else {
        return;
    };
    if !matches!(element.raw().tag(), Tag::Class | Tag::ExtensionType) {
        return;
    }
    let node_field = this_or_ancestor_kind(c, node, NodeKind::VariableDeclaration)
        .and_then(|v| c.declared_element(v));
    if node_field.and_then(|f| enclosing(c, f)) == Some(element.raw()) {
        return;
    }
    let library = resolved.library;
    let value = c.constant_value(node);
    for &field in &ctx.instance(element.upcast()).fields {
        let field = field.raw();
        let flags = flags(c, field);
        if !(is_static(c, field) && flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)) {
            continue;
        }
        if member::is_accessible_in(&ctx, ElemRef::Base(field), library)
            && constant_values_equal(c, &element_constant_value(c, field), &value)
        {
            let text = format!(
                "{}.{}",
                name(c, element.raw()).unwrap_or(""),
                name(c, field).unwrap_or("")
            );
            c.report_node(out, &diag::USE_NAMED_CONSTANTS, node, &[&text]);
            return;
        }
    }
}
