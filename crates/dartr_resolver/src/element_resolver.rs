// Dart source: pkg/analyzer/lib/src/generated/element_resolver.dart

//! `ElementResolver`: the elements of the nodes that are not expressions
//! (constructor names, `super(...)` / `this(...)` constructor invocations,
//! combinators of imports and exports, ...). The Dart methods that do
//! nothing are not ported; their call sites in the resolver are comments.
//!
//! Partly STUB (unit C2): `visitConstructorName`,
//! `visitSuperConstructorInvocation`, `visitRedirectingConstructorInvocation`,
//! `visitImportDirective`, `visitExportDirective` (combinators) and
//! `visitCommentReference` are ported with the units that own those nodes
//! (C8 constructors, C9 comment references).

use dartr_ast::{Argument, ArgumentList, Expression, NamedArgument, NodeId};
use dartr_ast::{
    ConstructorDeclaration, ConstructorName, DotShorthandConstructorInvocation, Id,
    InstanceCreationExpression, RedirectingConstructorInvocation, SuperConstructorInvocation,
};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::{element_arg, type_arg};
use dartr_element::{ElemRef, FormalParameterElement, FragmentFlags, TypeKind};
use dartr_typesystem::{TypeExt, lookup, member};
use indexmap::IndexMap;

use crate::resolver::ResolverVisitor;

/// Dart `ConstructorElement.isFactory` of [element].
pub fn is_factory(rv: &ResolverVisitor<'_>, element: ElemRef) -> bool {
    let base = member::base_element(&rv.ctx, element);
    crate::element_ext::first_fragment_flags(&rv.ctx, base)
        .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// Dart `ElementResolver.visitConstructorName(node)`.
pub fn visit_constructor_name(rv: &mut ResolverVisitor<'_>, node: Id<ConstructorName>) {
    let named_type = rv.ast[node].type_;
    let Some(&ty) = rv.tables.annotation_type.get(named_type) else {
        return;
    };
    if let TypeKind::Interface { .. } = rv.ctx.ty(ty) {
        // look up ConstructorElement
        let library = rv.unit.library;
        let constructor = match rv.ast[node].name {
            None => lookup::type_look_up_constructor(&rv.ctx, ty, None, library),
            Some(name) => {
                let lexeme = rv.lexeme(rv.ast[name].token).to_string();
                let constructor =
                    lookup::type_look_up_constructor(&rv.ctx, ty, Some(&lexeme), library);
                rv.set_element(name, constructor);
                constructor
            }
        };
        rv.set_element(node, constructor);
    }
}

/// Dart `ElementResolver.visitInstanceCreationExpression(node)`.
pub fn visit_instance_creation_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<InstanceCreationExpression>,
) {
    let constructor_name = rv.ast[node].constructor_name;
    let invoked_constructor = rv.element(constructor_name);
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, invoked_constructor, None);
}

/// Dart `ElementResolver.visitDotShorthandConstructorInvocation(node)`.
pub fn visit_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
) {
    let invoked_constructor = rv.element(node);
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, invoked_constructor, None);
}

/// Dart `ElementResolver.visitRedirectingConstructorInvocation(node)`.
pub fn visit_redirecting_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<RedirectingConstructorInvocation>,
) {
    let Some(enclosing_class) = rv.enclosing_class else {
        return;
    };
    let name = rv.ast[node].constructor_name;
    let lexeme = match name {
        None => "new".to_string(),
        Some(name) => rv.lexeme(rv.ast[name].token).to_string(),
    };
    let Some(element) = lookup::get_named_constructor(&rv.ctx, enclosing_class, &lexeme) else {
        return;
    };
    let element = ElemRef::Base(element.raw());
    if let Some(name) = name {
        rv.set_element(name, Some(element));
    }
    rv.set_element(node, Some(element));
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, Some(element), None);
}

