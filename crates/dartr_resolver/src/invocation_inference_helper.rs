// Dart source: pkg/analyzer/lib/src/dart/resolver/invocation_inference_helper.dart

//! `InvocationInferenceHelper`: the tear-off inference of identifiers, the
//! final inference step of method invocations and dot shorthand
//! invocations, and `ConstructorElementToInfer`.

use dartr_ast::{DotShorthandInvocation, Expression, Id, MethodInvocation, SimpleIdentifier};
use dartr_element::{
    EId, ElemRef, ElementId, InterfaceElement, LibraryElement, Nullability, TypeAliasElement, TypeId,
    TypeKind, TypeParameterElement,
};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::invocation_inferrer::{
    InferrerKind, InvocationInferrer, InvocationTarget, function_type_parameters,
};
use crate::resolver::ResolverVisitor;

/// Dart `ConstructorElementToInfer`: a constructor element to instantiate.
///
/// If the target is a class, [element] is a raw constructor of the class
/// and [type_parameters] are the type parameters of the class. If the
/// target is a type alias of an interface type, [element] is the
/// constructor of the class substituted with the type arguments of the
/// alias, and [type_parameters] are the type parameters of the alias.
#[derive(Clone, Debug)]
pub struct ConstructorElementToInfer {
    /// The type parameters used in [element].
    pub type_parameters: Vec<EId<TypeParameterElement>>,
    /// The element, might be a substituted member.
    pub element: ElemRef,
}

impl ConstructorElementToInfer {
    /// Dart `asType`: the generic function type that forwards to the
    /// constructor (`<T>(T) -> C<T>` for `class C<T> { C(T arg); }`), or the
    /// constructor type for a non-generic type.
    pub fn as_type(&self, rv: &ResolverVisitor<'_>) -> TypeId {
        let ctx = rv.ctx;
        let ty = member::type_(&ctx, self.element);
        if self.type_parameters.is_empty() {
            return ty;
        }
        let parameters = function_type_parameters(rv, Some(ty));
        let return_type = member::return_type(&ctx, self.element);
        ctx.function_type(&self.type_parameters, &parameters, return_type, Nullability::None, None)
    }
}

/// Dart `InvocationInferenceHelper.constructorElementToInfer(typeElement:,
/// constructorName:, definingLibrary:)`: the constructor and the type
/// parameters to infer, if [type_element] is a class or a type alias of an
/// interface type and it has the constructor.
pub fn constructor_element_to_infer(
    rv: &ResolverVisitor<'_>,
    type_element: Option<ElementId>,
    constructor_name: Option<&str>,
    defining_library: EId<LibraryElement>,
) -> Option<ConstructorElementToInfer> {
    let ctx = rv.ctx;
    let type_element = type_element?;
    let (type_parameters, raw_element);
    if let Some(interface) = type_element.cast::<InterfaceElement>() {
        type_parameters = ctx.interface_type_parameters(interface).to_vec();
        raw_element = match constructor_name {
            None => lookup::get_named_constructor(&ctx, interface, "new").map(|c| ElemRef::Base(c.raw())),
            Some(name) => lookup::get_named_constructor(&ctx, interface, name)
                .map(|c| ElemRef::Base(c.raw()))
                .filter(|&c| member::is_accessible_in(&ctx, c, defining_library)),
        };
    } else {
        let alias = type_element.cast::<TypeAliasElement>()?;
        let data = ctx.get(alias);
        type_parameters = data.type_params.clone();
        raw_element = match data.aliased_type.get() {
            Some(aliased) if matches!(ctx.ty(aliased), TypeKind::Interface { .. }) => {
                lookup::type_look_up_constructor(&ctx, aliased, constructor_name, defining_library)
            }
            _ => None,
        };
    }
    Some(ConstructorElementToInfer {
        type_parameters,
        element: raw_element?,
    })
}

/// Dart `InvocationInferenceHelper.inferTearOff(expression, identifier,
/// tearOffType, contextType:)`.
pub fn infer_tear_off(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<Expression>,
    identifier: Id<SimpleIdentifier>,
    tear_off_type: TypeId,
    context_type: TypeId,
) -> TypeId {
    if matches!(rv.ctx.ty(context_type), TypeKind::Function(_))
        && matches!(rv.ctx.ty(tear_off_type), TypeKind::Function(_))
    {
        let generic_metadata_is_enabled = rv.generic_metadata_is_enabled();
        let type_arguments = rv.infer_function_type_instantiation(
            context_type,
            tear_off_type,
            expression,
            generic_metadata_is_enabled,
        );
        // Dart `identifier.tearOffTypeArgumentTypes = typeArguments`.
        let list = rv.ctx.intern_list(&type_arguments);
        rv.tables.type_arg_types.insert(identifier, list);
        if !type_arguments.is_empty() {
            return rv.ctx.instantiate_function_type(tear_off_type, &type_arguments);
        }
    }
    tear_off_type
}

/// Dart `InvocationInferenceHelper.resolveDotShorthandInvocation(node:,
/// whyNotPromotedArguments:, contextType:, target:)`: downwards inference,
/// resolution of the arguments, and upwards inference of the invoked
/// executable.
pub fn resolve_dot_shorthand_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandInvocation>,
    context_type: TypeId,
    target: InvocationTarget,
) {
    let return_type = InvocationInferrer {
        kind: InferrerKind::DotShorthandInvocation(node),
        argument_list: rv.ast[node].argument_list,
        context_type,
        target: Some(target),
    }
    .resolve_invocation(rv);
    rv.record_static_type(node, return_type);
}

/// Dart `InvocationInferenceHelper.resolveMethodInvocation(node:,
/// whyNotPromotedArguments:, contextType:, target:)`: downwards inference,
/// resolution of the arguments, and upwards inference of the invoked
/// executable.
pub fn resolve_method_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<MethodInvocation>,
    context_type: TypeId,
    target: Option<InvocationTarget>,
) {
    let return_type = InvocationInferrer {
        kind: InferrerKind::MethodInvocation(node),
        argument_list: rv.ast[node].argument_list,
        context_type,
        target,
    }
    .resolve_invocation(rv);
    rv.record_static_type(node, return_type);
}
