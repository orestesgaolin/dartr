// Dart source: dart_style lib/src/piece/text.dart

use crate::back_end::code_writer::{CodeWriter, ShapeMode, Whitespace};
use crate::text::utf16_len;

use super::{PieceId, PieceImpl, State};

/// A simple atomic piece of code.
///
/// This may represent a series of tokens where no split can occur between
/// them. It may also contain one or more comments.
///
/// Dart has the sealed class `TextPiece` with the subclasses `CodePiece`,
/// `CommentPiece` and `EnableFormattingCommentPiece`; here they are the
/// variants of [TextKind].
pub struct TextPiece {
    /// The lines of text in this piece.
    ///
    /// Most [TextPieces] will contain only a single line, but a piece for a
    /// multi-line string or comment will have multiple lines. These are
    /// stored as separate lines instead of a single multi-line Dart String so
    /// that line endings are normalized and so that column calculation
    /// during line splitting calculates each line in the piece separately.
    lines: Vec<String>,

    /// Whether the code in this piece is considered to be "soft" code.
    ///
    /// See [FormattingStyle::use_soft_overflow] for more details.
    pub is_soft: bool,

    /// The offset from the beginning of [text] where the selection starts, or
    /// `None` if the selection does not start within this chunk.
    selection_start: Option<usize>,

    /// The offset from the beginning of [text] where the selection ends, or
    /// `None` if the selection does not start within this chunk.
    selection_end: Option<usize>,

    pub kind: TextKind,
}

pub enum TextKind {
    /// [TextPiece] for non-comment source code that may have comments
    /// attached to it.
    Code {
        /// Pieces for any comments that appear immediately before this code.
        leading_comments: Vec<PieceId>,

        /// Pieces for any comments that hang off the same line as this code.
        hanging_comments: Vec<PieceId>,
    },

    /// A [TextPiece] for a source code comment and the whitespace after it,
    /// if any.
    Comment {
        /// Whitespace at the end of the comment.
        trailing_whitespace: Whitespace,
    },

    /// A piece for the special `// dart format off` and `// dart format on`
    /// comments that are used to opt a region of code out of being
    /// formatted.
    EnableFormattingComment {
        /// Whitespace at the end of the comment.
        trailing_whitespace: Whitespace,

        /// Whether this comment disables formatting (`format off`) or
        /// re-enables it (`format on`).
        enabled: bool,

        /// The number of code points from the beginning of the unformatted
        /// source where the unformatted code should begin or end.
        ///
        /// If this piece is for `// dart format off`, then the offset is just
        /// past the `off`. If this piece is for `// dart format on`, it points
        /// to just before `//`.
        source_offset: usize,
    },
}

impl TextPiece {
    fn with_kind(soft: bool, kind: TextKind) -> TextPiece {
        TextPiece {
            lines: vec![String::new()],
            is_soft: soft,
            selection_start: None,
            selection_end: None,
            kind,
        }
    }

    /// Dart `CodePiece(leadingComments, soft: soft)`.
    pub fn code(leading_comments: Vec<PieceId>, soft: bool) -> TextPiece {
        TextPiece::with_kind(
            soft,
            TextKind::Code {
                leading_comments,
                hanging_comments: Vec::new(),
            },
        )
    }

    /// Dart `CommentPiece(trailingWhitespace, soft: soft)`.
    pub fn comment(trailing_whitespace: Whitespace, soft: bool) -> TextPiece {
        TextPiece::with_kind(
            soft,
            TextKind::Comment {
                trailing_whitespace,
            },
        )
    }

    /// Dart `EnableFormattingCommentPiece(sourceOffset, trailingWhitespace,
    /// soft: soft, enable: enable)`.
    pub fn enable_formatting_comment(
        source_offset: usize,
        trailing_whitespace: Whitespace,
        soft: bool,
        enable: bool,
    ) -> TextPiece {
        TextPiece::with_kind(
            soft,
            TextKind::EnableFormattingComment {
                trailing_whitespace,
                enabled: enable,
                source_offset,
            },
        )
    }

    /// Dart `CodePiece.addHangingComment`.
    pub fn add_hanging_comment(&mut self, comment: PieceId) {
        match &mut self.kind {
            TextKind::Code {
                hanging_comments, ..
            } => hanging_comments.push(comment),
            _ => unreachable!("only code pieces have hanging comments"),
        }
    }

    /// Whether this is a Dart `CodePiece`.
    pub fn is_code(&self) -> bool {
        matches!(self.kind, TextKind::Code { .. })
    }

