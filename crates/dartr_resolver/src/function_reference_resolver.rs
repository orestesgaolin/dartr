// Dart source: pkg/analyzer/lib/src/dart/resolver/function_reference_resolver.dart

//! `FunctionReferenceResolver`: an expression with explicit type arguments
//! (`f<int>`, `a.m<int>`, `List<int>`): a generic function instantiation, a
//! tear-off of the implicit `call` method (rewritten to an
//! `ImplicitCallReference`), or a type literal (rewritten to a
//! `TypeLiteral`).
//!
//! Also here: Dart `ResolverVisitor.visitFunctionReference`,
//! `visitImplicitCallReference`, `_insertImplicitCallReference` (after the
//! form checks of the core) and `getImplicitCallMethod`
//! (error_detection_helpers.dart).

use dartr_ast::{
    ConstructorReference, Expression, ExtensionOverride, FunctionReference, Id, Identifier,
    ImplicitCallReference, ImportPrefixReference, NodeId, PrefixedIdentifier, PropertyAccess,
    SimpleIdentifier, SuperExpression, ThisExpression, TypeArgumentList, TypeLiteral,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    ElemRef, ElementId, ExtensionElement, InstanceElement, InterfaceElement, Nullability, Tag,
    TypeAliasElement, TypeId, TypeKind, TypeParameterElement,
};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::extension_member_resolver::find_extension;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `InvocationTarget` / `TypeInstantiationTarget`: what the type
/// arguments apply to, for the wrong-number-of-type-arguments diagnostic.
#[derive(Clone, Copy, Debug)]
enum Target {
    /// `InvocationTargetExecutableElement` (and
    /// `InvocationTargetConstructorElement`).
    Executable(ElemRef),
    /// `InvocationTargetConstructorElement`: the constructor's class is
    /// reported.
    Constructor(ElemRef),
    /// `InvocationTargetFunctionTypedExpression`.
    FunctionTypedExpression(TypeId),
    /// `TypeInstantiationTargetInterfaceElement` /
    /// `TypeInstantiationTargetTypeAliasElement`.
    TypeDefining(ElementId),
}

impl Target {
    /// Dart `InvocationTargetFunctionTypedExpression.orNull(type)`.
    fn function_typed_or_null(rv: &ResolverVisitor<'_>, ty: Option<TypeId>) -> Option<Target> {
        let ty = ty?;
        matches!(rv.ctx.ty(ty), TypeKind::Function(_))
            .then_some(Target::FunctionTypedExpression(ty))
    }

    /// Dart `wrongNumberOfTypeArgumentsError(typeParameterCount:,
    /// typeArgumentCount:)`.
    fn wrong_number_of_type_arguments_error(
        self,
        rv: &ResolverVisitor<'_>,
        type_parameter_count: usize,
        type_argument_count: usize,
    ) -> LocatableDiagnostic {
        let ctx = rv.ctx;
        let (p, a) = (type_parameter_count as i64, type_argument_count as i64);
        let element_error = |element: ElementId| {
            diag::wrong_number_of_type_arguments_element(
                element.tag().element_kind().display_name(),
                ctx.element_name(element).unwrap_or(""),
                p,
                a,
            )
        };
        match self {
            Target::Executable(e) => element_error(member::base_element(&ctx, e)),
            Target::Constructor(e) => match member::enclosing_element(&ctx, e) {
                Some(enclosing) => element_error(enclosing),
                None => element_error(member::base_element(&ctx, e)),
            },
            Target::FunctionTypedExpression(t) => {
                diag::wrong_number_of_type_arguments_function(type_arg(&ctx, t), p, a)
            }
            Target::TypeDefining(e) => {
                diag::wrong_number_of_type_arguments(ctx.element_name(e).unwrap_or(""), p, a)
            }
        }
    }
}

/// Dart `ResolverVisitor.visitFunctionReference(node, contextType:)`.
pub fn visit_function_reference(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), crate::resolver::SchemaOf::new(context_type));
    }
    resolve(rv, node);
    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `ResolverVisitor.visitImplicitCallReference(node, contextType:)`.
pub fn visit_implicit_call_reference(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ImplicitCallReference>,
    context_type: TypeId,
) {
    let _ = context_type;
    rv.check_unreachable_node(node);
    let expression = rv.ast[node].expression;
    rv.resolve_expression(expression, TypeId::UNKNOWN);
    let type_arguments = rv.ast[node].type_arguments;
    rv.visit_opt(type_arguments);
}

/// Dart `ResolverVisitor._insertImplicitCallReference(expression,
/// contextType:)` after the form check and the context of an assignment
/// (the core does those): if [expression] should be treated as
/// `expression.call`, wraps it in an `ImplicitCallReference`.
pub fn insert_implicit_call_reference(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<Expression>,
    context_type: TypeId,
) {
    let Some(static_type) = rv.static_type(expression) else {
        return;
    };
    let Some(call_method) = get_implicit_call_method(rv, static_type, context_type) else {
        return;
    };
    let parent = rv.ast.parent(expression);
    let ctx = rv.ctx;

    // `expression` is to be treated as `expression.call`.
    let context = rv.type_system.flatten(context_type);
    let mut call_method_type = member::type_(&ctx, call_method);
    let type_argument_types = match (*ctx.ty(call_method_type), *ctx.ty(context)) {
        (TypeKind::Function(f), TypeKind::Function(_))
            if rv.is_constructor_tearoffs_enabled() && !ctx.list(f.type_params).is_empty() =>
        {
            // If the constructor-tearoffs feature is enabled, then so is
            // generic-metadata.
            let types =
                rv.infer_function_type_instantiation(context, call_method_type, expression, true);
            if !types.is_empty() {
                call_method_type = ctx.instantiate_function_type(call_method_type, &types);
            }
            types
        }
        _ => Vec::new(),
    };

    let call_reference = rv.ast.add(ImplicitCallReference {
        expression,
        type_arguments: None,
    });
    rv.replace_expression(expression, call_reference.upcast(), parent);
    rv.set_element(call_reference, Some(call_method));
    let list = ctx.intern_list(&type_argument_types);
    rv.tables.type_arg_types.insert(call_reference, list);
    rv.set_static_type(call_reference, call_method_type);
}

