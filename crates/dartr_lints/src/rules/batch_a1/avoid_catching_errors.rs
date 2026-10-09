// Dart source: pkg/linter/lib/src/rules/avoid_catching_errors.dart

use super::helpers::node_type;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{CatchClause, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{DisplayOptions, Nullability};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::CatchClause, "avoid_catching_errors", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let Some(annotation) = c.ast[Id::<CatchClause>::from_raw(node)].exception_type else {
        return;
    };
    let Some(ty) = node_type(c, annotation) else {
        return;
    };
    let Some(error) = r.ctx.library_by_uri("dart:core").and_then(|library| {
        r.ctx
            .get(library)
            .classes
            .iter()
            .copied()
            .find(|e| r.ctx.element_name(e.raw()) == Some("Error"))
    }) else {
        return;
    };
    let error_type = r.ctx.interface_type(error.upcast(), &[], Nullability::None);
    if !c
        .type_system()
        .is_some_and(|ts| ts.is_subtype_of(ty, error_type))
    {
        return;
    }
    if ty == error_type {
        c.report_node(out, &diag::AVOID_CATCHING_ERRORS_CLASS, node, &[]);
    } else {
        let name = dartr_element::type_display_string_with(&r.ctx, ty, DisplayOptions::default());
        c.report_node(out, &diag::AVOID_CATCHING_ERRORS_SUBCLASS, node, &[&name]);
    }
}
