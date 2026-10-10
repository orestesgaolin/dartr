// Dart source: pkg/linter/lib/src/rules/sort_child_properties_last.dart
use super::flutter::*;
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::InstanceCreationExpression,
        "sort_child_properties_last",
        check,
    );
}

/// Dart `_Visitor.isChildArg`.
fn is_child_arg(c: &LinterContext<'_>, argument: NodeId) -> bool {
    let Some(named) = c.ast.cast::<NamedArgument>(argument) else {
        return false;
    };
    let name = lexeme(c, c.ast[named].name);
    (name == "child" || name == "children")
        && is_widget_property(c, c.static_type(c.ast[named].argument_expression))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if !is_widget_type(c, c.static_type(node)) {
        return;
    }
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    let arguments = c.ast.list_raw(c.ast[n.argument_list].arguments);
    if arguments.len() < 2
        || is_child_arg(c, *arguments.last().unwrap())
        || arguments.iter().filter(|&&a| is_child_arg(c, a)).count() != 1
    {
        return;
    }
    let after_child: Vec<NodeId> = arguments
        .iter()
        .rev()
        .take_while(|&&a| !is_child_arg(c, a))
        .copied()
        .collect();
    let only_closures_after_child = !after_child.iter().any(|&a| {
        c.ast.cast::<NamedArgument>(a).is_some_and(|named| {
            kind(c, c.ast[named].argument_expression) != NodeKind::FunctionExpression
        })
    });
    if !only_closures_after_child {
        let argument = *arguments.iter().find(|&&a| is_child_arg(c, a)).unwrap();
        let name = lexeme(c, c.ast[c.ast.cast::<NamedArgument>(argument).unwrap()].name);
        c.report_node(out, &diag::SORT_CHILD_PROPERTIES_LAST, argument, &[name]);
    }
}
