// Dart source: dart_style lib/src/short/chunk_builder.dart

use crate::comment_type::CommentType;
use crate::constants::Cost;
use crate::source_code::SourceCode;

use super::arena::{Arena, ChunkId, NestingId, RuleId};
use super::chunk::{Chunk, OpenSpan};
use super::line_writer::{LineWriter, WriterContext};
use super::nesting_builder::NestingBuilder;
use super::source_comment::SourceComment;

/// Dart `RegExp(r'[a-zA-Z0-9_]$')`: whether the last character of a string
/// is an identifier character.
fn has_trailing_identifier_char(text: &str) -> bool {
    text.as_bytes()
        .last()
        .is_some_and(|&c| c.is_ascii_alphanumeric() || c == b'_')
}

/// The state of one Dart `ChunkBuilder`: the builder for the top-level code
/// or for the contents of one block.
struct BuilderLevel {
    /// The chunks of this builder. For a block, they become the children of
    /// [block_chunk] when the block ends.
    chunks: Vec<ChunkId>,

    /// The block chunk that owns [chunks], or `None` for the top level.
    block_chunk: Option<ChunkId>,

    /// The number of newlines that should be written before the next
    /// non-whitespace token.
    ///
    /// This will always be 0, 1, or 2.
    pending_newlines: i32,

    /// Whether a non-breaking space should be written before the next text.
    pending_space: bool,

    /// Whether the next chunk should use expression nesting.
    pending_nested: bool,

    /// Whether the next chunk should be flush left.
    pending_flush_left: bool,

    /// Whether the most recently written output was a comment.
    after_comment: bool,

    /// The nested stack of rules that are currently in use.
    ///
    /// New chunks are implicitly split by the innermost rule when the chunk is
    /// ended.
    rules: Vec<RuleId>,

    /// The set of rules known to contain hard splits that will in turn force
    /// these rules to harden.
    ///
    /// This is accumulated lazily while chunks are being built. Then, once they
    /// are all done, the rules are all hardened. We do this later because some
    /// rules may not have all of their constraints fully wired up until after
    /// the hard split appears. For example, a hard split in a positional
    /// argument list needs to force the named arguments to split too, but we
    /// don't create that rule until after the positional arguments are done.
    hard_split_rules: Vec<RuleId>,

    /// The list of rules that are waiting until the next whitespace has been
    /// written before they start.
    lazy_rules: Vec<RuleId>,

    /// The nested stack of spans that are currently being written.
    open_spans: Vec<OpenSpan>,

    /// The current state.
    nesting: NestingBuilder,

    /// The stack of nesting levels where block arguments may start.
    ///
    /// A block argument's contents will nest at the last level in this stack.
    block_argument_nesting: Vec<NestingId>,

    /// The number of calls to [prevent_split()] that have not been ended by a
    /// call to [end_prevent_split()].
    ///
    /// Splitting is completely disabled inside string interpolation. We do want
    /// to fix the whitespace inside interpolation, though, so we still format
    /// them. This tracks whether we're inside an interpolation. We can't use a
    /// simple bool because interpolation can nest.
    ///
    /// When this is non-zero, splits are ignored.
    prevent_split_nesting: i32,
}

impl BuilderLevel {
    fn new(arena: &mut Arena, block_chunk: Option<ChunkId>) -> BuilderLevel {
        BuilderLevel {
            chunks: Vec::new(),
            block_chunk,
            pending_newlines: 0,
            pending_space: false,
            pending_nested: false,
            pending_flush_left: false,
            after_comment: false,
            rules: Vec::new(),
            hard_split_rules: Vec::new(),
            lazy_rules: Vec::new(),
            open_spans: Vec::new(),
            nesting: NestingBuilder::new(arena),
            block_argument_nesting: Vec::new(),
            prevent_split_nesting: 0,
        }
    }
}

/// Takes the incremental serialized output of [SourceVisitor]--the source text
/// along with any comments and preserved whitespace--and produces a coherent
/// tree of [Chunk]s which can then be split into physical lines.
///
/// Keeps track of leading indentation, expression nesting, and all of the hairy
/// code required to seamlessly integrate existing comments into the pure
/// output produced by [SourceVisitor].
///
/// Dart has one `ChunkBuilder` per block; [start_block] returns the builder
/// for the block and [end_block] returns the parent builder. Here, one
/// [ChunkBuilder] keeps the stack of those builders and every method acts on
/// the innermost one.
pub struct ChunkBuilder {
    /// The number of characters of code that can fit in a single line.
    page_width: i32,

    pub arena: Arena,