/// Dart `ErrorDetectionHelpers.getImplicitCallMethod(type, context,
/// errorNode)`: if an assignment from [ty] to [context] is a case of an
/// implicit `call` method, the `call` method.
pub fn get_implicit_call_method(
    rv: &ResolverVisitor<'_>,
    ty: TypeId,
    context: TypeId,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    let mut ty = ty;
    let mut visited_types = indexmap::IndexSet::new();
    visited_types.insert(ty);
    while let TypeKind::TypeParameter { nullability, .. } = *ctx.ty(ty) {
        if nullability != Nullability::None {
            // The value might be `null`, so implicit `.call` tearoff is
            // invalid.
            return None;
        }
        ty = ctx.type_parameter_type_bound(ty);
        if !visited_types.insert(ty) {
            // A cycle!
            return None;
        }
    }
    if !rv.type_system.accepts_function_type(context) {
        return None;
    }
    let TypeKind::Interface {
        element,
        nullability,
        ..
    } = *ctx.ty(ty)
    else {
        return None;
    };
    if nullability == Nullability::Question {
        return None;
    }
    let library = ctx.element_data(element.raw()).and_then(|d| d.library);
    let name = Name::for_library(&ctx, library, "call");
    InheritanceManager3::new(ctx)
        .get_member3(ty, name, GetMemberOptions::default())
        .filter(|&m| member::base_element(&ctx, m).tag() == Tag::Method)
}

/// Dart `FunctionReferenceResolver.resolve(node)`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<FunctionReference>) {
    let function = rv.ast[node].function;
    let type_arguments = rv.ast[node].type_arguments;
    rv.visit_opt(type_arguments);

    if let Some(function) = rv.ast.cast::<SimpleIdentifier>(function) {
        resolve_simple_identifier_function(rv, node, function);
    } else if let Some(function) = rv.ast.cast::<PrefixedIdentifier>(function) {
        resolve_prefixed_identifier_function(rv, node, function);
    } else if let Some(function) = rv.ast.cast::<PropertyAccess>(function) {
        resolve_property_access_function(rv, node, function);
    } else if let Some(function) = rv.ast.cast::<ConstructorReference>(function) {
        if let Some(type_arguments) = type_arguments {
            // Something like `List.filled<int>`.
            rv.resolve_expression(function.upcast(), TypeId::UNKNOWN);
            // We can safely assume `function.constructorName.name` is
            // non-null because if no name had been given, the construct
            // would have been interpreted as a type literal (e.g.
            // `List<int>`).
            let constructor_name = rv.ast[function].constructor_name;
            let named_type = rv.ast[constructor_name].type_;
            let class_name = qualified_name(rv, named_type);
            let name = rv.ast[constructor_name]
                .name
                .map(|n| rv.lexeme(rv.ast[n].token).to_string())
                .unwrap_or_default();
            let d = diag::wrong_number_of_type_arguments_constructor(&class_name, &name);
            let d = rv.at(d, type_arguments);
            rv.report(d);
            let constructor_element = rv.element(constructor_name);
            let raw_type = rv.static_type(function);
            resolve_type(
                rv,
                node,
                raw_type,
                constructor_element.map(Target::Constructor),
            );
        }
    } else {
        // TODO(srawlins): Handle `function` being a [SuperExpression].
        let function = rv.resolve_expression(function, TypeId::UNKNOWN);
        let function_type = rv.static_type(function);
        match function_type {
            None => resolve_disallowed_expression(rv, node, function_type),
            Some(t) if matches!(rv.ctx.ty(t), TypeKind::Function(_)) => {
                resolve_type(rv, node, Some(t), Some(Target::FunctionTypedExpression(t)));
            }
            Some(t) => {
                let call_method = get_call_method(rv, node, Some(t));
                match call_method {
                    Some(call_method) if is_method(call_method, rv) => {
                        resolve_as_implicit_call_reference(rv, node, call_method);
                    }
                    _ => resolve_disallowed_expression(rv, node, function_type),
                }
            }
        }
    }
}

/// Dart `NamedType.qualifiedName`.
fn qualified_name(rv: &ResolverVisitor<'_>, named_type: Id<dartr_ast::NamedType>) -> String {
    let name = rv.lexeme(rv.ast[named_type].name);
    match rv.ast[named_type].import_prefix {
        Some(prefix) => format!("{}.{name}", rv.lexeme(rv.ast[prefix].name)),
        None => name.to_string(),
    }
}

/// Whether [element] is a `MethodElement`.
fn is_method(element: ElemRef, rv: &ResolverVisitor<'_>) -> bool {
    member::base_element(&rv.ctx, element).tag() == Tag::Method
}

/// Dart `_checkTypeArguments(typeArgumentList, name, typeParameters,
/// target:)`.
fn check_type_arguments(
    rv: &mut ResolverVisitor<'_>,
    type_argument_list: Id<TypeArgumentList>,
    type_parameter_count: usize,
    target: Option<Target>,
) -> Vec<TypeId> {
    let arguments = rv.ast.list(rv.ast[type_argument_list].arguments).to_vec();
    if arguments.len() != type_parameter_count {
        if let Some(target) = target {
            let d = target.wrong_number_of_type_arguments_error(
                rv,
                type_parameter_count,
                arguments.len(),
            );
            let d = rv.at(d, type_argument_list);
            rv.report(d);
        }
        vec![TypeId::DYNAMIC; type_parameter_count]
    } else {
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
    }
}

