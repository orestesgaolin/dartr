// Dart source: dart_style lib/src/piece/infix.dart

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for a series of binary expressions at the same precedence, like:
///
///     a + b + c
pub struct InfixPiece {
    /// The series of operands.
    ///
    /// Since we don't split on both sides of the operator, the operators will
    /// be embedded in the operand pieces. If the operator is a hanging one,
    /// it will be in the preceding operand, so `1 + 2` becomes
    /// "Infix(`1 +`, `2`)". A leading operator like `foo as int` becomes
    /// "Infix(`foo`, `as int`)".
    operands: Vec<PieceId>,

    /// What kind of indentation should be applied to the subsequent operands.
    indent: Indent,

    /// `None` for the 3.7 style (Dart `_InfixPiece3Dot7`). For the 3.8 and
    /// later style (Dart `_InfixPiece`): whether this piece is for a
    /// conditional expression.
    is_conditional: Option<bool>,
}

impl InfixPiece {
    /// Creates an [InfixPiece] for the given series of [operands].
    pub fn new(operands: Vec<PieceId>, is_3_dot_7: bool, indent: Indent) -> InfixPiece {
        InfixPiece {
            operands,
            indent,
            is_conditional: if is_3_dot_7 { None } else { Some(false) },
        }
    }

    /// Creates an [InfixPiece] for a conditional (`?:`) expression.
    pub fn conditional(operands: Vec<PieceId>, is_3_dot_7: bool) -> InfixPiece {
        if is_3_dot_7 {
            InfixPiece {
                operands,
                indent: Indent::Expression,
                is_conditional: None,
            }
        } else {
            InfixPiece {
                operands,
                indent: Indent::Infix,
                is_conditional: Some(true),
            }
        }
    }
}

impl PieceImpl for InfixPiece {
    fn additional_states(&self) -> States {
        States::of(&[State::SPLIT])
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, _child: PieceId) -> ShapeSet {
        ShapeSet::any_if(state == State::SPLIT)
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for &operand in &self.operands {
            callback(operand);
        }
    }

    fn fixed_state_for_page_width(&self, pieces: &Pieces, page_width: i32) -> Option<State> {
        let mut total_length = 0;

        for &operand in &self.operands {
            // If any operand contains a newline, then we have to split.
            if pieces.contains_hard_newline(operand) {
                return Some(State::SPLIT);
            }

            total_length += pieces.total_characters(operand);
            if total_length > page_width {
                break;
            }
        }

        // If the total length doesn't fit in the page, then we have to split.
        if total_length > page_width {
            return Some(State::SPLIT);
        }

        None
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        match self.is_conditional {
            Some(is_conditional) => {
                writer.push_indent(self.indent);

                // If this is a conditional expression (or chain of them), then
                // allow the leading condition to be headline formatted in an
                // assignment, like:
                //
                //     variable = condition
                //         ? thenBranch
                //         : elseBranch;
                //
                // We only do this for conditional expressions and not other
                // infix operators because with other operators, the operands
                // are homogeneous and it makes more sense to split before the
                // first one so that they are aligned in parallel:
                //
                //     variable =
                //         operand +
                //         another;
                if is_conditional {
                    writer.set_shape_mode(ShapeMode::BeforeHeadline);
                }
                writer.format(self.operands[0], false);
                if is_conditional {
                    writer.set_shape_mode(ShapeMode::AfterHeadline);
                }

                for i in 1..self.operands.len() {
                    writer.split_if(state == State::SPLIT);

                    // If this is a branch of a conditional expression, then
                    // indent the branch's contents past the `?` or `:`.
                    if is_conditional {
                        writer.push_indent(Indent::Block);
                    }

                    // We can format each operand separately if the operand is
                    // on its own line. This happens when the operator is split
                    // and we aren't the first or last operand.
                    let separate = state == State::SPLIT && i < self.operands.len() - 1;
                    writer.format(self.operands[i], separate);
                    if is_conditional {
                        writer.pop_indent();
                    }
                }

                writer.pop_indent();
            }
            None => {
                writer.push_indent(self.indent);

                for i in 0..self.operands.len() {
                    // We can format each operand separately if the operand is
                    // on its own line. This happens when the operator is split
                    // and we aren't the first or last operand.
                    let separate =
                        state == State::SPLIT && i > 0 && i < self.operands.len() - 1;

                    writer.format(self.operands[i], separate);
                    if i < self.operands.len() - 1 {
                        writer.split_if(state == State::SPLIT);
                    }
                }

                writer.pop_indent();
            }
        }
    }
}
