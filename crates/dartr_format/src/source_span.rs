// Dart source: dart_style lib/src/exceptions.dart (FormatterException.message)
// Dart source: package:source_span (SourceSpan.message, highlighter)
// Dart source: source_span-1.10.2 lib/src/file.dart (SourceFile, _FileSpan)
// Dart source: source_span-1.10.2 lib/src/span_mixin.dart (message, highlight)
// Dart source: source_span-1.10.2 lib/src/span_with_context.dart
// Dart source: source_span-1.10.2 lib/src/highlighter.dart
// Dart source: source_span-1.10.2 lib/src/utils.dart
// Dart source: source_span-1.10.2 lib/src/colors.dart
// Dart source: term_glyph-1.2.2 lib/src/generated/unicode_glyph_set.dart
// Dart source: path-1.9.1 lib/src/context.dart (prettyUri, relative, split)
// Dart source: path-1.9.1 lib/src/parsed_path.dart (normalize)

//! The error message of [FormatterException]: Dart
//! `span.message(error.message, color: color)` of `package:source_span`.
//!
//! Dart strings are sequences of UTF-16 code units, and `package:source_span`
//! counts offsets, columns and lengths in code units. This port keeps all
//! text as `Vec<u16>` so that every column, padding and underline matches
//! the Dart output, also for text with non-ASCII characters.

use crate::exceptions::FormatterException;

/// Dart `FormatterException.message({bool? color})`.
pub fn formatter_exception_message(exception: &FormatterException, color: bool) -> String {
    let errors = &exception.errors;
    let mut buffer = String::new();
    buffer.push_str("Could not format because the source could not be parsed:\n");

    // In case we get a huge series of cascaded errors, just show the first few.
    let shown_errors = if errors.len() > 10 {
        &errors[..10]
    } else {
        &errors[..]
    };

    for error in shown_errors {
        let mut source: Vec<u16> = error.source.encode_utf16().collect();

        // If the parse error is for something missing from the end of the file,
        // the error position will go past the end of the source. In that case,
        // just pad the source with spaces so we can report it nicely.
        if error.offset + error.length > source.len() {
            let padding = error.offset + error.length - source.len();
            source.extend(std::iter::repeat_n(SPACE, padding));
        }

        let file = SourceFile::from_code_units(source, Some(&error.path));
        let span = file.span(error.offset, Some(error.offset + error.length));
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(&span.message(&error.message, color));
    }

    if shown_errors.len() != errors.len() {
        buffer.push('\n');
        buffer.push_str(&format!(
            "({} more errors...)",
            errors.len() - shown_errors.len()
        ));
    }

    buffer
}

/// Dart `SourceFile.fromString(text, url: url).span(start, end)
/// .message(message, color: color)`. [start] and [end] are UTF-16 code unit
/// offsets. Without a [url], the message has no " of <url>" part.
pub fn span_message(
    text: &str,
    url: Option<&str>,
    start: usize,
    end: usize,
    message: &str,
    color: bool,
) -> String {
    let file = SourceFile::from_string(text, url);
    file.span(start, Some(end)).message(message, color)
}

/// Dart `SourceSpanException(message, span).toString()` for a span of
/// `SourceFile.fromString(text, url: url).span(start, end)`.
pub fn span_exception_text(
    text: &str,
    url: Option<&str>,
    start: usize,
    end: usize,
    message: &str,
) -> String {
    format!(
        "Error on {}",
        span_message(text, url, start, end, message, false)
    )
}

// ---------------------------------------------------------------------------
// charcode.dart, colors.dart, term_glyph

const LF: u16 = 10;
const CR: u16 = 13;
const SPACE: u16 = 32;
const TAB: u16 = 9;

/// Dart `colors.dart`: color constants used for generating messages.
pub mod colors {
    pub const RED: &str = "\u{1b}[31m";
    pub const YELLOW: &str = "\u{1b}[33m";
    pub const BLUE: &str = "\u{1b}[34m";
    pub const NONE: &str = "\u{1b}[0m";
}

/// Dart `package:term_glyph` with its default `unicodeGlyphs`.
pub mod glyph {
    pub const HORIZONTAL_LINE: &str = "─";
    pub const VERTICAL_LINE: &str = "│";
    pub const TOP_LEFT_CORNER: &str = "┌";
    pub const BOTTOM_LEFT_CORNER: &str = "└";
    pub const CROSS: &str = "┼";
    pub const UP_END: &str = "╵";
    pub const DOWN_END: &str = "╷";
    pub const HORIZONTAL_LINE_BOLD: &str = "━";

    /// Dart `glyphOrAscii(glyph, alternative)`: the glyph, because the
    /// default glyph set is the unicode one.
    pub fn glyph_or_ascii<'a>(glyph: &'a str, _alternative: &'a str) -> &'a str {
        glyph
    }
}

// ---------------------------------------------------------------------------
// Dart `String` helpers on UTF-16 code units.

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Dart `string.indexOf(pattern, start)`.
fn index_of(string: &[u16], pattern: &[u16], start: usize) -> Option<usize> {
    if pattern.is_empty() {
        return (start <= string.len()).then_some(start);
    }
    if pattern.len() > string.len() {
        return None;
    }
    (start..=string.len() - pattern.len()).find(|&i| &string[i..i + pattern.len()] == pattern)
}

/// Dart `string.indexOf(char, start)` for one code unit.
fn index_of_unit(string: &[u16], unit: u16, start: usize) -> Option<usize> {
    if start >= string.len() {
        return None;
    }
    string[start..]
        .iter()
        .position(|&c| c == unit)
        .map(|i| i + start)
}

/// Dart `string.lastIndexOf(char, start)` for one code unit: searches
/// backward from [start] (inclusive). Returns `-1` when not found.
fn last_index_of_unit(string: &[u16], unit: u16, start: Option<usize>) -> isize {
    if string.is_empty() {
        return -1;
    }
    let start = start.unwrap_or(string.len() - 1).min(string.len() - 1);
    string[..=start]
        .iter()
        .rposition(|&c| c == unit)
        .map_or(-1, |i| i as isize)
}

/// Dart `string.contains(pattern)`.
fn contains(string: &[u16], pattern: &[u16]) -> bool {
    index_of(string, pattern, 0).is_some()
}

/// Dart `string.replaceAll('\r\n', '\n')`.
fn replace_crlf(string: &[u16]) -> Vec<u16> {
    let mut result = Vec::with_capacity(string.len());
    let mut i = 0;
    while i < string.len() {
        if string[i] == CR && i + 1 < string.len() && string[i + 1] == LF {
            result.push(LF);
            i += 2;
        } else {
            result.push(string[i]);
            i += 1;
        }
    }
    result
}