    /// Append [text] to the end of this piece.
    ///
    /// If [text] may contain any newline characters, then [multiline] must be
    /// `true`.
    ///
    /// If [selection_start] and/or [selection_end] are given, then notes that
    /// the corresponding selection markers appear that many code units from
    /// where [text] will be appended.
    pub fn append(
        &mut self,
        text: &str,
        multiline: bool,
        selection_start: Option<usize>,
        selection_end: Option<usize>,
    ) {
        if let Some(selection_start) = selection_start {
            self.selection_start = Some(self.adjust_selection(selection_start));
        }

        if let Some(selection_end) = selection_end {
            self.selection_end = Some(self.adjust_selection(selection_end));
        }

        if multiline {
            // Dart `text.split(RegExp(r'\r\n?|\n'))`.
            let bytes = text.as_bytes();
            let mut start = 0;
            let mut i = 0;
            let mut first = true;
            while i < bytes.len() {
                let terminator = match bytes[i] {
                    b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
                    b'\r' | b'\n' => 1,
                    _ => 0,
                };
                if terminator > 0 {
                    if !first {
                        self.lines.push(String::new());
                    }
                    self.lines.last_mut().unwrap().push_str(&text[start..i]);
                    first = false;
                    i += terminator;
                    start = i;
                } else {
                    i += 1;
                }
            }
            if !first {
                self.lines.push(String::new());
            }
            self.lines.last_mut().unwrap().push_str(&text[start..]);
        } else {
            self.lines.last_mut().unwrap().push_str(text);
        }
    }

    /// Adjust [offset] by the current length of this [TextPiece].
    fn adjust_selection(&self, mut offset: usize) -> usize {
        for line in &self.lines {
            offset += utf16_len(line);
        }

        offset
    }

    fn format_selection(&self, writer: &mut CodeWriter<'_, '_>) {
        if let Some(start) = self.selection_start {
            writer.start_selection(start);
        }

        if let Some(end) = self.selection_end {
            writer.end_selection(end);
        }
    }

    fn format_lines<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, soft: bool) {
        // Allow a multiline string or comment to be treated like a headline
        // when the right-hand side of an assignment or named argument, like:
        //
        //     variable = """
        //         some string
        //         """;
        if self.lines.len() > 1 {
            writer.set_shape_mode(ShapeMode::BeforeHeadline);
        }

        writer.write(&self.lines[0], soft);

        if self.lines.len() > 1 {
            writer.set_shape_mode(ShapeMode::AfterHeadline);
        }

        for line in &self.lines[1..] {
            writer.newline_with(false, true);
            writer.write(line, soft);
        }
    }
}

impl PieceImpl for TextPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        match &self.kind {
            TextKind::Code {
                leading_comments,
                hanging_comments,
            } => {
                self.format_selection(writer);

                if !leading_comments.is_empty() {
                    // Always put leading comments on a new line.
                    writer.newline();

                    for &comment in leading_comments {
                        writer.format(comment, false);
                    }
                }

                self.format_lines(writer, self.is_soft);

                for &comment in hanging_comments {
                    writer.space();
                    writer.format(comment, false);
                }
            }
            TextKind::Comment {
                trailing_whitespace,
            } => {
                self.format_selection(writer);
                self.format_lines(writer, true);
                writer.whitespace(*trailing_whitespace, false);
            }
            TextKind::EnableFormattingComment {
                trailing_whitespace,
                enabled,
                source_offset,
            } => {
                self.format_selection(writer);
                self.format_lines(writer, true);
                writer.whitespace(*trailing_whitespace, false);
                writer.set_formatting_enabled(*enabled, *source_offset);
            }
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        if let TextKind::Code {
            leading_comments,
            hanging_comments,
        } = &self.kind
        {
            for &comment in leading_comments {
                callback(comment);
            }
            for &comment in hanging_comments {
                callback(comment);
            }
        }
    }

    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        match &self.kind {
            TextKind::Code { .. } => Some(self.lines.len() > 1),
            TextKind::Comment {
                trailing_whitespace,
            }
            | TextKind::EnableFormattingComment {
                trailing_whitespace,
                ..
            } => Some(trailing_whitespace.has_newline() || self.lines.len() > 1),
        }
    }

    fn calculate_total_characters(&self) -> Option<i32> {
        // Since soft text might not force an overflowing piece to split, we
        // don't include it in the calculation to preemptively split pieces.
        // Otherwise, the optimization might conseratively split more than the
        // solver would for a solution that does overflow.
        if self.is_soft {
            return Some(0);
        }

        let mut total = 0;

        for line in &self.lines {
            total += utf16_len(line) as i32;
        }

        Some(total)
    }
}

/// A piece that writes a single space.
pub struct SpacePiece;

impl PieceImpl for SpacePiece {
    fn for_each_child(&self, _callback: &mut dyn FnMut(PieceId)) {}

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.space();
    }

    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        Some(false)
    }

    fn calculate_total_characters(&self) -> Option<i32> {
        Some(1)
    }
}

/// A piece that writes a single newline.
pub struct NewlinePiece;

impl PieceImpl for NewlinePiece {
    fn for_each_child(&self, _callback: &mut dyn FnMut(PieceId)) {}

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.newline();
    }

    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        Some(true)
    }

    fn calculate_total_characters(&self) -> Option<i32> {
        Some(0)
    }
}
