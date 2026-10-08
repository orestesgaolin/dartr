//! `dartr_syntax`: the Dart scanner (tokens), ported from
//! `pkg/_fe_analyzer_shared/lib/src/scanner` of the pinned Dart SDK, and the
//! analyzer use of it (`pkg/analyzer/lib/src/dart/scanner`).
//!
//! # Use
//!
//! - [`scan_for_analyzer`] scans like the analyzer does for `parseString` and
//!   file analysis: it returns the token stream, the first token after the
//!   error tokens, and the scanner diagnostics.
//! - [`scan_string`] is the front end entry point (`scanString`): error
//!   tokens stay at the start of the stream, as the fasta parser expects.
//!
//! # Token storage
//!
//! The Dart scanner builds a doubly linked list of token objects. Here all
//! tokens of a file are in one arena, [`Tokens`] (a `Vec<Token>`), and
//! refer to each other by [`TokenId`] (a `u32` index):
//!
//! - `next` / `previous` are the Dart links. The stream order is the link
//!   order, not the arena order: error tokens are inserted at the start of
//!   the stream after other tokens exist, and the parser port can insert
//!   synthetic tokens (or split `>>` into `>` `>`) by pushing a new token
//!   with [`Tokens::push`] and relinking `next`/`previous` (see
//!   [`Tokens::set_next`]). Nothing is ever removed from the arena.
//! - `preceding_comments` points to the first comment token; comments are
//!   linked with `next` and are in the same arena.
//! - `end_group` is Dart `BeginToken.endGroup`; `before_synthetic` is the
//!   Dart field of synthetic tokens.
//! - The EOF token's `next` is itself, as in Dart.
//!
//! A [`Token`] is plain data (about 44 bytes): type, flags, UTF-16 offset and
//! length (the analyzer reports UTF-16 offsets), and the byte range of the
//! lexeme in the shared source text (`Arc<str>`), so lexemes are borrowed,
//! not copied. Operators and keywords use the static lexeme of their
//! [`TokenType`]; the few synthetic lexemes that are not a substring of the
//! source (for example `"abc"` for the unterminated string `"abc`) are in a
//! side table. [`TokenType`] is the Dart `TokenType.index` (one byte);
//! [`TokenType::name`] is the Dart `TokenType.name`.
//!
//! # Input
//!
//! The scanner reads UTF-8 bytes but produces the same UTF-16 code unit
//! sequence as Dart `StringScanner` (which the analyzer uses), so that
//! offsets, error positions and the treatment of characters outside the BMP
//! (two surrogates) are the same as in the analyzer.

// The port keeps the Dart form of range checks (`a <= c && c <= b`).
#![allow(
    clippy::manual_range_contains,
    clippy::too_many_arguments,
    clippy::collapsible_if
)]

pub mod abstract_scanner;
pub mod analyzer_scanner;
pub mod characters;
pub mod diagnostic;
pub mod error_token;
pub mod keyword_state;
pub mod scanner;
mod string_scanner;
pub mod token;
pub mod token_constants;
pub mod token_type;

pub use abstract_scanner::{LanguageVersionInfo, ScannerConfiguration};
pub use analyzer_scanner::{AnalyzerScanResult, scan_for_analyzer};
pub use diagnostic::{Diagnostic, ScannerDiagnosticCode, Severity};
pub use error_token::{ErrorKind, ErrorToken, ScannerMessageCode};
pub use scanner::{ScannerResult, scan_string, strip_bom};
pub use token::{Token, TokenId, Tokens};
pub use token_type::{Keyword, KeywordStyle, TokenType};
