// Dart source: pkg/linter/lib/src/rules/parameter_assignments.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, ElementKind, ParameterKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_assignment_expression("parameter_assignments", check);
    registry.add_postfix_expression("parameter_assignments", check);
    registry.add_prefix_expression("parameter_assignments", check);
    registry.add_pattern_assignment("parameter_assignments", check_pattern_assignment);
}

fn target(ctx: &LinterContext<'_>, node: NodeId) -> Option<(NodeId, dartr_syntax::TokenId)> {
    match ctx.ast.kind(node) {
        NodeKind::AssignmentExpression => {
            let n = &ctx.ast[Id::<AssignmentExpression>::from_raw(node)];
            Some((n.left_hand_side.raw(), n.operator))
        }
        NodeKind::PostfixExpression => {
            let n = &ctx.ast[Id::<PostfixExpression>::from_raw(node)];
            Some((n.operand.raw(), n.operator))
        }
        NodeKind::PrefixExpression => {
            let n = &ctx.ast[Id::<PrefixExpression>::from_raw(node)];
            Some((n.operand.raw(), n.operator))
        }
        _ => None,
    }
}

fn parameter_element(ctx: &LinterContext<'_>, expression: NodeId) -> Option<ElementId> {
    let resolved = ctx.resolved?;
    if ctx.ast.kind(expression) != NodeKind::SimpleIdentifier {
        return None;
    }
    let element = member::base_element(&resolved.ctx, ctx.element(expression)?);
    (element.kind() == ElementKind::Parameter).then_some(element)
}

fn parameter_list_declares(
    ctx: &LinterContext<'_>,
    list: Id<FormalParameterList>,
    element: ElementId,
) -> bool {
    ctx.ast
        .list(ctx.ast[list].parameters)
        .iter()
        .any(|parameter| ctx.declared_element(parameter.raw()) == Some(element))
}

fn is_descendant_of(ctx: &LinterContext<'_>, mut node: NodeId, ancestor: NodeId) -> bool {
    loop {
        if node == ancestor {
            return true;
        }
        let Some(parent) = ctx.ast.parent(node) else {
            return false;
        };
        node = parent;
    }
}

fn parameter_body(ctx: &LinterContext<'_>, use_node: NodeId, element: ElementId) -> Option<NodeId> {
    let mut node = use_node;
    let mut primary_body = None;
    while let Some(parent) = ctx.ast.parent(node) {
        let declaration = match ctx.ast.kind(parent) {
            NodeKind::ConstructorDeclaration => {
                let declaration = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(parent)];
                Some((declaration.parameters, declaration.body.raw()))
            }
            NodeKind::MethodDeclaration => {
                let declaration = &ctx.ast[Id::<MethodDeclaration>::from_raw(parent)];
                declaration
                    .parameters
                    .map(|parameters| (parameters, declaration.body.raw()))
            }
            NodeKind::FunctionExpression => {
                let declaration = &ctx.ast[Id::<FunctionExpression>::from_raw(parent)];
                declaration
                    .parameters
                    .map(|parameters| (parameters, declaration.body.raw()))
            }
            _ => None,
        };
        if let Some((parameters, body)) = declaration
            && parameter_list_declares(ctx, parameters, element)
            && is_descendant_of(ctx, use_node, body)
        {
            return Some(body);
        }
        if ctx.ast.kind(parent) == NodeKind::PrimaryConstructorBody {
            primary_body = Some(parent);
        }
        let primary = match ctx.ast.kind(parent) {
            NodeKind::ClassDeclaration => Some(
                ctx.ast[Id::<ClassDeclaration>::from_raw(parent)]
                    .name_part
                    .raw(),
            ),
            NodeKind::EnumDeclaration => Some(
                ctx.ast[Id::<EnumDeclaration>::from_raw(parent)]
                    .name_part
                    .raw(),
            ),
            NodeKind::ExtensionTypeDeclaration => Some(
                ctx.ast[Id::<ExtensionTypeDeclaration>::from_raw(parent)]
                    .name_part
                    .raw(),
            ),
            _ => None,
        };
        if let (Some(primary), Some(primary_body)) = (primary, primary_body)
            && let Some(primary) = ctx.ast.cast::<PrimaryConstructorDeclaration>(primary)
            && parameter_list_declares(ctx, ctx.ast[primary].formal_parameters, element)
        {
            return Some(
                ctx.ast[Id::<PrimaryConstructorBody>::from_raw(primary_body)]
                    .body
                    .raw(),
            );
        }
        node = parent;
    }
    None
}

fn is_in_parameter_body(ctx: &LinterContext<'_>, mut node: NodeId, element: ElementId) -> bool {
    if !ctx
        .resolved
        .is_some_and(|resolved| resolved.potentially_mutated_in_scope.contains(&element))
    {
        return false;
    }
    let Some(body) = parameter_body(ctx, node, element) else {
        return false;
    };
    loop {
        if node == body {
            return true;
        }
        let Some(parent) = ctx.ast.parent(node) else {
            return false;
        };
        node = parent;
    }
}

