// Dart source: dart_style lib/src/short/chunk.dart
// Dart source: dart_style lib/src/short/selection.dart

use std::cell::Cell;

use crate::text::utf16_len;

use super::arena::{Arena, ChunkId, NestingId, RuleId, SpanId};

/// A chunk of non-breaking output text that may begin on a newline.
///
/// Chunks are created by [ChunkBuilder] and fed into [LineSplitter]. Each
/// contains the data describing where the chunk should appear when starting a
/// new line, how desireable it is to split, and the subsequent text for that
/// line.
///
/// Line splitting before a chunk comes in a few different forms.
///
/// *   A "hard" split is a mandatory newline. The formatted output will contain
///     at least one newline before the chunk's text.
/// *   A "soft" split is a discretionary newline. If a line doesn't fit within
///     the page width, one or more soft splits may be turned into newlines to
///     wrap the line to fit within the bounds. If a soft split is not turned
///     into a newline, it may instead appear as a space or zero-length string
///     in the output, depending on [space_when_unsplit].
/// *   A "double" split expands to two newlines. In other words, it leaves a
///     blank line in the output. Hard or soft splits may be doubled. This is
///     determined by [is_double].
///
/// A split controls the leading spacing of the line before the chunk's text,
/// both block-based [indent] and expression-wrapping-based [nesting].
///
/// A Dart `BlockChunk` is a chunk with [block] data.
pub struct Chunk {
    /// The literal text output for the chunk.
    text: String,

    /// The length of [text] in UTF-16 code units.
    text_length: i32,

    /// The number of characters of indentation from the left edge of the block
    /// that contains this chunk.
    ///
    /// For top level chunks that are not inside any block, this also includes
    /// leading indentation.
    pub indent: i32,

    /// The expression nesting level preceding this chunk.
    ///
    /// This is used to determine how much to increase the indentation when a
    /// line starts at this chunk. A single statement may be indented multiple
    /// times if the splits occur in more deeply nested expressions, for example:
    ///
    ///     // 40 columns                           |
    ///     someFunctionName(argument, argument,
    ///         argument, anotherFunction(argument,
    ///             argument));
    pub nesting: NestingId,

    /// The [Rule] that controls when a split should occur before this chunk.
    ///
    /// Multiple splits may share a [Rule].
    pub rule: RuleId,

    /// Whether or not an extra blank line should be output before this chunk if
    /// it splits.
    is_double: bool,

    /// If `true`, then the line beginning with this chunk should always be at
    /// column zero regardless of any indentation or expression nesting.
    ///
    /// Used for multi-line strings and commented out code.
    flush_left: bool,

    /// Whether this chunk should prepend an extra space if it does not split.
    ///
    /// This is `true`, for example, in a chunk following a ",".
    space_when_unsplit: bool,

    /// Whether this chunk marks the end of a range of chunks that can be line
    /// split independently of the following chunks.
    can_divide: bool,

    /// The [Span]s that contain this chunk.
    pub spans: Vec<SpanId>,

    /// Dart `Selection.selectionStart`: the offset from the beginning of
    /// [text] where the selection starts, or `None` if the selection does not
    /// start within this chunk.
    pub selection_start: Option<i32>,

    /// Dart `Selection.selectionEnd`.
    pub selection_end: Option<i32>,

    /// The data of a Dart `BlockChunk`.
    pub block: Option<Box<BlockChunk>>,
}

/// A [Chunk] containing a list of nested "child" chunks that are formatted
/// independently of the surrounding chunks.
///
/// This is used for blocks, function expressions, collection literals, etc.
/// Basically, anywhere we have a delimited body of code whose formatting
/// doesn't depend on how the surrounding code is formatted except to determine
/// indentation.
///
/// This chunk's own text is the closing delimiter of the block, so its
/// children come before itself. For example, given this code:
///
///     main() {
///       var list = [
///         element,
///       ];
///     }
///
/// It is organized into a tree of chunks like so:
///
///    - Chunk           "main() {"
///    - BlockChunk
///      |- Chunk          "var list = ["
///      |- BlockChunk
///      |  |- Chunk         "element,"
///      |  '- (text)      "];"
///      '- (text)       "}"
pub struct BlockChunk {
    /// If this block is for a collection literal in an argument list, this will
    /// be the chunk preceding this literal argument.
    ///
    /// That chunk is owned by the argument list and if it splits, this collection
    /// may need extra expression-level indentation.
    pub argument: Option<ChunkId>,

    /// The child chunks in this block.
    pub children: Vec<ChunkId>,

    /// Cache of [Arena::unsplit_block_length] (the children do not change
    /// once the block is complete).
    unsplit_block_length: Cell<Option<i32>>,
}

impl Chunk {
    /// Creates a new empty chunk with the given split properties.
    pub fn new(
        rule: RuleId,
        indent: i32,
        nesting: NestingId,
        space: bool,
        flush_left: bool,
        is_double: bool,
    ) -> Chunk {
        Chunk {
            text: String::new(),
            text_length: 0,
            indent,
            nesting,
            rule,
            is_double,
            flush_left,
            space_when_unsplit: space,
            can_divide: true,
            spans: Vec::new(),
            selection_start: None,
            selection_end: None,
            block: None,
        }
    }

    /// Dart `BlockChunk(argument, rule, indent, nesting, space:, flushLeft:)`.
    pub fn new_block(
        argument: Option<ChunkId>,
        rule: RuleId,
        indent: i32,
        nesting: NestingId,
        space: bool,
        flush_left: bool,
    ) -> Chunk {
        let mut chunk = Chunk::new(rule, indent, nesting, space, flush_left, false);
        chunk.block = Some(Box::new(BlockChunk {
            argument,
            children: Vec::new(),
            unsplit_block_length: Cell::new(None),
        }));
        chunk
    }

