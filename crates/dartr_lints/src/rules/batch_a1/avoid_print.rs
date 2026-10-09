// Dart source: pkg/linter/lib/src/rules/avoid_print.dart

use super::helpers::{element_library_uri, element_name, unparenthesized};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodInvocation, NodeId, NodeKind, NodeType};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::MethodInvocation, "avoid_print", check);
}

fn is_print(c: &LinterContext<'_>, node: impl Into<NodeId>) -> bool {
    c.element(node).is_some_and(|e| {
        element_name(c, e) == Some("print") && element_library_uri(c, e) == Some("dart:core")
    })
}

fn is_debug_only(c: &LinterContext<'_>, node: NodeId) -> bool {
    let mut child = node;
    while let Some(parent) = c.ast.parent(child) {
        if dartr_ast::FunctionBody::test(c.ast.kind(parent)) {
            return false;
        }
        if let Some(if_) = c.ast.cast::<dartr_ast::IfStatement>(parent)
            && c.ast[if_].then_statement.raw() == child
        {
            let condition = unparenthesized(c.ast, c.ast[if_].expression);
            if c.element(condition).is_some_and(|element| {
                element_name(c, element) == Some("kDebugMode")
                    && element_library_uri(c, element) == Some("package:flutter/foundation.dart")
            }) {
                return true;
            }
        }
        child = parent;
    }
    false
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    if is_print(c, n.method_name) && !is_debug_only(c, node) {
        c.report_node(out, &diag::AVOID_PRINT, n.method_name, &[]);
    }
    for argument in c.ast.list_raw(c.ast[n.argument_list].arguments) {
        if c.ast.kind(*argument) == NodeKind::SimpleIdentifier && is_print(c, *argument) {
            c.report_node(out, &diag::AVOID_PRINT, *argument, &[]);
        }
    }
}
