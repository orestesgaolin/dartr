// Dart source: pkg/linter/lib/src/rules/prefer_for_elements_to_map_fromIterable.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::InstanceCreationExpression,
        "prefer_for_elements_to_map_fromIterable",
        check,
    );
}

/// Dart `_extractClosure`.
fn extract_closure(c: &LinterContext<'_>, name: &str, argument: NodeId) -> bool {
    if named_argument_name(c, argument) != Some(name) {
        return false;
    }
    let expression = unparenthesized(c, argument_expression(c, argument));
    let Some(function) = c.ast.cast::<FunctionExpression>(expression) else {
        return false;
    };
    let f = &c.ast[function];
    let parameters = parameters(c, f.parameters);
    if f.parameters.is_none() || parameters.len() != 1 || !parameter_is_required(c, parameters[0]) {
        return false;
    }
    // Dart `hasSingleExpressionBody`.
    let body = f.body.raw();
    if kind(c, body) == NodeKind::ExpressionFunctionBody {
        true
    } else if let Some(b) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[b].block].statements);
        statements.len() == 1 && kind(c, statements[0]) == NodeKind::ReturnStatement
    } else {
        false
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    let Some(element) = c.element(n.constructor_name).map(|e| base(c, e)) else {
        return;
    };
    if name(c, element) != Some("fromIterable")
        || enclosing(c, element) != Some(ctx.tp.map_element().raw())
    {
        return;
    }
    let args = arguments(c, n.argument_list);
    if args.len() != 3 {
        return;
    }
    let key = extract_closure(c, "key", args[1]) || extract_closure(c, "key", args[2]);
    let value = extract_closure(c, "value", args[2]) || extract_closure(c, "value", args[1]);
    if key && value {
        c.report_node(out, &diag::PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE, node, &[]);
    }
}