    /// The literal text output for the chunk.
    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The length of [text] in UTF-16 code units (Dart `text.length`).
    #[inline]
    pub fn text_length(&self) -> i32 {
        self.text_length
    }

    #[inline]
    pub fn is_double(&self) -> bool {
        self.is_double
    }

    #[inline]
    pub fn flush_left(&self) -> bool {
        self.flush_left
    }

    #[inline]
    pub fn space_when_unsplit(&self) -> bool {
        self.space_when_unsplit
    }

    #[inline]
    pub fn can_divide(&self) -> bool {
        self.can_divide
    }

    /// Whether this is a Dart `BlockChunk`.
    #[inline]
    pub fn is_block(&self) -> bool {
        self.block.is_some()
    }

    /// The number of characters in this chunk when unsplit.
    #[inline]
    pub fn length(&self) -> i32 {
        (if self.space_when_unsplit { 1 } else { 0 }) + self.text_length
    }

    /// Append [text] to the end of the chunk's text.
    pub fn append_text(&mut self, text: &str) {
        self.text.push_str(text);
        self.text_length += utf16_len(text) as i32;
    }

    /// Updates the split information for a previously created chunk in response
    /// to a split from a comment.
    pub fn update_split(&mut self, flush_left: Option<bool>, is_double: bool, space: Option<bool>) {
        debug_assert!(self.text.is_empty());

        if let Some(flush_left) = flush_left {
            self.flush_left = flush_left;
        }

        // Don't discard an already known blank newline, but do potentially add one.
        if is_double {
            self.is_double = true;
        }

        if let Some(space) = space {
            self.space_when_unsplit = space;
        }
    }

    /// Prevent the line splitter from diving at this chunk.
    ///
    /// This should be called on any chunk where line splitting choices before
    /// and after this chunk relate to each other.
    pub fn prevent_divide(&mut self) {
        self.can_divide = false;
    }

    /// Sets [selection_start] to be [start] code units into [text].
    pub fn start_selection(&mut self, start: i32) {
        self.selection_start = Some(start);
    }

    /// Sets [selection_start] to be [from_end] code units from the end of
    /// [text].
    pub fn start_selection_from_end(&mut self, from_end: i32) {
        self.selection_start = Some(self.text_length - from_end);
    }

    /// Sets [selection_end] to be [end] code units into [text].
    pub fn end_selection(&mut self, end: i32) {
        self.selection_end = Some(end);
    }

    /// Sets [selection_end] to be [from_end] code units from the end of
    /// [text].
    pub fn end_selection_from_end(&mut self, from_end: i32) {
        self.selection_end = Some(self.text_length - from_end);
    }
}

impl Arena {
    /// Dart `Chunk.dummy()`: creates a dummy chunk.
    ///
    /// This is returned in some places by [ChunkBuilder] when there is no useful
    /// chunk to yield and it will not end up being used by the caller anyway.
    pub fn dummy_chunk(&mut self) -> ChunkId {
        let nesting = self.new_nesting_level();
        let mut chunk = Chunk::new(Arena::DUMMY_RULE, 0, nesting, false, false, false);
        chunk.append_text("(dummy)");
        self.add_chunk(chunk)
    }

    /// Dart `Chunk.unsplitBlockLength`: the unsplit length of all of this
    /// chunk's block contents.
    ///
    /// Does not include this chunk's own length, just the length of its child
    /// block chunks (recursively).
    pub fn unsplit_block_length(&self, chunk: ChunkId) -> i32 {
        let Some(block) = &self.chunk(chunk).block else {
            return 0;
        };
        if let Some(length) = block.unsplit_block_length.get() {
            return length;
        }

        let mut length = 0;
        for &child in &block.children {
            length += self.chunk(child).length() + self.unsplit_block_length(child);
        }

        block.unsplit_block_length.set(Some(length));
        length
    }

    /// Dart `Chunk.indentBlock`: returns `true` if this chunk is a block whose
    /// children should be expression indented given a set of rule values
    /// provided by [get_value].
    ///
    /// [get_value] takes a [Rule] and returns the chosen split state value for
    /// that [Rule].
    pub fn indent_block(&self, chunk: ChunkId, get_value: impl Fn(RuleId) -> i32) -> bool {
        let Some(block) = &self.chunk(chunk).block else {
            return false;
        };
        let Some(argument) = block.argument else {
            return false;
        };

        // There may be no rule if the block occurs inside a string interpolation.
        // In that case, it's not clear if anything will look particularly nice, but
        // expression nesting is probably marginally better.
        let rule = self.chunk(argument).rule;
        if rule == Arena::DUMMY_RULE {
            return true;
        }

        self.rule(rule).is_split(get_value(rule), argument)
    }
}

/// The in-progress state for a [Span] that has been started but has not yet
/// been completed.
#[derive(Clone, Copy, Debug)]
pub struct OpenSpan {
    /// Index of the first chunk contained in this span.
    pub start: usize,

    /// The cost applied when the span is split across multiple lines or `null`
    /// if the span is for a multisplit.
    pub cost: i32,
}

/// Delimits a range of chunks that must end up on the same line to avoid an
/// additional cost.
///
/// These are used to encourage the line splitter to try to keep things
/// together, like parameter lists and binary operator expressions.
///
/// This is a wrapper around the cost so that spans have unique identities.
/// This way we can correctly avoid paying the cost multiple times if the same
/// span is split by multiple chunks.
///
/// Spans can be marked during processing in an algorithm but should be left
/// unmarked when the algorithm finishes to make marking work in subsequent
/// calls.
pub struct Span {
    /// The cost applied when the span is split across multiple lines or `null`
    /// if the span is for a multisplit.
    pub cost: i32,

    pub is_marked: bool,
}