/// Dart `string.split('\n')`.
fn split_lines(string: &[u16]) -> Vec<&[u16]> {
    string.split(|&c| c == LF).collect()
}

/// Dart `string.substring(start, end)`, clamped to the string instead of
/// throwing a `RangeError`.
fn substring(string: &[u16], start: usize, end: usize) -> &[u16] {
    let end = end.min(string.len());
    let start = start.min(end);
    &string[start..end]
}

// ---------------------------------------------------------------------------
// utils.dart

/// Returns whether [span] covers multiple lines.
fn is_multiline(span: &SourceSpanWithContext) -> bool {
    span.start.line != span.end.line
}

/// Sets the first `None` element of [list] to [element].
fn replace_first_null(list: &mut [Option<usize>], element: usize) {
    // Dart throws an `ArgumentError` when there is no null element.
    if let Some(slot) = list.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(element);
    }
}

/// Sets the element of [list] that currently contains [element] to `None`.
fn replace_with_null(list: &mut [Option<usize>], element: usize) {
    // Dart throws an `ArgumentError` when there is no matching element.
    if let Some(slot) = list.iter_mut().find(|slot| **slot == Some(element)) {
        *slot = None;
    }
}

/// Returns the number of instances of [code_unit] in [string].
fn count_code_units(string: &[u16], code_unit: u16) -> usize {
    string.iter().filter(|&&c| c == code_unit).count()
}

/// Finds a line in [context] containing [text] at the specified [column].
///
/// Returns the index in [context] where that line begins, or `None` if none
/// exists.
fn find_line_start(context: &[u16], text: &[u16], column: usize) -> Option<usize> {
    // If the text is empty, we just want to find the first line that has at least
    // [column] characters.
    if text.is_empty() {
        let mut beginning_of_line = 0;
        loop {
            match index_of_unit(context, LF, beginning_of_line) {
                None => {
                    return (context.len() - beginning_of_line >= column)
                        .then_some(beginning_of_line);
                }
                Some(index) => {
                    if index - beginning_of_line >= column {
                        return Some(beginning_of_line);
                    }
                    beginning_of_line = index + 1;
                }
            }
        }
    }

    let mut index = index_of(context, text, 0);
    while let Some(i) = index {
        // Start looking before [index] in case [text] starts with a newline.
        let line_start = if i == 0 {
            0
        } else {
            (last_index_of_unit(context, LF, Some(i - 1)) + 1) as usize
        };
        let text_column = i - line_start;
        if column == text_column {
            return Some(line_start);
        }
        index = index_of(context, text, i + 1);
    }
    None
}

// ---------------------------------------------------------------------------
// location.dart, span_with_context.dart

/// Dart `SourceLocation`: a 0-based [offset], [line] and [column].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

/// Dart `SourceSpanWithContext`: a segment of source text with the text
/// around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpanWithContext {
    /// Dart `sourceUrl`: the URL string as given to [SourceFile], if any.
    pub source_url: Option<String>,
    pub start: SourceLocation,
    pub end: SourceLocation,
    pub text: Vec<u16>,
    /// Text around the span, which includes the line containing this span.
    pub context: Vec<u16>,
}

impl SourceSpanWithContext {
    /// Dart `SourceSpanWithContext(start, end, text, context)`. Dart throws
    /// an `ArgumentError` when [context] doesn't contain [text] at
    /// `start.column`. This port only checks it in debug builds.
    pub fn new(
        source_url: Option<String>,
        start: SourceLocation,
        end: SourceLocation,
        text: Vec<u16>,
        context: Vec<u16>,
    ) -> SourceSpanWithContext {
        debug_assert!(contains(&context, &text));
        debug_assert!(find_line_start(&context, &text, start.column).is_some());
        SourceSpanWithContext {
            source_url,
            start,
            end,
            text,
            context,
        }
    }

    pub fn length(&self) -> usize {
        self.end.offset - self.start.offset
    }

    /// Dart `SourceSpanMixin.compareTo`.
    fn compare_to(&self, other: &SourceSpanWithContext) -> std::cmp::Ordering {
        self.start
            .offset
            .cmp(&other.start.offset)
            .then(self.end.offset.cmp(&other.end.offset))
    }

    /// Dart `SourceSpanMixin.message(message, color: color)`.
    pub fn message(&self, message: &str, color: bool) -> String {
        let mut buffer = format!(
            "line {}, column {}",
            self.start.line + 1,
            self.start.column + 1
        );
        if let Some(url) = &self.source_url {
            buffer.push_str(" of ");
            buffer.push_str(&pretty_uri(url));
        }
        buffer.push_str(": ");
        buffer.push_str(message);

        let highlight = self.highlight(color);
        if !highlight.is_empty() {
            buffer.push('\n');
            buffer.push_str(&highlight);
        }
        buffer
    }

    /// Dart `SourceSpanMixin.highlight(color: color)`. The span is always a
    /// `SourceSpanWithContext`, so it is highlighted also when it is empty.
    pub fn highlight(&self, color: bool) -> String {
        Highlighter::new(self, color).highlight()
    }
}

// ---------------------------------------------------------------------------
// file.dart

/// Dart `SourceFile`: a chunk of text with a URL associated with it.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// The URL where the source file is located (Dart parses it to a `Uri`;
    /// this port keeps the string and parses it in [pretty_uri]).
    pub url: Option<String>,

    /// An array of offsets for each line beginning in the file.
    ///
    /// Each offset refers to the first character *after* the newline. If the
    /// source file has a trailing newline, the final offset won't actually be in
    /// the file.
    line_starts: Vec<usize>,

    /// The code units of the characters in this file.
    decoded_chars: Vec<u16>,
}

impl SourceFile {
    /// Dart `SourceFile.fromString(text, url: url)`.
    pub fn from_string(text: &str, url: Option<&str>) -> SourceFile {
        SourceFile::from_code_units(units(text), url)
    }

    /// Dart `SourceFile._fromList(decodedChars, url: url)`.
    pub fn from_code_units(decoded_chars: Vec<u16>, url: Option<&str>) -> SourceFile {
        let mut line_starts = vec![0];
        for i in 0..decoded_chars.len() {
            let mut c = decoded_chars[i];
            if c == CR {
                // Return not followed by newline is treated as a newline
                let j = i + 1;
                if j >= decoded_chars.len() || decoded_chars[j] != LF {
                    c = LF;
                }
            }
            if c == LF {
                line_starts.push(i + 1);
            }
        }
        SourceFile {
            url: url.map(str::to_string),
            line_starts,
            decoded_chars,
        }
    }

