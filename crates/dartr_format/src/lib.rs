//! dartr_format: a port of the Dart formatter (`package:dart_style` at the
//! revision that the Dart SDK 3.13.3 pins, `dart_style_rev` in the SDK
//! `DEPS`) on the dartr AST.
//!
//! - [`dart_formatter`]: the entry point ([`DartFormatter::format_source`]):
//!   parses, selects the style from the language version and checks that
//!   only whitespace changed.
//! - [`front_end`], [`back_end`], [`piece`]: the "tall" style (language
//!   version 3.7 and later): the AST is converted to a tree of pieces, and
//!   the solver selects the states of the pieces.
//! - [`short`]: the "short" style (language versions before 3.7): chunks,
//!   rules and the line splitter.
//! - [`testing`]: the `.unit` / `.stmt` test file format of dart_style.
//!
//! Offsets (selections, error offsets) are UTF-16 code units like Dart, and
//! line widths are measured in UTF-16 code units ([`text::utf16_len`]).

pub mod ast_extensions;
pub mod back_end;
pub mod comment_type;
pub mod constants;
pub mod dart_formatter;
pub mod dart_version_history;
pub mod exceptions;
pub mod front_end;
pub mod piece;
pub mod short;
pub mod source_code;
pub mod source_span;
pub mod string_compare;
pub mod testing;
pub mod text;

pub use dart_formatter::{DartFormatter, TrailingCommas};
pub use dart_version_history::{DartVersionHistory, Version};
pub use exceptions::{FormatError, FormatterError, FormatterException};
pub use source_code::SourceCode;
