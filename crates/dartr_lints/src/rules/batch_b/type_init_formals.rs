// Dart source: pkg/linter/lib/src/rules/type_init_formals.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, FormalParameterElement, Tag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::FieldFormalParameter, "type_init_formals", check);
    r.add(NodeKind::SuperFormalParameter, "type_init_formals", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let node_type = match kind(c, node) {
        NodeKind::FieldFormalParameter => c.ast[Id::<FieldFormalParameter>::from_raw(node)].type_,
        _ => c.ast[Id::<SuperFormalParameter>::from_raw(node)].type_,
    };
    let Some(node_type) = node_type else { return };
    let Some(parameter) = c.declared_element(node) else {
        return;
    };
    let Some(ty) = annotation_type(c, node_type) else {
        return;
    };
    let other = match (kind(c, node), parameter.tag()) {
        (NodeKind::FieldFormalParameter, Tag::FieldFormalParameter) => ctx
            .get(EId::<FormalParameterElement>::from_raw(parameter))
            .field
            .get()
            .and_then(|f| element_type(c, ElemRef::Base(f.raw()))),
        (NodeKind::SuperFormalParameter, Tag::SuperFormalParameter) => {
            super_constructor_parameter(c, parameter).and_then(|p| element_type(c, p))
        }
        _ => return,
    };
    if other.is_some_and(|other| types_equal(c, ty, other)) {
        c.report_node(out, &diag::TYPE_INIT_FORMALS, node_type, &[]);
    }
}