    /// The length of the file in characters.
    pub fn length(&self) -> usize {
        self.decoded_chars.len()
    }

    /// The number of lines in the file.
    pub fn lines(&self) -> usize {
        self.line_starts.len()
    }

    /// Returns a span from [start] to [end] (exclusive).
    ///
    /// If [end] isn't passed, it defaults to the end of the file.
    pub fn span(&self, start: usize, end: Option<usize>) -> FileSpan<'_> {
        let end = end.unwrap_or(self.length());
        assert!(end >= start, "End {end} must come after start {start}.");
        assert!(
            end <= self.length(),
            "End {end} must not be greater than the number of characters in the file, {}.",
            self.length()
        );
        FileSpan {
            file: self,
            start,
            end,
        }
    }

    /// Gets the 0-based line corresponding to [offset].
    pub fn get_line(&self, offset: usize) -> usize {
        if offset >= *self.line_starts.last().unwrap() {
            return self.line_starts.len() - 1;
        }
        // Dart `_binarySearch(offset) - 1`.
        let mut min = 0;
        let mut max = self.line_starts.len() - 1;
        while min < max {
            let half = min + (max - min) / 2;
            if self.line_starts[half] > offset {
                max = half;
            } else {
                min = half + 1;
            }
        }
        max - 1
    }

    /// Gets the 0-based column corresponding to [offset].
    pub fn get_column(&self, offset: usize) -> usize {
        let line = self.get_line(offset);
        offset - self.line_starts[line]
    }

    /// Gets the offset for a [line] and [column].
    pub fn get_offset(&self, line: usize, column: usize) -> usize {
        self.line_starts[line] + column
    }

    /// Returns the text of the file from [start] to [end] (exclusive).
    pub fn get_text(&self, start: usize, end: usize) -> Vec<u16> {
        self.decoded_chars[start..end].to_vec()
    }

    /// Dart `FileLocation` at [offset].
    fn location(&self, offset: usize) -> SourceLocation {
        SourceLocation {
            offset,
            line: self.get_line(offset),
            column: self.get_column(offset),
        }
    }
}

/// Dart `_FileSpan`: a span within a [SourceFile].
#[derive(Clone, Copy, Debug)]
pub struct FileSpan<'a> {
    pub file: &'a SourceFile,
    start: usize,
    end: usize,
}

impl FileSpan<'_> {
    pub fn length(&self) -> usize {
        self.end - self.start
    }

    pub fn start(&self) -> SourceLocation {
        self.file.location(self.start)
    }

    pub fn end(&self) -> SourceLocation {
        self.file.location(self.end)
    }

    pub fn text(&self) -> Vec<u16> {
        self.file.get_text(self.start, self.end)
    }

    pub fn context(&self) -> Vec<u16> {
        let file = self.file;
        let end_line = file.get_line(self.end);
        let end_column = file.get_column(self.end);

        let end_offset = if end_column == 0 && end_line != 0 {
            // If [end] is at the very beginning of the line, the span covers the
            // previous newline, so we only want to include the previous line in the
            // context...

            if self.length() == 0 {
                // ...unless this is a point span, in which case we want to include the
                // next line (or the empty string if this is the end of the file).
                return if end_line == file.lines() - 1 {
                    Vec::new()
                } else {
                    file.get_text(
                        file.get_offset(end_line, 0),
                        file.get_offset(end_line + 1, 0),
                    )
                };
            }

            self.end
        } else if end_line == file.lines() - 1 {
            // If the span covers the last line of the file, the context should go all
            // the way to the end of the file.
            file.length()
        } else {
            // Otherwise, the context should cover the full line on which [end]
            // appears.
            file.get_offset(end_line + 1, 0)
        };

        file.get_text(file.get_offset(file.get_line(self.start), 0), end_offset)
    }

    /// This span as a [SourceSpanWithContext].
    pub fn with_context(&self) -> SourceSpanWithContext {
        SourceSpanWithContext {
            source_url: self.file.url.clone(),
            start: self.start(),
            end: self.end(),
            text: self.text(),
            context: self.context(),
        }
    }

    /// Dart `SourceSpanMixin.message(message, color: color)`.
    pub fn message(&self, message: &str, color: bool) -> String {
        self.with_context().message(message, color)
    }

    /// Dart `SourceSpanMixin.highlight(color: color)`.
    pub fn highlight(&self, color: bool) -> String {
        self.with_context().highlight(color)
    }
}

// ---------------------------------------------------------------------------
// highlighter.dart

/// The key that groups the lines of one file. Dart uses the span's
/// `sourceUrl`, or a new opaque `Object` for each span without a URL.
#[derive(Clone, Debug, PartialEq, Eq)]
enum UrlKey {
    Uri(String),
    Opaque(usize),
}

/// A class for writing a chunk of text with a particular span highlighted.
pub struct Highlighter {
    /// The highlights. [Line::highlights] and `highlights_by_column` refer to
    /// them by index (Dart compares `_Highlight` objects by identity).
    highlights: Vec<Highlight>,

    /// The lines to display, including context around the highlighted spans.
    lines: Vec<Line>,

    /// The color to highlight the primary [Highlight] within its context, or
    /// `None` if it should not be colored.
    primary_color: Option<&'static str>,

    /// The color to highlight the secondary [Highlight]s within their context,
    /// or `None` if they should not be colored.
    secondary_color: Option<&'static str>,

    /// The number of characters before the bar in the sidebar.
    padding_before_sidebar: usize,

    /// The maximum number of multiline spans that cover any part of a single
    /// line in [lines].
    max_multiline_spans: usize,

    /// Whether [lines] includes lines from multiple different files.
    multiple_files: bool,

    /// The buffer to which to write the result, in UTF-16 code units.
    buffer: Vec<u16>,
}

/// The number of spaces to render for hard tabs that appear in `_span.text`.
///
/// We don't want to render raw tabs, because they'll mess up our character
/// alignment.
const SPACES_PER_TAB: usize = 4;

impl Highlighter {
    /// Creates a [Highlighter] that will return a string highlighting [span]
    /// within the text of its file when [highlight] is called.
    ///
    /// If [color] is `true`, the text is highlighted with the default color
    /// (red).
    pub fn new(span: &SourceSpanWithContext, color: bool) -> Highlighter {
        Highlighter::with_color(span, if color { Some(colors::RED) } else { None })
    }

    /// Dart `Highlighter(span, color: color)` where [color] is an ANSI
    /// terminal color escape, or `None` for no color.
    pub fn with_color(span: &SourceSpanWithContext, color: Option<&'static str>) -> Highlighter {
        let highlights = vec![Highlight::new(span, None, true)];
        Highlighter::new_internal(highlights, color, None)
    }

