// Dart source: pkg/linter/lib/src/rules/prefer_iterable_whereType.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::MethodInvocation, "prefer_iterable_wheretype", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    if simple_name(c, n.method_name) != "where" {
        return;
    }
    let Some(target) = real_target(c, node) else { return };
    if !implements_interface(c, c.static_type(target), "Iterable", "dart.core") {
        return;
    }
    let args = arguments(c, n.argument_list);
    if args.len() != 1 {
        return;
    }
    let Some(function) = c.ast.cast::<FunctionExpression>(args[0]) else {
        return;
    };
    let f = &c.ast[function];
    let parameters = f.parameters.map(|p| c.ast.list_raw(c.ast[p].parameters).to_vec());
    if parameters.as_ref().map(|p| p.len()) != Some(1) {
        return;
    }
    let body = f.body.raw();
    let mut expression = None;
    if let Some(block) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[block].block].statements);
        if statements.len() != 1 {
            return;
        }
        if let Some(ret) = c.ast.cast::<ReturnStatement>(statements[0]) {
            expression = c.ast[ret].expression.map(|e| e.raw());
        }
    } else if let Some(e) = c.ast.cast::<ExpressionFunctionBody>(body) {
        expression = Some(c.ast[e].expression.raw());
    }
    let Some(expression) = expression.map(|e| unparenthesized(c, e)) else {
        return;
    };
    if let Some(is) = c.ast.cast::<IsExpression>(expression)
        && c.ast[is].not_operator.is_none()
        && let Some(target) = c.ast.cast::<SimpleIdentifier>(c.ast[is].expression.raw())
    {
        let parameter_name = parameter_name(c, parameters.unwrap()[0]);
        if Some(simple_name(c, target)) == parameter_name {
            c.report_node(out, &diag::PREFER_ITERABLE_WHERETYPE, n.method_name, &[]);
        }
    }
}

/// Dart `FormalParameter.name?.lexeme`.
pub(crate) fn parameter_name<'a>(c: &'a LinterContext<'_>, parameter: NodeId) -> Option<&'a str> {
    let token = match kind(c, parameter) {
        NodeKind::RegularFormalParameter => c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].name,
        NodeKind::FieldFormalParameter => Some(c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].name),
        NodeKind::SuperFormalParameter => Some(c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].name),
        _ => None,
    };
    token.map(|t| lexeme(c, t))
}
