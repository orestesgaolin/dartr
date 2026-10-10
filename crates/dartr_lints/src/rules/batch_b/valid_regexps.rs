// Dart source: pkg/linter/lib/src/rules/valid_regexps.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::InstanceCreationExpression, "valid_regexps", check);
}

/// Dart `StringLiteral.stringValue`.
pub(crate) fn string_value(c: &LinterContext<'_>, node: NodeId) -> Option<String> {
    match kind(c, node) {
        NodeKind::SimpleStringLiteral => {
            Some(c.ast[Id::<SimpleStringLiteral>::from_raw(node)].value.to_string())
        }
        NodeKind::AdjacentStrings => {
            let mut value = String::new();
            for &s in c.ast.list_raw(c.ast[Id::<AdjacentStrings>::from_raw(node)].strings) {
                value.push_str(&string_value(c, s)?);
            }
            Some(value)
        }
        _ => None,
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    let Some(element) = c
        .element(n.constructor_name)
        .and_then(|e| enclosing(c, base(c, e)))
    else {
        return;
    };
    if name(c, element) != Some("RegExp") || library_uri(c, element) != Some("dart:core") {
        return;
    }
    let args = c.ast.list_raw(c.ast[n.argument_list].arguments);
    if args.is_empty() {
        return;
    }
    let unicode = args.iter().any(|&arg| {
        c.ast.cast::<NamedArgument>(arg).is_some_and(|named| {
            lexeme(c, c.ast[named].name) == "unicode"
                && c.ast
                    .cast::<BooleanLiteral>(c.ast[named].argument_expression.raw())
                    .is_some_and(|b| c.ast[b].value)
        })
    });
    let source_expression = args[0];
    if StringLiteral::test(kind(c, source_expression))
        && let Some(source) = string_value(c, source_expression)
    {
        let flags = if unicode { "u" } else { "" };
        if regress::Regex::with_flags(&source, flags).is_err() {
            c.report_node(out, &diag::VALID_REGEXPS, source_expression, &[]);
        }
    }
}
