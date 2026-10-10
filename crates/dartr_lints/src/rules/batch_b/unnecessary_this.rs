// Dart source: pkg/linter/lib/src/rules/unnecessary_this.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, Tag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ConstructorFieldInitializer, "unnecessary_this", visit_constructor_field_initializer);
    r.add(NodeKind::ThisExpression, "unnecessary_this", visit_this_expression);
}

fn visit_constructor_field_initializer(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(this_keyword) = c.ast[Id::<ConstructorFieldInitializer>::from_raw(node)].this_keyword {
        c.report_token(out, &diag::UNNECESSARY_THIS, this_keyword, &[]);
    }
}

fn visit_this_expression(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(parent) = c.ast.parent(node) else { return };
    let element = if let Some(access) = c.ast.cast::<PropertyAccess>(parent) {
        if lexeme(c, c.ast[access].operator).starts_with('?') {
            return;
        }
        c.write_or_read_element(c.ast[access].property_name)
    } else if let Some(invocation) = c.ast.cast::<MethodInvocation>(parent) {
        if c.ast[invocation].operator.is_some_and(|o| lexeme(c, o).starts_with('?')) {
            return;
        }
        c.element(c.ast[invocation].method_name)
    } else {
        return;
    };
    let element = element.map(|e| base(c, e));
    if can_reference_element_without_this_prefix(c, element, node) {
        let this_keyword = c.ast[Id::<ThisExpression>::from_raw(node)].this_keyword;
        c.report_token(out, &diag::UNNECESSARY_THIS, this_keyword, &[]);
    }
}

/// Dart `_canReferenceElementWithoutThisPrefix`, with the lookup of
/// `resolveNameInScope` recorded by the resolver.
fn can_reference_element_without_this_prefix(c: &LinterContext<'_>, element: Option<ElementId>, node: NodeId) -> bool {
    let Some(element) = element else { return false };
    let Some(&(getter, setter)) = c.resolved.as_ref().and_then(|r| r.this_scope_lookup.get(node)) else {
        return false;
    };
    let should_resolve_setter = element.tag() == Tag::Setter;
    let (requested, different) = if should_resolve_setter { (setter, getter) } else { (getter, setter) };
    if let Some(requested) = requested {
        return requested == element;
    }
    if let Some(different) = different {
        return enclosing(c, different).is_some_and(|e| e.tag() == Tag::Class);
    }
    true
}
