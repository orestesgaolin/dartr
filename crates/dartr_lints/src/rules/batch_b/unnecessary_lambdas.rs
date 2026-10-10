// Dart source: pkg/linter/lib/src/rules/unnecessary_lambdas.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, FnParam, FragmentFlags, Tag, TypeId, TypeKind};
use indexmap::{IndexMap, IndexSet};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::FunctionExpression,
        "unnecessary_lambdas",
        visit_function_expression,
    );
}

/// Dart `ExpressionExtension.canonicalElement` (analyzer).
fn canonical(c: &LinterContext<'_>, expression: NodeId) -> Option<ElementId> {
    canonical_element(c, expression).and_then(|e| c.canonical_element2(e))
}

/// Dart `_extractElementsOfSimpleIdentifiers`.
fn extract_elements_of_simple_identifiers(
    c: &LinterContext<'_>,
    node: NodeId,
) -> IndexSet<Option<ElementId>> {
    let mut elements = IndexSet::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::SimpleIdentifier {
            elements.insert(c.element(n).map(|e| base(c, e)));
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
    elements
}

/// Dart `Element?.isFinal` extension of this rule.
fn element_is_final(c: &LinterContext<'_>, element: Option<ElementId>) -> bool {
    let Some(element) = element else { return true };
    let Some(ctx) = rctx(c) else { return true };
    let variable_is_final = |v: ElementId| {
        flags(c, v).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
            && !flags(c, v).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE)
    };
    match element.tag() {
        Tag::Getter | Tag::Setter => {
            let origin_variable = flags(c, element)
                .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE);
            let variable = ctx
                .property_accessor(dartr_element::EId::from_raw(element))
                .variable
                .get()
                .map(|v| v.raw());
            origin_variable && variable.is_some_and(variable_is_final)
        }
        Tag::Field
        | Tag::TopLevelVariable
        | Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable
        | Tag::FormalParameter
        | Tag::FieldFormalParameter
        | Tag::SuperFormalParameter => variable_is_final(element),
        _ => true,
    }
}

/// Dart `_FinalExpressionChecker.isFinalNode`.
fn is_final_node(
    c: &LinterContext<'_>,
    parameters: &IndexSet<Option<ElementId>>,
    node: Option<NodeId>,
) -> bool {
    let Some(node) = node else { return true };
    let node = unparenthesized(c, node);
    match kind(c, node) {
        NodeKind::FunctionExpression => {
            let referenced = extract_elements_of_simple_identifiers(c, node);
            !referenced.iter().any(|e| parameters.contains(e))
        }
        NodeKind::PrefixedIdentifier => {
            let n = &c.ast[Id::<PrefixedIdentifier>::from_raw(node)];
            is_final_node(c, parameters, Some(n.prefix.raw()))
                && is_final_node(c, parameters, Some(n.identifier.raw()))
        }
        NodeKind::PropertyAccess => {
            let n = &c.ast[Id::<PropertyAccess>::from_raw(node)];
            is_final_node(c, parameters, n.target.map(|t| t.raw()))
                && is_final_node(c, parameters, Some(n.property_name.raw()))
        }
        NodeKind::SimpleIdentifier => {
            let element = c.element(node).map(|e| base(c, e));
            if parameters.contains(&element) {
                return false;
            }
            element_is_final(c, element)
        }
        _ => false,
    }
}

/// Dart `Expression?.mightBeDeferred`.
fn might_be_deferred(c: &LinterContext<'_>, node: Option<NodeId>) -> bool {
    let Some(node) = node else { return false };
    let element = if let Some(p) = c.ast.cast::<PrefixedIdentifier>(node) {
        c.element(c.ast[p].prefix)
    } else if kind(c, node) == NodeKind::SimpleIdentifier {
        c.element(node)
    } else {
        None
    };
    element.map(|e| base(c, e)).is_some_and(|e| {
        e.tag() == Tag::Prefix && super::prefer_const_constructors::prefix_is_deferred(c, e)
    })
}

