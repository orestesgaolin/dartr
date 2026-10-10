// Dart source: pkg/linter/lib/src/rules/use_test_throws_matchers.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::Tag;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::TryStatement, "use_test_throws_matchers", check);
}

/// Dart `_Visitor.isTestInvocation`.
fn is_test_invocation(c: &LinterContext<'_>, statement: NodeId, function_name: &str) -> bool {
    let Some(statement) = c.ast.cast::<ExpressionStatement>(statement) else {
        return false;
    };
    let Some(invocation) = c
        .ast
        .cast::<MethodInvocation>(c.ast[statement].expression.raw())
    else {
        return false;
    };
    let Some(element) = c.element(c.ast[invocation].method_name) else {
        return false;
    };
    let element = base(c, element);
    element.tag() == Tag::TopLevelFunction
        && library_uri(c, element) == Some("package:test_api/src/frontend/expect.dart")
        && name(c, element) == Some(function_name)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<TryStatement>::from_raw(node)];
    let statements = c.ast.list_raw(c.ast[n.body].statements);
    if c.ast.list_raw(n.catch_clauses).len() != 1 || statements.is_empty() {
        return;
    }
    let last = *statements.last().unwrap();
    if is_test_invocation(c, last, "fail") && n.finally_block.is_none() {
        c.report_node(out, &diag::USE_TEST_THROWS_MATCHERS, last, &[]);
    }
}
