// Dart source: pkg/analyzer/lib/src/error/type_arguments_verifier.dart

//! `TypeArgumentsVerifier`: the type arguments of named types, invocations,
//! constructor references, enum constants and collection literals (bounds,
//! counts, type parameters in constant literals, raw types). The error
//! verifier calls the `check_*` functions on single nodes (Dart
//! `_typeArgumentsVerifier.checkX(node)`).

use dartr_ast::{
    ConstructorName, ConstructorReference, EnumConstantDeclaration, FunctionExpressionInvocation,
    FunctionReference, GenericFunctionType, Id, InstanceCreationExpression, ListLiteral,
    MethodInvocation, NamedType, NodeId, NodeKind, RecordTypeAnnotation,
    RecordTypeAnnotationNamedField, RecordTypeAnnotationPositionalField, RegularFormalParameter,
    SetOrMapLiteral, TypeAnnotation, TypeArgumentList,
};
use dartr_diagnostics::{DiagnosticMessage, LocatableDiagnostic, diag};
use dartr_element::diagnostics::{type_arg, type_display_string};
use dartr_element::{
    Ctx, EId, ElemRef, Tag, TypeAliasElement, TypeId, TypeKind, TypeParameterElement,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::{TypeExt, member};

use super::VerifierHost;
use crate::ast_ext;
use crate::scope::library_feature_enabled;

/// Dart `ExpectedTypeArgumentsDiagnosticCode`.
type ExpectedTypeArgumentsCode = fn(i64) -> LocatableDiagnostic;

/// Dart `InvalidTypeArgumentDiagnosticCode`.
type InvalidTypeArgumentCode = fn(&str) -> LocatableDiagnostic;

/// Dart `TypeArgumentsVerifier.checkConstructorReference(node)`.
pub fn check_constructor_reference<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<ConstructorReference>,
) {
    let ctx = host.ctx();
    let ast = host.ast();
    let named_type = ast[ast[node].constructor_name].type_;
    let class_element = host
        .element(named_type)
        .map(|e| member::base_element(&ctx, e));
    let type_parameters: Vec<EId<TypeParameterElement>> = match class_element {
        Some(e) if e.tag() == Tag::TypeAlias => ctx
            .get(EId::<TypeAliasElement>::from_raw(e))
            .type_params
            .clone(),
        Some(e) if is_interface_tag(e.tag()) => {
            ctx.interface_type_parameters(EId::from_raw(e)).to_vec()
        }
        _ => return,
    };

    if type_parameters.is_empty() {
        return;
    }

    for &type_parameter in &type_parameters {
        if ctx.element_name(type_parameter.raw()).is_none() {
            return;
        }
    }

    let ast = host.ast();
    let Some(type_argument_list) = ast[named_type].type_arguments else {
        return;
    };
    let constructor_type = host.static_type(node);
    match constructor_type.map(|t| ctx.ty(t)) {
        // An erroneous constructor reference.
        Some(TypeKind::Dynamic) => return,
        Some(TypeKind::Function(_)) => {}
        _ => return,
    }
    let type_argument_nodes = type_argument_nodes(host, type_argument_list);
    let type_arguments: Vec<TypeId> = type_argument_nodes
        .iter()
        .map(|&t| type_or_throw(host, t))
        .collect();
    if type_arguments.len() != type_parameters.len() {
        // Wrong number of type arguments to be reported elsewhere.
        return;
    }
    let type_argument_list_length = type_argument_nodes.len();
    let substitution = MapSubstitution::from_pairs(&type_parameters, &type_arguments);
    for i in 0..type_arguments.len() {
        let type_parameter = type_parameters[i];
        let type_argument = type_arguments[i];

        let Some(bound) = ctx.type_parameter_bound(type_parameter) else {
            continue;
        };

        let bound = substitution.substitute_type(&ctx, bound);

        if !host.type_system().is_subtype_of(type_argument, bound) {
            let error_node: NodeId = if i < type_argument_list_length {
                type_argument_nodes[i].raw()
            } else {
                node.raw()
            };
            let d = diag::type_argument_not_matching_bounds(
                type_arg(&ctx, type_argument),
                ctx.element_name(type_parameter.raw()).unwrap_or(""),
                type_arg(&ctx, bound),
            );
            let d = host.at(d, error_node);
            host.report(d);
        }
    }
}

