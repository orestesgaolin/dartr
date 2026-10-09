// Dart source: pkg/linter/lib/src/rules/cascade_invocations.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ElementId, FieldElement, FormalParameterElement, FragmentFlags, GetterElement,
    LocalVariableElement, MethodElement, SetterElement, TopLevelVariableElement,
};

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

fn property_variable(context: &LinterContext<'_>, element: ElementId) -> Option<ElementId> {
    let resolved = context.resolved?;
    if let Some(getter) = element.cast::<GetterElement>() {
        return resolved.ctx.get(getter).variable.get().map(|id| id.raw());
    }
    if let Some(setter) = element.cast::<SetterElement>() {
        return resolved.ctx.get(setter).variable.get().map(|id| id.raw());
    }
    Some(element)
}

fn unparenthesized(context: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(parenthesized) = context.ast.cast::<ParenthesizedExpression>(node) {
        node = context.ast[parenthesized].expression.raw();
    }
    node
}

fn is_static(context: &LinterContext<'_>, element: ElementId) -> bool {
    context
        .resolved
        .and_then(|resolved| resolved.ctx.element_data(element))
        .and_then(|data| context.resolved?.ctx.fragment_data(data.first_fragment))
        .is_some_and(|fragment| {
            fragment
                .flags
                .has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
                || fragment
                    .flags
                    .has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
        })
}

fn is_variable(element: ElementId) -> bool {
    element.is::<FieldElement>()
        || element.is::<TopLevelVariableElement>()
        || element.is::<LocalVariableElement>()
        || element.is::<FormalParameterElement>()
}

fn prefix_element(context: &LinterContext<'_>, expression: NodeId) -> Option<ElementId> {
    let expression = unparenthesized(context, expression);
    if let Some(prefixed) = context.ast.cast::<PrefixedIdentifier>(expression) {
        return canonical(context, context.ast[prefixed].prefix.raw());
    }
    if let Some(property) = context.ast.cast::<PropertyAccess>(expression) {
        let property = &context.ast[property];
        if context.ast.tokens.lexeme(property.operator) == "."
            && property
                .target
                .is_some_and(|target| context.ast.kind(target) == NodeKind::SimpleIdentifier)
        {
            return property
                .target
                .and_then(|target| canonical(context, target.raw()));
        }
    }
    None
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
    let expression = unparenthesized(context, context.ast[s].expression.raw());
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
            let Some(method) = context
                .element(n.method_name)
                .and_then(|element| super::helpers::base_element(context, element))
            else {
                return Cascadable::default();
            };
            if !method.is::<MethodElement>() || is_static(context, method) {
                return Cascadable::default();
            }
            Cascadable {
                element: canonical(context, target.raw()),
                critical: vec![n.method_name.raw(), n.argument_list.raw()],
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
            let left = unparenthesized(context, n.left_hand_side.raw());
            if context.ast.kind(left) == NodeKind::SimpleIdentifier {
                return Cascadable {
                    element: canonical(context, left),
                    critical: vec![n.right_hand_side.raw()],
                    can_receive: context.ast.tokens.lexeme(n.operator) != "??=",
                    ..Default::default()
                };
            }
            let element = prefix_element(context, left);
            let can_receive = context.ast.tokens.lexeme(n.operator) != "??="
                && element
                    .is_some_and(|element| is_variable(element) && !is_static(context, element));
            Cascadable {
                element,
                critical: vec![n.right_hand_side.raw()],
                can_join: true,
                can_receive,
                can_be_cascaded: true,
            }
        }
        NodeKind::CascadeExpression => {
            let n = &context.ast[context.ast.cast::<CascadeExpression>(expression).unwrap()];
            let simple = context.ast.kind(n.target) == NodeKind::SimpleIdentifier;
            Cascadable {
                element: canonical(context, n.target.raw()),
                critical: context.ast.list_raw(n.cascade_sections).to_vec(),
                can_join: simple,
                can_receive: simple,
                can_be_cascaded: true,
            }
        }
        _ => Cascadable::default(),
    }
}
fn references(context: &LinterContext<'_>, root: NodeId, target: ElementId) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let node_element = canonical(context, node);
        if node_element == Some(target)
            || node_element.and_then(|element| property_variable(context, element)) == Some(target)
            || property_variable(context, target)
                .is_some_and(|target_variable| node_element == Some(target_variable))
        {
            return true;
        }
        if context.ast.kind(node) == NodeKind::FunctionExpression
            && property_variable(context, target).is_some_and(|target| {
                target.is::<FieldElement>() || target.is::<TopLevelVariableElement>()
            })
        {
            return true;
        }
        if let Some(element) = context
            .element(node)
            .and_then(|element| super::helpers::base_element(context, element))
            && element.is::<MethodElement>()
            && property_variable(context, target).is_some_and(|target| {
                target.is::<FieldElement>() || target.is::<TopLevelVariableElement>()
            })
            && context.resolved.is_some_and(|resolved| {
                let executable_enclosing = resolved
                    .ctx
                    .element_data(element)
                    .and_then(|data| data.enclosing);
                let target = property_variable(context, target).unwrap_or(target);
                let target_enclosing = resolved
                    .ctx
                    .element_data(target)
                    .and_then(|data| data.enclosing);
                executable_enclosing.is_some() && executable_enclosing == target_enclosing
            })
        {
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
