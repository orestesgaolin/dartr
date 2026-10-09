// Dart source: dart_style lib/src/piece/type_test.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for an `as` or `is` expression.
pub struct TypeTestPiece {
    /// The expression being tested or cast.
    expression: PieceId,

    /// The `as`, `is`, or `is!` operator.
    operator: PieceId,

    /// The type being tested against or cast to.
    ty: PieceId,

    /// Whether the expression can be block formatted.
    can_block_split: bool,
}

/// Allow the expression to block split.
///
/// Unlike most pieces that allow block splitting, the cost here isn't zero
/// because we would prefer to split at `=` if it lets the entire type test
/// expression fit on one line:
///
///     variable =
///         function(argument) as Type;
const BLOCK_SPLIT_EXPRESSION: State = State::new(1);

impl TypeTestPiece {
    pub fn new(
        expression: PieceId,
        operator: PieceId,
        ty: PieceId,
        can_block_split: bool,
    ) -> TypeTestPiece {
        TypeTestPiece {
            expression,
            operator,
            ty,
            can_block_split,
        }
    }
}

impl PieceImpl for TypeTestPiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.can_block_split {
            states.push(BLOCK_SPLIT_EXPRESSION);
        }
        states.push(State::SPLIT);
        states
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        if state == State::SPLIT {
            writer.push_indent(Indent::Expression);
        }

        writer.format(self.expression, false);
        writer.split_if(state == State::SPLIT);
        writer.format(self.operator, false);
        writer.space();
        writer.format(self.ty, false);

        if state == State::SPLIT {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.expression);
        callback(self.operator);
        callback(self.ty);
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if state == State::UNSPLIT {
            ShapeSet::ONLY_INLINE
        } else if state == BLOCK_SPLIT_EXPRESSION && child == self.expression {
            ShapeSet::ONLY_BLOCK
        } else {
            ShapeSet::ALL
        }
    }
}
