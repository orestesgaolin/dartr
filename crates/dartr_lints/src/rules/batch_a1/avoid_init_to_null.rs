// Dart source: pkg/linter/lib/src/rules/avoid_init_to_null.dart

use super::helpers::{declared_type, is_null_literal};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    FieldFormalParameter, Id, NodeId, NodeKind, RegularFormalParameter, SuperFormalParameter,
    VariableDeclaration,
};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::VariableDeclaration,
        NodeKind::FieldFormalParameter,
        NodeKind::RegularFormalParameter,
        NodeKind::SuperFormalParameter,
    ] {
        registry.add(kind, "avoid_init_to_null", check);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ts) = c.type_system() else { return };
    let default = match c.ast.kind(node) {
        NodeKind::VariableDeclaration => {
            let n = &c.ast[Id::<VariableDeclaration>::from_raw(node)];
            let list = c
                .ast
                .parent(node)
                .and_then(|p| c.ast.cast::<dartr_ast::VariableDeclarationList>(p));
            if list.is_some_and(|l| {
                c.ast[l]
                    .keyword
                    .is_some_and(|k| matches!(c.ast.tokens.lexeme(k), "const" | "final"))
            }) {
                return;
            }
            n.initializer
        }
        NodeKind::RegularFormalParameter => c.ast[Id::<RegularFormalParameter>::from_raw(node)]
            .default_clause
            .map(|d| c.ast[d].value),
        NodeKind::FieldFormalParameter => c.ast[Id::<FieldFormalParameter>::from_raw(node)]
            .default_clause
            .map(|d| c.ast[d].value),
        NodeKind::SuperFormalParameter => c.ast[Id::<SuperFormalParameter>::from_raw(node)]
            .default_clause
            .map(|d| c.ast[d].value),
        _ => None,
    };
    if default.is_some_and(|value| is_null_literal(c.ast, value))
        && declared_type(c, node).is_some_and(|ty| ts.is_nullable(ty))
    {
        c.report_node(out, &diag::AVOID_INIT_TO_NULL, node, &[]);
    }
}