/// Dart `_getCallMethod(node, type)`.
fn get_call_method(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    ty: Option<TypeId>,
) -> Option<ElemRef> {
    let ty = ty?;
    let ctx = rv.ctx;
    let TypeKind::Interface {
        element,
        nullability,
        ..
    } = *ctx.ty(ty)
    else {
        return None;
    };
    let call_method_name = Name::for_library(&ctx, Some(rv.unit.library), "call");
    if nullability == Nullability::Question {
        // If the interface type is nullable, only an applicable extension
        // method applies.
        return find_extension(rv, ty, node.raw(), &call_method_name).getter;
    }
    // Otherwise, a 'call' method on the interface, or on an applicable
    // extension method applies.
    let library = ctx
        .element_data(element.raw())
        .and_then(|d| d.library)
        .unwrap_or(rv.unit.library);
    lookup::type_look_up_method(&ctx, ty, "call", library, lookup::LookUpOptions::default())
        .or_else(|| find_extension(rv, ty, node.raw(), &call_method_name).getter)
}

/// Dart `_reportInvalidAccessToStaticMember(nameNode, element,
/// implicitReceiver:)`.
fn report_invalid_access_to_static_member(
    rv: &mut ResolverVisitor<'_>,
    name_node: Id<SimpleIdentifier>,
    element: ElemRef,
    implicit_receiver: bool,
) {
    let ctx = rv.ctx;
    let Some(enclosing_element) = member::enclosing_element(&ctx, element) else {
        return;
    };
    let enclosing_name = ctx.element_name(enclosing_element).map(str::to_string);
    let name = rv.lexeme(rv.ast[name_node].token).to_string();
    let d = if implicit_receiver {
        let display = enclosing_name.unwrap_or_default();
        if rv.enclosing_extension.is_some() {
            diag::unqualified_reference_to_static_member_of_extended_type(&display)
        } else {
            diag::unqualified_reference_to_non_local_static_member(&display)
        }
    } else if enclosing_element.tag() == Tag::Extension && enclosing_name.is_none() {
        let kind = member::base_element(&ctx, element)
            .tag()
            .element_kind()
            .display_name();
        diag::instance_access_to_static_member_of_unnamed_extension(&name, kind)
    } else {
        // It is safe to assume that `enclosingElement.name` is non-`null`
        // because it can only be `null` for extensions, and we handle that
        // case above.
        let kind = member::base_element(&ctx, element)
            .tag()
            .element_kind()
            .display_name();
        let enclosing_kind = if enclosing_element.tag() == Tag::Mixin {
            "mixin"
        } else {
            enclosing_element.tag().element_kind().display_name()
        };
        diag::instance_access_to_static_member(
            &name,
            kind,
            &enclosing_name.unwrap_or_default(),
            enclosing_kind,
        )
    };
    let d = rv.at(d, name_node);
    rv.report(d);
}

/// Dart `_resolve(node:, rawType:, name:, target:)`: resolves [node]'s
/// static type, as an instantiated function type, and type argument types,
/// using [raw_type] as the uninstantiated function type.
fn resolve_type(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    raw_type: Option<TypeId>,
    target: Option<Target>,
) {
    let ctx = rv.ctx;
    if raw_type.is_none() {
        rv.record_static_type(node, TypeId::DYNAMIC);
    }
    let mut raw_type = raw_type;
    if raw_type.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Invalid)) {
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }
    if let Some(t) = raw_type
        && let TypeKind::TypeParameter { param, .. } = *ctx.ty(t)
    {
        // If the type of the function is a type parameter, the tearoff is
        // disallowed, reported in [_resolveDisallowedExpression]. Use the
        // type parameter's bound here in an attempt to assign the intended
        // types.
        raw_type = ctx.type_parameter_bound(param);
    }

    match raw_type.map(|t| (t, *ctx.ty(t))) {
        Some((raw_type, TypeKind::Function(f))) => {
            // A FunctionReference with type arguments and with a
            // ConstructorReference child is invalid. E.g. `List.filled<int>`.
            // [CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR]
            // is reported elsewhere; don't check type arguments here.
            let function = rv.ast[node].function;
            if rv.ast.is::<ConstructorReference>(function) {
                rv.record_static_type(node, TypeId::INVALID);
            } else {
                match rv.ast[node].type_arguments {
                    None => rv.record_static_type(node, raw_type),
                    Some(type_arguments) => {
                        let count = ctx.list(f.type_params).len();
                        let type_argument_types =
                            check_type_arguments(rv, type_arguments, count, target);
                        let invoke_type =
                            ctx.instantiate_function_type(raw_type, &type_argument_types);
                        let list = ctx.intern_list(&type_argument_types);
                        rv.tables.type_arg_types.insert(node, list);
                        rv.record_static_type(node, invoke_type);
                    }
                }
            }
        }
        _ => {
            if rv.is_constructor_tearoffs_enabled() {
                // Only report constructor tearoff-related errors if the
                // constructor tearoff feature is enabled.
                let function = rv.ast[node].function;
                let d = rv.at(diag::disallowed_type_instantiation_expression(), function);
                rv.report(d);
                rv.record_static_type(node, TypeId::INVALID);
            } else if raw_type.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Dynamic)) {
                rv.record_static_type(node, TypeId::DYNAMIC);
            } else {
                rv.record_static_type(node, TypeId::INVALID);
            }
        }
    }
}

