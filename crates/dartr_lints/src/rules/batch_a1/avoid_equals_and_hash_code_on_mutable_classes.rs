// Dart source: pkg/linter/lib/src/rules/avoid_equals_and_hash_code_on_mutable_classes.dart

use super::helpers::{ancestor, lexeme};
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
    // Dart `InterfaceElementExtension.hasImmutableAnnotation`.
    let immutable = r
        .ctx
        .element_all_supertypes(class_element.upcast())
        .iter()
        .filter_map(|&t| r.ctx.interface_element(t))
        .map(|e| e.raw())
        .chain(std::iter::once(class_element.raw()))
        .any(|e| c.has_immutable(e));
    if immutable {
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
