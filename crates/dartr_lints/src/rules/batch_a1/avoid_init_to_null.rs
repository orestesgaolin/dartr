// Dart source: pkg/linter/lib/src/rules/avoid_init_to_null.dart

use super::helpers::{declared_type, is_null_literal};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    FieldFormalParameter, Id, NodeId, NodeKind, RegularFormalParameter, SuperFormalParameter,
    VariableDeclaration,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElementId, FormalParameterElement, Tag};

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
        if c.ast.kind(node) == NodeKind::SuperFormalParameter {
            let Some(r) = c.resolved else { return };
            let Some(parameter) = c
                .declared_element(node)
                .and_then(|element| element.cast::<dartr_element::FormalParameterElement>())
            else {
                return;
            };
            let Some(super_parameter) =
                dartr_link::outline::super_constructor_parameter(&r.ctx, parameter)
            else {
                return;
            };
            if inherited_default_is_null(c, super_parameter.raw(), 0) != Some(true) {
                return;
            }
        }
        c.report_node(out, &diag::AVOID_INIT_TO_NULL, node, &[]);
    }
}

/// Implements the upstream `defaultValueCode ?? 'null'` comparison from the
/// source AST. An imported parameter without a resolved declaration remains
/// unknown; a constant with a null value is not equivalent to the text
/// `null` for this rule.
fn inherited_default_is_null(
    c: &LinterContext<'_>,
    parameter: ElementId,
    depth: usize,
) -> Option<bool> {
    if depth >= 64 {
        return None;
    }
    for unit in std::iter::once(*c).chain(
        (0..c.resolved_units.len())
            .filter(|&index| index != c.current_unit)
            .filter_map(|index| c.resolved_unit(index)),
    ) {
        let Some(node) = (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .find(|&node| unit.declared_element(node) == Some(parameter))
        else {
            continue;
        };
        let clause = match unit.ast.kind(node) {
            NodeKind::RegularFormalParameter => {
                unit.ast[Id::<RegularFormalParameter>::from_raw(node)].default_clause
            }
            NodeKind::FieldFormalParameter => {
                unit.ast[Id::<FieldFormalParameter>::from_raw(node)].default_clause
            }
            NodeKind::SuperFormalParameter => {
                unit.ast[Id::<SuperFormalParameter>::from_raw(node)].default_clause
            }
            _ => return None,
        };
        if let Some(clause) = clause {
            let value = unit.ast[clause].value;
            let offset = unit.ast.offset(value) as usize;
            let end = offset + unit.ast.length(value) as usize;
            return unit.source.get(offset..end).map(|source| source == "null");
        }
        if parameter.tag() != Tag::SuperFormalParameter {
            return Some(true);
        }
        let resolved = unit.resolved?;
        let parameter = EId::<FormalParameterElement>::from_raw(parameter);
        let inherited = dartr_link::outline::super_constructor_parameter(&resolved.ctx, parameter)?;
        return inherited_default_is_null(&unit, inherited.raw(), depth + 1);
    }
    None
}
