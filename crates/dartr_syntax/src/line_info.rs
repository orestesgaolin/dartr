// Dart source: pkg/analyzer/lib/source/line_info.dart

//! Line starts of a file and the mapping from offsets to line and column
//! (Dart `LineInfo`, `CharacterLocation`). Offsets are UTF-16 code units,
//! like token offsets.

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A location in a file: one-based line and column numbers (Dart
/// `CharacterLocation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CharacterLocation {
    pub line_number: u32,
    pub column_number: u32,
}

impl fmt::Display for CharacterLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line_number, self.column_number)
    }
}

/// The offsets of the line starts of a file (Dart `LineInfo`).
#[derive(Debug, Default)]
pub struct LineInfo {
    /// The offset of the first character of each line (the first entry is
    /// 0).
    pub line_starts: Vec<u32>,
    /// Dart `_previousLine`: the index of the line found by the last
    /// [`LineInfo::get_location`]. Atomic (relaxed) so that a `LineInfo`
    /// can be shared between threads; it is only a search hint.
    previous_line: AtomicUsize,
}

impl Clone for LineInfo {
    fn clone(&self) -> Self {
        LineInfo {
            line_starts: self.line_starts.clone(),
            previous_line: AtomicUsize::new(self.previous_line.load(Ordering::Relaxed)),
        }
    }
}

impl PartialEq for LineInfo {
    fn eq(&self, other: &Self) -> bool {
        self.line_starts == other.line_starts
    }
}

impl Eq for LineInfo {}

impl LineInfo {
    /// Dart `LineInfo(lineStarts)`. Panics if [line_starts] is empty (Dart
    /// throws an `ArgumentError`).
    pub fn new(line_starts: Vec<u32>) -> LineInfo {
        assert!(!line_starts.is_empty(), "lineStarts must be non-empty");
        LineInfo {
            line_starts,
            previous_line: AtomicUsize::new(0),
        }
    }

    /// Dart `LineInfo.fromContent`: the line starts of [content] (`\n`,
    /// `\r\n` and `\r` end a line).
    pub fn from_content(content: &str) -> LineInfo {
        let mut line_starts = vec![0u32];
        let units: Vec<u16> = content.encode_utf16().collect();
        let length = units.len();
        let mut i = 0;
        while i < length {
            let unit = units[i];
            if unit > 0x0D {
                i += 1;
                continue;
            }
            if unit == 0x0D {
                // A following `\n` registers the line start.
                if !(i + 1 < length && units[i + 1] == 0x0A) {
                    line_starts.push(i as u32 + 1);
                }
            } else if unit == 0x0A {
                line_starts.push(i as u32 + 1);
            }
            i += 1;
        }
        LineInfo::new(line_starts)
    }

    /// Dart `lineCount`.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Dart `getLocation`: the one-based line and column of [offset].
    pub fn get_location(&self, offset: u32) -> CharacterLocation {
        let starts = &self.line_starts;
        let mut min = 0usize;
        let mut max = starts.len() - 1;
        let previous = self.previous_line.load(Ordering::Relaxed);
        if offset >= starts[previous] {
            min = previous;
            if min == starts.len() - 1 || offset < starts[min + 1] {
                return CharacterLocation {
                    line_number: min as u32 + 1,
                    column_number: offset - starts[min] + 1,
                };
            }
        }
        while min < max {
            let midpoint = (max - min + 1) / 2 + min;
            if starts[midpoint] > offset {
                max = midpoint - 1;
            } else {
                min = midpoint;
            }
        }
        self.previous_line.store(min, Ordering::Relaxed);
        CharacterLocation {
            line_number: min as u32 + 1,
            column_number: offset - starts[min] + 1,
        }
    }

    /// Dart `getOffsetOfLine`: the offset of the first character of the
    /// zero-based line [line_number].
    pub fn get_offset_of_line(&self, line_number: usize) -> Result<u32, String> {
        if line_number >= self.line_count() {
            return Err(format!(
                "Invalid line number: {line_number}; must be between 0 and {}",
                self.line_count() - 1
            ));
        }
        Ok(self.line_starts[line_number])
    }

    /// Dart `getOffsetOfLineAfter`.
    pub fn get_offset_of_line_after(&self, offset: u32) -> Result<u32, String> {
        self.get_offset_of_line(self.get_location(offset).line_number as usize)
    }

    /// Dart `lineNumberDifference`.
    pub fn line_number_difference(&self, offset1: u32, offset2: u32) -> i64 {
        self.get_location(offset2).line_number as i64
            - self.get_location(offset1).line_number as i64
    }

    /// Dart `onSameLine`.
    pub fn on_same_line(&self, offset1: u32, offset2: u32) -> bool {
        self.line_number_difference(offset1, offset2) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations() {
        let info = LineInfo::from_content("a\nbc\r\nd\re");
        assert_eq!(info.line_starts, vec![0, 2, 6, 8]);
        assert_eq!(info.get_location(0).to_string(), "1:1");
        assert_eq!(info.get_location(3).to_string(), "2:2");
        assert_eq!(info.get_location(9).to_string(), "4:2");
        assert_eq!(info.get_location(1).to_string(), "1:2");
        assert!(info.on_same_line(2, 4));
        assert_eq!(info.get_offset_of_line(2), Ok(6));
    }
}
