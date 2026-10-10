// Dart source: pkg/linter/lib/src/rules/unnecessary_null_aware_operator_on_extension_on_nullable.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElementId, ExtensionElement, Nullability, Tag};
use dartr_typesystem::TypeExt;

const RULE: &str = "unnecessary_null_aware_operator_on_extension_on_nullable";

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::IndexExpression, RULE, index);
    r.add(NodeKind::MethodInvocation, RULE, method);
    r.add(NodeKind::PropertyAccess, RULE, property);
}

/// Dart `isExtensionOverrideOrStaticTypeIsNullable` of the real target.
fn target_is_override_or_nullable(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(target) = real_target(c, node) else {
        return false;
    };
    kind(c, target) == NodeKind::ExtensionOverride
        || c.static_type(target)
            .is_some_and(|t| ctx.nullability_suffix(t) == Nullability::Question)
}

/// Dart `_isExtensionOnNullableType`.
fn is_extension_on_nullable_type(c: &LinterContext<'_>, enclosing: Option<ElementId>) -> bool {
    let (Some(ctx), Some(ts)) = (rctx(c), c.type_system()) else {
        return false;
    };
    let Some(e) = enclosing.filter(|e| e.tag() == Tag::Extension) else {
        return false;
    };
    ctx.get(EId::<ExtensionElement>::from_raw(e))
        .extended_type
        .get()
        .is_some_and(|t| ts.is_nullable(t))
}

fn write_enclosing(c: &LinterContext<'_>, assignment: NodeId) -> Option<ElementId> {
    let element = *c.resolved?.tables.write_element.get(assignment)?;
    enclosing(c, base(c, element))
}

/// Whether [node] is in a cascade with `?..`.
fn null_aware_cascade(c: &LinterContext<'_>, period: Option<dartr_syntax::TokenId>) -> bool {
    period.is_some_and(|p| lexeme(c, p) == "?..")
}

fn index(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<IndexExpression>::from_raw(node)];
    let Some(question) = n.question else { return };
    let null_aware = n.question.is_some() || null_aware_cascade(c, n.period);
    if !null_aware || !target_is_override_or_nullable(c, node) {
        return;
    }
    // Dart `inSetterContext()`.
    let in_setter_context = c.ast.parent(node).is_some_and(|p| match kind(c, p) {
        NodeKind::AssignmentExpression => {
            c.ast[Id::<AssignmentExpression>::from_raw(p)].left_hand_side.raw() == node
        }
        NodeKind::PrefixExpression => {
            let pe = &c.ast[Id::<PrefixExpression>::from_raw(p)];
            matches!(lexeme(c, pe.operator), "++" | "--")
        }
        NodeKind::PostfixExpression => {
            let pe = &c.ast[Id::<PostfixExpression>::from_raw(p)];
            matches!(lexeme(c, pe.operator), "++" | "--")
        }
        _ => false,
    });
    let enclosing_element = if in_setter_context {
        this_or_ancestor_kind(c, node, NodeKind::AssignmentExpression).and_then(|a| write_enclosing(c, a))
    } else {
        c.element(node).and_then(|e| enclosing(c, base(c, e)))
    };
    if is_extension_on_nullable_type(c, enclosing_element) {
        c.report_token(out, &diag::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, question, &[]);
    }
}

fn method(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    let Some(operator) = n.operator else { return };
    if !matches!(lexeme(c, operator), "?." | "?..") || !target_is_override_or_nullable(c, node) {
        return;
    }
    let enclosing_element = c.element(n.method_name).and_then(|e| enclosing(c, base(c, e)));
    if is_extension_on_nullable_type(c, enclosing_element) {
        c.report_token(out, &diag::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, operator, &[]);
    }
}

fn property(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<PropertyAccess>::from_raw(node)];
    if !matches!(lexeme(c, n.operator), "?." | "?..") || !target_is_override_or_nullable(c, node) {
        return;
    }
    let real_parent = ancestors(c, node).find(|&p| kind(c, p) != NodeKind::ParenthesizedExpression);
    let enclosing_element = match real_parent {
        Some(p) if kind(c, p) == NodeKind::AssignmentExpression => write_enclosing(c, p),
        _ => c.element(n.property_name).and_then(|e| enclosing(c, base(c, e))),
    };
    if is_extension_on_nullable_type(c, enclosing_element) {
        c.report_token(out, &diag::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, n.operator, &[]);
    }
}
