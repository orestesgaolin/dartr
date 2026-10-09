// Dart source: dart_style lib/src/piece/variable.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A variable declaration.
///
/// Used for local variable declaration statements, top-level variable
/// declarations and field declarations. Also used to handle splitting
/// between a function or function type's return type and the rest of the
/// function.
///
/// Typed and untyped variables have slightly different splitting logic.
/// Untyped variables never split after the keyword but do indent subsequent
/// variables:
///
///     var longVariableName = initializer,
///         anotherVariable = anotherInitializer;
///
/// Typed variables can split that way too:
///
///     String longVariableName = initializer,
///         anotherVariable = anotherInitializer;
///
/// But they can also split after the type annotation. When that happens, the
/// variables aren't indented:
///
///     VeryLongTypeName
///     longVariableName = initializer,
///     anotherVariable = anotherInitializer;
pub struct VariablePiece {
    /// The leading keywords (`var`, `final`, `late`) and optional type
    /// annotation.
    header: PieceId,

    /// Each individual variable being declared.
    variables: Vec<PieceId>,

    /// Whether the variable declaration has a type annotation.
    has_type: bool,

    /// Whether we are using the 3.7 style.
    is_3_dot_7: bool,
}

/// Split between each variable in a multiple variable declaration.
const BETWEEN_VARIABLES: State = State::new(1);

/// Split after the type annotation and between each variable.
const AFTER_TYPE: State = State::with_cost(2, 2);

impl VariablePiece {
    /// Creates a [VariablePiece].
    ///
    /// The [has_type] parameter should be `true` if the variable declaration
    /// has a type annotation.
    pub fn new(
        header: PieceId,
        variables: Vec<PieceId>,
        has_type: bool,
        is_3_dot_7: bool,
    ) -> VariablePiece {
        VariablePiece {
            header,
            variables,
            has_type,
            is_3_dot_7,
        }
    }
}

impl PieceImpl for VariablePiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.variables.len() > 1 {
            states.push(BETWEEN_VARIABLES);
        }
        if self.has_type {
            states.push(AFTER_TYPE);
        }
        states
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        // If the variable doesn't allow any other states (because it's just
        // `var x` etc.) then allow any shape. That way, if there's a comment
        // inside, the solver doesn't get confused trying to invalidate the
        // VariablePiece.
        if !self.is_3_dot_7 && self.variables.len() == 1 && !self.has_type {
            return ShapeSet::ALL;
        }

        if child == self.header {
            ShapeSet::any_if(state != State::UNSPLIT)
        } else {
            ShapeSet::any_if(self.variables.len() == 1 || state != State::UNSPLIT)
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        writer.format(self.header, false);

        // If we split at the variables (but not the type), then indent the
        // variables and their initializers.
        if state == BETWEEN_VARIABLES {
            writer.push_indent(Indent::Expression);
        }

        // Split after the type annotation.
        writer.split_if(state == AFTER_TYPE);

        for (i, &variable) in self.variables.iter().enumerate() {
            // Split between variables.
            if i > 0 {
                writer.split_if(state != State::UNSPLIT);
            }

            writer.format(variable, false);
        }

        if state == BETWEEN_VARIABLES {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.header);
        for &variable in &self.variables {
            callback(variable);
        }
    }
}