/// Dart `Expression.containsNullAwareInvocationInChain`.
fn contains_null_aware_invocation_in_chain(c: &LinterContext<'_>, node: Option<NodeId>) -> bool {
    let Some(node) = node else { return false };
    if let Some(n) = c.ast.cast::<PropertyAccess>(node) {
        lexeme(c, c.ast[n].operator).starts_with('?')
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else if let Some(n) = c.ast.cast::<MethodInvocation>(node) {
        c.ast[n]
            .operator
            .is_some_and(|o| lexeme(c, o).starts_with('?'))
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else if let Some(n) = c.ast.cast::<IndexExpression>(node) {
        c.ast[n].question.is_some()
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else {
        false
    }
}

/// Dart `argumentsMatchParameters` (`util/dart_type_utilities.dart`).
pub(crate) fn arguments_match_parameters(
    c: &LinterContext<'_>,
    arguments: &[NodeId],
    parameters: &[NodeId],
) -> bool {
    let mut named_parameters: IndexMap<&str, Option<ElementId>> = IndexMap::new();
    let mut named_arguments: IndexMap<&str, ElementId> = IndexMap::new();
    let mut positional_parameters = Vec::new();
    let mut positional_arguments = Vec::new();
    for &parameter in parameters {
        let Some(identifier) = super::prefer_iterable_wheretype::parameter_name(c, parameter)
        else {
            continue;
        };
        let element = c.declared_element(parameter);
        if parameter_kind(c, parameter).is_named() {
            named_parameters.insert(identifier, element);
        } else {
            positional_parameters.push(element);
        }
    }
    for &argument in arguments {
        if let Some(named) = c.ast.cast::<NamedArgument>(argument) {
            let Some(element) = canonical(c, c.ast[named].argument_expression.raw()) else {
                return false;
            };
            named_arguments.insert(lexeme(c, c.ast[named].name), element);
        } else {
            let Some(element) = canonical(c, argument) else {
                return false;
            };
            positional_arguments.push(element);
        }
    }
    if positional_parameters.len() != positional_arguments.len()
        || named_parameters.len() != named_arguments.len()
    {
        return false;
    }
    for (argument, parameter) in positional_arguments.iter().zip(&positional_parameters) {
        if Some(*argument) != *parameter {
            return false;
        }
    }
    for (key, parameter) in &named_parameters {
        if named_arguments.get(key).copied() != *parameter {
            return false;
        }
    }
    true
}

fn parameter_kind(c: &LinterContext<'_>, parameter: NodeId) -> ParameterKind {
    match kind(c, parameter) {
        NodeKind::RegularFormalParameter => {
            c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].kind
        }
        NodeKind::FieldFormalParameter => {
            c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].kind
        }
        NodeKind::SuperFormalParameter => {
            c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].kind
        }
        _ => ParameterKind::Required,
    }
}

