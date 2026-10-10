// Dart source: pkg/linter/lib/src/rules/use_string_buffers.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElemRef;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::DoStatement, "use_string_buffers", check);
    r.add(NodeKind::ForStatement, "use_string_buffers", check);
    r.add(NodeKind::WhileStatement, "use_string_buffers", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let body = match kind(c, node) {
        NodeKind::DoStatement => c.ast[Id::<DoStatement>::from_raw(node)].body.raw(),
        NodeKind::ForStatement => c.ast[Id::<ForStatement>::from_raw(node)].body.raw(),
        _ => c.ast[Id::<WhileStatement>::from_raw(node)].body.raw(),
    };
    let mut visitor = UseStringBuffer {
        local_elements: Vec::new(),
    };
    visitor.visit(c, body, out);
}

/// Dart `_UseStringBufferVisitor`.
struct UseStringBuffer {
    local_elements: Vec<Option<dartr_element::ElementId>>,
}

impl UseStringBuffer {
    fn visit(&mut self, c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
        match kind(c, node) {
            NodeKind::AssignmentExpression => self.assignment(c, node, out),
            NodeKind::Block => {
                for &s in c
                    .ast
                    .list_raw(c.ast[Id::<Block>::from_raw(node)].statements)
                {
                    self.visit(c, s, out);
                }
            }
            NodeKind::ExpressionStatement => {
                let e = c.ast[Id::<ExpressionStatement>::from_raw(node)]
                    .expression
                    .raw();
                self.visit(c, e, out);
            }
            NodeKind::ParenthesizedExpression => {
                let e = unparenthesized(c, node);
                self.visit(c, e, out);
            }
            NodeKind::VariableDeclarationStatement => {
                let list = c.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables;
                for &v in c.ast.list(c.ast[list].variables) {
                    self.local_elements.push(c.declared_element(v));
                }
            }
            _ => {}
        }
    }

    fn assignment(&mut self, c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
        let (Some(ctx), Some(resolved)) = (rctx(c), c.resolved) else {
            return;
        };
        let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
        let operator = lexeme(c, n.operator);
        if operator != "+=" && operator != "=" {
            return;
        }
        let left = n.left_hand_side.raw();
        let write_type = resolved.tables.write_type.get(node).copied();
        if kind(c, left) == NodeKind::SimpleIdentifier
            && let Some(write_type) = write_type
            && ctx.interface_element(write_type).is_some()
            && ctx.is_dart_core_string(write_type)
        {
            let write_element = resolved.tables.write_element.get(node).map(|&e| base(c, e));
            if operator == "+=" && !self.local_elements.contains(&write_element) {
                c.report_node(out, &diag::USE_STRING_BUFFERS, node, &[]);
            }
            if operator == "=" {
                identifier_is_prefix(c, left, n.right_hand_side.raw(), out);
            }
        }
    }
}

/// Dart `_IdentifierIsPrefixVisitor`.
fn identifier_is_prefix(
    c: &LinterContext<'_>,
    identifier: NodeId,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    match kind(c, node) {
        NodeKind::BinaryExpression => {
            let b = &c.ast[Id::<BinaryExpression>::from_raw(node)];
            if lexeme(c, b.operator) == "+" {
                identifier_is_prefix(c, identifier, b.left_operand.raw(), out);
            }
        }
        NodeKind::InterpolationExpression => {
            let e = c.ast[Id::<InterpolationExpression>::from_raw(node)]
                .expression
                .raw();
            identifier_is_prefix(c, identifier, e, out);
        }
        NodeKind::ParenthesizedExpression => {
            let e = unparenthesized(c, node);
            identifier_is_prefix(c, identifier, e, out);
        }
        NodeKind::SimpleIdentifier => {
            let e: Option<ElemRef> = c.element(node);
            if e == c.element(identifier) {
                c.report_node(out, &diag::USE_STRING_BUFFERS, identifier, &[]);
            }
        }
        NodeKind::StringInterpolation => {
            let elements = c
                .ast
                .list_raw(c.ast[Id::<StringInterpolation>::from_raw(node)].elements);
            if elements.len() >= 2
                && c.ast
                    .cast::<InterpolationString>(elements[0])
                    .is_some_and(|s| c.ast[s].value.is_empty())
            {
                identifier_is_prefix(c, identifier, elements[1], out);
            }
        }
        _ => {}
    }
}
