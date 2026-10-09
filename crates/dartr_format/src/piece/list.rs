// Dart source: dart_style lib/src/piece/list.dart

//! `BlockFormat` of `list.dart` is in [crate::ast_extensions].

use rustc_hash::FxHashSet;

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};
use crate::constants::Cost;

use super::{PieceId, PieceImpl, PieceKind, Pieces, ShapeSet, State, States};

/// A piece for a non-empty splittable series of items.
///
/// Items may optionally be delimited with brackets and may have commas added
/// after elements.
///
/// Used for argument lists, collection literals, parameter lists, etc. This
/// class handles adding and removing the trailing comma depending on whether
/// the list is split or not. It handles comments inside the sequence of
/// elements.
///
/// These pieces can be formatted in one of three ways:
///
/// [State::UNSPLIT] Fully unsplit:
///
///     function(argument, argument, argument);
///
/// If one of the elements is a "block element", then we allow newlines
/// inside it to support output like:
///
///     function(argument, () {
///       blockElement;
///     }, argument);
///
/// [State::SPLIT] Split around all of the items:
///
///     function(
///       argument,
///       argument,
///       argument,
///     );
///
/// ListPieces are usually constructed using [DelimitedListBuilder].
pub struct ListPiece {
    /// The opening bracket before the elements, if any.
    before: Option<PieceId>,

    /// The list of elements.
    elements: Vec<PieceId>,

    /// The elements that should have a blank line preserved between them and
    /// the next piece.
    blanks_after: FxHashSet<PieceId>,

    /// The closing bracket after the elements, if any.
    after: Option<PieceId>,

    /// The details of how this particular list should be formatted.
    style: ListStyle,

    /// The index of the last element in [elements] that has content and
    /// isn't a comment, or `-1` if all elements are comments.
    last_non_comment_element: i32,

    /// Whether this list should have [Shape::Block] when it splits.
    ///
    /// This is true for most lists, but false for some lists where we don't
    /// want them to be treated as block-formatted in the surrounding context,
    /// mainly type argument lists.
    is_block_shaped: bool,
}

impl ListPiece {
    /// Creates a new [ListPiece] in [pieces] and returns its id.
    ///
    /// [elements] must not be empty. (If there are no elements, just
    /// concatenate the brackets directly.)
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        pieces: &mut Pieces,
        before: Option<PieceId>,
        elements: Vec<PieceId>,
        blanks_after: FxHashSet<PieceId>,
        after: Option<PieceId>,
        style: ListStyle,
        last_non_comment_element: i32,
        block_shaped: bool,
    ) -> PieceId {
        assert!(!elements.is_empty());

        // For most elements, we know whether or not it will have a comma
        // based only on the comma style and its position in the list, so pin
        // those here.
        for (i, &element) in elements.iter().enumerate() {
            let i = i as i32;
            match style.commas {
                Commas::AlwaysTrailing => {
                    // Has a comma after every element.
                    pieces.pin(element, APPEND_COMMA);
                }
                Commas::Trailing => {
                    // Always has a comma after every element except the last.
                    // The last will be constrained to have one or not
                    // depending on whether the list splits. See
                    // applyConstraints().
                    if i < last_non_comment_element {
                        pieces.pin(element, APPEND_COMMA);
                    }
                }
                Commas::NonTrailing => {
                    // Never a trailing comma after the last element.
                    pieces.pin(
                        element,
                        if i < last_non_comment_element {
                            APPEND_COMMA
                        } else {
                            State::UNSPLIT
                        },
                    );
                }
                Commas::None => {
                    // No comma after any element.
                    pieces.pin(element, State::UNSPLIT);
                }
            }
        }

        pieces.add(ListPiece {
            before,
            elements,
            blanks_after,
            after,
            style,
            last_non_comment_element,
            is_block_shaped: block_shaped,
        })
    }

    /// Whether any element in this argument list can be block formatted.
    pub fn has_block_element(&self, pieces: &Pieces) -> bool {
        self.elements
            .iter()
            .any(|&element| element_piece(pieces, element).allow_newlines_when_unsplit)
    }
}