fn default_is_implicit_null(ctx: &LinterContext<'_>, element: ElementId) -> bool {
    for i in 0..ctx.ast.node_count() {
        let node = NodeId::from_index(i);
        if !matches!(
            ctx.ast.kind(node),
            NodeKind::RegularFormalParameter
                | NodeKind::FieldFormalParameter
                | NodeKind::SuperFormalParameter
        ) {
            continue;
        }
        if ctx.declared_element(node) != Some(element) {
            continue;
        }
        return match ctx.ast.kind(node) {
            NodeKind::RegularFormalParameter => {
                let n = &ctx.ast[Id::<RegularFormalParameter>::from_raw(node)];
                (n.kind == ParameterKind::Positional
                    || n.kind == ParameterKind::Named && n.required_keyword.is_none())
                    && n.default_clause.is_none()
            }
            NodeKind::FieldFormalParameter => {
                let n = &ctx.ast[Id::<FieldFormalParameter>::from_raw(node)];
                (n.kind == ParameterKind::Positional
                    || n.kind == ParameterKind::Named && n.required_keyword.is_none())
                    && n.default_clause.is_none()
            }
            NodeKind::SuperFormalParameter => {
                let n = &ctx.ast[Id::<SuperFormalParameter>::from_raw(node)];
                (n.kind == ParameterKind::Positional
                    || n.kind == ParameterKind::Named && n.required_keyword.is_none())
                    && n.default_clause.is_none()
            }
            _ => false,
        };
    }
    false
}

fn earlier_if_null_assignment(ctx: &LinterContext<'_>, node: NodeId, element: ElementId) -> bool {
    (0..ctx.ast.node_count()).any(|i| {
        let other = NodeId::from_index(i);
        if ctx.ast.kind(other) != NodeKind::AssignmentExpression
            || ctx.ast.offset(other) >= ctx.ast.offset(node)
        {
            return false;
        }
        let n = &ctx.ast[Id::<AssignmentExpression>::from_raw(other)];
        ctx.ast.tokens.lexeme(n.operator) == "??="
            && parameter_element(ctx, n.left_hand_side.raw()) == Some(element)
            && is_in_parameter_body(ctx, other, element)
    })
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some((expression, operator)) = target(ctx, node) else {
        return;
    };
    let Some(element) = parameter_element(ctx, expression) else {
        return;
    };
    if !is_in_parameter_body(ctx, node, element) {
        return;
    }
    let implicit_null = default_is_implicit_null(ctx, element);
    let op = ctx.ast.tokens.lexeme(operator);
    if matches!(
        ctx.ast.kind(node),
        NodeKind::PrefixExpression | NodeKind::PostfixExpression
    ) && !matches!(op, "++" | "--")
    {
        return;
    }
    if implicit_null && matches!(op, "++" | "--") {
        return;
    }
    if implicit_null && op == "??=" && !earlier_if_null_assignment(ctx, node, element) {
        return;
    }
    let name = ctx
        .resolved
        .and_then(|r| r.ctx.element_name(element))
        .unwrap_or("");
    ctx.report_node(out, &diag::PARAMETER_ASSIGNMENTS, node, &[name]);
}

fn report_assigned_pattern(
    ctx: &LinterContext<'_>,
    root: NodeId,
    candidate: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    if ctx.ast.kind(candidate) != NodeKind::AssignedVariablePattern {
        return;
    }
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let Some(element) = ctx.element(candidate) else {
        return;
    };
    let element = member::base_element(&resolved.ctx, element);
    if element.kind() != ElementKind::Parameter || !is_in_parameter_body(ctx, root, element) {
        return;
    }
    let name = resolved.ctx.element_name(element).unwrap_or("");
    ctx.report_node(out, &diag::PARAMETER_ASSIGNMENTS, root, &[name]);
}

fn check_pattern_assignment(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let pattern = ctx.ast[Id::<PatternAssignment>::from_raw(node)]
        .pattern
        .raw();
    match ctx.ast.kind(pattern) {
        NodeKind::RecordPattern => {
            for field in ctx
                .ast
                .list(ctx.ast[Id::<RecordPattern>::from_raw(pattern)].fields)
            {
                report_assigned_pattern(ctx, pattern, ctx.ast[*field].pattern.raw(), out);
            }
        }
        NodeKind::ObjectPattern => {
            for field in ctx
                .ast
                .list(ctx.ast[Id::<ObjectPattern>::from_raw(pattern)].fields)
            {
                report_assigned_pattern(ctx, pattern, ctx.ast[*field].pattern.raw(), out);
            }
        }
        NodeKind::ListPattern => {
            for element in ctx
                .ast
                .list(ctx.ast[Id::<ListPattern>::from_raw(pattern)].elements)
            {
                report_assigned_pattern(ctx, pattern, element.raw(), out);
            }
        }
        NodeKind::MapPattern => {
            for element in ctx
                .ast
                .list(ctx.ast[Id::<MapPattern>::from_raw(pattern)].elements)
            {
                if let Some(entry) = ctx.ast.cast::<MapPatternEntry>(*element) {
                    report_assigned_pattern(ctx, pattern, ctx.ast[entry].value.raw(), out);
                } else {
                    report_assigned_pattern(ctx, pattern, element.raw(), out);
                }
            }
        }
        _ => {}
    }
}
