// Dart source: pkg/analyzer/lib/src/error/duplicate_definition_verifier.dart

//! STUB (wd-errors): the `DuplicateDefinitionVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;

use crate::error_verifier::ErrorVerifier;

/// Dart `DuplicationDefinitionContext` (one per library).
#[derive(Debug, Default)]
pub struct DuplicationDefinitionContext {}

/// Dart `DuplicateDefinitionVerifier.checkCatchClause(node)`.
pub fn check_catch_clause(ev: &mut ErrorVerifier<'_>, node: Id<CatchClause>) {
    let _ = (ev, node);
}

/// Dart `DuplicateDefinitionVerifier.checkForVariables(node)`.
pub fn check_for_variables(ev: &mut ErrorVerifier<'_>, node: Id<VariableDeclarationList>) {
    let _ = (ev, node);
}

/// Dart `DuplicateDefinitionVerifier.checkParameters(node)`.
pub fn check_parameters(ev: &mut ErrorVerifier<'_>, node: Id<FormalParameterList>) {
    let _ = (ev, node);
}

/// Dart `DuplicateDefinitionVerifier.checkStatements(statements)`.
pub fn check_statements(ev: &mut ErrorVerifier<'_>, statements: &[Id<Statement>]) {
    let _ = (ev, statements);
}

/// Dart `DuplicateDefinitionVerifier.checkTypeParameters(node)`.
pub fn check_type_parameters(ev: &mut ErrorVerifier<'_>, node: Id<TypeParameterList>) {
    let _ = (ev, node);
}

/// Dart `DuplicateDefinitionVerifier.checkUnit(node)`.
pub fn check_unit(ev: &mut ErrorVerifier<'_>, node: Id<CompilationUnit>) {
    let _ = (ev, node);
}
