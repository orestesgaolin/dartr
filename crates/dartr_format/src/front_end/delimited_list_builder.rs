// Dart source: dart_style lib/src/front_end/delimited_list_builder.dart

use rustc_hash::FxHashSet;

use dartr_ast::{NamedArgument, NodeId};
use dartr_syntax::TokenId;

use crate::ast_extensions::{BlockFormat, first_non_comment_token, node_block_format_type};
use crate::comment_type::CommentType;
use crate::piece::{ListElementPiece, ListPiece, ListStyle, PieceId, PieceKind, Pieces, State};

use super::ast_node_visitor::AstNodeVisitor;
use super::comment_writer::CommentSequence;

/// Incrementally builds a [ListPiece], handling commas, comments, and
/// newlines that may appear before, between, or after its contents.
///
/// Users of this should call [left_bracket()] first, passing in the opening
/// delimiter token. Then call [add()] for each [AstNode] that is inside the
/// delimiters. The [right_bracket()] with the closing delimiter and finally
/// [build()] to get the resulting [ListPiece].
pub struct DelimitedListBuilder<'a> {
    /// The opening bracket before the elements, if any.
    left_bracket: Option<PieceId>,

    /// The list of elements in the list.
    elements: Vec<PieceId>,

    /// The element that should have a blank line preserved between them and
    /// the next piece.
    blanks_after: FxHashSet<PieceId>,

    /// The closing bracket after the elements, if any.
    right_bracket: Option<PieceId>,

    must_split: bool,

    style: ListStyle,

    /// The comments that should appear before the next element.
    leading_comments: Vec<PieceId>,

    /// The list of comments following the most recently written element
    /// before any comma following the element.
    comments_before_comma: CommentSequence<'a>,
}

fn list_element(arena: &mut Pieces, id: PieceId) -> &mut ListElementPiece {
    match arena.kind_mut(id) {
        PieceKind::ListElement(element) => element,
        _ => unreachable!("not a list element"),
    }
}