/// Dart `ElementResolver.visitSuperConstructorInvocation(node)`.
pub fn visit_super_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SuperConstructorInvocation>,
) {
    let Some(enclosing_class) = rv.enclosing_class else {
        return;
    };
    let Some(super_type) = rv.ctx.interface(enclosing_class).supertype.get() else {
        return;
    };
    if !matches!(rv.ctx.ty(super_type), TypeKind::Interface { .. }) {
        return;
    }
    let library = rv.unit.library;
    let name = rv.ast[node].constructor_name;
    let super_name = name.map(|n| rv.lexeme(rv.ast[n].token).to_string());
    let element =
        lookup::type_look_up_constructor(&rv.ctx, super_type, super_name.as_deref(), library)
            .filter(|&e| member::is_accessible_in(&rv.ctx, e, library));
    let Some(element) = element else {
        let d = match &super_name {
            Some(super_name) => diag::undefined_constructor_in_initializer(
                type_arg(&rv.ctx, super_type),
                super_name,
            ),
            None => {
                let class_name = rv
                    .ctx
                    .interface_element(super_type)
                    .and_then(|e| rv.ctx.element_name(e.raw()))
                    .unwrap_or("<unknown>")
                    .to_string();
                diag::undefined_constructor_in_initializer_default(&class_name)
            }
        };
        let d = rv.at(d, node);
        rv.report(d);
        return;
    };
    if is_factory(rv, element) {
        // Check if we've reported [NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS].
        let all_factories = member::enclosing_interface(&rv.ctx, element).is_none_or(|class| {
            rv.ctx
                .interface(class)
                .constructors
                .iter()
                .all(|&c| is_factory(rv, ElemRef::Base(c.raw())))
        });
        if !all_factories {
            let base = member::base_element(&rv.ctx, element);
            let d = rv.at(
                diag::non_generative_constructor(element_arg(&rv.ctx, base)),
                node,
            );
            rv.report(d);
        }
    }
    if let Some(name) = name {
        rv.set_element(name, Some(element));
    }
    rv.set_element(node, Some(element));
    // Dart: if the extended type is an undefined name that the library
    // fragment ignores (`shouldIgnoreUndefinedNamedType`), the arguments are
    // not resolved to parameters. Not ported (it needs the ignored
    // undefined names of the library fragment).
    let argument_list = rv.ast[node].argument_list;
    let enclosing_list = rv
        .ast
        .parent(node)
        .and_then(|p| rv.ast.cast::<ConstructorDeclaration>(p))
        .map(|c| rv.ast[c].parameters);
    resolve_arguments_to_function(rv, argument_list, Some(element), enclosing_list);
}

/// Dart `ElementResolver._resolveArgumentsToFunction`.
fn resolve_arguments_to_function(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<dartr_ast::ArgumentList>,
    executable_element: Option<ElemRef>,
    enclosing_constructor_formal_parameter_list: Option<Id<dartr_ast::FormalParameterList>>,
) {
    let Some(executable_element) = executable_element else {
        return;
    };
    let parameters = member::formal_parameters(&rv.ctx, executable_element);
    resolve_arguments_to_parameters(
        rv,
        argument_list,
        &parameters,
        true,
        enclosing_constructor_formal_parameter_list,
    );
}

// ------------------------------------------------------------ arguments to parameters
//
// Dart `ResolverVisitor.resolveArgumentsToParameters` with the
// `enclosingConstructorFormalParameterList` of super constructor invocations
// (`invocation_inferrer::resolve_arguments_to_parameters` does not take it),
// and `_reportNotEnoughPositionalArguments` for enum constants.

/// The expression of [argument] (Dart `argument.argumentExpression`).
fn argument_expression(rv: &ResolverVisitor<'_>, argument: Id<Argument>) -> Id<Expression> {
    match rv.ast.cast::<NamedArgument>(argument) {
        Some(named) => rv.ast[named].argument_expression,
        None => rv
            .ast
            .cast::<Expression>(argument)
            .expect("an expression argument"),
    }
}

