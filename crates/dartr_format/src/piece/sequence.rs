// Dart source: dart_style lib/src/piece/sequence.dart

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};

use super::{PieceId, PieceImpl, PieceKind, Pieces, State};

/// A piece for a series of statements or members inside a block or
/// declaration body or at the top level of a program.
///
/// Constructed using a [SequenceBuilder].
pub struct SequencePiece {
    /// The series of members or statements.
    elements: Vec<PieceId>,
}

impl SequencePiece {
    pub fn new(elements: Vec<PieceId>) -> SequencePiece {
        SequencePiece { elements }
    }
}

fn sequence_element(pieces: &Pieces, id: PieceId) -> &SequenceElementPiece {
    match pieces.kind(id) {
        PieceKind::SequenceElement(element) => element,
        _ => unreachable!("SequencePiece elements are SequenceElementPieces"),
    }
}

impl PieceImpl for SequencePiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        let pieces = writer.pieces();
        writer.push_indent(Indent::None);

        for (i, &element) in self.elements.iter().enumerate() {
            writer.format(element, true);

            if i < self.elements.len() - 1 {
                writer.pop_indent();
                writer.push_indent(sequence_element(pieces, self.elements[i + 1]).indent);
                writer.newline_with(sequence_element(pieces, element).blank_after, false);
            }
        }

        writer.pop_indent();
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for &element in &self.elements {
            callback(element);
        }
    }

    /// If there are multiple elements, there are newlines between them.
    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        Some(self.elements.len() > 1)
    }
}

/// A piece for a non-empty brace-delimited series of statements or members
/// inside a block or declaration body.
///
/// Unlike [ListPiece], always splits between the elements.
///
/// Constructed using a [SequenceBuilder].
pub struct BlockPiece {
    /// The opening delimiter.
    left_bracket: PieceId,

    /// The series of members or statements.
    elements: PieceId,

    /// The closing delimiter.
    right_bracket: PieceId,
}

impl BlockPiece {
    pub fn new(left_bracket: PieceId, elements: PieceId, right_bracket: PieceId) -> BlockPiece {
        BlockPiece {
            left_bracket,
            elements,
            right_bracket,
        }
    }
}

impl PieceImpl for BlockPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.format(self.left_bracket, false);
        writer.push_indent(Indent::Block);
        writer.set_shape_mode(ShapeMode::Block);
        writer.newline();
        writer.format(self.elements, false);
        writer.pop_indent();
        writer.newline();
        writer.set_shape_mode(ShapeMode::Merge);
        writer.format(self.right_bracket, false);
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.left_bracket);
        callback(self.elements);
        callback(self.right_bracket);
    }

    /// A [BlockPiece] is never empty and always splits between the
    /// delimiters.
    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        Some(true)
    }
}

/// An element inside a [SequencePiece].
///
/// Tracks the underlying [Piece] along with surrounding whitespace.
pub struct SequenceElementPiece {
    /// The indentation on the line before this element, relative to the
    /// surrounding [Piece].
    indent: Indent,

    /// The [Piece] for the element.
    pub piece: PieceId,

    /// The comments that should appear at the end of this element's line.
    pub hanging_comments: Vec<PieceId>,

    /// Whether there should be a blank line after this element.
    pub blank_after: bool,
}

impl SequenceElementPiece {
    pub fn new(indent: Indent, piece: PieceId) -> SequenceElementPiece {
        SequenceElementPiece {
            indent,
            piece,
            hanging_comments: Vec::new(),
            blank_after: false,
        }
    }
}

impl PieceImpl for SequenceElementPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.format(self.piece, false);

        for &comment in &self.hanging_comments {
            writer.space();
            writer.format(comment, false);
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.piece);
        for &comment in &self.hanging_comments {
            callback(comment);
        }
    }
}
