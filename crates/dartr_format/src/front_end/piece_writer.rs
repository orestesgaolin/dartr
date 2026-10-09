// Dart source: dart_style lib/src/front_end/piece_writer.dart

//! Dart `PieceWriter` holds a reference to the visitor and the visitor holds
//! a reference to the `PieceWriter`. Here the state of the `PieceWriter` is
//! [PieceWriter] (a field of [AstNodeVisitor]) and its methods are methods
//! of [AstNodeVisitor], so Dart `pieces.token(...)` is `self.token(...)`.

use dartr_ast::{Id, NodeId};
use dartr_syntax::TokenId;

use crate::back_end::code_writer::Whitespace;
use crate::back_end::solution_cache::SolutionCache;
use crate::back_end::solver::Solver;
use crate::piece::{
    AdjacentPiece, Commas, LeadingCommentPiece, ListStyle, PieceId, PieceKind, Pieces, SpacePiece,
    TextPiece,
};
use crate::source_code::SourceCode;
use crate::text::utf16_len;

use super::ast_node_visitor::AstNodeVisitor;
use super::comment_writer::{CommentSequence, SourceComment};
use super::delimited_list_builder::DelimitedListBuilder;
use super::formatting_style::FormattingStyle;
use super::piece_factory::NodeContext;
use super::sequence_builder::SequenceBuilder;

/// Something that can be passed where Dart passes an `AstNode?`.
pub trait OptNode {
    fn opt_node(self) -> Option<NodeId>;
}

impl OptNode for NodeId {
    #[inline]
    fn opt_node(self) -> Option<NodeId> {
        Some(self)
    }
}

impl OptNode for Option<NodeId> {
    #[inline]
    fn opt_node(self) -> Option<NodeId> {
        self
    }
}

impl<T: ?Sized> OptNode for Id<T> {
    #[inline]
    fn opt_node(self) -> Option<NodeId> {
        Some(self.raw())
    }
}

impl<T: ?Sized> OptNode for Option<Id<T>> {
    #[inline]
    fn opt_node(self) -> Option<NodeId> {
        self.map(Id::raw)
    }
}

/// The state of Dart `PieceWriter`: builds [TextPiece]s for [Token]s and
/// comments.
///
/// Handles updating selection markers and attaching comments to the tokens
/// before and after the comments.
pub struct PieceWriter<'a> {
    source: &'a SourceCode,

    /// The most recent previously-created [CodePiece].
    ///
    /// We hold a reference to this so we can attach hanging comments to it,
    /// which we don't discover until we reach the token after the one used to
    /// create this piece.
    previous_code: Option<PieceId>,

    /// Whether we have reached a token or comment that lies at or beyond the
    /// selection start offset in the original code.
    ///
    /// Makes sure we insert the start marker in some piece even if it happens
    /// to lie between two tokens in the input.
    passed_selection_start: bool,

    /// Whether we have reached a token or comment that lies at or beyond the
    /// selection end offset in the original code.
    ///
    /// Makes sure we insert the end marker in some piece even if it happens
    /// to lie between two tokens in the input.
    passed_selection_end: bool,

    /// The character offset of the end of the selection with any trailing
    /// whitespace removed.
    ///
    /// This can only be accessed if there is a selection.
    selection_end: Option<usize>,

    /// The stack of pieces being built by calls to [build()].
    ///
    /// Each call to [build()] pushes a new list onto this stack. All of the
    /// pieces written during that call to [build()] end up in that list.
    /// When the [build()] callback returns, the topmost list is popped and
    /// the result returned as an [AdjacentPiece] (or just the single piece
    /// if there is only one).
    pieces: Vec<Vec<PieceId>>,

    /// The last piece in [pieces], if it's a [CodePiece] that can have more
    /// code appended to it or `None` if there is no trailing element or the
    /// trailing piece can't be appended to.
    current_code: Option<PieceId>,

    /// If [space()] has been called and we haven't appended a space to the
    /// previous code or adding a [SpacePiece] yet.
    pending_space: bool,
}

