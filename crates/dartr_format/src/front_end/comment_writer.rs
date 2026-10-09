// Dart source: dart_style lib/src/front_end/comment_writer.dart

use rustc_hash::FxHashSet;

use dartr_ast::Ast;
use dartr_syntax::{LineInfo, TokenId, TokenType};

use crate::comment_type::CommentType;

/// Functionality used by [AstNodeVisitor], [DelimitedListBuilder], and
/// [SequenceBuilder] to build pieces from the comment tokens between
/// meaningful tokens used by AST nodes.
///
/// Also handles tracking newlines between tokens and comments so that
/// information can be used to preserve discretionary blank lines in places
/// where they are allowed. These are handled along with comments because
/// both comments and whitespace are found between the linear series of
/// [Token]s produced by the analyzer parser. Likewise, both are output as
/// whitespace (in the sense of not being executable code) interleaved with
/// the [Piece]-building code that walks the actual AST and processes the
/// code tokens.
///
/// Comments are a challenge because they confound the intuitive tree-like
/// structure of the code. A comment can appear between any two tokens, and a
/// line comment can force the formatter to insert a newline in places where
/// one wouldn't otherwise make sense. When that happens, the formatter then
/// has to decide how to indent the next line.
///
/// At the same time, comments appearing in idiomatic locations like between
/// statements should be formatted gracefully and give users control over the
/// blank lines around them. To support all of that, comments are handled in
/// a couple of different ways.
///
/// Comments between top-level declarations, member declarations inside
/// types, and statements are handled directly by [SequenceBuilder]. Comments
/// inside argument lists, collection literals, and other similar constructs
/// are handled directly be [DelimitedPieceBuilder].
///
/// All other comments occur inside the middle of some expression or other
/// construct. These get directly embedded in the [TextPiece] of the code
/// being written. When that [TextPiece] is output later, it will include the
/// comments as well.
pub struct CommentWriter<'a> {
    ast: &'a Ast,

    line_info: &'a LineInfo,

    /// The tokens whose preceding comments have already been taken by calls
    /// to [take_comments_before()].
    taken_tokens: FxHashSet<TokenId>,
}

impl<'a> CommentWriter<'a> {
    pub fn new(ast: &'a Ast, line_info: &'a LineInfo) -> CommentWriter<'a> {
        CommentWriter {
            ast,
            line_info,
            taken_tokens: FxHashSet::default(),
        }
    }

    /// Returns the comments that appear before [token].
    ///
    /// The caller is required to write them because a later call to write
    /// [token] for this token will not write the preceding comments. Used by
    /// [SequenceBuilder] and [DelimitedListBuilder] which handle comment
    /// formatting themselves.
    pub fn take_comments_before(&mut self, token: TokenId) -> CommentSequence<'a> {
        if !self.taken_tokens.insert(token) {
            return CommentSequence::empty();
        }
        self.comments_before_internal(token)
    }

    /// Returns the comments that appear before [token].
    pub fn comments_before(&self, token: TokenId) -> CommentSequence<'a> {
        // In the common case where there are no comments before the token,
        // early out. This avoids calculating the number of newlines between
        // every pair of tokens which is slow and unnecessary.
        if self.ast.tokens.get(token).preceding_comments.is_none() {
            return CommentSequence::empty();
        }

        // Don't yield the comments if some other construct already handled
        // them.
        if self.taken_tokens.contains(&token) {
            return CommentSequence::empty();
        }

        self.comments_before_internal(token)
    }

    /// Takes all of the comment tokens preceding [token] and builds a
    /// [CommentSequence] that tracks them and the whitespace between them.
    fn comments_before_internal(&self, token: TokenId) -> CommentSequence<'a> {
        let tokens = &self.ast.tokens;
        let previous = tokens.previous(token);
        let mut previous_line = self.end_line(previous);
        let token_line = self.start_line(token);

        // Edge case: The analyzer includes the "\n" in the script tag's
        // lexeme, which confuses some of these calculations. We don't want to
        // allow a blank line between the script tag and a following comment
        // anyway, so just override the script tag's line.
        if tokens.ty(previous) == TokenType::SCRIPT_TAG {
            previous_line = token_line;
        }

        let first_comment = tokens.get(token).preceding_comments;
        let mut comments = CommentSequence {
            lines_between: Vec::new(),
            comments: Vec::new(),
        };
        for comment in tokens.comments(token) {
            let comment_line = self.start_line(comment);

            let text = dart_trim(tokens.lexeme(comment));
            let mut lines_before = comment_line - previous_line;
            let mut flush_left = self.start_column(comment) == 1;

            if text.starts_with("///") && !text.starts_with("////") {
                // Line doc comments are always indented even if they were
                // flush left.
                flush_left = false;

                // Always add a blank line (if possible) before a doc comment
                // block.
                if comment == first_comment {
                    lines_before = 2;
                }
            }

            let ty = if text.starts_with("///") && !text.starts_with("////")
                || text.starts_with("/**") && text != "/**/"
            {
                CommentType::Doc
            } else if tokens.ty(comment) == TokenType::SINGLE_LINE_COMMENT {
                CommentType::Line
            } else if comment_line == previous_line || comment_line == token_line {
                CommentType::InlineBlock
            } else {
                CommentType::Block
            };

            let source_comment = SourceComment {
                text,
                ty,
                offset: tokens.offset(comment) as usize,
                flush_left,
            };

            comments.add(lines_before, source_comment);

            previous_line = self.end_line(comment);
        }

        comments.set_lines_before_next_token(token_line - previous_line);
        comments
    }

    /// Whether there are any newlines between [from] and [to].
    pub fn has_newline_between(&self, from: TokenId, to: TokenId) -> bool {
        self.end_line(from) < self.start_line(to)
    }

    /// Dart `LineInfo.getLocation(offset)` for a Dart `int` offset: the
    /// synthetic token before the first token has offset -1.
    fn location(&self, offset: u32) -> (i32, i32) {
        if (offset as i32) < 0 {
            // Dart: line 1, column `offset - lineStarts[0] + 1`.
            return (1, offset as i32 + 1);
        }
        let location = self.line_info.get_location(offset);
        (location.line_number as i32, location.column_number as i32)
    }

    /// Gets the 1-based line number that the beginning of [token] lies on.
    fn start_line(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.offset(token)).0
    }

    /// Gets the 1-based line number that the end of [token] lies on.
    fn end_line(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.get(token).end()).0
    }

    /// Gets the 1-based column number that the beginning of [token] lies on.
    fn start_column(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.offset(token)).1
    }
}