/// Dart `_Visitor.parametersMatch`.
fn parameters_match(c: &LinterContext<'_>, invocation_type: TypeId, node_type: TypeId) -> bool {
    let Some(ctx) = rctx(c) else { return true };
    let Some(ts) = c.type_system() else {
        return true;
    };
    let TypeKind::Function(invocation) = *ctx.ty(invocation_type) else {
        return true;
    };
    let TypeKind::Function(node) = *ctx.ty(node_type) else {
        return true;
    };
    let invocation_params: Vec<FnParam> = ctx.list(invocation.params).to_vec();
    let node_params: Vec<FnParam> = ctx.list(node.params).to_vec();
    let required_positional = |ps: &[FnParam]| {
        ps.iter()
            .filter(|p| p.kind.is_required_positional())
            .count()
    };
    if required_positional(&invocation_params) != required_positional(&node_params) {
        return false;
    }
    let optional_positional = |ps: &[FnParam]| {
        ps.iter()
            .filter(|p| p.kind.is_optional_positional())
            .copied()
            .collect::<Vec<_>>()
    };
    // The pairs are (node, invocation), as in the Dart record.
    for (invocation_param, node_param) in optional_positional(&node_params)
        .into_iter()
        .zip(optional_positional(&invocation_params))
    {
        if !ts.is_assignable_to(invocation_param.ty, node_param.ty, false) {
            return false;
        }
    }
    for parameter in invocation_params.iter().filter(|p| p.kind.is_named()) {
        let invocation_parameter = parameter
            .name
            .and_then(|name| node_params.iter().find(|p| p.name == Some(name)));
        let Some(invocation_parameter) = invocation_parameter else {
            if parameter.kind.is_required() {
                return false;
            }
            continue;
        };
        if !ts.is_assignable_to(parameter.ty, invocation_parameter.ty, false) {
            return false;
        }
        if parameter.kind.is_required() != invocation_parameter.kind.is_required() {
            return false;
        }
    }
    true
}

fn function_type(c: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let ctx = rctx(c)?;
    let element = c.declared_element(node)?;
    Some(dartr_typesystem::member::type_(
        &ctx,
        ElemRef::Base(element),
    ))
}

fn visit_function_expression(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<FunctionExpression>::from_raw(node)];
    let element_name = c
        .declared_element(node)
        .and_then(|e| name(c, e))
        .filter(|n| !n.is_empty());
    if element_name.is_some() || body_keyword(c, n.body.raw()).0.is_some() {
        return;
    }
    let body = n.body.raw();
    if let Some(block) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[block].block].statements);
        if statements.len() == 1 {
            let statement = statements[0];
            let expression = if let Some(s) = c.ast.cast::<ExpressionStatement>(statement) {
                Some(c.ast[s].expression.raw())
            } else if let Some(s) = c.ast.cast::<ReturnStatement>(statement) {
                c.ast[s].expression.map(|e| e.raw())
            } else {
                None
            };
            if let Some(expression) = expression
                && InvocationExpression::test(kind(c, expression))
            {
                visit_invocation_expression(c, expression, node, out);
            }
        }
    } else if let Some(body) = c.ast.cast::<ExpressionFunctionBody>(body) {
        let expression = c.ast[body].expression.raw();
        if InvocationExpression::test(kind(c, expression)) {
            visit_invocation_expression(c, expression, node, out);
        } else if c.is_feature_enabled(ExperimentalFlag::ConstructorTearoffs)
            && kind(c, expression) == NodeKind::InstanceCreationExpression
        {
            visit_instance_creation(c, expression, node, out);
        }
    }
}

/// Dart `_visitInstanceCreation`.
fn visit_instance_creation(
    c: &LinterContext<'_>,
    expression: NodeId,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = rctx(c) else { return };
    let e = &c.ast[Id::<InstanceCreationExpression>::from_raw(expression)];
    let is_const = match e.keyword {
        Some(k) => lexeme(c, k) == "const",
        None => in_constant_context(c, expression),
    };
    let named_type = c.ast[e.constructor_name].type_;
    let is_deferred = c.ast[named_type]
        .import_prefix
        .and_then(|p| c.element(p))
        .map(|e| base(c, e))
        .is_some_and(|e| {
            e.tag() == Tag::Prefix && super::prefer_const_constructors::prefix_is_deferred(c, e)
        });
    if is_const || is_deferred {
        return;
    }
    let Some(node_type) = function_type(c, node) else {
        return;
    };
    let Some(constructor) = c.element(e.constructor_name) else {
        return;
    };
    let invocation_type = dartr_typesystem::member::type_(&ctx, constructor);
    let Some(ts) = c.type_system() else { return };
    if !ts.is_assignable_to(invocation_type, node_type, false) {
        return;
    }
    let arguments = c.ast.list_raw(c.ast[e.argument_list].arguments);
    if arguments
        .iter()
        .any(|&a| kind(c, a) != NodeKind::SimpleIdentifier)
    {
        return;
    }
    let parameters = parameters(
        c,
        c.ast[Id::<FunctionExpression>::from_raw(node)].parameters,
    );
    if parameters.len() != arguments.len() {
        return;
    }
    for (&argument, &parameter) in arguments.iter().zip(&parameters) {
        if Some(simple_name(c, Id::from_raw(argument)))
            != super::prefer_iterable_wheretype::parameter_name(c, parameter)
        {
            return;
        }
    }
    c.report_node(out, &diag::UNNECESSARY_LAMBDAS, node, &[]);
}

