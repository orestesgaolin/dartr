// Dart source: pkg/linter/lib/src/rules/analyzer_element_model_tracking.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    BlockClassBody, ClassDeclaration, FieldDeclaration, Id, MethodDeclaration, NodeId, NodeKind,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElemRef;
use dartr_typesystem::member;

use super::helpers::{element_library_uri, element_name, lexeme, metadata};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ClassDeclaration,
        "analyzer_element_model_tracking",
        check,
    );
}

fn annotation_kind<'a>(c: &'a LinterContext<'_>, node: NodeId) -> Option<&'a str> {
    let element = c.element(node)?;
    if element_library_uri(c, element) == Some("package:analyzer/src/fine/annotations.dart") {
        element_name(c, element)
    } else {
        None
    }
}

fn tracking<'a>(c: &'a LinterContext<'a>, node: NodeId) -> Vec<(NodeId, &'a str)> {
    metadata(c.ast, node)
        .into_iter()
        .filter_map(|annotation| {
            let kind = annotation_kind(c, annotation.raw())?;
            kind.starts_with("tracked")
                .then_some((annotation.raw(), kind))
        })
        .collect()
}

fn validate(
    c: &LinterContext<'_>,
    annotations: &[(NodeId, &str)],
    allowed: &[&str],
    required: bool,
    name: dartr_syntax::TokenId,
    out: &mut Vec<Diagnostic>,
) {
    let mut found = false;
    for &(annotation, kind) in annotations {
        if allowed.contains(&kind) {
            if found {
                c.report_node(
                    out,
                    &diag::ANALYZER_ELEMENT_MODEL_TRACKING_MORE_THAN_ONE,
                    annotation,
                    &[],
                );
            }
            found = true;
        } else {
            c.report_node(
                out,
                &diag::ANALYZER_ELEMENT_MODEL_TRACKING_BAD,
                annotation,
                &[],
            );
        }
    }
    if required && !found {
        c.report_token(out, &diag::ANALYZER_ELEMENT_MODEL_TRACKING_ZERO, name, &[]);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if !metadata(c.ast, node)
        .iter()
        .any(|annotation| annotation_kind(c, annotation.raw()) == Some("elementClass"))
    {
        return;
    }
    let class = &c.ast[Id::<ClassDeclaration>::from_raw(node)];
    let Some(body) = c.ast.cast::<BlockClassBody>(class.body) else {
        return;
    };
    for member in c.ast.list_raw(c.ast[body].members) {
        let annotations = tracking(c, *member);
        match c.ast.kind(*member) {
            NodeKind::ConstructorDeclaration => {
                validate(c, &annotations, &[], false, c.ast.begin_token(*member), out)
            }
            NodeKind::FieldDeclaration => {
                let field = &c.ast[Id::<FieldDeclaration>::from_raw(*member)];
                for variable in c.ast.list(c.ast[field.fields].variables) {
                    let name = c.ast[*variable].name;
                    let required =
                        field.static_keyword.is_none() && !lexeme(c, name).starts_with('_');
                    validate(
                        c,
                        &annotations,
                        &[
                            "trackedIncludedInId",
                            "trackedIndirectly",
                            "trackedInternal",
                        ],
                        required,
                        name,
                        out,
                    );
                }
            }
            NodeKind::MethodDeclaration => {
                let method = &c.ast[Id::<MethodDeclaration>::from_raw(*member)];
                let is_setter = method
                    .property_keyword
                    .is_some_and(|keyword| lexeme(c, keyword) == "set");
                let is_abstract = c.declared_element(*member).is_some_and(|element| {
                    c.resolved
                        .is_some_and(|r| member::is_abstract(&r.ctx, ElemRef::Base(element)))
                });
                let required = method
                    .modifier_keyword
                    .is_none_or(|keyword| lexeme(c, keyword) != "static")
                    && !lexeme(c, method.name).starts_with('_')
                    && !is_setter
                    && !is_abstract;
                validate(
                    c,
                    &annotations,
                    &[
                        "trackedDirectly",
                        "trackedDirectlyExpensive",
                        "trackedDirectlyOpaque",
                        "trackedIncludedInId",
                        "trackedIndirectly",
                        "trackedInternal",
                    ],
                    required,
                    method.name,
                    out,
                );
            }
            _ => {}
        }
    }
}