impl<'a> DelimitedListBuilder<'a> {
    /// Creates a new [DelimitedListBuilder] for an argument list, collection
    /// literal, etc.
    pub fn new(style: ListStyle) -> DelimitedListBuilder<'a> {
        DelimitedListBuilder {
            left_bracket: None,
            elements: Vec::new(),
            blanks_after: FxHashSet::default(),
            right_bracket: None,
            must_split: false,
            style,
            leading_comments: Vec::new(),
            comments_before_comma: CommentSequence::empty(),
        }
    }

    /// Creates the final [ListPiece] out of the added brackets, delimiters,
    /// elements, and style.
    #[inline]
    pub fn build(&mut self, v: &mut AstNodeVisitor<'a>) -> PieceId {
        self.build_with(v, false, true)
    }

    /// Creates the final [ListPiece] out of the added brackets, delimiters,
    /// elements, and style.
    pub fn build_with(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        force_split: bool,
        block_shaped: bool,
    ) -> PieceId {
        // To simplify the piece tree, if there are no elements, just return
        // the brackets concatenated together. We don't have to worry about
        // comments here since they would be in the [elements] list if there
        // were any.
        if self.elements.is_empty() {
            let left = self.left_bracket;
            let right = self.right_bracket;
            return v.build(|v| {
                if let Some(bracket) = left {
                    v.add(bracket);
                }
                if let Some(bracket) = right {
                    v.add(bracket);
                }
            });
        }

        // Find the index of the last non-comment element. The list may have
        // trailing elements that are just comments. In that case, those are
        // not considered the last element when it comes to deciding whether
        // to add a trailing comma on the last element.
        let mut last_non_comment_element = -1;
        for i in (0..self.elements.len()).rev() {
            if !list_element(&mut v.arena, self.elements[i]).is_comment() {
                last_non_comment_element = i as i32;
                break;
            }
        }

        let piece = ListPiece::create(
            &mut v.arena,
            self.left_bracket,
            std::mem::take(&mut self.elements),
            std::mem::take(&mut self.blanks_after),
            self.right_bracket,
            self.style,
            last_non_comment_element,
            block_shaped,
        );
        if self.must_split || force_split {
            v.arena.pin(piece, State::SPLIT);
        }
        piece
    }

    /// Adds the opening [bracket] to the built list.
    pub fn left_bracket(&mut self, v: &mut AstNodeVisitor<'a>, bracket: TokenId) {
        let piece = v.token_piece(bracket);
        self.add_left_bracket(piece);
    }

    /// Adds the opening bracket [piece] to the built list.
    pub fn add_left_bracket(&mut self, piece: PieceId) {
        self.left_bracket = Some(piece);
    }

    /// Adds the closing [bracket] to the built list along with any comments
    /// that precede it.
    ///
    /// If [delimiter] is given, it is a second bracket occurring immediately
    /// after [bracket]. This is used for parameter lists with optional or
    /// named parameters, like:
    ///
    ///     function(mandatory, {named});
    ///
    /// Here, [bracket] will be `)` and [delimiter] will be `}`.
    ///
    /// If [semicolon] is given, it is the optional `;` in an enum declaration
    /// after the enum constants when there are no subsequent members.
    /// Comments before the `;` are kept, but the `;` itself is discarded.
    pub fn right_bracket(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        bracket: TokenId,
        delimiter: Option<TokenId>,
        semicolon: Option<TokenId>,
    ) {
        // Handle comments after the last element.
        let mut comments_before = v.comments.take_comments_before(bracket);

        // Merge the comments before the delimiter (if there is one) and the
        // bracket. If there is a delimiter, this will move comments between
        // it and the bracket to before the delimiter, as in:
        //
        //     // Before:
        //     f([parameter] /* comment */) {}
        //
        //     // After:
        //     f([parameter /* comment */]) {}
        if let Some(delimiter) = delimiter {
            comments_before = v
                .comments
                .take_comments_before(delimiter)
                .concatenate(comments_before);
        }

        if let Some(semicolon) = semicolon {
            comments_before = v
                .comments
                .take_comments_before(semicolon)
                .concatenate(comments_before);
        }

        self.add_comments(v, comments_before, false);

        self.right_bracket = Some(v.build(|v| {
            v.token(delimiter);
            v.token(bracket);
        }));
    }

    /// Adds [piece] to the built list.
    ///
    /// Use this when the piece is composed of more than one [AstNode] or
    /// [Token] and [visit()] can't be used. When calling this, make sure to
    /// call [add_comments_before()] for the first token in the [piece].
    ///
    /// Assumes there is no comma after this piece.
    pub fn add(&mut self, v: &mut AstNodeVisitor<'a>, piece: PieceId) {
        let leading = std::mem::take(&mut self.leading_comments);
        let element = v.arena.add(ListElementPiece::new(leading, piece));
        self.elements.push(element);
        self.comments_before_comma = CommentSequence::empty();
    }

    /// Adds the contents of [inner] to this outer [DelimitedListBuilder].
    ///
    /// This is used when a [DelimiterListBuilder] is building a piece that
    /// will then become an element in a surrounding [DelimitedListBuilder].
    /// It ensures that any comments around a trailing comma after [inner]
    /// don't get lost and are instead hoisted up to be captured by this
    /// builder.
    pub fn add_inner_builder(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        mut inner: DelimitedListBuilder<'a>,
        force_split: bool,
    ) {
        // Add the elements of the line to this builder.
        let piece = inner.build_with(v, force_split, true);
        self.add(v, piece);

        // Make sure that any trailing comments on the line aren't lost.
        self.comments_before_comma = inner.comments_before_comma;
    }

    /// Writes any comments appearing before [token] to the list.
    pub fn add_comments_before(&mut self, v: &mut AstNodeVisitor<'a>, token: TokenId) {
        // Handle comments between the preceding element and this one.
        let comments_before_element = v.comments.take_comments_before(token);
        self.add_comments(v, comments_before_element, true);
    }

    /// Adds [element] to the built list.
    pub fn visit(&mut self, v: &mut AstNodeVisitor<'a>, element: NodeId) {
        let ast = v.ast;
        // Handle comments between the preceding element and this one.
        self.add_comments_before(v, first_non_comment_token(ast, element));

        // Traverse the element itself.
        let piece = v.node_piece(element);
        self.add(v, piece);

        let next_token = ast.tokens.next(ast.end_token(element));
        if ast.tokens.lexeme(next_token) == "," {
            self.comments_before_comma = v.comments.take_comments_before(next_token);
        }
    }

    /// Visits a list of [elements].
    ///
    /// If [allow_block_argument] is `true`, then allows one element to
    /// receive block formatting if appropriate, as in:
    ///
    ///     function(argument, [
    ///       block,
    ///       like,
    ///     ], argument);
    pub fn visit_all(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        elements: &[NodeId],
        allow_block_argument: bool,
    ) {
        for &element in elements {
            self.visit(v, element);
        }

        if allow_block_argument {
            self.set_block_argument(v, elements);
        }
    }

    /// Inserts an inner left delimiter between two elements.
    ///
    /// This is used for parameter lists when there are both mandatory and
    /// optional or named parameters to insert the `[` or `{`, respectively.
    ///
    /// This should not be used if [delimiter] appears before all elements.
    /// In that case, pass it to [left_bracket].
    pub fn left_delimiter(&mut self, v: &mut AstNodeVisitor<'a>, delimiter: TokenId) {
        assert!(!self.elements.is_empty());

        // Preserve any comments before the delimiter. Treat them as occurring
        // before the previous element's comma. This means that:
        //
        //     function(p1, /* comment */ [p1]);
        //
        // Will be formatted as:
        //
        //     function(p1 /* comment */, [p1]);
        //
        // (In practice, it's such an unusual place for a comment that it
        // doesn't matter that much where it goes and this seems to be simple
        // and reasonable looking.)
        let comments = v.comments.take_comments_before(delimiter);
        let before = std::mem::replace(&mut self.comments_before_comma, CommentSequence::empty());
        self.comments_before_comma = before.concatenate(comments);

        // Attach the delimiter to the previous element.
        let lexeme = v.ast.tokens.lexeme(delimiter);
        list_element(&mut v.arena, *self.elements.last().unwrap()).set_delimiter(lexeme);
    }

    /// Adds [comments] to the list.
    ///
    /// If [has_element_after] is `true` then another element will be written
    /// after these comments. Otherwise, we are at the comments after the last
    /// element before the closing delimiter.
    fn add_comments(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        comments: CommentSequence<'a>,
        has_element_after: bool,
    ) {
        // Early out if there's nothing to do.
        if self.comments_before_comma.is_empty()
            && comments.is_empty()
            && comments.lines_before_next_token() <= 1
        {
            return;
        }

        if self.comments_before_comma.requires_newline() || comments.requires_newline() {
            self.must_split = true;
        }

        let lines_before_next_token = comments.lines_before_next_token();

        // Figure out which comments are anchored to the preceding element,
        // which are freestanding, and which are attached to the next element.
        let (inline_comments, hanging_comments, separate_comments, leading_comments) =
            self.split_comma_comments(comments, has_element_after);

        // Add any hanging inline block comments to the previous element
        // before the subsequent ",".
        for comment in inline_comments.iter() {
            let comment_piece =
                v.comment_piece(comment, crate::back_end::code_writer::Whitespace::None);
            list_element(&mut v.arena, *self.elements.last().unwrap())
                .add_comment(comment_piece, true);
        }

        // Add any remaining hanging line comments to the previous element
        // after the ",".
        if !hanging_comments.is_empty() {
            for comment in hanging_comments.iter() {
                let comment_piece =
                    v.comment_piece(comment, crate::back_end::code_writer::Whitespace::None);
                list_element(&mut v.arena, *self.elements.last().unwrap())
                    .add_comment(comment_piece, false);
            }
        }

        // Preserve one blank line between the last element and a trailing
        // comment.
        let mut wrote_blank = false;
        if !has_element_after && !self.elements.is_empty() && lines_before_next_token > 1 {
            self.blanks_after.insert(*self.elements.last().unwrap());
            wrote_blank = true;
        }

        // Comments that are neither hanging nor leading are treated like
        // their own elements.
        for i in 0..separate_comments.len() {
            let comment = separate_comments.get(i);
            if separate_comments.lines_before(i) > 1 && !self.elements.is_empty() {
                self.blanks_after.insert(*self.elements.last().unwrap());
                wrote_blank = true;
            }

            let comment_piece =
                v.comment_piece(comment, crate::back_end::code_writer::Whitespace::None);
            let element = v.arena.add(ListElementPiece::comment(comment_piece));
            self.elements.push(element);
        }

        // Preserve a blank line after the last comment before the next
        // element, unless we've already written one before one of the
        // comments.
        if !wrote_blank && !self.elements.is_empty() && lines_before_next_token > 1 {
            self.blanks_after.insert(*self.elements.last().unwrap());
        }

        // Leading comments are written before the next element.
        for comment in leading_comments.iter() {
            let comment_piece =
                v.comment_piece(comment, crate::back_end::code_writer::Whitespace::None);
            self.leading_comments.push(comment_piece);
        }
    }

    /// Given the comments that followed the previous element before its
    /// comma and [comments_before_element], the comments before the element
    /// we are about to write (and after the preceding element's comma),
    /// splits them into to four comment sequences:
    ///
    /// * The inline block comments that should hang off the preceding
    ///   element before its comma.
    /// * The line comments that should hang off the end of the preceding
    ///   element after its comma.
    /// * The comments that should be formatted like separate elements.
    /// * The comments that should lead the beginning of the next element we
    ///   are about to write.
    ///
    /// For example:
    ///
    ///     function(
    ///       argument /* inline */, // hanging
    ///       // separate
    ///       /* leading */ nextArgument
    ///     );
    ///
    /// Calculating these takes into account whether there are newlines
    /// before or after the comments, and which side of the commas the
    /// comments appear on.
    ///
    /// If [has_element_after] is `true` then another element will be written
    /// after these comments. Otherwise, we are at the comments after the last
    /// element before the closing delimiter.
    #[allow(clippy::type_complexity)]
    fn split_comma_comments(
        &mut self,
        mut comments_before_element: CommentSequence<'a>,
        has_element_after: bool,
    ) -> (
        CommentSequence<'a>,
        CommentSequence<'a>,
        CommentSequence<'a>,
        CommentSequence<'a>,
    ) {
        // If we're on the final comma after the last element, the comma isn't
        // meaningful because there can't be leading comments after it.
        if !has_element_after {
            let before =
                std::mem::replace(&mut self.comments_before_comma, CommentSequence::empty());
            self.comments_before_comma = before.concatenate(comments_before_element);
            comments_before_element = CommentSequence::empty();
        }

        // Edge case: A line comment on the same line as the preceding element
        // but after the comma is treated as hanging.
        if !comments_before_element.is_empty()
            && comments_before_element.get(0).ty == CommentType::Line
            && comments_before_element.lines_before(0) == 0
        {
            let (hanging, remaining) = comments_before_element.split_at(1);
            let before =
                std::mem::replace(&mut self.comments_before_comma, CommentSequence::empty());
            self.comments_before_comma = before.concatenate(hanging);
            comments_before_element = remaining;
        }

        // Inline block comments before the `,` stay with the preceding
        // element, as in:
        //
        //     function(
        //       argument /* hanging */ /* comment */,
        //       argument,
        //     );
        let mut inline_comment_count = 0;
        if !self.elements.is_empty() {
            while inline_comment_count < self.comments_before_comma.len() {
                // Once we hit a single non-inline comment, the rest won't be
                // either.
                if !self.comments_before_comma.is_hanging(inline_comment_count)
                    || self.comments_before_comma.get(inline_comment_count).ty
                        != CommentType::InlineBlock
                {
                    break;
                }

                inline_comment_count += 1;
            }
        }

        let (inline_comments, remaining_comments_before_comma) = self
            .comments_before_comma
            .clone()
            .split_at(inline_comment_count);

        let mut hanging_comment_count = 0;
        if !self.elements.is_empty() {
            while hanging_comment_count < remaining_comments_before_comma.len() {
                // Once we hit a single non-hanging comment, the rest won't be
                // either.
                if !remaining_comments_before_comma.is_hanging(hanging_comment_count) {
                    break;
                }

                hanging_comment_count += 1;
            }
        }

        let (hanging_comments, separate_comments_before_comma) =
            remaining_comments_before_comma.split_at(hanging_comment_count);

        // Inline block comments on the same line as the next element lead at
        // the beginning of that line, as in:
        //
        //     function(
        //       argument,
        //       /* leading */ /* comment */ argument,
        //     );
        let mut leading_comment_count = 0;
        if has_element_after && !comments_before_element.is_empty() {
            while leading_comment_count < comments_before_element.len() {
                // Count backwards from the end. Once we hit a non-leading
                // comment, the preceding ones aren't either.
                let comment_index = comments_before_element.len() - leading_comment_count - 1;
                if !comments_before_element.is_leading(comment_index) {
                    break;
                }

                leading_comment_count += 1;
            }
        }

        let split = comments_before_element.len() - leading_comment_count;
        let (separate_comments_after_comma, leading_comments) =
            comments_before_element.split_at(split);

        // Comments that are neither hanging nor leading are formatted like
        // separate elements, as in:
        //
        //     function(
        //       argument,
        //       /* comment */
        //       argument,
        //       // another
        //     );
        let separate_comments =
            separate_comments_before_comma.concatenate(separate_comments_after_comma);

        (
            inline_comments,
            hanging_comments,
            separate_comments,
            leading_comments,
        )
    }

    /// Given an argument list, determines which if any of the arguments
    /// should get special block-like formatting as in the list literal in:
    ///
    ///     function(argument, [
    ///       block,
    ///       like,
    ///     ], argument);
    ///
    /// Looks at the [BlockFormat] types of all of the elements to determine
    /// if one of them should be block formatted.
    ///
    /// Also, if an argument list has an adjacent strings expression followed
    /// by a block formattable function expression, we allow the adjacent
    /// strings to split without forcing the list to split so that it can
    /// continue to have block formatting. This is pretty special-cased, but
    /// it makes calls to `test()` and `group()` look better and those are so
    /// common that it's worth massaging them some. It allows:
    ///
    ///     test('some long description'
    ///         'split across multiple lines', () {
    ///       expect(1, 1);
    ///     });
    ///
    /// Without this special rule, the newline in the adjacent strings would
    /// prevent block formatting and lead to the entire test body to be
    /// indented:
    ///
    ///     test(
    ///       'some long description'
    ///       'split across multiple lines',
    ///       () {
    ///         expect(1, 1);
    ///       },
    ///     );
    ///
    /// Stores the result of this calculation by setting flags on the
    /// [ListElement]s.
    fn set_block_argument(&mut self, v: &mut AstNodeVisitor<'a>, arguments: &[NodeId]) {
        let ast = v.ast;
        let candidate_index = candidate_block_argument(v, arguments);
        if candidate_index == -1 {
            return;
        }
        let candidate_index = candidate_index as usize;

        // The block argument must be positional.
        if ast.is::<NamedArgument>(arguments[candidate_index]) {
            return;
        }

        // Only allow up to one trailing argument after the block argument.
        // This handles the common `tags` and `timeout` named arguments in
        // `test()` and `group()` while still mostly having the block argument
        // be at the end of the argument list.
        if (candidate_index as i64) < arguments.len() as i64 - 2 {
            return;
        }

        // Edge case: If the first argument is adjacent strings and the second
        // argument is a function literal, with optionally a third non-block
        // argument, then treat the function as the block argument.
        //
        // This matches the `test()` and `group()` and other similar APIs
        // where you have a message string followed by a block-like function
        // expression but little else, as in:
        //
        //     test('Some long test description '
        //         'that splits into multiple lines.', () {
        //       expect(1 + 2, 3);
        //     });
        if candidate_index == 1
            && node_block_format_type(ast, arguments[1]) == BlockFormat::Function
            && !ast.is::<NamedArgument>(arguments[0])
        {
            let first_argument_format_type = node_block_format_type(ast, arguments[0]);
            if matches!(
                first_argument_format_type,
                BlockFormat::UnindentedAdjacentStrings | BlockFormat::IndentedAdjacentStrings
            ) {
                // The adjacent strings.
                let first = list_element(&mut v.arena, self.elements[0]);
                first.allow_newlines_when_unsplit = true;
                if first_argument_format_type == BlockFormat::UnindentedAdjacentStrings {
                    first.indent_when_block_formatted = true;
                }

                // The block-formattable function.
                list_element(&mut v.arena, self.elements[1]).allow_newlines_when_unsplit = true;
                return;
            }
        }

        // If we get here, we have a block argument.
        list_element(&mut v.arena, self.elements[candidate_index]).allow_newlines_when_unsplit =
            true;
    }
}

