// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (visitDotShorthandInvocation, visitDotShorthandPropertyAccess, _resolvePropertyAccessRhs_common), pkg/analyzer/lib/src/dart/resolver/method_invocation_resolver.dart (resolveDotShorthand and its helpers), pkg/analyzer/lib/src/dart/resolver/property_element_resolver.dart (resolveDotShorthand, _resolveTargetInterfaceElement for a read)

//! Dot shorthands: `.foo` (a static getter, a constructor tear-off),
//! `.foo(...)` (a static method, or a constructor: rewritten to a
//! `DotShorthandConstructorInvocation`), `.new(...)` / `.named(...)`
//! (`instance_creation_expression_resolver`). The member is looked up in
//! the context type (the type schema of the enclosing dot shorthand, Dart
//! `getDotShorthandContext`).

use dartr_ast::{
    DotShorthandConstructorInvocation, DotShorthandInvocation, DotShorthandPropertyAccess,
    Expression, FunctionExpressionInvocation, Id, SimpleIdentifier,
};
use dartr_diagnostics::diag;
use dartr_element::{ElemRef, InstanceElement, InterfaceElement, Tag, TypeId, TypeKind};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::{TypeExt, lookup, member};

use crate::instance_creation_expression_resolver::constructor_element_to_infer;
use crate::invocation_inference_helper::infer_tear_off;
use crate::invocation_inferrer::{InferrerKind, InvocationInferrer, InvocationTarget};
use crate::resolver::{ResolverVisitor, SchemaOf};

/// Dart `ResolverVisitor.visitDotShorthandConstructorInvocation(node,
/// contextType:)`.
pub fn visit_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
    context_type: TypeId,
) {
    crate::instance_creation_expression_resolver::visit_dot_shorthand_constructor_invocation(
        rv,
        node,
        context_type,
    );
}

/// Dart `ResolverVisitor.visitDotShorthandInvocation(node, contextType:)`.
pub fn visit_dot_shorthand_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SchemaOf::new(context_type));
    }

    rv.check_unreachable_node(node);
    let type_arguments = rv.ast[node].type_arguments;
    rv.visit_opt(type_arguments);
    let rewritten_expression = resolve_dot_shorthand_invocation(rv, node, context_type);

    // TODO(paulberry): why don't we do this for
    // DotShorthandConstructorInvocationImpl?
    let rewritten_to_constructor =
        rewritten_expression.is_some_and(|e| rv.ast.is::<DotShorthandConstructorInvocation>(e));
    if !rewritten_to_constructor {
        let replacement = rv.insert_generic_function_instantiation(node.upcast(), context_type);
        // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
        rv.insert_implicit_call_reference(replacement, context_type);
    }

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `ResolverVisitor.visitDotShorthandPropertyAccess(node,
/// contextType:)`.
pub fn visit_dot_shorthand_property_access(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandPropertyAccess>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SchemaOf::new(context_type));
    }

    rv.check_unreachable_node(node);
    let result = resolve_dot_shorthand_property_access(rv, node, context_type);
    let property_name = rv.ast[node].property_name;
    resolve_property_access_rhs_common(rv, result, node.upcast(), property_name, context_type);

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// The dot shorthand context type (Dart
/// `getDotShorthandContext().unwrapTypeSchemaView()`), `_` when there is
/// none (Dart asserts that there is one).
fn dot_shorthand_context(rv: &mut ResolverVisitor<'_>) -> TypeId {
    if rv.is_dot_shorthand_context_empty() {
        return TypeId::UNKNOWN;
    }
    rv.get_dot_shorthand_context().unwrap_type_schema_view()
}

/// The interface element of the dot shorthand context, after `FutureOr<S>`
/// is replaced by `S`, if it is accessible in the library.
fn accessible_context_element(
    rv: &mut ResolverVisitor<'_>,
) -> (TypeId, Option<dartr_element::EId<InterfaceElement>>) {
    let context = dot_shorthand_context(rv);
    // The static namespace denoted by `S` is also the namespace denoted by
    // `FutureOr<S>`.
    let context = rv.type_system.future_or_base(context);
    let library = rv.unit.library;
    let element = rv
        .ctx
        .interface_element(context)
        .filter(|&e| member::is_accessible_in(&rv.ctx, ElemRef::Base(e.raw()), library));
    (context, element)
}

