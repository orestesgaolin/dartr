// Dart source: pkg/analyzer/lib/src/error/nullable_dereference_verifier.dart

//! Checks of potentially nullable dereferences (Dart
//! `NullableDereferenceVerifier`), which the resolver calls while it
//! resolves.
//!
//! Open point: the why-not-promoted context messages (Dart
//! `ResolverVisitor.computeWhyNotPromotedMessages`) are not attached.

use dartr_ast::{Expression, Id, NodeId};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{TypeId, TypeKind};

use super::VerifierHost;

/// Dart `NullableDereferenceVerifier.expression(locatableDiagnostic,
/// expression, type: type)`: reports [locatable] at [expression] if its
/// type (or [ty]) is potentially nullable. Returns whether it reported.
pub fn expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    locatable: LocatableDiagnostic,
    expression: Id<Expression>,
    ty: Option<TypeId>,
) -> bool {
    // Dart `expression.typeOrThrow` (no throw: an unresolved expression is
    // `dynamic`, which is never reported).
    let ty = ty.unwrap_or_else(|| host.static_type(expression).unwrap_or(TypeId::DYNAMIC));
    check(host, locatable, expression.raw(), ty)
}

/// Dart `NullableDereferenceVerifier.report(locatableDiagnostic,
/// errorEntity, receiverType, messages: ...)`.
pub fn report<'a, H: VerifierHost<'a>>(
    host: &mut H,
    locatable: LocatableDiagnostic,
    error_entity: NodeId,
    receiver_type: TypeId,
) {
    let ctx = host.ctx();
    // Dart `receiverType == typeProvider.nullType` (Dart `==`).
    let locatable = if host
        .type_system()
        .dart_eq(receiver_type, ctx.tp.null_type())
    {
        diag::invalid_use_of_null_value()
    } else {
        locatable
    };
    let d = host.at(locatable, error_entity);
    host.report(d);
}

/// Dart `NullableDereferenceVerifier._check(locatableDiagnostic, errorNode,
/// receiverType)`: if [receiver_type] is potentially nullable, reports it.
/// Returns whether it reported.
fn check<'a, H: VerifierHost<'a>>(
    host: &mut H,
    locatable: LocatableDiagnostic,
    error_node: NodeId,
    receiver_type: TypeId,
) -> bool {
    if matches!(
        host.ctx().ty(receiver_type),
        TypeKind::Dynamic | TypeKind::Invalid
    ) || !host.type_system().is_potentially_nullable(receiver_type)
    {
        return false;
    }
    report(host, locatable, error_node, receiver_type);
    true
}
