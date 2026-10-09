// Dart source: pkg/analyzer/lib/src/dart/resolver/function_expression_resolver.dart

//! `FunctionExpressionResolver`: a function expression (a closure, or the
//! function expression of a function declaration): inference of the formal
//! parameter types from the context, the body and its return type, the
//! static type.
//!
//! `AnonymousMethodInvocation` (the anonymous methods experiment) is a
//! STUB (unit C8).

use dartr_ast::{
    AnonymousMethodInvocation, FormalParameterList, FunctionDeclaration,
    FunctionDeclarationStatement, FunctionExpression, Id, TypeParameterList,
};
use dartr_element::{
    EId, ElementId, ExecutableElement, FormalParameterElement, Nullability, TypeId, TypeKind,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_typesystem::TypeExt;
use dartr_typesystem::element_type::executable_type;

use crate::element_ext;
use crate::resolver::ResolverVisitor;

/// The executable element declared by a function expression (Dart
/// `node.declaredFragment!.element`).
pub fn function_expression_element(
    rv: &ResolverVisitor<'_>,
    node: Id<FunctionExpression>,
) -> Option<EId<ExecutableElement>> {
    let fragment = *rv.tables.declared_fragment.get(node)?;
    let element = *rv.ctx.fragment_data(fragment)?.element.try_get()?;
    element.cast::<ExecutableElement>()
}

/// Dart `ResolverVisitor.visitFunctionExpression(node, contextType:)`.
pub fn visit_function_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpression>,
    context_type: TypeId,
) {
    let Some(element) = function_expression_element(rv, node) else {
        // Not bound yet (element binding not ported for this node).
        rv.record_static_type(node, TypeId::DYNAMIC);
        return;
    };
    let outer_function = rv.enclosing_function;
    rv.enclosing_function = Some(element);
    resolve(rv, node, context_type, element);
    rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.enclosing_function = outer_function;
}

/// Dart `FunctionExpressionResolver.resolve(node, contextType:)`.
pub fn resolve(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpression>,
    context_type: TypeId,
    element: EId<ExecutableElement>,
) {
    let parent = rv.ast.parent(node).expect("parent");
    let is_function_declaration = rv.ast.is::<FunctionDeclaration>(parent);
    let body = rv.ast[node].body;
    let is_closure = rv.flow_analysis.is_active() && !is_function_declaration;
    let formal_parameters: Vec<EId<FormalParameterElement>> =
        rv.ctx.executable(element).formal_params.clone();

    if is_closure {
        rv.flow_analysis
            .executable_declaration_enter(node.raw(), Some(&formal_parameters), true);
    }

    let was_function_type_supplied = matches!(rv.ctx.ty(context_type), TypeKind::Function(_));
    let mut imposed_type = None;
    if was_function_type_supplied {
        let type_parameters = rv.ast[node].type_parameters;
        if let Some(instantiated) = match_type_parameters(rv, type_parameters, context_type) {
            infer_formal_parameters(
                rv,
                rv.ast[node].parameters,
                &formal_parameters,
                instantiated,
            );
            let TypeKind::Function(f) = *rv.ctx.ty(instantiated) else {
                unreachable!()
            };
            if !matches!(rv.ctx.ty(f.ret), TypeKind::Dynamic | TypeKind::Unknown) {
                imposed_type = Some(f.ret);
            }
        }
    }

    let type_parameters = rv.ast[node].type_parameters;
    rv.visit_opt(type_parameters);
    let parameters = rv.ast[node].parameters;
    rv.visit_opt(parameters);
    let imposed_type = rv.resolve_function_body(body, imposed_type);
    if is_function_declaration {
        // A side effect of visiting the children is that the parameters are
        // now in scope, so we can visit the documentation comment now.
        let declaration: Id<FunctionDeclaration> = rv.ast.cast(parent).unwrap();
        let comment = rv.ast[declaration].documentation_comment;
        rv.visit_opt(comment);
    }
    resolve2(rv, node, element, imposed_type);

    if is_closure {
        rv.check_for_body_may_complete_normally(body.raw(), body.raw());
        if let Some(flow) = rv.flow_analysis.flow.as_mut() {
            flow.function_expression_end();
        }
    }

    // Dart: `checkForTypeParameterBoundRecursion` and `DefaultTypesBuilder`
    // for the type parameters of local generic functions: not ported yet
    // (they need summary2/default_types_builder.dart, unit C8).
}