    levels: Vec<BuilderLevel>,
}

impl ChunkBuilder {
    pub fn new(page_width: i32, indent: i32) -> ChunkBuilder {
        let mut arena = Arena::new();
        let level = BuilderLevel::new(&mut arena, None);
        let mut builder = ChunkBuilder {
            page_width,
            arena,
            levels: vec![level],
        };
        builder.indent(Some(indent));
        builder.start_block_argument_nesting();
        builder
    }

    #[inline]
    fn level(&mut self) -> &mut BuilderLevel {
        self.levels.last_mut().unwrap()
    }

    #[inline]
    fn level_ref(&self) -> &BuilderLevel {
        self.levels.last().unwrap()
    }

    /// The number of characters of code that can fit in a single line.
    pub fn page_width(&self) -> i32 {
        self.page_width
    }

    /// The current innermost rule.
    pub fn rule(&self) -> RuleId {
        *self.level_ref().rules.last().unwrap()
    }

    /// Writes [string], the text for a single token, to the output.
    ///
    /// By default, this also implicitly adds one level of nesting if we aren't
    /// currently nested at all. We do this here so that if a comment appears
    /// after any token within a statement or top-level form and that comment
    /// leads to splitting, we correctly nest. Even pathological cases like:
    ///
    ///
    ///     import // comment
    ///         "this_gets_nested.dart";
    ///
    /// If we didn't do this here, we'd have to call [nest_expression] after the
    /// first token of practically every grammar production.
    ///
    /// If [merge_empty_splits] is `true`, the default, then any pending split
    /// information will be combined with a previously created split if no text
    /// has been written since. This generally comes into play when text is
    /// written after a comment. The comment may leave some pending split
    /// information while [SourceVisitor] may have also created a split and we
    /// want to combine those.
    ///
    /// It is only `false` when writing the contents of a multiline string. There,
    /// we may *want* to have a series of empty chunks because those represent
    /// empty lines in the multiline string.
    pub fn write(&mut self, string: &str, merge_empty_splits: bool) {
        self.emit_pending_whitespace(false, merge_empty_splits);
        self.write_text(string, None);

        self.activate_lazy_rules();

        let level = self.level();
        level.nesting.commit_nesting();
        level.after_comment = false;
    }

    /// Dart `_lazyRules.forEach(_activateRule); _lazyRules.clear();`.
    fn activate_lazy_rules(&mut self) {
        if self.level_ref().lazy_rules.is_empty() {
            return;
        }
        let lazy_rules = std::mem::take(&mut self.level().lazy_rules);
        for &rule in &lazy_rules {
            self.activate_rule(rule);
        }
        let mut lazy_rules = lazy_rules;
        lazy_rules.clear();
        self.level().lazy_rules = lazy_rules;
    }

    /// Writes one or two hard newlines.
    ///
    /// Doesn't immediately write them. That way line breaking is correctly
    /// interleaved with any comments that appear before the next token.
    ///
    /// If [is_double] is `true`, inserts an extra blank line. If [flush_left] is
    /// `true`, the next line will start at column 1 and ignore indentation and
    /// nesting. If [nest] is `true` then the next line will use expression
    /// nesting.
    pub fn write_newline(&mut self, is_double: bool, flush_left: bool, nest: bool) {
        let level = self.level();
        level.pending_newlines = if is_double { 2 } else { 1 };
        level.pending_flush_left = flush_left;
        level.pending_nested = nest;
    }

    /// Writes a space before the subsequent non-whitespace text.
    pub fn write_space(&mut self) {
        self.level().pending_space = true;
    }

    /// Write a split owned by the current innermost rule.
    ///
    /// If [nest] is `false`, ignores any current expression nesting. Otherwise,
    /// uses the current nesting level. If unsplit, it expands to a space if
    /// [space] is `true`.
    pub fn split(&mut self, nest: bool, space: bool) -> ChunkId {
        // If we are not allowed to split at all, don't. Returning null for the
        // chunk is safe since the rule that uses the chunk will itself get
        // discarded because no chunk references it.
        let level = self.level();
        if level.prevent_split_nesting > 0 {
            level.pending_newlines = 0;
            level.pending_nested = false;

            if space {
                level.pending_space = true;
            }
            return self.arena.dummy_chunk();
        }

        // If a hard split after a comment is already pending, then prefer that over
        // a soft split.
        if level.pending_newlines > 0 {
            return self.arena.dummy_chunk();
        }

        self.write_split(false, false, nest, space, true)
    }

