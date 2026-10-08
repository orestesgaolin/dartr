// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s.

#![allow(unused_variables)]

use dartr_syntax::{TokenId, Tokens};

use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Dart `checkAsyncAwaitYieldAsIdentifier`.
pub fn check_async_await_yield_as_identifier<L: Listener>(identifier: TokenId, parser: &mut Parser<L>) {
    todo!()
}

/// Dart `looksLikeStartOfNextClassMember`.
pub fn looks_like_start_of_next_class_member(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `looksLikeStartOfNextTopLevelDeclaration`.
pub fn looks_like_start_of_next_top_level_declaration(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}
