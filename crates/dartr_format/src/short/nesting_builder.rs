// Dart source: dart_style lib/src/short/nesting_builder.dart

use crate::constants::Indent;

use super::arena::{Arena, NestingId};

/// Keeps track of block indentation and expression nesting while the source
/// code is being visited and the chunks are being built.
///
/// This class requires (and verifies) that indentation and nesting are
/// stratified from each other. Expression nesting is always inside block
/// indentation, which means it is an error to try to change the block
/// indentation while any expression nesting is in effect.
pub struct NestingBuilder {
    /// The block indentation levels.
    ///
    /// This is tracked as a stack of numbers, each of which is the total number
    /// of spaces of block indentation. We only store the stack of previous
    /// levels as a convenience to the caller: it spares you from having to pass
    /// the unindent amount to [unindent()].
    stack: Vec<i32>,

    /// When not `None`, the expression nesting after the next token is written.
    ///
    /// When the nesting is increased, we don't want it to take effect until
    /// after at least one token has been written. That ensures that comments
    /// appearing before the first token are correctly indented. For example, a
    /// binary operator expression increases the nesting before the first operand
    /// to ensure any splits within the left operand are handled correctly. If we
    /// changed the nesting level immediately, then code like:
    ///
    ///     {
    ///       // comment
    ///       foo + bar;
    ///     }
    ///
    /// would incorrectly get indented because the line comment adds a split which
    /// would take the nesting level of the binary operator into account even
    /// though we haven't written any of its tokens yet.
    ///
    /// Likewise, when nesting is decreased, we may want to defer that until
    /// we've written the next token to handle uncommon cases like:
    ///
    ///     do // comment
    ///         {
    ///       ...
    ///     }
    ///
    /// Here, if we discard the expression nesting before we reach the "{", then
    /// it won't get indented as it should.
    pending_nesting: Option<NestingId>,

    /// The current nesting, ignoring any pending nesting.
    nesting: NestingId,
}

impl NestingBuilder {
    pub fn new(arena: &mut Arena) -> NestingBuilder {
        NestingBuilder {
            stack: vec![0],
            pending_nesting: None,
            nesting: arena.new_nesting_level(),
        }
    }

    /// The current number of characters of block indentation.
    #[inline]
    pub fn indentation(&self) -> i32 {
        *self.stack.last().unwrap()
    }

    /// The current nesting, ignoring any pending nesting.
    #[inline]
    pub fn nesting(&self) -> NestingId {
        self.nesting
    }

    /// The current nesting, including any pending nesting.
    #[inline]
    pub fn current_nesting(&self) -> NestingId {
        self.pending_nesting.unwrap_or(self.nesting)
    }

    /// Creates a new indentation level [spaces] deeper than the current one.
    ///
    /// If omitted, [spaces] defaults to [Indent.block].
    pub fn indent(&mut self, spaces: Option<i32>) {
        let spaces = spaces.unwrap_or(Indent::BLOCK);
        self.stack.push(self.indentation() + spaces);
    }

    /// Discards the most recent indentation level.
    pub fn unindent(&mut self) {
        self.stack.pop();
    }

    /// Begins a new expression nesting level [indent] deeper than the current
    /// one if it splits.
    ///
    /// Expressions that are more nested will get increased indentation when split
    /// if the previous line has a lower level of nesting.
    ///
    /// If [indent] is omitted, defaults to [Indent.expression].
    pub fn nest(&mut self, arena: &mut Arena, indent: Option<i32>) {
        let indent = indent.unwrap_or(Indent::EXPRESSION);
        let base = self.pending_nesting.unwrap_or(self.nesting);
        self.pending_nesting = Some(arena.nest(base, indent));
    }

    /// Discards the most recent level of expression nesting.
    pub fn unnest(&mut self, arena: &Arena) {
        let base = self.pending_nesting.unwrap_or(self.nesting);
        // If this fails, an unnest() call did not have a preceding nest() call.
        self.pending_nesting = arena.nesting(base).parent;
        debug_assert!(self.pending_nesting.is_some());
    }

    /// Applies any pending nesting now that we are ready for it to take effect.
    #[inline]
    pub fn commit_nesting(&mut self) {
        if let Some(pending) = self.pending_nesting.take() {
            self.nesting = pending;
        }
    }
}