    /// Outputs the series of [comments] and associated whitespace that appear
    /// before [token] (which is not written by this).
    ///
    /// The list contains each comment as it appeared in the source between the
    /// last token written and the next one that's about to be written.
    ///
    /// [lines_before_token] is the number of lines between the last comment (or
    /// previous token if there are no comments) and the next token.
    pub fn write_comments(
        &mut self,
        mut comments: Vec<SourceComment>,
        lines_before_token: i32,
        token: &str,
    ) -> i32 {
        // Edge case: if we require a blank line, but there exists one between
        // some of the comments, or after the last one, then we don't need to
        // enforce one before the first comment. Example:
        //
        //     library foo;
        //     // comment
        //
        //     class Bar {}
        //
        // Normally, a blank line is required after `library`, but since there is
        // one after the comment, we don't need one before it. This is mainly so
        // that commented out directives stick with their preceding group.
        if self.level_ref().pending_newlines == 2 && comments[0].lines_before < 2 {
            if lines_before_token > 1 {
                self.write_newline(false, false, false);
            } else {
                for comment in comments.iter().skip(1) {
                    if comment.lines_before > 1 {
                        self.write_newline(false, false, false);
                        break;
                    }
                }
            }
        }

        // Edge case: if the comments are completely inline (i.e. just a series of
        // block comments with no newlines before, after, or between them), then
        // they will eat any pending newlines. Make sure that doesn't happen by
        // putting the pending whitespace before the first comment. Turns this:
        //
        //     library foo; /* a */ /* b */ import 'a.dart';
        //
        // into:
        //
        //     library foo;
        //
        //     /* a */ /* b */ import 'a.dart';
        let pending_newlines = self.level_ref().pending_newlines;
        if lines_before_token == 0
            && pending_newlines > comments[0].lines_before
            && comments
                .iter()
                .all(|comment| comment.type_ == CommentType::InlineBlock)
        {
            comments[0].lines_before = pending_newlines;
        }

        // Write each comment and the whitespace between them.
        for i in 0..comments.len() {
            let comment = &comments[i];

            // See if the comment should follow text on the current line.
            let chunk = self.chunk_for_comment(comment, token);
            if let Some(chunk) = chunk {
                // The comment follows other text, so decide if it gets a space before
                // it.
                let pending_space = self.needs_space_before_comment(comment, chunk);
                let level = self.levels.last_mut().unwrap();
                level.pending_space = pending_space;
                let last = *level.chunks.last().unwrap();
                if pending_space && chunk != last {
                    // We've already created a split after the comment, so if it doesn't
                    // split, it should get a space.
                    self.arena.chunk_mut(last).update_split(None, false, Some(true));
                }
            } else {
                // Split before the comment if it starts a line.
                let level = self.level_ref();
                if level.pending_newlines == 0 {
                    if comment.lines_before > 0
                        && (level.after_comment || comment.type_ != CommentType::InlineBlock)
                    {
                        let is_double = self.needs_blank_line_before_comment(comment);
                        self.write_newline(is_double, comment.flush_left, true);
                    } else if let Some(&last) = level.chunks.last() {
                        let pending_space = self.needs_space_before_comment(comment, last);
                        self.level().pending_space = pending_space;
                    }
                } else {
                    let flush_left = comment.flush_left;
                    self.level().pending_flush_left = flush_left;
                }

                let is_double = self.needs_blank_line_before_comment(comment);
                self.emit_pending_whitespace(is_double, true);
            }

            let comment = &comments[i];
            self.write_text(&comment.text, chunk);

            if let Some(start) = comment.selection_start {
                self.start_selection_from_end(comment.text_length - start);
            }

            if let Some(end) = comment.selection_end {
                self.end_selection_from_end(comment.text_length - end);
            }

            // Make sure there is at least one newline after a line comment and allow
            // one or two after a block comment that has nothing after it.
            let lines_after = if i < comments.len() - 1 {
                comments[i + 1].lines_before
            } else {
                let mut lines_after = lines_before_token;

                // Always force a newline after multi-line block comments. Prevents
                // mistakes like:
                //
                //     /**
                //      * Some doc comment.
                //      */ someFunction() { ... }
                if lines_after == 0 && comments.last().unwrap().text.contains('\n') {
                    lines_after = 1;
                }
                lines_after
            };

            if lines_after > 0 {
                let is_double = self.level_ref().pending_newlines == 2 || lines_after > 1;
                self.write_newline(is_double, false, true);
            }
        }

        // If the comment has text following it (aside from a grouping character),
        // it needs a trailing space.
        let pending_space = self.needs_space_after_comment(comments.last().unwrap(), token);
        let level = self.level();
        level.pending_space = pending_space;
        level.after_comment = true;

        // Dart reads `comments.first.linesBefore` after this call.
        comments[0].lines_before
    }

