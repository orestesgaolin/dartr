// Dart source: pkg/linter/lib/src/rules/use_primary_constructors.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::PrimaryConstructors) {
        return;
    }
    r.add(NodeKind::ClassDeclaration, "use_primary_constructors", check);
    r.add(NodeKind::EnumDeclaration, "use_primary_constructors", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let name_part = match kind(c, node) {
        NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(node)].name_part.raw(),
        _ => c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part.raw(),
    };
    let Some(name_with_type_parameters) = c.ast.cast::<NameWithTypeParameters>(name_part) else {
        return;
    };
    let container_name = c.ast[name_with_type_parameters].type_name;
    let mut has_constructor = false;
    let mut root = None;
    for member in super::sort_unnamed_constructors_first::body_members(c, node) {
        let Some(constructor) = c.ast.cast::<ConstructorDeclaration>(member) else {
            continue;
        };
        let m = &c.ast[constructor];
        if m.external_keyword.is_some() {
            return;
        }
        has_constructor = true;
        if m.factory_keyword.is_none() {
            // Dart `redirect`: the element of a trailing redirecting invocation.
            let redirect = c
                .ast
                .list_raw(m.initializers)
                .last()
                .filter(|&&i| kind(c, i) == NodeKind::RedirectingConstructorInvocation)
                .and_then(|&i| c.element(i));
            if redirect.is_none() {
                if root.is_some() {
                    return;
                }
                root = Some(constructor);
            }
        }
    }
    if !has_constructor {
        c.report_token(out, &diag::USE_PRIMARY_CONSTRUCTORS, container_name, &[]);
        return;
    }
    let Some(root) = root else { return };
    let n = &c.ast[root];
    if let Some(name) = n.name {
        c.report_token(out, &diag::USE_PRIMARY_CONSTRUCTORS, name, &[]);
    } else if let Some(type_name) = n.type_name {
        c.report_node(out, &diag::USE_PRIMARY_CONSTRUCTORS, type_name, &[]);
    } else if let Some(keyword) = n.new_keyword {
        c.report_token(out, &diag::USE_PRIMARY_CONSTRUCTORS, keyword, &[]);
    }
}
