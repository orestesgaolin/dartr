// Dart source: pkg/analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitFunctionExpressionInvocation),
// pkg/analyzer/lib/src/dart/resolver/extension_member_resolver.dart (getOverrideMember)

//! `FunctionExpressionInvocationResolver`: invocations of an expression
//! (`f()` where `f` is a local variable or a getter, `(e)()`, `e.call`
//! implicitly, `E(o)()` with an extension override).

use dartr_ast::{
    ExtensionOverride, FunctionExpressionInvocation, Id, MethodInvocation, NodeId,
};
use dartr_diagnostics::diag;
use dartr_element::{EId, ElemRef, ExtensionElement, InstanceElement, Tag, TypeId, TypeKind};
use dartr_flow::shared_type::SharedTypeSchemaView;
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::{lookup, member};

use crate::invocation_inferrer::{InferrerKind, InvocationInferrer, InvocationTarget, type_argument_types};
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitFunctionExpressionInvocation(node,
/// contextType:)`.
pub fn visit_function_expression_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpressionInvocation>,
    context_type: TypeId,
) {
    // If the node is a dot shorthand, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SharedTypeSchemaView::new(context_type));
    }

    let function = rv.ast[node].function;
    TypeAnalyzer::analyze_expression(
        rv,
        function,
        SharedTypeSchemaView::new(TypeId::UNKNOWN),
        true,
        false,
        false,
    );
    rv.pop_rewrite();

    resolve(rv, node, context_type);
    let replacement = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    // Dart `checkForArgumentTypesNotAssignableInList(node.argumentList,
    // whyNotPromotedArguments)` (wave D).
    rv.insert_implicit_call_reference(replacement, context_type);

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `FunctionExpressionInvocationResolver.resolve(node,
/// whyNotPromotedArguments, contextType:)`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<FunctionExpressionInvocation>, context_type: TypeId) {
    let function = rv.ast[node].function;

    if let Some(extension_override) = rv.ast.cast::<ExtensionOverride>(function) {
        resolve_receiver_extension_override(rv, node, extension_override, context_type);
        return;
    }

    let receiver_type = rv.static_type(function).unwrap_or(TypeId::DYNAMIC);
    if check_for_use_of_void_result(rv, function.raw(), receiver_type) {
        unresolved(rv, node, TypeId::DYNAMIC, context_type);
        return;
    }

    let receiver_type = rv.type_system.resolve_to_bound(receiver_type);
    if let TypeKind::Function(_) = rv.ctx.ty(receiver_type) {
        rv.nullable_dereference_expression(diag::unchecked_invocation_of_nullable_value(), function, None);
        resolve_with_target(
            rv,
            node,
            context_type,
            InvocationTarget::FunctionTypedExpression(receiver_type),
        );
        return;
    }

    if receiver_type == TypeId::NEVER {
        let d = rv.at(diag::receiver_of_type_never(), function);
        rv.report(d);
        unresolved(rv, node, TypeId::NEVER, context_type);
        return;
    }

    let result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: Some(function),
            receiver_type,
            name: "call",
            has_read: true,
            has_write: false,
            property_error_entity: function.raw(),
            name_error_entity: function.raw(),
            parent_node: None,
        },
    );
    let call_element = result.getter;

    if result.record_field.is_some() {
        let d = rv.at(diag::invocation_of_non_function_expression(), function);
        rv.report(d);
        unresolved(rv, node, TypeId::INVALID, context_type);
        return;
    }

    let Some(call_element) = call_element else {
        if result.needs_getter_error {
            let d = rv.at(diag::invocation_of_non_function_expression(), function);
            rv.report(d);
        }
        let ty = if result.is_getter_invalid {
            TypeId::INVALID
        } else {
            TypeId::DYNAMIC
        };
        unresolved(rv, node, ty, context_type);
        return;
    };

    if member::base_element(&rv.ctx, call_element).tag() != Tag::Method {
        let d = rv.at(diag::invocation_of_non_function_expression(), function);
        rv.report(d);
        unresolved(rv, node, TypeId::INVALID, context_type);
        return;
    }

    rv.set_element(node, Some(call_element));
    resolve_with_target(
        rv,
        node,
        context_type,
        InvocationTarget::ExecutableElement(call_element),
    );
}

/// Dart `_checkForUseOfVoidResult(expression, type)`.
fn check_for_use_of_void_result(rv: &mut ResolverVisitor<'_>, expression: NodeId, ty: TypeId) -> bool {
    if !matches!(rv.ctx.ty(ty), TypeKind::Void) {
        return false;
    }
    let error_node = match rv.ast.cast::<MethodInvocation>(expression) {
        Some(m) => rv.ast[m].method_name.raw(),
        None => expression,
    };
    let d = rv.at(diag::use_of_void_result(), error_node);
    rv.report(d);
    true
}

