// Dart source: dart_style lib/src/comment_type.dart

/// The kind of a comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommentType {
    /// A `///` or `/**` doc comment.
    Doc,

    /// A non-doc line comment.
    Line,

    /// A `/* ... */` comment that should be on its own line.
    ///
    /// These occur when the block comment doesn't appear with any code on the
    /// same line preceding the `/*` or after the `*/`.
    Block,

    /// A `/* ... */` comment that can share a line with other code.
    ///
    /// These occur when there is code on the same line either immediately
    /// preceding the `/*`, after the `*/`, or both. An inline block comment
    /// may be multiple lines, as in:
    ///
    ///     code /* comment
    ///       more */
    InlineBlock,
}

impl CommentType {
    /// Whether a comment of this type may contain newlines inside its lexeme.
    pub fn may_be_multiline(self) -> bool {
        self != CommentType::Line
    }
}
