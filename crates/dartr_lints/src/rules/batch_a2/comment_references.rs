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

fn reference_element(
    context: &LinterContext<'_>,
    expression: NodeId,
) -> Option<dartr_element::ElemRef> {
    match context.ast.kind(expression) {
        NodeKind::SimpleIdentifier => context.element(expression),
        NodeKind::PrefixedIdentifier => context
            .element(context.ast[context.ast.cast::<PrefixedIdentifier>(expression)?].identifier),
        NodeKind::PropertyAccess => context
            .element(context.ast[context.ast.cast::<PropertyAccess>(expression)?].property_name),
        _ => context.element(expression),
    }
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
fn comment_link_references(context: &LinterContext<'_>, node: NodeId) -> Vec<String> {
    let Some(comment) = super::helpers::ancestors(context.ast, node)
        .find_map(|ancestor| context.ast.cast::<Comment>(ancestor))
    else {
        return Vec::new();
    };
    let mut references = Vec::new();
    for &token in context.ast.token_list(context.ast[comment].tokens) {
        let text = context.ast.tokens.lexeme(token);
        let Some(left) = text.find('[') else {
            continue;
        };
        let Some(right) = text[left + 1..].find(']').map(|index| left + 1 + index) else {
            continue;
        };
        let prefix = &text[..left];
        let slash_count = prefix.bytes().take_while(|&byte| byte == b'/').count();
        let is_comment_start =
            slash_count >= 3 && prefix[slash_count..].chars().all(char::is_whitespace);
        if is_comment_start && text.as_bytes().get(right + 1) == Some(&b':') {
            references.push(text[left + 1..right].to_string());
        }
    }
    references
}

fn identifier_name(context: &LinterContext<'_>, expression: NodeId) -> Option<String> {
    match context.ast.kind(expression) {
        NodeKind::SimpleIdentifier => Some(
            context
                .ast
                .tokens
                .lexeme(context.ast[context.ast.cast::<SimpleIdentifier>(expression)?].token)
                .to_string(),
        ),
        NodeKind::PrefixedIdentifier => {
            let identifier = &context.ast[context.ast.cast::<PrefixedIdentifier>(expression)?];
            Some(format!(
                "{}.{}",
                context
                    .ast
                    .tokens
                    .lexeme(context.ast[identifier.prefix].token),
                context
                    .ast
                    .tokens
                    .lexeme(context.ast[identifier.identifier].token)
            ))
        }
        _ => None,
    }
}

fn check_reference(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<CommentReference>(node).unwrap()];
    if n.is_synthetic || context.is_synthetic(n.expression) {
        return;
    }
    let expression = n.expression.raw();
    let link_references = comment_link_references(context, node);
    if Identifier::test(context.ast.kind(expression)) {
        if reference_element(context, expression).is_none()
            && identifier_name(context, expression)
                .is_some_and(|name| !link_references.contains(&name))
        {
            context.report_node(out, &diag::COMMENT_REFERENCES, expression, &[]);
        }
    } else if let Some(property) = context.ast.cast::<PropertyAccess>(expression) {
        let property = &context.ast[property];
        if context.element(property.property_name).is_none()
            && let Some(target) = property
                .target
                .and_then(|target| context.ast.cast::<PrefixedIdentifier>(target.raw()))
        {
            let target = &context.ast[target];
            let name = format!(
                "{}.{}.{}",
                context.ast.tokens.lexeme(context.ast[target.prefix].token),
                context
                    .ast
                    .tokens
                    .lexeme(context.ast[target.identifier].token),
                context
                    .ast
                    .tokens
                    .lexeme(context.ast[property.property_name].token)
            );
            if !link_references.contains(&name) {
                context.report_node(out, &diag::COMMENT_REFERENCES, expression, &[]);
            }
        }
    }
}
