// Dart source: pkg/linter/lib/src/rules/avoid_redundant_argument_values.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    ArgumentList, Id, InstanceCreationExpression, MethodInvocation, NamedArgument, NodeId, NodeKind,
};
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
    if arguments.is_empty() {
        return;
    }
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
        let kind = r
            .ctx
            .get(EId::<FormalParameterElement>::from_raw(formal.raw()))
            .kind;
        if kind == dartr_ast::ParameterKind::NamedRequired
            || kind == dartr_ast::ParameterKind::Required
        {
            continue;
        }
        // Dart `param.metadata.hasRequired`.
        if c.has_package_meta_getter(formal.raw(), "required")
            || c.has_constructor_annotation(formal.raw(), "meta", "Required")
        {
            continue;
        }
        // The parameters of a function type that substitution or LUB/GLB
        // changed are synthetic copies
        // (`FormalParameterElementMixinExtension.copyWith`) whose
        // `computeConstantValue()` is `null`.
        let is_substituted_generic_member = |executable: dartr_element::ElementId| {
            r.ctx
                .element_data(executable)
                .and_then(|d| d.enclosing)
                .is_some_and(|owner| {
                    owner
                        .cast::<dartr_element::InterfaceElement>()
                        .is_some_and(|i| !r.ctx.interface_type_parameters(i).is_empty())
                        || owner
                            .cast::<dartr_element::ExtensionElement>()
                            .is_some_and(|e| !r.ctx.get(e).type_params.is_empty())
                })
                && dartr_constant::has_type_parameter_reference(
                    &r.ctx,
                    member::type_(&r.ctx, ElemRef::Base(executable)),
                )
        };
        let synthetic = redirected.is_none()
            && r.ctx
                .element_data(base)
                .and_then(|d| d.enclosing)
                .is_some_and(|executable| {
                    c.ast.parent(node).is_some_and(|parent| {
                        if let Some(invocation) = c.ast.cast::<MethodInvocation>(parent) {
                            (c.ast[invocation].target.is_some()
                                || c.ast[invocation].operator.is_some_and(|op| {
                                    matches!(c.ast.tokens.lexeme(op), ".." | "?..")
                                }))
                                && is_substituted_generic_member(executable)
                        } else if let Some(invocation) =
                            c.ast
                                .cast::<dartr_ast::FunctionExpressionInvocation>(parent)
                        {
                            if is_substituted_generic_member(executable) {
                                true
                            } else if c.element(invocation).is_none() {
                                let fn_ty = c.static_type(c.ast[invocation].function).map(|t| {
                                    dartr_typesystem::TypeSystem::new(r.ctx).resolve_to_bound(t)
                                });
                                let exec_ty = member::type_(&r.ctx, ElemRef::Base(executable));
                                if let Some(fn_ty) = fn_ty
                                    && let (
                                        dartr_element::TypeKind::Function(fn_f),
                                        dartr_element::TypeKind::Function(exec_f),
                                    ) = (r.ctx.ty(fn_ty), r.ctx.ty(exec_ty))
                                {
                                    r.ctx.list(fn_f.type_params).len()
                                        == r.ctx.list(exec_f.type_params).len()
                                        && r.ctx.with_nullability(
                                            fn_ty,
                                            dartr_element::Nullability::None,
                                        ) != r.ctx.with_nullability(
                                            exec_ty,
                                            dartr_element::Nullability::None,
                                        )
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                });
        if synthetic {
            if kind == dartr_ast::ParameterKind::Positional {
                break;
            }
            continue;
        }
        if let Some(value) = c.constant_value(expression)
            && value.has_known_value()
            && let Some(default) = c.default_value(parameter)
            && default.has_known_value()
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
