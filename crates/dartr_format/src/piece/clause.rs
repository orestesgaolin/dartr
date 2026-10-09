// Dart source: dart_style lib/src/piece/clause.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A list of "clauses" where each clause starts with a keyword and has a
/// comma-separated list of items under it.
///
/// Used for `show` and `hide` combinators in import and export directives,
/// and `extends`, `implements`, and `with` clauses in type declarations.
///
/// Clauses can be chained on one line if they all fit, like:
///
///     import 'animals.dart' show Ant, Bat hide Cat, Dog;
///
/// Or can split before all of the clauses, like:
///
///     import 'animals.dart'
///         show Ant, Bat
///         hide Cat, Dog;
///
/// They can also split before every item in any of the clauses. If they do
/// so, then the clauses must split too.
///
/// This ensures that when any wrapping occurs, the keywords are always at the
/// beginning of the line.
pub struct ClausePiece {
    /// The leading construct the clauses are applied to: a class declaration,
    /// import directive, etc.
    header: PieceId,

    clauses: Vec<PieceId>,

    /// If `true`, then we're allowed to split between the clauses without
    /// splitting before the first one too.
    ///
    /// This is used for class declarations where the `extends` clauses is
    /// treated a little specially because it's a deeper coupling to the class
    /// and so we want it to stay on the top line even if the other clauses
    /// split, like:
    ///
    ///     class BaseClass extends Derived
    ///         implements OtherThing {
    ///       ...
    ///     }
    allow_leading_clause: bool,
}

/// State where we split between the clauses but not before the first one.
const BETWEEN_CLAUSES: State = State::new(1);

impl ClausePiece {
    pub fn new(header: PieceId, clauses: Vec<PieceId>, allow_leading_clause: bool) -> ClausePiece {
        let allow_leading_clause = allow_leading_clause && clauses.len() > 1;
        ClausePiece {
            header,
            clauses,
            allow_leading_clause,
        }
    }
}

impl PieceImpl for ClausePiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.allow_leading_clause {
            states.push(BETWEEN_CLAUSES);
        }
        states.push(State::SPLIT);
        states
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if child == self.header {
            // If the header splits, force the clauses to split too.
            ShapeSet::any_if(state == State::SPLIT)
        } else if self.allow_leading_clause && child == self.clauses[0] {
            // A split inside the first clause forces a split before the
            // keyword.
            ShapeSet::any_if(state == State::SPLIT)
        } else {
            // For the other clauses (or if there is no leading one), any
            // split inside a clause forces all of them to split.
            ShapeSet::any_if(state != State::UNSPLIT)
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        writer.format(self.header, false);

        writer.push_indent(Indent::Expression);

        for &clause in &self.clauses {
            if self.allow_leading_clause && clause == self.clauses[0] {
                // Before the leading clause, only split when in the fully
                // split state. A split inside the first clause forces a split
                // before the keyword.
                writer.split_if(state == State::SPLIT);
                writer.format(clause, false);
            } else {
                // For the other clauses (or if there is no leading one), split
                // in the fully split state and any split inside and clause
                // forces all of them to split.
                writer.split_if(state != State::UNSPLIT);
                writer.format(clause, false);
            }
        }

        writer.pop_indent();
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.header);
        for &clause in &self.clauses {
            callback(clause);
        }
    }
}
