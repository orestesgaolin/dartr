// Dart source: pkg/linter/lib/src/rules/prefer_function_declarations_over_variables.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::VariableDeclaration,
        "prefer_function_declarations_over_variables",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    let n = &c.ast[Id::<VariableDeclaration>::from_raw(node)];
    if n.initializer
        .is_none_or(|i| kind(c, i) != NodeKind::FunctionExpression)
    {
        return;
    }
    let function = this_or_ancestor(c, node, |n| FunctionBody::test(kind(c, n)));
    match function {
        None => {
            // Dart `VariableDeclaration.isFinal`.
            let is_final = c
                .ast
                .parent(node)
                .and_then(|l| c.ast.cast::<VariableDeclarationList>(l))
                .and_then(|l| c.ast[l].keyword)
                .is_some_and(|k| lexeme(c, k) == "final");
            if is_final {
                c.report_node(
                    out,
                    &diag::PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES,
                    node,
                    &[],
                );
            }
        }
        Some(_) => {
            if let Some(element) = c.declared_element(node)
                && !resolved.potentially_mutated_in_scope.contains(&element)
            {
                c.report_node(
                    out,
                    &diag::PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES,
                    node,
                    &[],
                );
            }
        }
    }
}
