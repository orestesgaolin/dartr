// Dart source: pkg/linter/lib/src/rules/prefer_constructors_over_static_methods.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeId;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::MethodDeclaration,
        "prefer_constructors_over_static_methods",
        check,
    );
}

/// Dart `_BodyVisitor.containsInstanceCreation`: `found` is the result of
/// the last visited instance creation (the children of a matching one are
/// not visited).
fn has_new_invocation(c: &LinterContext<'_>, return_type: TypeId, body: NodeId) -> bool {
    let mut found = false;
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::InstanceCreationExpression {
            found = c.static_type(n).is_some_and(|t| types_equal(c, t, return_type));
            if found {
                continue;
            }
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
    found
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.modifier_keyword.is_none_or(|k| lexeme(c, k) != "static") || n.type_parameters.is_some() {
        return;
    }
    let Some(return_type) = n.return_type.and_then(|t| annotation_type(c, t)) else {
        return;
    };
    if ctx.interface_element(return_type).is_none() {
        return;
    }
    // Dart `typeToCheckOrNull`.
    let Some(declaration) = c.ast.parent(node).and_then(|p| c.ast.parent(p)) else {
        return;
    };
    let name_part = match kind(c, declaration) {
        NodeKind::ExtensionTypeDeclaration => c.ast[Id::<ExtensionTypeDeclaration>::from_raw(declaration)].name_part,
        NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(declaration)].name_part,
        _ => return,
    };
    if class_name_part_type_parameters(c, name_part.raw()) {
        return;
    }
    let Some(element) = c.declared_element(declaration) else { return };
    let this_type = ctx.interface_this_type(dartr_element::EId::from_raw(element));
    if !types_equal(c, this_type, return_type) {
        return;
    }
    if has_new_invocation(c, return_type, n.body.raw()) {
        c.report_token(out, &diag::PREFER_CONSTRUCTORS_OVER_STATIC_METHODS, n.name, &[]);
    }
}

/// Whether the name part of a class / extension type declaration has type
/// parameters.
fn class_name_part_type_parameters(c: &LinterContext<'_>, name_part: NodeId) -> bool {
    if let Some(n) = c.ast.cast::<NameWithTypeParameters>(name_part) {
        c.ast[n].type_parameters.is_some()
    } else if let Some(p) = c.ast.cast::<PrimaryConstructorDeclaration>(name_part) {
        c.ast[p].type_parameters.is_some()
    } else {
        false
    }
}
