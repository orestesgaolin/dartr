// Dart source: dart_style lib/src/back_end/code.dart

use std::rc::Rc;

use crate::source_code::SourceCode;
use crate::text::{substring_utf16, utf16_len};

/// Base class for an object that represents fully formatted code.
///
/// We use this instead of immediately generating a string for the resulting
/// formatted code because of separate formatting. Often, a subtree of the
/// [Piece] tree can be solved and formatted separately. The resulting
/// [Solution] may be used by multiple different surrounding solutions while
/// the [Solver] works its magic looking for the best solution. When a
/// separately formatted child solution is merged into its parent, we want
/// that to be fast. Appending strings to a buffer is fairly fast, but not as
/// fast simply appending a single [GroupCode] to the parent solution's
/// [GroupCode].
///
/// The text is borrowed from the pieces (`'p`).
pub enum Code<'p> {
    /// A newline followed by any leading indentation.
    Newline {
        /// Whether a blank line (two newlines) should be written.
        blank: bool,

        /// The number of spaces of indentation after this newline.
        indent: i32,
    },

    /// Literal source text.
    Text(&'p str),

    Group(Rc<GroupCode<'p>>),

    /// Marks the location of the beginning or end of a selection as occurring
    /// [offset] characters past the point where this marker object appears in
    /// the list of [Code] objects.
    Marker {
        /// What kind of selection endpoint is being marked.
        marker: Marker,

        /// The number of characters into the next [Code] object where the
        /// marker should appear in the resulting output.
        offset: usize,
    },

    EnableFormatting {
        /// Whether this comment disables formatting (`format off`) or
        /// re-enables it (`format on`).
        enabled: bool,

        /// The number of code points from the beginning of the unformatted
        /// source where the unformatted code should begin or end.
        ///
        /// If this piece is for `// dart format off`, then the offset is just
        /// past the `off`. If this piece is for `// dart format on`, it
        /// points to just before `//`.
        source_offset: usize,
    },
}

/// Which selection marker is pointed to by a [Code::Marker].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Marker {
    Start,
    End,
}

/// A [Code] object which can be written to and contain other child [Code]
/// objects.
pub struct GroupCode<'p> {
    /// How many spaces the first text inside this group should be indented.
    indent: i32,

    /// The child [Code] objects contained in this group.
    children: Vec<Code<'p>>,
}