/// Dart `_resolveAsImplicitCallReference(node, callMethod)`: `node<...>` is
/// to be treated as `node.call<...>`.
fn resolve_as_implicit_call_reference(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    call_method: ElemRef,
) {
    let ctx = rv.ctx;
    let call_method_type = member::type_(&ctx, call_method);
    let type_parameter_count = match *ctx.ty(call_method_type) {
        TypeKind::Function(f) => ctx.list(f.type_params).len(),
        _ => 0,
    };
    // `node.typeArguments`, coming from the parser, is never null.
    let type_argument_types = match rv.ast[node].type_arguments {
        Some(type_arguments) => check_type_arguments(
            rv,
            type_arguments,
            type_parameter_count,
            Some(Target::Executable(call_method)),
        ),
        None => vec![TypeId::DYNAMIC; type_parameter_count],
    };
    let function = rv.ast[node].function;
    let type_arguments = rv.ast[node].type_arguments;
    let parent = rv.ast.parent(node);
    let call_reference = rv.ast.add(ImplicitCallReference {
        expression: function,
        type_arguments,
    });
    rv.replace_expression(node.upcast(), call_reference.upcast(), parent);
    rv.set_element(call_reference, Some(call_method));
    let list = ctx.intern_list(&type_argument_types);
    rv.tables.type_arg_types.insert(call_reference, list);
    let instantiated_type = if type_argument_types.len() == type_parameter_count
        && matches!(ctx.ty(call_method_type), TypeKind::Function(_))
    {
        ctx.instantiate_function_type(call_method_type, &type_argument_types)
    } else {
        call_method_type
    };
    rv.record_static_type(call_reference, instantiated_type);
}

/// Dart `_resolveConstructorReference(node)`.
fn resolve_constructor_reference(rv: &mut ResolverVisitor<'_>, node: Id<FunctionReference>) {
    // TODO(srawlins): Rewrite and resolve [node] as a constructor reference.
    let function = rv.ast[node].function;
    rv.visit_node(function.raw());
    rv.set_static_type(node, TypeId::DYNAMIC);
}

/// Dart `_resolveDirectTypeLiteral(node, name, element)`: resolves [node] as
/// a [TypeLiteral] referencing an interface type directly (not through a
/// type alias).
fn resolve_direct_type_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    name: Id<Identifier>,
    element: dartr_element::EId<InterfaceElement>,
) {
    let ctx = rv.ctx;
    let count = ctx.interface_type_parameters(element).len();
    let type_arguments = match rv.ast[node].type_arguments {
        // `node.typeArguments`, coming from the parser, is never null.
        Some(list) => {
            check_type_arguments(rv, list, count, Some(Target::TypeDefining(element.raw())))
        }
        None => vec![TypeId::DYNAMIC; count],
    };
    let ty = ctx.instantiate_interface(element, &type_arguments, Nullability::None);
    resolve_type_literal(rv, node, ty, name);
}

/// Dart `_resolveDisallowedExpression(node, rawType)`: resolves [node] as a
/// type instantiation on an illegal expression.
fn resolve_disallowed_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    raw_type: Option<TypeId>,
) {
    if rv.is_constructor_tearoffs_enabled() {
        // Only report constructor tearoff-related errors if the constructor
        // tearoff feature is enabled.
        let function = rv.ast[node].function;
        let d = rv.at(diag::disallowed_type_instantiation_expression(), function);
        rv.report(d);
    }
    let target = Target::function_typed_or_null(rv, raw_type);
    resolve_type(rv, node, raw_type, target);
}

/// Dart `_resolveExtensionOverride(node, function, override)`.
fn resolve_extension_override(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    function: Id<PropertyAccess>,
    override_: Id<ExtensionOverride>,
) {
    let property_name = rv.ast[function].property_name;
    let name = rv.lexeme(rv.ast[property_name].token).to_string();
    let Some(member) = get_override_member(rv, override_, &name) else {
        rv.record_static_type(node, TypeId::INVALID);
        return;
    };
    let ctx = rv.ctx;
    if member::is_static(&ctx, member) {
        let d = rv.at(
            diag::extension_override_access_to_static_member(),
            property_name,
        );
        rv.report(d);
        // Continue to resolve type.
    }
    if crate::ast_ext::property_access_is_cascaded(rv.ast, function) {
        let d = rv.at_token(
            diag::extension_override_with_cascade(),
            rv.ast[override_].name,
        );
        rv.report(d);
        // Continue to resolve type.
    }
    let base = member::base_element(&ctx, member);
    if matches!(base.tag(), Tag::Getter | Tag::Setter) {
        let raw_type = member::return_type(&ctx, member);
        resolve_type(rv, node, Some(raw_type), Some(Target::Executable(member)));
        return;
    }
    let raw_type = member::type_(&ctx, member);
    resolve_type(rv, node, Some(raw_type), Some(Target::Executable(member)));
}

/// Dart `ExtensionMemberResolver.getOverrideMember(node, name).getter2`:
/// the getter or method [name] of the extension of [node], substituted
/// with the type arguments of the override.
///
/// TEMPORARY: `extension_member_resolver.rs` (unit C6) owns the Dart method;
/// this is its getter part.
fn get_override_member(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    name: &str,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    let extension = rv.base_element(node)?.cast::<ExtensionElement>()?;
    let instance = extension.upcast::<InstanceElement>();
    let element = lookup::get_getter(&ctx, instance, name)
        .map(|g| g.raw())
        .or_else(|| lookup::get_method(&ctx, instance, name).map(|m| m.raw()))?;
    let type_parameters = ctx.get(extension).type_params.clone();
    let type_arguments = rv
        .tables
        .type_arg_types
        .get(node)
        .map(|&l| ctx.list(l).to_vec());
    match type_arguments {
        Some(args) if !type_parameters.is_empty() && args.len() == type_parameters.len() => {
            let substitution = dartr_typesystem::type_algebra::MapSubstitution::from_pairs(
                &type_parameters,
                &args,
            );
            Some(member::substitute(
                &ctx,
                ElemRef::Base(element),
                &substitution,
            ))
        }
        _ => Some(ElemRef::Base(element)),
    }
}

