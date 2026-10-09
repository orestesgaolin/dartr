// Dart source: pkg/linter/lib/src/rules/avoid_redundant_argument_values.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ArgumentList, Id, NamedArgument, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, FormalParameterElement};
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ArgumentList,
        "avoid_redundant_argument_values",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(r), Some(ts)) = (c.resolved, c.constant_type_system()) else {
        return;
    };
    let arguments = c
        .ast
        .list_raw(c.ast[Id::<ArgumentList>::from_raw(node)].arguments);
    for argument in arguments.iter().rev() {
        let expression = c
            .ast
            .cast::<NamedArgument>(*argument)
            .map_or(*argument, |named| c.ast[named].argument_expression.raw());
        let Some(parameter) = r.tables.param_element.get(*argument).copied() else {
            continue;
        };
        let base = member::base_element(&r.ctx, parameter);
        let Some(formal) = base.cast::<FormalParameterElement>() else {
            continue;
        };
        let kind = r
            .ctx
            .get(EId::<FormalParameterElement>::from_raw(formal.raw()))
            .kind;
        if kind == dartr_ast::ParameterKind::NamedRequired
            || kind == dartr_ast::ParameterKind::Required
        {
            continue;
        }
        if let (Some(default), Some(value)) =
            (c.default_value(parameter), c.constant_value(expression))
            && default.has_known_value()
            && value.has_known_value()
            && value.dart_eq(&default, &ts)
        {
            c.report_node(out, &diag::AVOID_REDUNDANT_ARGUMENT_VALUES, expression, &[]);
        }
        if kind == dartr_ast::ParameterKind::Positional {
            break;
        }
    }
}
