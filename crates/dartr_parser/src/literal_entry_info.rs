// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/literal_entry_info.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s and
//! keeps these names and signatures. Dart `LiteralEntryInfo` is a class
//! hierarchy with some mutable instances (`ForCondition`, `Nested`); here it
//! is an enum that is moved through the steps: Dart
//! `info = info.computeNext(token)` is `info = info.compute_next(tokens, token)`.

#![allow(unused_variables)]

use dartr_syntax::{TokenId, Tokens};

use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Dart `LiteralEntryInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralEntryInfo {
    /// Dart `simpleEntry`.
    Simple,
}

/// Dart `simpleEntry`.
pub const SIMPLE_ENTRY: LiteralEntryInfo = LiteralEntryInfo::Simple;

impl LiteralEntryInfo {
    /// Dart `hasEntry`.
    pub fn has_entry(&self) -> bool {
        todo!()
    }

    /// Dart `ifConditionDelta`.
    pub fn if_condition_delta(&self) -> i32 {
        todo!()
    }

    /// Dart `parse(token, parser)`.
    pub fn parse<L: Listener>(&mut self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }

    /// Dart `computeNext(token)`.
    pub fn compute_next(self, tokens: &Tokens, token: TokenId) -> Option<LiteralEntryInfo> {
        todo!()
    }
}

/// Dart `computeLiteralEntry`.
pub fn compute_literal_entry(tokens: &Tokens, token: TokenId) -> LiteralEntryInfo {
    todo!()
}

/// Dart `looksLikeLiteralEntry`.
pub fn looks_like_literal_entry(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}
