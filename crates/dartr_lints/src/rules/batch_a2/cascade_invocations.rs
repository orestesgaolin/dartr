// Dart source: pkg/linter/lib/src/rules/cascade_invocations.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementId;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::Block,
        NodeKind::SwitchCase,
        NodeKind::SwitchDefault,
        NodeKind::SwitchPatternCase,
    ] {
        registry.add(kind, "cascade_invocations", check);
    }
}

#[derive(Default)]
struct Cascadable {
    element: Option<ElementId>,
    critical: Vec<NodeId>,
    can_join: bool,
    can_receive: bool,
    can_be_cascaded: bool,
}
fn canonical(context: &LinterContext<'_>, node: NodeId) -> Option<ElementId> {
    context
        .element(node)
        .and_then(|e| super::helpers::base_element(context, e))
}
fn box_for(context: &LinterContext<'_>, statement: NodeId) -> Cascadable {
    if let Some(s) = context.ast.cast::<VariableDeclarationStatement>(statement) {
        let vars = &context.ast[context.ast[s].variables];
        let variables = context.ast.list(vars.variables);
        if variables.len() != 1 {
            return Cascadable::default();
        }
        let v = &context.ast[variables[0]];
        if v.initializer
            .is_some_and(|i| context.ast.kind(i) == NodeKind::AwaitExpression)
        {
            return Cascadable::default();
        }
        return Cascadable {
            element: context.declared_element(variables[0]),
            critical: v.initializer.map(Id::raw).into_iter().collect(),
            can_receive: vars
                .keyword
                .is_none_or(|t| context.ast.tokens.lexeme(t) != "const"),
            ..Default::default()
        };
    }
    let Some(s) = context.ast.cast::<ExpressionStatement>(statement) else {
        return Cascadable::default();
    };
    let expression = context.ast[s].expression.raw();
    match context.ast.kind(expression) {
        NodeKind::MethodInvocation => {
            let n = &context.ast[context.ast.cast::<MethodInvocation>(expression).unwrap()];
            let Some(target) = n
                .target
                .filter(|t| context.ast.kind(*t) == NodeKind::SimpleIdentifier)
            else {
                return Cascadable::default();
            };
            if n.operator
                .is_none_or(|t| context.ast.tokens.lexeme(t) != ".")
            {
                return Cascadable::default();
            }
            Cascadable {
                element: canonical(context, target.raw()),
                critical: vec![n.argument_list.raw()],
                can_join: true,
                can_receive: true,
                can_be_cascaded: true,
            }
        }
        NodeKind::PrefixedIdentifier => {
            let n = &context.ast[context.ast.cast::<PrefixedIdentifier>(expression).unwrap()];
            Cascadable {
                element: canonical(context, n.prefix.raw()),
                critical: vec![n.identifier.raw()],
                can_join: true,
                can_receive: true,
                can_be_cascaded: true,
            }
        }
        NodeKind::PropertyAccess => {
            let n = &context.ast[context.ast.cast::<PropertyAccess>(expression).unwrap()];
            let Some(target) = n
                .target
                .filter(|t| context.ast.kind(*t) == NodeKind::SimpleIdentifier)
            else {
                return Cascadable::default();
            };
            if context.ast.tokens.lexeme(n.operator) != "." {
                return Cascadable::default();
            }
            Cascadable {
                element: canonical(context, target.raw()),
                critical: vec![n.property_name.raw()],
                can_join: true,
                can_receive: true,
                can_be_cascaded: true,
            }
        }
        NodeKind::AssignmentExpression => {
            let n = &context.ast[context
                .ast
                .cast::<AssignmentExpression>(expression)
                .unwrap()];
            let left = n.left_hand_side.raw();
            if context.ast.kind(left) != NodeKind::SimpleIdentifier {
                return Cascadable::default();
            }
            Cascadable {
                element: canonical(context, left),
                critical: vec![n.right_hand_side.raw()],
                can_receive: context.ast.tokens.lexeme(n.operator) != "??=",
                ..Default::default()
            }
        }
        _ => Cascadable::default(),
    }
}
fn references(context: &LinterContext<'_>, root: NodeId, target: ElementId) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if canonical(context, node) == Some(target) {
            return true;
        }
        if context.ast.kind(node) == NodeKind::FunctionExpression {
            return true;
        }
        pending.extend(context.ast.children(node));
    }
    false
}
fn compatible(context: &LinterContext<'_>, current: &Cascadable, previous: &Cascadable) -> bool {
    let Some(element) = current.element else {
        return false;
    };
    previous.can_receive
        && current.can_join
        && (current.can_be_cascaded || previous.can_be_cascaded)
        && previous.element == Some(element)
        && !current
            .critical
            .iter()
            .chain(&previous.critical)
            .any(|&n| references(context, n, element))
}
fn statements(context: &LinterContext<'_>, node: NodeId) -> Vec<NodeId> {
    match context.ast.kind(node) {
        NodeKind::Block => context
            .ast
            .list_raw(context.ast[context.ast.cast::<Block>(node).unwrap()].statements)
            .to_vec(),
        NodeKind::SwitchCase => context
            .ast
            .list_raw(context.ast[context.ast.cast::<SwitchCase>(node).unwrap()].statements)
            .to_vec(),
        NodeKind::SwitchDefault => context
            .ast
            .list_raw(context.ast[context.ast.cast::<SwitchDefault>(node).unwrap()].statements)
            .to_vec(),
        NodeKind::SwitchPatternCase => context
            .ast
            .list_raw(context.ast[context.ast.cast::<SwitchPatternCase>(node).unwrap()].statements)
            .to_vec(),
        _ => vec![],
    }
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let statements = statements(context, node);
    let mut previous = Cascadable::default();
    let mut previous_statement = None;
    let mut first = None;
    for statement in statements {
        let current = box_for(context, statement);
        if compatible(context, &current, &previous) {
            first.get_or_insert(statement);
        } else if let (Some(start), Some(end)) = (first.take(), previous_statement) {
            context.report_offset(
                out,
                &diag::CASCADE_INVOCATIONS,
                context.ast.offset(start) as usize,
                (context.ast.end(end) - context.ast.offset(start)) as usize,
                &[],
            );
        }
        previous = current;
        previous_statement = Some(statement);
    }
    if let (Some(start), Some(end)) = (first, previous_statement) {
        context.report_offset(
            out,
            &diag::CASCADE_INVOCATIONS,
            context.ast.offset(start) as usize,
            (context.ast.end(end) - context.ast.offset(start)) as usize,
            &[],
        );
    }
}
