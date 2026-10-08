// Ported from package:source_span lib/src/file.dart and location.dart.
// Copyright (c) 2013, the Dart project authors. BSD-3-Clause.

/// Zero-based location in Dart UTF-16 code units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}
impl SourceLocation {
    pub fn point_span(self) -> FileSpan {
        FileSpan {
            start: self,
            end: self,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileSpan {
    pub start: SourceLocation,
    pub end: SourceLocation,
}
pub type SourceSpan = FileSpan;
impl FileSpan {
    pub fn length(self) -> usize {
        self.end.offset - self.start.offset
    }
    pub fn expand(self, other: Self) -> Self {
        Self {
            start: if self.start.offset <= other.start.offset {
                self.start
            } else {
                other.start
            },
            end: if self.end.offset >= other.end.offset {
                self.end
            } else {
                other.end
            },
        }
    }
}

/// The source-span line model treats LF and standalone CR as line breaks.
/// It deliberately does not treat NEL, LS or PS as line breaks.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub units: Vec<u16>,
    line_starts: Vec<usize>,
    byte_offsets: Vec<usize>,
}
impl SourceFile {
    pub fn new(text: &str) -> Self {
        let units: Vec<_> = text.encode_utf16().collect();
        let mut line_starts = vec![0];
        for (i, &c) in units.iter().enumerate() {
            if c == 10 || (c == 13 && units.get(i + 1) != Some(&10)) {
                line_starts.push(i + 1);
            }
        }
        let mut byte_offsets = Vec::with_capacity(units.len() + 1);
        for (i, c) in text.char_indices() {
            for _ in 0..c.len_utf16() {
                byte_offsets.push(i);
            }
        }
        byte_offsets.push(text.len());
        Self {
            units,
            line_starts,
            byte_offsets,
        }
    }
    pub fn location(&self, offset: usize) -> SourceLocation {
        assert!(offset <= self.units.len());
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        SourceLocation {
            offset,
            line,
            column: offset - self.line_starts[line],
        }
    }
    pub fn span(&self, start: usize, end: usize) -> FileSpan {
        assert!(start <= end);
        FileSpan {
            start: self.location(start),
            end: self.location(end),
        }
    }
    pub fn get_text(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.units[start..end])
    }
    pub fn byte_offset(&self, offset: usize) -> usize {
        self.byte_offsets[offset]
    }
    pub fn length(&self) -> usize {
        self.units.len()
    }
}