impl<'a> PieceWriter<'a> {
    pub fn new(source: &'a SourceCode) -> PieceWriter<'a> {
        PieceWriter {
            source,
            previous_code: None,
            passed_selection_start: false,
            passed_selection_end: false,
            selection_end: None,
            pieces: Vec::new(),
            current_code: None,
            pending_space: false,
        }
    }
}

fn text_piece(arena: &mut Pieces, id: PieceId) -> &mut TextPiece {
    match arena.kind_mut(id) {
        PieceKind::Text(text) => text,
        _ => unreachable!("not a text piece"),
    }
}

impl<'a> AstNodeVisitor<'a> {
    /// Writes [token] to the piece currently being written.
    ///
    /// Does nothing if [token] is `None`.
    #[inline]
    pub fn token(&mut self, token: impl Into<Option<TokenId>>) {
        self.token_with(token, false, false, false);
    }

    /// Writes [token] as "soft" code. See
    /// [FormattingStyle::use_soft_overflow] for more details.
    #[inline]
    pub fn token_soft(&mut self, token: impl Into<Option<TokenId>>) {
        self.token_with(token, false, false, true);
    }

    /// Writes [token] to the piece currently being written.
    ///
    /// Does nothing if [token] is `None`. If [space_before] is `true`,
    /// writes a space before the token, likewise with [space_after].
    ///
    /// If [soft] is `true`, then the token is considered to be "soft" code.
    /// See [FormattingStyle::use_soft_overflow] for more details.
    pub fn token_with(
        &mut self,
        token: impl Into<Option<TokenId>>,
        space_before: bool,
        space_after: bool,
        soft: bool,
    ) {
        let Some(token) = token.into() else {
            return;
        };
        let soft = soft && self.style.use_soft_overflow();

        if space_before {
            self.space();
        }

        // TODO(rnystrom): If [_currentCode] is `null` but [_pendingSpace] is
        // `true`, it should be possible to create a new code piece and write
        // the leading space to it instead of having a leading SpacePiece.
        // Unfortunately, that sometimes leads to duplicate spaces in the
        // output, so it might take some tweaking to get working.

        let ast = self.ast;
        if ast.tokens.get(token).preceding_comments.is_some() {
            // Don't append to the previous token if there is a comment after
            // it.
            self.begin_code_token(token, soft);
        } else if let Some(code) = self
            .writer
            .current_code
            .filter(|&code| text_piece(&mut self.arena, code).is_soft == soft)
        {
            // Append to the current code piece. Each piece must be uniformly
            // all soft or all hard text, so if this token doesn't match the
            // existing piece's softness, then start a new piece.
            if self.writer.pending_space {
                text_piece(&mut self.arena, code).append(" ", false, None, None);
                self.writer.pending_space = false;
            }

            self.write_text(
                code,
                ast.tokens.lexeme(token),
                ast.tokens.offset(token) as usize,
                false,
            );
        } else {
            self.begin_code_token(token, soft);
        }

        if space_after {
            self.space();
        }
    }

    /// Writes [token], which may contain internal newlines.
    pub fn multiline_token(&mut self, token: TokenId, soft: bool) {
        let ast = self.ast;
        let comments = self.comments.comments_before(token);

        let leading = self.split_comments(comments, token);
        let piece = self.arena.add(TextPiece::code(
            leading,
            self.style.use_soft_overflow() && soft,
        ));
        self.write_text(
            piece,
            ast.tokens.lexeme(token),
            ast.tokens.offset(token) as usize,
            true,
        );

        // Remember it so we can attach hanging comments later.
        self.writer.previous_code = Some(piece);

        // Multiline tokens are always their own pieces.
        self.add(piece);
    }

    /// Visits [node] if not `None` and writes the result.
    #[inline]
    pub fn visit(&mut self, node: impl OptNode) {
        self.visit_with(node, false, false, NodeContext::None);
    }

    /// Visits [node] if not `None` in [context] and writes the result.
    #[inline]
    pub fn visit_in(&mut self, node: impl OptNode, context: NodeContext) {
        self.visit_with(node, false, false, context);
    }

    /// Visits [node] if not `None` and writes the result.
    pub fn visit_with(
        &mut self,
        node: impl OptNode,
        space_before: bool,
        space_after: bool,
        context: NodeContext,
    ) {
        let Some(node) = node.opt_node() else {
            return;
        };

        if space_before {
            self.space();
        }
        self.visit_node(node, context);
        if space_after {
            self.space();
        }
    }

    /// Appends a space before the previous code being written and the next.
    #[inline]
    pub fn space(&mut self) {
        self.writer.pending_space = true;
    }

