// Dart source: dart_style lib/src/back_end/code_writer.dart

use std::rc::Rc;

use rustc_hash::FxHashSet;

use crate::piece::{PieceId, Pieces, Shape};
use crate::text::utf16_len;

use super::code::GroupCode;
use super::solution::Solution;
use super::solution_cache::SolutionCache;

/// The interface used by [Piece]s to output formatted code.
///
/// The back-end lowers the tree of pieces to the final formatted code by
/// allowing each piece to produce the output for the code it represents.
/// This way, each piece has full flexibility for how to apply its own
/// formatting logic.
///
/// To build the resulting output code, when a piece is formatted, it is
/// passed an instance of this class. It has methods that the piece can call
/// to add output text to the resulting code, recursively format child
/// pieces, insert whitespace, etc.
pub struct CodeWriter<'p, 'c> {
    page_width: i32,

    /// The pieces being formatted.
    pieces: &'p Pieces,

    /// Previously cached formatted subtrees.
    cache: &'c mut SolutionCache<'p>,

    /// The solution this [CodeWriter] is generating code for.
    solution: &'c mut Solution<'p>,

    /// The code being written.
    code: GroupCode<'p>,

    /// What whitespace should be written before the next non-whitespace
    /// text.
    ///
    /// When whitespace is written, instead of immediately writing it, we
    /// queue it as pending. This ensures that we don't write trailing
    /// whitespace, avoids writing spaces at the beginning of lines, and
    /// allows collapsing multiple redundant newlines.
    pending_whitespace: Whitespace,

    /// The number of spaces of indentation that should be begin the next
    /// line when [pending_whitespace] is [Whitespace::Newline] or
    /// [Whitespace::BlankLine].
    pending_indent: i32,

    /// The number of characters in the line currently being written.
    column: i32,

    /// The number of characters at the end of the current line that are
    /// "soft".
    ///
    /// See [FormattingStyle::use_soft_overflow] for more details.
    soft_characters: i32,

    /// The stack of indentation levels.
    ///
    /// Each entry in the stack is the absolute number of spaces of leading
    /// indentation that should be written when beginning a new line to
    /// account for block nesting, expression wrapping, constructor
    /// initializers, etc.
    indent_stack: Vec<IndentLevel>,

    /// The stack of information for each [Piece] currently being formatted.
    ///
    /// This allows [CodeWriter] to pass itself data from parent to child
    /// through [format()] calls and back up from child to parent without
    /// every override of [format()] having to thread that data through.
    piece_formats: Vec<FormatState>,

    /// Whether we have already found the first line where whose piece should
    /// be used to expand further solutions.
    ///
    /// This is the first line that either overflows or contains an invalid
    /// newline. When expanding solutions, we use the first solvable piece on
    /// this line.
    found_expand_line: bool,

    /// The solvable pieces on the first overflowing or invalid line, if we've
    /// found any.
    ///
    /// A piece is "solvable" if we haven't already bound it to a state and
    /// there are multiple states it accepts. This is the piece whose states
    /// will be bound when we expand the [Solution] that this [CodeWriter] is
    /// building into further solutions.
    ///
    /// If [found_expand_line] is `true`, then this contains the list of
    /// unsolved pieces that were being formatted when text was written to
    /// the first problematic line.
    expand_pieces: Vec<PieceId>,

    /// The stack of solvable pieces currently being formatted.
    ///
    /// We use this to track which pieces are in play when text is written to
    /// the current line so that we know which piece should be expanded in
    /// the next solution if the line ends up overflowing.
    current_unsolved_pieces: Vec<PieceId>,

    /// The set of unsolved pieces that were being formatted when text was
    /// written to the current line (Dart insertion-ordered `Set`): the
    /// pieces in insertion order and the set for membership.
    current_line_pieces: Vec<PieceId>,
    current_line_piece_set: FxHashSet<PieceId>,

    /// `cache.style.is3Dot7`.
    is_3_dot_7: bool,

    /// `cache.style.useSoftOverflow`.
    use_soft_overflow: bool,

    /// `cache.style.pinStateByPageWidthBeforeSolving`.
    pin_state_by_page_width_before_solving: bool,
}

