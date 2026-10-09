// Dart source: pkg/linter/lib/src/util/leak_detector_visitor.dart

use crate::LinterContext;
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode};
use dartr_element::{
    ElemRef, ElementId, ExtensionTypeElement, FieldElement, FieldFormalParameterElement,
    FormalParameterElement, FragmentFlags, GetterElement, LocalVariableElement, SetterElement,
    TopLevelVariableElement,
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
    if declaration
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
    let Some(kind) = kinds
        .iter()
        .find(|kind| super::helpers::implements(context, ty, kind.library, kind.interface))
    else {
        return;
    };
    if has_valid_use(context, root, report_node, element, local, kind.method) {
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

fn element_matches(context: &LinterContext<'_>, element: ElemRef, variable: ElementId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let base = member::base_element(&resolved.ctx, element);
    let candidate = if let Some(getter) = base.cast::<GetterElement>() {
        resolved.ctx.get(getter).variable.get().map(|id| id.raw())
    } else if let Some(setter) = base.cast::<SetterElement>() {
        resolved.ctx.get(setter).variable.get().map(|id| id.raw())
    } else {
        Some(base)
    };
    let Some(candidate) = candidate else {
        return false;
    };
    if candidate == variable {
        return true;
    }
    representation_variable(context, candidate) == Some(variable)
        || representation_variable(context, variable) == Some(candidate)
}

fn representation_variable(context: &LinterContext<'_>, variable: ElementId) -> Option<ElementId> {
    if !variable.is::<FieldElement>()
        && !variable.is::<TopLevelVariableElement>()
        && !variable.is::<LocalVariableElement>()
        && !variable.is::<FormalParameterElement>()
    {
        return None;
    }
    let resolved = context.resolved?;
    let ty = member::type_(&resolved.ctx, ElemRef::Base(variable));
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
    required_method: &str,
) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        match context.ast.kind(node) {
            NodeKind::MethodInvocation => {
                let invocation = context.ast.cast::<MethodInvocation>(node).unwrap();
                let n = &context.ast[invocation];
                let real_target = method_invocation_real_target(context.ast, invocation);
                let method = context.ast.tokens.lexeme(context.ast[n.method_name].token);
                if method == required_method
                    && real_target
                        .is_some_and(|target| target_contains(context, target.raw(), variable))
                {
                    return true;
                }
                if method == required_method
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
                    if identifier_matches(context, argument.raw(), variable) {
                        return true;
                    }
                }
            }
            NodeKind::PrefixedIdentifier => {
                let n = &context.ast[context.ast.cast::<PrefixedIdentifier>(node).unwrap()];
                if identifier_matches(context, n.prefix.raw(), variable)
                    && context.ast.tokens.lexeme(context.ast[n.identifier].token) == required_method
                {
                    return true;
                }
            }
            NodeKind::ReturnStatement if local => {
                let n = &context.ast[context.ast.cast::<ReturnStatement>(node).unwrap()];
                if n.expression
                    .is_some_and(|e| identifier_matches(context, e.raw(), variable))
                {
                    return true;
                }
            }
            NodeKind::AssignmentExpression => {
                let n = &context.ast[context.ast.cast::<AssignmentExpression>(node).unwrap()];
                let right = n.right_hand_side.raw();
                if context.ast.kind(right) == NodeKind::SimpleIdentifier
                    && (canonical_assignment_target(context, n.left_hand_side.raw())
                        == Some(variable)
                        || assignment_property_has_name(context, n.left_hand_side.raw(), variable)
                        || (identifier_matches(context, right, variable)
                            && canonical_assignment_target(context, n.left_hand_side.raw())
                                .is_none()))
                {
                    return true;
                }
            }
            NodeKind::ConstructorFieldInitializer => {
                let n = &context.ast[context
                    .ast
                    .cast::<ConstructorFieldInitializer>(node)
                    .unwrap()];
                if canonical_assignment_target(context, n.field_name.raw()) == Some(variable) {
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
    super::helpers::ancestors(ast, node.raw()).find_map(|ancestor| {
        ast.cast::<CascadeExpression>(ancestor)
            .map(|cascade| ast[cascade].target)
    })
}

fn target_contains(context: &LinterContext<'_>, target: NodeId, variable: ElementId) -> bool {
    if identifier_matches(context, target, variable) {
        return true;
    }
    if let Some(p) = context.ast.cast::<PropertyAccess>(target) {
        let p = &context.ast[p];
        return p
            .target
            .is_some_and(|target| context.ast.kind(target) == NodeKind::ThisExpression)
            && context
                .element(p.property_name)
                .and_then(|e| super::helpers::base_element(context, e))
                == Some(variable);
    }
    if let Some(p) = context.ast.cast::<PrefixedIdentifier>(target) {
        return identifier_matches(context, context.ast[p].prefix.raw(), variable);
    }
    if let Some(p) = context.ast.cast::<PostfixExpression>(target) {
        return identifier_matches(context, context.ast[p].operand.raw(), variable);
    }
    false
}

fn canonical_assignment_target(context: &LinterContext<'_>, node: NodeId) -> Option<ElementId> {
    let element = context
        .element(node)
        .and_then(|element| super::helpers::base_element(context, element))
        .or_else(|| {
            context
                .ast
                .cast::<PropertyAccess>(node)
                .and_then(|property| context.element(context.ast[property].property_name))
                .and_then(|element| super::helpers::base_element(context, element))
        })?;
    let resolved = context.resolved?;
    if let Some(getter) = element.cast::<GetterElement>() {
        return resolved.ctx.get(getter).variable.get().map(|id| id.raw());
    }
    if let Some(setter) = element.cast::<SetterElement>() {
        return resolved.ctx.get(setter).variable.get().map(|id| id.raw());
    }
    Some(element)
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
