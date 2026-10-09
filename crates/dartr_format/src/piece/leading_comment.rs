// Dart source: dart_style lib/src/piece/leading_comment.dart

use crate::back_end::code_writer::CodeWriter;

use super::{PieceId, PieceImpl, State};

/// A piece for a series of leading comments preceding some other piece.
///
/// We use this and hoist comments out from the inner piece so that a newline
/// in the comments doesn't erroneously force the inner piece to split. For
/// example, if comments preceding an infix operator's left operand:
///
///     value =
///         // comment
///         a + b;
///
/// Here, the `// comment` will be hoisted out and stored in a
/// [LeadingCommentPiece] instead of being a leading comment in the
/// [CodePiece] for `a`. If we left the comment in `a`, then the newline
/// after the line comment would force the `+` operator to split yielding:
///
///     value =
///         // comment
///         a +
///             b;
pub struct LeadingCommentPiece {
    comments: Vec<PieceId>,
    piece: PieceId,
}

impl LeadingCommentPiece {
    pub fn new(comments: Vec<PieceId>, piece: PieceId) -> LeadingCommentPiece {
        LeadingCommentPiece { comments, piece }
    }
}

impl PieceImpl for LeadingCommentPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        // If a piece has a leading comment, that comment should not also be a
        // hanging comment, so ensure it begins its own line. This is also
        // important to ensure that formatting is idempotent: If we don't do
        // this, a comment might be a leading comment in the input and then
        // get output on the same line as some preceding code, which would
        // lead it to be a hanging comment the next time the formatter runs.
        writer.newline();
        for &comment in &self.comments {
            writer.format(comment, false);
        }

        writer.format(self.piece, false);
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for &comment in &self.comments {
            callback(comment);
        }
        callback(self.piece);
    }
}
