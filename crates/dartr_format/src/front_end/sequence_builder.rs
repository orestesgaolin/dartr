// Dart source: dart_style lib/src/front_end/sequence_builder.dart

use dartr_ast::NodeId;
use dartr_syntax::TokenId;

use crate::ast_extensions::first_non_comment_token;
use crate::back_end::code_writer::{Indent, Whitespace};
use crate::piece::{
    BlockPiece, NewlinePiece, PieceId, PieceKind, Pieces, SequenceElementPiece, SequencePiece,
};

use super::ast_node_visitor::AstNodeVisitor;

/// Incrementally builds a [SequencePiece], including handling comments and
/// newlines that may appear before, between, or after its contents.
///
/// Comments are handled specially here so that we can give them better
/// formatting than we would be able to if we treated all comments
/// generally.
///
/// Most comments appear around statements in a block, members in a class,
/// or at the top level of a file. For those, we treat them essentially like
/// separate statements inside the sequence. This lets us gracefully handle
/// indenting them and supporting blank lines around them the same way we
/// handle other statements or members in a sequence.
#[derive(Default)]
pub struct SequenceBuilder {
    /// The opening bracket before the elements, if any.
    left_bracket: Option<PieceId>,

    /// The series of elements in the sequence.
    elements: Vec<PieceId>,

    /// The closing bracket after the elements, if any.
    right_bracket: Option<PieceId>,

    /// Whether a blank line should be allowed after the current element.
    allow_blank: bool,
}

fn sequence_element(arena: &mut Pieces, id: PieceId) -> &mut SequenceElementPiece {
    match arena.kind_mut(id) {
        PieceKind::SequenceElement(element) => element,
        _ => unreachable!("not a sequence element"),
    }
}

impl SequenceBuilder {
    pub fn new() -> SequenceBuilder {
        SequenceBuilder::default()
    }

    #[inline]
    pub fn build(&mut self, v: &mut AstNodeVisitor<'_>) -> PieceId {
        self.build_with(v, false)
    }

    pub fn build_with(&mut self, v: &mut AstNodeVisitor<'_>, force_split: bool) -> PieceId {
        // If the sequence only contains a single piece, just return it
        // directly and discard the unnecessary wrapping.
        if self.left_bracket.is_none() && self.elements.len() == 1 && self.right_bracket.is_none()
        {
            let element = sequence_element(&mut v.arena, self.elements[0]);
            if element.hanging_comments.is_empty() {
                return element.piece;
            }
        }

        // If there are no elements, don't bother making a SequencePiece or
        // BlockPiece.
        if self.elements.is_empty() {
            let left = self.left_bracket;
            let right = self.right_bracket;
            return v.build(|v| {
                if let Some(bracket) = left {
                    v.add(bracket);
                }

                if force_split || left.is_none() {
                    let newline = v.arena.add(NewlinePiece);
                    v.add(newline);
                }

                if let Some(bracket) = right {
                    v.add(bracket);
                }
            });
        }

        // Discard any trailing blank line after the last element.
        sequence_element(&mut v.arena, *self.elements.last().unwrap()).blank_after = false;

        let sequence = v
            .arena
            .add(SequencePiece::new(std::mem::take(&mut self.elements)));
        if let (Some(left), Some(right)) = (self.left_bracket, self.right_bracket) {
            return v.arena.add(BlockPiece::new(left, sequence, right));
        }

        sequence
    }

    /// Adds the opening [bracket] to the built sequence.
    pub fn left_bracket(&mut self, v: &mut AstNodeVisitor<'_>, bracket: TokenId, soft: bool) {
        self.left_bracket = Some(v.token_piece_with(bracket, None, false, soft));
    }

    /// Adds the closing [bracket] to the built sequence along with any
    /// comments that precede it.
    pub fn right_bracket(&mut self, v: &mut AstNodeVisitor<'_>, bracket: TokenId) {
        // Place any comments before the bracket inside the block.
        self.add_comments_before(v, bracket);
        self.right_bracket = Some(v.token_piece(bracket));
    }

    /// Adds [piece] to this sequence.
    ///
    /// The caller should have already called [add_comments_before()] with
    /// the first token in [piece].
    #[inline]
    pub fn add(&mut self, v: &mut AstNodeVisitor<'_>, piece: PieceId) {
        self.add_with(v, piece, Indent::None, true);
    }