/// Dart `TypeArgumentsVerifier.checkEnumConstantDeclaration(node)`.
pub fn check_enum_constant_declaration<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<EnumConstantDeclaration>,
) {
    let ctx = host.ctx();
    // Dart `node.constructorElement`.
    let Some(constructor_element) = host.element(node) else {
        return;
    };

    // Dart `constructorElement.enclosingElement.typeParameters`.
    let type_parameters: Vec<EId<TypeParameterElement>> =
        match member::enclosing_element(&ctx, constructor_element) {
            Some(e) if is_interface_tag(e.tag()) => {
                ctx.interface_type_parameters(EId::from_raw(e)).to_vec()
            }
            _ => Vec::new(),
        };

    for &type_parameter in &type_parameters {
        if ctx.element_name(type_parameter.raw()).is_none() {
            return;
        }
    }

    let ast = host.ast();
    let type_argument_list = ast[node].arguments.and_then(|a| ast[a].type_arguments);
    let type_argument_nodes = type_argument_list.map(|l| type_argument_nodes(host, l));
    if let (Some(type_argument_list), Some(type_argument_nodes)) =
        (type_argument_list, &type_argument_nodes)
        && type_argument_nodes.len() != type_parameters.len()
    {
        let d = diag::wrong_number_of_type_arguments_enum(
            type_parameters.len() as i64,
            type_argument_nodes.len() as i64,
        );
        let d = host.at(d, type_argument_list);
        host.report(d);
    }

    if type_parameters.is_empty() {
        return;
    }

    // Check that type arguments are regular-bounded.
    let return_type = member::return_type(&ctx, constructor_element);
    let type_arguments = interface_type_arguments(&ctx, return_type);
    let substitution = from_pairs(&type_parameters, &type_arguments);
    for i in 0..type_arguments.len().min(type_parameters.len()) {
        let type_parameter = type_parameters[i];
        let type_argument = type_arguments[i];

        let Some(bound) = ctx.type_parameter_bound(type_parameter) else {
            continue;
        };

        let bound = substitution.substitute_type(&ctx, bound);

        if !host.type_system().is_subtype_of(type_argument, bound) {
            let d = diag::type_argument_not_matching_bounds(
                type_arg(&ctx, type_argument),
                ctx.element_name(type_parameter.raw()).unwrap_or(""),
                type_arg(&ctx, bound),
            );
            // Dart `typeArgumentNodes?[i] ?? node.name`.
            let d = match type_argument_nodes.as_ref().and_then(|n| n.get(i)) {
                Some(&n) => host.at(d, n),
                None => {
                    let name = host.ast()[node].name;
                    host.at_token(d, name)
                }
            };
            host.report(d);
        }
    }
}

/// Dart `TypeArgumentsVerifier.checkFunctionExpressionInvocation(node)`.
pub fn check_function_expression_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<FunctionExpressionInvocation>,
) {
    let ctx = host.ctx();
    // For some function expressions, like an implicit 'call' invocation,
    // the function type is on `node`'s `element`. For anonymous function
    // expressions, the function is on `node`'s `function`.
    let function = host.ast()[node].function;
    let function_type = match host.element(node) {
        Some(e) => Some(member::type_(&ctx, e)),
        None => host.static_type(function),
    };
    let type_arguments = host.ast()[node].type_arguments;
    let invoke_type = host.tables().invoke_type.get(node).copied();
    check_invocation_type_arguments(host, type_arguments, function_type, invoke_type);
}

