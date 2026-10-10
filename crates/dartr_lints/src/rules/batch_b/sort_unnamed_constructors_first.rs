// Dart source: pkg/linter/lib/src/rules/sort_unnamed_constructors_first.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ClassDeclaration,
        "sort_unnamed_constructors_first",
        check,
    );
    r.add(
        NodeKind::EnumDeclaration,
        "sort_unnamed_constructors_first",
        check,
    );
    r.add(
        NodeKind::ExtensionTypeDeclaration,
        "sort_unnamed_constructors_first",
        check,
    );
}

/// The members of the body of a class, enum or extension type.
pub(crate) fn body_members(c: &LinterContext<'_>, node: NodeId) -> Vec<NodeId> {
    let body = match kind(c, node) {
        NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(node)].body.raw(),
        NodeKind::EnumDeclaration => c.ast[Id::<EnumDeclaration>::from_raw(node)].body.raw(),
        NodeKind::ExtensionTypeDeclaration => c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)]
            .body
            .raw(),
        NodeKind::MixinDeclaration => c.ast[Id::<MixinDeclaration>::from_raw(node)].body.raw(),
        NodeKind::ExtensionDeclaration => {
            c.ast[Id::<ExtensionDeclaration>::from_raw(node)].body.raw()
        }
        _ => return Vec::new(),
    };
    if let Some(b) = c.ast.cast::<BlockClassBody>(body) {
        c.ast.list_raw(c.ast[b].members).to_vec()
    } else if let Some(b) = c.ast.cast::<BlockEnumBody>(body) {
        c.ast.list_raw(c.ast[b].members).to_vec()
    } else {
        Vec::new()
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut seen_named_constructor = false;
    for member in body_members(c, node) {
        let Some(constructor) = c.ast.cast::<ConstructorDeclaration>(member) else {
            continue;
        };
        let Some(element) = c.declared_element(member) else {
            continue;
        };
        if name(c, element) == Some("new") {
            if seen_named_constructor {
                // Dart `ConstructorDeclaration.errorRange`.
                let n = &c.ast[constructor];
                let start = n
                    .type_name
                    .map(|t| c.ast.offset(t) as usize)
                    .or_else(|| {
                        n.new_keyword
                            .or(n.factory_keyword)
                            .map(|k| c.ast.tokens.get(k).offset as usize)
                    })
                    .unwrap_or(c.ast.offset(member) as usize);
                let end = match n.name {
                    Some(name) => {
                        let t = c.ast.tokens.get(name);
                        t.offset as usize + t.length as usize
                    }
                    None => match n.type_name {
                        Some(t) => c.ast.end(t) as usize,
                        None => {
                            let k = c
                                .ast
                                .tokens
                                .get(n.new_keyword.or(n.factory_keyword).unwrap());
                            k.offset as usize + k.length as usize
                        }
                    },
                };
                c.report_offset(
                    out,
                    &diag::SORT_UNNAMED_CONSTRUCTORS_FIRST,
                    start,
                    end - start,
                    &[],
                );
            }
        } else {
            seen_named_constructor = true;
        }
    }
}