/// Dart `_resolveFunctionTypeFunction(receiver, methodName, receiverType)`:
/// resolves a possible function tearoff of a function typed receiver.
fn resolve_function_type_function(
    rv: &mut ResolverVisitor<'_>,
    receiver: Id<Expression>,
    method_name: Id<SimpleIdentifier>,
    receiver_type: TypeId,
) -> Option<ElemRef> {
    let name = rv.lexeme(rv.ast[method_name].token).to_string();
    let method_element = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: Some(receiver),
            receiver_type,
            name: &name,
            has_read: true,
            has_write: false,
            property_error_entity: method_name.raw(),
            name_error_entity: method_name.raw(),
            parent_node: None,
        },
    )
    .getter;
    if let Some(method_element) = method_element
        && member::is_static(&rv.ctx, method_element)
    {
        report_invalid_access_to_static_member(rv, method_name, method_element, false);
    }
    method_element
}

/// Dart `_resolvePrefixedIdentifierFunction(node, function)`.
fn resolve_prefixed_identifier_function(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    function: Id<PrefixedIdentifier>,
) {
    let ctx = rv.ctx;
    rv.resolve_expression(function.upcast(), TypeId::UNKNOWN);
    // The prefixed identifier is not rewritten here (Dart reads `function`
    // after `popRewrite()`).
    let property_type = rv.static_type(function).unwrap_or(TypeId::DYNAMIC);
    let identifier = rv.ast[function].identifier;
    let function_element = rv.base_element(identifier);

    if function_element.is_some_and(|e| e.tag() == Tag::Extension) {
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }
    if matches!(ctx.ty(property_type), TypeKind::Invalid) {
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }
    if matches!(ctx.ty(property_type), TypeKind::Dynamic) {
        let d = rv.at(
            diag::generic_method_type_instantiation_on_dynamic(),
            function,
        );
        rv.report(d);
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }

    if let Some(call_method) = get_call_method(rv, node, Some(property_type))
        && is_method(call_method, rv)
    {
        resolve_as_implicit_call_reference(rv, node, call_method);
        return;
    }

    if matches!(ctx.ty(property_type), TypeKind::Function(_)) {
        rv.set_static_type(function, property_type);
        resolve_type(
            rv,
            node,
            Some(property_type),
            Some(Target::FunctionTypedExpression(property_type)),
        );
        return;
    }

    let prefix = rv.ast[function].prefix;
    if rv
        .base_element(prefix)
        .is_some_and(|e| e.tag() == Tag::Prefix)
        && let Some(function_element) = rv.element(identifier)
    {
        resolve_receiver_prefix(
            rv,
            node,
            function,
            member::base_element(&ctx, function_element),
        );
        return;
    }

    let d = rv.at(diag::disallowed_type_instantiation_expression(), identifier);
    rv.report(d);
    rv.record_static_type(node, TypeId::INVALID);
}

/// Dart `PropertyAccessImpl.realTarget`.
fn property_access_real_target(
    rv: &ResolverVisitor<'_>,
    node: Id<PropertyAccess>,
) -> Option<Id<Expression>> {
    if crate::ast_ext::property_access_is_cascaded(rv.ast, node) {
        let mut current = rv.ast.parent(node);
        while let Some(p) = current {
            if let Some(c) = rv.ast.cast::<dartr_ast::CascadeExpression>(p) {
                return Some(rv.ast[c].target);
            }
            current = rv.ast.parent(p);
        }
        return None;
    }
    rv.ast[node].target
}