/// Dart `TypeArgumentsVerifier.checkFunctionReference(node)`.
pub fn check_function_reference<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<FunctionReference>,
) {
    let ast = host.ast();
    let function = ast[node].function;
    let type_arguments = ast[node].type_arguments;
    let generic_type = host.static_type(function);
    let instantiated_type = host.static_type(node);
    check_invocation_type_arguments(host, type_arguments, generic_type, instantiated_type);
}

/// Dart `TypeArgumentsVerifier.checkListLiteral(node)`.
pub fn check_list_literal<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<ListLiteral>) {
    let ast = host.ast();
    let Some(type_arguments) = ast[node].type_arguments else {
        return;
    };
    if ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw()) {
        for argument in type_argument_nodes(host, type_arguments) {
            check_type_argument_const(host, argument, diag::invalid_type_argument_in_const_list);
        }
    }
    check_type_argument_count(
        host,
        type_arguments,
        1,
        diag::expected_one_list_type_arguments,
    );
}

/// Dart `TypeArgumentsVerifier.checkMapLiteral(node)`.
pub fn check_map_literal<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<SetOrMapLiteral>) {
    let ast = host.ast();
    let Some(type_arguments) = ast[node].type_arguments else {
        return;
    };
    if is_const_set_or_map(host, node) {
        for argument in type_argument_nodes(host, type_arguments) {
            check_type_argument_const(host, argument, diag::invalid_type_argument_in_const_map);
        }
    }
    check_type_argument_count(
        host,
        type_arguments,
        2,
        diag::expected_two_map_type_arguments,
    );
}

/// Dart `TypeArgumentsVerifier.checkMethodInvocation(node)`.
pub fn check_method_invocation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<MethodInvocation>) {
    let ast = host.ast();
    // Dart `node.function` is the method name.
    let method_name = ast[node].method_name;
    let type_arguments = ast[node].type_arguments;
    let generic_type = host.static_type(method_name);
    let invoke_type = host.tables().invoke_type.get(node).copied();
    check_invocation_type_arguments(host, type_arguments, generic_type, invoke_type);
}

/// Dart `TypeArgumentsVerifier.checkNamedType(node)`.
pub fn check_named_type<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<NamedType>) {
    check_for_type_argument_not_matching_bounds(host, node);
    let ast = host.ast();
    let parent = ast.parent(node);
    let in_instance_creation = parent.is_some_and(|p| {
        ast.is::<ConstructorName>(p)
            && ast
                .parent(p)
                .is_some_and(|gp| ast.is::<InstanceCreationExpression>(gp))
    });
    if !in_instance_creation {
        check_for_raw_type_name(host, node);
    }
}

/// Dart `TypeArgumentsVerifier.checkSetLiteral(node)`.
pub fn check_set_literal<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<SetOrMapLiteral>) {
    let ast = host.ast();
    let Some(type_arguments) = ast[node].type_arguments else {
        return;
    };
    if is_const_set_or_map(host, node) {
        for argument in type_argument_nodes(host, type_arguments) {
            check_type_argument_const(host, argument, diag::invalid_type_argument_in_const_set);
        }
    }
    check_type_argument_count(
        host,
        type_arguments,
        1,
        diag::expected_one_set_type_arguments,
    );
}

/// Dart `TypedLiteral.isConst` of a set or map literal.
fn is_const_set_or_map<'a, H: VerifierHost<'a>>(host: &H, node: Id<SetOrMapLiteral>) -> bool {
    let ast = host.ast();
    ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw())
}