fn element_piece(pieces: &Pieces, id: PieceId) -> &ListElementPiece {
    match pieces.kind(id) {
        PieceKind::ListElement(element) => element,
        _ => unreachable!("ListPiece elements are ListElementPieces"),
    }
}

impl PieceImpl for ListPiece {
    fn additional_states(&self) -> States {
        States::of(&[State::SPLIT])
    }

    fn apply_constraints(&self, state: State, constrain: &mut super::Constrain) {
        // Give the last element a trailing comma only if the list is split.
        if self.style.commas == Commas::Trailing && self.last_non_comment_element != -1 {
            constrain(
                self.elements[self.last_non_comment_element as usize],
                if state == State::SPLIT {
                    APPEND_COMMA
                } else {
                    State::UNSPLIT
                },
            );
        }
    }

    fn state_cost(&self, state: State) -> i32 {
        if state == State::SPLIT {
            return self.style.split_cost;
        }
        state.cost()
    }

    fn allowed_child_shapes(&self, pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        if state == State::SPLIT {
            return ShapeSet::ALL;
        }
        if Some(child) == self.before {
            return ShapeSet::ALL;
        }
        if Some(child) == self.after {
            return ShapeSet::ALL;
        }

        // Only some elements (usually a single block element) allow newlines
        // when the list itself isn't split.
        ShapeSet::any_if(match pieces.kind(child) {
            PieceKind::ListElement(element) => element.allow_newlines_when_unsplit,
            _ => false,
        })
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        let pieces = writer.pieces();

        // Format the opening bracket, if there is one.
        if let Some(before) = self.before {
            writer.format(before, false);

            if state != State::UNSPLIT {
                writer.push_indent(Indent::Block);
            }

            if self.is_block_shaped {
                writer.set_shape_mode(ShapeMode::Block);
            }

            // Whitespace after the opening bracket.
            writer.split_if_with(
                state == State::SPLIT,
                self.style.space_when_unsplit && !self.elements.is_empty(),
                false,
            );
        }

        // Format the elements.
        for (i, &element) in self.elements.iter().enumerate() {
            let element_piece = element_piece(pieces, element);

            // If this element allows newlines when the list isn't split, add
            // indentation if it requires it.
            if state == State::UNSPLIT && element_piece.indent_when_block_formatted {
                writer.push_indent(Indent::Expression);
            }

            // We can format each list item separately if the item is on its
            // own line. This happens when the list is split and there is
            // something before and after the item, either brackets or other
            // items.
            let separate = state == State::SPLIT
                && (i > 0 || self.before.is_some())
                && (i < self.elements.len() - 1 || self.after.is_some());
            writer.format(element, separate);

            if state == State::UNSPLIT && element_piece.indent_when_block_formatted {
                writer.pop_indent();
            }

            // Write a space or newline between elements.
            if i < self.elements.len() - 1 {
                writer.split_if_with(
                    state == State::SPLIT,
                    // No space after the "[" or "{" in a parameter list.
                    element_piece.delimiter.is_empty(),
                    self.blanks_after.contains(&element),
                );
            }
        }

        // Format the closing bracket, if any.
        if let Some(after) = self.after {
            if state == State::SPLIT {
                writer.pop_indent();
            }

            // Whitespace before the closing bracket.
            writer.split_if_with(
                state == State::SPLIT,
                self.style.space_when_unsplit && !self.elements.is_empty(),
                false,
            );

            if self.is_block_shaped {
                writer.set_shape_mode(ShapeMode::Merge);
            }

            writer.format(after, false);
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        if let Some(before) = self.before {
            callback(before);
        }

        for &element in &self.elements {
            callback(element);
        }

        if let Some(after) = self.after {
            callback(after);
        }
    }

    fn fixed_state_for_page_width(&self, pieces: &Pieces, page_width: i32) -> Option<State> {
        let mut surrounding_length = 0;
        if let Some(before) = self.before {
            // A newline in the opening bracket (like a line comment after the
            // bracket) forces the list to split.
            if pieces.contains_hard_newline(before) {
                return Some(State::SPLIT);
            }

            surrounding_length += pieces.total_characters(before);
        }

        if let Some(after) = self.after {
            surrounding_length += pieces.total_characters(after);

            // Note that a newline in `_after` does *not* force the list to
            // split, so we ignore it here. This is typically a line comment
            // after the closing bracket.
        }

        let mut current_line_length = surrounding_length;
        let mut first = true;
        for &element in &self.elements {
            // If the element can be block formatted, then it might contain a
            // newline that doesn't force the list to split. In that case,
            // elements after this one won't be on the same line as the one
            // whose length we have accumulated. Reset the length to start a
            // new line for the remaining elements.
            if element_piece(pieces, element).allow_newlines_when_unsplit {
                current_line_length = surrounding_length;
                continue;
            }

            if pieces.contains_hard_newline(element) {
                return Some(State::SPLIT);
            }

            current_line_length += pieces.total_characters(element);

            // The comma and space between elements.
            if !first {
                current_line_length += 2;
            }
            first = false;

            if current_line_length > page_width {
                return Some(State::SPLIT);
            }
        }

        None
    }
}

/// Dart `ListElementPiece._appendComma`.
pub const APPEND_COMMA: State = State::with_cost(1, 0);

/// An element in a [ListPiece].
///
/// Contains any leading inline comments, the element's code content, and
/// trailing comments.
///
/// Leading and trailing comments may be empty if there are no comments. The
/// content may be empty when the element piece represents a comment that is
/// on its own line and formatted like a standalone element. In that case,
/// [hanging_comments] will contain the comment.
///
/// This piece also handles writing the comma after the content (but before
/// any hanging comments) when appropriate. The split state of the
/// surrounding list often determines whether the last element's trailing
/// comma is shown. To handle that, this piece has two states:
/// [State::UNSPLIT] omits the comma and [APPEND_COMMA] writes it. The parent
/// [ListPiece] will pin or constrain its child elements appropriately to
/// control whether or not the comma is written.
pub struct ListElementPiece {
    /// The leading inline block comments before the content.
    leading_comments: Vec<PieceId>,

