// Dart source: pkg/analyzer/lib/src/dart/resolver/extension_member_resolver.dart

//! `ExtensionMemberResolver`: explicit extension overrides and implicit
//! extension member selection.

use dartr_ast::{
    BinaryExpression, CascadeExpression, Expression, ExtensionOverride,
    FunctionExpressionInvocation, Id, IndexExpression, MethodInvocation, NamedArgument, NodeId,
    PrefixExpression, PropertyAccess,
};
use dartr_diagnostics::{DiagnosticReporter, diag};
use dartr_element::diagnostics::{element_arg, type_arg, type_display_string};
use dartr_element::{
    EId, ElemRef, ExtensionElement, InterfaceElement, Nullability, TypeId, TypeKind,
    TypeParameterElement,
};
use dartr_typesystem::TypeExt;
use dartr_typesystem::generic_inferrer::{GenericInferrer, InferenceErrorEntity, InferenceFlags};
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::MapSubstitution;

use crate::applicable_extensions::{
    InstantiatedExtensionWithMember, applicable_to, having_member_with_base_name,
    having_static_member_with_name,
};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitExtensionOverride(node, contextType: contextType)`.
pub fn visit_extension_override(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    context_type: TypeId,
) {
    let _ = context_type;
    let (import_prefix, type_arguments, argument_list) = {
        let n = &rv.ast[node];
        (n.import_prefix, n.type_arguments, n.argument_list)
    };
    rv.visit_opt(import_prefix);
    rv.visit_opt(type_arguments);

    let Some(element) = extension_element(rv, node) else {
        rv.visit_node(argument_list.raw());
        return;
    };
    let receiver_context_type = compute_override_receiver_context_type(rv, node, element);
    let arguments = rv.ast.list(rv.ast[argument_list].arguments).to_vec();
    if arguments.len() != 1 {
        rv.visit_node(argument_list.raw());
        validate_override_context(rv, node);
        let d = rv.at(diag::invalid_extension_argument_count(), argument_list);
        rv.report(d);
        let dynamic_types = list_of_dynamic(rv.ctx.get(element).type_params.len());
        record_override_types(rv, node, &dynamic_types, TypeId::DYNAMIC);
        return;
    }

    let Some(receiver_expression) = argument_expression(rv, arguments[0].raw()) else {
        rv.visit_node(arguments[0].raw());
        let dynamic_types = list_of_dynamic(rv.ctx.get(element).type_params.len());
        record_override_types(rv, node, &dynamic_types, TypeId::DYNAMIC);
        validate_override_context(rv, node);
        return;
    };
    let receiver_expression = rv.resolve_expression(
        receiver_expression,
        receiver_context_type.unwrap_or(TypeId::UNKNOWN),
    );
    validate_override_context(rv, node);
    resolve_override(rv, node, element, receiver_expression);
}

/// Dart `ExtensionResolutionResult` / `ExtensionResolutionError`: the getter
/// and setter of the single most specific applicable extension, or
/// whether the extensions are ambiguous.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExtensionResolutionResult {
    pub getter: Option<dartr_element::ElemRef>,
    pub setter: Option<dartr_element::ElemRef>,
    /// Dart `ExtensionResolutionError.ambiguous`.
    pub is_ambiguous: bool,
}

/// Dart `ExtensionMemberResolver.findExtension(type, nameEntity, name)`:
/// the extension member [name] applicable to [ty]. [name_entity] is the
/// node to report an ambiguity on.
pub fn find_extension(
    rv: &mut ResolverVisitor<'_>,
    ty: TypeId,
    name_entity: dartr_ast::NodeId,
    name: &dartr_typesystem::inheritance_manager3::Name,
) -> ExtensionResolutionResult {
    let accessible_extensions = rv
        .unit
        .scopes
        .accessible_extensions(rv.unit.fragment)
        .to_vec();
    let candidates = accessible_extensions
        .into_iter()
        .filter_map(|extension| having_member_with_base_name(rv, extension, name));
    let extensions = applicable_to(rv, candidates, ty);

    if extensions.is_empty() {
        return ExtensionResolutionResult::default();
    }
    if extensions.len() == 1 {
        return as_resolution_result(extensions[0]);
    }

    let most_specific = choose_most_specific(rv, extensions);
    if most_specific.len() == 1 {
        return as_resolution_result(most_specific[0]);
    }

    let name_text = name.text(&rv.ctx);
    let diagnostic = if most_specific.len() == 2 {
        diag::ambiguous_extension_member_access_two(
            name_text,
            element_arg(&rv.ctx, most_specific[0].extension.raw()),
            element_arg(&rv.ctx, most_specific[1].extension.raw()),
        )
    } else {
        let descriptions: Vec<String> = most_specific
            .iter()
            .map(|e| {
                rv.ctx
                    .element_name(e.extension.raw())
                    .map(|name| format!("extension '{name}'"))
                    .unwrap_or_else(|| {
                        format!(
                            "unnamed extension on '{}'",
                            type_display_string(&rv.ctx, e.extended_type, true)
                        )
                    })
            })
            .collect();
        let descriptions = comma_separated_with_and(&descriptions);
        diag::ambiguous_extension_member_access_three_or_more(name_text, &descriptions)
    };
    let d = rv.at(diagnostic, name_entity);
    rv.report(d);
    ExtensionResolutionResult {
        is_ambiguous: true,
        ..ExtensionResolutionResult::default()
    }
}

/// Dart `StaticExtensionResolutionResult`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StaticExtensionResolutionResult {
    pub member: Option<ElemRef>,
    pub is_ambiguous: bool,
}

/// Dart `ExtensionMemberResolver.findStaticExtension`.
pub fn find_static_extension(
    rv: &ResolverVisitor<'_>,
    declaration: EId<InterfaceElement>,
    _name_entity: NodeId,
    name: &dartr_typesystem::inheritance_manager3::Name,
) -> StaticExtensionResolutionResult {
    let accessible_extensions = rv
        .unit
        .scopes
        .accessible_extensions(rv.unit.fragment)
        .to_vec();
    let extensions: Vec<_> = accessible_extensions
        .into_iter()
        .flat_map(|extension| having_static_member_with_name(rv, extension, name))
        .filter(|candidate| on_declaration(rv, candidate.extension) == Some(declaration))
        .collect();
    match extensions.as_slice() {
        [] => StaticExtensionResolutionResult::default(),
        [extension] => StaticExtensionResolutionResult {
            member: Some(extension.member),
            is_ambiguous: false,
        },
        _ => StaticExtensionResolutionResult {
            member: None,
            is_ambiguous: true,
        },
    }
}

/// Dart `ExtensionMemberResolver.computeOverrideReceiverContextType`.
pub fn compute_override_receiver_context_type(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    element: EId<ExtensionElement>,
) -> Option<TypeId> {
    let argument_list = rv.ast[node].argument_list;
    if rv.ast.list(rv.ast[argument_list].arguments).len() != 1 {
        return None;
    }
    let extension = rv.ctx.get(element);
    let extended_type = extension.extended_type.get()?;
    let type_argument_types = if let Some(type_arguments) = rv.ast[node].type_arguments {
        let arguments = rv.ast.list(rv.ast[type_arguments].arguments);
        if arguments.len() == extension.type_params.len() {
            arguments
                .iter()
                .map(|&a| {
                    rv.tables
                        .annotation_type
                        .get(a)
                        .copied()
                        .unwrap_or(TypeId::DYNAMIC)
                })
                .collect()
        } else {
            list_of_dynamic(extension.type_params.len())
        }
    } else {
        vec![TypeId::UNKNOWN; extension.type_params.len()]
    };
    Some(
        MapSubstitution::from_pairs(&extension.type_params, &type_argument_types)
            .substitute_type(&rv.ctx, extended_type),
    )
}

/// Dart `ExtensionMemberResolver.getOverrideMember`.
pub fn get_override_member(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    name: &str,
) -> ExtensionResolutionResult {
    let Some(element) = extension_element(rv, node) else {
        return ExtensionResolutionResult::default();
    };
    let instance = element.upcast();
    let (getter, setter) = if name == "[]" {
        (
            dartr_typesystem::lookup::get_method(&rv.ctx, instance, "[]")
                .map(|e| ElemRef::Base(e.raw())),
            dartr_typesystem::lookup::get_method(&rv.ctx, instance, "[]=")
                .map(|e| ElemRef::Base(e.raw())),
        )
    } else {
        (
            dartr_typesystem::lookup::get_getter(&rv.ctx, instance, name)
                .map(|e| ElemRef::Base(e.raw()))
                .or_else(|| {
                    dartr_typesystem::lookup::get_method(&rv.ctx, instance, name)
                        .map(|e| ElemRef::Base(e.raw()))
                }),
            dartr_typesystem::lookup::get_setter(&rv.ctx, instance, name)
                .map(|e| ElemRef::Base(e.raw())),
        )
    };
    if getter.is_none() && setter.is_none() {
        return ExtensionResolutionResult::default();
    }
    let extension = rv.ctx.get(element);
    let Some(type_argument_types) = rv.tables.type_arg_types.get(node) else {
        return ExtensionResolutionResult::default();
    };
    let substitution =
        MapSubstitution::from_pairs(&extension.type_params, rv.ctx.list(*type_argument_types));
    ExtensionResolutionResult {
        getter: getter.map(|e| member::substitute(&rv.ctx, e, &substitution)),
        setter: setter.map(|e| member::substitute(&rv.ctx, e, &substitution)),
        is_ambiguous: false,
    }
}

fn extension_element(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
) -> Option<EId<ExtensionElement>> {
    member::base_element(&rv.ctx, rv.element(node)?).cast::<ExtensionElement>()
}

/// Dart `ExtensionElementImpl.onDeclaration`.
fn on_declaration(
    rv: &ResolverVisitor<'_>,
    extension: EId<ExtensionElement>,
) -> Option<EId<InterfaceElement>> {
    let extended_type = rv.ctx.get(extension).extended_type.get()?;
    match *rv.ctx.ty(extended_type) {
        TypeKind::Interface {
            element,
            nullability: Nullability::None,
            ..
        } if !rv.ctx.is_dart_async_future_or(extended_type) => Some(element),
        _ => None,
    }
}

fn argument_expression(rv: &ResolverVisitor<'_>, argument: NodeId) -> Option<Id<Expression>> {
    rv.ast.cast::<Expression>(argument).or_else(|| {
        rv.ast
            .cast::<NamedArgument>(argument)
            .map(|a| rv.ast[a].argument_expression)
    })
}

/// Dart `ExtensionMemberResolver.resolveOverride`.
fn resolve_override(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    element: EId<ExtensionElement>,
    receiver_expression: Id<Expression>,
) {
    let mut receiver_type = rv.type_or_throw(receiver_expression);
    if is_null_aware(rv, node) {
        receiver_type = rv.type_system.promote_to_non_null(receiver_type);
    }

    let type_argument_types = infer_type_arguments(rv, node, element, receiver_type);
    let extension = rv.ctx.get(element);
    let substitution = MapSubstitution::from_pairs(&extension.type_params, &type_argument_types);
    let extended_type = extension
        .extended_type
        .get()
        .map(|t| substitution.substitute_type(&rv.ctx, t))
        .unwrap_or(TypeId::DYNAMIC);
    record_override_types(rv, node, &type_argument_types, extended_type);
    check_type_arguments_matching_bounds(
        rv,
        node,
        &extension.type_params,
        &type_argument_types,
        &substitution,
    );

    if matches!(*rv.ctx.ty(receiver_type), TypeKind::Void) {
        let d = rv.at(diag::use_of_void_result(), receiver_expression);
        rv.report(d);
    } else if !rv.type_system.is_assignable_to(
        receiver_type,
        extended_type,
        rv.unit.options.strict_casts,
    ) {
        let d = diag::extension_override_argument_not_assignable(
            type_arg(&rv.ctx, receiver_type),
            type_arg(&rv.ctx, extended_type),
        );
        let d = rv.at(d, receiver_expression);
        rv.report(d);
    }
}

/// Dart `ExtensionMemberResolver._inferTypeArguments`.
fn infer_type_arguments(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    element: EId<ExtensionElement>,
    receiver_type: TypeId,
) -> Vec<TypeId> {
    let extension = rv.ctx.get(element);
    let type_parameters = extension.type_params.clone();
    if let Some(type_arguments) = rv.ast[node].type_arguments {
        let arguments = rv.ast.list(rv.ast[type_arguments].arguments).to_vec();
        if arguments.len() == type_parameters.len() {
            return arguments
                .iter()
                .map(|&a| {
                    rv.tables
                        .annotation_type
                        .get(a)
                        .copied()
                        .unwrap_or(TypeId::DYNAMIC)
                })
                .collect();
        }
        let extension_name = rv.ctx.element_name(element.raw()).unwrap_or("");
        let d = diag::wrong_number_of_type_arguments_extension(
            extension_name,
            type_parameters.len() as i64,
            arguments.len() as i64,
        );
        let d = rv.at(d, type_arguments);
        rv.report(d);
        return list_of_dynamic(type_parameters.len());
    }

    let extended_type = extension.extended_type.get().unwrap_or(TypeId::DYNAMIC);
    let flags = InferenceFlags {
        generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
        inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
        strict_inference: rv.unit.options.strict_inference,
    };
    let name_token = rv.ast[node].name;
    let token = rv.ast.tokens.get(name_token);
    let entity = InferenceErrorEntity::other(token.offset as usize, token.length as usize);
    let operations = rv.flow_analysis.type_operations;
    let mut reported = Vec::new();
    let result = {
        let mut listener = |d| reported.push(d);
        let mut reporter = DiagnosticReporter::new(&mut listener);
        let mut inferrer = GenericInferrer::new(
            rv.type_system,
            &type_parameters,
            Some(&mut reporter),
            Some(entity),
            flags,
            operations,
            None,
        );
        inferrer.constrain_argument(
            receiver_type,
            extended_type,
            "extendedType",
            Some(node.raw()),
        );
        inferrer.choose_final_types()
    };
    rv.flush_type_analyzer_errors();
    if rv.lock_level == 0 {
        rv.diagnostics.extend(reported);
    }
    result
}

fn check_type_arguments_matching_bounds(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    type_parameters: &[EId<TypeParameterElement>],
    type_argument_types: &[TypeId],
    substitution: &MapSubstitution,
) {
    let Some(type_arguments) = rv.ast[node].type_arguments else {
        return;
    };
    let arguments = rv.ast.list(rv.ast[type_arguments].arguments).to_vec();
    for (i, (&argument, &parameter)) in type_argument_types
        .iter()
        .zip(type_parameters.iter())
        .enumerate()
    {
        let Some(bound) = rv.ctx.type_parameter_bound(parameter) else {
            continue;
        };
        let bound = substitution.substitute_type(&rv.ctx, bound);
        if rv.type_system.is_subtype_of(argument, bound) {
            continue;
        }
        let Some(parameter_name) = rv.ctx.element_name(parameter.raw()) else {
            continue;
        };
        let d = diag::type_argument_not_matching_bounds(
            type_arg(&rv.ctx, argument),
            parameter_name,
            type_arg(&rv.ctx, bound),
        );
        let d = rv.at(d, arguments[i]);
        rv.report(d);
    }
}

fn record_override_types(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    type_arguments: &[TypeId],
    extended_type: TypeId,
) {
    let list = rv.ctx.intern_list(type_arguments);
    rv.tables.type_arg_types.insert(node, list);
    rv.tables.extended_type.insert(node, extended_type);
}

fn validate_override_context(rv: &mut ResolverVisitor<'_>, node: Id<ExtensionOverride>) {
    if is_valid_context(rv, node) {
        return;
    }
    if !is_cascade_target(rv, node) {
        let d = rv.at(diag::extension_override_without_access(), node);
        rv.report(d);
    }
    rv.set_static_type(node, TypeId::DYNAMIC);
}

fn is_null_aware(rv: &ResolverVisitor<'_>, node: Id<ExtensionOverride>) -> bool {
    let end = rv.ast.end_token(rv.ast[node].argument_list);
    let next = rv.ast.tokens.next(end);
    matches!(
        rv.ast.tokens.ty(next),
        dartr_syntax::TokenType::QUESTION_PERIOD | dartr_syntax::TokenType::QUESTION
    )
}

fn is_cascade_target(rv: &ResolverVisitor<'_>, node: Id<ExtensionOverride>) -> bool {
    rv.ast
        .parent(node)
        .and_then(|p| rv.ast.cast::<CascadeExpression>(p))
        .is_some_and(|p| rv.ast[p].target.raw() == node.raw())
}

fn is_valid_context(rv: &ResolverVisitor<'_>, node: Id<ExtensionOverride>) -> bool {
    let Some(parent) = rv.ast.parent(node) else {
        return false;
    };
    rv.ast
        .cast::<BinaryExpression>(parent)
        .is_some_and(|p| rv.ast[p].left_operand.raw() == node.raw())
        || rv
            .ast
            .cast::<FunctionExpressionInvocation>(parent)
            .is_some_and(|p| rv.ast[p].function.raw() == node.raw())
        || rv
            .ast
            .cast::<IndexExpression>(parent)
            .is_some_and(|p| rv.ast[p].target.map(|t| t.raw()) == Some(node.raw()))
        || rv
            .ast
            .cast::<MethodInvocation>(parent)
            .is_some_and(|p| rv.ast[p].target.map(|t| t.raw()) == Some(node.raw()))
        || rv.ast.is::<PrefixExpression>(parent)
        || rv
            .ast
            .cast::<PropertyAccess>(parent)
            .is_some_and(|p| rv.ast[p].target.map(|t| t.raw()) == Some(node.raw()))
}

/// Dart `ExtensionMemberResolver._chooseMostSpecific`.
fn choose_most_specific(
    rv: &ResolverVisitor<'_>,
    extensions: Vec<InstantiatedExtensionWithMember>,
) -> Vec<InstantiatedExtensionWithMember> {
    let mut best_so_far = None;
    let mut none_more_specific = Vec::new();
    for candidate in extensions {
        if !none_more_specific.is_empty() {
            let mut is_most_specific = true;
            let mut has_more_specific = false;
            for &other in &none_more_specific {
                if !is_more_specific(rv, candidate, other) {
                    is_most_specific = false;
                }
                if is_more_specific(rv, other, candidate) {
                    has_more_specific = true;
                }
            }
            if is_most_specific {
                best_so_far = Some(candidate);
                none_more_specific.clear();
            } else if !has_more_specific {
                none_more_specific.push(candidate);
            }
        } else if let Some(best) = best_so_far {
            if is_more_specific(rv, best, candidate) {
                // Keep the current best.
            } else if is_more_specific(rv, candidate, best) {
                best_so_far = Some(candidate);
            } else {
                none_more_specific.push(best);
                none_more_specific.push(candidate);
                best_so_far = None;
            }
        } else {
            best_so_far = Some(candidate);
        }
    }
    best_so_far.map_or(none_more_specific, |best| vec![best])
}

/// Dart `ExtensionMemberResolver._isMoreSpecific`.
fn is_more_specific(
    rv: &ResolverVisitor<'_>,
    e1: InstantiatedExtensionWithMember,
    e2: InstantiatedExtensionWithMember,
) -> bool {
    let e1_is_in_sdk = extension_is_in_sdk(rv, e1.extension);
    let e2_is_in_sdk = extension_is_in_sdk(rv, e2.extension);
    if e1_is_in_sdk && !e2_is_in_sdk {
        return false;
    }
    if !e1_is_in_sdk && e2_is_in_sdk {
        return true;
    }
    if !rv
        .type_system
        .is_subtype_of(e1.extended_type, e2.extended_type)
    {
        return false;
    }
    if !rv
        .type_system
        .is_subtype_of(e2.extended_type, e1.extended_type)
    {
        return true;
    }
    let bound1 = instantiate_to_bounds(rv, e1.extension);
    let bound2 = instantiate_to_bounds(rv, e2.extension);
    rv.type_system.is_subtype_of(bound1, bound2) && !rv.type_system.is_subtype_of(bound2, bound1)
}

fn instantiate_to_bounds(rv: &ResolverVisitor<'_>, extension: EId<ExtensionElement>) -> TypeId {
    let extension = rv.ctx.get(extension);
    let extended_type = extension.extended_type.get().unwrap_or(TypeId::DYNAMIC);
    let (arguments, _) = rv
        .type_system
        .instantiate_type_formals_to_bounds(&extension.type_params, None);
    MapSubstitution::from_pairs(&extension.type_params, &arguments)
        .substitute_type(&rv.ctx, extended_type)
}

fn extension_is_in_sdk(rv: &ResolverVisitor<'_>, extension: EId<ExtensionElement>) -> bool {
    rv.ctx
        .get(extension)
        .library
        .is_some_and(|library| rv.ctx.library_uri(library).starts_with("dart:"))
}

fn as_resolution_result(extension: InstantiatedExtensionWithMember) -> ExtensionResolutionResult {
    ExtensionResolutionResult {
        getter: extension.getter,
        setter: extension.setter,
        is_ambiguous: false,
    }
}

fn list_of_dynamic(length: usize) -> Vec<TypeId> {
    vec![TypeId::DYNAMIC; length]
}

fn comma_separated_with_and(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, rest) = values.split_last().expect("non-empty");
            format!("{}, and {last}", rest.join(", "))
        }
    }
}
