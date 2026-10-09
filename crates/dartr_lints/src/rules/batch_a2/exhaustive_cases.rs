// Dart source: pkg/linter/lib/src/rules/exhaustive_cases.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_constant::DartObjectImpl;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, ElementFlags, FragmentFlags, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::SwitchStatement, "exhaustive_cases", check);
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let switch = &context.ast[context.ast.cast::<SwitchStatement>(node).unwrap()];
    let Some(TypeKind::Interface { element, .. }) = context
        .static_type(switch.expression)
        .map(|t| *resolved.ctx.ty(t))
    else {
        return;
    };
    let Some(class) = element.raw().cast::<ClassElement>() else {
        return;
    };
    if resolved
        .ctx
        .get(class)
        .flags
        .has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
    {
        return;
    }
    let interface = resolved.ctx.interface(element);
    for &constructor in &interface.constructors {
        let data = resolved.ctx.get(constructor);
        let name = data.name.map(|n| resolved.ctx.name_str(n)).unwrap_or("");
        let factory = resolved
            .ctx
            .fragment_data(data.first_fragment)
            .is_some_and(|f| f.flags.has(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY));
        if !name.starts_with('_') || factory {
            return;
        }
    }
    let this_type = resolved.ctx.interface_this_type(element);
    if let Some(library) = resolved.ctx.get(class).library {
        for &other in &resolved.ctx.get(library).classes {
            if other == class {
                continue;
            }
            let mut supertype = resolved.ctx.get(other).supertype.get();
            while let Some(ty) = supertype {
                let Some(super_element) = resolved.ctx.interface_element(ty) else {
                    break;
                };
                if super_element.raw() == class.raw() {
                    return;
                }
                supertype = resolved.ctx.interface(super_element).supertype.get();
            }
        }
    }
    let mut constants: Vec<(DartObjectImpl, Vec<(String, bool)>)> = vec![];
    let mut enum_constant_count = 0;
    for &field in &interface.fields {
        let data = resolved.ctx.get(field);
        let Some(fragment) = resolved.ctx.fragment_data(data.first_fragment) else {
            continue;
        };
        if !fragment
            .flags
            .has(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
            || !fragment
                .flags
                .has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
            || fragment
                .flags
                .has(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            || !context.type_system().is_some_and(|ts| {
                ts.dart_eq(
                    member::type_(&resolved.ctx, dartr_element::ElemRef::Base(field.raw())),
                    this_type,
                )
            })
        {
            continue;
        }
        let Some((declaration_context, declaration)) = declaration_context(context, field.raw())
        else {
            continue;
        };
        let Some(value) = declaration_context
            .ast
            .cast::<VariableDeclaration>(declaration)
            .and_then(|variable| declaration_context.ast[variable].initializer)
            .and_then(|expression| declaration_context.constant_value(expression))
        else {
            continue;
        };
        let name = data
            .name
            .map(|n| resolved.ctx.name_str(n).to_string())
            .unwrap_or_default();
        let deprecated = variable_is_deprecated(&declaration_context, declaration);
        enum_constant_count += 1;
        if let Some((_, names)) = constants.iter_mut().find(|(candidate, _)| {
            candidate.dart_eq(&value, &crate::constants::ConstantTypeSystem(resolved.ctx))
        }) {
            names.push((name, deprecated));
        } else {
            constants.push((value, vec![(name, deprecated)]));
        }
    }
    if enum_constant_count < 2 {
        return;
    }
    for &member_node in context.ast.list(switch.members) {
        if context.ast.kind(member_node) == NodeKind::SwitchDefault {
            return;
        }
        let expression = switch_member_expression(context, member_node.raw());
        let Some(value) = expression.and_then(|expression| {
            if let Some(dot) = context.ast.cast::<DotShorthandPropertyAccess>(expression) {
                context.constant_value(context.ast[dot].property_name)
            } else {
                context.constant_value(expression)
            }
        }) else {
            continue;
        };
        constants.retain(|(candidate, _)| {
            !candidate.dart_eq(&value, &crate::constants::ConstantTypeSystem(resolved.ctx))
        });
    }
    let start = context.ast.offset(node) as usize;
    let length = (context.ast.tokens.get(switch.right_parenthesis).end() - context.ast.offset(node))
        as usize;
    for (_, names) in constants {
        let name = names
            .iter()
            .find(|(_, deprecated)| !deprecated)
            .or_else(|| names.first())
            .map(|(name, _)| name.as_str())
            .unwrap_or("");
        context.report_offset(out, &diag::EXHAUSTIVE_CASES, start, length, &[name]);
    }
}

fn declaration_context<'a>(
    context: &LinterContext<'a>,
    element: dartr_element::ElementId,
) -> Option<(LinterContext<'a>, NodeId)> {
    for unit in std::iter::once(*context).chain(
        (0..context.resolved_units.len())
            .filter(|&index| index != context.current_unit)
            .filter_map(|index| context.resolved_unit(index)),
    ) {
        if let Some(node) = (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .find(|&node| unit.declared_element(node) == Some(element))
        {
            return Some((unit, node));
        }
    }
    None
}

fn unparenthesized_expression(context: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(parenthesized) = context.ast.cast::<ParenthesizedExpression>(node) {
        node = context.ast[parenthesized].expression.raw();
    }
    node
}

fn unparenthesized_pattern(context: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(parenthesized) = context.ast.cast::<ParenthesizedPattern>(node) {
        node = context.ast[parenthesized].pattern.raw();
    }
    node
}

fn switch_member_expression(context: &LinterContext<'_>, member: NodeId) -> Option<NodeId> {
    if let Some(case) = context.ast.cast::<SwitchCase>(member) {
        return Some(unparenthesized_expression(
            context,
            context.ast[case].expression.raw(),
        ));
    }
    let pattern_case = context.ast.cast::<SwitchPatternCase>(member)?;
    let pattern = unparenthesized_pattern(
        context,
        context.ast[context.ast[pattern_case].guarded_pattern]
            .pattern
            .raw(),
    );
    let constant = context.ast.cast::<ConstantPattern>(pattern)?;
    Some(unparenthesized_expression(
        context,
        context.ast[constant].expression.raw(),
    ))
}

fn variable_is_deprecated(context: &LinterContext<'_>, variable: NodeId) -> bool {
    let declaration = &context.ast[context.ast.cast::<VariableDeclaration>(variable).unwrap()];
    if super::deprecated_consistency::has_deprecated(context, declaration.metadata) {
        return true;
    }
    context
        .ast
        .parent(variable)
        .and_then(|parent| context.ast.cast::<VariableDeclarationList>(parent))
        .and_then(|list| context.ast.parent(list.raw()))
        .and_then(|parent| context.ast.cast::<FieldDeclaration>(parent))
        .is_some_and(|field| {
            super::deprecated_consistency::has_deprecated(context, context.ast[field].metadata)
        })
}
