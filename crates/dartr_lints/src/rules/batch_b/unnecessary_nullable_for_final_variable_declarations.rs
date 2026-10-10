// Dart source: pkg/linter/lib/src/rules/unnecessary_nullable_for_final_variable_declarations.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, TypeKind};

const RULE: &str = "unnecessary_nullable_for_final_variable_declarations";

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::FieldDeclaration, RULE, check);
    r.add(NodeKind::PatternVariableDeclaration, RULE, check);
    r.add(NodeKind::TopLevelVariableDeclaration, RULE, check);
    r.add(NodeKind::VariableDeclarationStatement, RULE, check);
}

/// Dart `_Visitor.check` of a pattern.
fn check_pattern(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(ts), Some(resolved)) = (rctx(c), c.type_system(), c.resolved) else {
        return;
    };
    let Some(pattern) = c.ast.cast::<DeclaredVariablePattern>(node) else {
        return;
    };
    let Some(ty) = c
        .declared_element(node)
        .and_then(|e| element_type(c, ElemRef::Base(e)))
    else {
        return;
    };
    if matches!(*ctx.ty(ty), TypeKind::Dynamic) {
        return;
    }
    let Some(value) = resolved
        .tables
        .pattern_info
        .get(node)
        .and_then(|i| i.matched_value_type)
    else {
        return;
    };
    if ts.is_nullable(ty) && ts.is_non_nullable(value) {
        c.report_token(
            out,
            &diag::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS,
            c.ast[pattern].name,
            &[],
        );
    }
}

/// Dart `_Visitor._visit`.
fn visit_variable(
    c: &LinterContext<'_>,
    list: Id<VariableDeclarationList>,
    variable: Id<VariableDeclaration>,
    out: &mut Vec<Diagnostic>,
) {
    let (Some(ctx), Some(ts)) = (rctx(c), c.type_system()) else {
        return;
    };
    let keyword = c.ast[list].keyword.map(|k| lexeme(c, k));
    if !matches!(keyword, Some("final" | "const")) {
        return;
    }
    let name = c.ast[variable].name;
    if c.ast.tokens.get(name).is_synthetic() {
        return;
    }
    let Some(initializer_type) = c.ast[variable].initializer.and_then(|i| c.static_type(i)) else {
        return;
    };
    let Some(ty) = c
        .declared_element(variable)
        .and_then(|e| element_type(c, ElemRef::Base(e)))
    else {
        return;
    };
    if matches!(*ctx.ty(ty), TypeKind::Dynamic) {
        return;
    }
    if ts.is_nullable(ty) && ts.is_non_nullable(initializer_type) {
        c.report_token(
            out,
            &diag::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS,
            name,
            &[],
        );
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match kind(c, node) {
        NodeKind::FieldDeclaration => {
            let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
            for &v in c.ast.list(c.ast[n.fields].variables) {
                if lexeme(c, c.ast[v].name).starts_with('_') || n.static_keyword.is_some() {
                    visit_variable(c, n.fields, v, out);
                }
            }
        }
        NodeKind::PatternVariableDeclaration => {
            let n = &c.ast[Id::<PatternVariableDeclaration>::from_raw(node)];
            if lexeme(c, n.keyword) != "final" {
                return;
            }
            let pattern = n.pattern.raw();
            if let Some(record) = c.ast.cast::<RecordPattern>(pattern) {
                for &field in c.ast.list(c.ast[record].fields) {
                    check_pattern(c, c.ast[field].pattern.raw(), out);
                }
            }
            if let Some(list) = c.ast.cast::<ListPattern>(pattern) {
                for &element in c.ast.list_raw(c.ast[list].elements) {
                    check_pattern(c, element, out);
                }
            }
        }
        NodeKind::TopLevelVariableDeclaration => {
            let list = c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables;
            for &v in c.ast.list(c.ast[list].variables) {
                visit_variable(c, list, v, out);
            }
        }
        _ => {
            let list = c.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables;
            for &v in c.ast.list(c.ast[list].variables) {
                visit_variable(c, list, v, out);
            }
        }
    }
}