    /// Creates a new indentation level [spaces] deeper than the current one.
    ///
    /// If omitted, [spaces] defaults to [Indent.block].
    pub fn indent(&mut self, spaces: Option<i32>) {
        self.level().nesting.indent(spaces);
    }

    /// Discards the most recent indentation level.
    pub fn unindent(&mut self) {
        self.level().nesting.unindent();
    }

    /// Starts a new span with [cost].
    ///
    /// Each call to this needs a later matching call to [end_span].
    pub fn start_span(&mut self, cost: i32) {
        let level = self.level();
        let start = level.chunks.len();
        level.open_spans.push(OpenSpan { start, cost });
    }

    /// Dart `startSpan()` with the default cost.
    pub fn start_span_normal(&mut self) {
        self.start_span(Cost::NORMAL);
    }

    /// Ends the innermost span.
    pub fn end_span(&mut self) {
        let level = self.levels.last_mut().unwrap();
        let open_span = level.open_spans.pop().unwrap();

        // A span that just covers a single chunk can't be split anyway.
        let end = level.chunks.len();
        if open_span.start == end {
            return;
        }

        // Add the span to every chunk that can split it.
        let span = self.arena.add_span(open_span.cost);
        for i in open_span.start..end {
            let chunk = level.chunks[i];
            let rule = self.arena.chunk(chunk).rule;
            if !self.arena.rule(rule).is_hardened() {
                self.arena.chunk_mut(chunk).spans.push(span);
            }
        }
    }

    /// Starts a new [Rule].
    ///
    /// If omitted, defaults to a new [Rule].
    pub fn start_rule(&mut self, rule: Option<RuleId>) -> RuleId {
        let rule = rule.unwrap_or_else(|| self.arena.new_rule());

        // If there are any pending lazy rules, start them now so that the proper
        // stack ordering of rules is maintained.
        self.activate_lazy_rules();

        self.activate_rule(rule);
        rule
    }

    fn activate_rule(&mut self, rule: RuleId) {
        // See if any of the rules that contain this one care if it splits.
        let level = self.levels.last_mut().unwrap();
        for &outer in &level.rules {
            if !self.arena.rule(outer).splits_on_inner_rules() {
                continue;
            }
            self.arena.rule_mut(rule).constrain_when_split(outer);
        }
        level.rules.push(rule);
    }

    /// Starts a new [Rule] that comes into play *after* the next whitespace
    /// (including comments) is written.
    ///
    /// This is used for operators who want to start a rule before the first
    /// operand but not get forced to split if a comment appears before the
    /// entire expression.
    ///
    /// If [rule] is omitted, defaults to a new [Rule].
    pub fn start_lazy_rule(&mut self, rule: Option<RuleId>) -> RuleId {
        let rule = rule.unwrap_or_else(|| self.arena.new_rule());

        self.level().lazy_rules.push(rule);
        rule
    }

    /// Ends the innermost rule.
    pub fn end_rule(&mut self) {
        let level = self.level();
        if !level.lazy_rules.is_empty() {
            level.lazy_rules.pop();
        } else {
            level.rules.pop();
        }
    }

    /// Pre-emptively forces all of the current rules to become hard splits.
    ///
    /// This is called by [SourceVisitor] when it can determine that a rule will
    /// will always be split. Turning it (and the surrounding rules) into hard
    /// splits lets the writer break the output into smaller pieces for the line
    /// splitter, which helps performance and avoids failing on very large input.
    ///
    /// In particular, it's easy for the visitor to know that collections with a
    /// large number of items must split. Doing that early avoids crashing the
    /// splitter when it tries to recurse on huge collection literals.
    pub fn force_rules(&mut self) {
        self.handle_hard_split();
    }

    /// Begins a new expression nesting level [indent] spaces deeper than the
    /// current one if it splits.
    ///
    /// If [indent] is omitted, defaults to [Indent.expression]. If [now] is
    /// `true`, commits the nesting change immediately instead of waiting until
    /// after the next chunk of text is written.
    pub fn nest_expression(&mut self, indent: Option<i32>, now: bool) {
        let level = self.levels.last_mut().unwrap();
        level.nesting.nest(&mut self.arena, indent);
        if now {
            level.nesting.commit_nesting();
        }
    }

    /// Dart `nestExpression()` with the defaults.
    pub fn nest(&mut self) {
        self.nest_expression(None, false);
    }

