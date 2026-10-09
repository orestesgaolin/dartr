// Dart source: pkg/analyzer/lib/src/dart/resolver/invocation_inference_helper.dart

//! Partly STUB (unit C3): `InvocationInferenceHelper`. Ported:
//! [`infer_tear_off`] (identifier resolution needs it).
//!
//! The public functions are the entry points that the resolver core calls
//! (the Dart `ResolverVisitor.visitX` of the node kinds this file owns,
//! together with the Dart resolver class). Until the port lands they use
//! the fallback of the core: [`crate::resolver::ResolverVisitor::fallback_expression`]
//! for expressions (resolves the subexpressions, the type is `dynamic`),
//! nothing for statements and collection elements.

use dartr_ast::{Expression, Id, SimpleIdentifier};
use dartr_element::{TypeId, TypeKind};

use crate::resolver::ResolverVisitor;

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
            use dartr_typesystem::TypeExt;
            return rv.ctx.instantiate_function_type(tear_off_type, &type_arguments);
        }
    }
    tear_off_type
}