/// Dart `_checkForRawTypeName(node)`: checks a type annotation for a raw
/// generic type, and reports `strictRawType` if
/// `AnalysisOptions.strictRawTypes` is set.
fn check_for_raw_type_name<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<NamedType>) {
    if !host.options().strict_raw_types {
        return;
    }
    let ast = host.ast();
    if ast[node].type_arguments.is_some() {
        // Type has explicit type arguments.
        return;
    }
    let ty = type_or_throw(host, node.upcast());
    let element = host.element(node);
    if is_missing_type_arguments(host, ty, element) {
        let ast = host.ast();
        let unwrapped_parent = parent_escaping_type_arguments(ast, node);
        if matches!(
            unwrapped_parent.map(|p| ast.kind(p)),
            Some(
                NodeKind::AsExpression
                    | NodeKind::CastPattern
                    | NodeKind::IsExpression
                    | NodeKind::ObjectPattern
                    | NodeKind::TypeLiteral
            )
        ) {
            // Do not report a "Strict raw type" warning in this case; too
            // noisy. See https://github.com/dart-lang/language/blob/master/resources/type-system/strict-raw-types.md#conditions-for-a-raw-type-hint
        } else {
            let ctx = host.ctx();
            let d = host.at(diag::strict_raw_type(type_arg(&ctx, ty)), node);
            host.report(d);
        }
    }
}

/// Dart `parentEscapingTypeArguments(node)` (local function of
/// `_checkForRawTypeName`).
fn parent_escaping_type_arguments(ast: &dartr_ast::Ast, node: Id<NamedType>) -> Option<NodeId> {
    let mut parent = ast.parent(node)?;
    while matches!(
        ast.kind(parent),
        NodeKind::TypeArgumentList | NodeKind::NamedType
    ) {
        match ast.parent(parent) {
            Some(grandparent) => parent = grandparent,
            None => return Some(parent),
        }
    }
    Some(parent)
}

/// One violated bound (Dart `_TypeArgumentIssue`).
struct TypeArgumentIssue {
    /// The index for type argument within the passed type arguments.
    index: usize,
    /// The non-null name of the type parameter.
    parameter_name: String,
    /// The substituted bound of the type parameter.
    parameter_bound: TypeId,
    /// The type argument that violated the [parameter_bound].
    argument: TypeId,
}