/// Dart `_visitInvocationExpression`.
fn visit_invocation_expression(
    c: &LinterContext<'_>,
    node: NodeId,
    node_to_lint: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let Some(resolved) = c.resolved.as_ref() else {
        return;
    };
    let Some(parameter_list) = c.ast[Id::<FunctionExpression>::from_raw(node_to_lint)].parameters
    else {
        return;
    };
    let node_to_lint_params = parameters(c, Some(parameter_list));
    let argument_list = if let Some(m) = c.ast.cast::<MethodInvocation>(node) {
        c.ast[m].argument_list
    } else {
        c.ast[Id::<FunctionExpressionInvocation>::from_raw(node)].argument_list
    };
    let arguments = c.ast.list_raw(c.ast[argument_list].arguments).to_vec();
    if !arguments_match_parameters(c, &arguments, &node_to_lint_params) {
        return;
    }
    let Some(node_type) = function_type(c, node_to_lint) else {
        return;
    };
    let Some(&invocation_type) = resolved.tables.invoke_type.get(node) else {
        return;
    };
    let Some(ts) = c.type_system() else { return };
    if !ts.is_assignable_to(invocation_type, node_type, false)
        && !parameters_match(c, invocation_type, node_type)
    {
        return;
    }
    let parameters: IndexSet<Option<ElementId>> = node_to_lint_params
        .iter()
        .map(|&p| c.declared_element(p))
        .collect();
    if let Some(invocation) = c.ast.cast::<FunctionExpressionInvocation>(node) {
        let function = c.ast[invocation].function.raw();
        if might_be_deferred(c, Some(function)) {
            return;
        }
        if is_final_node(c, &parameters, Some(function)) {
            c.report_node(out, &diag::UNNECESSARY_LAMBDAS, node_to_lint, &[]);
        }
    } else if let Some(invocation) = c.ast.cast::<MethodInvocation>(node) {
        let m = &c.ast[invocation];
        let target = m.target.map(|t| t.raw());
        if might_be_deferred(c, target) {
            return;
        }
        let tearoff_type = invocation_type;
        if let Some(parent) = c.ast.parent(node_to_lint) {
            if let Some(named) = c.ast.cast::<NamedArgument>(parent) {
                let Some(arg_type) = c.static_type(c.ast[named].argument_expression) else {
                    return;
                };
                if !ts.is_subtype_of(tearoff_type, arg_type) {
                    return;
                }
            } else if kind(c, parent) == NodeKind::VariableDeclaration {
                let Some(ctx) = rctx(c) else { return };
                let Some(variable) = c.declared_element(parent) else {
                    return;
                };
                let variable_type = dartr_typesystem::member::type_(&ctx, ElemRef::Base(variable));
                if !ts.is_subtype_of(tearoff_type, variable_type) {
                    return;
                }
            }
        }
        if !contains_null_aware_invocation_in_chain(c, Some(node))
            && is_final_node(c, &parameters, target)
            && element_is_final(c, c.element(m.method_name).map(|e| base(c, e)))
            && m.type_arguments.is_none()
        {
            c.report_node(out, &diag::UNNECESSARY_LAMBDAS, node_to_lint, &[]);
        }
    }
}