/// If an argument in [arguments] is a candidate to be block formatted,
/// returns its index.
///
/// If there is a single non-empty block bodied function expression in
/// [arguments], returns its index. Otherwise, if there is a single non-empty
/// collection literal in [arguments], returns its index. Otherwise, returns
/// `-1`.
fn candidate_block_argument(v: &AstNodeVisitor<'_>, arguments: &[NodeId]) -> i32 {
    let ast = v.ast;
    // The index of the function expression argument, or -1 if none has been
    // found or -2 if there are multiple.
    let mut function_index = -1;

    // The index of the collection literal argument, or -1 if none has been
    // found or -2 if there are multiple.
    let mut collection_index = -1;

    for (i, &argument) in arguments.iter().enumerate() {
        // See if it's an expression that supports block formatting.
        match node_block_format_type(ast, argument) {
            BlockFormat::Function => {
                if function_index >= 0 {
                    function_index = -2;
                } else {
                    function_index = i as i32;
                }
            }

            BlockFormat::Collection => {
                if collection_index >= 0 {
                    collection_index = -2;
                } else {
                    collection_index = i as i32;
                }
            }

            BlockFormat::Invocation
            | BlockFormat::IndentedAdjacentStrings
            | BlockFormat::UnindentedAdjacentStrings
            | BlockFormat::None => {} // Normal argument.
        }
    }

    if function_index >= 0 {
        return function_index;
    }
    if collection_index >= 0 {
        return collection_index;
    }

    -1
}
