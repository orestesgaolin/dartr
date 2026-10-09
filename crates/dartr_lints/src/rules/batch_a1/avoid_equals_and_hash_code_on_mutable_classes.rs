// Dart source: pkg/linter/lib/src/rules/avoid_equals_and_hash_code_on_mutable_classes.dart

use super::helpers::{ancestor, has_resolved_annotation, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodDeclaration, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};

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
    if has_resolved_annotation(c, class, "immutable")
        || super::helpers::metadata(c.ast, class)
            .into_iter()
            .any(|annotation| c.element(annotation).is_none())
    {
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