    /// Discards the most recent level of expression nesting.
    ///
    /// Expressions that are more nested will get increased indentation when split
    /// if the previous line has a lower level of nesting.
    ///
    /// If [now] is `false`, does not commit the nesting change until after the
    /// next chunk of text is written.
    pub fn unnest_now(&mut self, now: bool) {
        let level = self.levels.last_mut().unwrap();
        level.nesting.unnest(&self.arena);
        if now {
            level.nesting.commit_nesting();
        }
    }

    /// Dart `unnest()` (commits the change immediately).
    pub fn unnest(&mut self) {
        self.unnest_now(true);
    }

    /// Marks the selection starting point as occurring [from_end] characters to
    /// the left of the end of what's currently been written.
    ///
    /// It counts backwards from the end because this is called *after* the chunk
    /// of text containing the selection has been output.
    pub fn start_selection_from_end(&mut self, from_end: i32) {
        let last = *self.level_ref().chunks.last().unwrap();
        self.arena.chunk_mut(last).start_selection_from_end(from_end);
    }

    /// Marks the selection ending point as occurring [from_end] characters to the
    /// left of the end of what's currently been written.
    ///
    /// It counts backwards from the end because this is called *after* the chunk
    /// of text containing the selection has been output.
    pub fn end_selection_from_end(&mut self, from_end: i32) {
        let chunks = &self.levels.last().unwrap().chunks;
        let last = *chunks.last().unwrap();

        // If the selection marker is right on a split, then put it before the
        // newline.
        if !self.arena.chunk(last).text().is_empty() {
            self.arena.chunk_mut(last).end_selection_from_end(from_end);
        } else {
            let previous = chunks[chunks.len() - 2];
            self.arena
                .chunk_mut(previous)
                .end_selection_from_end(from_end);
        }
    }

    /// Captures the current nesting level as marking where subsequent block
    /// arguments should start.
    pub fn start_block_argument_nesting(&mut self) {
        let level = self.level();
        let nesting = level.nesting.current_nesting();
        level.block_argument_nesting.push(nesting);
    }

    /// Releases the last nesting level captured by [start_block_argument_nesting].
    pub fn end_block_argument_nesting(&mut self) {
        self.level().block_argument_nesting.pop();
    }

    /// Starts a new block chunk and makes the builder for it the current one.
    ///
    /// Nested blocks are handled using their own independent [LineWriter].
    pub fn start_block(&mut self, argument_chunk: Option<ChunkId>, indent: bool, space: bool) {
        // Start a block chunk for the block. It will contain the chunks for the
        // contents of the block, and its own text will be the closing block
        // delimiter.
        let level = self.levels.last_mut().unwrap();
        let chunk = Chunk::new_block(
            argument_chunk,
            *level.rules.last().unwrap(),
            level.nesting.indentation(),
            *level.block_argument_nesting.last().unwrap(),
            space,
            level.pending_flush_left,
        );
        let chunk = self.arena.add_chunk(chunk);
        level.chunks.push(chunk);
        level.pending_flush_left = false;

        let mut child = BuilderLevel::new(&mut self.arena, Some(chunk));
        let nesting = child.nesting.current_nesting();
        child.block_argument_nesting.push(nesting);
        self.levels.push(child);

        if indent {
            self.indent(None);
        }

        // Create a hard split for the contents. The rule on the parent BlockChunk
        // determines whether the body is split or not. This hard rule is only when
        // the block's contents are split.
        let rule = self.arena.new_hard_rule();
        self.start_rule(Some(rule));
        self.split(false, space);
    }

    /// Ends the current builder, which must have been created by
    /// [start_block()].
    ///
    /// Forces the chunk that owns the block to split if it can tell that the
    /// block contents will always split. It does that by looking for hard splits
    /// in the block that aren't for top level elements in the block. If
    /// [force_split] is `true`, the block always splits.
    ///
    /// Makes the builder for the surrounding block the current one.
    pub fn end_block(&mut self, mut force_split: bool) {
        self.divide_chunks();

        let level = self.levels.last().unwrap();

        // If the last chunk ends with a comment that wants a newline after it,
        // then force the block contents to split.
        force_split |= level.pending_nested;

        // If we don't already know if the block is going to split, see if it
        // contains any hard splits or is longer than a page.
        if !force_split {
            let mut length = 0;
            let first_rule = level.rules[0];
            for &chunk in &level.chunks {
                let c = self.arena.chunk(chunk);
                length += c.length() + self.arena.unsplit_block_length(chunk);
                if length > self.page_width {
                    force_split = true;
                    break;
                }

                // If there are any hardened splits in the chunks (aside from ones
                // using the initial hard rule created by [startBlock()] which are for
                // the top level elements in the block), then force the block to split.
                if self.arena.rule(c.rule).is_hardened() && c.rule != first_rule {
                    force_split = true;
                    break;
                }
            }
        }

        let level = self.levels.pop().unwrap();
        let block_chunk = level.block_chunk.unwrap();
        self.arena
            .chunk_mut(block_chunk)
            .block
            .as_mut()
            .unwrap()
            .children = level.chunks;

        // If there is a hard newline within the block, force the surrounding rule
        // for it so that we apply that constraint.
        if force_split {
            self.force_rules();
        }
    }