    /// Dart `Highlighter.multiple`: highlights [primary_span] as well as all
    /// the spans in [secondary_spans], each with its label.
    pub fn multiple(
        primary_span: &SourceSpanWithContext,
        primary_label: &str,
        secondary_spans: &[(SourceSpanWithContext, String)],
        color: bool,
        primary_color: Option<&'static str>,
        secondary_color: Option<&'static str>,
    ) -> Highlighter {
        let mut highlights = vec![Highlight::new(primary_span, Some(primary_label), true)];
        for (span, label) in secondary_spans {
            highlights.push(Highlight::new(span, Some(label), false));
        }
        Highlighter::new_internal(
            highlights,
            if color {
                Some(primary_color.unwrap_or(colors::RED))
            } else {
                None
            },
            if color {
                Some(secondary_color.unwrap_or(colors::BLUE))
            } else {
                None
            },
        )
    }

    fn new_internal(
        highlights: Vec<Highlight>,
        primary_color: Option<&'static str>,
        secondary_color: Option<&'static str>,
    ) -> Highlighter {
        let lines = Highlighter::collate_lines(&highlights);
        let padding_before_sidebar = 1 + std::cmp::max(
            // In a purely mathematical world, floor(log10(n)) would give the
            // number of digits in n, but floating point errors render that
            // unreliable in practice.
            (lines.last().unwrap().number + 1).to_string().len(),
            // If [_lines] aren't contiguous, we'll write "..." in place of a
            // line number.
            if Highlighter::contiguous(&lines) {
                0
            } else {
                3
            },
        );
        let max_multiline_spans = lines
            .iter()
            .map(|line| {
                line.highlights
                    .iter()
                    .filter(|&&h| is_multiline(&highlights[h].span))
                    .count()
            })
            .max()
            .unwrap();
        let multiple_files = lines.iter().any(|line| line.url != lines[0].url);
        Highlighter {
            highlights,
            lines,
            primary_color,
            secondary_color,
            padding_before_sidebar,
            max_multiline_spans,
            multiple_files,
            buffer: Vec::new(),
        }
    }

    /// Returns whether [lines] contains any adjacent lines from the same source
    /// file that aren't adjacent in the original file.
    fn contiguous(lines: &[Line]) -> bool {
        lines
            .windows(2)
            .all(|pair| pair[0].number + 1 == pair[1].number || pair[0].url != pair[1].url)
    }

    /// Collect all the source lines from the contexts of all spans in
    /// [highlights], and associates them with the highlights that cover them.
    fn collate_lines(highlights: &[Highlight]) -> Vec<Line> {
        // Assign spans without URLs opaque Objects as keys. Each such Object will
        // be different, but they can then be used later on to determine which lines
        // came from the same span even if they'd all otherwise have `null` URLs.
        let mut highlights_by_url: Vec<(UrlKey, Vec<usize>)> = Vec::new();
        for (index, highlight) in highlights.iter().enumerate() {
            let key = match &highlight.span.source_url {
                Some(url) => UrlKey::Uri(url.clone()),
                None => UrlKey::Opaque(index),
            };
            match highlights_by_url.iter_mut().find(|(k, _)| *k == key) {
                Some((_, list)) => list.push(index),
                None => highlights_by_url.push((key, vec![index])),
            }
        }
        for (_, list) in &mut highlights_by_url {
            list.sort_by(|&a, &b| highlights[a].span.compare_to(&highlights[b].span));
        }

        let mut result = Vec::new();
        for (url, highlights_for_file) in highlights_by_url {
            // First, create a list of all the lines in the current file that we have
            // context for along with their line numbers.
            let mut lines: Vec<Line> = Vec::new();
            for &h in &highlights_for_file {
                let span = &highlights[h].span;
                let context = &span.context;
                // If [highlight.span.context] contains lines prior to the one
                // [highlight.span.text] appears on, write those first.
                let line_start = find_line_start(context, &span.text, span.start.column)
                    .expect("the span text is in the context");

                let lines_before_span = count_code_units(&context[..line_start], LF);

                let first_line_number = span.start.line as isize - lines_before_span as isize;
                for (line_number, line) in (first_line_number..).zip(split_lines(context)) {
                    // Only add a line if it hasn't already been added for a previous span
                    if lines.is_empty() || line_number > lines.last().unwrap().number as isize {
                        lines.push(Line {
                            text: line.to_vec(),
                            number: line_number as usize,
                            url: url.clone(),
                            highlights: Vec::new(),
                        });
                    }
                }
            }

            // Next, associate each line with each highlights that covers it.
            let mut active_highlights: Vec<usize> = Vec::new();
            let mut highlight_index = 0;
            for line in &mut lines {
                active_highlights.retain(|&h| highlights[h].span.end.line >= line.number);

                let old_highlight_length = active_highlights.len();
                for &h in highlights_for_file.iter().skip(highlight_index) {
                    if highlights[h].span.start.line > line.number {
                        break;
                    }
                    active_highlights.push(h);
                }
                highlight_index += active_highlights.len() - old_highlight_length;

                line.highlights.extend_from_slice(&active_highlights);
            }

            result.extend(lines);
        }
        result
    }