impl<'p> GroupCode<'p> {
    pub fn new(indent: i32) -> GroupCode<'p> {
        GroupCode {
            indent,
            children: Vec::new(),
        }
    }

    /// Appends [text] to this code.
    #[inline]
    pub fn write(&mut self, text: &'p str) {
        self.children.push(Code::Text(text));
    }

    /// Writes a newline and the subsequent indentation to this code.
    ///
    /// If [blank] is `true`, then a blank line is written. Otherwise, only a
    /// single newline is written. The [indent] parameter is the number of
    /// spaces of leading indentation on the next line after the newline.
    pub fn newline(&mut self, blank: bool, indent: i32) {
        // Don't insert a redundant newline at the top of a group.
        if !self.children.is_empty() {
            self.children.push(Code::Newline { blank, indent });
        }
    }

    /// Adds an entire existing code [group] as a child of this one.
    pub fn group(&mut self, group: Rc<GroupCode<'p>>) {
        self.children.push(Code::Group(group));
    }

    /// Mark the selection start as occurring [offset] characters after the
    /// code that has already been written.
    pub fn start_selection(&mut self, offset: usize) {
        self.children.push(Code::Marker {
            marker: Marker::Start,
            offset,
        });
    }

    /// Mark the selection end as occurring [offset] characters after the code
    /// that has already been written.
    pub fn end_selection(&mut self, offset: usize) {
        self.children.push(Code::Marker {
            marker: Marker::End,
            offset,
        });
    }

    /// Disables or re-enables formatting in a region of code.
    pub fn set_formatting_enabled(&mut self, enabled: bool, source_offset: usize) {
        self.children.push(Code::EnableFormatting {
            enabled,
            source_offset,
        });
    }

    /// Traverse the [Code] tree and build the final formatted string.
    ///
    /// Whenever a newline is written, writes [line_ending].
    ///
    /// Returns the formatted string and the selection markers if there are
    /// any.
    pub fn build(&self, source: &SourceCode, line_ending: &str) -> SourceCode {
        let mut builder = StringBuilder {
            source,
            line_ending,
            buffer: String::with_capacity(source.text.len() + source.text.len() / 4),
            selection_start: None,
            selection_end: None,
            indent: 0,
            disable_formatting_start: None,
        };
        builder.traverse_group(self);
        builder.finish()
    }

    /// Write the [Code] to a string of output code, ignoring selection and
    /// format on/off markers.
    pub fn to_debug_string(&self) -> String {
        let mut buffer = String::new();
        let mut indent = 0;
        fn traverse(group: &GroupCode<'_>, buffer: &mut String, indent: &mut i32) {
            *indent = group.indent;
            for child in &group.children {
                match child {
                    Code::Newline {
                        blank,
                        indent: new_indent,
                    } => {
                        buffer.push('\n');
                        if *blank {
                            buffer.push('\n');
                        }
                        *indent = *new_indent;
                    }
                    Code::Text(text) => {
                        for _ in 0..*indent {
                            buffer.push(' ');
                        }
                        *indent = 0;
                        buffer.push_str(text);
                    }
                    Code::Group(group) => traverse(group, buffer, indent),
                    Code::Marker { .. } | Code::EnableFormatting { .. } => {}
                }
            }
        }
        traverse(self, &mut buffer, &mut indent);
        buffer
    }
}

/// Traverses a [Code] tree and produces the final string of output code and
/// the selection markers, if any.
struct StringBuilder<'s> {
    source: &'s SourceCode,
    line_ending: &'s str,
    buffer: String,

    /// The offset from the beginning of the source to where the selection
    /// start marker is, if there is one.
    selection_start: Option<usize>,

    /// The offset from the beginning of the source to where the selection end
    /// marker is, if there is one.
    selection_end: Option<usize>,

    /// How many spaces of indentation should be written before the next
    /// text.
    indent: i32,

    /// If formatting has been disabled, then this is the offset from the
    /// beginning of the source, to where the disabled formatting begins.
    ///
    /// Otherwise, `None` to indicate that formatting is enabled.
    disable_formatting_start: Option<usize>,
}

const SPACES: &str = "                                                                ";