/// Dart `_checkForTypeArgumentNotMatchingBounds(namedType)`: verifies that
/// the type arguments in the given [named_type] are all within their
/// bounds.
fn check_for_type_argument_not_matching_bounds<'a, H: VerifierHost<'a>>(
    host: &mut H,
    named_type: Id<NamedType>,
) {
    let ctx = host.ctx();
    let Some(ty) = host.tables().annotation_type.get(named_type).copied() else {
        return;
    };

    let Some((element_name, type_parameters, type_arguments)) =
        type_parameters_and_arguments(&ctx, ty)
    else {
        return;
    };

    let Some(element_name) = element_name else {
        return;
    };

    if type_parameters.is_empty() {
        return;
    }

    // Check for regular-bounded.
    let mut issues: Option<Vec<TypeArgumentIssue>> = None;
    let substitution = from_pairs(&type_parameters, &type_arguments);
    for (i, &type_argument) in type_arguments.iter().enumerate() {
        let Some(&type_parameter) = type_parameters.get(i) else {
            break;
        };
        let Some(type_parameter_name) = ctx.element_name(type_parameter.raw()) else {
            return;
        };

        if let TypeKind::Function(f) = ctx.ty(type_argument)
            && !ctx.list(f.type_params).is_empty()
            && !library_feature_enabled(&ctx, host.library(), ExperimentalFlag::GenericMetadata)
        {
            let error_node = type_argument_error_node(host.ast(), named_type, i);
            let d = host.at(
                diag::generic_function_type_cannot_be_type_argument(),
                error_node,
            );
            host.report(d);
            continue;
        }

        let Some(bound) = ctx.type_parameter_bound(type_parameter) else {
            continue;
        };

        let bound = substitution.substitute_type(&ctx, bound);

        if !host.type_system().is_subtype_of(type_argument, bound) {
            issues.get_or_insert_with(Vec::new).push(TypeArgumentIssue {
                index: i,
                parameter_name: type_parameter_name.to_string(),
                parameter_bound: bound,
                argument: type_argument,
            });
        }
    }

    // If regular-bounded, we are done.
    let Some(issues) = issues else {
        return;
    };

    let has_type_arguments = host.ast()[named_type].type_arguments.is_some();
    let file_path = ctx.fragment(host.fragment()).source.path.to_string();
    let (offset, length) = {
        let ast = host.ast();
        (ast.offset(named_type) as i64, ast.length(named_type) as i64)
    };
    // Dart `buildContextMessages(invertedTypeArguments:)`.
    let build_context_messages = |inverted_type_arguments: Option<&[TypeId]>| {
        let type_arguments_to_string = |type_arguments: &[TypeId]| {
            type_arguments
                .iter()
                .map(|&t| type_display_string(&ctx, t, false))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut messages = Vec::new();
        let mut add_message = |message: String| {
            messages.push(DiagnosticMessage {
                file_path: file_path.clone(),
                offset,
                length,
                message,
                url: None,
            });
        };
        if !has_type_arguments {
            let type_str = format!(
                "{element_name}<{}>",
                type_arguments_to_string(&type_arguments)
            );
            add_message(format!(
                "The raw type was instantiated as '{type_str}', and is not regular-bounded."
            ));
        }
        if let Some(inverted_type_arguments) = inverted_type_arguments {
            let inverted_type_str = format!(
                "{element_name}<{}>",
                type_arguments_to_string(inverted_type_arguments)
            );
            add_message(format!(
                "The inverted type '{inverted_type_str}' is also not regular-bounded, so the \
                 type is not well-bounded."
            ));
        }
        messages
    };

    // If not allowed to be super-bounded, report issues.
    if !should_allow_super_bounded_types(host, named_type) {
        for issue in issues {
            let d = diag::type_argument_not_matching_bounds(
                type_arg(&ctx, issue.argument),
                &issue.parameter_name,
                type_arg(&ctx, issue.parameter_bound),
            )
            .with_context_messages(build_context_messages(None));
            let error_node = type_argument_error_node(host.ast(), named_type, issue.index);
            let d = host.at(d, error_node);
            host.report(d);
        }
        return;
    }

    // Prepare type arguments for checking for super-bounded.
    let inverted_type = host.type_system().replace_top_and_bottom(ty);
    let inverted_type_arguments: Vec<TypeId> = if let Some(alias) = ctx.type_alias(inverted_type) {
        ctx.list(ctx.alias(alias).args).to_vec()
    } else if let TypeKind::Interface { args, .. } = ctx.ty(inverted_type) {
        ctx.list(*args).to_vec()
    } else {
        return;
    };

    // Check for super-bounded.
    let inverted_substitution = from_pairs(&type_parameters, &inverted_type_arguments);
    for (i, &type_argument) in inverted_type_arguments.iter().enumerate() {
        let Some(&type_parameter) = type_parameters.get(i) else {
            break;
        };
        let Some(type_parameter_name) = ctx.element_name(type_parameter.raw()) else {
            return;
        };

        let Some(bound) = ctx.type_parameter_bound(type_parameter) else {
            continue;
        };

        let bound = inverted_substitution.substitute_type(&ctx, bound);

        if !host.type_system().is_subtype_of(type_argument, bound) {
            let d = diag::type_argument_not_matching_bounds(
                type_arg(&ctx, type_argument),
                type_parameter_name,
                type_arg(&ctx, bound),
            )
            .with_context_messages(build_context_messages(Some(&inverted_type_arguments)));
            let error_node = type_argument_error_node(host.ast(), named_type, i);
            let d = host.at(d, error_node);
            host.report(d);
        }
    }
}

/// The name of the element, its type parameters and the type arguments of
/// the alias of [ty], or of [ty] if it is an interface type (the first
/// part of Dart `_checkForTypeArgumentNotMatchingBounds`).
type ParametersAndArguments<'a> = (Option<&'a str>, Vec<EId<TypeParameterElement>>, Vec<TypeId>);

