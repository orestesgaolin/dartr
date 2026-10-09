// Dart source: pkg/linter/lib/src/rules/annotate_redeclares.dart

use super::helpers::{element_name, has_resolved_annotation};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodDeclaration, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, InterfaceElement};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::MethodDeclaration, "annotate_redeclares", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.modifier_keyword
        .is_some_and(|k| c.ast.tokens.lexeme(k) == "static")
        || has_resolved_annotation(c, node, "redeclare")
    {
        return;
    }
    if !super::helpers::ancestor(c.ast, node, NodeKind::ExtensionTypeDeclaration).is_some() {
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
    if InheritanceManager3::new(r.ctx)
        .get_inherited(enclosing, Name::for_library(&r.ctx, data.library, name))
        .is_some()
    {
        c.report_token(out, &diag::ANNOTATE_REDECLARES, n.name, &[name]);
    }
}