/// Dart `_resolve(node, whyNotPromotedArguments, contextType:, target:)`.
fn resolve_with_target(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpressionInvocation>,
    context_type: TypeId,
    target: InvocationTarget,
) {
    let return_type = InvocationInferrer {
        kind: InferrerKind::FunctionExpressionInvocation(node),
        argument_list: rv.ast[node].argument_list,
        context_type,
        target: Some(target),
    }
    .resolve_invocation(rv);
    rv.record_static_type(node, return_type);
}

/// Dart `_resolveReceiverExtensionOverride(node, function,
/// whyNotPromotedArguments, contextType:)`.
fn resolve_receiver_extension_override(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpressionInvocation>,
    function: Id<ExtensionOverride>,
    context_type: TypeId,
) {
    let call_element = get_override_member(rv, function, "call").0;
    rv.set_element(node, call_element);

    let Some(call_element) = call_element else {
        let name = rv.lexeme(rv.ast[function].name).to_string();
        let d = rv.at(diag::invocation_of_extension_without_call(&name), function);
        rv.report(d);
        unresolved(rv, node, TypeId::DYNAMIC, context_type);
        return;
    };

    if member::is_static(&rv.ctx, call_element) {
        let argument_list = rv.ast[node].argument_list;
        let d = rv.at(diag::extension_override_access_to_static_member(), argument_list);
        rv.report(d);
    }

    resolve_with_target(
        rv,
        node,
        context_type,
        InvocationTarget::ExecutableElement(call_element),
    );
}

/// Dart `_unresolved(node, type, whyNotPromotedArguments, contextType:)`.
fn unresolved(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpressionInvocation>,
    ty: TypeId,
    context_type: TypeId,
) {
    set_explicit_type_argument_types(rv, node);
    InvocationInferrer {
        kind: InferrerKind::FunctionExpressionInvocation(node),
        argument_list: rv.ast[node].argument_list,
        context_type,
        target: None,
    }
    .resolve_invocation(rv);
    rv.tables.invoke_type.insert(node, ty);
    rv.record_static_type(node, ty);
}

/// Dart `_setExplicitTypeArgumentTypes(node)`: inference cannot be done,
/// but the type argument types are still filled.
fn set_explicit_type_argument_types(rv: &mut ResolverVisitor<'_>, node: Id<FunctionExpressionInvocation>) {
    let types = match rv.ast[node].type_arguments {
        Some(list) => type_argument_types(rv, list),
        None => Vec::new(),
    };
    let list = rv.ctx.intern_list(&types);
    rv.tables.type_arg_types.insert(node, list);
}

/// Dart `ExtensionMemberResolver.getOverrideMember(node, name)`: the getter
/// and the setter named [name] of the extension of the override [node],
/// substituted with its type arguments.
///
/// A local port: the extension member resolver (C6) is not ported yet.
pub(crate) fn get_override_member(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    name: &str,
) -> (Option<ElemRef>, Option<ElemRef>) {
    let ctx = rv.ctx;
    let Some(element) = rv.base_element(node).and_then(|e| e.cast::<ExtensionElement>()) else {
        return (None, None);
    };
    let instance: EId<InstanceElement> = EId::from_raw(element.raw());
    let (getter, setter) = if name == "[]" {
        (
            lookup::get_method(&ctx, instance, "[]").map(|e| e.raw()),
            lookup::get_method(&ctx, instance, "[]=").map(|e| e.raw()),
        )
    } else {
        (
            lookup::get_getter(&ctx, instance, name)
                .map(|e| e.raw())
                .or_else(|| lookup::get_method(&ctx, instance, name).map(|e| e.raw())),
            lookup::get_setter(&ctx, instance, name).map(|e| e.raw()),
        )
    };
    if getter.is_none() && setter.is_none() {
        return (None, None);
    }
    let type_parameters = ctx.instance(instance).type_params.clone();
    // Dart `node.typeArgumentTypes!` (set by `resolveOverride`).
    let type_arguments: Vec<TypeId> = rv
        .tables
        .type_arg_types
        .get(node)
        .map(|l| ctx.list(*l).to_vec())
        .unwrap_or_default();
    let substitution = if type_arguments.len() == type_parameters.len() {
        MapSubstitution::from_pairs(&type_parameters, &type_arguments)
    } else {
        MapSubstitution::empty()
    };
    let substitute = |e: dartr_element::ElementId| member::substitute(&ctx, ElemRef::Base(e), &substitution);
    (getter.map(substitute), setter.map(substitute))
}
