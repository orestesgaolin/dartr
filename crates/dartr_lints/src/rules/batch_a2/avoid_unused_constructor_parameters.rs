// Dart source: pkg/linter/lib/src/rules/avoid_unused_constructor_parameters.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, FieldFormalParameterElement, SuperFormalParameterElement};
use indexmap::IndexMap;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ConstructorDeclaration,
        "avoid_unused_constructor_parameters",
        check,
    );
    registry.add(
        NodeKind::PrimaryConstructorDeclaration,
        "avoid_unused_constructor_parameters",
        check,
    );
}

fn parameter_name(context: &LinterContext<'_>, node: NodeId) -> Option<dartr_syntax::TokenId> {
    match context.ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            context.ast[context.ast.cast::<RegularFormalParameter>(node)?].name
        }
        _ => None,
    }
}

fn primary_constructor_members(context: &LinterContext<'_>, node: NodeId) -> Vec<NodeId> {
    let Some(owner) = context.ast.parent(node) else {
        return vec![];
    };
    let body = match context.ast.kind(owner) {
        NodeKind::ClassDeclaration => context.ast[context
            .ast
            .cast::<ClassDeclaration>(owner)
            .expect("checked kind")]
        .body
        .raw(),
        NodeKind::EnumDeclaration => context.ast[context
            .ast
            .cast::<EnumDeclaration>(owner)
            .expect("checked kind")]
        .body
        .raw(),
        NodeKind::ExtensionTypeDeclaration => context.ast[context
            .ast
            .cast::<ExtensionTypeDeclaration>(owner)
            .expect("checked kind")]
        .body
        .raw(),
        _ => return vec![],
    };
    let members = match context.ast.kind(body) {
        NodeKind::BlockClassBody => {
            context.ast[context
                .ast
                .cast::<BlockClassBody>(body)
                .expect("checked kind")]
            .members
        }
        NodeKind::BlockEnumBody => {
            context.ast[context
                .ast
                .cast::<BlockEnumBody>(body)
                .expect("checked kind")]
            .members
        }
        _ => return vec![],
    };
    context.ast.list_raw(members).to_vec()
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (parameters, mut roots) = match context.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = &context.ast[context.ast.cast::<ConstructorDeclaration>(node).unwrap()];
            if n.augment_keyword.is_some()
                || n.redirected_constructor.is_some()
                || n.external_keyword.is_some()
            {
                return;
            }
            let mut roots = vec![n.body.raw()];
            roots.extend(context.ast.list_raw(n.initializers).iter().copied());
            (n.parameters, roots)
        }
        NodeKind::PrimaryConstructorDeclaration => {
            let n = &context.ast[context
                .ast
                .cast::<PrimaryConstructorDeclaration>(node)
                .unwrap()];
            let mut roots = vec![];
            for member in primary_constructor_members(context, node) {
                if let Some(primary_body) = context.ast.cast::<PrimaryConstructorBody>(member) {
                    roots.push(context.ast[primary_body].body.raw());
                    roots.extend(
                        context
                            .ast
                            .list_raw(context.ast[primary_body].initializers)
                            .iter()
                            .copied(),
                    );
                } else if let Some(field) = context.ast.cast::<FieldDeclaration>(member) {
                    let variables = &context.ast[context.ast[field].fields];
                    roots.extend(
                        context
                            .ast
                            .list(variables.variables)
                            .iter()
                            .filter_map(|variable| context.ast[*variable].initializer)
                            .map(Id::raw),
                    );
                }
            }
            (n.formal_parameters, roots)
        }
        _ => return,
    };
    let parameters = &context.ast[parameters];
    let mut unused: IndexMap<ElementId, (NodeId, dartr_syntax::TokenId)> = IndexMap::new();
    for &parameter in context.ast.list(parameters.parameters) {
        if context.ast.kind(parameter) != NodeKind::RegularFormalParameter {
            continue;
        }
        let Some(name) = parameter_name(context, parameter.raw()) else {
            continue;
        };
        if context.ast.tokens.lexeme(name).chars().all(|c| c == '_') {
            continue;
        }
        let Some(element) = context.declared_element(parameter) else {
            continue;
        };
        if element.is::<FieldFormalParameterElement>()
            || element.is::<SuperFormalParameterElement>()
            || super::deprecated_consistency::has_deprecated(
                context,
                context.ast[context
                    .ast
                    .cast::<RegularFormalParameter>(parameter)
                    .unwrap()]
                .metadata,
            )
        {
            continue;
        }
        unused.insert(element, (parameter.raw(), name));
    }
    let mut pending = std::mem::take(&mut roots);
    while let Some(current) = pending.pop() {
        if let Some(identifier) = context.ast.cast::<SimpleIdentifier>(current)
            && let Some(element) = context.element(identifier)
            && let Some(base) = super::helpers::base_element(context, element)
        {
            unused.shift_remove(&base);
        }
        pending.extend(context.ast.children(current));
    }
    for (_, (parameter, name)) in unused {
        let text = context.ast.tokens.lexeme(name);
        context.report_node(
            out,
            &diag::AVOID_UNUSED_CONSTRUCTOR_PARAMETERS,
            parameter,
            &[text],
        );
    }
}
