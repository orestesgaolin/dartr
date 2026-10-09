// Dart source: dart_style lib/src/piece/type.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{Constrain, PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// Piece for a type declaration with a body containing members.
///
/// Used for class, enum, and extension declarations.
pub struct TypePiece {
    /// The leading keywords and modifiers, type name, type parameters, and
    /// any other `extends`, `with`, etc. clauses.
    header: PieceId,

    /// The type body.
    body: PieceId,

    /// What kind of body the type has.
    body_type: TypeBodyType,
}

impl TypePiece {
    pub fn new(header: PieceId, body: PieceId, body_type: TypeBodyType) -> TypePiece {
        TypePiece {
            header,
            body,
            body_type,
        }
    }
}

impl PieceImpl for TypePiece {
    fn additional_states(&self) -> States {
        if self.body_type == TypeBodyType::List {
            States::of(&[State::SPLIT])
        } else {
            States::EMPTY
        }
    }

    fn apply_constraints(&self, state: State, constrain: &mut Constrain) {
        // If the body may or may not split, force it to split when the header
        // does.
        if state == State::SPLIT {
            constrain(self.body, State::SPLIT);
        }
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if child == self.body {
            return ShapeSet::ALL;
        }

        // If the body may or may not split, then a newline in the header or
        // clauses forces the body to split.
        ShapeSet::any_if(self.body_type != TypeBodyType::List || state == State::SPLIT)
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.format(self.header, false);
        if self.body_type != TypeBodyType::Semicolon {
            writer.space();
        }
        writer.format(self.body, false);
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.header);
        callback(self.body);
    }
}

/// Piece for a type with a primary constructor (or extension type
/// representation type) in the header.
///
/// These use a separate [Piece] class to handle the interactions between
/// splitting in the constructor parameter list, and/or any subsequent
/// clauses in the header. There are a few ways it can split with some
/// constraints between them:
///
/// [State::UNSPLIT] Everything in the header on one line:
///
///     class C(int x) extends S {
///       body() {}
///     }
///
/// [BEFORE_CLAUSES] Split before every clause including the first. This is
/// only allowed when the parameter list does not split:
///
///     class C(int x, int y)
///         extends Super
///         implements I {
///       body() {}
///     }
///
/// [INLINE_CLAUSES] The parameter list must split and the clauses all fit on
/// one line between the `)` and the beginning of the body:
///
///     class C(
///       int x,
///       int y,
///     ) extends S {
///       body() {}
///     }
///
/// [BETWEEN_CLAUSES] The parameter list must split and all but the first
/// clause start their own lines:
///
///     class C(
///       int x,
///       int y,
///     ) extends S
///         implements I {
///       body() {}
///     }
///
/// [State::SPLIT] Similar to [BEFORE_CLAUSES] but allows the parameter list
/// to split too. Mainly so that if the constraints of the previous states
/// can't otherwise be solved, then solver can still pick an invalid
/// solution.
pub struct PrimaryTypePiece {
    /// The leading keywords and modifiers, type name, type parameters, and
    /// any other `extends`, `with`, etc. clauses.
    header: PieceId,

    /// The constructor's formal parameter list.
    parameters: PieceId,

    /// The `extends`, `with`, `implements`, etc. clauses, if any.
    clauses: Vec<PieceId>,

    /// The type body.
    body: PieceId,

    /// What kind of body the type has.
    body_type: TypeBodyType,
}

/// Split before all clauses and don't allow the parameter list to split.
const BEFORE_CLAUSES: State = State::new(1);

/// Keep all clauses inline and force the parameter list to split.
const INLINE_CLAUSES: State = State::new(2);

/// Split before every clause but the first.
const BETWEEN_CLAUSES: State = State::new(3);

impl PrimaryTypePiece {
    pub fn new(
        header: PieceId,
        parameters: PieceId,
        clauses: Vec<PieceId>,
        body: PieceId,
        body_type: TypeBodyType,
    ) -> PrimaryTypePiece {
        PrimaryTypePiece {
            header,
            parameters,
            clauses,
            body,
            body_type,
        }
    }
}

impl PieceImpl for PrimaryTypePiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if !self.clauses.is_empty() {
            states.push(BEFORE_CLAUSES);
            states.push(INLINE_CLAUSES);
        }
        states.push(BETWEEN_CLAUSES);
        states.push(State::SPLIT);
        states
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if child == self.header {
            return ShapeSet::ALL;
        }

        if state == State::UNSPLIT {
            // We only restrict the header from splitting.
            if child != self.body {
                return ShapeSet::ONLY_INLINE;
            }
        } else if state == BEFORE_CLAUSES {
            // The parameters can't split and the clauses can.
            if child == self.parameters {
                return ShapeSet::ONLY_INLINE;
            }
        } else if state == INLINE_CLAUSES {
            // The parameters must split and the clauses can't.
            if child == self.parameters {
                return ShapeSet::ONLY_BLOCK;
            }
            if self.clauses.contains(&child) {
                return ShapeSet::ONLY_INLINE;
            }
        } else if state == BETWEEN_CLAUSES {
            // The parameters must split and the clauses can.
            if child == self.parameters {
                return ShapeSet::ONLY_BLOCK;
            }
        }

        ShapeSet::ALL
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        writer.format(self.header, false);
        writer.format(self.parameters, false);

        // Indent all of the clauses if any will start a line.
        let indent = state == BEFORE_CLAUSES
            || state == BETWEEN_CLAUSES && self.clauses.len() > 1
            || state == State::SPLIT;
        if indent {
            writer.push_indent(Indent::Infix);
        }

        for &clause in &self.clauses {
            writer.split_if(
                state == BEFORE_CLAUSES
                    || state == BETWEEN_CLAUSES && clause != self.clauses[0]
                    || state == State::SPLIT,
            );
            writer.format(clause, false);
        }

        if indent {
            writer.pop_indent();
        }

        if self.body_type != TypeBodyType::Semicolon {
            writer.space();
        }
        writer.format(self.body, false);
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.header);
        callback(self.parameters);
        for &clause in &self.clauses {
            callback(clause);
        }
        callback(self.body);
    }
}

/// What kind of body is used for the type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeBodyType {
    /// An always-split block body, as in a class declaration.
    Block,

    /// A curly-brace delimited list that may or may not split.
    ///
    /// Used for enums with constants but no members.
    List,

    /// A single `;` body, used for mixin application classes.
    Semicolon,
}