    /// Finishes writing and returns a [SourceCode] containing the final output
    /// and updated selection, if any.
    pub fn end(mut self, line_ending: &str, source: &SourceCode) -> SourceCode {
        debug_assert!(self.levels.len() == 1);

        self.divide_chunks();

        let chunks = std::mem::take(&mut self.levels[0].chunks);
        let mut ctx = WriterContext::new(&mut self.arena, line_ending, self.page_width);
        let mut writer = LineWriter::new(chunks, 0);
        let result = writer.write_lines(&mut ctx, source.is_compilation_unit);
        let length = writer.length();

        let mut selection_start = None;
        let mut selection_length = None;
        if source.selection_start.is_some() {
            // If we haven't hit the beginning and/or end of the selection yet, they
            // must be at the very end of the code.
            let start = result.selection_start.unwrap_or(length);
            let end = result.selection_end.unwrap_or(length);

            selection_start = Some(start);
            selection_length = Some(end.wrapping_sub(start));
        }

        SourceCode {
            uri: source.uri.clone(),
            text: result.text,
            is_compilation_unit: source.is_compilation_unit,
            selection_start,
            selection_length,
        }
    }

    pub fn prevent_split(&mut self) {
        self.level().prevent_split_nesting += 1;
    }

    pub fn end_prevent_split(&mut self) {
        let level = self.level();
        level.prevent_split_nesting -= 1;
        debug_assert!(level.prevent_split_nesting >= 0, "Mismatched calls.");
    }

    /// Writes the current pending [Whitespace] to the output, if any.
    ///
    /// This should only be called after source lines have been preserved to turn
    /// any ambiguous whitespace into a concrete choice.
    fn emit_pending_whitespace(&mut self, mut is_double: bool, merge_empty_splits: bool) {
        let level = self.level_ref();
        if level.pending_newlines == 0 {
            return;
        }

        if level.pending_newlines == 2 {
            is_double = true;
        }
        let nest = level.pending_nested;
        self.write_split(true, is_double, nest, false, merge_empty_splits);
    }

    /// Tries to find an existing chunk to append [comment] to.
    ///
    /// If [comment] should be appending to an existing line (in other words,
    /// should be moved before a split), then this returns that [Chunk].
    /// Otherwise, returns `None`.
    fn chunk_for_comment(&self, comment: &SourceComment, token: &str) -> Option<ChunkId> {
        let chunks = &self.level_ref().chunks;

        // Not if there is nothing before it.
        if chunks.is_empty() {
            return None;
        }

        // Don't move a comment to a preceding line.
        if comment.lines_before != 0 {
            return None;
        }

        // Multi-line comments are always pushed to the next line.
        if comment.type_ == CommentType::Doc {
            return None;
        }
        if comment.type_ == CommentType::Block {
            return None;
        }

        let mut chunk = *chunks.last().unwrap();

        // We may have started a split for a new chunk but not written any text yet.
        // In that case, the comment may get written to the previous chunk. Keep a
        // generic method comment before '(' with the '(', so don't move it before
        // the split.
        if self.arena.chunk(chunk).text().is_empty()
            && chunks.len() > 1
            && (!is_generic_method_comment(comment) || token != "(")
        {
            chunk = chunks[chunks.len() - 2];
        }

        // A block comment following a comma probably refers to the following item.
        let text = self.arena.chunk(chunk).text();
        if text.ends_with(',') && comment.type_ == CommentType::InlineBlock {
            return None;
        }

        // If the text before the split is an open grouping character, it looks
        // better to keep it with the elements than with the bracket itself.
        if text.ends_with('(')
            || text.ends_with('[')
            || (text.ends_with('{') && !text.ends_with("${"))
        {
            return None;
        }

        Some(chunk)
    }