    /// Returns the highlighted span text.
    ///
    /// This method should only be called once.
    pub fn highlight(mut self) -> String {
        let first_url = self.lines[0].url.clone();
        self.write_file_start(&first_url);

        // Each index of this list represents a column after the sidebar that could
        // contain a line indicating an active highlight. If it's `None`, that
        // column is empty; if it contains a highlight, it should be drawn for that
        // column.
        let mut highlights_by_column: Vec<Option<usize>> = vec![None; self.max_multiline_spans];

        for i in 0..self.lines.len() {
            let line = self.lines[i].clone();
            if i > 0 {
                let last_line = &self.lines[i - 1];
                if last_line.url != line.url {
                    self.write_sidebar(None, None, Some(glyph::UP_END));
                    self.buffer.push(LF);
                    self.write_file_start(&line.url);
                } else if last_line.number + 1 != line.number {
                    self.write_sidebar(None, Some("..."), None);
                    self.buffer.push(LF);
                }
            }

            // If a highlight covers the entire first line other than initial
            // whitespace, don't bother pointing out exactly where it begins. Iterate
            // in reverse so that longer highlights (which are sorted after shorter
            // highlights) appear further out, leading to fewer crossed lines.
            for &h in line.highlights.iter().rev() {
                let span = &self.highlights[h].span;
                if is_multiline(span)
                    && span.start.line == line.number
                    && is_only_whitespace(substring(&line.text, 0, span.start.column))
                {
                    replace_first_null(&mut highlights_by_column, h);
                }
            }

            self.write_sidebar(Some(line.number), None, None);
            self.buffer.push(SPACE);
            self.write_multiline_highlights(&line, &highlights_by_column, None);
            if !highlights_by_column.is_empty() {
                self.buffer.push(SPACE);
            }

            let primary = line
                .highlights
                .iter()
                .copied()
                .find(|&h| self.highlights[h].is_primary);

            if let Some(primary) = primary {
                let span = &self.highlights[primary].span;
                let start_column = if span.start.line == line.number {
                    span.start.column
                } else {
                    0
                };
                let end_column = if span.end.line == line.number {
                    span.end.column
                } else {
                    line.text.len()
                };
                let color = self.primary_color;
                self.write_highlighted_text(&line.text, start_column, end_column, color);
            } else {
                self.write_text(&line.text);
            }
            self.buffer.push(LF);

            // Always write the primary span's indicator first so that it's right next
            // to the highlighted text.
            if let Some(primary) = primary {
                self.write_indicator(&line, primary, &mut highlights_by_column);
            }
            for &h in &line.highlights {
                if self.highlights[h].is_primary {
                    continue;
                }
                self.write_indicator(&line, h, &mut highlights_by_column);
            }
        }

        self.write_sidebar(None, None, Some(glyph::UP_END));
        String::from_utf16_lossy(&self.buffer)
    }

    /// Writes the beginning of the file highlight for the file with the given
    /// [url] (or opaque object if it comes from a span with a null URL).
    fn write_file_start(&mut self, url: &UrlKey) {
        match url {
            UrlKey::Uri(url) if self.multiple_files => {
                self.write_sidebar(None, None, Some(glyph::TOP_LEFT_CORNER));
                self.colorize(Some(colors::BLUE), |this| {
                    this.write_str(&glyph::HORIZONTAL_LINE.repeat(2));
                    this.write_str(">");
                });
                self.write_str(&format!(" {}", pretty_uri(url)));
            }
            _ => self.write_sidebar(None, None, Some(glyph::DOWN_END)),
        }
        self.buffer.push(LF);
    }

    /// Writes the post-sidebar highlight bars for [line] according to
    /// [highlights_by_column].
    ///
    /// If [current] is passed, it's the highlight for which an indicator is being
    /// written. If it appears in [highlights_by_column], a horizontal line is
    /// written from its column to the rightmost column.
    fn write_multiline_highlights(
        &mut self,
        line: &Line,
        highlights_by_column: &[Option<usize>],
        current: Option<usize>,
    ) {
        // Whether we've written a sidebar indicator for opening a new span on this
        // line, and which color should be used for that indicator's rightward line.
        let mut opened_on_this_line = false;
        let mut opened_on_this_line_color: Option<&'static str> = None;

        let current_color = current.and_then(|c| self.color_of(c));
        let mut found_current = false;
        for &highlight in highlights_by_column {
            let start_line = highlight.map(|h| self.highlights[h].span.start.line);
            let end_line = highlight.map(|h| self.highlights[h].span.end.line);
            if current.is_some() && highlight == current {
                found_current = true;
                debug_assert!(start_line == Some(line.number) || end_line == Some(line.number));
                self.colorize(current_color, |this| {
                    this.write_str(if start_line == Some(line.number) {
                        glyph::TOP_LEFT_CORNER
                    } else {
                        glyph::BOTTOM_LEFT_CORNER
                    });
                });
            } else if found_current {
                self.colorize(current_color, |this| {
                    this.write_str(if highlight.is_none() {
                        glyph::HORIZONTAL_LINE
                    } else {
                        glyph::CROSS
                    });
                });
            } else if let Some(h) = highlight {
                let color = self.color_of(h);
                let is_labeled = self.highlights[h].label.is_some();
                let span_end_column = self.highlights[h].span.end.column;
                self.colorize(color, |this| {
                    let vertical = if opened_on_this_line {
                        glyph::CROSS
                    } else {
                        glyph::VERTICAL_LINE
                    };
                    if current.is_some() {
                        this.write_str(vertical);
                    } else if start_line == Some(line.number) {
                        this.colorize(opened_on_this_line_color, |this| {
                            this.write_str(glyph::glyph_or_ascii(
                                if opened_on_this_line { "┬" } else { "┌" },
                                "/",
                            ));
                        });
                        opened_on_this_line = true;
                        if opened_on_this_line_color.is_none() {
                            opened_on_this_line_color = color;
                        }
                    } else if end_line == Some(line.number) && span_end_column == line.text.len() {
                        this.write_str(if is_labeled {
                            vertical
                        } else {
                            glyph::glyph_or_ascii("└", "\\")
                        });
                    } else {
                        this.colorize(opened_on_this_line_color, |this| {
                            this.write_str(vertical);
                        });
                    }
                });
            } else if opened_on_this_line {
                self.colorize(opened_on_this_line_color, |this| {
                    this.write_str(glyph::HORIZONTAL_LINE);
                });
            } else {
                self.buffer.push(SPACE);
            }
        }
    }

    // Writes [text], with text between [start_column] and [end_column] colorized in
    // the same way as [colorize].
    fn write_highlighted_text(
        &mut self,
        text: &[u16],
        start_column: usize,
        end_column: usize,
        color: Option<&'static str>,
    ) {
        self.write_text(substring(text, 0, start_column));
        self.colorize(color, |this| {
            this.write_text(substring(text, start_column, end_column));
        });
        self.write_text(substring(text, end_column, text.len()));
    }

