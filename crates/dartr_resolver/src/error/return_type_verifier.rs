// Dart source: pkg/analyzer/lib/src/error/return_type_verifier.dart

//! STUB (wd-errors): the `ReturnTypeVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;

use crate::error_verifier::ErrorVerifier;

// `ReturnTypeVerifier.enclosingExecutable` is
// `ErrorVerifier::enclosing_executable`.

/// Dart `ReturnTypeVerifier.verifyExpressionFunctionBody(node)`.
pub fn verify_expression_function_body(
    ev: &mut ErrorVerifier<'_>,
    node: Id<ExpressionFunctionBody>,
) {
    let _ = (ev, node);
}

/// Dart `ReturnTypeVerifier.verifyReturnStatement(statement)`.
pub fn verify_return_statement(ev: &mut ErrorVerifier<'_>, statement: Id<ReturnStatement>) {
    let _ = (ev, statement);
}

/// Dart `ReturnTypeVerifier.verifyReturnType(returnType)`.
pub fn verify_return_type(ev: &mut ErrorVerifier<'_>, return_type: Option<Id<TypeAnnotation>>) {
    let _ = (ev, return_type);
}