    /// Writes an optional modifier that precedes other code.
    #[inline]
    pub fn modifier(&mut self, keyword: Option<TokenId>) {
        self.token_with(keyword, false, true, false);
    }

    /// Adds [piece] to the current piece being built.
    pub fn add(&mut self, piece: PieceId) {
        self.flush_space();
        self.writer.pieces.last_mut().unwrap().push(piece);
        self.writer.current_code = None;
    }

    /// Creates and returns a new piece.
    ///
    /// Invokes [build_callback]. All tokens and AST nodes written during that
    /// callback are collected into the returned piece.
    #[inline]
    pub fn build(&mut self, build_callback: impl FnOnce(&mut Self)) -> PieceId {
        self.build_with_metadata(&[], false, build_callback)
    }

    /// Creates and returns a new piece.
    ///
    /// Invokes [build_callback]. All tokens and AST nodes written during that
    /// callback are collected into the returned piece.
    ///
    /// If [metadata] is non-empty, then wraps the resulting piece in another
    /// piece beginning with that metadata. If [inline_metadata] is `true`,
    /// then the metadata is allowed to stay on the same line as the content.
    /// Otherwise, a newline is inserted after every annotation.
    pub fn build_with_metadata(
        &mut self,
        metadata: &[NodeId],
        inline_metadata: bool,
        build_callback: impl FnOnce(&mut Self),
    ) -> PieceId {
        self.flush_space();
        self.writer.current_code = None;

        let mut leading_pieces = Vec::new();
        if !metadata.is_empty() {
            for &annotation in metadata {
                let piece = self.node_piece(annotation);
                leading_pieces.push(piece);
            }

            // If there are comments between the metadata and declaration,
            // then hoist them out too so they don't get embedded inside the
            // beginning piece of the declaration. [SequenceBuilder] handles
            // that for most comments preceding a declaration but won't see
            // these ones because they come after the metadata.
            let ast = self.ast;
            let next = ast.tokens.next(ast.end_token(*metadata.last().unwrap()));
            let comments = self.take_comments_before(next);
            leading_pieces.extend(comments);
        }

        self.writer.pieces.push(Vec::new());

        build_callback(self);

        self.flush_space();
        self.writer.current_code = None;

        let built_pieces = self.writer.pieces.pop().unwrap();
        debug_assert!(!built_pieces.is_empty());

        let built_piece = if built_pieces.len() == 1 {
            built_pieces[0]
        } else {
            self.arena.add(AdjacentPiece::new(built_pieces))
        };

        if leading_pieces.is_empty() {
            // No metadata, so return the content piece directly.
            built_piece
        } else if inline_metadata {
            // Wrap the metadata and content in a splittable list.
            let mut list = DelimitedListBuilder::new(ListStyle {
                commas: Commas::None,
                space_when_unsplit: true,
                ..ListStyle::default()
            });

            for piece in leading_pieces {
                list.add(self, piece);
            }

            list.add(self, built_piece);
            list.build(self)
        } else {
            // Wrap the metadata and content in a sequence.
            let mut sequence = SequenceBuilder::new();
            for piece in leading_pieces {
                sequence.add(self, piece);
            }

            sequence.add(self, built_piece);
            sequence.build_with(self, true)
        }
    }

    /// Creates a separate piece for [token], including any comments that
    /// should be attached to that token.
    ///
    /// If [discarded_token] is given, it is a token immediately before
    /// [token] that is going to be discarded. Passing it in here ensures any
    /// comments before it are preserved.
    ///
    /// If [comma_after] is `true`, looks for and writes a comma following
    /// the token if there is one.
    ///
    /// If [soft] is `true`, then the token is considered to be "soft" code.
    /// See [FormattingStyle::use_soft_overflow] for more details.
    pub fn token_piece_with(
        &mut self,
        token: TokenId,
        discarded_token: Option<TokenId>,
        comma_after: bool,
        soft: bool,
    ) -> PieceId {
        let token_piece = self.make_code_piece(token, discarded_token, soft);

        if comma_after {
            let ast = self.ast;
            let next_token = ast.tokens.next(token);
            if ast.tokens.lexeme(next_token) == "," {
                let comma = self.make_code_piece(next_token, None, soft);
                return self.arena.add(AdjacentPiece::new(vec![token_piece, comma]));
            }
        }

        token_piece
    }

