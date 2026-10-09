// Dart source: dart_style lib/src/piece/type_parameter_bound.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A piece for a type parameter and its bound.
///
/// Handles not splitting before `extends` if we can split inside the bound's
/// own type arguments, or splitting before `extends` if that isn't enough to
/// get it to fit.
pub struct TypeParameterBoundPiece {
    /// The type parameter name.
    type_parameter: PieceId,

    /// The bound with the preceding `extends` keyword.
    bound: PieceId,
}

/// Split inside the type arguments of the bound, but not at `extends`, as in:
///
///     class C<
///       T extends Map<
///         LongKeyType,
///         LongValueType
///       >
///     >{}
const INSIDE_BOUND: State = State::new(1);

/// Split at `extends`, like:
///
///     class C<
///       LongTypeParameters
///           extends LongBoundType
///     >{}
const BEFORE_EXTENDS: State = State::new(2);

impl TypeParameterBoundPiece {
    pub fn new(type_parameter: PieceId, bound: PieceId) -> TypeParameterBoundPiece {
        TypeParameterBoundPiece {
            type_parameter,
            bound,
        }
    }
}

impl PieceImpl for TypeParameterBoundPiece {
    fn additional_states(&self) -> States {
        States::of(&[INSIDE_BOUND, BEFORE_EXTENDS])
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if state == State::UNSPLIT {
            ShapeSet::ONLY_INLINE
        } else if state == INSIDE_BOUND {
            ShapeSet::any_if(child == self.bound)
        } else if state == BEFORE_EXTENDS {
            ShapeSet::ALL
        } else {
            panic!("Unexpected state.")
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        if state == BEFORE_EXTENDS {
            writer.push_indent(Indent::Expression);
        }
        writer.format(self.type_parameter, false);
        writer.split_if(state == BEFORE_EXTENDS);
        writer.format(self.bound, false);
        if state == BEFORE_EXTENDS {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.type_parameter);
        callback(self.bound);
    }
}