impl<'p, 'c> CodeWriter<'p, 'c> {
    /// [leading_indent] is the number of spaces of leading indentation at the
    /// beginning of the first line and [subsequent_indent] is the
    /// indentation of each line after that, independent of indentation
    /// created by pieces being written.
    pub fn new(
        page_width: i32,
        leading_indent: i32,
        subsequent_indent: i32,
        pieces: &'p Pieces,
        cache: &'c mut SolutionCache<'p>,
        solution: &'c mut Solution<'p>,
    ) -> CodeWriter<'p, 'c> {
        let is_3_dot_7 = cache.style.is_3_dot_7();
        let use_soft_overflow = cache.style.use_soft_overflow();
        let pin_state_by_page_width_before_solving =
            cache.style.pin_state_by_page_width_before_solving();
        let mut indent_stack = vec![IndentLevel::new(Indent::None, leading_indent)];

        // If there is additional indentation on subsequent lines, then push
        // that onto the stack. When the first newline is written,
        // [pending_indent] will pick this up and use it for subsequent lines.
        if subsequent_indent > leading_indent {
            indent_stack.push(IndentLevel::new(Indent::None, subsequent_indent));
        }

        CodeWriter {
            page_width,
            pieces,
            cache,
            solution,
            code: GroupCode::new(leading_indent),
            pending_whitespace: Whitespace::None,
            // Track the leading indent before the first line.
            pending_indent: leading_indent,
            column: leading_indent,
            soft_characters: 0,
            indent_stack,
            piece_formats: Vec::new(),
            found_expand_line: false,
            expand_pieces: Vec::new(),
            current_unsolved_pieces: Vec::new(),
            current_line_pieces: Vec::new(),
            current_line_piece_set: FxHashSet::default(),
            is_3_dot_7,
            use_soft_overflow,
            pin_state_by_page_width_before_solving,
        }
    }

