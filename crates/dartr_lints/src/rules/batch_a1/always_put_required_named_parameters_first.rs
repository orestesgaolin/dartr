// Dart source: pkg/linter/lib/src/rules/always_put_required_named_parameters_first.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    FieldFormalParameter, FormalParameterList, Id, NodeId, NodeKind, ParameterKind,
    RegularFormalParameter, SuperFormalParameter,
};
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::FormalParameterList,
        "always_put_required_named_parameters_first",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut optional_seen = false;
    for parameter in c
        .ast
        .list_raw(c.ast[Id::<FormalParameterList>::from_raw(node)].parameters)
    {
        let (kind, name) = match c.ast.kind(*parameter) {
            NodeKind::RegularFormalParameter => {
                let parameter = &c.ast[Id::<RegularFormalParameter>::from_raw(*parameter)];
                (parameter.kind, parameter.name)
            }
            NodeKind::FieldFormalParameter => {
                let parameter = &c.ast[Id::<FieldFormalParameter>::from_raw(*parameter)];
                (parameter.kind, Some(parameter.name))
            }
            NodeKind::SuperFormalParameter => {
                let parameter = &c.ast[Id::<SuperFormalParameter>::from_raw(*parameter)];
                (parameter.kind, Some(parameter.name))
            }
            _ => continue,
        };
        if !kind.is_named() {
            continue;
        }
        if kind == ParameterKind::NamedRequired {
            if optional_seen && let Some(name) = name {
                c.report_token(
                    out,
                    &diag::ALWAYS_PUT_REQUIRED_NAMED_PARAMETERS_FIRST,
                    name,
                    &[],
                );
            }
        } else {
            optional_seen = true;
        }
    }
}
