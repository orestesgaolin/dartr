//! `dartr_parser`: the Dart parser, ported from
//! `pkg/_fe_analyzer_shared/lib/src/parser` of the pinned Dart SDK.
//!
//! # Use
//!
//! The parser is event based (Dart "fasta" parser): [`Parser`] reads the
//! token stream of `dartr_syntax` and calls the methods of a [`Listener`]
//! (`begin_x` / `end_x` / `handle_x`). The listener is a generic parameter
//! (static dispatch). [`parse_for_analyzer`] scans and parses a file the way
//! the analyzer does (`parseString`).
//!
//! # Port conventions
//!
//! - Every file starts with a comment that names its Dart source file.
//!   Methods keep their Dart names in snake_case (`parseUnit` is
//!   `parse_unit`; a private `_foo` is `foo`, except where that collides:
//!   `_parseExpression` is `parse_expression_impl`).
//! - Dart named and optional parameters are positional parameters here, in
//!   declaration order; callers pass the Dart default values explicitly.
//! - A Dart `Token` is a [`TokenId`] into the token arena; `Token?` is
//!   `Option<TokenId>`. The parser owns the arena
//!   (`parser.listener.tokens`, see [`ListenerStack`]); token properties
//!   are read with helpers (`self.next(t)` for `t.next!`, `self.is_a(t, ..)`
//!   for `t.isA(..)`).
//! - Messages are `dartr_diagnostics::cfe_codes` messages (`CfeMessage`). A
//!   Dart `Template` that takes a token is a `fn(&str) -> CfeMessage` that
//!   takes the lexeme.
//! - Where Dart replaces `parser.listener` for a while (`NullListener`,
//!   `ForwardingListener`, recovery listeners), the port pushes a
//!   [`listener_stack::Layer`] and pops it after.

// The port keeps the structure of the Dart code.
#![allow(
    clippy::manual_range_contains,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::needless_return,
    clippy::new_without_default
)]

pub mod analyzer;
pub mod assert;
pub mod async_modifier;
pub mod block_kind;
pub mod constructor_reference_context;
pub mod declaration_kind;
pub mod directive_context;
pub mod error_token;
pub mod event_recorder;
pub mod experimental_features;
pub mod experimental_flags;
pub mod formal_parameter_kind;
pub mod identifier_context;
pub mod identifier_context_impl;
pub mod listener;
pub mod listener_stack;
pub mod literal_entry_info;
pub mod literal_entry_info_impl;
pub mod loop_state;
pub mod member_kind;
pub mod modifier_context;
pub mod parser_error;
pub mod parser_impl;
pub mod quote;
pub mod token_stream_rewriter;
pub mod type_info;
pub mod type_info_impl;
pub mod util;

pub use analyzer::{AnalyzerParseResult, parse_for_analyzer};
pub use experimental_features::{ExperimentalFeatures, ExperimentalFlag};
pub use listener::Listener;
pub use listener_stack::ListenerStack;
pub use parser_impl::Parser;
pub use dartr_syntax::{TokenId, Tokens};