/// Dart `ResolverVisitor.resolveArgumentsToParameters(argumentList:,
/// formalParameters:, diagnosticReporter:,
/// enclosingConstructorFormalParameterList:)`: records the parameter of
/// each argument (Dart `correspondingStaticParameters`, here
/// `ResolutionTables::param_element` keyed by the argument expression) and
/// reports the argument count and name errors when [report] is set.
///
/// The super formal parameters of
/// [enclosing_constructor_formal_parameter_list] (Dart
/// `verifySuperFormalParameters`) count as arguments.
pub fn resolve_arguments_to_parameters(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    formal_parameters: &[ElemRef],
    report: bool,
    enclosing_constructor_formal_parameter_list: Option<Id<dartr_ast::FormalParameterList>>,
) {
    let ctx = rv.ctx;
    let mut required_parameter_count = 0;
    let mut unnamed_parameter_count = 0;
    let mut unnamed_parameters: Vec<ElemRef> = Vec::new();
    let mut named_parameters: Option<IndexMap<String, ElemRef>> = None;
    for &parameter in formal_parameters {
        let base = member::base_element(&ctx, parameter);
        let Some(base) = base.cast::<FormalParameterElement>() else {
            continue;
        };
        let data = ctx.get(base);
        if data.kind.is_required_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
            required_parameter_count += 1;
        } else if data.kind.is_optional_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
        } else {
            let name = ctx.element_name(base.raw()).unwrap_or("").to_string();
            named_parameters
                .get_or_insert_with(IndexMap::new)
                .insert(name, parameter);
        }
    }
    let mut unnamed_index = 0;
    let arguments = rv.ast.list(rv.ast[argument_list].arguments).to_vec();
    let mut resolved_parameters: Vec<Option<ElemRef>> = vec![None; arguments.len()];
    let mut positional_argument_count = 0;
    let mut no_blank_arguments = true;
    let mut first_unresolved_argument: Option<Id<Expression>> = None;
    let mut last_positional_argument: Option<Id<Expression>> = None;
    for (i, &argument) in arguments.iter().enumerate() {
        if rv.ast.is::<NamedArgument>(argument) {
            continue;
        }
        let expression = argument_expression(rv, argument);
        if let Some(identifier) = rv.ast.cast::<dartr_ast::SimpleIdentifier>(expression)
            && rv.lexeme(rv.ast[identifier].token).is_empty()
        {
            no_blank_arguments = false;
        }
        positional_argument_count += 1;
        if unnamed_index < unnamed_parameter_count {
            resolved_parameters[i] = Some(unnamed_parameters[unnamed_index]);
            unnamed_index += 1;
        } else if first_unresolved_argument.is_none() {
            first_unresolved_argument = Some(expression);
        }
        last_positional_argument = Some(expression);
    }

    let mut used_names: Option<indexmap::IndexSet<String>> = None;
    if let Some(list) = enclosing_constructor_formal_parameter_list {
        let result =
            crate::error::super_formal_parameters_verifier::verify_super_formal_parameters(
                rv,
                list,
                report,
                positional_argument_count != 0,
            );
        positional_argument_count += result.positional_argument_count;
        if !result.named_argument_names.is_empty() {
            used_names = Some(result.named_argument_names.into_iter().collect());
        }
    }

    for (i, &argument) in arguments.iter().enumerate() {
        let Some(named) = rv.ast.cast::<NamedArgument>(argument) else {
            continue;
        };
        let name_token = rv.ast[named].name;
        let name = rv.lexeme(name_token).to_string();
        let mut element = named_parameters
            .as_ref()
            .and_then(|m| m.get(&name).copied());
        if element.is_none() {
            element = if name.starts_with('_') && name.len() > 1 {
                named_parameters
                    .as_ref()
                    .and_then(|m| m.get(&name[1..]).copied())
            } else {
                None
            };
            if report {
                let d = match element {
                    None => diag::undefined_named_parameter(&name),
                    Some(_) => diag::use_of_private_parameter_name(&name[1..]),
                };
                let d = rv.at_token(d, name_token);
                rv.report(d);
            }
        } else {
            resolved_parameters[i] = element;
        }
        if !used_names
            .get_or_insert_with(Default::default)
            .insert(name.clone())
            && report
        {
            let d = rv.at_token(diag::duplicate_named_argument(&name), name_token);
            rv.report(d);
        }
    }

    if report {
        if positional_argument_count < required_parameter_count && no_blank_arguments {
            let token = match last_positional_argument {
                Some(e) => rv.ast.tokens.next(rv.ast.end_token(e)),
                None => rv.ast.tokens.next(rv.ast[argument_list].left_parenthesis),
            };
            let parent = rv.ast.parent(argument_list);
            report_not_enough_positional_arguments(
                rv,
                token,
                required_parameter_count,
                positional_argument_count,
                parent,
            );
        } else if positional_argument_count > unnamed_parameter_count && no_blank_arguments {
            let named_parameter_count = named_parameters.as_ref().map_or(0, |m| m.len());
            let named_argument_count = used_names.as_ref().map_or(0, |s| s.len());
            if let Some(first) = first_unresolved_argument {
                let d = if named_parameter_count > named_argument_count {
                    diag::extra_positional_arguments_could_be_named(
                        unnamed_parameter_count as i64,
                        positional_argument_count as i64,
                    )
                } else {
                    diag::extra_positional_arguments(
                        unnamed_parameter_count as i64,
                        positional_argument_count as i64,
                    )
                };
                let d = rv.at(d, first);
                rv.report(d);
            }
        }
    }

    for (i, &argument) in arguments.iter().enumerate() {
        // Keyed by the argument expression (the expression of a named
        // argument; Dart `Expression.correspondingParameter`).
        let expression = argument_expression(rv, argument);
        match resolved_parameters[i] {
            Some(p) => {
                rv.tables.param_element.insert(expression, p);
            }
            None => {
                rv.tables.param_element.remove(expression);
            }
        }
    }
}

