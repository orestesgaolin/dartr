// Dart source: pkg/linter/lib/src/rules/prefer_const_declarations.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::FieldDeclaration,
        "prefer_const_declarations",
        check,
    );
    r.add(
        NodeKind::TopLevelVariableDeclaration,
        "prefer_const_declarations",
        check,
    );
    r.add(
        NodeKind::VariableDeclarationStatement,
        "prefer_const_declarations",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = match kind(c, node) {
        NodeKind::FieldDeclaration => {
            let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
            if n.static_keyword.is_none() {
                return;
            }
            n.fields
        }
        NodeKind::TopLevelVariableDeclaration => {
            c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables
        }
        _ => c.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables,
    };
    let keyword = c.ast[list].keyword.map(|k| lexeme(c, k));
    if keyword == Some("const") || keyword != Some("final") {
        return;
    }
    let all = c.ast.list(c.ast[list].variables).iter().all(|&v| {
        let Some(initializer) = c.ast[v].initializer else {
            return false;
        };
        let typed_literal = matches!(
            kind(c, initializer),
            NodeKind::ListLiteral | NodeKind::SetOrMapLiteral
        );
        (!typed_literal || lexeme(c, c.ast.begin_token(initializer)) == "const")
            && !c.has_constant_error(initializer)
    });
    if all {
        c.report_node(out, &diag::PREFER_CONST_DECLARATIONS, list, &[]);
    }
}
