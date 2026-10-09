// Dart source: dart_style lib/src/piece/constructor.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{Constrain, PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A constructor declaration.
///
/// This is somewhat similar to [FunctionPiece], but constructor initializers
/// add a lot of complexity. In particular, there are constraints between how
/// the parameter list is allowed to split and how the initializer list is
/// allowed to split. Only a few combinations of splits are allowed:
///
/// [State::UNSPLIT] No splits at all, in the parameters or initializers.
///
///       SomeClass(param) : a = 1, b = 2;
///
/// [SPLIT_BEFORE_INITIALIZERS] Split before the `:` and between the
/// initializers but not in the parameters.
///
///       SomeClass(param)
///         : a = 1,
///           b = 2;
///
/// [SPLIT_BETWEEN_INITIALIZERS] Split between the initializers but not
/// before the `:`. This state should only be chosen when the parameters
/// split. If there are no parameters, this state is excluded.
///
///       SomeClass(
///         param
///       ) : a = 1,
///           b = 2;
///
/// In addition, this piece deals with indenting initializers appropriately
/// based on whether the parameter list has a `]` or `}` before the `)`. If
/// there are optional parameters, then initializers after the first are
/// indented one space more to line up with the first initializer.
pub struct ConstructorPiece {
    /// Whether there are parameters or comments inside the parameter list.
    ///
    /// If so, then we allow splitting the parameter list while leaving the
    /// `:` on the same line as the `)`.
    can_split_parameters: bool,

    /// Whether the parameter list contains a `]` or `}` closing delimiter
    /// before the `)`.
    has_optional_parameter: bool,

    /// The leading keywords, class name, and constructor name.
    header: PieceId,

    /// The constructor parameter list.
    ///
    /// Will be `None` if the constructor is a primary constructor
    /// initializer body.
    parameters: Option<PieceId>,

    /// If this is a redirecting constructor, the redirection clause.
    redirect: Option<PieceId>,

    /// If there are initializers, the `:` before them.
    initializer_separator: Option<PieceId>,

    /// The constructor initializers, if there are any.
    initializers: Option<PieceId>,

    /// The constructor body.
    body: PieceId,
}

const SPLIT_BEFORE_INITIALIZERS: State = State::with_cost(1, 1);

const SPLIT_BETWEEN_INITIALIZERS: State = State::with_cost(2, 2);

impl ConstructorPiece {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        header: PieceId,
        parameters: PieceId,
        body: PieceId,
        can_split_parameters: bool,
        has_optional_parameter: bool,
        redirect: Option<PieceId>,
        initializer_separator: Option<PieceId>,
        initializers: Option<PieceId>,
    ) -> ConstructorPiece {
        ConstructorPiece {
            can_split_parameters,
            has_optional_parameter,
            header,
            parameters: Some(parameters),
            redirect,
            initializer_separator,
            initializers,
            body,
        }
    }

    /// Creates a constructor piece for a primary constructor initializer
    /// body.
    pub fn this_block(
        header: PieceId,
        body: PieceId,
        initializer_separator: Option<PieceId>,
        initializers: Option<PieceId>,
    ) -> ConstructorPiece {
        ConstructorPiece {
            can_split_parameters: false,
            has_optional_parameter: false,
            header,
            parameters: None,
            redirect: None,
            initializer_separator,
            initializers,
            body,
        }
    }
}

impl PieceImpl for ConstructorPiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.initializers.is_some() {
            states.push(SPLIT_BEFORE_INITIALIZERS);
        }
        if self.can_split_parameters && self.initializers.is_some() {
            states.push(SPLIT_BETWEEN_INITIALIZERS);
        }
        states
    }

    /// Apply constraints between how the parameters may split and how the
    /// initializers may split.
    fn apply_constraints(&self, state: State, constrain: &mut Constrain) {
        // If there are both parameters and initializers, constrain how they
        // interact.
        if let (Some(parameters), Some(initializers)) = (self.parameters, self.initializers) {
            if state == State::UNSPLIT {
                // All parameters and initializers on one line.
                constrain(parameters, State::UNSPLIT);
                constrain(initializers, State::UNSPLIT);
            } else if state == SPLIT_BEFORE_INITIALIZERS {
                // Only split before the `:` when the parameters fit on one
                // line.
                constrain(parameters, State::UNSPLIT);
                constrain(initializers, State::SPLIT);
            } else if state == SPLIT_BETWEEN_INITIALIZERS {
                // Split both the parameters and initializers and put the `) :`
                // on its own line.
                constrain(parameters, State::SPLIT);
                constrain(initializers, State::SPLIT);
            }
        }
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if child == self.body {
            return ShapeSet::ALL;
        }

        // If there's a newline in the header or parameters (like a line
        // comment after the `)`), then don't allow the initializers to remain
        // unsplit.
        ShapeSet::any_if(self.initializers.is_none() || state != State::UNSPLIT)
    }

    fn contains_newline(&self, pieces: &Pieces, id: PieceId, state: State) -> bool {
        state == SPLIT_BEFORE_INITIALIZERS
            || state != State::UNSPLIT
            || pieces.contains_hard_newline(id)
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        writer.format(self.header, false);
        if let Some(parameters) = self.parameters {
            writer.format(parameters, false);
        }

        if let Some(redirect) = self.redirect {
            writer.space();
            writer.format(redirect, false);
        }

        if let Some(initializers) = self.initializers {
            writer.push_indent(Indent::Block);
            writer.split_if(state == SPLIT_BEFORE_INITIALIZERS);

            writer.format(self.initializer_separator.unwrap(), false);
            writer.space();

            // Indent subsequent initializers past the `:`.
            if self.has_optional_parameter && state == SPLIT_BETWEEN_INITIALIZERS {
                writer.push_indent(Indent::InitializerWithOptionalParameter);
            } else {
                writer.push_indent(Indent::Initializer);
            }

            writer.format(initializers, false);
            writer.pop_indent();
            writer.pop_indent();
        }

        writer.format(self.body, false);
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.header);
        // Dart has `if (_redirect case var parameters?) callback(parameters);`
        // here, so it visits the redirect twice and never the parameters.
        // Kept as is because it affects the metrics and constraints.
        if let Some(parameters) = self.redirect {
            callback(parameters);
        }
        if let Some(redirect) = self.redirect {
            callback(redirect);
        }
        if let Some(separator) = self.initializer_separator {
            callback(separator);
        }
        if let Some(initializers) = self.initializers {
            callback(initializers);
        }
        callback(self.body);
    }
}
