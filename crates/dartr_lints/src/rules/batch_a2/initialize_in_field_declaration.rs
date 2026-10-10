// Dart source: pkg/linter/lib/src/rules/initialize_in_field_declaration.dart

use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FieldElement, FormalParameterElement, FragmentFlags};
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry, context: &LinterContext<'_>) {
    if context.is_feature_enabled(ExperimentalFlag::PrimaryConstructors) {
        registry.add(
            NodeKind::PrimaryConstructorBody,
            "initialize_in_field_declaration",
            check,
        );
    }
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let body = &context.ast[context.ast.cast::<PrimaryConstructorBody>(node).unwrap()];
    let Some(declaration) = super::helpers::ancestors(context.ast, node).find_map(|ancestor| {
        let name_part = match context.ast.kind(ancestor) {
            NodeKind::ClassDeclaration => {
                context.ast[context.ast.cast::<ClassDeclaration>(ancestor)?].name_part
            }
            NodeKind::EnumDeclaration => {
                context.ast[context.ast.cast::<EnumDeclaration>(ancestor)?].name_part
            }
            NodeKind::ExtensionTypeDeclaration => {
                context.ast[context.ast.cast::<ExtensionTypeDeclaration>(ancestor)?].name_part
            }
            _ => return None,
        };
        context.ast.cast::<PrimaryConstructorDeclaration>(name_part)
    }) else {
        return;
    };
    let Some(constructor) = context
        .declared_element(declaration)
        .and_then(|element| element.cast::<dartr_element::ConstructorElement>())
        .map(|element| element.raw())
    else {
        return;
    };
    let constructor_enclosing =
        member::enclosing_element(&resolved.ctx, dartr_element::ElemRef::Base(constructor));
    for &initializer in context.ast.list(body.initializers) {
        let Some(initializer) = context.ast.cast::<ConstructorFieldInitializer>(initializer) else {
            continue;
        };
        let initializer = &context.ast[initializer];
        let Some(field) = context
            .element(initializer.field_name)
            .and_then(|e| super::helpers::base_element(context, e))
        else {
            continue;
        };
        if !field.is::<FieldElement>()
            || member::enclosing_element(&resolved.ctx, dartr_element::ElemRef::Base(field))
                != constructor_enclosing
        {
            continue;
        }
        let is_late = resolved
            .ctx
            .element_data(field)
            .and_then(|e| resolved.ctx.fragment_data(e.first_fragment))
            .is_some_and(|f| f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE));
        if is_late {
            continue;
        }
        let mut pending = vec![initializer.expression.raw()];
        let mut references_parameter = false;
        while let Some(current) = pending.pop() {
            if context.ast.kind(current) == NodeKind::SimpleIdentifier
                && let Some(element) = context
                    .element(current)
                    .and_then(|e| super::helpers::base_element(context, e))
                && element.is::<FormalParameterElement>()
                && member::enclosing_element(&resolved.ctx, dartr_element::ElemRef::Base(element))
                    == Some(constructor)
            {
                references_parameter = true;
                break;
            }
            pending.extend(context.ast.children(current));
        }
        if references_parameter {
            context.report_node(
                out,
                &diag::INITIALIZE_IN_FIELD_DECLARATION,
                initializer.field_name,
                &[],
            );
        }
    }
}
