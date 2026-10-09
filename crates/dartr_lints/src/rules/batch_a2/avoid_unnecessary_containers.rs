// Dart source: pkg/linter/lib/src/rules/avoid_unnecessary_containers.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::InstanceCreationExpression,
        "avoid_unnecessary_containers",
        check,
    );
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(child_type) = context.static_type(node) else {
        return;
    };
    if !super::helpers::implements(
        context,
        child_type,
        "package:flutter/src/widgets/framework.dart",
        "Widget",
    ) {
        return;
    }
    let Some(named) = context
        .ast
        .parent(node)
        .and_then(|p| context.ast.cast::<NamedArgument>(p))
    else {
        return;
    };
    if context.ast.tokens.lexeme(context.ast[named].name) != "child" {
        return;
    }
    let Some(arguments) = context
        .ast
        .parent(named)
        .and_then(|p| context.ast.cast::<ArgumentList>(p))
    else {
        return;
    };
    if context.ast.list(context.ast[arguments].arguments).len() != 1 {
        return;
    }
    let Some(parent) = context
        .ast
        .parent(arguments)
        .and_then(|p| context.ast.cast::<InstanceCreationExpression>(p))
    else {
        return;
    };
    let Some(parent_type) = context.static_type(parent) else {
        return;
    };
    if super::helpers::implements(
        context,
        parent_type,
        "package:flutter/src/widgets/container.dart",
        "Container",
    ) {
        context.report_node(
            out,
            &diag::AVOID_UNNECESSARY_CONTAINERS,
            context.ast[parent].constructor_name,
            &[],
        );
    }
}
