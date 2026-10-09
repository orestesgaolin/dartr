// Dart source: pkg/analyzer/lib/src/error/immutable_verifier.dart

//! STUB (wd-errors): not ported yet.

use dartr_ast::NodeId;
use dartr_syntax::TokenId;

use super::UnitVerifier;

/// Dart `ImmutableVerifier.checkDeclaration(node, nameToken:)` (a class,
/// class type alias or mixin declaration).
pub fn check_declaration(v: &mut UnitVerifier<'_>, node: NodeId, name_token: TokenId) {
    let _ = (v, node, name_token);
}
