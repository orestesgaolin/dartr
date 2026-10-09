// Dart source: dart_style lib/src/piece/case.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// Piece for a case pattern, guard, and body in a switch expression.
pub struct CaseExpressionPiece {
    /// The pattern the value is matched against.
    pattern: PieceId,

    /// If there is a `when` clause, that clause.
    guard: Option<PieceId>,

    /// The `=>` token separating the pattern and body.
    arrow: PieceId,

    /// The case body expression.
    body: PieceId,

    /// Whether the pattern can be block formatted.
    can_block_split_pattern: bool,

    /// Whether the outermost pattern is a logical-or pattern.
    ///
    /// We format these specially to make them look like parallel cases:
    ///
    ///     switch (obj) {
    ///       firstPattern ||
    ///       secondPattern ||
    ///       thirdPattern =>
    ///         body;
    ///     }
    pattern_is_logical_or: bool,

    /// Whether the body expression can be block formatted.
    can_block_split_body: bool,
}

/// Split inside the body, which must be block shaped, like:
///
///     pattern => function(
///       argument,
///     ),
const BLOCK_SPLIT_BODY: State = State::with_cost(1, 0);

/// Split after the `=>` before the body.
const BEFORE_BODY: State = State::new(2);

/// Split before the `when` guard clause and after the `=>`.
const BEFORE_WHEN_AND_BODY: State = State::new(3);

impl CaseExpressionPiece {
    pub fn new(
        pattern: PieceId,
        guard: Option<PieceId>,
        arrow: PieceId,
        body: PieceId,
        can_block_split_pattern: bool,
        pattern_is_logical_or: bool,
        can_block_split_body: bool,
    ) -> CaseExpressionPiece {
        CaseExpressionPiece {
            pattern,
            guard,
            arrow,
            body,
            can_block_split_pattern,
            pattern_is_logical_or,
            can_block_split_body,
        }
    }
}

impl PieceImpl for CaseExpressionPiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.can_block_split_body {
            states.push(BLOCK_SPLIT_BODY);
        }
        states.push(BEFORE_BODY);
        if self.guard.is_some() {
            states.push(BEFORE_WHEN_AND_BODY);
        }
        states
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        // If the outermost pattern is `||`, then always let it split even
        // while allowing the body on the same line as `=>`.
        if child == self.pattern && self.pattern_is_logical_or {
            return ShapeSet::ALL;
        }

        // There are almost never splits in the arrow piece. It requires a
        // comment in a funny location, but if it happens, allow it.
        if child == self.arrow {
            return ShapeSet::ALL;
        }
        if state == BLOCK_SPLIT_BODY && child == self.body {
            return ShapeSet::ONLY_BLOCK;
        }

        // Only allow the pattern to split if there is no guard.
        if state == BEFORE_BODY && child == self.pattern {
            return ShapeSet::any_if(self.guard.is_none());
        }
        if state == BEFORE_BODY && child == self.body {
            return ShapeSet::ALL;
        }

        if state == BEFORE_WHEN_AND_BODY {
            return ShapeSet::ALL;
        }
        ShapeSet::ONLY_INLINE
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        // If there is a split guard, then indent the pattern past it.
        let indent_pattern_for_guard = !self.can_block_split_pattern
            && !self.pattern_is_logical_or
            && state == BEFORE_WHEN_AND_BODY;

        if indent_pattern_for_guard {
            writer.push_indent(Indent::Expression);
        }

        writer.format(self.pattern, false);

        if indent_pattern_for_guard {
            writer.pop_indent();
        }

        if let Some(guard) = self.guard {
            writer.push_indent(Indent::Expression);
            writer.split_if(state == BEFORE_WHEN_AND_BODY);
            writer.format(guard, false);
            writer.pop_indent();
        }

        writer.space();
        writer.format(self.arrow, false);

        let indent_body = state != State::UNSPLIT && state != BLOCK_SPLIT_BODY;
        if indent_body {
            writer.push_indent(Indent::Block);
        }

        writer.split_if(state == BEFORE_BODY || state == BEFORE_WHEN_AND_BODY);
        writer.format(self.body, false);

        if indent_body {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.pattern);
        if let Some(guard) = self.guard {
            callback(guard);
        }
        callback(self.arrow);
        callback(self.body);
    }
}