fn type_parameters_and_arguments<'a>(
    ctx: &Ctx<'a>,
    ty: TypeId,
) -> Option<ParametersAndArguments<'a>> {
    if let Some(alias) = ctx.type_alias(ty) {
        let alias = ctx.alias(alias);
        let element = ctx.get(alias.element);
        Some((
            ctx.element_name(alias.element.raw()),
            element.type_params.clone(),
            ctx.list(alias.args).to_vec(),
        ))
    } else if let TypeKind::Interface { element, args, .. } = *ctx.ty(ty) {
        Some((
            ctx.element_name(element.raw()),
            ctx.interface_type_parameters(element).to_vec(),
            ctx.list(args).to_vec(),
        ))
    } else {
        None
    }
}

/// Dart `_checkInvocationTypeArguments(typeArgumentList, genericType,
/// instantiatedType)`: verifies that each type argument in
/// [type_argument_list] is within its bounds, as defined by
/// [generic_type].
fn check_invocation_type_arguments<'a, H: VerifierHost<'a>>(
    host: &mut H,
    type_argument_list: Option<Id<TypeArgumentList>>,
    generic_type: Option<TypeId>,
    instantiated_type: Option<TypeId>,
) {
    let ctx = host.ctx();
    let Some(type_argument_list) = type_argument_list else {
        return;
    };

    let (Some(generic_type), Some(instantiated_type)) = (generic_type, instantiated_type) else {
        return;
    };
    let TypeKind::Function(generic) = ctx.ty(generic_type) else {
        return;
    };
    if !matches!(ctx.ty(instantiated_type), TypeKind::Function(_)) {
        return;
    }

    let type_argument_nodes = type_argument_nodes(host, type_argument_list);
    let fn_type_params = ctx.list(generic.type_params);
    let type_args: Vec<TypeId> = type_argument_nodes
        .iter()
        .map(|&t| type_or_throw(host, t))
        .collect();

    // If the amount mismatches, clean up the lists to be substitutable. The
    // mismatch in size is reported elsewhere, but we must successfully
    // perform substitution to validate bounds on mismatched lists.
    let provided_length = type_args.len().min(fn_type_params.len());
    let fn_type_params = &fn_type_params[..provided_length];
    let type_args = &type_args[..provided_length];

    for i in 0..provided_length {
        // Check the `extends` clause for the type parameter, if any.
        //
        // Also substitute to handle cases like this:
        //
        //     <TFrom, TTo extends TFrom>
        //     <TFrom, TTo extends Iterable<TFrom>>
        //     <T extends Cloneable<T>>
        //
        let arg_type = type_args[i];

        if let TypeKind::Function(f) = ctx.ty(arg_type)
            && !ctx.list(f.type_params).is_empty()
            && !library_feature_enabled(&ctx, host.library(), ExperimentalFlag::GenericMetadata)
        {
            let d = host.at(
                diag::generic_function_type_cannot_be_type_argument(),
                type_argument_nodes[i],
            );
            host.report(d);
            continue;
        }

        let fn_type_param = fn_type_params[i];
        let Some(fn_type_param_name) = ctx.element_name(fn_type_param.raw()) else {
            continue;
        };

        let Some(raw_bound) = ctx.type_parameter_bound(fn_type_param) else {
            continue;
        };

        let substitution = MapSubstitution::from_pairs(fn_type_params, type_args);
        let bound = substitution.substitute_type(&ctx, raw_bound);
        if !host.type_system().is_subtype_of(arg_type, bound) {
            let d = diag::type_argument_not_matching_bounds(
                type_arg(&ctx, arg_type),
                fn_type_param_name,
                type_arg(&ctx, bound),
            );
            let d = host.at(d, type_argument_nodes[i]);
            host.report(d);
        }
    }
}

