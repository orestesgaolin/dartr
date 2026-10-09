// Dart source: pkg/linter/lib/src/rules/avoid_redundant_argument_values.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ArgumentList, Id, InstanceCreationExpression, NamedArgument, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ConstructorElement, EId, ElemRef, FormalParameterElement};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

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
    if c.ast.parent(node).is_none_or(|parent| {
        !matches!(
            c.ast.kind(parent),
            NodeKind::Annotation
                | NodeKind::EnumConstantArguments
                | NodeKind::FunctionExpressionInvocation
                | NodeKind::InstanceCreationExpression
                | NodeKind::MethodInvocation
        )
    }) {
        return;
    }
    let redirected = match redirected_parameters(c, node) {
        Ok(parameters) => parameters,
        Err(()) => return,
    };
    let mut positional_indices = vec![None; arguments.len()];
    let mut positional = 0;
    for (index, argument) in arguments.iter().enumerate() {
        if c.ast.kind(*argument) != NodeKind::NamedArgument {
            positional_indices[index] = Some(positional);
            positional += 1;
        }
    }
    for (index, argument) in arguments.iter().enumerate().rev() {
        let expression = c
            .ast
            .cast::<NamedArgument>(*argument)
            .map_or(*argument, |named| c.ast[named].argument_expression.raw());
        let parameter = redirected.as_ref().and_then(|parameters| {
            if let Some(named) = c.ast.cast::<NamedArgument>(*argument) {
                let name = c.ast.tokens.lexeme(c.ast[named].name);
                parameters.iter().copied().find(|parameter| {
                    r.ctx.get(*parameter).kind.is_named()
                        && r.ctx.element_name(parameter.raw()) == Some(name)
                })
            } else {
                let index = positional_indices[index]?;
                parameters
                    .iter()
                    .copied()
                    .filter(|parameter| r.ctx.get(*parameter).kind.is_positional())
                    .nth(index)
            }
        });
        let parameter = parameter
            .map(|parameter| ElemRef::Base(parameter.raw()))
            .or_else(|| c.corresponding_parameter(*argument));
        let Some(parameter) = parameter else {
            continue;
        };
        let base = member::base_element(&r.ctx, parameter);
        let Some(formal) = base.cast::<FormalParameterElement>() else {
            continue;
        };
        if super::helpers::element_annotation_status(
            c,
            formal.raw(),
            super::helpers::KnownAnnotation::Required,
        ) != Some(false)
        {
            continue;
        }
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

fn redirected_parameters(
    c: &LinterContext<'_>,
    argument_list: NodeId,
) -> Result<Option<Vec<EId<FormalParameterElement>>>, ()> {
    let Some(r) = c.resolved else { return Ok(None) };
    let Some(creation) = c
        .ast
        .parent(argument_list)
        .and_then(|parent| c.ast.cast::<InstanceCreationExpression>(parent))
    else {
        return Ok(None);
    };
    let Some(mut constructor) = c
        .element(c.ast[creation].constructor_name)
        .and_then(|element| member::base_element(&r.ctx, element).cast::<ConstructorElement>())
    else {
        return Ok(None);
    };
    if !dartr_link::dump::is_factory(&r.ctx, constructor.raw()) {
        return Ok(None);
    }
    let Some(mut redirected) = r.ctx.get(constructor).redirected_constructor.get() else {
        return Ok(None);
    };
    let mut visited = IndexSet::new();
    visited.insert(constructor.raw());
    loop {
        let base = member::base_element(&r.ctx, redirected);
        let Some(next) = base.cast::<ConstructorElement>() else {
            return Ok(None);
        };
        if !visited.insert(next.raw()) {
            return Err(());
        }
        constructor = next;
        let Some(further) = r.ctx.get(constructor).redirected_constructor.get() else {
            break;
        };
        redirected = further;
    }
    Ok(Some(r.ctx.get(constructor).formal_params.clone()))
}
