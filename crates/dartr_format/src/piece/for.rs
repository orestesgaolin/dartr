// Dart source: dart_style lib/src/piece/for.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for the `for (...)` part of a for statement or element.
pub struct ForPiece {
    /// The `for` keyword.
    for_keyword: PieceId,

    /// The part inside `( ... )`, including the parentheses themselves, at
    /// the header of a for statement.
    parts: PieceId,

    /// Whether the contents of the parentheses in the `for (...)` should be
    /// expression indented or not.
    ///
    /// This is usually not necessary because the contents will either be a
    /// [ListPiece] which adds its own block indentation, or an [AssignPiece]
    /// which indents as necessary. But in the rare case the for-parts is a
    /// variable or pattern variable declaration with metadata that splits, we
    /// need to ensure that the metadata is indented, as in:
    ///
    ///     for (@LongAnnotation
    ///         @AnotherAnnotation
    ///         var element in list) { ... }
    indent: bool,
}

impl ForPiece {
    pub fn new(for_keyword: PieceId, parts: PieceId, indent: bool) -> ForPiece {
        ForPiece {
            for_keyword,
            parts,
            indent,
        }
    }
}

impl PieceImpl for ForPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.format(self.for_keyword, false);
        writer.space();
        if self.indent {
            writer.push_collapsible_indent();
        }
        writer.format(self.parts, false);
        if self.indent {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.for_keyword);
        callback(self.parts);
    }
}

/// A piece for the `<variable> in <expression>` part of a for-in loop.
///
/// Can be formatted two ways:
///
/// [State::UNSPLIT] No split at all:
///
///     for (var x in y) ...
///
/// This state also allows splitting the sequence expression if it's block
/// shaped:
///
///     for (var i in [
///       element1,
///       element2,
///       element3,
///     ];
///
/// [State::SPLIT] Split at the `in` operator and allow expression splitting
/// on either side. Allows:
///
///     for (var (longVariable &&
///             anotherVariable)
///         in longOperand +
///             anotherOperand) {
///       ...
///     }
pub struct ForInPiece {
    /// The variable or pattern initialized with each loop iteration.
    variable: PieceId,

    /// The `in` keyword followed by the sequence expression.
    sequence: PieceId,

    /// `None` for the 3.8 and later style (Dart `_ForInPiece`). For the 3.7
    /// style (Dart `_ForInPiece3Dot7`): if `true` then the sequence
    /// expression supports being block-formatted, like:
    ///
    ///     for (var e in [
    ///       element1,
    ///       element2,
    ///     ]) {
    ///       // ...
    ///     }
    can_block_split_sequence_3_dot_7: Option<bool>,
}

impl ForInPiece {
    pub fn new(
        variable: PieceId,
        sequence: PieceId,
        can_block_split_sequence: bool,
        is_3_dot_7: bool,
    ) -> ForInPiece {
        ForInPiece {
            variable,
            sequence,
            can_block_split_sequence_3_dot_7: if is_3_dot_7 {
                Some(can_block_split_sequence)
            } else {
                None
            },
        }
    }
}

impl PieceImpl for ForInPiece {
    fn additional_states(&self) -> States {
        States::of(&[State::SPLIT])
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        match self.can_block_split_sequence_3_dot_7 {
            None => {
                if state == State::UNSPLIT && child == self.sequence {
                    // Always allow block-splitting the sequence if it
                    // supports it.
                    ShapeSet::INLINE_OR_BLOCK
                } else if state == State::UNSPLIT {
                    ShapeSet::ONLY_INLINE
                } else {
                    ShapeSet::ALL
                }
            }
            Some(can_block_split_sequence) => {
                if state == State::SPLIT {
                    return ShapeSet::ALL;
                }

                // Always allow block-splitting the sequence if it supports it.
                ShapeSet::any_if(child == self.sequence && can_block_split_sequence)
            }
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        // When splitting at `in`, both operands may split or not and will be
        // indented if they do.
        if state == State::SPLIT {
            writer.push_indent(Indent::Expression);
        }

        writer.format(self.variable, false);
        writer.split_if(state == State::SPLIT);
        writer.format(self.sequence, false);

        if state == State::SPLIT {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.variable);
        callback(self.sequence);
    }
}