    /// Writes an indicator for where [highlight] starts, ends, or both below
    /// [line].
    ///
    /// This may either add or remove [highlight] from [highlights_by_column].
    fn write_indicator(
        &mut self,
        line: &Line,
        highlight: usize,
        highlights_by_column: &mut [Option<usize>],
    ) {
        let color = self.color_of(highlight);
        let span = self.highlights[highlight].span.clone();
        let is_primary = self.highlights[highlight].is_primary;
        if !is_multiline(&span) {
            self.write_sidebar(None, None, None);
            self.buffer.push(SPACE);
            self.write_multiline_highlights(line, highlights_by_column, Some(highlight));
            if !highlights_by_column.is_empty() {
                self.buffer.push(SPACE);
            }

            let underline_length = self.colorize(color, |this| {
                let start = this.buffer.len();
                this.write_underline(
                    line,
                    &span,
                    if is_primary {
                        "^"
                    } else {
                        glyph::HORIZONTAL_LINE_BOLD
                    },
                );
                this.buffer.len() - start
            });
            self.write_label(highlight, highlights_by_column, underline_length);
        } else if span.start.line == line.number {
            if highlights_by_column.contains(&Some(highlight)) {
                return;
            }
            replace_first_null(highlights_by_column, highlight);

            self.write_sidebar(None, None, None);
            self.buffer.push(SPACE);
            self.write_multiline_highlights(line, highlights_by_column, Some(highlight));
            self.colorize(color, |this| {
                this.write_arrow(line, span.start.column, true)
            });
            self.buffer.push(LF);
        } else if span.end.line == line.number {
            let covers_whole_line = span.end.column == line.text.len();
            if covers_whole_line && self.highlights[highlight].label.is_none() {
                replace_with_null(highlights_by_column, highlight);
                return;
            }

            self.write_sidebar(None, None, None);
            self.buffer.push(SPACE);
            self.write_multiline_highlights(line, highlights_by_column, Some(highlight));

            let underline_length = self.colorize(color, |this| {
                let start = this.buffer.len();
                if covers_whole_line {
                    this.write_str(&glyph::HORIZONTAL_LINE.repeat(3));
                } else {
                    this.write_arrow(line, span.end.column.saturating_sub(1), false);
                }
                this.buffer.len() - start
            });
            self.write_label(highlight, highlights_by_column, underline_length);
            replace_with_null(highlights_by_column, highlight);
        }
    }

    /// Underlines the portion of [line] covered by [span] with repeated instances
    /// of [character].
    fn write_underline(&mut self, line: &Line, span: &SourceSpanWithContext, character: &str) {
        debug_assert!(!is_multiline(span));
        debug_assert!(contains(&line.text, &span.text));

        let mut start_column = span.start.column;
        let mut end_column = span.end.column;

        // Adjust the start and end columns to account for any tabs that were
        // converted to spaces.
        let tabs_before = count_tabs(substring(&line.text, 0, start_column));
        let tabs_inside = count_tabs(substring(&line.text, start_column, end_column));
        start_column += tabs_before * (SPACES_PER_TAB - 1);
        end_column += (tabs_before + tabs_inside) * (SPACES_PER_TAB - 1);

        self.write_str(&" ".repeat(start_column));
        self.write_str(
            &character.repeat(std::cmp::max(end_column.saturating_sub(start_column), 1)),
        );
    }

    /// Write an arrow pointing to column [column] in [line].
    ///
    /// If the arrow points to a tab character, this will point to the beginning
    /// of the tab if [beginning] is `true` and the end if it's `false`.
    fn write_arrow(&mut self, line: &Line, column: usize, beginning: bool) {
        let tabs = count_tabs(substring(
            &line.text,
            0,
            column + if beginning { 0 } else { 1 },
        ));
        self.write_str(&glyph::HORIZONTAL_LINE.repeat(1 + column + tabs * (SPACES_PER_TAB - 1)));
        self.write_str("^");
    }

    /// Writes [highlight]'s label.
    ///
    /// The `buffer` is assumed to be written to the point where the first line
    /// of `highlight.label` can be written after a space, but this takes care of
    /// writing indentation and highlight columns for later lines.
    ///
    /// The [highlights_by_column] are used to write ongoing highlight lines if the
    /// label is more than one line long.
    ///
    /// The [underline_length] is the length of the line written between the
    /// highlights and the beginning of the first label.
    fn write_label(
        &mut self,
        highlight: usize,
        highlights_by_column: &[Option<usize>],
        underline_length: usize,
    ) {
        let Some(label) = self.highlights[highlight].label.clone() else {
            self.buffer.push(LF);
            return;
        };

        let lines = split_lines(&label);
        let color = self.color_of(highlight);
        self.colorize(color, |this| {
            this.buffer.push(SPACE);
            this.buffer.extend_from_slice(lines[0]);
        });
        self.buffer.push(LF);

        for text in lines.iter().skip(1) {
            self.write_sidebar(None, None, None);
            self.buffer.push(SPACE);
            for &column_highlight in highlights_by_column {
                if column_highlight.is_none() || column_highlight == Some(highlight) {
                    self.buffer.push(SPACE);
                } else {
                    self.write_str(glyph::VERTICAL_LINE);
                }
            }

            self.write_str(&" ".repeat(underline_length));
            self.colorize(color, |this| {
                this.buffer.push(SPACE);
                this.buffer.extend_from_slice(text);
            });
            self.buffer.push(LF);
        }
    }

    /// Writes a snippet from the source text, converting hard tab characters into
    /// plain indentation.
    fn write_text(&mut self, text: &[u16]) {
        for &char in text {
            if char == TAB {
                self.buffer
                    .extend(std::iter::repeat_n(SPACE, SPACES_PER_TAB));
            } else {
                self.buffer.push(char);
            }
        }
    }

    // Writes a sidebar to [buffer] that includes [line] as the line number if
    // given and writes [end] at the end (defaults to [glyph::VERTICAL_LINE]).
    //
    // If [text] is given, it's used in place of the line number. It can't be
    // passed at the same time as [line].
    fn write_sidebar(&mut self, line: Option<usize>, text: Option<&str>, end: Option<&str>) {
        debug_assert!(line.is_none() || text.is_none());

        // Add 1 to line to convert from computer-friendly 0-indexed line numbers to
        // human-friendly 1-indexed line numbers.
        let text = match line {
            Some(line) => (line + 1).to_string(),
            None => text.unwrap_or("").to_string(),
        };
        let padding = self.padding_before_sidebar;
        self.colorize(Some(colors::BLUE), |this| {
            // Dart `padRight` counts code units; `text` is ASCII.
            this.write_str(&format!("{text:<padding$}"));
            this.write_str(end.unwrap_or(glyph::VERTICAL_LINE));
        });
    }

    /// Colors all text written to [buffer] during [callback], if colorization is
    /// enabled and [color] is not `None`.
    fn colorize<T>(&mut self, color: Option<&str>, callback: impl FnOnce(&mut Self) -> T) -> T {
        let enabled = self.primary_color.is_some() && color.is_some();
        if let (true, Some(color)) = (enabled, color) {
            self.write_str(color);
        }
        let result = callback(self);
        if enabled {
            self.write_str(colors::NONE);
        }
        result
    }

    /// The color of [highlight]: the primary or the secondary color.
    fn color_of(&self, highlight: usize) -> Option<&'static str> {
        if self.highlights[highlight].is_primary {
            self.primary_color
        } else {
            self.secondary_color
        }
    }

    fn write_str(&mut self, s: &str) {
        self.buffer.extend(s.encode_utf16());
    }
}