/// Dart `String.trim()`: removes leading and trailing whitespace as defined
/// by Dart (Unicode White_Space and the BOM).
pub fn dart_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
}

/// A comment in the source, with a bit of information about the surrounding
/// whitespace.
#[derive(Clone, Copy, Debug)]
pub struct SourceComment<'a> {
    /// The text of the comment, including `//`, `/*`, and `*/`.
    pub text: &'a str,

    pub ty: CommentType,

    /// Whether this comment starts at column one in the source.
    ///
    /// Comments that start at the start of the line will not be indented in
    /// the output. This way, commented out chunks of code do not get
    /// erroneously re-indented.
    pub flush_left: bool,

    /// The number of code points in the original source code preceding the
    /// start of this comment.
    ///
    /// Used to track selection markers within the comment.
    pub offset: usize,
}

impl SourceComment<'_> {
    /// Whether this comment ends with a mandatory newline, because it's a
    /// line comment or a block comment that should be on its own line.
    pub fn requires_newline(&self) -> bool {
        self.ty != CommentType::InlineBlock
    }
}

/// A list of source code comments and the number of newlines between them,
/// as well as the number of newlines before the first comment and after the
/// last comment.
///
/// If there are no comments, this just tracks the number of newlines between
/// a pair of tokens.
///
/// This class is not simply a list of "comment + newline" pairs because we
/// want to know the number of newlines before the first comment and after
/// the last. That means there is always one more newline count that there
/// are comments, including the degenerate case where there are no comments
/// but one newline count.
///
/// For example, this code:
///
///     a /* c1 */
///     /* c2 */
///
///     /* c3 */
///
///
///     b
///
/// Produces a sequence like:
///
/// * 0 newlines between `a` and `/* c1 */`
/// * Comment `/* c1 */`
/// * 1 newline between `/* c1 */` and `/* c2 */`
/// * Comment `/* c2 */`
/// * 2 newlines between `/* c2 */` and `/* c3 */`
/// * Comment `/* c3 */`
/// * 3 newlines between `/* c3 */` and `b`
#[derive(Clone, Debug)]
pub struct CommentSequence<'a> {
    /// The number of newlines between a pair of comments or the preceding or
    /// following tokens.
    ///
    /// This list is always one element longer than [comments].
    lines_between: Vec<i32>,

    comments: Vec<SourceComment<'a>>,
}