    /// The pieces being formatted.
    #[inline]
    pub fn pieces(&self) -> &'p Pieces {
        self.pieces
    }

    /// Returns the final formatted code and the next pieces that can be
    /// expanded from the solution this [CodeWriter] is writing, if any.
    pub fn finish(mut self) -> (GroupCode<'p>, Vec<PieceId>) {
        self.finish_line();

        (self.code, self.expand_pieces)
    }

    /// Appends [text] to the output.
    ///
    /// If [text] contains any internal newlines, the caller is responsible
    /// for also calling [handleNewline()].
    ///
    /// When possible, avoid calling this directly. Instead, any input code
    /// lexemes should be written to TextPieces which then call this. That
    /// way, selections inside lexemes are correctly updated.
    ///
    /// If [soft] is `true`, then [text] is considered to be "soft" code. See
    /// [FormattingStyle::use_soft_overflow] for more details.
    pub fn write(&mut self, text: &'p str, soft: bool) {
        self.flush_whitespace();
        self.code.write(text);
        let length = utf16_len(text) as i32;
        self.column += length;

        if soft {
            self.soft_characters += length;
        } else {
            // We only count trailing soft characters at the end of the line,
            // so writing any non-soft code resets the count.
            self.soft_characters = 0;
        }

        // If we haven't found an overflowing line yet, then this line might
        // be one so keep track of the unsolved pieces we've encountered on
        // it.
        if !self.found_expand_line {
            for &piece in &self.current_unsolved_pieces {
                if self.current_line_piece_set.insert(piece) {
                    self.current_line_pieces.push(piece);
                }
            }
        }
    }

    /// Increases the indentation by [indent] relative to the current amount
    /// of indentation.
    pub fn push_indent(&mut self, indent: Indent) {
        if self.is_3_dot_7 {
            self.push_indent_3_dot_7(indent, false);
        } else {
            let parent = *self.indent_stack.last().unwrap();

            // Combine the new indentation with the surrounding one.
            let offset = match (parent.ty, indent) {
                // On the right-hand side of `=`, `:`, or `=>`, don't indent
                // subsequent infix operands so that they all align:
                //
                //     variable =
                //         operand +
                //         another;
                (Indent::Assignment, Indent::Infix) => 0,

                // We have already indented the control flow header, so
                // collapse the duplicate indentation.
                (Indent::ControlFlowClause, Indent::Expression) => 0,
                (Indent::ControlFlowClause, Indent::Infix) => 0,

                // If we get here, the parent context has no effect, so just
                // apply the indentation directly.
                (_, _) => indent.spaces(),
            };

            self.indent_stack
                .push(IndentLevel::new(indent, parent.spaces + offset));
        }
    }

    /// Increases the indentation in a control flow clause in a "collapsible"
    /// way.
    ///
    /// This is only used in a couple of corners of if-case and for-in
    /// headers where the indentation is unusual.
    pub fn push_collapsible_indent(&mut self) {
        if self.is_3_dot_7 {
            self.push_indent_3_dot_7(Indent::Expression, true);
        } else {
            self.push_indent(Indent::ControlFlowClause);
        }
    }

    /// The 3.7 style of indentation and collapsible indentation tracking.
    ///
    /// Splits in if-case and for-in loop headers are tricky to indent
    /// gracefully. For example, if an infix expression inside the case
    /// splits, we don't want it to be double indented:
    ///
    ///     if (object
    ///         case veryLongConstant
    ///                 as VeryLongType) {
    ///       ;
    ///     }
    ///
    /// That suggests that the [IfCasePiece] shouldn't add indentation for the
    /// case pattern since the [InfixPiece] inside it will already indent the
    /// RHS.
    ///
    /// But if the case is a variable pattern that splits, the
    /// [VariablePiece] does *not* add indentation because in most other
    /// places where it occurs, that's what we want. If the [IfCasePiece]
    /// doesn't indent the pattern, you get:
    ///
    ///     if (object
    ///         case VeryLongType
    ///         veryLongVariable
    ///         ) {
    ///       ;
    ///     }
    ///
    /// To deal with this, 3.7 had a notion of "collapsible" indentation. In
    /// 3.8 and later, there is a different mechanism for merging indentation
    /// kinds. This function implements the former.
    fn push_indent_3_dot_7(&mut self, indent: Indent, can_collapse: bool) {
        let parent_indent = self.indent_stack.last().unwrap().spaces;
        let parent_collapse = self.indent_stack.last().unwrap().collapsible;

        if parent_collapse == indent.spaces() {
            // We're indenting by the same existing collapsible amount, so
            // collapse this new indentation with that existing one.
            self.indent_stack
                .push(IndentLevel::v_3_dot_7(parent_indent, 0));
        } else if can_collapse {
            // We should never get multiple levels of nested collapsible
            // indentation.
            debug_assert!(parent_collapse == 0);

            // Increase the indentation and note that it can be collapsed with
            // further indentation.
            self.indent_stack.push(IndentLevel::v_3_dot_7(
                parent_indent + indent.spaces(),
                indent.spaces(),
            ));
        } else {
            // Regular indentation, so just increase the indent.
            self.indent_stack
                .push(IndentLevel::v_3_dot_7(parent_indent + indent.spaces(), 0));
        }
    }

    /// Discards the indentation change from the last call to
    /// [push_indent()].
    pub fn pop_indent(&mut self) {
        self.indent_stack.pop();
    }

    /// Inserts a newline if [condition] is true, otherwise writes a space.
    #[inline]
    pub fn split_if(&mut self, condition: bool) {
        self.split_if_with(condition, true, false);
    }

    /// Inserts a newline if [condition] is true.
    ///
    /// If [space] is `true` and [condition] is `false`, writes a space.
    ///
    /// If [blank] is `true`, writes an extra newline to produce a blank line.
    pub fn split_if_with(&mut self, condition: bool, space: bool, blank: bool) {
        if condition {
            self.newline_with(blank, false);
        } else if space {
            self.space();
        }
    }

    /// Writes a single space to the output.
    #[inline]
    pub fn space(&mut self) {
        self.whitespace(Whitespace::Space, false);
    }

    /// Inserts a line split in the output.
    #[inline]
    pub fn newline(&mut self) {
        self.newline_with(false, false);
    }

    /// Inserts a line split in the output.
    ///
    /// If [blank] is `true`, writes an extra newline to produce a blank line.
    ///
    /// If [flush_left] is `true`, then the new line begins at column 1 and
    /// ignores any surrounding indentation. This is used for multi-line
    /// block comments and multi-line strings.
    pub fn newline_with(&mut self, blank: bool, flush_left: bool) {
        self.whitespace(
            if blank {
                Whitespace::BlankLine
            } else {
                Whitespace::Newline
            },
            flush_left,
        );
    }

    /// Queues [whitespace] to be written to the output.
    ///
    /// If any non-whitespace is written after this call, then this whitespace
    /// will be written first. Also handles merging multiple kinds of
    /// whitespace intelligently together.
    ///
    /// If [flush_left] is `true`, then the new line begins at column 1 and
    /// ignores any surrounding indentation. This is used for multi-line
    /// block comments and multi-line strings.
    pub fn whitespace(&mut self, whitespace: Whitespace, flush_left: bool) {
        if matches!(whitespace, Whitespace::Newline | Whitespace::BlankLine) {
            let last = self.piece_formats.len() - 1;
            Self::apply_newline_to_shape(&mut self.piece_formats[last], Shape::Other);
            self.pending_indent = if flush_left {
                0
            } else {
                self.indent_stack.last().unwrap().spaces
            };
        }

        self.pending_whitespace = self.pending_whitespace.collapse(whitespace);
    }

    /// When a newline is written by the current piece of one of its
    /// children, determines how that affects the current piece's shape.
    pub fn set_shape_mode(&mut self, mode: ShapeMode) {
        self.piece_formats.last_mut().unwrap().mode = mode;
    }

    /// Format [piece] and insert the result into the code being written and
    /// returned by [finish()].
    ///
    /// If [separate] is `true`, then [piece] is formatted and solved using a
    /// separate Solver and the result inserted into this CodeWriter's
    /// Solution. This lets us solve branches of the piece tree separately
    /// and compose the optimal results together.
    ///
    /// It's only safe to pass [separate] when the piece's formatting depends
    /// only on its starting indentation and state. If the piece's formatting
    /// can be affected by the contents of the current line, the contents
    /// after the piece's ending line, or constraints between pieces, then
    /// [separate] should be `false`. It's up to the parent piece to only
    /// call this when it's safe to do so. In practice, this usually means
    /// when the parent piece knows that [piece] will have a newline before
    /// and after it.
    pub fn format(&mut self, piece: PieceId, separate: bool) {
        if separate {
            self.format_separate(piece);
        } else {
            self.format_inline(piece);
        }
    }

    /// Format [piece] using a separate [Solver] and merge the result into
    /// this writer's [solution].
    fn format_separate(&mut self, piece: PieceId) {
        let state_if_bound = self.solution.piece_state_if_bound(self.pieces, piece);
        let subsequent_indent = self.indent_stack.last().unwrap().spaces;
        let solution = self.cache.find(
            self.pieces,
            piece,
            state_if_bound,
            self.page_width,
            self.pending_indent,
            subsequent_indent,
        );

        self.pending_indent = 0;
        self.flush_whitespace();

        self.solution
            .merge_subtree(solution.overflow, solution.cost);
        self.code.group(Rc::clone(&solution.code));
    }

    /// Format [piece] writing directly into this [CodeWriter].
    fn format_inline(&mut self, piece: PieceId) {
        let pieces = self.pieces;
        let mut is_unsolved =
            !self.solution.is_bound(pieces, piece) && pieces.has_additional_states(piece);

        // See if we can immediately bind it based on the page width and the
        // piece's contents.
        if is_unsolved && !self.pin_state_by_page_width_before_solving {
            // If the solution doesn't bind the piece already, we may be able
            // to eagerly bind it to a state knowing just the page width
            // (minus any leading indentation). If so, do that now. We do that
            // here instead of pinning the pieces because doing so here lets
            // us take leading indication into account which may vary based
            // on the surrounding pieces when we get here.
            is_unsolved = !self.solution.try_bind_by_page_width(
                pieces,
                piece,
                self.page_width - self.indent_stack[0].spaces,
            );
        }

        if is_unsolved {
            self.current_unsolved_pieces.push(piece);
        }

        // Begin a new formatting context for this child.
        self.piece_formats.push(FormatState::new(piece));

        // Format the child piece.
        let state = self.solution.piece_state(pieces, piece);
        pieces.get(piece).kind.as_impl().format(self, state);

        let child = self.piece_formats.pop().unwrap();

        // Restore the surrounding piece's context.
        if is_unsolved {
            self.current_unsolved_pieces.pop();
        }

        // Now that we know the child's shape, see if the parent permits it.
        if let Some(parent) = self.piece_formats.last_mut() {
            let parent_state = self.solution.piece_state(pieces, parent.piece);
            let allowed_shapes =
                pieces.allowed_child_shapes(parent.piece, parent_state, child.piece);

            let invalid = if self.is_3_dot_7 {
                // If the child must be inline, then invalidate because we know
                // it contains some kind of newline.
                // TODO(rnystrom): It would be better if this logic wasn't
                // different for 3.7. The only place where the distinction
                // between this code and the logic in the else clause comes
                // into play is with CaseExpressionPiece.
                child.shape != Shape::Inline && allowed_shapes.is_only_inline()
            } else {
                !allowed_shapes.contains(child.shape)
            };

            if invalid {
                self.solution.invalidate(pieces, parent.piece);
            }

            // If the child had newlines, propagate that to the parent's
            // shape.
            if child.shape != Shape::Inline {
                Self::apply_newline_to_shape(parent, child.shape);
            }
        }
    }

    /// Sets [selectionStart] to be [start] code units into the output.
    pub fn start_selection(&mut self, start: usize) {
        self.flush_whitespace();
        self.code.start_selection(start);
    }

    /// Sets [selectionEnd] to be [end] code units into the output.
    pub fn end_selection(&mut self, end: usize) {
        self.flush_whitespace();
        self.code.end_selection(end);
    }

    /// Disables or re-enables formatting in a region of code.
    pub fn set_formatting_enabled(&mut self, enabled: bool, source_offset: usize) {
        self.code.set_formatting_enabled(enabled, source_offset);
    }

    /// Write any pending whitespace.
    ///
    /// This is called before non-whitespace text is about to be written, or
    /// before the selection is updated since the latter requires an accurate
    /// count of the written text, including whitespace.
    fn flush_whitespace(&mut self) {
        match self.pending_whitespace {
            Whitespace::None => {} // Nothing to do.

            Whitespace::Newline | Whitespace::BlankLine => {
                self.finish_line();
                self.column = self.pending_indent;
                self.soft_characters = 0;
                self.code.newline(
                    self.pending_whitespace == Whitespace::BlankLine,
                    self.column,
                );
            }

            Whitespace::Space => {
                self.code.write(" ");
                self.column += 1;
                self.soft_characters += 1;
            }
        }

        self.pending_whitespace = Whitespace::None;
    }

    fn finish_line(&mut self) {
        // If the completed line is too long, track the overflow.
        if self.column > self.page_width {
            let mut overflow = self.column - self.page_width;

            // If soft overflow is enabled, then collapse any trailing soft
            // characters to a single point of overflow.
            if self.use_soft_overflow && self.soft_characters > 0 {
                if self.soft_characters >= overflow {
                    // All of the overflowing characters are soft.
                    overflow = 1;
                } else {
                    // The overflow contains both hard and soft overflow. Count
                    // each character of hard overflow and collapse the soft
                    // overflow.
                    let hard_overflow = overflow - self.soft_characters;
                    overflow = hard_overflow + 1;
                }
            }

            self.solution.add_overflow(overflow);
        }

        // If we found a problematic line, and there is are pieces on the line
        // that we can try to split, then remember them so that the solution
        // will expand them next.
        if self.found_expand_line {
            return;
        }
        if !self.current_line_pieces.is_empty()
            && (self.column > self.page_width || !self.solution.is_valid())
        {
            self.expand_pieces
                .extend_from_slice(&self.current_line_pieces);
            self.found_expand_line = true;
        } else {
            // This line was OK, so we don't need to expand the pieces on it.
            self.current_line_pieces.clear();
            self.current_line_piece_set.clear();
        }
    }

    /// Determine how a newline affects the current piece's shape.
    fn apply_newline_to_shape(state: &mut FormatState, shape: Shape) {
        state.shape = match state.mode {
            ShapeMode::Merge => state.shape.merge(shape),
            ShapeMode::Block => Shape::Block,
            ShapeMode::BeforeHeadline => Shape::Other,
            // If there were no newlines inside the headline, now that there
            // is one, we have a headline shape.
            ShapeMode::AfterHeadline if state.shape == Shape::Inline => Shape::Headline,
            // If there was already a newline in the headline, preserve that
            // shape.
            ShapeMode::AfterHeadline => state.shape,
            ShapeMode::Other => Shape::Other,
        };
    }
}