    content: Option<PieceId>,

    /// Whether newlines are allowed in this element when this list is
    /// unsplit.
    ///
    /// This is generally only true for a single "block" element, as in:
    ///
    ///     function(argument, [
    ///       block,
    ///       element,
    ///     ], another);
    pub allow_newlines_when_unsplit: bool,

    /// Whether we should increase indentation when formatting this element
    /// when the list isn't split.
    ///
    /// This only comes into play for unsplit lists and is only relevant when
    /// the element contains newlines, which means that this is only ever
    /// useful when [allow_newlines_when_unsplit] is also true.
    ///
    /// This is used for adjacent strings expression at the beginning of an
    /// argument list followed by a function expression, like in a `test()`
    /// call. Since the adjacent strings may not require indentation when the
    /// list is fully split, this ensures that they are indented properly when
    /// the list isn't split. Avoids:
    ///
    ///     test('long description'
    ///     'that should be indented', () {
    ///       body;
    ///     });
    pub indent_when_block_formatted: bool,

    /// If this piece has an opening delimiter after the comma, this is its
    /// lexeme, otherwise an empty string.
    ///
    /// This is only used for parameter lists when an optional or named
    /// parameter section begins in the middle of the parameter list, like:
    ///
    ///     function(
    ///       int parameter1, [
    ///       int parameter2,
    ///     ]);
    delimiter: String,

    /// The hanging inline block and line comments that appear after the
    /// content.
    hanging_comments: Vec<PieceId>,

    /// The number of hanging comments that should appear before the
    /// delimiter.
    ///
    /// A list item may have hanging comments before and after the delimiter,
    /// as in:
    ///
    ///     function(
    ///       argument /* 1 */ /* 2 */, /* 3 */ /* 4 */ // 5
    ///     );
    ///
    /// This field counts the number of comments that should be before the
    /// delimiter (here `,` and 2).
    comments_before_delimiter: usize,
}

impl ListElementPiece {
    pub fn new(leading_comments: Vec<PieceId>, element: PieceId) -> ListElementPiece {
        ListElementPiece {
            leading_comments,
            content: Some(element),
            allow_newlines_when_unsplit: false,
            indent_when_block_formatted: false,
            delimiter: String::new(),
            hanging_comments: Vec::new(),
            comments_before_delimiter: 0,
        }
    }

