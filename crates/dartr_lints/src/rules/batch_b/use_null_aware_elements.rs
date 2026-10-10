// Dart source: pkg/linter/lib/src/rules/use_null_aware_elements.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, Tag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::NullAwareElements) {
        return;
    }
    r.add(NodeKind::IfElement, "use_null_aware_elements", check);
}

fn canonical_base(c: &LinterContext<'_>, node: NodeId) -> Option<ElementId> {
    canonical_element(c, node).map(|e| base(c, e))
}

/// `SimpleIdentifier(canonicalElement: var reference)`.
fn simple_reference(c: &LinterContext<'_>, node: NodeId) -> Option<Option<ElementId>> {
    (kind(c, node) == NodeKind::SimpleIdentifier).then(|| canonical_base(c, node))
}

/// `PostfixExpression(operand: SimpleIdentifier(...), operator: '!')`.
fn bang_reference(c: &LinterContext<'_>, node: NodeId) -> Option<Option<ElementId>> {
    let p = c.ast.cast::<PostfixExpression>(node)?;
    if lexeme(c, c.ast[p].operator) != "!" {
        return None;
    }
    simple_reference(c, c.ast[p].operand.raw())
}

fn spread_expression(c: &LinterContext<'_>, node: NodeId) -> Option<NodeId> {
    c.ast.cast::<SpreadElement>(node).map(|s| c.ast[s].expression.raw())
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<IfElement>::from_raw(node)];
    if n.else_keyword.is_some() {
        return;
    }
    let then_element = n.then_element.raw();
    let mut target: Option<ElementId> = None;
    let expression = n.expression.raw();
    if let Some(binary) = c.ast.cast::<BinaryExpression>(expression) {
        let b = &c.ast[binary];
        if lexeme(c, b.operator) == "!=" {
            if kind(c, b.left_operand) == NodeKind::NullLiteral {
                target = canonical_base(c, b.right_operand.raw());
            } else if kind(c, b.right_operand) == NodeKind::NullLiteral {
                target = canonical_base(c, b.left_operand.raw());
            }
        }
    } else if let Some(case_clause) = n.case_clause {
        let guarded = c.ast[case_clause].guarded_pattern;
        let g = &c.ast[guarded];
        if g.when_clause.is_none()
            && let Some(null_check) = c.ast.cast::<NullCheckPattern>(g.pattern.raw())
            && kind(c, c.ast[null_check].pattern) == NodeKind::DeclaredVariablePattern
        {
            target = c.declared_element(c.ast[null_check].pattern);
        }
    }
    let Some(target) = target else { return };
    let promotable = is_promotable(target);
    let getter = target.tag() == Tag::Getter;
    let mut matched = false;
    if promotable {
        let reference = simple_reference(c, then_element)
            .or_else(|| spread_expression(c, then_element).and_then(|e| simple_reference(c, e)));
        if let Some(reference) = reference {
            matched = Some(target) == reference;
        } else if let Some(entry) = c.ast.cast::<MapLiteralEntry>(then_element) {
            let (key, value) = (c.ast[entry].key.raw(), c.ast[entry].value.raw());
            matched = (kind(c, key) == NodeKind::SimpleIdentifier && Some(target) == canonical_base(c, key))
                || (kind(c, value) == NodeKind::SimpleIdentifier && Some(target) == canonical_base(c, value));
        }
    } else if getter {
        let reference = bang_reference(c, then_element)
            .or_else(|| spread_expression(c, then_element).and_then(|e| bang_reference(c, e)));
        if let Some(reference) = reference {
            matched = Some(target) == reference;
        } else if let Some(entry) = c.ast.cast::<MapLiteralEntry>(then_element) {
            let (key, value) = (c.ast[entry].key.raw(), c.ast[entry].value.raw());
            matched = bang_reference(c, key) == Some(Some(target)) || bang_reference(c, value) == Some(Some(target));
        }
    }
    if matched {
        c.report_token(out, &diag::USE_NULL_AWARE_ELEMENTS, n.if_keyword, &[]);
    }
}