/// Dart `ResolverVisitor._reportNotEnoughPositionalArguments`.
pub fn report_not_enough_positional_arguments(
    rv: &mut ResolverVisitor<'_>,
    token: dartr_syntax::TokenId,
    required_parameter_count: usize,
    actual_argument_count: usize,
    name_node: Option<NodeId>,
) {
    let name = name_node.and_then(|n| invoked_name(rv, n));
    let d = if required_parameter_count == 1 {
        match name {
            Some(name) => diag::not_enough_positional_arguments_name_singular(&name),
            None => diag::not_enough_positional_arguments_singular(),
        }
    } else {
        match name {
            Some(name) => diag::not_enough_positional_arguments_name_plural(
                required_parameter_count as i64,
                actual_argument_count as i64,
                &name,
            ),
            None => diag::not_enough_positional_arguments_plural(
                required_parameter_count as i64,
                actual_argument_count as i64,
            ),
        }
    };
    let d = rv.at_token(d, token);
    rv.report(d);
}

/// The name that `_reportNotEnoughPositionalArguments` puts into the
/// message (Dart: the switch over `nameNode`).
fn invoked_name(rv: &ResolverVisitor<'_>, node: NodeId) -> Option<String> {
    use dartr_ast::{
        DotShorthandConstructorInvocation, DotShorthandInvocation, EnumConstantArguments,
        EnumConstantDeclaration, InstanceCreationExpression, MethodInvocation,
        RedirectingConstructorInvocation, SuperConstructorInvocation,
    };
    let ast = &*rv.ast;
    let ctx = rv.ctx;
    // `'${element.returnType.getDisplayString()}.new'`.
    let constructor_new = |element: Option<ElemRef>| {
        element.map(|e| {
            let ty = member::return_type(&ctx, e);
            format!(
                "{}.new",
                dartr_element::diagnostics::type_display_string(&ctx, ty, true)
            )
        })
    };
    if let Some(n) = ast.cast::<InstanceCreationExpression>(node) {
        let constructor_name = ast[n].constructor_name;
        return Some(match ast[constructor_name].name {
            Some(id) => rv.lexeme(ast[id].token).to_string(),
            None => format!("{}.new", rv.lexeme(ast[ast[constructor_name].type_].name)),
        });
    }
    if let Some(n) = ast.cast::<RedirectingConstructorInvocation>(node) {
        return match ast[n].constructor_name {
            Some(id) => Some(rv.lexeme(ast[id].token).to_string()),
            None => constructor_new(rv.element(n)),
        };
    }
    if let Some(n) = ast.cast::<SuperConstructorInvocation>(node) {
        return match ast[n].constructor_name {
            Some(id) => Some(rv.lexeme(ast[id].token).to_string()),
            None => constructor_new(rv.element(n)),
        };
    }
    if let Some(n) = ast.cast::<MethodInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].method_name].token).to_string());
    }
    if let Some(n) = ast.cast::<EnumConstantArguments>(node) {
        return ast
            .parent(n)
            .filter(|&p| ast.is::<EnumConstantDeclaration>(p))
            .and_then(|p| enum_constant_type_name(rv, p));
    }
    if ast.is::<EnumConstantDeclaration>(node) {
        return enum_constant_type_name(rv, node);
    }
    if let Some(n) = ast.cast::<DotShorthandConstructorInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].constructor_name].token).to_string());
    }
    if let Some(n) = ast.cast::<DotShorthandInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].member_name].token).to_string());
    }
    None
}

/// Dart `declaredFragment!.element.type.getDisplayString()` of an enum
/// constant declaration.
fn enum_constant_type_name(rv: &ResolverVisitor<'_>, node: NodeId) -> Option<String> {
    let fragment = *rv.tables.declared_fragment.get(node)?;
    let element = *rv.ctx.fragment_data(fragment)?.element.try_get()?;
    let ty = crate::element_ext::variable_type(&rv.ctx, element);
    Some(dartr_element::diagnostics::type_display_string(
        &rv.ctx, ty, true,
    ))
}