    /// Writes [metadata] followed by the code written by [build_callback].
    ///
    /// If [metadata] is empty, then invokes [build_callback] directly.
    /// Otherwise, creates a new [Piece] that contains the pieces written
    /// from [metadata] followed by the code written by [build_callback].
    pub fn with_metadata(
        &mut self,
        metadata: &[NodeId],
        inline_metadata: bool,
        build_callback: impl FnOnce(&mut Self),
    ) {
        // If there's no metadata (the common case), then call the callback
        // directly instead of creating a separate AdjacentBuilder. That way,
        // we avoid splitting pieces at the boundary here if not needed.
        if metadata.is_empty() {
            build_callback(self);
        } else {
            let piece = self.build_with_metadata(metadata, inline_metadata, build_callback);
            self.add(piece);
        }
    }

    /// Creates a new [Piece] for [comment] and returns it.
    pub fn comment_piece(
        &mut self,
        comment: &SourceComment<'_>,
        trailing_whitespace: Whitespace,
    ) -> PieceId {
        let soft = self.style.use_soft_overflow();
        let piece = match comment.text {
            "// dart format off" => TextPiece::enable_formatting_comment(
                comment.offset + utf16_len(comment.text),
                trailing_whitespace,
                soft,
                false,
            ),
            "// dart format on" => TextPiece::enable_formatting_comment(
                comment.offset + utf16_len(comment.text),
                trailing_whitespace,
                soft,
                true,
            ),
            _ => TextPiece::comment(trailing_whitespace, soft),
        };
        let piece = self.arena.add(piece);

        self.write_text(
            piece,
            comment.text,
            comment.offset,
            comment.ty.may_be_multiline(),
        );
        piece
    }

    /// Applies any hanging comments before [token] to the preceding
    /// [CodePiece] and takes and returns any remaining leading comments.
    pub fn take_comments_before(&mut self, token: TokenId) -> Vec<PieceId> {
        let comments = self.comments.take_comments_before(token);
        self.split_comments(comments, token)
    }

    /// Takes any comments preceding [first_token] and wraps them around the
    /// piece generated by [build_callback].
    ///
    /// For comments preceding a single AST node, this hoisting is handled
    /// automatically. But there are some places in the language where there
    /// is a conceptual piece of syntax that we want to hoist the comments out
    /// of so that the syntax doesn't split but there isn't actually a single
    /// AST node associated with that syntax.
    ///
    /// This method lets you hoist comments before an arbitary amount of
    /// syntax visited and built by calling [build_callback].
    pub fn hoist_leading_comments(
        &mut self,
        first_token: TokenId,
        build_callback: impl FnOnce(&mut Self) -> PieceId,
    ) {
        let leading_comments = self.take_comments_before(first_token);

        let mut piece = build_callback(self);
        if !leading_comments.is_empty() {
            piece = self
                .arena
                .add(LeadingCommentPiece::new(leading_comments, piece));
        }

        self.add(piece);
    }

    /// Begins a new [CodeToken] that can potentially have more code written
    /// to it.
    fn begin_code_token(&mut self, token: TokenId, soft: bool) {
        self.flush_space();
        let code = self.make_code_piece(token, None, soft);
        self.writer.pieces.last_mut().unwrap().push(code);
        self.writer.current_code = Some(code);
    }

    /// Outputs any pending space before more code is written or the current
    /// piece is completed.
    fn flush_space(&mut self) {
        if !self.writer.pending_space {
            return;
        }

        let space = self.arena.add(SpacePiece);
        self.writer.pieces.last_mut().unwrap().push(space);
        self.writer.pending_space = false;
    }

    /// Creates a [CodePiece] for [token] and handles any comments that
    /// precede it, which get attached either as hanging comments on the
    /// preceding [CodePiece] or leading comments on this one.
    ///
    /// If [discarded_token] is given, it is a token immediately before
    /// [token] that is going to be discarded. Passing it in here ensures any
    /// comments before it are preserved.
    ///
    /// If [soft] is `true`, then the token is considered to be "soft" code.
    /// See [FormattingStyle::use_soft_overflow] for more details.
    fn make_code_piece(
        &mut self,
        token: TokenId,
        discarded_token: Option<TokenId>,
        soft: bool,
    ) -> PieceId {
        let ast = self.ast;
        let mut comments = self.comments.comments_before(token);

        // Include any comments on the preceding discarded token, if there is
        // one.
        if let Some(discarded_token) = discarded_token {
            comments = self
                .comments
                .comments_before(discarded_token)
                .concatenate(comments);
        }

        let leading = self.split_comments(comments, token);
        let piece = self.arena.add(TextPiece::code(
            leading,
            self.style.use_soft_overflow() && soft,
        ));
        self.write_text(
            piece,
            ast.tokens.lexeme(token),
            ast.tokens.offset(token) as usize,
            false,
        );

        // Remember it so we can attach hanging comments later.
        self.writer.previous_code = Some(piece);
        piece
    }

