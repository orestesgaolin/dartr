// Dart source: pkg/analyzer/lib/src/error/null_safe_api_verifier.dart

//! `NullSafeApiVerifier`: `Future<T>.value()` and `Completer<T>.complete()`
//! with a non-nullable `T` and an argument that is effectively `null`
//! (`null_argument_to_non_null_type`).

use dartr_ast::{
    ArgumentList, Id, InstanceCreationExpression, MethodInvocation, NamedArgument, NodeId,
};
use dartr_diagnostics::diag;
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

use super::support::{elem_ref_of, node_range};
use super::{UnitVerifier, VerifierHost};

/// Dart `NullSafeApiVerifier.instanceCreation(expression)`.
pub fn instance_creation(v: &mut UnitVerifier<'_>, node: Id<InstanceCreationExpression>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(constructor) = elem_ref_of(v.tables, ast[node].constructor_name) else {
        return;
    };
    let ty = member::return_type(&ctx, constructor);
    let is_future_value =
        ctx.is_dart_async_future(ty) && member::name(&ctx, constructor) == Some("value");
    if is_future_value && let Some(&t) = ctx.type_arguments(ty).first() {
        check_types(v, node.raw(), "Future.value", t, ast[node].argument_list);
    }
}

/// Dart `NullSafeApiVerifier.methodInvocation(node)`.
pub fn method_invocation(v: &mut UnitVerifier<'_>, node: Id<MethodInvocation>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(target) = crate::ast_ext::method_invocation_real_target(ast, node) else {
        return;
    };
    let Some(&target_type) = v.tables.static_type.get(target.raw()) else {
        return;
    };
    let TypeKind::Interface { element, .. } = *ctx.ty(target_type) else {
        return;
    };
    let method_name = ast.tokens.lexeme(ast[ast[node].method_name].token);
    if ctx.is_element(element.raw(), "dart.async", "Completer") && method_name == "complete" {
        if let Some(&t) = ctx.type_arguments(target_type).first() {
            check_types(v, node.raw(), "Completer.complete", t, ast[node].argument_list);
        }
    }
}

/// Dart `_checkTypes(node, memberName, type, args)`.
fn check_types(
    v: &mut UnitVerifier<'_>,
    node: NodeId,
    member_name: &str,
    ty: TypeId,
    args: Id<ArgumentList>,
) {
    let ast = v.ast;
    let arguments = ast.list(ast[args].arguments);
    // If there's more than one argument, something else is wrong (and will
    // generate another diagnostic). Also, only check the argument type if
    // we expect a non-nullable type in the first place.
    if arguments.len() > 1 || !v.type_system.is_non_nullable(ty) {
        return;
    }
    let argument = arguments.first().map(|a| a.raw());
    let argument_type = argument.and_then(|a| {
        let expression = match ast.cast::<NamedArgument>(a) {
            Some(n) => ast[n].argument_expression.raw(),
            None => a,
        };
        v.tables.static_type.get(expression).copied()
    });
    // Skip if the type is not currently resolved.
    if argument.is_some() && argument_type.is_none() {
        return;
    }
    let argument_is_null = argument.is_none() || v.type_system.is_null(argument_type.unwrap());
    if argument_is_null {
        let type_name = dartr_element::diagnostics::type_display_string(&v.ctx, ty, true);
        let range = node_range(ast, argument.unwrap_or(node));
        v.report(
            diag::null_argument_to_non_null_type(member_name, &type_name)
                .at_offset(range.0 as usize, range.1 as usize),
        );
    }
}