/// Dart `_resolvePropertyAccessFunction(node, function)`.
fn resolve_property_access_function(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    function: Id<PropertyAccess>,
) {
    let ctx = rv.ctx;
    rv.resolve_expression(function.upcast(), TypeId::UNKNOWN);
    let function_static_type = rv.static_type(function);
    if let Some(call_method) = get_call_method(rv, node, function_static_type)
        && is_method(call_method, rv)
    {
        resolve_as_implicit_call_reference(rv, node, call_method);
        return;
    }
    let Some(target) = property_access_real_target(rv, function) else {
        rv.set_static_type(node, TypeId::DYNAMIC);
        return;
    };

    let target_type;
    if rv.ast.is::<SuperExpression>(target) || rv.ast.is::<ThisExpression>(target) {
        target_type = rv.static_type(target).unwrap_or(TypeId::DYNAMIC);
    } else if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(target) {
        let target_element = rv
            .rt
            .scope_lookup_result
            .get(identifier)
            .and_then(|r| r.getter);
        match target_element {
            Some(e) if e.is::<dartr_element::VariableElement>() => {
                target_type = crate::element_ext::variable_type(&ctx, e);
            }
            Some(e) if matches!(e.tag(), Tag::Getter | Tag::Setter) => {
                target_type = member::variable(&ctx, ElemRef::Base(e))
                    .map(|v| member::type_(&ctx, v))
                    .unwrap_or(TypeId::DYNAMIC);
            }
            _ => {
                // TODO(srawlins): Can we get here?
                rv.set_static_type(node, TypeId::DYNAMIC);
                return;
            }
        }
    } else if let Some(override_) = rv.ast.cast::<ExtensionOverride>(target) {
        resolve_extension_override(rv, node, function, override_);
        return;
    } else {
        let target_type = rv.static_type(target);
        match target_type.map(|t| ctx.ty(t)) {
            Some(TypeKind::Dynamic) => {
                let d = rv.at(diag::generic_method_type_instantiation_on_dynamic(), node);
                rv.report(d);
                rv.record_static_type(node, TypeId::INVALID);
                return;
            }
            Some(TypeKind::Invalid) => {
                rv.record_static_type(node, TypeId::INVALID);
                return;
            }
            _ => {}
        }
        let property_name = rv.ast[function].property_name;
        let function_type = resolve_type_property(rv, target, property_name, function.raw());
        match function_type {
            Some(t) if matches!(ctx.ty(t), TypeKind::Function(_)) => {
                rv.set_static_type(function, t);
                resolve_type(rv, node, Some(t), Some(Target::FunctionTypedExpression(t)));
                return;
            }
            Some(_) => {
                // If the property is unknown, [UNDEFINED_GETTER] is reported
                // elsewhere. If it is known, we must report the bad type
                // instantiation here.
                let d = rv.at(
                    diag::disallowed_type_instantiation_expression(),
                    property_name,
                );
                rv.report(d);
            }
            None => {}
        }
        rv.record_static_type(node, TypeId::INVALID);
        return;
    }

    let property_name = rv.ast[function].property_name;
    let name = rv.lexeme(rv.ast[property_name].token).to_string();
    let property_element = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: Some(target),
            receiver_type: target_type,
            name: &name,
            has_read: true,
            has_write: false,
            property_error_entity: property_name.raw(),
            name_error_entity: function.raw(),
            parent_node: None,
        },
    )
    .getter;
    let raw_type = rv.static_type(function);
    resolve_type(rv, node, raw_type, property_element.map(Target::Executable));
}

/// Dart `_resolveReceiverPrefix(node, prefixElement, prefix, element)`.
fn resolve_receiver_prefix(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    prefix: Id<PrefixedIdentifier>,
    element: ElementId,
) {
    let ctx = rv.ctx;
    let mut element = element;
    if let Some(multiply) = element.cast::<dartr_element::MultiplyDefinedElement>() {
        if let Some(&first) = ctx.get(multiply).conflicting_elements.first() {
            element = first;
        }
        // TODO(srawlins): Add a resolution test for this case.
    }

    // Classes and type aliases are checked first so as to include a
    // PropertyAccess parent check, which does not need to be done for
    // functions.
    if element.is::<InterfaceElement>() || element.tag() == Tag::TypeAlias {
        // A type-instantiated constructor tearoff like `prefix.C<int>.name`
        // is initially represented as a [PropertyAccess] with a
        // [FunctionReference] 'target'.
        if rv
            .ast
            .parent(node)
            .is_some_and(|p| rv.ast.is::<PropertyAccess>(p))
        {
            resolve_constructor_reference(rv, node);
        } else if let Some(interface) = element.cast::<InterfaceElement>() {
            resolve_direct_type_literal(rv, node, prefix.upcast(), interface);
        } else if let Some(alias) = element.cast::<TypeAliasElement>() {
            resolve_type_alias(rv, node, alias, prefix.upcast());
        }
        return;
    } else if element.tag() == Tag::Extension {
        let identifier = rv.ast[prefix].identifier;
        rv.set_element(identifier, Some(ElemRef::Base(element)));
        rv.set_static_type(identifier, TypeId::INVALID);
        rv.set_static_type(prefix, TypeId::INVALID);
        resolve_disallowed_expression(rv, node, Some(TypeId::INVALID));
        return;
    }

    // Dart asserts here: member of a prefixed element is not a class,
    // mixin, type alias, or executable element.
    rv.set_static_type(node, TypeId::INVALID);
}