/// Returns the number of hard tabs in [text].
fn count_tabs(text: &[u16]) -> usize {
    count_code_units(text, TAB)
}

/// Returns whether [text] contains only space or tab characters.
fn is_only_whitespace(text: &[u16]) -> bool {
    text.iter().all(|&c| c == SPACE || c == TAB)
}

/// Information about how to highlight a single section of a source file.
#[derive(Clone, Debug)]
struct Highlight {
    /// The section of the source file to highlight.
    ///
    /// This is normalized to make it easier for [Highlighter] to work with.
    span: SourceSpanWithContext,

    /// Whether this is the primary span in the highlight.
    ///
    /// The primary span is highlighted with a different character and colored
    /// differently than non-primary spans.
    is_primary: bool,

    /// The label to include inline when highlighting [span].
    ///
    /// This helps distinguish clarify what each highlight means when multiple are
    /// used in the same message.
    label: Option<Vec<u16>>,
}

impl Highlight {
    fn new(span: &SourceSpanWithContext, label: Option<&str>, primary: bool) -> Highlight {
        let new_span = Highlight::normalize_context(span);
        let new_span = Highlight::normalize_newlines(new_span);
        let new_span = Highlight::normalize_trailing_newline(new_span);
        let new_span = Highlight::normalize_end_of_line(new_span);
        Highlight {
            span: new_span,
            is_primary: primary,
            label: label.map(|label| replace_crlf(&units(label))),
        }
    }

    /// Normalizes [span] to ensure that it's a [SourceSpanWithContext] whose
    /// context actually contains its text at the expected column.
    ///
    /// If it's not already a [SourceSpanWithContext], adjust the start and end
    /// locations' line and column fields so that the highlighter can assume they
    /// match up with the context.
    fn normalize_context(span: &SourceSpanWithContext) -> SourceSpanWithContext {
        if find_line_start(&span.context, &span.text, span.start.column).is_some() {
            return span.clone();
        }
        SourceSpanWithContext::new(
            span.source_url.clone(),
            SourceLocation {
                offset: span.start.offset,
                line: 0,
                column: 0,
            },
            SourceLocation {
                offset: span.end.offset,
                line: count_code_units(&span.text, LF),
                column: Highlight::last_line_length(&span.text),
            },
            span.text.clone(),
            span.text.clone(),
        )
    }

    /// Normalizes [span] to replace Windows-style newlines with Unix-style
    /// newlines.
    fn normalize_newlines(span: SourceSpanWithContext) -> SourceSpanWithContext {
        let text = &span.text;
        if !contains(text, &[CR, LF]) {
            return span;
        }

        let mut end_offset = span.end.offset;
        for i in 0..text.len() - 1 {
            if text[i] == CR && text[i + 1] == LF {
                end_offset -= 1;
            }
        }

        SourceSpanWithContext::new(
            span.source_url.clone(),
            span.start,
            SourceLocation {
                offset: end_offset,
                line: span.end.line,
                column: span.end.column,
            },
            replace_crlf(text),
            replace_crlf(&span.context),
        )
    }

    /// Normalizes [span] to remove a trailing newline from `span.context`.
    ///
    /// If necessary, also adjust `span.end` so that it doesn't point past where
    /// the trailing newline used to be.
    fn normalize_trailing_newline(span: SourceSpanWithContext) -> SourceSpanWithContext {
        if !span.context.ends_with(&[LF]) {
            return span;
        }

        // If there's a full blank line on the end of [span.context], it's probably
        // significant, so we shouldn't trim it.
        if span.text.ends_with(&[LF, LF]) {
            return span;
        }

        let context = span.context[..span.context.len() - 1].to_vec();
        let mut text = span.text.clone();
        let mut start = span.start;
        let mut end = span.end;
        if span.text.ends_with(&[LF]) && Highlight::is_text_at_end_of_context(&span) {
            text = span.text[..span.text.len() - 1].to_vec();
            if text.is_empty() {
                end = start;
            } else {
                end = SourceLocation {
                    offset: span.end.offset - 1,
                    line: span.end.line - 1,
                    column: Highlight::last_line_length(&context),
                };
                start = if span.start.offset == span.end.offset {
                    end
                } else {
                    span.start
                };
            }
        }
        SourceSpanWithContext::new(span.source_url, start, end, text, context)
    }

    /// Normalizes [span] so that the end location is at the end of a line rather
    /// than at the beginning of the next line.
    fn normalize_end_of_line(span: SourceSpanWithContext) -> SourceSpanWithContext {
        if span.end.column != 0 {
            return span;
        }
        if span.end.line == span.start.line {
            return span;
        }

        let text = span.text[..span.text.len() - 1].to_vec();
        let column = (text.len() as isize - last_index_of_unit(&text, LF, None) - 1) as usize;

        // If the context also ends with a newline, it's possible that we don't
        // have the full context for that line, so we shouldn't print it at all.
        let context = if span.context.ends_with(&[LF]) {
            span.context[..span.context.len() - 1].to_vec()
        } else {
            span.context.clone()
        };
        SourceSpanWithContext::new(
            span.source_url,
            span.start,
            SourceLocation {
                offset: span.end.offset - 1,
                line: span.end.line - 1,
                column,
            },
            text,
            context,
        )
    }

    /// Returns the length of the last line in [text], whether or not it ends in a
    /// newline.
    fn last_line_length(text: &[u16]) -> usize {
        if text.is_empty() {
            0
        } else if text[text.len() - 1] == LF {
            if text.len() == 1 {
                0
            } else {
                (text.len() as isize - last_index_of_unit(text, LF, Some(text.len() - 2)) - 1)
                    as usize
            }
        } else {
            (text.len() as isize - last_index_of_unit(text, LF, None) - 1) as usize
        }
    }

    /// Returns whether [span]'s text runs all the way to the end of its context.
    fn is_text_at_end_of_context(span: &SourceSpanWithContext) -> bool {
        find_line_start(&span.context, &span.text, span.start.column)
            .expect("the span text is in the context")
            + span.start.column
            + span.length()
            == span.context.len()
    }
}

/// A single line of the source file being highlighted.
#[derive(Clone, Debug)]
struct Line {
    /// The text of the line, not including the trailing newline.
    text: Vec<u16>,

    /// The 0-based line number in the source file.
    number: usize,

    /// The URL of the source file in which this line appears.
    ///
    /// For lines created from spans without an explicit URL, this is an opaque
    /// key that differs between lines that come from different spans.
    url: UrlKey,

    /// All highlights that cover any portion of this line, in source span order.
    ///
    /// This is populated after the initial line is created.
    highlights: Vec<usize>,
}