/// Dart `_inferFormalParameters`: infers the types of the implicitly typed
/// formal parameters from the context function type.
fn infer_formal_parameters(
    rv: &mut ResolverVisitor<'_>,
    node: Option<Id<FormalParameterList>>,
    formal_parameters: &[EId<FormalParameterElement>],
    context_type: TypeId,
) {
    if node.is_none() {
        return;
    }
    let ctx = rv.ctx;
    let ts = rv.type_system;
    let TypeKind::Function(f) = *ctx.ty(context_type) else {
        return;
    };
    let infer_type = |p: EId<FormalParameterElement>, inferred: TypeId| {
        // Check that there is no declared type, and that we have not already
        // inferred a type in some fashion.
        let current = ctx.get(p).type_.get().unwrap_or(TypeId::INVALID);
        if element_ext::has_implicit_type(&ctx, p.raw())
            && matches!(ctx.ty(current), TypeKind::Dynamic)
        {
            // If no type is declared for a parameter and there is a
            // corresponding parameter in the context type schema with type
            // schema `K`, the parameter is given an inferred type `T` where
            // `T` is derived from `K` as follows.
            let mut inferred = ts.greatest_closure_of_schema(inferred);
            // If the greatest closure of `K` is `S` and `S` is a subtype of
            // `Null`, then `T` is `Object?`. Otherwise, `T` is `S`.
            if ts.is_subtype_of(inferred, ts.null_none()) {
                inferred = ts.object_question();
            }
            if !matches!(ctx.ty(inferred), TypeKind::Dynamic) {
                ctx.get(p).type_.set(Some(inferred));
            }
        }
    };
    let context_params = ctx.list(f.params);
    {
        let node_positional = formal_parameters
            .iter()
            .copied()
            .filter(|&p| ctx.get(p).kind.is_positional());
        let context_positional = context_params.iter().filter(|p| p.kind.is_positional());
        for (p, c) in node_positional.zip(context_positional) {
            infer_type(p, c.ty);
        }
    }
    {
        for p in formal_parameters
            .iter()
            .copied()
            .filter(|&p| ctx.get(p).kind.is_named())
        {
            let name = ctx.get(p).name;
            if let Some(c) = context_params
                .iter()
                .find(|c| c.kind.is_named() && c.name == name)
            {
                infer_type(p, c.ty);
            }
        }
    }
    // The cached function type of the element depends on the parameter
    // types.
    let _ = formal_parameters;
}

/// Dart `_matchTypeParameters`: the context function type expressed with
/// the type parameters of [type_parameter_list], or `None` if the numbers of
/// type parameters differ.
fn match_type_parameters(
    rv: &ResolverVisitor<'_>,
    type_parameter_list: Option<Id<TypeParameterList>>,
    ty: TypeId,
) -> Option<TypeId> {
    let ctx = rv.ctx;
    let TypeKind::Function(f) = *ctx.ty(ty) else {
        return None;
    };
    let fn_type_params = ctx.list(f.type_params);
    let Some(list) = type_parameter_list else {
        return if fn_type_params.is_empty() {
            Some(ty)
        } else {
            None
        };
    };
    let type_parameters = rv.ast.list(rv.ast[list].type_parameters).to_vec();
    if type_parameters.len() != fn_type_params.len() {
        return None;
    }
    let mut args = Vec::with_capacity(type_parameters.len());
    for tp in type_parameters {
        let fragment = *rv.tables.declared_fragment.get(tp)?;
        let element = *ctx.fragment_data(fragment)?.element.try_get()?;
        let element = element.cast::<dartr_element::TypeParameterElement>()?;
        args.push(ctx.type_parameter_type(element, Nullability::None));
    }
    Some(ctx.instantiate_function_type(ty, &args))
}

/// Dart `_resolve2`.
fn resolve2(
    rv: &mut ResolverVisitor<'_>,
    node: Id<FunctionExpression>,
    element: EId<ExecutableElement>,
    imposed_type: TypeId,
) {
    if should_update_return_type(rv, node) {
        let data = rv.ctx.executable(element);
        data.return_type.set(Some(imposed_type));
        // Dart resets `_type` when the return type changes.
        data.type_.set(None);
    }
    let ty = executable_type(&rv.ctx, element);
    rv.record_static_type(node, ty);
}

/// Dart `_shouldUpdateReturnType`.
fn should_update_return_type(rv: &ResolverVisitor<'_>, node: Id<FunctionExpression>) -> bool {
    let parent = rv.ast.parent(node).expect("parent");
    if let Some(declaration) = rv.ast.cast::<FunctionDeclaration>(parent) {
        // Local function without declared return type.
        let grandparent = rv.ast.parent(parent).expect("parent");
        rv.ast.is::<FunctionDeclarationStatement>(grandparent)
            && rv.ast[declaration].return_type.is_none()
    } else {
        // Pure function expression.
        true
    }
}

/// Dart `ResolverVisitor.visitAnonymousMethodInvocation`. STUB (C8).
pub fn visit_anonymous_method_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<AnonymousMethodInvocation>,
    context_type: TypeId,
) {
    let _ = context_type;
    rv.fallback_expression(node.upcast());
}

/// Unused import guard.
#[allow(dead_code)]
fn _e(_: ElementId) {}
