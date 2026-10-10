// Dart source: pkg/linter/lib/src/rules/deprecated_consistency.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ConstructorElement, FieldFormalParameterElement};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ConstructorDeclaration,
        "deprecated_consistency",
        check_constructor,
    );
    registry.add(
        NodeKind::PrimaryConstructorDeclaration,
        "deprecated_consistency",
        check_constructor,
    );
    registry.add(
        NodeKind::FieldFormalParameter,
        "deprecated_consistency",
        check_parameter,
    );
}
pub(super) fn has_deprecated(context: &LinterContext<'_>, metadata: NodeList<Annotation>) -> bool {
    has_annotation(
        context,
        metadata,
        "dart:core",
        &["deprecated", "Deprecated"],
    )
}

pub(super) fn has_annotation(
    context: &LinterContext<'_>,
    metadata: NodeList<Annotation>,
    expected_library: &str,
    expected_names: &[&str],
) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    context.ast.list(metadata).iter().any(|&annotation| {
        context.element(annotation).is_some_and(|element| {
            let base = member::base_element(&resolved.ctx, element);
            let name = if base.is::<ConstructorElement>() {
                resolved
                    .ctx
                    .element_data(base)
                    .and_then(|data| data.enclosing)
                    .and_then(|enclosing| resolved.ctx.element_name(enclosing))
            } else {
                resolved.ctx.element_name(base)
            };
            let library_matches = member::library(&resolved.ctx, element)
                .is_some_and(|library| resolved.ctx.library_uri(library) == expected_library);
            name.is_some_and(|name| expected_names.contains(&name)) && library_matches
        })
    })
}
fn check_constructor(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(container) = super::helpers::ancestors(context.ast, node).find(|node| {
        matches!(
            context.ast.kind(*node),
            NodeKind::ClassDeclaration
                | NodeKind::EnumDeclaration
                | NodeKind::ExtensionTypeDeclaration
        )
    }) else {
        return;
    };
    let enclosing_metadata = match context.ast.kind(container) {
        NodeKind::ClassDeclaration => {
            context.ast[context.ast.cast::<ClassDeclaration>(container).unwrap()].metadata
        }
        NodeKind::EnumDeclaration => {
            context.ast[context.ast.cast::<EnumDeclaration>(container).unwrap()].metadata
        }
        NodeKind::ExtensionTypeDeclaration => {
            context.ast[context
                .ast
                .cast::<ExtensionTypeDeclaration>(container)
                .unwrap()]
            .metadata
        }
        _ => return,
    };
    if !has_deprecated(context, enclosing_metadata) {
        return;
    }
    let own_deprecated = match context.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let metadata =
                context.ast[context.ast.cast::<ConstructorDeclaration>(node).unwrap()].metadata;
            has_deprecated(context, metadata)
        }
        NodeKind::PrimaryConstructorDeclaration => {
            let mut pending = vec![container];
            let mut found = false;
            while let Some(current) = pending.pop() {
                if let Some(body) = context.ast.cast::<PrimaryConstructorBody>(current) {
                    found = has_deprecated(context, context.ast[body].metadata);
                    break;
                }
                pending.extend(context.ast.children(current));
            }
            found
        }
        _ => return,
    };
    if !own_deprecated {
        report_constructor(context, node, out);
    }
}

fn report_constructor(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let range = match context.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let constructor =
                &context.ast[context.ast.cast::<ConstructorDeclaration>(node).unwrap()];
            let start = constructor
                .type_name
                .map(|name| context.ast.offset(name))
                .or_else(|| {
                    constructor
                        .new_keyword
                        .map(|keyword| context.ast.tokens.offset(keyword))
                })
                .or_else(|| {
                    constructor
                        .factory_keyword
                        .map(|keyword| context.ast.tokens.offset(keyword))
                });
            start.map(|start| {
                let end = constructor
                    .name
                    .map(|name| context.ast.tokens.get(name).end())
                    .or_else(|| constructor.type_name.map(|name| context.ast.end(name)))
                    .or_else(|| {
                        constructor
                            .new_keyword
                            .map(|keyword| context.ast.tokens.get(keyword).end())
                    })
                    .or_else(|| {
                        constructor
                            .factory_keyword
                            .map(|keyword| context.ast.tokens.get(keyword).end())
                    })
                    .unwrap_or(start);
                (start, end)
            })
        }
        NodeKind::PrimaryConstructorDeclaration => {
            let constructor = &context.ast[context
                .ast
                .cast::<PrimaryConstructorDeclaration>(node)
                .unwrap()];
            let start = context.ast.tokens.offset(constructor.type_name);
            let end = constructor
                .constructor_name
                .map(|name| context.ast.tokens.get(context.ast[name].name).end())
                .unwrap_or_else(|| context.ast.tokens.get(constructor.type_name).end());
            Some((start, end))
        }
        _ => None,
    };
    if let Some((start, end)) = range {
        context.report_offset(
            out,
            &diag::DEPRECATED_CONSISTENCY_CONSTRUCTOR,
            start as usize,
            (end - start) as usize,
            &[],
        );
    }
}

fn check_parameter(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let Some(parameter) = context
        .declared_element(node)
        .and_then(|element| element.cast::<FieldFormalParameterElement>())
    else {
        return;
    };
    let Some(field) = resolved.ctx.get(parameter).field.get() else {
        return;
    };
    if resolved
        .ctx
        .element_name(field.raw())
        .is_some_and(|name| name.starts_with('_'))
    {
        return;
    }
    let Some(field_node) =
        (0..context.ast.node_count())
            .map(NodeId::from_index)
            .find(|&candidate| {
                context.ast.kind(candidate) == NodeKind::VariableDeclaration
                    && context.declared_element(candidate) == Some(field.raw())
            })
    else {
        return;
    };
    let field_deprecated = super::helpers::ancestors(context.ast, field_node)
        .find_map(|ancestor| context.ast.cast::<FieldDeclaration>(ancestor))
        .is_some_and(|declaration| has_deprecated(context, context.ast[declaration].metadata));
    let parameter_metadata =
        context.ast[context.ast.cast::<FieldFormalParameter>(node).unwrap()].metadata;
    let parameter_deprecated = has_deprecated(context, parameter_metadata);
    if field_deprecated && !parameter_deprecated {
        context.report_node(out, &diag::DEPRECATED_CONSISTENCY_PARAMETER, node, &[]);
    } else if parameter_deprecated && !field_deprecated {
        context.report_token(
            out,
            &diag::DEPRECATED_CONSISTENCY_FIELD,
            context.ast[context.ast.cast::<VariableDeclaration>(field_node).unwrap()].name,
            &[],
        );
    }
}
