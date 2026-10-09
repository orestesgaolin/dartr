// Dart source: pkg/linter/lib/src/rules/hash_and_equals.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::ClassDeclaration, "hash_and_equals", check);
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let class = &context.ast[context.ast.cast::<ClassDeclaration>(node).unwrap()];
    let Some(body) = context.ast.cast::<BlockClassBody>(class.body) else {
        return;
    };
    let mut equals = None;
    let mut hash = None;
    for &member in context.ast.list(context.ast[body].members) {
        if let Some(method) = context.ast.cast::<MethodDeclaration>(member) {
            let method = &context.ast[method];
            let name = context.ast.tokens.lexeme(method.name);
            if method.operator_keyword.is_some() && name == "==" {
                equals = Some(method.name);
            }
            if name == "hashCode" {
                hash = Some(method.name);
            }
        } else if let Some(field) = context.ast.cast::<FieldDeclaration>(member) {
            let fields = &context.ast[context.ast[field].fields];
            for &variable in context.ast.list(fields.variables) {
                let token = context.ast[variable].name;
                if context.ast.tokens.lexeme(token) == "hashCode" {
                    hash = Some(token);
                }
            }
        }
    }
    match (equals, hash) {
        (Some(token), None) => {
            context.report_token(out, &diag::HASH_AND_EQUALS, token, &["hashCode", "=="])
        }
        (None, Some(token)) => {
            context.report_token(out, &diag::HASH_AND_EQUALS, token, &["==", "hashCode"])
        }
        _ => {}
    }
}
