// Dart source: pkg/linter/lib/src/rules/prefer_final_parameters.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::Tag;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if c.is_feature_enabled(ExperimentalFlag::PrimaryConstructors) {
        return;
    }
    r.add(NodeKind::ConstructorDeclaration, "prefer_final_parameters", check);
    r.add(NodeKind::FunctionExpression, "prefer_final_parameters", check);
    r.add(NodeKind::MethodDeclaration, "prefer_final_parameters", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else { return };
    let (list, _body) = match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            (Some(n.parameters), n.body)
        }
        NodeKind::FunctionExpression => {
            let n = &c.ast[Id::<FunctionExpression>::from_raw(node)];
            (n.parameters, n.body)
        }
        _ => {
            let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            (n.parameters, n.body)
        }
    };
    for parameter in parameters(c, list) {
        let keyword = parameter_keyword(c, parameter);
        if matches!(keyword, Some("final") | Some("const"))
            || matches!(
                kind(c, parameter),
                NodeKind::FieldFormalParameter | NodeKind::SuperFormalParameter
            )
        {
            continue;
        }
        let Some(element) = c.declared_element(parameter) else {
            continue;
        };
        if element.tag() != Tag::FieldFormalParameter
            && !is_wildcard_variable(c, element)
            && !resolved.potentially_mutated_in_scope.contains(&element)
        {
            let name = super::prefer_iterable_wheretype::parameter_name(c, parameter).unwrap_or("");
            c.report_node(out, &diag::PREFER_FINAL_PARAMETERS, parameter, &[name]);
        }
    }
}
