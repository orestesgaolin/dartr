// Dart source: dart_style lib/src/short/line_writer.dart

use std::rc::Rc;

use rustc_hash::FxHashMap;

use super::arena::{Arena, ChunkId};
use super::line_splitting::line_splitter::LineSplitter;

/// The state shared by the [LineWriter]s of one format run: the arena, the
/// options and the cache of formatted blocks.
pub struct WriterContext<'a> {
    pub arena: &'a mut Arena,

    pub line_ending: &'a str,

    /// The number of characters allowed in a single line.
    pub page_width: i32,

    /// The cache of blocks that have already been formatted.
    ///
    /// To cache formatted blocks, we just need to know which block it is (the
    /// chunk that contains it) and how far it was indented when we formatted it
    /// (the starting column). This is Dart `_BlockKey`.
    block_cache: FxHashMap<(ChunkId, i32), Rc<FormatResult>>,
}

impl<'a> WriterContext<'a> {
    pub fn new(arena: &'a mut Arena, line_ending: &'a str, page_width: i32) -> Self {
        WriterContext {
            arena,
            line_ending,
            page_width,
            block_cache: FxHashMap::default(),
        }
    }

    /// Gets the results of formatting the child block of [chunk] at starting
    /// [column].
    ///
    /// If that block has already been formatted, reuses the results.
    ///
    /// The column is the column for the delimiters. The contents of the block
    /// are always implicitly one level deeper than that.
    ///
    ///     main() {
    ///       function(() {
    ///         block;
    ///       });
    ///     }
    ///
    /// When we format the function expression's body, [column] will be 2, not 4.
    pub fn format_block(&mut self, chunk: ChunkId, column: i32) -> Rc<FormatResult> {
        let key = (chunk, column);

        // Use the cached one if we have it.
        if let Some(cached) = self.block_cache.get(&key) {
            return cached.clone();
        }

        let children = self
            .arena
            .chunk(chunk)
            .block
            .as_ref()
            .unwrap()
            .children
            .clone();
        let mut writer = LineWriter::new(children, column);
        let result = Rc::new(writer.write_lines(self, false));
        self.block_cache.insert(key, result.clone());
        result
    }
}

/// Given a series of chunks, splits them into lines and writes the result to
/// a buffer.
pub struct LineWriter {
    buffer: String,

    /// The length of [buffer] in UTF-16 code units (Dart `_buffer.length`).
    buffer_length: usize,

    chunks: Vec<ChunkId>,

    /// The number of characters of additional indentation to apply to each line.
    ///
    /// This is used when formatting blocks to get the output into the right
    /// column based on where the block appears.
    block_indentation: i32,

    /// The offset in [buffer] where the selection starts in the formatted code.
    ///
    /// This will be `None` if there is no selection or the writer hasn't reached
    /// the beginning of the selection yet.
    selection_start: Option<usize>,

    /// The offset in [buffer] where the selection ends in the formatted code.
    ///
    /// This will be `None` if there is no selection or the writer hasn't reached
    /// the end of the selection yet.
    selection_end: Option<usize>,
}

impl LineWriter {
    /// Creates a line writer for [chunks] (the top level chunks with
    /// [block_indentation] 0, or the children of a block).
    pub fn new(chunks: Vec<ChunkId>, block_indentation: i32) -> LineWriter {
        LineWriter {
            buffer: String::new(),
            buffer_length: 0,
            chunks,
            block_indentation,
            selection_start: None,
            selection_end: None,
        }
    }

    /// The number of characters that have been written to the output.
    pub fn length(&self) -> usize {
        self.buffer_length
    }

    #[inline]
    fn write_str(&mut self, text: &str, length: usize) {
        self.buffer.push_str(text);
        self.buffer_length += length;
    }

    /// Takes all of the chunks and divides them into sublists and line splits
    /// each list.
    ///
    /// Since this is linear and line splitting is worse it's good to feed the
    /// line splitter smaller lists of chunks when possible.
    pub fn write_lines(
        &mut self,
        ctx: &mut WriterContext,
        is_compilation_unit: bool,
    ) -> FormatResult {
        // Now that we know what hard splits there will be, break the chunks into
        // independently splittable lines.
        let mut total_cost = 0;
        let mut start = 0;

        for i in 0..self.chunks.len() {
            if !ctx.arena.chunk(self.chunks[i]).can_divide() {
                continue;
            }

            total_cost += self.complete_line(ctx, start, i);
            start = i;
        }

        if start < self.chunks.len() {
            total_cost += self.complete_line(ctx, start, self.chunks.len());
        }

        // Be a good citizen, end with a newline.
        if is_compilation_unit {
            let line_ending = ctx.line_ending;
            self.write_str(line_ending, line_ending.len());
        }

        FormatResult {
            text: std::mem::take(&mut self.buffer),
            text_length: self.buffer_length,
            cost: total_cost,
            selection_start: self.selection_start,
            selection_end: self.selection_end,
        }
    }

