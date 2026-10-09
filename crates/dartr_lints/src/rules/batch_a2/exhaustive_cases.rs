// Dart source: pkg/linter/lib/src/rules/exhaustive_cases.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{diag, Diagnostic};
use dartr_element::{ClassElement, ElementFlags, FragmentFlags, TypeKind};
use dartr_typesystem::{member, TypeExt};

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
    let class_data = resolved.ctx.get(class);
    if class_data
        .flags
        .has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
    {
        return;
    }
    for &constructor in &class_data.constructors {
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
    let this_type = resolved.ctx.interface_this_type(class.upcast());
    let library = class_data.library;
    if let Some(library) = library {
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
    let mut constants = vec![];
    for &field in &class_data.fields {
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
            || !context.type_system().is_some_and(|ts| {
                ts.dart_eq(
                    member::type_(&resolved.ctx, dartr_element::ElemRef::Base(field.raw())),
                    this_type,
                )
            })
        {
            continue;
        }
        let declaration = (0..context.ast.node_count())
            .map(NodeId::from_index)
            .find(|&n| context.declared_element(n) == Some(field.raw()));
        let Some(value) = declaration.and_then(|n| {
            context
                .ast
                .cast::<VariableDeclaration>(n)
                .and_then(|v| context.ast[v].initializer)
                .and_then(|e| context.constant_value(e))
        }) else {
            continue;
        };
        let name = data
            .name
            .map(|n| resolved.ctx.name_str(n).to_string())
            .unwrap_or_default();
        constants.push((name, value));
    }
    if constants.len() < 2 {
        return;
    }
    for &member_node in context.ast.list(switch.members) {
        if context.ast.kind(member_node) == NodeKind::SwitchDefault {
            return;
        }
        let expression = context
            .ast
            .cast::<SwitchCase>(member_node)
            .map(|case| context.ast[case].expression.raw());
        let Some(value) = expression.and_then(|e| context.constant_value(e)) else {
            continue;
        };
        constants.retain(|(_, candidate)| {
            !candidate.dart_eq(&value, &crate::constants::ConstantTypeSystem(resolved.ctx))
        });
    }
    let start = context.ast.offset(node) as usize;
    let length = (context.ast.tokens.get(switch.right_parenthesis).end() - context.ast.offset(node))
        as usize;
    for (name, _) in constants {
        context.report_offset(out, &diag::EXHAUSTIVE_CASES, start, length, &[&name]);
    }
}
