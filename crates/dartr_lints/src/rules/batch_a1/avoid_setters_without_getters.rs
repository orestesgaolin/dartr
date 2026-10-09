// Dart source: pkg/linter/lib/src/rules/avoid_setters_without_getters.dart

use super::helpers::{element_name, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodDeclaration, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, InterfaceElement};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodDeclaration,
        "avoid_setters_without_getters",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.property_keyword.is_none_or(|k| lexeme(c, k) != "set") {
        return;
    }
    let Some(element) = c.declared_element(node) else {
        return;
    };
    let Some(data) = r.ctx.element_data(element) else {
        return;
    };
    let Some(enclosing) = data.enclosing.and_then(|e| e.cast::<InterfaceElement>()) else {
        return;
    };
    let Some(name) = element_name(c, ElemRef::Base(element)) else {
        return;
    };
    let manager = InheritanceManager3::new(r.ctx);
    let setter = Name::for_library(&r.ctx, data.library, &format!("{name}="));
    if manager
        .get_overridden(enclosing, setter)
        .is_some_and(|members| !members.is_empty())
    {
        return;
    }
    let getter = Name::for_library(&r.ctx, data.library, name);
    if manager.get_member(enclosing, getter).is_none() {
        c.report_token(out, &diag::AVOID_SETTERS_WITHOUT_GETTERS, n.name, &[]);
    }
}
