// Dart source: pkg/linter/lib/src/rules/unnecessary_constructor_name.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ConstructorDeclaration, "unnecessary_constructor_name", check);
    r.add(
        NodeKind::PrimaryConstructorDeclaration,
        "unnecessary_constructor_name",
        check,
    );
    r.add(
        NodeKind::InstanceCreationExpression,
        "unnecessary_constructor_name",
        check,
    );
}

/// The count of the constructors named `new` of the class of [declaration].
fn unnamed_constructors(c: &LinterContext<'_>, declaration: NodeId) -> usize {
    let Some(ctx) = rctx(c) else { return 0 };
    let Some(class) = c
        .declared_element(declaration)
        .and_then(|e| enclosing(c, e))
        .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
    else {
        return 0;
    };
    ctx.interface(class)
        .constructors
        .iter()
        .filter(|k| name(c, k.raw()) == Some("new"))
        .count()
}

fn check_token(c: &LinterContext<'_>, name: Option<dartr_syntax::TokenId>, out: &mut Vec<Diagnostic>) {
    if let Some(name) = name
        && lexeme(c, name) == "new"
    {
        c.report_token(out, &diag::UNNECESSARY_CONSTRUCTOR_NAME, name, &[]);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            if unnamed_constructors(c, node) > 1 {
                return;
            }
            if let Some(parent) = c.ast.parent(node).and_then(|p| c.ast.parent(p))
                && let Some(et) = c.ast.cast::<ExtensionTypeDeclaration>(parent)
                && let Some(primary) = c.ast.cast::<PrimaryConstructorDeclaration>(c.ast[et].name_part)
                && c.ast[primary].constructor_name.is_none()
            {
                return;
            }
            check_token(c, c.ast[Id::<ConstructorDeclaration>::from_raw(node)].name, out);
        }
        NodeKind::PrimaryConstructorDeclaration => {
            if unnamed_constructors(c, node) > 1 {
                return;
            }
            let name = c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)]
                .constructor_name
                .map(|n| c.ast[n].name);
            check_token(c, name, out);
        }
        _ => {
            let constructor_name = c.ast[Id::<InstanceCreationExpression>::from_raw(node)].constructor_name;
            let name = c.ast[constructor_name].name.map(|n| c.ast[n].token);
            check_token(c, name, out);
        }
    }
}
