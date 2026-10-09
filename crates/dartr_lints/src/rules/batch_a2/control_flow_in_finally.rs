// Dart source: pkg/linter/lib/src/rules/control_flow_in_finally.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::BreakStatement,
        NodeKind::ContinueStatement,
        NodeKind::ReturnStatement,
    ] {
        registry.add(kind, "control_flow_in_finally", check);
    }
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (kind, label) = match context.ast.kind(node) {
        NodeKind::BreakStatement => (
            "break",
            context.ast[context.ast.cast::<BreakStatement>(node).unwrap()].label,
        ),
        NodeKind::ContinueStatement => (
            "continue",
            context.ast[context.ast.cast::<ContinueStatement>(node).unwrap()].label,
        ),
        NodeKind::ReturnStatement => ("return", None),
        _ => return,
    };
    let labeled_target = label.and_then(|label| label_target(context, label.raw()));
    let mut child = node;
    let mut nested_function = false;
    let mut local_target = false;
    while let Some(parent) = context.ast.parent(child) {
        if FunctionBody::test(context.ast.kind(parent)) {
            nested_function = true;
        }
        if label.is_none()
            && matches!(kind, "break" | "continue")
            && matches!(
                context.ast.kind(parent),
                NodeKind::DoStatement | NodeKind::ForStatement | NodeKind::WhileStatement
            )
        {
            local_target = true;
        }
        if label.is_none()
            && kind == "break"
            && context.ast.kind(parent) == NodeKind::SwitchStatement
        {
            local_target = true;
        }
        if context.ast.kind(parent) == NodeKind::TryStatement {
            let try_node = &context.ast[context.ast.cast::<TryStatement>(parent).unwrap()];
            if try_node.finally_block.is_some_and(|block| {
                std::iter::successors(Some(node), |node| context.ast.parent(*node))
                    .any(|ancestor| ancestor == block.raw())
            }) && !nested_function
                && !local_target
                && !labeled_target.is_some_and(|target| {
                    std::iter::successors(Some(target), |node| context.ast.parent(*node))
                        .any(|ancestor| ancestor == parent)
                })
            {
                context.report_node(out, &diag::CONTROL_FLOW_IN_FINALLY, node, &[kind]);
            }
            return;
        }
        child = parent;
    }
}

fn label_target(context: &LinterContext<'_>, reference: NodeId) -> Option<NodeId> {
    let element = context
        .element(reference)
        .and_then(|element| super::helpers::base_element(context, element))?;
    let label = (0..context.ast.node_count())
        .map(NodeId::from_index)
        .find(|&node| {
            context.ast.kind(node) == NodeKind::Label
                && context.declared_element(node) == Some(element)
        })?;
    let mut target = context.ast.parent(label)?;
    while let Some(statement) = context.ast.cast::<LabeledStatement>(target) {
        target = context.ast[statement].statement.raw();
    }
    Some(target)
}