    pub fn comment(comment: PieceId) -> ListElementPiece {
        ListElementPiece {
            leading_comments: Vec::new(),
            content: None,
            allow_newlines_when_unsplit: false,
            indent_when_block_formatted: false,
            delimiter: String::new(),
            hanging_comments: vec![comment],
            comments_before_delimiter: 0,
        }
    }

    /// Whether this piece is a comment-only element, like the second element
    /// in:
    ///
    ///     [
    ///       real,
    ///       // Comment.
    ///       anotherReal,
    ///     ]
    pub fn is_comment(&self) -> bool {
        self.content.is_none()
    }

    pub fn add_comment(&mut self, comment: PieceId, before_delimiter: bool) {
        self.hanging_comments.push(comment);
        if before_delimiter {
            self.comments_before_delimiter += 1;
        }
    }

    pub fn set_delimiter(&mut self, delimiter: &str) {
        self.delimiter = delimiter.to_string();
    }
}

impl PieceImpl for ListElementPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        for &comment in &self.leading_comments {
            writer.format(comment, false);
            writer.space();
        }

        if let Some(content) = self.content {
            writer.format(content, false);

            for i in 0..self.comments_before_delimiter {
                writer.space();
                writer.format(self.hanging_comments[i], false);
            }

            if state == APPEND_COMMA {
                writer.write(",", true);
            }

            if !self.delimiter.is_empty() {
                writer.space();
                writer.write(&self.delimiter, false);
            }
        }

        for i in self.comments_before_delimiter..self.hanging_comments.len() {
            if i > 0 || self.content.is_some() {
                writer.space();
            }
            writer.format(self.hanging_comments[i], false);
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for &comment in &self.leading_comments {
            callback(comment);
        }
        if let Some(content) = self.content {
            callback(content);
        }
        for &comment in &self.hanging_comments {
            callback(comment);
        }
    }
}

/// Where commas should be added in a [ListPiece].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Commas {
    /// Add a comma after every element, regardless of whether or not it is
    /// split.
    AlwaysTrailing,

    /// Add a comma after every element when the elements split, including
    /// the last. When not split, omit the trailing comma.
    Trailing,

    /// Add a comma after every element except for the last, regardless of
    /// whether or not it is split.
    NonTrailing,

    /// Don't add commas after any elements.
    None,
}

/// The various ways a "list" can appear syntactically and be formatted.
///
/// [ListPiece] is used for most places in code where a series of elements
/// can be either all on one line or can be each split to their own line with
/// no extra indentation: argument lists, parameter lists, collection
/// literals, type arguments, switch expression cases, etc.
///
/// These have similar enough formatting to use the same class. And, in
/// particular, they all handle comments between elements the same way. But
/// they vary in whether or not a trailing comma is allowed, whether there
/// should be spaces inside the delimiters when the elements aren't split,
/// etc. This class captures those options.
#[derive(Clone, Copy, Debug)]
pub struct ListStyle {
    /// How commas should be handled by the list.
    ///
    /// Most lists use [Commas::Trailing]. Type parameters and type arguments
    /// use [Commas::NonTrailing]. For loop parts and switch values use
    /// [Commas::None].
    pub commas: Commas,

    /// The cost of splitting this list. Normally 1, but higher for some lists
    /// that look worse when split.
    pub split_cost: i32,

    /// Whether this list should have spaces inside the bracket when it
    /// doesn't split. This is false for most lists, but true for switch
    /// expression bodies:
    ///
    ///     v = switch (e) { 1 => 'one', 2 => 'two' };
    ///     //              ^                      ^
    pub space_when_unsplit: bool,
}

impl Default for ListStyle {
    fn default() -> ListStyle {
        ListStyle {
            commas: Commas::Trailing,
            split_cost: Cost::NORMAL,
            space_when_unsplit: false,
        }
    }
}

impl ListStyle {
    /// Dart `ListStyle(commas: commas)`.
    pub fn with_commas(commas: Commas) -> ListStyle {
        ListStyle {
            commas,
            ..ListStyle::default()
        }
    }
}
