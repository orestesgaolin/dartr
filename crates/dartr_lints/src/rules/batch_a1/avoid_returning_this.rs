// Dart source: pkg/linter/lib/src/rules/avoid_returning_this.dart

use super::helpers::{ancestor, declared_type, has_resolved_annotation};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::MethodDeclaration, "avoid_returning_this", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.operator_keyword.is_some() || has_resolved_annotation(c, node, "override") {
        return;
    }
    if super::helpers::metadata(c.ast, node)
        .into_iter()
        .any(|annotation| c.element(annotation).is_none())
    {
        return;
    }
    let Some(owner) = [
        NodeKind::ClassDeclaration,
        NodeKind::EnumDeclaration,
        NodeKind::MixinDeclaration,
    ]
    .into_iter()
    .find_map(|kind| ancestor(c.ast, node, kind)) else {
        return;
    };
    let (Some(return_type), Some(owner_element)) =
        (declared_type(c, node), c.declared_element(owner))
    else {
        return;
    };
    if !matches!(*r.ctx.ty(return_type), TypeKind::Interface { element, .. } if element.raw() == owner_element)
    {
        return;
    }
    if let Some(body) = c.ast.cast::<ExpressionFunctionBody>(n.body) {
        if c.ast.kind(c.ast[body].expression) == NodeKind::ThisExpression {
            c.report_token(out, &diag::AVOID_RETURNING_THIS, n.name, &[]);
        }
        return;
    }
    let mut returns = Vec::new();
    let mut pending = c.ast.children(n.body).into_iter().rev().collect::<Vec<_>>();
    while let Some(child) = pending.pop() {
        if c.ast.kind(child) == NodeKind::FunctionExpression {
            continue;
        }
        if let Some(ret) = c.ast.cast::<ReturnStatement>(child) {
            if c.ast[ret]
                .expression
                .is_some_and(|e| c.ast.kind(e) == NodeKind::ThisExpression)
            {
                returns.push(ret);
            } else {
                return;
            }
        }
        pending.extend(c.ast.children(child).into_iter().rev());
    }
    if let Some(first) = returns.first().and_then(|r| c.ast[*r].expression) {
        c.report_node(out, &diag::AVOID_RETURNING_THIS, first, &[]);
    }
}
