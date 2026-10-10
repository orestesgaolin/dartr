// Dart source: pkg/linter/lib/src/rules/use_super_parameters.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, ElementId, FormalParameterElement, Tag};
use indexmap::IndexSet;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::SuperParameters) {
        return;
    }
    r.add(NodeKind::ConstructorDeclaration, "use_super_parameters", visit_constructor_declaration);
    r.add(NodeKind::PrimaryConstructorDeclaration, "use_super_parameters", visit_primary_constructor_declaration);
}

fn is_formal_parameter(element: ElementId) -> bool {
    matches!(element.tag(), Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter)
}

fn is_named(c: &LinterContext<'_>, parameter: ElemRef) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let base = dartr_typesystem::member::base_element(&ctx, parameter);
    ctx.get(EId::<FormalParameterElement>::from_raw(base)).kind.is_named()
}

/// Dart `_referencedParameters`.
fn referenced_parameters(c: &LinterContext<'_>, body: Option<NodeId>) -> IndexSet<ElemRef> {
    let mut found = IndexSet::new();
    let mut stack: Vec<NodeId> = body.into_iter().collect();
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::SimpleIdentifier
            && let Some(element) = c.element(n)
            && is_formal_parameter(base(c, element))
        {
            found.insert(element);
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
    found
}

fn visit_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
    for &initializer in c.ast.list_raw(n.initializers).iter().rev() {
        if let Some(invocation) = c.ast.cast::<SuperConstructorInvocation>(initializer) {
            // Dart `ConstructorDeclaration.errorRange`.
            let start = n.type_name.map_or_else(
                || {
                    let t = c.ast.tokens.get(n.new_keyword.or(n.factory_keyword).unwrap());
                    (t.offset, t.offset + t.length)
                },
                |t| (c.ast.offset(t), c.ast.end(t)),
            );
            let end = n.name.map_or(start.1, |t| {
                let t = c.ast.tokens.get(t);
                t.offset + t.length
            });
            check(c, (start.0, end - start.0), invocation, n.parameters, Some(n.body.raw()), out);
            return;
        }
    }
}

fn visit_primary_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(body) = super::tighten_type_of_initializing_formals::primary_constructor_body(c, node) else {
        return;
    };
    let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
    for &initializer in c.ast.list_raw(c.ast[body].initializers).iter().rev() {
        if let Some(invocation) = c.ast.cast::<SuperConstructorInvocation>(initializer) {
            // Dart `PrimaryConstructorDeclaration.errorRange`.
            let start = c.ast.offset(node);
            let end = n.constructor_name.map_or_else(
                || {
                    let t = c.ast.tokens.get(c.ast.begin_token(node));
                    t.offset + t.length
                },
                |cn| c.ast.end(cn),
            );
            check(c, (start, end - start), invocation, n.formal_parameters, Some(c.ast[body].body.raw()), out);
            return;
        }
    }
}

fn check(
    c: &LinterContext<'_>,
    error_range: (u32, u32),
    super_invocation: Id<SuperConstructorInvocation>,
    parameter_list: Id<FormalParameterList>,
    body: Option<NodeId>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(constructor) = c.element(super_invocation) else { return };
    let referenced = referenced_parameters(c, body);
    let parameters = parameters(c, Some(parameter_list));
    let Some(mut identifiers) =
        check_for_convertible_positional_params(c, super_invocation, &parameters, &referenced)
    else {
        return;
    };
    for &parameter in &parameters {
        let Some(element) = c.declared_element(parameter) else { continue };
        if element.tag() == Tag::FieldFormalParameter {
            continue;
        }
        if is_named(c, ElemRef::Base(element))
            && !referenced.contains(&ElemRef::Base(element))
            && check_named_parameter(c, element, constructor, super_invocation)
            && let Some(name) = super::prefer_iterable_wheretype::parameter_name(c, parameter)
        {
            identifiers.push(name.to_string());
        }
    }
    let (offset, length) = (error_range.0 as usize, error_range.1 as usize);
    match identifiers.len() {
        0 => {}
        1 => c.report_offset(out, &diag::USE_SUPER_PARAMETERS_SINGLE, offset, length, &[&identifiers[0]]),
        n => {
            let quoted: Vec<String> = identifiers.iter().map(|s| format!("'{s}'")).collect();
            let message = if n == 2 {
                format!("{} and {}", quoted[0], quoted[1])
            } else {
                format!("{}, and {}", quoted[..n - 1].join(", "), quoted[n - 1])
            };
            c.report_offset(out, &diag::USE_SUPER_PARAMETERS_MULTIPLE, offset, length, &[&message]);
        }
    }
}

