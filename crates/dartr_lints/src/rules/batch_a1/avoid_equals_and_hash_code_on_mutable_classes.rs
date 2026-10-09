// Dart source: pkg/linter/lib/src/rules/avoid_equals_and_hash_code_on_mutable_classes.dart

use super::helpers::{KnownAnnotation, ancestor, annotation_status, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodDeclaration, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ClassElement;
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodDeclaration,
        "avoid_equals_and_hash_code_on_mutable_classes",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.augment_keyword.is_some() || !matches!(lexeme(c, n.name), "==" | "hashCode") {
        return;
    }
    let Some(class) = ancestor(c.ast, node, NodeKind::ClassDeclaration) else {
        return;
    };
    let Some(r) = c.resolved else { return };
    let Some(class_element) = c
        .declared_element(class)
        .and_then(|element| element.cast::<ClassElement>())
    else {
        return;
    };
    let this_type = r.ctx.interface_this_type(class_element.upcast());
    let mut immutable = annotation_status(c, class, KnownAnnotation::Immutable);
    for supertype in r.ctx.all_supertypes(this_type) {
        let Some(element) = r.ctx.interface_element(supertype) else {
            continue;
        };
        match super::helpers::element_annotation_status(
            c,
            element.raw(),
            KnownAnnotation::Immutable,
        ) {
            Some(true) => {
                immutable = Some(true);
                break;
            }
            None => {
                // A `dart:` library cannot use the package:meta `immutable`
                // annotation. Its unrelated VM metadata does not make this
                // predicate unknown.
                if !r
                    .ctx
                    .element_library_uri(element.raw())
                    .is_some_and(|uri| uri.starts_with("dart:"))
                {
                    immutable = None;
                }
            }
            Some(false) => {}
        }
    }
    if immutable != Some(false) {
        return;
    }
    let name = lexeme(c, n.name);
    let token = n
        .augment_keyword
        .or(n.external_keyword)
        .or(n.modifier_keyword)
        .or_else(|| n.return_type.map(|type_| c.ast.begin_token(type_)))
        .or(n.property_keyword)
        .or(n.operator_keyword)
        .unwrap_or(n.name);
    c.report_token(
        out,
        &diag::AVOID_EQUALS_AND_HASH_CODE_ON_MUTABLE_CLASSES,
        token,
        &[name],
    );
}