impl StringBuilder<'_> {
    /// Dart `_buffer.length` (UTF-16 code units).
    fn buffer_length(&self) -> usize {
        utf16_len(&self.buffer)
    }

    fn traverse_group(&mut self, group: &GroupCode<'_>) {
        self.indent = group.indent;
        for child in &group.children {
            self.traverse(child);
        }
    }

    fn traverse(&mut self, code: &Code<'_>) {
        match code {
            Code::Newline { blank, indent } => {
                // If formatting has been disabled, then don't write the
                // formatted output. The unformatted output will be written
                // when formatting is re-enabled.
                if self.disable_formatting_start.is_none() {
                    self.buffer.push_str(self.line_ending);
                    if *blank {
                        self.buffer.push_str(self.line_ending);
                    }
                    self.indent = *indent;
                }
            }

            Code::Text(text) => {
                // If formatting has been disabled, then don't write the
                // formatted output. The unformatted output will be written
                // when formatting is re-enabled.
                if self.disable_formatting_start.is_none() {
                    // Write any pending indentation.
                    let mut indent = self.indent.max(0) as usize;
                    while indent > 0 {
                        let n = indent.min(SPACES.len());
                        self.buffer.push_str(&SPACES[..n]);
                        indent -= n;
                    }
                    self.indent = 0;

                    self.buffer.push_str(text);
                }
            }

            Code::Group(group) => self.traverse_group(group),

            Code::Marker { marker, offset } => {
                if let Some(disable_formatting_start) = self.disable_formatting_start {
                    // The marker appears inside a region where formatting is
                    // disabled. In that case, calculating where the marker
                    // will end up in the final formatted output is more
                    // complicated because we haven't actually written any of
                    // the code between the `// dart format off` comment and
                    // this marker to [_buffer] yet. However, we do know the
                    // *absolute* position of the selection markers in the
                    // original source.
                    let source_start = self.source.selection_start.unwrap();
                    match marker {
                        Marker::Start => {
                            // Calculate how far into the unformatted code
                            // where the marker should appear.
                            let marker_offset_in_unformatted =
                                source_start as i64 - disable_formatting_start as i64;
                            self.selection_start = Some(
                                (self.buffer_length() as i64 + marker_offset_in_unformatted)
                                    as usize,
                            );
                        }
                        Marker::End => {
                            let end = source_start + self.source.selection_length.unwrap();

                            // Calculate how far into the unformatted code
                            // where the marker should appear.
                            let marker_offset_in_unformatted =
                                end as i64 - disable_formatting_start as i64;
                            self.selection_end = Some(
                                (self.buffer_length() as i64 + marker_offset_in_unformatted)
                                    as usize,
                            );
                        }
                    }
                } else {
                    // Calculate the absolute offset from the beginning of the
                    // formatted output where the selection marker will appear
                    // based on how much formatted output we've written,
                    // pending indentation, and then the relative offset of
                    // the marker into the subsequent [Code] we will write.
                    let absolute_position =
                        self.buffer_length() + self.indent.max(0) as usize + offset;
                    match marker {
                        Marker::Start => self.selection_start = Some(absolute_position),
                        Marker::End => self.selection_end = Some(absolute_position),
                    }
                }
            }

            Code::EnableFormatting {
                enabled: false,
                source_offset,
            } => {
                // Region markers don't nest. If we've already turned off
                // formatting, then ignore any subsequent `// dart format off`
                // comments until it's been turned back on.
                if self.disable_formatting_start.is_none() {
                    self.disable_formatting_start = Some(*source_offset);
                }
            }

            Code::EnableFormatting {
                enabled: true,
                source_offset,
            } => {
                // If we didn't disable formatting, then enabling it does
                // nothing.
                if let Some(disable_formatting_start) = self.disable_formatting_start {
                    // Write all of the unformatted text from the
                    // `// dart format off` comment to the end of the
                    // `// dart format on` comment.
                    self.buffer.push_str(substring_utf16(
                        &self.source.text,
                        disable_formatting_start,
                        *source_offset,
                    ));
                    self.disable_formatting_start = None;
                }
            }
        }
    }

    fn finish(mut self) -> SourceCode {
        if let Some(disable_formatting_start) = self.disable_formatting_start {
            // Formatting was disabled and never re-enabled, so write the rest
            // of the source file as unformatted text.
            self.buffer.push_str(substring_utf16(
                &self.source.text,
                disable_formatting_start,
                usize::MAX,
            ));
        } else if self.source.is_compilation_unit {
            // Be a good citizen, end with a newline.
            self.buffer.push_str(self.line_ending);
        }

        let mut selection_start = self.selection_start;
        let mut selection_length = None;
        if self.source.selection_start.is_some() {
            // If we haven't hit the beginning and/or end of the selection yet,
            // they must be at the very end of the code.
            let length = self.buffer_length();
            let start = *selection_start.get_or_insert(length);
            let selection_end = self.selection_end.unwrap_or(length);
            selection_length = Some(selection_end.saturating_sub(start));
        }

        SourceCode {
            uri: self.source.uri.clone(),
            text: self.buffer,
            is_compilation_unit: self.source.is_compilation_unit,
            selection_start,
            selection_length,
        }
    }
}
