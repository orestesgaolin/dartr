// Dart source: dart_style lib/src/piece/if_case.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// Piece for the contents inside the parentheses for an if-case statement or
/// element: the expression, `case`, pattern, and `when` clause, if any.
///
/// They can split a few different ways:
///
/// [State::UNSPLIT] All on one line:
///
///     if (obj case pattern when cond) ...
///
/// The pattern may also be block-formatted in this state:
///
///     if (obj case [
///       subpattern,
///     ] when cond) ...
///
/// [BEFORE_WHEN] Split before the guard clause but not the pattern:
///
///     if (obj case pattern
///         when cond) ...
///
/// [BEFORE_CASE] Split before the `case` clause but not the guard:
///
///     if (obj
///         case pattern when cond) ...
///
/// [BEFORE_CASE_AND_WHEN] Split before both `case` and `when`:
///
///     if (obj
///         case pattern
///         when cond) ...
pub struct IfCasePiece {
    /// The value expression being matched.
    value: PieceId,

    /// The pattern the value is matched against along with the leading
    /// `case`.
    pattern: PieceId,

    /// If there is a `when` clause, that clause.
    guard: Option<PieceId>,

    /// Whether the pattern can be block formatted.
    can_block_split_pattern: bool,

    /// Dart `_IfCasePieceBlockFormatWithGuard`: the behavior before 3.13
    /// where a block-formatted pattern may be followed by a guard on the
    /// same line.
    block_format_with_guard: bool,
}

/// Split before the `when` guard clause.
const BEFORE_WHEN: State = State::new(1);

/// Split before the `case` pattern clause.
const BEFORE_CASE: State = State::new(2);

/// Split before the `case` pattern clause and the `when` guard clause.
const BEFORE_CASE_AND_WHEN: State = State::new(3);

impl IfCasePiece {
    pub fn new(
        value: PieceId,
        pattern: PieceId,
        guard: Option<PieceId>,
        can_block_split_pattern: bool,
        can_block_format_pattern_with_guard: bool,
    ) -> IfCasePiece {
        IfCasePiece {
            value,
            pattern,
            guard,
            can_block_split_pattern,
            block_format_with_guard: can_block_format_pattern_with_guard,
        }
    }
}

impl PieceImpl for IfCasePiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.guard.is_some() {
            states.push(BEFORE_WHEN);
        }
        states.push(BEFORE_CASE);
        if self.guard.is_some() {
            states.push(BEFORE_CASE_AND_WHEN);
        }
        states
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        let is_guard = self.guard == Some(child);
        if self.block_format_with_guard {
            // When not splitting before `case` or `when`, we only allow
            // newlines in block-formatted patterns.
            if state == State::UNSPLIT && child == self.pattern {
                return ShapeSet::any_if(self.can_block_split_pattern);
            }

            // Allow newlines only in the guard if we split before `when`.
            if state == BEFORE_WHEN && is_guard {
                return ShapeSet::ALL;
            }

            // Only allow the guard on the same line as the pattern if it
            // doesn't split.
            if state == BEFORE_CASE && !is_guard {
                return ShapeSet::ALL;
            }
            if state == BEFORE_CASE_AND_WHEN {
                return ShapeSet::ALL;
            }
            return ShapeSet::ONLY_INLINE;
        }

        // When not splitting before `case` or `when`, we only allow splitting
        // a block-formatted pattern if there is no guard.
        if state == State::UNSPLIT
            && child == self.pattern
            && self.can_block_split_pattern
            && self.guard.is_none()
        {
            return ShapeSet::INLINE_OR_BLOCK;
        }

        // Allow newlines only in the guard if we split before `when`.
        if state == BEFORE_WHEN && is_guard {
            return ShapeSet::ALL;
        }

        // If there's no guard, then we can split anywhere in the pattern when
        // splitting after `case`.
        if state == BEFORE_CASE && child == self.pattern && self.guard.is_none() {
            return ShapeSet::ALL;
        }

        // If there is a guard, then the entire pattern and guard must fit on
        // one line, but the value expression can split.
        if state == BEFORE_CASE && child == self.value {
            return ShapeSet::ALL;
        }

        // Once we split at both `case` and `when`, then splits are allowed
        // everywhere.
        if state == BEFORE_CASE_AND_WHEN {
            return ShapeSet::ALL;
        }

        ShapeSet::ONLY_INLINE
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        if state != State::UNSPLIT {
            writer.push_indent(Indent::Expression);
        }

        writer.format(self.value, false);

        // The case clause and pattern.
        writer.split_if(state == BEFORE_CASE || state == BEFORE_CASE_AND_WHEN);

        if !self.can_block_split_pattern {
            writer.push_collapsible_indent();
        }

        writer.format(self.pattern, false);

        if !self.can_block_split_pattern {
            writer.pop_indent();
        }

        // The guard clause.
        if let Some(guard) = self.guard {
            writer.split_if(state == BEFORE_WHEN || state == BEFORE_CASE_AND_WHEN);
            writer.format(guard, false);
        }

        if state != State::UNSPLIT {
            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.value);
        callback(self.pattern);
        if let Some(guard) = self.guard {
            callback(guard);
        }
    }
}
