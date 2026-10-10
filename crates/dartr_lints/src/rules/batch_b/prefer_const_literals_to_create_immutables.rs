// Dart source: pkg/linter/lib/src/rules/prefer_const_literals_to_create_immutables.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ListLiteral,
        "prefer_const_literals_to_create_immutables",
        check,
    );
    r.add(
        NodeKind::SetOrMapLiteral,
        "prefer_const_literals_to_create_immutables",
        check,
    );
}

fn check(c: &LinterContext<'_>, literal: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let keyword = match kind(c, literal) {
        NodeKind::ListLiteral => c.ast[Id::<ListLiteral>::from_raw(literal)].const_keyword,
        _ => c.ast[Id::<SetOrMapLiteral>::from_raw(literal)].const_keyword,
    };
    if keyword.is_some() || in_constant_context(c, literal) {
        return;
    }
    let mut node = Some(literal);
    while let Some(n) = node {
        if matches!(
            kind(c, n),
            NodeKind::ParenthesizedExpression
                | NodeKind::ArgumentList
                | NodeKind::ListLiteral
                | NodeKind::SetOrMapLiteral
                | NodeKind::MapLiteralEntry
                | NodeKind::NamedArgument
        ) {
            node = c.ast.parent(n);
        } else {
            break;
        }
    }
    let Some(creation) = node.filter(|&n| kind(c, n) == NodeKind::InstanceCreationExpression) else {
        return;
    };
    // Dart `_hasImmutableAnnotation`.
    let mut current = c.static_type(creation).filter(|&t| ctx.interface_element(t).is_some());
    if current.is_none() {
        return;
    }
    let mut immutable = false;
    while let Some(t) = current {
        let Some(e) = ctx.interface_element(t) else { break };
        if c.has_immutable(e.raw()) {
            immutable = true;
            break;
        }
        current = ctx.superclass(t);
    }
    if immutable && c.can_be_const(literal) {
        c.report_node(out, &diag::PREFER_CONST_LITERALS_TO_CREATE_IMMUTABLES, literal, &[]);
    }
}
