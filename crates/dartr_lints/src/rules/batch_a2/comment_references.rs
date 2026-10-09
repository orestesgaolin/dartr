// Dart source: pkg/linter/lib/src/rules/comment_references.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::Comment, "comment_references", check_comment);
    registry.add(
        NodeKind::CommentReference,
        "comment_references",
        check_reference,
    );
}
fn check_comment(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<Comment>(node).unwrap()];
    for &token in context.ast.token_list(n.tokens) {
        let t = context.ast.tokens.get(token);
        if t.is_synthetic() {
            continue;
        }
        let text = context.ast.tokens.lexeme(token);
        let mut start = 0;
        while let Some(left) = text[start..].find('[').map(|i| start + i) {
            let Some(right) = text[left + 1..].find(']').map(|i| left + 1 + i) else {
                break;
            };
            let reference = &text[left + 1..right];
            if matches!(reference, "this" | "null" | "true" | "false") {
                context.report_offset(
                    out,
                    &diag::COMMENT_REFERENCES,
                    t.offset as usize + left + 1,
                    reference.len(),
                    &[],
                );
            }
            start = right + 1;
        }
    }
}
fn check_reference(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<CommentReference>(node).unwrap()];
    if n.is_synthetic || context.is_synthetic(n.expression) {
        return;
    }
    match context.ast.kind(n.expression) {
        NodeKind::SimpleIdentifier | NodeKind::PrefixedIdentifier
            if context.element(n.expression).is_none()
                && !is_link_reference(context, node, &context.text(n.expression)) =>
        {
            context.report_node(out, &diag::COMMENT_REFERENCES, n.expression, &[]);
        }
        NodeKind::PropertyAccess => {
            let p = &context.ast[context.ast.cast::<PropertyAccess>(n.expression).unwrap()];
            if context.element(p.property_name).is_none()
                && p.target.is_some_and(|target| {
                    context.ast.kind(target) == NodeKind::PrefixedIdentifier
                        && !is_link_reference(context, node, &context.text(n.expression))
                })
            {
                context.report_node(out, &diag::COMMENT_REFERENCES, n.expression, &[]);
            }
        }
        _ => {}
    }
}

fn is_link_reference(context: &LinterContext<'_>, node: NodeId, name: &str) -> bool {
    let Some(comment) =
        std::iter::successors(context.ast.parent(node), |node| context.ast.parent(*node))
            .find_map(|node| context.ast.cast::<Comment>(node))
    else {
        return false;
    };
    context
        .ast
        .token_list(context.ast[comment].tokens)
        .iter()
        .any(|&token| {
            let text = context.ast.tokens.lexeme(token);
            let Some(left) = text.find('[') else {
                return false;
            };
            let prefix = &text[..left];
            let slash_count = prefix.chars().take_while(|&c| c == '/').count();
            if slash_count < 3 || !prefix[slash_count..].trim().is_empty() {
                return false;
            }
            let Some(right) = text[left + 1..].find(']').map(|i| left + 1 + i) else {
                return false;
            };
            text.get(right + 1..right + 2) == Some(":") && &text[left + 1..right] == name
        })
}