// ------------------------------------------------------------ invocations

/// Dart `MethodInvocationResolver.resolveDotShorthand(node,
/// whyNotPromotedArguments, contextType:)`. Returns the node that replaces
/// [node] (a `FunctionExpressionInvocation` or a
/// `DotShorthandConstructorInvocation`), if any.
fn resolve_dot_shorthand_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    context_type: TypeId,
) -> Option<Id<Expression>> {
    let (_, element) = accessible_context_element(rv);
    if let Some(element) = element {
        return resolve_receiver_type_literal_for_dot_shorthand(rv, node, element, context_type);
    }

    let member_name = rv.ast[node].member_name;
    let name = rv.lexeme(rv.ast[member_name].token).to_string();
    let context_display =
        dartr_element::diagnostics::type_display_string(&rv.ctx, context_type, true);
    let d = rv.at(
        diag::dot_shorthand_undefined_invocation(&name, &context_display),
        member_name,
    );
    rv.report(d);
    set_invalid_type_resolution_for_dot_shorthand(rv, node, false, context_type);
    None
}

/// Dart `MethodInvocationResolver._resolveElement(classElement,
/// propertyName)` (also `FunctionReferenceResolver._resolveStaticElement`).
fn resolve_element(
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

/// Dart `_resolveReceiverTypeLiteralForDotShorthand(node, receiver,
/// nameNode, name, whyNotPromotedArguments, contextType:)`.
fn resolve_receiver_type_literal_for_dot_shorthand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    receiver: dartr_element::EId<InterfaceElement>,
    context_type: TypeId,
) -> Option<Id<Expression>> {
    let ctx = rv.ctx;
    let name_node = rv.ast[node].member_name;
    let name = rv.lexeme(rv.ast[name_node].token).to_string();
    let element = resolve_element(rv, receiver, name_node);
    if let Some(element) = element
        && member::base_element(&ctx, element).is::<dartr_element::ExecutableElement>()
        && member::is_static(&ctx, element)
    {
        rv.set_element(name_node, Some(element));
        if matches!(
            member::base_element(&ctx, element).tag(),
            Tag::Getter | Tag::Setter
        ) {
            let return_type = member::return_type(&ctx, element);
            let invocation =
                rewrite_as_function_expression_invocation(rv, node, return_type, context_type);
            return Some(invocation.upcast());
        }
        let ty = member::type_(&ctx, element);
        set_resolution_for_dot_shorthand(rv, node, ty, context_type, element);
        return None;
    }
    let library = rv.unit.library;
    if let Some(constructor) = lookup::get_named_constructor(&ctx, receiver, &name)
        .map(|c| ElemRef::Base(c.raw()))
        .filter(|&c| member::is_accessible_in(&ctx, c, library))
    {
        // The dot shorthand is a constructor invocation so we rewrite to a
        // [DotShorthandConstructorInvocation].
        let replacement = rv.ast.add(DotShorthandConstructorInvocation {
            const_keyword: None,
            period: rv.ast[node].period,
            constructor_name: name_node,
            type_arguments: rv.ast[node].type_arguments,
            argument_list: rv.ast[node].argument_list,
        });
        rv.set_element(replacement, Some(constructor));
        if rv.rt.is_dot_shorthand(node.raw()) {
            rv.rt.dot_shorthand.insert(replacement, ());
        }
        let parent = rv.ast.parent(node);
        rv.replace_expression(node.upcast(), replacement.upcast(), parent);
        crate::instance_creation_expression_resolver::resolve_dot_shorthand(
            rv,
            replacement,
            context_type,
        );
        return Some(replacement.upcast());
    }

    let receiver_name = ctx.element_name(receiver.raw()).unwrap_or("").to_string();
    let d = rv.at(
        diag::dot_shorthand_undefined_invocation(&name, &receiver_name),
        name_node,
    );
    rv.report(d);
    set_invalid_type_resolution_for_dot_shorthand(rv, node, element.is_none(), context_type);
    None
}