    /// Returns `true` if a space should be output between the end of the current
    /// output and the subsequent comment which is about to be written.
    ///
    /// This is only called if the comment is trailing text in the unformatted
    /// source. In most cases, a space will be output to separate the comment
    /// from what precedes it. This returns false if:
    ///
    /// *   This comment does begin the line in the output even if it didn't in
    ///     the source.
    /// *   The comment is a block comment immediately following a grouping
    ///     character (`(`, `[`, or `{`). This is to allow `foo(/* comment */)`,
    ///     et. al.
    fn needs_space_before_comment(&self, comment: &SourceComment, chunk: ChunkId) -> bool {
        // Not at the beginning of a line.
        let text = self.arena.chunk(chunk).text();
        if text.is_empty() {
            return false;
        }

        // Always put a space before line comments.
        if comment.type_ == CommentType::Line {
            return true;
        }

        // Magic generic method comments like "Foo/*<T>*/" don't get spaces.
        if is_generic_method_comment(comment) && has_trailing_identifier_char(text) {
            return false;
        }

        // Block comments do not get a space if following a grouping character.
        !text.ends_with('(') && !text.ends_with('[') && !text.ends_with('{')
    }

    /// Returns `true` if a space should be output after the last comment which
    /// was just written and the token that will be written.
    fn needs_space_after_comment(&self, comment: &SourceComment, token: &str) -> bool {
        let level = self.level_ref();

        // Not at the beginning of a line.
        if self
            .arena
            .chunk(*level.chunks.last().unwrap())
            .text()
            .is_empty()
        {
            return false;
        }

        if level.pending_newlines > 0 {
            return false;
        }

        // Magic generic method comments like "Foo/*<T>*/" don't get spaces.
        if is_generic_method_comment(comment) && token == "(" {
            return false;
        }

        // Otherwise, it gets a space if the following token is not a delimiter or
        // the empty string, for EOF.
        token != ")" && token != "]" && token != "}" && token != "," && token != ";" && !token.is_empty()
    }

    fn needs_blank_line_before_comment(&self, comment: &SourceComment) -> bool {
        // Only if the source code has a blank line.
        if comment.lines_before < 2 {
            return false;
        }

        // Don't allow blank lines at the beginning of a block.
        let chunks = &self.level_ref().chunks;
        let Some(&last) = chunks.last() else {
            return false;
        };

        // Don't allow blank lines at the beginning of a child block.
        let text = self.arena.chunk(last).text();
        if text.ends_with('{') || text.ends_with('[') {
            return false;
        }

        true
    }

    /// Starts a new chunk with the given split information.
    ///
    /// Returns the chunk.
    fn write_split(
        &mut self,
        is_hard: bool,
        mut is_double: bool,
        nest: bool,
        space: bool,
        merge_empty_splits: bool,
    ) -> ChunkId {
        let level = self.levels.last_mut().unwrap();

        // If we've already just created a split (i.e. we have a new chunk but it's
        // still empty) then update that split with the new information. This avoids
        // duplicate splits when line comments occur in places where SourceVisitor
        // also inserts splits.
        let chunk;
        if merge_empty_splits
            && level
                .chunks
                .last()
                .is_some_and(|&last| self.arena.chunk(last).text().is_empty())
        {
            chunk = *level.chunks.last().unwrap();

            // Don't allow a blank newline at the top of a block.
            if is_double
                && level.chunks.len() > 1
                && self
                    .arena
                    .chunk(level.chunks[level.chunks.len() - 2])
                    .text()
                    .ends_with('{')
            {
                is_double = false;
            }

            // Must split before it so that there is a newline after the line comment.
            let rule = self.arena.chunk(chunk).rule;
            self.arena.rule_mut(rule).harden();

            let flush_left = level.pending_flush_left;
            self.arena
                .chunk_mut(chunk)
                .update_split(Some(flush_left), is_double, None);
        } else {
            let nesting = if nest {
                level.nesting.nesting()
            } else {
                self.arena.new_nesting_level()
            };
            chunk = self.start_chunk(nesting, is_hard, is_double, space);
        }

        let rule = self.arena.chunk(chunk).rule;
        if self.arena.rule(rule).is_hardened() {
            self.handle_hard_split();
        }

        let level = self.level();
        level.pending_newlines = 0;
        level.pending_nested = false;
        chunk
    }

    /// Writes [text] to either the current chunk or a new one if the current
    /// chunk is complete.
    fn write_text(&mut self, text: &str, chunk: Option<ChunkId>) {
        let chunk = match chunk {
            Some(chunk) => chunk,
            None => {
                if self.level_ref().chunks.is_empty() {
                    let nesting = self.arena.new_nesting_level();
                    self.start_chunk(nesting, true, false, false);
                }

                *self.level_ref().chunks.last().unwrap()
            }
        };

        let level = self.levels.last_mut().unwrap();
        let chunk = self.arena.chunk_mut(chunk);
        if level.pending_space && !chunk.text().is_empty() {
            chunk.append_text(" ");
        }
        level.pending_space = false;

        chunk.append_text(text);
    }