// ---------------------------------------------------------------------------
// package:path (posix style) prettyUri

/// Dart `p.prettyUri(Uri.parse(uri))` with the current working directory
/// as `p.current`.
pub fn pretty_uri(uri: &str) -> String {
    let current = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_string());
    pretty_uri_from(uri, &current)
}

/// Dart `Context(current: current).prettyUri(Uri.parse(uri))` for the posix
/// style.
///
/// Returns a pretty URI for a path: a path relative to [current] if it is
/// not longer than the absolute path, the absolute path otherwise, or the
/// URI itself if it is not a `file:` or scheme-less URI.
pub fn pretty_uri_from(uri: &str, current: &str) -> String {
    let parsed = match ParsedUri::parse(uri) {
        Some(parsed) => parsed,
        // Dart `Uri.parse` throws a `FormatException` here. Show the URL
        // unchanged.
        None => return uri.to_string(),
    };
    if parsed.scheme != "file" && !parsed.scheme.is_empty() {
        return parsed.to_string_value();
    }

    let path = posix_normalize(&parsed.path_from_uri());
    let rel = posix_relative(&path, current);

    // Only return a relative path if it's actually shorter than the absolute
    // path. This avoids ugly things like long "../" chains to get to the root
    // and then go back down.
    if posix_split(&rel).len() > posix_split(&path).len() {
        path
    } else {
        rel
    }
}

/// The parts of Dart `Uri.parse(uri)` that [pretty_uri_from] uses.
struct ParsedUri<'a> {
    source: &'a str,
    /// The lower-case scheme, or the empty string.
    scheme: String,
    /// The raw (percent-encoded) path.
    path: &'a str,
}

impl<'a> ParsedUri<'a> {
    /// Dart `Uri.parse`. Returns `None` where Dart throws a
    /// `FormatException` (an invalid scheme).
    fn parse(source: &'a str) -> Option<ParsedUri<'a>> {
        let mut scheme = String::new();
        let mut rest = source;
        if let Some(colon) = source.find([':', '/', '?', '#'])
            && source.as_bytes()[colon] == b':'
        {
            let candidate = &source[..colon];
            let valid = candidate
                .bytes()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
                && candidate
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'+' || c == b'-' || c == b'.');
            if !valid {
                return None;
            }
            scheme = candidate.to_ascii_lowercase();
            rest = &source[colon + 1..];
        }

        // Skip the authority.
        if let Some(after) = rest.strip_prefix("//") {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            rest = &after[end..];
        }

        // Drop the query and the fragment.
        let end = rest.find(['?', '#']).unwrap_or(rest.len());
        Some(ParsedUri {
            source,
            scheme,
            path: &rest[..end],
        })
    }

    /// Dart `uri.toString()` for a URI with a scheme.
    fn to_string_value(&self) -> String {
        format!("{}{}", self.scheme, &self.source[self.scheme.len()..])
    }

    /// Dart `Style.posix.pathFromUri(uri)`: `Uri.decodeComponent(uri.path)`.
    /// A `file:` URI always has an absolute path.
    fn path_from_uri(&self) -> String {
        let decoded = percent_decode(self.path);
        if self.scheme == "file" && !decoded.starts_with('/') {
            format!("/{decoded}")
        } else {
            decoded
        }
    }
}

/// Decodes the `%XX` escapes of [s] as UTF-8 bytes. Dart `Uri.parse`
/// escapes a `%` that is not followed by two hex digits, so decoding keeps it.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            result.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&result).into_owned()
}

/// Dart `ParsedPath.parse(path, Style.posix)`: whether the path has the `/`
/// root, and the parts between separators.
fn posix_parse(path: &str) -> (bool, Vec<&str>) {
    let (is_absolute, rest) = match path.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, path),
    };
    let parts = if rest.is_empty() {
        Vec::new()
    } else {
        rest.split('/').collect()
    };
    (is_absolute, parts)
}

/// Dart `ParsedPath.normalize()` for the posix style: the normalized parts.
fn posix_normalize_parts<'a>(is_absolute: bool, parts: &[&'a str]) -> Vec<&'a str> {
    // Handle '.', '..', and empty parts.
    let mut leading_doubles = 0;
    let mut new_parts: Vec<&str> = Vec::new();
    for &part in parts {
        if part == "." || part.is_empty() {
            // Do nothing. Ignore it.
        } else if part == ".." {
            // Pop the last part off.
            if new_parts.pop().is_none() {
                // Backed out past the beginning, so preserve the "..".
                leading_doubles += 1;
            }
        } else {
            new_parts.push(part);
        }
    }

    // A relative path can back out from the start directory.
    if !is_absolute {
        let mut parts = vec![".."; leading_doubles];
        parts.extend(new_parts);
        new_parts = parts;
    }

    // If we collapsed down to nothing, do ".".
    if new_parts.is_empty() && !is_absolute {
        new_parts.push(".");
    }
    new_parts
}

/// Dart `p.normalize(path)` for the posix style.
fn posix_normalize(path: &str) -> String {
    let (is_absolute, parts) = posix_parse(path);
    let parts = posix_normalize_parts(is_absolute, &parts);
    format!("{}{}", if is_absolute { "/" } else { "" }, parts.join("/"))
}

/// Dart `p.split(path)` for the posix style.
fn posix_split(path: &str) -> Vec<&str> {
    let (is_absolute, parts) = posix_parse(path);
    let mut result: Vec<&str> = Vec::new();
    if is_absolute {
        result.push("/");
    }
    result.extend(parts.into_iter().filter(|part| !part.is_empty()));
    result
}

/// Dart `p.relative(path)` for the posix style with an absolute [current]
/// directory.
fn posix_relative(path: &str, current: &str) -> String {
    // Avoid expensive computation if the path is already relative.
    if !path.starts_with('/') {
        return posix_normalize(path);
    }

    let (_, from_parts) = posix_parse(current);
    let from_parts = posix_normalize_parts(true, &from_parts);
    let (_, path_parts) = posix_parse(path);
    let path_parts = posix_normalize_parts(true, &path_parts);

    // Strip off their common prefix.
    let common = from_parts
        .iter()
        .zip(&path_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let from_parts = &from_parts[common..];
    let path_parts = &path_parts[common..];

    // If there are any directories left in the from path, we need to walk up
    // out of them.
    let mut parts: Vec<&str> = vec![".."; from_parts.len()];
    parts.extend_from_slice(path_parts);

    // Corner case: the paths completely collapsed.
    if parts.is_empty() {
        return ".".to_string();
    }

    parts.join("/")
}