/// Dart `_rewriteAsFunctionExpressionInvocation(node, null, node.period,
/// node.memberName, node.typeArguments, node.argumentList,
/// getterReturnType, isCascaded: false, ...)` for a dot shorthand
/// invocation of a static getter.
fn rewrite_as_function_expression_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    getter_return_type: TypeId,
    context_type: TypeId,
) -> Id<FunctionExpressionInvocation> {
    let target_type = getter_return_type;
    let member_name = rv.ast[node].member_name;
    let function_expression = rv.ast.add(DotShorthandPropertyAccess {
        period: rv.ast[node].period,
        property_name: member_name,
    });
    // The getter is static: Dart's `flow.propertyGet` applies to instance
    // members only.
    rv.record_static_type(member_name, target_type);
    rv.set_static_type(function_expression, target_type);

    let invocation = rv.ast.add(FunctionExpressionInvocation {
        function: function_expression.upcast(),
        type_arguments: rv.ast[node].type_arguments,
        argument_list: rv.ast[node].argument_list,
    });
    let parent = rv.ast.parent(node);
    rv.replace_expression(node.upcast(), invocation.upcast(), parent);
    // Dart `functionExpressionInvocationResolver.resolve(invocation, ...)`
    // (unit C3).
    crate::function_expression_invocation_resolver::visit_function_expression_invocation(
        rv,
        invocation,
        context_type,
    );
    invocation
}

/// Dart `_setExplicitTypeArgumentTypes()`.
fn set_explicit_type_argument_types(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
) {
    let types: Vec<TypeId> = match rv.ast[node].type_arguments {
        Some(list) => rv
            .ast
            .list(rv.ast[list].arguments)
            .iter()
            .map(|&a| {
                rv.tables
                    .annotation_type
                    .get(a)
                    .copied()
                    .unwrap_or(TypeId::DYNAMIC)
            })
            .collect(),
        None => Vec::new(),
    };
    let list = rv.ctx.intern_list(&types);
    rv.tables.type_arg_types.insert(node, list);
}

/// Dart `_setDynamicTypeResolutionForDotShorthand(node,
/// setNameTypeToDynamic:, ...)`.
fn set_dynamic_type_resolution_for_dot_shorthand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    set_name_type_to_dynamic: bool,
    context_type: TypeId,
) {
    let member_name = rv.ast[node].member_name;
    if set_name_type_to_dynamic {
        rv.set_static_type(member_name, TypeId::DYNAMIC);
    }
    rv.tables.invoke_type.insert(node, TypeId::DYNAMIC);
    rv.set_static_type(node, TypeId::DYNAMIC);
    set_explicit_type_argument_types(rv, node);
    resolve_arguments_finish_dot_shorthand_inference(rv, node, context_type, None);
}

/// Dart `_setInvalidTypeResolutionForDotShorthand(node,
/// setNameTypeToDynamic:, ...)`.
fn set_invalid_type_resolution_for_dot_shorthand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    set_name_type_to_dynamic: bool,
    context_type: TypeId,
) {
    let member_name = rv.ast[node].member_name;
    if set_name_type_to_dynamic {
        rv.set_static_type(member_name, TypeId::INVALID);
    }
    set_explicit_type_argument_types(rv, node);
    resolve_arguments_finish_dot_shorthand_inference(rv, node, context_type, None);
    rv.tables.invoke_type.insert(node, TypeId::INVALID);
    rv.set_static_type(node, TypeId::INVALID);
}

