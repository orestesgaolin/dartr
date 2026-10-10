// Dart source: pkg/linter/lib/src/rules/prefer_foreach.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElemRef;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ForStatement, "prefer_foreach", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let parts = c.ast[Id::<ForStatement>::from_raw(node)].for_loop_parts.raw();
    if matches!(
        kind(c, parts),
        NodeKind::ForEachPartsWithDeclaration
            | NodeKind::ForEachPartsWithIdentifier
            | NodeKind::ForEachPartsWithPattern
    ) {
        let mut visitor = PreferForEach { element: None, for_each_statement: None };
        visitor.visit(c, node, out);
    }
}

/// Dart `_PreferForEachVisitor`.
struct PreferForEach {
    element: Option<ElemRef>,
    for_each_statement: Option<NodeId>,
}

impl PreferForEach {
    fn visit(&mut self, c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
        match kind(c, node) {
            NodeKind::Block => {
                let statements = c.ast.list_raw(c.ast[Id::<Block>::from_raw(node)].statements);
                if statements.len() == 1 {
                    self.visit(c, statements[0], out);
                }
            }
            NodeKind::ExpressionStatement => {
                let e = c.ast[Id::<ExpressionStatement>::from_raw(node)].expression.raw();
                self.visit(c, e, out);
            }
            NodeKind::ForStatement => {
                let n = &c.ast[Id::<ForStatement>::from_raw(node)];
                if let Some(parts) = c.ast.cast::<ForEachPartsWithDeclaration>(n.for_loop_parts.raw())
                    && let Some(element) = c.declared_element(c.ast[parts].loop_variable)
                {
                    self.for_each_statement = Some(node);
                    self.element = Some(ElemRef::Base(element));
                    self.visit(c, n.body.raw(), out);
                }
            }
            NodeKind::FunctionExpressionInvocation => {
                let args = arguments(c, c.ast[Id::<FunctionExpressionInvocation>::from_raw(node)].argument_list);
                if args.len() == 1 && canonical_element(c, args[0]) == self.element {
                    self.report(c, out);
                }
            }
            NodeKind::MethodInvocation => {
                let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
                let args = arguments(c, n.argument_list);
                if args.len() == 1
                    && canonical_element(c, args[0]) == self.element
                    && n.target.is_none_or(|t| !self.references(c, t.raw()))
                {
                    self.report(c, out);
                }
            }
            NodeKind::ParenthesizedExpression => {
                let inner = unparenthesized(c, node);
                self.visit(c, inner, out);
            }
            _ => {}
        }
    }

    fn report(&self, c: &LinterContext<'_>, out: &mut Vec<Diagnostic>) {
        // Dart `rule.reportAtNode(forEachStatement)` (a null node reports nothing).
        if let Some(statement) = self.for_each_statement {
            c.report_node(out, &diag::PREFER_FOREACH, statement, &[]);
        }
    }

    /// Dart `_ReferenceFinder.references`.
    fn references(&self, c: &LinterContext<'_>, target: NodeId) -> bool {
        if canonical_element(c, target) == self.element {
            return true;
        }
        let mut stack = vec![target];
        while let Some(n) = stack.pop() {
            if canonical_element(c, n) == self.element {
                return true;
            }
            stack.extend(c.ast.children(n));
        }
        false
    }
}
