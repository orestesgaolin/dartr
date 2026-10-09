// Dart source: pkg/linter/lib/src/rules/avoid_renaming_method_parameters.dart

use super::helpers::{element_name, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodDeclaration, NodeId, NodeKind, RegularFormalParameter};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, FormalParameterElement, InterfaceElement};
use dartr_typesystem::{
    inheritance_manager3::{InheritanceManager3, Name},
    member,
};

pub fn register(registry: &mut RuleVisitorRegistry, context: &LinterContext<'_>) {
    if context.is_in_lib_dir() {
        registry.add(
            NodeKind::MethodDeclaration,
            "avoid_renaming_method_parameters",
            check,
        );
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.modifier_keyword.is_some_and(|k| lexeme(c, k) == "static")
        || n.documentation_comment.is_some()
    {
        return;
    }
    let (Some(parameters), Some(element)) = (n.parameters, c.declared_element(node)) else {
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
    let Some(parent) = InheritanceManager3::new(r.ctx)
        .get_inherited(enclosing, Name::for_library(&r.ctx, data.library, name))
    else {
        return;
    };
    let parent_params = member::formal_parameters(&r.ctx, parent);
    let local_params = c
        .ast
        .list_raw(c.ast[parameters].parameters)
        .iter()
        .filter_map(|p| c.ast.cast::<RegularFormalParameter>(*p))
        .filter(|p| c.ast[*p].kind.is_positional());
    for (local, parent) in local_params.zip(parent_params.into_iter().filter(|p| {
        let base = member::base_element(&r.ctx, *p);
        r.ctx
            .get(EId::<FormalParameterElement>::from_raw(base))
            .kind
            .is_positional()
    })) {
        let Some(local_name) = c.ast[local].name.map(|t| lexeme(c, t)) else {
            continue;
        };
        let Some(parent_name) = element_name(c, parent) else {
            continue;
        };
        if local_name != parent_name && local_name != "_" && parent_name != "_" {
            c.report_token(
                out,
                &diag::AVOID_RENAMING_METHOD_PARAMETERS,
                c.ast[local].name.unwrap(),
                &[local_name, parent_name],
            );
        }
    }
}