/// Dart `_setResolutionForDotShorthand(node, type, whyNotPromotedArguments,
/// contextType:, target:)`.
fn set_resolution_for_dot_shorthand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    ty: TypeId,
    context_type: TypeId,
    target: ElemRef,
) {
    let ctx = rv.ctx;
    let member_name = rv.ast[node].member_name;
    // TODO(scheglov): We need this for StaticTypeAnalyzer to run inference.
    // But it seems weird. Do we need to know the raw type of a function?!
    rv.set_static_type(member_name, ty);

    if matches!(ctx.ty(ty), TypeKind::Dynamic) || ctx.is_dart_core_function(ty) {
        set_dynamic_type_resolution_for_dot_shorthand(rv, node, false, context_type);
        return;
    }
    if matches!(ctx.ty(ty), TypeKind::Function(_)) {
        // Dart `inferenceHelper.resolveDotShorthandInvocation(node:, ...)`.
        resolve_arguments_finish_dot_shorthand_inference(
            rv,
            node,
            context_type,
            Some((target, ty)),
        );
        return;
    }
    if matches!(ctx.ty(ty), TypeKind::Void) {
        set_invalid_type_resolution_for_dot_shorthand(rv, node, true, context_type);
        let d = rv.at(diag::use_of_void_result(), member_name);
        rv.report(d);
        return;
    }
    set_invalid_type_resolution_for_dot_shorthand(rv, node, false, context_type);
    let name = rv.lexeme(rv.ast[member_name].token).to_string();
    let d = rv.at(diag::invocation_of_non_function(&name), member_name);
    rv.report(d);
}

/// Dart `_resolveArguments_finishDotShorthandInference(node, ...)` (no
/// target) and `InvocationInferenceHelper.resolveDotShorthandInvocation`
/// (with the invoked element and its type): the
/// `DotShorthandInvocationInferrer`.
fn resolve_arguments_finish_dot_shorthand_inference(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    context_type: TypeId,
    target: Option<(ElemRef, TypeId)>,
) {
    let return_type = InvocationInferrer {
        kind: InferrerKind::DotShorthandInvocation(node),
        argument_list: rv.ast[node].argument_list,
        context_type,
        target: target.map(|(e, _)| InvocationTarget::ExecutableElement(e)),
    }
    .resolve_invocation(rv);
    rv.record_static_type(node, return_type);
}

// ------------------------------------------------------------ properties

/// Dart `PropertyElementResolverResult` (the read part).
#[derive(Clone, Copy, Debug, Default)]
pub struct PropertyResult {
    /// Dart `readElementRequested2`.
    pub read_element_requested: Option<ElemRef>,
    /// Dart `readElementRecovery2`.
    pub read_element_recovery: Option<ElemRef>,
    /// Dart `getType`.
    pub get_type: Option<TypeId>,
}

impl PropertyResult {
    /// Dart `readElement2`.
    fn read_element(&self) -> Option<ElemRef> {
        self.read_element_requested.or(self.read_element_recovery)
    }
}

/// Dart `PropertyElementResolver.resolveDotShorthand(node, contextType:)`.
fn resolve_dot_shorthand_property_access(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandPropertyAccess>,
    context_type: TypeId,
) -> PropertyResult {
    let ctx = rv.ctx;
    let (context, context_element) = accessible_context_element(rv);
    let Some(context_element) = context_element else {
        let d = rv.at(diag::dot_shorthand_missing_context(), node);
        rv.report(d);
        return PropertyResult::default();
    };
    let identifier = rv.ast[node].property_name;
    let name = rv.lexeme(rv.ast[identifier].token).to_string();
    let library = rv.unit.library;
    // Find constructor tearoffs.
    if let Some(element) = lookup::type_look_up_constructor(&ctx, context, Some(&name), library) {
        if !crate::element_resolver::is_factory(rv, element)
            && let Some(enclosing) = member::enclosing_element(&ctx, element)
            && enclosing.tag() == Tag::Class
            && crate::element_ext::first_fragment_flags(&ctx, enclosing)
                .contains(dartr_element::FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT)
        {
            let d = rv.at(
                diag::tearoff_of_generative_constructor_of_abstract_class(),
                node,
            );
            rv.report(d);
        }

        // Infer type parameters.
        let element_to_infer =
            constructor_element_to_infer(rv, Some(context_element.raw()), Some(&name));
        if let Some(element_to_infer) = element_to_infer
            && !element_to_infer.type_parameters.is_empty()
        {
            let as_type = element_to_infer.as_type(rv);
            let inferred = infer_tear_off(rv, node.upcast(), identifier, as_type, context_type);
            let TypeKind::Function(f) = *ctx.ty(inferred) else {
                return PropertyResult::default();
            };
            let base = member::base_element(&ctx, element_to_infer.element);
            rv.set_element(identifier, Some(ElemRef::Base(base)));
            return PropertyResult {
                read_element_requested: Some(ElemRef::Base(base)),
                read_element_recovery: None,
                get_type: Some(f.ret),
            };
        }

        return PropertyResult {
            read_element_requested: Some(element),
            read_element_recovery: None,
            get_type: Some(member::return_type(&ctx, element)),
        };
    }

    // Didn't find any constructor tearoffs, look for static getters.
    resolve_target_interface_element(rv, context_element, identifier)
}

