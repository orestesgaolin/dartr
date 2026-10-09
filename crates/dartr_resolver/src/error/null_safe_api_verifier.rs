// Dart source: pkg/analyzer/lib/src/error/null_safe_api_verifier.dart

//! STUB (wd-errors): not ported yet.

use dartr_ast::{Id, InstanceCreationExpression, MethodInvocation};

use super::UnitVerifier;

/// Dart `NullSafeApiVerifier.instanceCreation(expression)`.
pub fn instance_creation(v: &mut UnitVerifier<'_>, node: Id<InstanceCreationExpression>) {
    let _ = (v, node);
}

/// Dart `NullSafeApiVerifier.methodInvocation(node)`.
pub fn method_invocation(v: &mut UnitVerifier<'_>, node: Id<MethodInvocation>) {
    let _ = (v, node);
}
