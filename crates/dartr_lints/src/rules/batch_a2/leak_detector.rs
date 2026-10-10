// Dart source: pkg/linter/lib/src/util/leak_detector_visitor.dart

use crate::LinterContext;
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode};
use dartr_element::{
    ElemRef, ElementId, ExtensionTypeElement, FieldElement, FieldFormalParameterElement,
    FragmentFlags, GetterElement, SetterElement, VariableElement,
};
use dartr_typesystem::{TypeExt, member};

pub struct LeakKind {
    pub library: &'static str,
    pub interface: &'static str,
    pub method: &'static str,
}

pub fn check_declaration(
    context: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
    code: &'static DiagnosticCode,
    kinds: &[LeakKind],
) {
    match context.ast.kind(node) {
        NodeKind::FieldDeclaration => {
            let n = &context.ast[context.ast.cast::<FieldDeclaration>(node).unwrap()];
            let root = std::iter::successors(Some(node), |n| context.ast.parent(*n))
                .last()
                .unwrap();
            for &variable in context.ast.list(context.ast[n.fields].variables) {
                check_variable(context, variable.raw(), root, false, out, code, kinds);
            }
        }
        NodeKind::VariableDeclarationStatement => {
            let n = &context.ast[context
                .ast
                .cast::<VariableDeclarationStatement>(node)
                .unwrap()];
            let Some(root) = super::helpers::nearest_function_body(context.ast, node) else {
                return;
            };
            for &variable in context.ast.list(context.ast[n.variables].variables) {
                check_variable(context, variable.raw(), root, true, out, code, kinds);
            }
        }
        NodeKind::PrimaryConstructorDeclaration => {
            if super::helpers::ancestors(context.ast, node)
                .any(|node| context.ast.kind(node) == NodeKind::ExtensionTypeDeclaration)
            {
                return;
            }
            let Some(resolved) = context.resolved else {
                return;
            };
            let root = std::iter::successors(Some(node), |node| context.ast.parent(*node))
                .last()
                .unwrap();
            let declaration = &context.ast[context
                .ast
                .cast::<PrimaryConstructorDeclaration>(node)
                .unwrap()];
            for &parameter in context
                .ast
                .list(context.ast[declaration.formal_parameters].parameters)
            {
                let Some(formal) = context
                    .declared_element(parameter)
                    .and_then(|element| element.cast::<FieldFormalParameterElement>())
                else {
                    continue;
                };
                let data = resolved.ctx.get(formal);
                let declaring =
                    resolved
                        .ctx
                        .fragment_data(data.first_fragment)
                        .is_some_and(|fragment| {
                            fragment
                                .flags
                                .has(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
                        });
                let Some(field) = data.field.get().filter(|_| declaring) else {
                    continue;
                };
                check_element(
                    context,
                    parameter.raw(),
                    field.raw(),
                    root,
                    false,
                    out,
                    code,
                    kinds,
                );
            }
        }
        _ => {}
    }
}

fn check_variable(
    context: &LinterContext<'_>,
    variable: NodeId,
    root: NodeId,
    local: bool,
    out: &mut Vec<Diagnostic>,
    code: &'static DiagnosticCode,
    kinds: &[LeakKind],
) {
    let declaration = &context.ast[context.ast.cast::<VariableDeclaration>(variable).unwrap()];
    if declaration.equals.is_some()
        && declaration
            .initializer
            .is_some_and(|e| context.ast.kind(e) == NodeKind::SimpleIdentifier)
    {
        return;
    }
    let Some(element) = context.declared_element(variable) else {
        return;
    };
    check_element(context, variable, element, root, local, out, code, kinds);
}

#[allow(clippy::too_many_arguments)]
fn check_element(
    context: &LinterContext<'_>,
    report_node: NodeId,
    element: ElementId,
    root: NodeId,
    local: bool,
    out: &mut Vec<Diagnostic>,
    code: &'static DiagnosticCode,
    kinds: &[LeakKind],
) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let ty = member::type_(&resolved.ctx, ElemRef::Base(element));
    let required_methods: Vec<_> = kinds
        .iter()
        .filter(|kind| super::helpers::implements(context, ty, kind.library, kind.interface))
        .map(|kind| kind.method)
        .collect();
    if required_methods.is_empty() {
        return;
    }
    if has_valid_use(
        context,
        root,
        report_node,
        element,
        local,
        &required_methods,
    ) {
        return;
    }
    context.report_node(out, code, report_node, &[]);
}

