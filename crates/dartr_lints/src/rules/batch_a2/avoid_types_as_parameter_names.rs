// Dart source: pkg/linter/lib/src/rules/avoid_types_as_parameter_names.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ClassElement, ExtensionTypeElement, FragmentFlags, TypeAliasElement, TypeParameterElement,
};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::FormalParameterList,
        "avoid_types_as_parameter_names",
        check_parameters,
    );
    registry.add(
        NodeKind::CatchClause,
        "avoid_types_as_parameter_names",
        check_catch,
    );
    registry.add(
        NodeKind::TypeParameterList,
        "avoid_types_as_parameter_names",
        check_type_parameters,
    );
}
fn is_type_element(element: dartr_element::ElementId) -> bool {
    element.is::<ClassElement>()
        || element.is::<ExtensionTypeElement>()
        || element.is::<TypeAliasElement>()
        || element.is::<TypeParameterElement>()
}
fn visible_type(
    context: &LinterContext<'_>,
    scope: NodeId,
    text: &str,
    excluded: Option<dartr_element::ElementId>,
) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let name = resolved.ctx.name(text);
    let library = resolved.ctx.get(resolved.library);
    if library
        .public_namespace
        .try_get()
        .and_then(|ns| ns.defined_names.get(&name))
        .copied()
        .is_some_and(|e| Some(e) != excluded && is_type_element(e))
    {
        return true;
    }
    for ancestor in std::iter::successors(Some(scope), |n| context.ast.parent(*n)) {
        for child in context.ast.children(ancestor) {
            let Some(list) = context.ast.cast::<TypeParameterList>(child) else {
                continue;
            };
            for &parameter in context.ast.list(context.ast[list].type_parameters) {
                let token = context.ast[parameter].name;
                if context.ast.tokens.lexeme(token) == text
                    && context
                        .declared_element(parameter)
                        .is_some_and(|e| Some(e) != excluded)
                {
                    return true;
                }
            }
        }
    }
    false
}
fn check_parameters(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<FormalParameterList>(node).unwrap()];
    for &parameter in context.ast.list(n.parameters) {
        let Some(p) = context.ast.cast::<RegularFormalParameter>(parameter) else {
            continue;
        };
        let p = &context.ast[p];
        let Some(name) = p.name else {
            continue;
        };
        let Some(resolved) = context.resolved else {
            return;
        };
        let Some(fragment) = resolved
            .tables
            .declared_fragment
            .get(parameter.raw())
            .copied()
            .and_then(|f| resolved.ctx.fragment_data(f))
        else {
            continue;
        };
        if !fragment
            .flags
            .has(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE)
        {
            continue;
        }
        let text = context.ast.tokens.lexeme(name);
        if visible_type(context, node, text, context.declared_element(parameter)) {
            context.report_token(
                out,
                &diag::AVOID_TYPES_AS_PARAMETER_NAMES_FORMAL_PARAMETER,
                name,
                &[text],
            );
        }
    }
}
fn check_catch(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<CatchClause>(node).unwrap()];
    let Some(parameter) = n.exception_parameter else {
        return;
    };
    let token = context.ast[parameter].name;
    let text = context.ast.tokens.lexeme(token);
    if visible_type(context, node, text, context.declared_element(parameter)) {
        context.report_node(
            out,
            &diag::AVOID_TYPES_AS_PARAMETER_NAMES_FORMAL_PARAMETER,
            parameter,
            &[text],
        );
    }
}
fn check_type_parameters(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<TypeParameterList>(node).unwrap()];
    for &parameter in context.ast.list(n.type_parameters) {
        let token = context.ast[parameter].name;
        let text = context.ast.tokens.lexeme(token);
        if visible_type(
            context,
            context.ast.parent(node).unwrap_or(node),
            text,
            context.declared_element(parameter),
        ) {
            context.report_token(
                out,
                &diag::AVOID_TYPES_AS_PARAMETER_NAMES_TYPE_PARAMETER,
                token,
                &[text],
            );
        }
    }
}
