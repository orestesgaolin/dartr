// Dart source: pkg/linter/lib/src/rules/use_to_and_as_if_applicable.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::MethodDeclaration, "use_to_and_as_if_applicable", check);
}

/// Dart `_beginsWithAsOrTo`: `RegExp(r'(to|as|_to|_as)[A-Z]').matchAsPrefix(name)`.
fn begins_with_as_or_to(name: &str) -> bool {
    ["to", "as", "_to", "_as"].iter().any(|p| {
        name.strip_prefix(p)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| ch.is_ascii_uppercase())
    })
}

/// Dart `_checkExpression`.
fn check_expression(c: &LinterContext<'_>, expression: Option<NodeId>) -> bool {
    let Some(expression) = expression.map(|e| unparenthesized(c, e)) else {
        return false;
    };
    let Some(creation) = c.ast.cast::<InstanceCreationExpression>(expression) else {
        return false;
    };
    let args = arguments(c, c.ast[creation].argument_list);
    args.len() == 1 && kind(c, args[0]) == NodeKind::ThisExpression
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    // Dart `_isVoid(node.returnType)`.
    let is_void = n.return_type.is_some_and(|t| {
        kind(c, t) == NodeKind::NamedType
            && annotation_type(c, t).is_some_and(|t| matches!(*ctx.ty(t), TypeKind::Void))
    });
    let no_parameters = n
        .parameters
        .is_some_and(|p| c.ast.list_raw(c.ast[p].parameters).is_empty());
    let body = n.body.raw();
    let body_ok = if let Some(e) = c.ast.cast::<ExpressionFunctionBody>(body) {
        check_expression(c, Some(c.ast[e].expression.raw()))
    } else if let Some(b) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[b].block].statements);
        statements.len() == 1
            && c.ast.cast::<ReturnStatement>(statements[0]).is_some_and(|r| {
                check_expression(c, c.ast[r].expression.map(|e| e.raw()))
            })
    } else {
        false
    };
    if method_property(c, node) != Some("get")
        && no_parameters
        && !is_void
        && !begins_with_as_or_to(lexeme(c, n.name))
        && look_up_inherited_method(c, node).is_none()
        && body_ok
    {
        c.report_token(out, &diag::USE_TO_AND_AS_IF_APPLICABLE, n.name, &[]);
    }
}
