// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/type_info_impl.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s.

#![allow(unused_variables)]

use dartr_syntax::{TokenId, Tokens};

/// Dart `looksLikeName`.
pub fn looks_like_name(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `looksLikeNameOrEndOfBlock`.
pub fn looks_like_name_or_end_of_block(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `looksLikeTypeParamOrArg`.
pub fn looks_like_type_param_or_arg(tokens: &Tokens, in_declaration: bool, token: TokenId) -> bool {
    todo!()
}

/// Dart `isVariance`.
pub fn is_variance(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `isCloser`.
pub fn is_closer(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `parseCloser` (may split `>>` and similar tokens).
pub fn parse_closer(tokens: &mut Tokens, before_closer: TokenId) -> bool {
    todo!()
}

/// Dart `splitCloser`.
pub fn split_closer(tokens: &mut Tokens, closer: TokenId) -> Option<TokenId> {
    todo!()
}
