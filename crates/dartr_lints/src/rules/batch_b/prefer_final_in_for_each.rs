// Dart source: pkg/linter/lib/src/rules/prefer_final_in_for_each.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ForEachPartsWithDeclaration,
        "prefer_final_in_for_each",
        declaration,
    );
    r.add(
        NodeKind::ForEachPartsWithPattern,
        "prefer_final_in_for_each",
        pattern,
    );
}

fn mutated(c: &LinterContext<'_>, element: dartr_element::ElementId) -> bool {
    c.resolved
        .is_some_and(|r| r.potentially_mutated_in_scope.contains(&element))
}

fn declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let loop_variable = c.ast[Id::<ForEachPartsWithDeclaration>::from_raw(node)].loop_variable;
    let v = &c.ast[loop_variable];
    if v.keyword.is_some_and(|k| lexeme(c, k) == "final") {
        return;
    }
    let function = this_or_ancestor(c, node, |n| FunctionBody::test(kind(c, n)));
    if function.is_some()
        && let Some(element) = c.declared_element(loop_variable)
        && !mutated(c, element)
    {
        let name = lexeme(c, v.name);
        c.report_token(
            out,
            &diag::PREFER_FINAL_IN_FOR_EACH_VARIABLE,
            v.name,
            &[name],
        );
    }
}

/// Dart `FunctionBody.potentiallyMutates`.
fn potentially_mutates(c: &LinterContext<'_>, pattern: NodeId) -> bool {
    if kind(c, pattern) != NodeKind::DeclaredVariablePattern {
        return true;
    }
    match c.declared_element(pattern) {
        None => true,
        Some(element) => mutated(c, element),
    }
}

fn pattern(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<ForEachPartsWithPattern>::from_raw(node)];
    if lexeme(c, n.keyword) == "final" {
        return;
    }
    if this_or_ancestor(c, node, |n| FunctionBody::test(kind(c, n))).is_none() {
        return;
    }
    let pattern = n.pattern.raw();
    let fields_mutated = |fields: NodeList<PatternField>| {
        c.ast
            .list(fields)
            .iter()
            .any(|&f| potentially_mutates(c, c.ast[f].pattern.raw()))
    };
    let report = if let Some(record) = c.ast.cast::<RecordPattern>(pattern) {
        !fields_mutated(c.ast[record].fields)
    } else if let Some(object) = c.ast.cast::<ObjectPattern>(pattern) {
        !fields_mutated(c.ast[object].fields)
    } else if let Some(list) = c.ast.cast::<ListPattern>(pattern) {
        !c.ast
            .list_raw(c.ast[list].elements)
            .iter()
            .any(|&e| potentially_mutates(c, e))
    } else if let Some(map) = c.ast.cast::<MapPattern>(pattern) {
        !c.ast.list_raw(c.ast[map].elements).iter().any(|&e| {
            match c.ast.cast::<MapPatternEntry>(e) {
                None => true,
                Some(entry) => potentially_mutates(c, c.ast[entry].value.raw()),
            }
        })
    } else {
        false
    };
    if report {
        c.report_node(out, &diag::PREFER_FINAL_IN_FOR_EACH_PATTERN, pattern, &[]);
    }
}
