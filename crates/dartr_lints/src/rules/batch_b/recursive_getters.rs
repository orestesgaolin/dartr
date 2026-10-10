// Dart source: pkg/linter/lib/src/rules/recursive_getters.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementId;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::FunctionDeclaration, "recursive_getters", check);
    r.add(NodeKind::MethodDeclaration, "recursive_getters", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match kind(c, node) {
        NodeKind::FunctionDeclaration => {
            let function = c.ast[Id::<FunctionDeclaration>::from_raw(node)].function_expression;
            if c.ast[function].parameters.is_some() {
                return;
            }
            verify_element(c, function.raw(), c.declared_element(node), out);
        }
        _ => {
            let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            if n.parameters.is_some() {
                return;
            }
            verify_element(c, n.body.raw(), c.declared_element(node), out);
        }
    }
}

/// Dart `_BodyVisitor`.
fn verify_element(c: &LinterContext<'_>, root: NodeId, element: Option<ElementId>, out: &mut Vec<Diagnostic>) {
    let Some(element) = element else { return };
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match kind(c, n) {
            NodeKind::ListLiteral | NodeKind::SetOrMapLiteral
                if n != root && c.ast.parent(n).is_some() && is_const_literal(c, n) =>
            {
                continue;
            }
            NodeKind::SimpleIdentifier => {
                if is_self_reference(c, n, element) {
                    let name = simple_name(c, Id::from_raw(n));
                    c.report_node(out, &diag::RECURSIVE_GETTERS, n, &[name]);
                }
            }
            _ => {}
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
}

/// Dart `TypedLiteral.isConst`.
fn is_const_literal(c: &LinterContext<'_>, n: NodeId) -> bool {
    let keyword = match kind(c, n) {
        NodeKind::ListLiteral => c.ast[Id::<ListLiteral>::from_raw(n)].const_keyword,
        _ => c.ast[Id::<SetOrMapLiteral>::from_raw(n)].const_keyword,
    };
    keyword.is_some() || in_constant_context(c, n)
}

/// Dart `_BodyVisitor.isSelfReference`.
fn is_self_reference(c: &LinterContext<'_>, node: NodeId, element: ElementId) -> bool {
    if c.element(node) != Some(dartr_element::ElemRef::Base(element)) {
        return false;
    }
    match c.ast.parent(node) {
        Some(p) if kind(c, p) == NodeKind::PrefixedIdentifier => false,
        Some(p) if kind(c, p) == NodeKind::PropertyAccess => c.ast[Id::<PropertyAccess>::from_raw(p)]
            .target
            .is_some_and(|t| kind(c, t) == NodeKind::ThisExpression),
        _ => true,
    }
}
