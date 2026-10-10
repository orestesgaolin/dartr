// Dart source: pkg/linter/lib/src/rules/unnecessary_statements.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FragmentFlags, Tag, TypeKind};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ExpressionStatement,
        "unnecessary_statements",
        statement,
    );
    r.add(
        NodeKind::ForStatement,
        "unnecessary_statements",
        for_statement,
    );
    r.add(
        NodeKind::CascadeExpression,
        "unnecessary_statements",
        cascade,
    );
}

fn report(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    c.report_node(out, &diag::UNNECESSARY_STATEMENTS, node, &[]);
}

/// Whether [element] is a getter that is declared explicitly (Dart
/// `element is GetterElement && element.isOriginDeclaration`).
fn is_declared_getter(c: &LinterContext<'_>, element: Option<dartr_element::ElemRef>) -> bool {
    element.is_some_and(|e| {
        let e = base(c, e);
        e.tag() == Tag::Getter
            && flags(c, e).contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
    })
}

/// Dart `_ReportNoClearEffectVisitor`.
fn report_no_clear_effect(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match kind(c, node) {
        NodeKind::AsExpression
        | NodeKind::AssignmentExpression
        | NodeKind::AwaitExpression
        | NodeKind::CascadeExpression
        | NodeKind::FunctionExpressionInvocation
        | NodeKind::InstanceCreationExpression
        | NodeKind::MethodInvocation
        | NodeKind::PatternAssignment
        | NodeKind::PostfixExpression
        | NodeKind::RethrowExpression
        | NodeKind::SuperConstructorInvocation
        | NodeKind::ThrowExpression => {}
        NodeKind::BinaryExpression => {
            let b = &c.ast[Id::<BinaryExpression>::from_raw(node)];
            if matches!(lexeme(c, b.operator), "??" | "||" | "&&") {
                report_no_clear_effect(c, b.right_operand.raw(), out);
            } else {
                report(c, node, out);
            }
        }
        NodeKind::ConditionalExpression => {
            let n = &c.ast[Id::<ConditionalExpression>::from_raw(node)];
            report_no_clear_effect(c, n.then_expression.raw(), out);
            report_no_clear_effect(c, n.else_expression.raw(), out);
        }
        NodeKind::PrefixedIdentifier => {
            let identifier = c.ast[Id::<PrefixedIdentifier>::from_raw(node)].identifier;
            if !is_declared_getter(c, c.element(identifier)) {
                report(c, node, out);
            }
        }
        NodeKind::PrefixExpression => {
            let operator = lexeme(c, c.ast[Id::<PrefixExpression>::from_raw(node)].operator);
            if operator != "--" && operator != "++" {
                report(c, node, out);
            }
        }
        NodeKind::PropertyAccess => {
            let name = c.ast[Id::<PropertyAccess>::from_raw(node)].property_name;
            if !is_declared_getter(c, c.element(name)) {
                report(c, node, out);
            }
        }
        NodeKind::SimpleIdentifier => {
            if !is_declared_getter(c, c.element(node)) {
                report(c, node, out);
            }
        }
        _ => report(c, node, out),
    }
}

fn statement(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.ast
        .parent(node)
        .is_some_and(|p| FunctionBody::test(kind(c, p)))
    {
        return;
    }
    let expression = c.ast[Id::<ExpressionStatement>::from_raw(node)]
        .expression
        .raw();
    report_no_clear_effect(c, expression, out);
}

fn for_statement(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let parts = c.ast[Id::<ForStatement>::from_raw(node)]
        .for_loop_parts
        .raw();
    if let Some(p) = c.ast.cast::<ForPartsWithExpression>(parts) {
        let p = &c.ast[p];
        if let Some(initialization) = p.initialization {
            report_no_clear_effect(c, initialization.raw(), out);
        }
        for &u in c.ast.list(p.updaters) {
            report_no_clear_effect(c, u.raw(), out);
        }
    }
}

fn cascade(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    for &section in c
        .ast
        .list_raw(c.ast[Id::<CascadeExpression>::from_raw(node)].cascade_sections)
    {
        if kind(c, section) == NodeKind::PropertyAccess
            && c.static_type(section)
                .is_some_and(|t| matches!(*ctx.ty(t), TypeKind::Function(_)))
        {
            report(c, section, out);
        }
    }
}