/// Dart `_checkTypeArgumentConst(typeAnnotation, diagnosticCode)`: checks
/// whether the given [type_annotation] contains a type parameter. The
/// [code] is `invalidTypeArgumentInConstList`, `...Map`, or `...Set`.
fn check_type_argument_const<'a, H: VerifierHost<'a>>(
    host: &mut H,
    type_annotation: Id<TypeAnnotation>,
    code: InvalidTypeArgumentCode,
) {
    let ctx = host.ctx();
    let ast = host.ast();
    if let Some(named_type) = ast.cast::<NamedType>(type_annotation) {
        let ty = host.tables().annotation_type.get(named_type).copied();
        if ty.is_some_and(|t| matches!(ctx.ty(t), TypeKind::TypeParameter { .. })) {
            let name = ast.tokens.lexeme(ast[named_type].name).to_string();
            let d = host.at(code(&name), type_annotation);
            host.report(d);
        } else if let Some(type_arguments) = ast[named_type].type_arguments {
            for argument in type_argument_nodes(host, type_arguments) {
                check_type_argument_const(host, argument, code);
            }
        }
    } else if let Some(function_type) = ast.cast::<GenericFunctionType>(type_annotation) {
        let return_type = ast[function_type].return_type;
        let parameters = ast
            .list(ast[ast[function_type].parameters].parameters)
            .to_vec();
        for parameter in parameters {
            let ast = host.ast();
            if let Some(parameter) = ast.cast::<RegularFormalParameter>(parameter)
                && ast[parameter].function_typed_suffix.is_none()
                && let Some(type_annotation) = ast[parameter].type_
            {
                check_type_parameter_type_or_recurse(host, type_annotation, code);
            }
            // `parameter` cannot legally be a function-typed, field, or
            // super formal parameter.
        }
        if let Some(return_type) = return_type {
            check_type_parameter_type_or_recurse(host, return_type, code);
        }
    } else if let Some(record_type) = ast.cast::<RecordTypeAnnotation>(type_annotation) {
        for field in ast.record_type_fields(record_type) {
            let ast = host.ast();
            let type_annotation =
                if let Some(f) = ast.cast::<RecordTypeAnnotationPositionalField>(field) {
                    ast[f].type_
                } else if let Some(f) = ast.cast::<RecordTypeAnnotationNamedField>(field) {
                    ast[f].type_
                } else {
                    continue;
                };
            check_type_parameter_type_or_recurse(host, type_annotation, code);
        }
    }
}

/// The common part of the generic function type and record type cases of
/// Dart `_checkTypeArgumentConst`: reports [type_annotation] if its type is
/// a type parameter type (with the display string of the type), otherwise
/// checks it recursively.
fn check_type_parameter_type_or_recurse<'a, H: VerifierHost<'a>>(
    host: &mut H,
    type_annotation: Id<TypeAnnotation>,
    code: InvalidTypeArgumentCode,
) {
    let ctx = host.ctx();
    let ty = host.tables().annotation_type.get(type_annotation).copied();
    match ty {
        Some(t) if matches!(ctx.ty(t), TypeKind::TypeParameter { .. }) => {
            let name = type_display_string(&ctx, t, false);
            let d = host.at(code(&name), type_annotation);
            host.report(d);
        }
        _ => check_type_argument_const(host, type_annotation, code),
    }
}

/// Dart `_checkTypeArgumentCount(typeArguments, expectedCount, code)`:
/// verifies that [type_arguments] contains exactly [expected_count]
/// elements, reporting an error with the [code] if not.
fn check_type_argument_count<'a, H: VerifierHost<'a>>(
    host: &mut H,
    type_arguments: Id<TypeArgumentList>,
    expected_count: usize,
    code: ExpectedTypeArgumentsCode,
) {
    let ast = host.ast();
    let actual_count = ast.list(ast[type_arguments].arguments).len();
    if actual_count != expected_count {
        let d = host.at(code(actual_count as i64), type_arguments);
        host.report(d);
    }
}

