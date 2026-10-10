// Dart source: pkg/linter/lib/src/rules/avoid_types_as_parameter_names.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ClassElement, DirectiveUri, ExtensionTypeElement, FragmentFlags, LibraryFragment,
    MethodElement, NamespaceCombinator, TypeAliasElement, TypeParameterElement,
};
use dartr_typesystem::TypeExt;

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
    excluded_enclosing: Option<dartr_element::ElementId>,
) -> Option<dartr_element::ElementId> {
    let resolved = context.resolved?;
    let name = resolved.ctx.name(text);
    let ancestor_elements: Vec<_> =
        std::iter::successors(Some(scope), |node| context.ast.parent(*node))
            .filter_map(|node| context.declared_element(node))
            .collect();
    for &ancestor in &ancestor_elements {
        for candidate in (0..context.ast.node_count()).map(NodeId::from_index) {
            let Some(element) = context
                .declared_element(candidate)
                .filter(|element| element.is::<TypeParameterElement>())
            else {
                continue;
            };
            let Some(data) = resolved.ctx.element_data(element) else {
                continue;
            };
            if data.name == Some(name)
                && data.enclosing == Some(ancestor)
                && Some(element) != excluded
                && data.enclosing != excluded_enclosing
            {
                return Some(element);
            }
        }
    }
    let library = resolved.ctx.get(resolved.library);
    let own = library
        .classes
        .iter()
        .map(|element| element.raw())
        .chain(library.extension_types.iter().map(|element| element.raw()))
        .chain(library.type_aliases.iter().map(|element| element.raw()))
        .find(|&element| {
            resolved.ctx.element_name(element) == Some(text) && Some(element) != excluded
        });
    if own.is_some() {
        return own;
    }
    let unit = std::iter::successors(Some(scope), |node| context.ast.parent(*node))
        .find(|node| context.ast.kind(*node) == NodeKind::CompilationUnit)?;
    let fragment = resolved
        .tables
        .declared_fragment
        .get(unit)
        .copied()?
        .cast::<LibraryFragment>()?;
    for import in &resolved.ctx.fragment(fragment).library_imports {
        if import.prefix.is_some() {
            continue;
        }
        let allowed = import
            .combinators
            .iter()
            .all(|combinator| match combinator {
                NamespaceCombinator::Hide { hidden_names, .. } => !hidden_names.contains(&name),
                NamespaceCombinator::Show { shown_names, .. } => shown_names.contains(&name),
            });
        if !allowed {
            continue;
        }
        let Some(imported_library) = (match &import.directive.uri {
            DirectiveUri::Library { library, .. } => Some(*library),
            _ => None,
        }) else {
            continue;
        };
        if let Some(element) = resolved
            .ctx
            .get(imported_library)
            .export_namespace
            .try_get()
            .and_then(|namespace| namespace.defined_names.get(&name))
            .copied()
            .filter(|element| Some(*element) != excluded && is_type_element(*element))
        {
            return Some(element);
        }
    }
    None
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
        if visible_type(
            context,
            node,
            text,
            context.declared_element(parameter),
            None,
        )
        .is_some()
        {
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
    if visible_type(
        context,
        node,
        text,
        context.declared_element(parameter),
        None,
    )
    .is_some()
    {
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
        if text == "_" && context.is_feature_enabled(crate::ExperimentalFlag::WildcardVariables) {
            continue;
        }
        let current = context.declared_element(parameter);
        let enclosing = current
            .and_then(|element| context.resolved?.ctx.element_data(element))
            .and_then(|data| data.enclosing);
        let visible = visible_type(
            context,
            context.ast.parent(node).unwrap_or(node),
            text,
            current,
            enclosing,
        );
        let method_type_parameter_shadow = enclosing
            .is_some_and(|element| element.is::<MethodElement>())
            && visible.is_some_and(|element| element.is::<TypeParameterElement>());
        if visible.is_some() && !method_type_parameter_shadow {
            context.report_token(
                out,
                &diag::AVOID_TYPES_AS_PARAMETER_NAMES_TYPE_PARAMETER,
                token,
                &[text],
            );
        }
    }
}