    /// Splits [comments] which precede [token] into [CommentPiece]s that
    /// hang off the preceding [CodePiece] and those that are leading
    /// comments on the [CodePiece] for [token].
    ///
    /// Attaches hanging comments to [previous_code]. Returns the list of
    /// leading comments that should precede [token].
    fn split_comments(&mut self, comments: CommentSequence<'_>, token: TokenId) -> Vec<PieceId> {
        if comments.is_empty() {
            return Vec::new();
        }

        let lexeme = self.ast.tokens.lexeme(token);
        let mut leading_comments = Vec::new();
        let mut after_hanging = false;
        for i in 0..comments.len() {
            let comment = comments.get(i);

            // The whitespace after this comment before the next comment or
            // code.
            let trailing_whitespace = if comment.requires_newline() {
                Whitespace::Newline
            } else {
                match lexeme {
                    // No space between a comment and delimiting punctuation.
                    "]" | "}" | "," | ";" => Whitespace::None,
                    _ => Whitespace::Space,
                }
            };

            let piece = self.comment_piece(comment, trailing_whitespace);

            if !after_hanging && comments.is_hanging(i) {
                // Attach it to the previous CodePiece.
                let previous = self.writer.previous_code.unwrap();
                text_piece(&mut self.arena, previous).add_hanging_comment(piece);
            } else {
                // Add it to the list of leading comments for the upcoming
                // token.
                leading_comments.push(piece);

                // Once we've found a single non-hanging comment, all
                // subsequent ones must also be non-hanging since they follow
                // it.
                after_hanging = true;
            }
        }

        leading_comments
    }

    /// Appends [text] to [piece] and updates any selection markers that fall
    /// within it.
    ///
    /// The [offset] parameter is the offset in the original source code of
    /// the beginning of where [text] appears.
    fn write_text(&mut self, piece: PieceId, text: &str, offset: usize, multiline: bool) {
        let (selection_start, selection_end) = if self.writer.source.selection_start.is_some() {
            let length = utf16_len(text);
            (
                self.find_selection_start_within(offset, length),
                self.find_selection_end_within(offset, length),
            )
        } else {
            (None, None)
        };
        text_piece(&mut self.arena, piece).append(text, multiline, selection_start, selection_end);
    }

    /// Finishes writing and returns a [SourceCode] containing the final
    /// output and updated selection, if any.
    pub fn finish(
        mut self,
        style: &FormattingStyle,
        source: &SourceCode,
        root_piece: PieceId,
    ) -> SourceCode {
        if style.pin_state_by_page_width_before_solving() {
            pin_pieces_by_page_width(
                &mut self.arena,
                root_piece,
                style.page_width - style.leading_indent,
            );
        }

        let arena = &self.arena;
        let mut cache = SolutionCache::new(style.clone());
        let solver = Solver::new(style.page_width, style.leading_indent, style.leading_indent);

        let solution = solver.format(&mut cache, arena, root_piece, None);
        solution.code().build(source, &style.line_ending)
    }

    /// Returns the number of characters past [position] in the source where
    /// the selection start appears if it appears within `position + length`.
    ///
    /// Returns `None` if the selection start has already been processed or
    /// is not within that range.
    fn find_selection_start_within(&mut self, position: usize, length: usize) -> Option<usize> {
        // If there is no selection, do nothing.
        let absolute_start = self.writer.source.selection_start?;

        // If we've already passed it, don't consider it again.
        if self.writer.passed_selection_start {
            return None;
        }

        // Calculate the start position relative to [offset].
        let mut relative_start = absolute_start as i64 - position as i64;

        // If it started in whitespace before this text, push it forward to
        // the beginning of the non-whitespace text.
        if relative_start < 0 {
            relative_start = 0;
        }

        // If we haven't reached it yet, don't consider it. If the start point
        // is right at the end of the token, don't consider that as reaching
        // it. Instead, we'll reach it on the next token, which will correctly
        // push it past any whitespace after this token and move it to the
        // beginning of the next one.
        if relative_start >= length as i64 {
            return None;
        }

        // We found it.
        self.writer.passed_selection_start = true;
        Some(relative_start as usize)
    }

