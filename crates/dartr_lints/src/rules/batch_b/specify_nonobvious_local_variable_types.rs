// Dart source: pkg/linter/lib/src/rules/specify_nonobvious_local_variable_types.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeKind;
use dartr_typesystem::TypeExt;

const RULE: &str = "specify_nonobvious_local_variable_types";

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ForStatement, RULE, check);
    r.add(NodeKind::PatternVariableDeclarationStatement, RULE, check);
    r.add(NodeKind::SwitchExpression, RULE, check);
    r.add(NodeKind::SwitchStatement, RULE, check);
    r.add(NodeKind::VariableDeclarationStatement, RULE, check);
}

/// Dart `_PatternVisitor` (a `GeneralizingAstVisitor`) over [root] and its
/// descendants.
fn pattern_visitor(c: &LinterContext<'_>, root: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if let Some(pattern) = c.ast.cast::<DeclaredVariablePattern>(n) {
            let ty = c.ast[pattern].type_.and_then(|t| annotation_type(c, t));
            let typed = ty.is_some_and(|t| {
                !matches!(*ctx.ty(t), TypeKind::Dynamic) && !ctx.is_dart_core_null(t)
            });
            if !typed {
                c.report_node(out, &diag::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES, n, &[]);
            }
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
}

fn variable_list(
    c: &LinterContext<'_>,
    list: Id<VariableDeclarationList>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = rctx(c) else { return };
    if let Some(ty) = c.ast[list].type_.and_then(|t| annotation_type(c, t))
        && !ctx.is_dart_core_null(ty)
    {
        return;
    }
    let mut needed = Vec::new();
    for &child in c.ast.list(c.ast[list].variables) {
        match c.ast[child].initializer {
            None => needed.push(child),
            Some(i) => {
                if !has_obvious_type(c, i.raw()) {
                    needed.push(child);
                }
            }
        }
    }
    if !needed.is_empty() {
        if c.ast.list(c.ast[list].variables).len() == 1 {
            c.report_node(
                out,
                &diag::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES,
                list,
                &[],
            );
        } else {
            for v in needed {
                c.report_node(out, &diag::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES, v, &[]);
            }
        }
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    match kind(c, node) {
        NodeKind::ForStatement => {
            let parts = c.ast[Id::<ForStatement>::from_raw(node)]
                .for_loop_parts
                .raw();
            if let Some(p) = c.ast.cast::<ForPartsWithDeclarations>(parts) {
                variable_list(c, c.ast[p].variables, out);
            } else if let Some(p) = c.ast.cast::<ForEachPartsWithDeclaration>(parts) {
                let loop_variable = c.ast[p].loop_variable;
                let ty = c.ast[loop_variable]
                    .type_
                    .and_then(|t| annotation_type(c, t));
                if ty.is_some_and(|t| !matches!(*ctx.ty(t), TypeKind::Dynamic)) {
                    return;
                }
                if has_obvious_type(c, c.ast[p].iterable.raw()) {
                    return;
                }
                c.report_node(
                    out,
                    &diag::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES,
                    loop_variable,
                    &[],
                );
            }
        }
        NodeKind::PatternVariableDeclarationStatement => {
            let declaration =
                c.ast[Id::<PatternVariableDeclarationStatement>::from_raw(node)].declaration;
            let d = &c.ast[declaration];
            if has_obvious_type(c, d.expression.raw()) {
                return;
            }
            pattern_visitor(c, d.pattern.raw(), out);
        }
        NodeKind::SwitchExpression => {
            let n = &c.ast[Id::<SwitchExpression>::from_raw(node)];
            if has_obvious_type(c, n.expression.raw()) {
                return;
            }
            for &case in c.ast.list(n.cases) {
                pattern_visitor(c, case.raw(), out);
            }
        }
        NodeKind::SwitchStatement => {
            let n = &c.ast[Id::<SwitchStatement>::from_raw(node)];
            if has_obvious_type(c, n.expression.raw()) {
                return;
            }
            for &member in c.ast.list_raw(n.members) {
                if kind(c, member) == NodeKind::SwitchPatternCase {
                    pattern_visitor(c, member, out);
                }
            }
        }
        _ => {
            let list = c.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables;
            variable_list(c, list, out);
        }
    }
}