fn identifier_matches(context: &LinterContext<'_>, node: NodeId, variable: ElementId) -> bool {
    context.ast.kind(node) == NodeKind::SimpleIdentifier
        && context
            .element(node)
            .is_some_and(|element| element_matches(context, element, variable))
}

fn identifier_is_equal(context: &LinterContext<'_>, node: NodeId, variable: ElementId) -> bool {
    context.ast.kind(node) == NodeKind::SimpleIdentifier
        && context
            .element(node)
            .is_some_and(|element| element_is_equal(context, element, variable))
}

fn identifier_is_exact(context: &LinterContext<'_>, node: NodeId, variable: ElementId) -> bool {
    context.ast.kind(node) == NodeKind::SimpleIdentifier
        && context.element(node) == Some(ElemRef::Base(variable))
}

fn element_is_equal(context: &LinterContext<'_>, element: ElemRef, variable: ElementId) -> bool {
    element == ElemRef::Base(variable) || element_matches(context, element, variable)
}

fn element_matches(context: &LinterContext<'_>, element: ElemRef, variable: ElementId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let base = member::base_element(&resolved.ctx, element);
    let matches_base = if let Some(getter) = base.cast::<GetterElement>() {
        resolved.ctx.get(getter).variable.get().map(|id| id.raw()) == Some(variable)
    } else if let Some(setter) = base.cast::<SetterElement>() {
        resolved.ctx.get(setter).variable.get().map(|id| id.raw()) == Some(variable)
    } else if base.is::<FieldElement>() {
        base == variable
    } else {
        false
    };
    if matches_base {
        return true;
    }
    representation_variable(context, element) == Some(variable)
        || representation_variable(context, ElemRef::Base(variable))
            .is_some_and(|representation| ElemRef::Base(representation) == element)
}

fn representation_variable(context: &LinterContext<'_>, variable: ElemRef) -> Option<ElementId> {
    let resolved = context.resolved?;
    if !member::base_element(&resolved.ctx, variable).is::<VariableElement>() {
        return None;
    }
    let ty = member::type_(&resolved.ctx, variable);
    let extension = resolved
        .ctx
        .interface_element(ty)?
        .raw()
        .cast::<ExtensionTypeElement>()?;
    resolved
        .ctx
        .get(extension)
        .fields
        .first()
        .map(|field| field.raw())
}