/// Dart `_resolveSimpleIdentifierFunction(node, function)`.
fn resolve_simple_identifier_function(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    function: Id<SimpleIdentifier>,
) {
    let ctx = rv.ctx;
    let element = rv
        .rt
        .scope_lookup_result
        .get(function)
        .and_then(|r| r.getter);
    let name = rv.lexeme(rv.ast[function].token).to_string();

    let Some(element) = element else {
        let Some(receiver_type) = rv.this_type() else {
            let d = rv.at(diag::undefined_identifier(&name), function);
            rv.report(d);
            rv.set_static_type(function, TypeId::INVALID);
            rv.record_static_type(node, TypeId::INVALID);
            return;
        };

        let result = type_property_resolver::resolve(
            rv,
            PropertyQuery {
                receiver: None,
                receiver_type,
                name: &name,
                has_read: true,
                has_write: false,
                property_error_entity: function.raw(),
                name_error_entity: function.raw(),
                parent_node: None,
            },
        );

        match result.getter {
            Some(method) => {
                if member::is_static(&ctx, method) {
                    report_invalid_access_to_static_member(rv, function, method, true);
                    // Continue to assign types.
                }
                if matches!(
                    member::base_element(&ctx, method).tag(),
                    Tag::Getter | Tag::Setter
                ) {
                    rv.set_element(function, Some(method));
                    let return_type = member::return_type(&ctx, method);
                    rv.set_static_type(function, return_type);
                    let raw_type = member::variable(&ctx, method).map(|v| member::type_(&ctx, v));
                    resolve_type(rv, node, raw_type, Some(Target::Executable(method)));
                    return;
                }
                rv.set_element(function, Some(method));
                let method_type = member::type_(&ctx, method);
                rv.set_static_type(function, method_type);
                resolve_type(
                    rv,
                    node,
                    Some(method_type),
                    Some(Target::Executable(method)),
                );
            }
            None => {
                let type_name =
                    dartr_element::diagnostics::type_display_string(&ctx, receiver_type, true);
                let d = rv.at(diag::undefined_method(&name, &type_name), function);
                rv.report(d);
                rv.set_static_type(function, TypeId::INVALID);
                rv.record_static_type(node, TypeId::INVALID);
            }
        }
        return;
    };

    let element_ref = ElemRef::Base(element);
    // Classes and type aliases are checked first so as to include a
    // PropertyAccess parent check, which does not need to be done for
    // functions.
    if element.is::<InterfaceElement>() || element.tag() == Tag::TypeAlias {
        // A type-instantiated constructor tearoff like `C<int>.name` or
        // `prefix.C<int>.name` is initially represented as a
        // [PropertyAccess] with a [FunctionReference] target.
        if rv
            .ast
            .parent(node)
            .is_some_and(|p| rv.ast.is::<PropertyAccess>(p))
        {
            let alias_of_function = element.cast::<TypeAliasElement>().is_some_and(|alias| {
                let aliased = ctx.get(alias).aliased_type.get().unwrap_or(TypeId::INVALID);
                matches!(ctx.ty(aliased), TypeKind::Function(_))
            });
            if alias_of_function {
                rv.set_element(function, Some(element_ref));
                resolve_type_alias(rv, node, element.cast().unwrap(), function.upcast());
            } else {
                resolve_constructor_reference(rv, node);
            }
        } else if let Some(interface) = element.cast::<InterfaceElement>() {
            rv.set_element(function, Some(element_ref));
            resolve_direct_type_literal(rv, node, function.upcast(), interface);
        } else if let Some(alias) = element.cast::<TypeAliasElement>() {
            rv.set_element(function, Some(element_ref));
            resolve_type_alias(rv, node, alias, function.upcast());
        }
        return;
    }
    match element.tag() {
        Tag::Method | Tag::LocalFunction | Tag::TopLevelFunction => {
            rv.set_element(function, Some(element_ref));
            let ty = member::type_(&ctx, element_ref);
            rv.set_static_type(function, ty);
            resolve_type(rv, node, Some(ty), Some(Target::Executable(element_ref)));
        }
        Tag::Getter | Tag::Setter => {
            rv.set_element(function, Some(element_ref));
            let variable_type = member::variable(&ctx, element_ref)
                .map(|v| member::type_(&ctx, v))
                .unwrap_or(TypeId::INVALID);
            rv.set_static_type(function, variable_type);
            if let Some(call_method) = get_call_method(rv, node, Some(variable_type))
                && is_method(call_method, rv)
            {
                resolve_as_implicit_call_reference(rv, node, call_method);
                return;
            }
            let return_type = member::return_type(&ctx, element_ref);
            resolve_type(
                rv,
                node,
                Some(return_type),
                Some(Target::Executable(element_ref)),
            );
        }
        Tag::Constructor => {
            rv.set_element(function, Some(element_ref));
            let ty = member::type_(&ctx, element_ref);
            rv.set_static_type(function, ty);
            resolve_type(rv, node, Some(ty), Some(Target::Executable(element_ref)));
        }
        _ if element.is::<dartr_element::VariableElement>() => {
            rv.set_element(function, Some(element_ref));
            let ty = crate::element_ext::variable_type(&ctx, element);
            rv.set_static_type(function, ty);
            if let Some(call_method) = get_call_method(rv, node, Some(ty))
                && is_method(call_method, rv)
            {
                resolve_as_implicit_call_reference(rv, node, call_method);
                return;
            }
            let target = Target::function_typed_or_null(rv, Some(ty));
            resolve_type(rv, node, Some(ty), target);
        }
        Tag::Extension => {
            rv.set_element(function, Some(element_ref));
            rv.set_static_type(function, TypeId::INVALID);
            resolve_disallowed_expression(rv, node, Some(TypeId::INVALID));
        }
        _ => resolve_disallowed_expression(rv, node, Some(TypeId::DYNAMIC)),
    }
}

/// Dart `_resolveStaticElement(classElement, propertyName)`: the element
/// that represents the property named [property_name] on [class_element].
fn resolve_static_element(
    rv: &ResolverVisitor<'_>,
    class_element: dartr_element::EId<InterfaceElement>,
    property_name: Id<SimpleIdentifier>,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    let name = rv.lexeme(rv.ast[property_name].token);
    let instance = class_element.upcast::<InstanceElement>();
    let mut element = None;
    if crate::ast_ext::simple_identifier_in_setter_context(rv.ast, property_name) {
        element = lookup::get_setter(&ctx, instance, name).map(|s| s.raw());
    }
    let element = element
        .or_else(|| lookup::get_getter(&ctx, instance, name).map(|g| g.raw()))
        .or_else(|| lookup::get_method(&ctx, instance, name).map(|m| m.raw()))?;
    let element = ElemRef::Base(element);
    member::is_accessible_in(&ctx, element, rv.unit.library).then_some(element)
}

/// Dart `_resolveTypeAlias(node:, element:, typeAlias:)`.
fn resolve_type_alias(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    element: dartr_element::EId<TypeAliasElement>,
    type_alias: Id<Identifier>,
) {
    let ctx = rv.ctx;
    let count = ctx.get(element).type_params.len();
    let type_arguments = match rv.ast[node].type_arguments {
        // `node.typeArguments`, coming from the parser, is never null.
        Some(list) => {
            check_type_arguments(rv, list, count, Some(Target::TypeDefining(element.raw())))
        }
        None => vec![TypeId::DYNAMIC; count],
    };
    let ty = ctx.instantiate_type_alias(element, &type_arguments, Nullability::None);
    resolve_type_literal(rv, node, ty, type_alias);
}

