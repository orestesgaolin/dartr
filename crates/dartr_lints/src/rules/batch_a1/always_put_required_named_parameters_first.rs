// Dart source: pkg/linter/lib/src/rules/always_put_required_named_parameters_first.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{FormalParameterList, Id, NodeId, NodeKind, ParameterKind, RegularFormalParameter};
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
        let Some(parameter) = c.ast.cast::<RegularFormalParameter>(*parameter) else {
            continue;
        };
        let parameter = &c.ast[parameter];
        if !parameter.kind.is_named() {
            continue;
        }
        if parameter.kind == ParameterKind::NamedRequired {
            if optional_seen && let Some(name) = parameter.name {
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