/// Dart `_resolveTargetInterfaceElement(typeReference:, isCascaded: false,
/// propertyName:, hasRead: true, hasWrite: false, resolvingDotShorthand:
/// true)`.
fn resolve_target_interface_element(
    rv: &mut ResolverVisitor<'_>,
    type_reference: dartr_element::EId<InterfaceElement>,
    property_name: Id<SimpleIdentifier>,
) -> PropertyResult {
    let ctx = rv.ctx;
    let library = rv.unit.library;
    let name = rv.lexeme(rv.ast[property_name].token).to_string();
    let instance = type_reference.upcast::<InstanceElement>();
    let accessible = |e: ElemRef| member::is_accessible_in(&ctx, e, library);
    let mut read_element = lookup::get_getter(&ctx, instance, &name)
        .map(|g| ElemRef::Base(g.raw()))
        .filter(|&e| accessible(e));
    if read_element.is_none() {
        read_element = lookup::get_method(&ctx, instance, &name)
            .map(|m| ElemRef::Base(m.raw()))
            .filter(|&e| accessible(e));
    }
    // Dart: when the direct lookups fail and `static-extensions` is
    // enabled, `typePropertyResolver.resolveStaticExtension` (an
    // experiment, not ported).

    let mut result = PropertyResult::default();
    match read_element {
        Some(element) => {
            result.get_type = Some(member::return_type(&ctx, element));
            // Dart `_checkForStaticAccessToInstanceMember`.
            if !member::is_static(&ctx, element) {
                let d = rv.at(diag::static_access_to_instance_member(&name), property_name);
                rv.report(d);
                result.read_element_recovery = Some(element);
            } else {
                result.read_element_requested = Some(element);
            }
        }
        None => {
            // We didn't resolve to any static getter or static field using
            // the context type.
            let type_name = ctx
                .element_name(type_reference.raw())
                .unwrap_or("")
                .to_string();
            let d = rv.at(
                diag::dot_shorthand_undefined_getter(&name, &type_name),
                property_name,
            );
            rv.report(d);
        }
    }
    result
}

/// Dart `ResolverVisitor._resolvePropertyAccessRhs_common(resolverResult,
/// node, propertyName, contextType)` for a dot shorthand property access.
fn resolve_property_access_rhs_common(
    rv: &mut ResolverVisitor<'_>,
    resolver_result: PropertyResult,
    node: Id<Expression>,
    property_name: Id<SimpleIdentifier>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let element = resolver_result.read_element();
    rv.set_element(property_name, element);

    let mut ty = match element.map(|e| member::base_element(&ctx, e).tag()) {
        Some(Tag::Method | Tag::Constructor) => member::type_(&ctx, element.unwrap()),
        Some(Tag::Getter) => resolver_result.get_type.unwrap_or(TypeId::INVALID),
        // Dart: `functionTypeCallType`, `recordField` and `atDynamicTarget`
        // do not occur for a dot shorthand.
        _ => TypeId::INVALID,
    };

    if !rv.is_constructor_tearoffs_enabled() {
        // Only perform a generic function instantiation on a
        // [PrefixedIdentifier] in pre-constructor-tearoffs code. In
        // constructor-tearoffs-enabled code, generic function instantiation
        // is performed at assignability check sites.
        ty = infer_tear_off(rv, node, property_name, ty, context_type);
    }

    rv.set_static_type(property_name, ty);
    rv.record_static_type(node, ty);
    let replacement = rv.insert_generic_function_instantiation(node, context_type);
    rv.insert_implicit_call_reference(replacement, context_type);
}