/// Dart `_checkForConvertiblePositionalParams`.
fn check_for_convertible_positional_params(
    c: &LinterContext<'_>,
    super_invocation: Id<SuperConstructorInvocation>,
    parameters: &[NodeId],
    referenced: &IndexSet<ElemRef>,
) -> Option<Vec<String>> {
    let mut positional_super_args = Vec::new();
    let argument_list = c.ast[super_invocation].argument_list;
    for &argument in c.ast.list_raw(c.ast[argument_list].arguments) {
        if kind(c, argument) == NodeKind::SimpleIdentifier {
            positional_super_args.push(argument);
        } else if kind(c, argument) != NodeKind::NamedArgument {
            return Some(Vec::new());
        }
    }
    if positional_super_args.is_empty() {
        return Some(Vec::new());
    }
    let mut convertible = Vec::new();
    let mut matched_param_index = 0;
    let mut seen = IndexSet::new();
    for super_arg in positional_super_args {
        let param_passed_to_super = c.element(super_arg)?;
        if !is_formal_parameter(base(c, param_passed_to_super)) {
            return None;
        }
        if is_named(c, param_passed_to_super) {
            return None;
        }
        if !seen.insert(param_passed_to_super) {
            return None;
        }
        let mut matched = false;
        for (j, &parameter) in parameters.iter().enumerate() {
            if matched {
                break;
            }
            match kind(c, parameter) {
                NodeKind::FieldFormalParameter => return Some(Vec::new()),
                NodeKind::SuperFormalParameter => return None,
                _ => {}
            }
            let element = ElemRef::Base(c.declared_element(parameter)?);
            if referenced.contains(&element) {
                return Some(Vec::new());
            }
            if element == param_passed_to_super {
                matched = true;
                let identifier = super::prefer_iterable_wheretype::parameter_name(c, parameter)?;
                convertible.push(identifier.to_string());
                if j < matched_param_index {
                    return Some(Vec::new());
                }
                matched_param_index = j;
            }
        }
    }
    Some(convertible)
}

/// Dart `_checkNamedParameter`.
fn check_named_parameter(
    c: &LinterContext<'_>,
    parameter: ElementId,
    super_constructor: ElemRef,
    super_invocation: Id<SuperConstructorInvocation>,
) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let parameter_name = name(c, parameter);
    let super_parameter = dartr_typesystem::member::formal_parameters(&ctx, super_constructor)
        .into_iter()
        .find(|&p| is_named(c, p) && dartr_typesystem::member::name(&ctx, p) == parameter_name);
    let Some(super_parameter) = super_parameter else { return false };
    let argument_list = c.ast[super_invocation].argument_list;
    let matching_argument = c.ast.list_raw(c.ast[argument_list].arguments).iter().any(|&argument| {
        c.ast.cast::<NamedArgument>(argument).is_some_and(|named| {
            Some(lexeme(c, c.ast[named].name)) == parameter_name && {
                let expression = c.ast[named].argument_expression.raw();
                kind(c, expression) == NodeKind::SimpleIdentifier
                    && c.element(expression) == Some(ElemRef::Base(parameter))
            }
        })
    });
    if !matching_argument {
        return false;
    }
    let Some(ts) = c.type_system() else { return false };
    let super_type = dartr_typesystem::member::type_(&ctx, super_parameter);
    let this_type = dartr_typesystem::member::type_(&ctx, ElemRef::Base(parameter));
    ts.is_assignable_to(super_type, this_type, false)
}
