// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/token.dart (Token.lexicallyFirst)
// Dart source: pkg/analyzer/lib/src/dart/ast/token.dart

//! Token helpers used by the AST.

use dartr_syntax::{TokenId, Tokens};

/// Dart `Token.lexicallyFirst`: the first non-null token with the smallest
/// offset (the earlier one on equal offsets).
pub fn lexically_first(tokens: &Tokens, candidates: &[Option<TokenId>]) -> Option<TokenId> {
    let mut result: Option<TokenId> = None;
    for &t in candidates {
        match (result, t) {
            (None, t) => result = t,
            (Some(r), Some(t)) if tokens.offset(t) < tokens.offset(r) => result = Some(t),
            _ => {}
        }
    }
    result
}
