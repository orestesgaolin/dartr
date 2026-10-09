// Dart source: dart_style lib/src/piece/control_flow.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{Constrain, PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for an if statement or element, while statement, or for statement
/// without a block body.
pub struct ControlFlowPiece {
    /// Whether this is an if statement versus if collection element.
    ///
    /// It's not meaningful for while and for statements/elements.
    is_statement: bool,

    sections: Vec<Section>,
}

impl ControlFlowPiece {
    pub fn new(is_statement: bool) -> ControlFlowPiece {
        ControlFlowPiece {
            is_statement,
            sections: Vec::new(),
        }
    }

    pub fn add(&mut self, header: PieceId, statement: PieceId, is_block: bool) {
        self.sections.push(Section {
            header,
            statement,
            is_block,
        });
    }
}

impl PieceImpl for ControlFlowPiece {
    fn additional_states(&self) -> States {
        States::of(&[State::SPLIT])
    }

    fn apply_constraints(&self, state: State, constrain: &mut Constrain) {
        // In an if element, any spread collection's split state must follow
        // the surrounding if element's: we either split all the spreads or
        // none of them. And if any of the non-spread then or else branches
        // split, then the spreads do too.
        if !self.is_statement {
            for section in &self.sections {
                if section.is_block {
                    constrain(section.statement, state);
                }
            }
        }
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, _child: PieceId) -> ShapeSet {
        ShapeSet::any_if(state == State::SPLIT)
    }

    fn contains_newline(&self, pieces: &Pieces, id: PieceId, state: State) -> bool {
        if state == State::SPLIT {
            for section in &self.sections {
                if !section.is_block {
                    return true;
                }
            }
        }

        state != State::UNSPLIT || pieces.contains_hard_newline(id)
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        for (i, section) in self.sections.iter().enumerate() {
            // A split in the condition forces the branches to split.
            writer.format(section.header, false);

            if !section.is_block {
                writer.push_indent(Indent::Block);
                writer.split_if(state == State::SPLIT);
            }

            // TODO(rnystrom): Investigate whether it's worth using `separate:`
            // here.
            writer.format(section.statement, false);

            // Reset the indentation for the subsequent `else` or `} else`
            // line.
            if !section.is_block {
                writer.pop_indent();
            }

            if i < self.sections.len() - 1 {
                writer.split_if(state == State::SPLIT && !section.is_block);
            }
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for section in &self.sections {
            callback(section.header);
            callback(section.statement);
        }
    }
}

/// A single branch in a chain of if-elses or body of a for or while.
///
/// For the first then branch, the [header] is the `if (condition)` part and
/// the statement is the then branch. For all `else if` branches, the
/// [header] is the `else if (condition)` and the statement is the subsequent
/// then branch. For the final `else` branch, if there is one, the [header]
/// is just `else` and the statement is the else branch.
struct Section {
    header: PieceId,
    statement: PieceId,

    /// Whether the [statement] piece is from a block or a spread collection
    /// literal.
    is_block: bool,
}