    /// Takes the chunks from [start] to [end], removes them, and runs the
    /// [LineSplitter] on them.
    fn complete_line(&mut self, ctx: &mut WriterContext, start: usize, end: usize) -> i32 {
        let chunks = &self.chunks[start..end];

        // Run the line splitter.
        let splits = LineSplitter::new(ctx, chunks, self.block_indentation).apply(ctx);

        // Write each chunk with the appropriate splits between them.
        for i in 0..end - start {
            let chunk = self.chunks[start + i];

            // Write the block chunk's children first.
            if ctx.arena.chunk(chunk).is_block() {
                if !splits.should_split_at(i) {
                    // This block didn't split (which implies none of the child blocks
                    // of that block split either, recursively), so write them all inline.
                    self.write_chunks_unsplit(ctx.arena, chunk);
                } else {
                    let line_ending = ctx.line_ending;
                    self.write_str(line_ending, line_ending.len());

                    // Include the formatted block contents.
                    let block = ctx.format_block(chunk, splits.get_column(i));

                    // If this block contains one of the selection markers, tell the
                    // writer where it ended up in the final output.
                    if let Some(selection_start) = block.selection_start {
                        self.selection_start = Some(self.length() + selection_start);
                    }

                    if let Some(selection_end) = block.selection_end {
                        self.selection_end = Some(self.length() + selection_end);
                    }

                    self.write_str(&block.text, block.text_length);
                }
            }

            if splits.should_split_at(i) {
                // Don't write an initial single newline at the beginning of the output.
                // If this is for a block, then the newline will be written before
                // writing the block. If it's the top level output, then it shouldn't
                // have an extra leading newline.
                if !self.buffer.is_empty() {
                    let line_ending = ctx.line_ending;
                    self.write_str(line_ending, line_ending.len());
                    if ctx.arena.chunk(chunk).is_double() {
                        self.write_str(line_ending, line_ending.len());
                    }
                }

                let column = splits.get_column(i).max(0) as usize;
                for _ in 0..column {
                    self.buffer.push(' ');
                }
                self.buffer_length += column;
            } else if ctx.arena.chunk(chunk).space_when_unsplit() {
                self.write_str(" ", 1);
            }

            self.write_chunk(ctx.arena, chunk);
        }

        splits.cost()
    }

    /// Writes the block chunks of [block] (and any child chunks of them,
    /// recursively) without any splitting.
    fn write_chunks_unsplit(&mut self, arena: &Arena, block: ChunkId) {
        for &chunk in &arena.chunk(block).block.as_ref().unwrap().children {
            if arena.chunk(chunk).space_when_unsplit() {
                self.write_str(" ", 1);
            }

            // Recurse into the block.
            if arena.chunk(chunk).is_block() {
                self.write_chunks_unsplit(arena, chunk);
            }

            self.write_chunk(arena, chunk);
        }
    }

    /// Writes [chunk] to the output and updates the selection if the chunk
    /// contains a selection marker.
    fn write_chunk(&mut self, arena: &Arena, chunk: ChunkId) {
        let chunk = arena.chunk(chunk);
        if let Some(selection_start) = chunk.selection_start {
            self.selection_start = Some((self.length() as i64 + selection_start as i64) as usize);
        }

        if let Some(selection_end) = chunk.selection_end {
            self.selection_end = Some((self.length() as i64 + selection_end as i64) as usize);
        }

        self.write_str(chunk.text(), chunk.text_length() as usize);
    }
}

/// The result of formatting a series of chunks.
pub struct FormatResult {
    /// The resulting formatted text, including newlines and leading whitespace
    /// to reach the proper column.
    pub text: String,

    /// The length of [text] in UTF-16 code units.
    pub text_length: usize,

    /// The numeric cost of the chosen solution.
    pub cost: i32,

    /// Where in the resulting buffer the selection starting point should appear
    /// if it was contained within this split list of chunks.
    ///
    /// Otherwise, this is `None`.
    pub selection_start: Option<usize>,

    /// Where in the resulting buffer the selection end point should appear if it
    /// was contained within this split list of chunks.
    ///
    /// Otherwise, this is `None`.
    pub selection_end: Option<usize>,
}