    fn start_chunk(
        &mut self,
        nesting: NestingId,
        is_hard: bool,
        is_double: bool,
        space: bool,
    ) -> ChunkId {
        let rule = if is_hard {
            self.arena.new_hard_rule()
        } else {
            *self.level_ref().rules.last().unwrap()
        };

        let level = self.levels.last_mut().unwrap();
        let chunk = Chunk::new(
            rule,
            level.nesting.indentation(),
            nesting,
            space,
            level.pending_flush_left,
            is_double,
        );
        let chunk = self.arena.add_chunk(chunk);
        level.chunks.push(chunk);

        level.pending_flush_left = false;
        chunk
    }

    /// Pre-processes the chunks after they are done being written by the visitor
    /// but before they are run through the line splitter.
    ///
    /// Marks ranges of chunks that can be line split independently to keep the
    /// batches we send to [LineSplitter] small.
    fn divide_chunks(&mut self) {
        // Harden all of the rules that we know get forced by containing hard
        // splits, along with all of the other rules they constrain.
        self.harden_rules();

        let level = self.levels.last().unwrap();
        for (i, &chunk) in level.chunks.iter().enumerate() {
            if !self.can_divide(i, chunk) {
                self.arena.chunk_mut(chunk).prevent_divide();
            }
        }
    }

    /// Returns true if we can divide the chunks at [index] and line split the
    /// ones before and after that separately.
    fn can_divide(&self, index: usize, chunk: ChunkId) -> bool {
        // Don't divide at the first chunk.
        if index == 0 {
            return false;
        }

        let c = self.arena.chunk(chunk);

        // Can't divide soft rules.
        if !self.arena.rule(c.rule).is_hardened() {
            return false;
        }

        // Can't divide in the middle of expression nesting.
        if self.arena.nesting(c.nesting).is_nested() {
            return false;
        }

        // If the chunk is the ending delimiter of a block, then don't separate it
        // and its children from the preceding beginning of the block.
        if c.is_block() {
            return false;
        }

        true
    }

    /// Hardens the active rules when a hard split occurs within them.
    fn handle_hard_split(&mut self) {
        let level = self.levels.last_mut().unwrap();
        let Some(&last) = level.rules.last() else {
            return;
        };

        // If the current rule doesn't care, it will "eat" the hard split and no
        // others will care either.
        if !self.arena.rule(last).splits_on_inner_rules() {
            return;
        }

        // Start with the innermost rule. This will traverse the other rules it
        // constrains.
        if !level.hard_split_rules.contains(&last) {
            level.hard_split_rules.push(last);
        }
    }

    /// Replaces all of the previously hardened rules with hard splits, along
    /// with every rule that those constrain to also split.
    ///
    /// This should only be called after all chunks have been written.
    fn harden_rules(&mut self) {
        let level = self.levels.last().unwrap();
        if level.hard_split_rules.is_empty() {
            return;
        }

        fn walk_constraints(arena: &mut Arena, rule: RuleId) {
            arena.rule_mut(rule).harden();

            // Follow this rule's constraints, recursively.
            let others: Vec<RuleId> = arena.rule(rule).constrained_rules().collect();
            for other in others {
                if other == rule {
                    continue;
                }

                let fully_split_value = arena.rule(rule).fully_split_value();
                if !arena.rule(other).is_hardened()
                    && arena.constrain(rule, fully_split_value, other)
                        == Some(arena.rule(other).fully_split_value())
                {
                    walk_constraints(arena, other);
                }
            }
        }

        for &rule in &level.hard_split_rules {
            walk_constraints(&mut self.arena, rule);
        }

        // Discard spans in hardened chunks since we know for certain they will
        // split anyway.
        for &chunk in &level.chunks {
            let rule = self.arena.chunk(chunk).rule;
            if self.arena.rule(rule).is_hardened() {
                self.arena.chunk_mut(chunk).spans.clear();
            }
        }
    }
}

/// Returns `true` if [comment] appears to be a magic generic method comment.
///
/// Those get spaced a little differently to look more like real syntax:
///
///     int f/*<S, T>*/(int x) => 3;
fn is_generic_method_comment(comment: &SourceComment) -> bool {
    comment.text.starts_with("/*<") || comment.text.starts_with("/*=")
}
