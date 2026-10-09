// Dart source: dart_style lib/src/piece/assign.dart

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for an assignment-like construct:
///
/// - Assignment (`=`, `+=`, etc.)
/// - Named arguments (`:`)
/// - Map entries (`:`)
/// - Record fields (`:`)
/// - Expression function bodies (`=>`)
///
/// Unlike other infix operators, these have some special formatting:
///
/// [State::UNSPLIT] No split at all:
///
///     var x = 123;
///
/// This state also allows splitting the right side if block shaped:
///
///     var list = [
///       element,
///     ];
///
/// [BLOCK_SPLIT_LEFT] Force the left-hand side, which must be a [ListPiece],
/// to split. Allow the right side to split or not. Allows all of:
///
///     var [
///       element,
///     ] = unsplitRhs;
///
///     var [
///       element,
///     ] = [
///       'block split RHS',
///     ];
///
///     var [
///       element,
///     ] = 'expression split' +
///         'the right hand side';
///
/// [BLOCK_OR_HEADLINE_SPLIT_RIGHT] Require the right-hand side to be block or
/// headline shaped and allow the left-side to expression split as in:
///
///     var (variable &&
///         anotherVariable) = [
///       element,
///     ];
///
/// [State::SPLIT] Split at the `=` or `in` operator and allow expression
/// splitting in either operand. Allows all of:
///
///     var (longVariable &&
///             anotherVariable) =
///         longOperand +
///             anotherOperand;
///
///     var [unsplitBlock] =
///         longOperand +
///             anotherOperand;
pub struct AssignPiece {
    /// The left-hand side of the operation and the operator itself.
    left: PieceId,

    /// The right-hand side of the operation.
    right: PieceId,

    /// Whether the piece should have a cost for splitting at the operator.
    ///
    /// Usually true because it's generally better to block split inside the
    /// operands when possible. But false for `=>` when the expression has a
    /// form where we'd rather keep the expression itself unsplit.
    avoid_split: bool,
}

/// Allow the right-hand side to block split.
const BLOCK_OR_HEADLINE_SPLIT_RIGHT: State = State::with_cost(1, 0);

/// Force the left-hand side to block split and allow the right-hand side to
/// split.
const BLOCK_SPLIT_LEFT: State = State::new(2);

impl AssignPiece {
    pub fn new(left: PieceId, right: PieceId) -> AssignPiece {
        AssignPiece::with_avoid_split(left, right, true)
    }

    pub fn with_avoid_split(left: PieceId, right: PieceId, avoid_split: bool) -> AssignPiece {
        AssignPiece {
            left,
            right,
            avoid_split,
        }
    }
}

impl PieceImpl for AssignPiece {
    fn additional_states(&self) -> States {
        States::of(&[
            BLOCK_OR_HEADLINE_SPLIT_RIGHT,
            BLOCK_SPLIT_LEFT,
            State::SPLIT,
        ])
    }

    fn state_cost(&self, state: State) -> i32 {
        if state == State::SPLIT {
            return if self.avoid_split { 1 } else { 0 };
        }
        state.cost()
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if state == State::UNSPLIT {
            ShapeSet::ONLY_INLINE
        } else if state == BLOCK_SPLIT_LEFT && child == self.left {
            ShapeSet::ONLY_BLOCK
        } else if state == BLOCK_SPLIT_LEFT && child == self.right {
            ShapeSet::INLINE_OR_OTHER
        } else if state == BLOCK_OR_HEADLINE_SPLIT_RIGHT && child == self.right {
            ShapeSet::BLOCK_OR_HEADLINE
        } else {
            ShapeSet::ALL
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        if state == State::SPLIT {
            // When splitting at the operator, indent the operands.
            writer.push_indent(Indent::Expression);

            // Treat a split `=` as potentially headline-shaped if the LHS
            // doesn't split. Allows:
            //
            //     variable = another =
            //         'split at second "="';
            writer.set_shape_mode(ShapeMode::BeforeHeadline);
            writer.format(self.left, false);
            writer.set_shape_mode(ShapeMode::AfterHeadline);

            writer.newline();
            writer.pop_indent();
            writer.push_indent(Indent::Assignment);
            writer.format(self.right, false);
            writer.pop_indent();
        } else {
            writer.format(self.left, false);
            writer.space();
            writer.format(self.right, false);
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.left);
        callback(self.right);
    }
}