    /// Adds [piece] to this sequence.
    pub fn add_with(
        &mut self,
        v: &mut AstNodeVisitor<'_>,
        piece: PieceId,
        indent: Indent,
        allow_blank_after: bool,
    ) {
        let element = v.arena.add(SequenceElementPiece::new(indent, piece));
        self.elements.push(element);

        self.allow_blank = allow_blank_after;
    }

    /// Visits [node] and adds the resulting [Piece] to this sequence,
    /// handling any comments or blank lines that appear before it.
    #[inline]
    pub fn visit(&mut self, v: &mut AstNodeVisitor<'_>, node: NodeId) {
        self.visit_with(v, node, Indent::None, false);
    }

    /// Visits [node] and adds the resulting [Piece] to this sequence,
    /// handling any comments or blank lines that appear before it.
    ///
    /// If [blank_before] is `true`, then ensures there is a blank line
    /// before [token]. If there any blank lines around the comments, then
    /// the first one will be preserved. Otherwise, writes a blank line before
    /// the first comment.
    pub fn visit_with(
        &mut self,
        v: &mut AstNodeVisitor<'_>,
        node: NodeId,
        indent: Indent,
        blank_before: bool,
    ) {
        let token = first_non_comment_token(v.ast, node);

        let wrote_blank = self.add_comments_before_with(v, token, indent, blank_before);

        if blank_before && !wrote_blank {
            self.add_blank(v);
        }

        let piece = v.node_piece(node);
        self.add_with(v, piece, indent, true);
    }

    /// Appends a blank line before the next piece in the sequence.
    pub fn add_blank(&mut self, v: &mut AstNodeVisitor<'_>) {
        let Some(&last) = self.elements.last() else {
            return;
        };
        if !self.allow_blank {
            return;
        }
        sequence_element(&mut v.arena, last).blank_after = true;
    }

    /// Writes any comments appearing before [token] to the sequence.
    #[inline]
    pub fn add_comments_before(&mut self, v: &mut AstNodeVisitor<'_>, token: TokenId) -> bool {
        self.add_comments_before_with(v, token, Indent::None, false)
    }

    /// Writes any comments appearing before [token] to the sequence.
    ///
    /// Also handles blank lines between preceding comments and elements and
    /// the subsequent element.
    ///
    /// Comments between sequence elements get special handling where
    /// comments on their own line become standalone sequence elements.
    ///
    /// If [blank_before] is `true`, then ensures there is a blank line
    /// before [token]. If there any blank lines around the comments, then
    /// the first one will be preserved. Otherwise, writes a blank line before
    /// the first comment.
    ///
    /// Returns `true` if processing the comments ended up writing any blank
    /// lines.
    pub fn add_comments_before_with(
        &mut self,
        v: &mut AstNodeVisitor<'_>,
        token: TokenId,
        indent: Indent,
        blank_before: bool,
    ) -> bool {
        let mut wrote_blank = false;
        let comments = v.comments.take_comments_before(token);

        // Default to putting the blank line before all comments unless there
        // are blank lines inside or after them.
        if blank_before && !comments.contains_blank_line() {
            self.add_blank(v);
            wrote_blank = true;
        }

        for i in 0..comments.len() {
            let comment = v.comment_piece(comments.get(i), Whitespace::None);

            if !self.elements.is_empty() && comments.is_hanging(i) {
                // Attach the comment to the previous element.
                let last = *self.elements.last().unwrap();
                sequence_element(&mut v.arena, last)
                    .hanging_comments
                    .push(comment);
            } else {
                if comments.lines_before(i) > 1 {
                    // Always preserve a blank line above sequence-level
                    // comments.
                    self.allow_blank = true;
                    self.add_blank(v);
                    wrote_blank = true;
                }

                // Write the comment as its own sequence piece.
                let element = v.arena.add(SequenceElementPiece::new(indent, comment));
                self.elements.push(element);
            }
        }

        // Write a blank before the token if there should be one.
        if comments.lines_before_next_token() > 1 {
            // If we just wrote a comment, then allow a blank line between it
            // and the element.
            if !comments.is_empty() {
                self.allow_blank = true;
            }

            self.add_blank(v);
            wrote_blank = true;
        }

        wrote_blank
    }
}