    /// Returns the number of characters past [position] in the source where
    /// the selection endpoint appears if it appears before
    /// `position + length`.
    ///
    /// Returns `None` if the selection endpoint has already been processed
    /// or is not within that range.
    fn find_selection_end_within(&mut self, position: usize, length: usize) -> Option<usize> {
        // If there is no selection, do nothing.
        self.writer.source.selection_length?;

        // If we've already passed it, don't consider it again.
        if self.writer.passed_selection_end {
            return None;
        }

        let selection_end = self.selection_end();
        let mut relative_end = selection_end as i64 - position as i64;

        // If it started in whitespace before this text, push it forward to
        // the beginning of the non-whitespace text.
        if relative_end < 0 {
            relative_end = 0;
        }

        // If we haven't reached the end point yet, don't consider it. Note
        // that, unlike [find_selection_start_within], we do consider the end
        // point being right at the end of this token to be reaching it. That
        // way, we don't push the end point *past* the next span of whitespace
        // and instead pull it tight to the end of this text.
        if relative_end > length as i64 {
            return None;
        }

        // In [find_selection_start_within], if the start marker is between
        // two tokens, we push it forward to the next one. In the above
        // statement, we push the end marker earlier to the previous token. If
        // the entire selection is in whitespace between two tokens, that
        // would cause the start and ends to cross. Prevent that and instead
        // push the end marker to the beginning of the next token where the
        // start marker will also be pushed.
        if relative_end == length as i64
            && Some(selection_end) == self.writer.source.selection_start
        {
            return None;
        }

        // We found it.
        self.writer.passed_selection_end = true;

        Some(relative_end as usize)
    }

    /// The character offset of the end of the selection with any trailing
    /// whitespace removed (Dart `late final _selectionEnd`).
    fn selection_end(&mut self) -> usize {
        if let Some(end) = self.writer.selection_end {
            return end;
        }
        let end = self.find_selection_end();
        self.writer.selection_end = Some(end);
        end
    }

    /// Calculates the character offset in the source text of the end of the
    /// selection.
    ///
    /// Removes any trailing whitespace from the selection. For example, if
    /// the original selection markers are:
    ///
    ///     function(lotsOfSpac‹eAfter,     ›     andBefore);
    ///
    /// Then this function moves the end marker to:
    ///
    ///     function(lotsOfSpac‹eAfter,›          andBefore);
    ///
    /// We do this because the formatter itself rewrites whitespace so it's
    /// not useful or even meaningful to try to preserve a selection's
    /// location within whitespace. Instead, we "rubberband" the end marker
    /// forward to the nearest non-whitespace character.
    fn find_selection_end(&self) -> usize {
        let source = self.writer.source;
        let start = source.selection_start.unwrap();
        let mut end = start + source.selection_length.unwrap();

        // If the selection bumps to the end of the source, pin it there.
        let units: Vec<u16> = source.text.encode_utf16().collect();
        if end == units.len() {
            return end;
        }

        // Trim off any trailing whitespace.
        while end > start {
            // Stop if we hit anything other than space, tab, newline or
            // carriage return.
            let char = units[end - 1];
            if char != 0x20 && char != 0x09 && char != 0x0a && char != 0x0d {
                break;
            }

            end -= 1;
        }

        end
    }
}

/// Traverse the piece tree at [root_piece] and attempt to pin pieces to
/// states if the [page_width] and the contents of the pieces are enough to
/// determine what state the piece will end up in.
fn pin_pieces_by_page_width(arena: &mut Pieces, root_piece: PieceId, page_width: i32) {
    // An explicit stack instead of recursion: deep piece trees would
    // otherwise overflow the stack. The order is the same as the Dart
    // pre-order traversal.
    let mut stack = vec![root_piece];
    let mut children = Vec::new();
    while let Some(piece) = stack.pop() {
        if let Some(state) = arena.fixed_state_for_page_width(piece, page_width) {
            arena.pin(piece, state);
        }

        children.clear();
        arena.for_each_child(piece, &mut |child| children.push(child));
        stack.extend(children.iter().rev());
    }
}
