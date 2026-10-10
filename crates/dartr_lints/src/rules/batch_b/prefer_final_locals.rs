// Dart source: pkg/linter/lib/src/rules/prefer_final_locals.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementId;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::DeclaredVariablePattern,
        "prefer_final_locals",
        declared_pattern,
    );
    r.add(
        NodeKind::PatternVariableDeclaration,
        "prefer_final_locals",
        pattern_declaration,
    );
    r.add(
        NodeKind::VariableDeclarationList,
        "prefer_final_locals",
        list,
    );
}

fn mutated(c: &LinterContext<'_>, element: ElementId) -> bool {
    c.resolved
        .is_some_and(|r| r.potentially_mutated_in_scope.contains(&element))
}

/// Dart `isPotentiallyMutated(pattern, function)`.
fn is_potentially_mutated(c: &LinterContext<'_>, pattern: NodeId) -> bool {
    if kind(c, pattern) == NodeKind::DeclaredVariablePattern {
        let mut element = c.declared_element(pattern);
        if let Some(e) = element
            && let Some((join, variables)) = pattern_variable_join(c, e)
        {
            if variables.first() != Some(&e) {
                return true;
            }
            element = Some(join);
        }
        match element {
            None => return true,
            Some(e) if mutated(c, e) => return true,
            _ => {}
        }
    }
    false
}

/// Dart `hasPotentiallyMutatedDeclaredVariableInScope`.
fn has_potentially_mutated_declared_variable(c: &LinterContext<'_>, node: NodeId) -> bool {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::DeclaredVariablePattern
            && let Some(e) = c.declared_element(n)
            && mutated(c, e)
        {
            return true;
        }
        stack.extend(c.ast.children(n));
    }
    false
}

/// Dart `isDeclaredFinal` of a `DeclaredVariablePattern`.
fn is_declared_final(c: &LinterContext<'_>, node: NodeId) -> bool {
    if c.ast[Id::<DeclaredVariablePattern>::from_raw(node)]
        .keyword
        .is_some_and(|k| lexeme(c, k) == "final")
    {
        return true;
    }
    this_or_ancestor_kind(c, node, NodeKind::ForEachPartsWithPattern).is_some_and(|p| {
        lexeme(c, c.ast[Id::<ForEachPartsWithPattern>::from_raw(p)].keyword) == "final"
    })
}

fn function_body(c: &LinterContext<'_>, node: NodeId) -> Option<NodeId> {
    this_or_ancestor(c, node, |n| FunctionBody::test(kind(c, n)))
}

fn declared_pattern(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if this_or_ancestor_kind(c, node, NodeKind::PatternVariableDeclaration).is_some()
        || is_declared_final(c, node)
    {
        return;
    }
    if function_body(c, node).is_none() {
        return;
    }
    let in_case_clause = this_or_ancestor_kind(c, node, NodeKind::CaseClause).is_some();
    if in_case_clause {
        if !is_potentially_mutated(c, node) {
            c.report_node(out, &diag::PREFER_FINAL_LOCALS, node, &[]);
        }
    } else {
        if let Some(for_each) = this_or_ancestor_kind(c, node, NodeKind::ForEachPartsWithPattern) {
            if has_potentially_mutated_declared_variable(c, for_each) {
                return;
            }
        } else if is_potentially_mutated(c, node) {
            return;
        }
        c.report_node(out, &diag::PREFER_FINAL_LOCALS, node, &[]);
    }
}

/// Dart `DartPattern.containsJustWildcards`.
fn contains_just_wildcards(c: &LinterContext<'_>, pattern: NodeId) -> bool {
    match kind(c, pattern) {
        NodeKind::ListPattern => c
            .ast
            .list_raw(c.ast[Id::<ListPattern>::from_raw(pattern)].elements)
            .iter()
            .all(|&e| DartPattern::test(kind(c, e)) && contains_just_wildcards(c, e)),
        NodeKind::MapPattern => c
            .ast
            .list_raw(c.ast[Id::<MapPattern>::from_raw(pattern)].elements)
            .iter()
            .all(|&e| {
                c.ast
                    .cast::<MapPatternEntry>(e)
                    .is_some_and(|entry| kind(c, c.ast[entry].value) == NodeKind::WildcardPattern)
            }),
        NodeKind::ObjectPattern => c
            .ast
            .list(c.ast[Id::<ObjectPattern>::from_raw(pattern)].fields)
            .iter()
            .all(|&f| contains_just_wildcards(c, c.ast[f].pattern.raw())),
        NodeKind::ParenthesizedPattern => contains_just_wildcards(
            c,
            c.ast[Id::<ParenthesizedPattern>::from_raw(pattern)]
                .pattern
                .raw(),
        ),
        NodeKind::RecordPattern => c
            .ast
            .list(c.ast[Id::<RecordPattern>::from_raw(pattern)].fields)
            .iter()
            .all(|&f| contains_just_wildcards(c, c.ast[f].pattern.raw())),
        NodeKind::WildcardPattern => true,
        _ => false,
    }
}

fn pattern_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<PatternVariableDeclaration>::from_raw(node)];
    if lexeme(c, n.keyword) == "final" {
        return;
    }
    if function_body(c, node).is_none() {
        return;
    }
    let in_case_clause = this_or_ancestor_kind(c, node, NodeKind::CaseClause).is_some();
    if in_case_clause {
        if !is_potentially_mutated(c, node) {
            c.report_node(out, &diag::PREFER_FINAL_LOCALS, node, &[]);
        }
    } else if !has_potentially_mutated_declared_variable(c, node) {
        if contains_just_wildcards(c, n.pattern.raw()) {
            return;
        }
        c.report_token(out, &diag::PREFER_FINAL_LOCALS, n.keyword, &[]);
    }
}

fn list(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<VariableDeclarationList>::from_raw(node)];
    if n.keyword
        .is_some_and(|k| matches!(lexeme(c, k), "const" | "final"))
    {
        return;
    }
    if function_body(c, node).is_none() {
        return;
    }
    for &variable in c.ast.list(n.variables) {
        let v = &c.ast[variable];
        if v.equals.is_none() || v.initializer.is_none() {
            return;
        }
        if let Some(element) = c.declared_element(variable)
            && (is_wildcard_variable(c, element) || mutated(c, element))
        {
            return;
        }
    }
    if let Some(keyword) = n.keyword {
        c.report_token(out, &diag::PREFER_FINAL_LOCALS, keyword, &[]);
    } else if let Some(ty) = n.type_ {
        c.report_node(out, &diag::PREFER_FINAL_LOCALS, ty, &[]);
    }
}