fn has_valid_use(
    context: &LinterContext<'_>,
    root: NodeId,
    variable_node: NodeId,
    variable: ElementId,
    local: bool,
    required_methods: &[&str],
) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        match context.ast.kind(node) {
            NodeKind::MethodInvocation => {
                let invocation = context.ast.cast::<MethodInvocation>(node).unwrap();
                let n = &context.ast[invocation];
                let real_target = method_invocation_real_target(context.ast, invocation);
                let method = context.ast.tokens.lexeme(context.ast[n.method_name].token);
                if required_methods.contains(&method)
                    && real_target
                        .is_some_and(|target| target_contains(context, target.raw(), variable))
                {
                    return true;
                }
                if required_methods.contains(&method)
                    && real_target.is_some()
                    && std::iter::successors(Some(node), |node| context.ast.parent(*node))
                        .any(|ancestor| ancestor == variable_node)
                {
                    return true;
                }
                if real_target.is_some_and(|target| {
                    context.ast.kind(target) == NodeKind::SimpleIdentifier
                        && identifier_matches(context, target.raw(), variable)
                }) {
                    return true;
                }
                for &argument in context.ast.list(context.ast[n.argument_list].arguments) {
                    if identifier_is_exact(context, argument.raw(), variable) {
                        return true;
                    }
                }
            }
            NodeKind::PrefixedIdentifier => {
                let n = &context.ast[context.ast.cast::<PrefixedIdentifier>(node).unwrap()];
                if identifier_is_exact(context, n.prefix.raw(), variable)
                    && required_methods
                        .contains(&context.ast.tokens.lexeme(context.ast[n.identifier].token))
                {
                    return true;
                }
            }
            NodeKind::ReturnStatement if local => {
                let n = &context.ast[context.ast.cast::<ReturnStatement>(node).unwrap()];
                if n.expression
                    .is_some_and(|e| identifier_is_equal(context, e.raw(), variable))
                {
                    return true;
                }
            }
            NodeKind::AssignmentExpression => {
                let n = &context.ast[context.ast.cast::<AssignmentExpression>(node).unwrap()];
                let right = n.right_hand_side.raw();
                let write_element = context
                    .resolved
                    .and_then(|resolved| resolved.tables.write_element.get(node).copied());
                if context.ast.kind(right) == NodeKind::SimpleIdentifier
                    && (write_element
                        .is_some_and(|element| element_is_equal(context, element, variable))
                        || assignment_property_has_name(context, n.left_hand_side.raw(), variable)
                        || (identifier_is_equal(context, right, variable)
                            && write_element.is_none()))
                {
                    return true;
                }
            }
            NodeKind::ConstructorFieldInitializer => {
                let n = &context.ast[context
                    .ast
                    .cast::<ConstructorFieldInitializer>(node)
                    .unwrap()];
                if identifier_is_exact(context, n.field_name.raw(), variable) {
                    return true;
                }
            }
            NodeKind::FieldFormalParameter
                if !local
                    && context
                        .declared_element(node)
                        .and_then(|element| element.cast::<FieldFormalParameterElement>())
                        .and_then(|element| context.resolved?.ctx.get(element).field.get())
                        .is_some_and(|field| field.raw() == variable) =>
            {
                return true;
            }
            _ => {}
        }
        pending.extend(context.ast.children(node));
    }
    false
}

fn method_invocation_real_target(ast: &Ast, node: Id<MethodInvocation>) -> Option<Id<Expression>> {
    let invocation = &ast[node];
    let is_cascaded = invocation
        .operator
        .is_some_and(|operator| matches!(ast.tokens.lexeme(operator), ".." | "?.."));
    if !is_cascaded {
        return invocation.target;
    }
    ancestor_cascade_target(ast, node.raw())
}

fn property_access_real_target(ast: &Ast, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
    let access = &ast[node];
    if !matches!(ast.tokens.lexeme(access.operator), ".." | "?..") {
        return access.target;
    }
    ancestor_cascade_target(ast, node.raw())
}

fn ancestor_cascade_target(ast: &Ast, node: NodeId) -> Option<Id<Expression>> {
    super::helpers::ancestors(ast, node).find_map(|ancestor| {
        ast.cast::<CascadeExpression>(ancestor)
            .map(|cascade| ast[cascade].target)
    })
}

fn target_contains(context: &LinterContext<'_>, target: NodeId, variable: ElementId) -> bool {
    if identifier_is_equal(context, target, variable) {
        return true;
    }
    if let Some(property) = context.ast.cast::<PropertyAccess>(target) {
        let p = &context.ast[property];
        return property_access_real_target(context.ast, property)
            .is_some_and(|target| context.ast.kind(target) == NodeKind::ThisExpression)
            && context
                .element(p.property_name)
                .is_some_and(|element| element_is_equal(context, element, variable));
    }
    if let Some(p) = context.ast.cast::<PostfixExpression>(target) {
        return identifier_is_equal(context, context.ast[p].operand.raw(), variable);
    }
    false
}

fn assignment_property_has_name(
    context: &LinterContext<'_>,
    node: NodeId,
    variable: ElementId,
) -> bool {
    let Some(property) = context.ast.cast::<PropertyAccess>(node) else {
        return false;
    };
    let Some(variable_name) = context
        .resolved
        .and_then(|resolved| resolved.ctx.element_name(variable))
    else {
        return false;
    };
    context
        .ast
        .tokens
        .lexeme(context.ast[context.ast[property].property_name].token)
        == variable_name
}