/// Different kinds of pending whitespace that have been requested.
///
/// Note that the order of values in the enum is significant: later ones have
/// more whitespace than previous ones.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Whitespace {
    /// No pending whitespace.
    None,

    /// A single space.
    Space,

    /// A single newline.
    Newline,

    /// Two newlines.
    BlankLine,
}

impl Whitespace {
    /// Combines two pending whitespaces and returns the result.
    ///
    /// When two whitespaces overlap, they aren't both written: we don't want
    /// two spaces or a newline followed by a space. Instead, the two
    /// whitespaces are collapsed such that the largest one wins.
    #[inline]
    pub fn collapse(self, other: Whitespace) -> Whitespace {
        self.max(other)
    }

    /// Whether this whitespace contains at least one newline.
    pub fn has_newline(self) -> bool {
        matches!(self, Whitespace::Newline | Whitespace::BlankLine)
    }
}

/// A kind of indentation that a [Piece] may output to control the leading
/// whitespace at the beginning of a line.
///
/// Each indentation type defines the number of spaces it writes. Indentation
/// is also semantic: a type describes *why* it writes that, or what kind of
/// syntax its coming from. This allows us to merge or combine indentation in
/// smarter ways in some contexts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Indent {
    // No indentation.
    None,

    /// The right-hand side of an `=`, `:`, or `=>`.
    Assignment,

    /// The contents of a block-like structure: block, collection literal,
    /// argument list, etc.
    Block,

    /// A split cascade chain.
    Cascade,

    /// Indentation when splits occur inside for-in and if-case clause
    /// headers.
    ControlFlowClause,

    /// Any general sort of split expression.
    Expression,

    /// "Indentation" for parenthesized expressions and other contexts where
    /// we want to prevent some inner expression's indentation from merging
    /// with the surrounding one.
    Grouping,

    /// An infix operator expression: `+`, `*`, `is`, etc.
    Infix,

    /// Constructor initializer when the parameter list doesn't have optional
    /// or named parameters.
    Initializer,

    /// Constructor initializer when the parameter list does have optional or
    /// named parameters.
    InitializerWithOptionalParameter,
}

