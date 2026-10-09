// Dart source: pkg/linter/lib/src/rules/avoid_implementing_value_types.dart

use super::helpers::{element_library_uri, interface_element, node_type};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ClassDeclaration, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ClassDeclaration,
        "avoid_implementing_value_types",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let Some(clause) = c.ast[Id::<ClassDeclaration>::from_raw(node)].implements_clause else {
        return;
    };
    let manager = InheritanceManager3::new(r.ctx);
    for interface in c.ast.list(c.ast[clause].interfaces) {
        let Some(element) = node_type(c, *interface).and_then(|ty| interface_element(c, ty)) else {
            continue;
        };
        let Some(member) = manager.get_member(element, Name::new(&r.ctx, None, "==")) else {
            continue;
        };
        if element_library_uri(c, member) != Some("dart:core") {
            c.report_node(out, &diag::AVOID_IMPLEMENTING_VALUE_TYPES, *interface, &[]);
        }
    }
}
