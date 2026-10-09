// Dart source: dart_style lib/src/piece/assign_3_dot_7.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{Constrain, PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for the 3.7 style of an assignment-like construct where an
/// operator is followed by an expression but where the left side of the
/// operator isn't also an expression.
///
/// Used for:
///
/// - Assignment (`=`, `+=`, etc.)
/// - Named arguments (`:`)
/// - Map entries (`:`)
/// - Record fields (`:`)
/// - Expression function bodies (`=>`)
///
/// These constructs can be formatted four ways:
///
/// [State::UNSPLIT] No split at all:
///
///     var x = 123;
///
/// This state also allows splitting the right side if it can be block
/// formatted:
///
///     var list = [
///       element,
///     ];
///
/// [BLOCK_SPLIT_LEFT] Force the left-hand side, which must be a [ListPiece],
/// to split. Allow the right side to split or not.
///
/// [BLOCK_SPLIT_RIGHT] Allow the right-hand side to block split or not, if
/// it wants. Since [State::UNSPLIT] and [BLOCK_SPLIT_LEFT] also allow the
/// right-hand side to block split, this state is only used when the
/// left-hand side expression splits, like:
///
///     var (variable &&
///         anotherVariable) = [
///       element,
///     ];
///
/// [AT_OPERATOR] Split at the `=` or `in` operator and allow expression
/// splitting in either operand.
pub struct AssignPiece3Dot7 {
    /// The left-hand side of the operation.
    left: Option<PieceId>,

    /// The `=` or other operator.
    operator: PieceId,

    /// The right-hand side of the operation.
    right: PieceId,

    /// If `true`, then the left side supports being block-formatted, like:
    ///
    ///     var [
    ///       element1,
    ///       element2,
    ///     ] = value;
    can_block_split_left: bool,

    /// If `true` then the right side supports being block-formatted, like:
    ///
    ///     var list = [
    ///       element1,
    ///       element2,
    ///     ];
    can_block_split_right: bool,

    /// If `true` then prefer to split at the operator instead of block
    /// splitting the right side.
    ///
    /// This is `true` for `=>` functions whose body is a function call. This
    /// keeps the called function next to its arguments instead having the
    /// function name stick to the `=>` while the arguments split.
    avoid_block_split_right: bool,
}

/// Force the block left-hand side to split and allow the right-hand side to
/// split.
const BLOCK_SPLIT_LEFT: State = State::new(1);

/// Allow the right-hand side to block split.
const BLOCK_SPLIT_RIGHT: State = State::with_cost(2, 0);

/// Split at the operator.
const AT_OPERATOR: State = State::new(3);

impl AssignPiece3Dot7 {
    pub fn new(
        operator: PieceId,
        right: PieceId,
        left: Option<PieceId>,
        can_block_split_left: bool,
        can_block_split_right: bool,
        avoid_block_split_right: bool,
    ) -> AssignPiece3Dot7 {
        AssignPiece3Dot7 {
            left,
            operator,
            right,
            can_block_split_left,
            can_block_split_right,
            avoid_block_split_right,
        }
    }
}

impl PieceImpl for AssignPiece3Dot7 {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        // If at least one operand can block split, allow splitting in
        // operands without splitting at the operator.
        if self.can_block_split_left {
            states.push(BLOCK_SPLIT_LEFT);
        }
        if self.can_block_split_right {
            states.push(BLOCK_SPLIT_RIGHT);
        }
        states.push(AT_OPERATOR);
        states
    }

    fn state_cost(&self, state: State) -> i32 {
        // Allow block splitting the right side, but increase the cost so that
        // we prefer splitting at the operator and not splitting in the right
        // piece if possible.
        if state == BLOCK_SPLIT_RIGHT && self.avoid_block_split_right {
            return 1;
        }

        state.cost()
    }

    fn apply_constraints(&self, state: State, constrain: &mut Constrain) {
        // Force the left side to block split when in that state.
        //
        // Otherwise, the solver may instead leave it unsplit and then split
        // the right side incorrectly as in:
        //
        //  (x, y) = longOperand2 +
        //      longOperand2 +
        //      longOperand3;
        if state == BLOCK_SPLIT_LEFT {
            constrain(self.left.unwrap(), State::SPLIT);
        }
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, _child: PieceId) -> ShapeSet {
        ShapeSet::any_if(state != State::UNSPLIT)
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        // When splitting at the operator, both operands may split or not and
        // will be indented if they do.
        if state == AT_OPERATOR {
            writer.push_indent(Indent::Expression);
        }

        if let Some(left) = self.left {
            writer.format(left, false);
        }

        writer.push_indent(Indent::Expression);
        writer.format(self.operator, false);
        writer.pop_indent();
        writer.split_if(state == AT_OPERATOR);

        // If the left side block splits and the right doesn't, then indent
        // the right side if it splits as in:
        //
        //     var [
        //       a,
        //       b,
        //     ] = long +
        //         expression;
        let indent_right = state == BLOCK_SPLIT_LEFT && !self.can_block_split_right;
        if indent_right {
            writer.push_indent(Indent::Expression);
        }
        writer.format(self.right, false);
        if indent_right {
            writer.pop_indent();
        }

        if state == AT_OPERATOR {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        if let Some(left) = self.left {
            callback(left);
        }
        callback(self.operator);
        callback(self.right);
    }

    fn fixed_state_for_page_width(&self, pieces: &Pieces, page_width: i32) -> Option<State> {
        // If either side (or both) can block split, then they may allow a
        // long assignment to still not end up splitting at the operator.
        if self.can_block_split_left || self.can_block_split_right {
            return None;
        }

        // Edge case: If the left operand is only a single character, then
        // splitting at the operator won't actually make the line any smaller,
        // so don't apply the optimization in that case:
        //
        //     e = someVeryLongExpression;
        //
        // Is no worse than:
        //
        //     e =
        //         someVeryLongExpression;
        if let Some(left) = self.left {
            if pieces.total_characters(left) == 1 {
                return None;
            }
        }

        // If either operand contains a newline or the whole assignment
        // doesn't fit then it will split at the operator since there's no
        // other way it can split because there are no block operands.
        let mut total_length = 0;
        if let Some(left) = self.left {
            if !self.can_block_split_left {
                if pieces.contains_hard_newline(left) {
                    return Some(AT_OPERATOR);
                }

                total_length += pieces.total_characters(left);
            }
        }

        total_length += pieces.total_characters(self.operator);

        if !self.can_block_split_right {
            if pieces.contains_hard_newline(self.right) {
                return Some(AT_OPERATOR);
            }
            total_length += pieces.total_characters(self.right);
        }

        if total_length > page_width {
            return Some(AT_OPERATOR);
        }

        None
    }
}