impl Indent {
    /// The number of spaces this type of indentation applies.
    pub fn spaces(self) -> i32 {
        match self {
            Indent::None => 0,
            Indent::Assignment => 4,
            Indent::Block => 2,
            Indent::Cascade => 2,
            Indent::ControlFlowClause => 4,
            Indent::Expression => 4,
            Indent::Grouping => 0,
            Indent::Infix => 4,
            Indent::Initializer => 2,
            Indent::InitializerWithOptionalParameter => 3,
        }
    }
}

/// Information for each piece currently being formatted while [CodeWriter]
/// traverses the piece tree.
struct FormatState {
    /// The piece being formatted.
    piece: PieceId,

    /// The piece's shape.
    ///
    /// This changes based on the newlines the piece writes.
    shape: Shape,

    /// How a newline affects the shape of this piece.
    mode: ShapeMode,
}

impl FormatState {
    fn new(piece: PieceId) -> FormatState {
        FormatState {
            piece,
            shape: Shape::Inline,
            mode: ShapeMode::Merge,
        }
    }
}

/// Determines how a newline inside a piece or a child piece affects the
/// shape of the current piece.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeMode {
    /// The piece's shape is merged with the incoming shape.
    ///
    /// If a newline is written directly by the piece itself, that has shape
    /// [Shape::Other].
    Merge,

    /// A newline makes this piece block-shaped.
    Block,

    /// We are in the first line of a potentially headline-shaped piece.
    ///
    /// A newline here means it's not headline shaped.
    BeforeHeadline,

    /// We've already written the headline part of a piece so a newline after
    /// this is fine and still leaves it headline shaped.
    AfterHeadline,

    /// A newline makes this piece have [Shape::Other].
    Other,
}

/// A level of indentation in the indentation stack.
#[derive(Clone, Copy, Debug)]
struct IndentLevel {
    /// The reason this indentation was added.
    ///
    /// Not used for 3.7 style.
    ty: Indent,

    /// The total number of spaces of indentation.
    spaces: i32,

    /// How many spaces of [spaces] can be collapsed with further
    /// indentation.
    ///
    /// Only used for 3.7 style.
    collapsible: i32,
}

impl IndentLevel {
    fn v_3_dot_7(spaces: i32, collapsible: i32) -> IndentLevel {
        IndentLevel {
            ty: Indent::None,
            spaces,
            collapsible,
        }
    }

    fn new(ty: Indent, spaces: i32) -> IndentLevel {
        IndentLevel {
            ty,
            spaces,
            collapsible: 0,
        }
    }
}
