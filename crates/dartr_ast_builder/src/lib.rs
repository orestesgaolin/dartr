//! `dartr_ast_builder`: builds the analyzer AST from the parser events, a
//! port of `pkg/analyzer/lib/src/fasta/ast_builder.dart` and what it uses
//! (`stack_listener.dart`, `doc_comment_builder.dart`,
//! `error_converter.dart`), and the analyzer's `parseString`.
//!
//! # Use
//!
//! [`parse_string`] scans, parses and builds a file like the analyzer
//! `parseString`: the result ([`ParsedUnit`]) has the [`dartr_ast::Ast`]
//! (tokens and nodes), the `CompilationUnit` node, the parse diagnostics,
//! the line starts ([`dartr_syntax::LineInfo`]), the language version and
//! the feature set of the file.
//!
//! # Structure
//!
//! - [`ast_builder`]: the `AstBuilder` listener (see its documentation for
//!   the port conventions).
//! - [`stack`]: the value stack (Dart `StackListener`).
//! - [`doc_comment_builder`]: comment references, code blocks, doc
//!   directives and `@docImport`s of documentation comments.
//! - [`error_converter`]: CFE messages to analyzer diagnostics.
//! - [`parse`]: `parseString`.

#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::needless_return
)]

pub mod ast_builder;
pub mod doc_comment_builder;
pub mod error_converter;
pub mod parse;
pub mod stack;

pub use ast_builder::AstBuilder;
pub use parse::{LibraryLanguageVersion, ParsedUnit, parse_string};