/// Dart `_isMissingTypeArguments(node, type, element)`: whether [ty] is a
/// generic type whose type arguments were not supplied from inference or a
/// non-dynamic default instantiation.
///
/// Dart returns `false` when the element is annotated with
/// `@optionalTypeArgs`; the metadata of elements is not resolved in the
/// element model yet, so this is not checked.
fn is_missing_type_arguments<'a, H: VerifierHost<'a>>(
    host: &H,
    ty: TypeId,
    element: Option<ElemRef>,
) -> bool {
    if element.is_none() {
        return false;
    }
    let ctx = host.ctx();
    let type_arguments: Vec<TypeId> = if let Some(alias) = ctx.type_alias(ty) {
        ctx.list(ctx.alias(alias).args).to_vec()
    } else if let TypeKind::Interface { args, .. } = ctx.ty(ty) {
        ctx.list(*args).to_vec()
    } else {
        return false;
    };

    // Check if this type has type arguments and at least one is dynamic.
    // If so, we may need to issue a strict-raw-types warning.
    type_arguments
        .iter()
        .any(|&t| matches!(ctx.ty(t), TypeKind::Dynamic))
}

/// Dart `_shouldAllowSuperBoundedTypes(namedType)`: whether [named_type]
/// occurs in a context where super-bounded types are allowed.
fn should_allow_super_bounded_types<'a, H: VerifierHost<'a>>(
    host: &H,
    named_type: Id<NamedType>,
) -> bool {
    let ast = host.ast();
    if let Some(parent) = ast.parent(named_type)
        && matches!(
            ast.kind(parent),
            NodeKind::ClassTypeAlias
                | NodeKind::ConstructorName
                | NodeKind::ExtendsClause
                | NodeKind::GenericTypeAlias
                | NodeKind::ImplementsClause
                | NodeKind::MixinOnClause
                | NodeKind::WithClause
        )
    {
        return false;
    }

    let ctx = host.ctx();
    if let Some(ty) = host.tables().annotation_type.get(named_type).copied()
        && ctx
            .interface_element(ty)
            .is_some_and(|e| e.tag() == Tag::ExtensionType)
    {
        return false;
    }

    true
}

/// Dart `_typeArgumentErrorNode(node, index)`: the type argument at
/// [index] of [node], or the [node] itself.
fn type_argument_error_node(ast: &dartr_ast::Ast, node: Id<NamedType>, index: usize) -> NodeId {
    if let Some(type_arguments) = ast[node].type_arguments
        && let Some(&argument) = ast.list(ast[type_arguments].arguments).get(index)
    {
        return argument.raw();
    }
    node.raw()
}

/// The type arguments of [list] (Dart `typeArgumentList.arguments`).
fn type_argument_nodes<'a, H: VerifierHost<'a>>(
    host: &H,
    list: Id<TypeArgumentList>,
) -> Vec<Id<TypeAnnotation>> {
    let ast = host.ast();
    ast.list(ast[list].arguments).to_vec()
}

/// Dart `typeAnnotation.typeOrThrow` (`dynamic` when not resolved).
fn type_or_throw<'a, H: VerifierHost<'a>>(host: &H, node: Id<TypeAnnotation>) -> TypeId {
    host.tables()
        .annotation_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::DYNAMIC)
}

/// The type arguments of an interface type (empty for other types).
fn interface_type_arguments(ctx: &Ctx<'_>, ty: TypeId) -> Vec<TypeId> {
    match ctx.ty(ty) {
        TypeKind::Interface { args, .. } => ctx.list(*args).to_vec(),
        _ => Vec::new(),
    }
}

/// Dart `Substitution.fromPairs2(parameters, types)`. Erroneous code can
/// have fewer type arguments than type parameters; the extra parameters
/// are not substituted.
fn from_pairs(parameters: &[EId<TypeParameterElement>], types: &[TypeId]) -> MapSubstitution {
    let n = parameters.len().min(types.len());
    MapSubstitution::from_pairs(&parameters[..n], &types[..n])
}

/// Whether [tag] is the tag of an interface element (Dart
/// `InterfaceElementImpl`).
fn is_interface_tag(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
    )
}
