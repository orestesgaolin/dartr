// Dart source: dart_style lib/src/short/source_comment.dart

use crate::comment_type::CommentType;

/// A comment in the source, with a bit of information about the surrounding
/// whitespace.
pub struct SourceComment {
    /// The text of the comment, including `//`, `/*`, and `*/`.
    pub text: String,

    /// The length of [text] in UTF-16 code units.
    pub text_length: i32,

    pub type_: CommentType,

    /// The number of newlines between the comment or token preceding this comment
    /// and the beginning of this one.
    ///
    /// Will be zero if the comment is a trailing one.
    pub lines_before: i32,

    /// Whether this comment starts at column one in the source.
    ///
    /// Comments that start at the start of the line will not be indented in the
    /// output. This way, commented out chunks of code do not get erroneously
    /// re-indented.
    pub flush_left: bool,

    /// Dart `Selection.selectionStart`.
    pub selection_start: Option<i32>,

    /// Dart `Selection.selectionEnd`.
    pub selection_end: Option<i32>,
}

impl SourceComment {
    pub fn new(text: String, type_: CommentType, lines_before: i32, flush_left: bool) -> Self {
        let text_length = crate::text::utf16_len(&text) as i32;
        SourceComment {
            text,
            text_length,
            type_,
            lines_before,
            flush_left,
            selection_start: None,
            selection_end: None,
        }
    }
}