impl<'a> CommentSequence<'a> {
    /// Dart `CommentSequence.empty`.
    pub fn empty() -> CommentSequence<'a> {
        CommentSequence {
            lines_between: vec![0],
            comments: Vec::new(),
        }
    }

    /// Whether this sequence contains any comments that require a newline.
    pub fn requires_newline(&self) -> bool {
        self.comments.iter().any(|comment| comment.requires_newline())
    }

    /// The number of newlines between the comment at [comment_index] and the
    /// preceding comment or token.
    pub fn lines_before(&self, comment_index: usize) -> i32 {
        self.lines_between[comment_index]
    }

    /// The number of newlines between the comment at [comment_index] and the
    /// following comment or token.
    pub fn lines_after(&self, comment_index: usize) -> i32 {
        self.lines_between[comment_index + 1]
    }

    /// Whether the comment at [comment_index] should be attached to the
    /// preceding token.
    pub fn is_hanging(&self, comment_index: usize) -> bool {
        // Don't move a comment to a preceding line.
        if self.lines_before(comment_index) != 0 {
            return false;
        }

        // Doc comments and non-inline `/* ... */` comments are always pushed
        // to the next line. Only inline block comments and line comments are
        // allowed to hang at the end of a line.
        let ty = self.comments[comment_index].ty;
        ty == CommentType::InlineBlock || ty == CommentType::Line
    }

    /// Whether the comment at [comment_index] should be attached to the
    /// following token.
    pub fn is_leading(&self, comment_index: usize) -> bool {
        // Don't move code on the next line up to the comment.
        if self.lines_after(comment_index) > 0 {
            return false;
        }

        // Doc comments and non-inline `/* ... */` comments are always pushed
        // to the next line.
        self.comments[comment_index].ty == CommentType::InlineBlock
    }

    /// The number of newlines between the last comment and the next token.
    ///
    /// If there are no comments, this is the number of lines between the
    /// next token and the preceding one.
    pub fn lines_before_next_token(&self) -> i32 {
        *self.lines_between.last().unwrap()
    }

    /// Whether there are any blank lines before or after any of the comments
    /// in this sequence in the original source code.
    pub fn contains_blank_line(&self) -> bool {
        for i in 0..self.len() {
            if self.lines_before(i) > 1 {
                return true;
            }
        }

        self.lines_before_next_token() > 1
    }

    /// The number of comments in the sequence.
    pub fn len(&self) -> usize {
        self.comments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    /// The comment at [index].
    pub fn get(&self, index: usize) -> &SourceComment<'a> {
        &self.comments[index]
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceComment<'a>> {
        self.comments.iter()
    }

    fn add(&mut self, lines_before: i32, comment: SourceComment<'a>) {
        self.lines_between.push(lines_before);
        self.comments.push(comment);
    }

    /// Records the number of lines between the end of the last comment and
    /// the beginning of the next token.
    fn set_lines_before_next_token(&mut self, lines_after: i32) {
        self.lines_between.push(lines_after);
    }

    /// Creates a new sequence that is this sequence followed by [other].
    ///
    /// Sums the trailing newline of the left sequence and the leading newline
    /// of the right sequence.
    pub fn concatenate(self, other: CommentSequence<'a>) -> CommentSequence<'a> {
        // Don't allocate new sequences if we don't need to.
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }

        let mut lines_between = Vec::with_capacity(self.lines_between.len() + other.lines_between.len() - 1);
        // Include all of the newlines from the left sequence, except the
        // last.
        lines_between.extend_from_slice(&self.lines_between[..self.lines_between.len() - 1]);
        // Combine the trailing newline of the left sequence and the leading
        // newline of the right sequence.
        lines_between.push(self.lines_between[self.lines_between.len() - 1] + other.lines_between[0]);
        // Include the remaining newlines of the right sequence.
        lines_between.extend_from_slice(&other.lines_between[1..]);

        let mut comments = self.comments;
        comments.extend(other.comments);

        CommentSequence {
            lines_between,
            comments,
        }
    }

    /// Splits this sequence into two subsequences where [index] indicates the
    /// number of comments in the first returned sequence and the second
    /// sequence gets the rest.
    ///
    /// The newline count right at the split point goes to the first sequence
    /// and the second sequence gets an initial newline count of zero.
    pub fn split_at(self, index: usize) -> (CommentSequence<'a>, CommentSequence<'a>) {
        // Don't allocate new sequences if we don't have to.
        if index == 0 {
            return (CommentSequence::empty(), self);
        }
        if index == self.len() {
            return (self, CommentSequence::empty());
        }

        let mut second_lines = vec![0];
        // 0 is the synthesized newline count before the first comment.
        second_lines.extend_from_slice(&self.lines_between[index + 1..]);
        let second = CommentSequence {
            lines_between: second_lines,
            comments: self.comments[index..].to_vec(),
        };
        let first = CommentSequence {
            // +1 to include the newline after the last comment.
            lines_between: self.lines_between[..index + 1].to_vec(),
            comments: self.comments[..index].to_vec(),
        };
        (first, second)
    }
}