/// Dart `_resolveTypeLiteral(node:, instantiatedType:, name:)`: rewrites
/// [node] as a `TypeLiteral` of a named type made from [name] and the type
/// arguments of [node].
fn resolve_type_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionReference>,
    instantiated_type: TypeId,
    name: Id<Identifier>,
) {
    // Dart `name.toNamedType(typeArguments: node.typeArguments, question:
    // null)`: the named type and its import prefix get the elements of the
    // identifiers.
    let (prefix_element, name_element) = match rv.ast.cast::<PrefixedIdentifier>(name) {
        Some(p) => {
            let prefix = rv.ast[p].prefix;
            let identifier = rv.ast[p].identifier;
            (rv.element(prefix), rv.element(identifier))
        }
        None => (None, rv.element(name)),
    };
    let type_arguments = rv.ast[node].type_arguments;
    let type_name = rv.ast.identifier_to_named_type(name, type_arguments, None);
    if let Some(import_prefix) = rv.ast[type_name].import_prefix {
        set_import_prefix_element(rv, import_prefix, prefix_element);
    }
    rv.set_element(type_name, name_element);
    rv.tables
        .annotation_type
        .insert(type_name, instantiated_type);
    let type_literal = rv.ast.add(TypeLiteral { type_: type_name });
    let parent = rv.ast.parent(node);
    rv.replace_expression(node.upcast(), type_literal.upcast(), parent);
    let type_type = rv.ctx.tp.type_type();
    rv.record_static_type(type_literal, type_type);
}

/// Dart `ImportPrefixReference.element = element`.
fn set_import_prefix_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ImportPrefixReference>,
    element: Option<ElemRef>,
) {
    rv.set_element(node, element);
}

/// Dart `_resolveTypeProperty(receiver:, name:, nameErrorEntity:)`:
/// resolves [name] as a property on [receiver]. Returns `None` if
/// [receiver]'s type is `null`, a type parameter type, or a type alias for
/// a non-interface type.
fn resolve_type_property(
    rv: &mut ResolverVisitor<'_>,
    receiver: Id<Expression>,
    name: Id<SimpleIdentifier>,
    name_error_entity: NodeId,
) -> Option<TypeId> {
    let ctx = rv.ctx;
    if rv.ast.is::<Identifier>(receiver) {
        let receiver_element = receiver_identifier_element(rv, receiver);
        if let Some(interface) = receiver_element.and_then(|e| e.cast::<InterfaceElement>()) {
            let element = resolve_static_element(rv, interface, name);
            rv.set_element(name, element);
            return element.and_then(|e| reference_type(rv, e));
        } else if let Some(alias) = receiver_element.and_then(|e| e.cast::<TypeAliasElement>()) {
            let aliased_type = ctx.get(alias).aliased_type.get().unwrap_or(TypeId::INVALID);
            return match ctx.interface_element(aliased_type) {
                Some(interface) => {
                    let element = resolve_static_element(rv, interface, name);
                    rv.set_element(name, element);
                    element.and_then(|e| reference_type(rv, e))
                }
                None => None,
            };
        }
    }

    let receiver_type = rv.static_type(receiver)?;
    match *ctx.ty(receiver_type) {
        TypeKind::TypeParameter { .. } => return None,
        TypeKind::Function(_) => {
            if rv.lexeme(rv.ast[name].token) == "call" {
                return Some(receiver_type);
            }
            let element = resolve_function_type_function(rv, receiver, name, receiver_type);
            rv.set_element(name, element);
            return element.and_then(|e| reference_type(rv, e));
        }
        _ => {}
    }

    let name_str = rv.lexeme(rv.ast[name].token).to_string();
    let element = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: Some(receiver),
            receiver_type,
            name: &name_str,
            has_read: true,
            has_write: false,
            property_error_entity: name.raw(),
            name_error_entity,
            parent_node: None,
        },
    )
    .getter;
    rv.set_element(name, element);
    if let Some(element) = element
        && member::is_static(&ctx, element)
    {
        report_invalid_access_to_static_member(rv, name, element, false);
    }
    element.and_then(|e| reference_type(rv, e))
}

/// The element of an identifier receiver (Dart `receiver.element` of an
/// `IdentifierImpl`: the element of a simple identifier, the element of
/// the identifier of a prefixed identifier).
fn receiver_identifier_element(
    rv: &ResolverVisitor<'_>,
    receiver: Id<Expression>,
) -> Option<ElementId> {
    match rv.ast.cast::<PrefixedIdentifier>(receiver) {
        Some(p) => rv.base_element(rv.ast[p].identifier),
        None => rv.base_element(receiver),
    }
}

/// Dart `Element.referenceType`: the type of [element] when accessed as a
/// reference, not immediately followed by parentheses and arguments.
fn reference_type(rv: &ResolverVisitor<'_>, element: ElemRef) -> Option<TypeId> {
    let ctx = rv.ctx;
    let base = member::base_element(&ctx, element);
    match base.tag() {
        Tag::Constructor | Tag::TopLevelFunction | Tag::LocalFunction | Tag::Method => {
            Some(member::type_(&ctx, element))
        }
        Tag::Getter | Tag::Setter => Some(member::return_type(&ctx, element)),
        _ if base.is::<dartr_element::VariableElement>() => Some(member::type_(&ctx, element)),
        _ => None,
    }
}

/// Unused import guard.
#[allow(dead_code)]
fn _t(_: dartr_element::EId<TypeParameterElement>) {}
