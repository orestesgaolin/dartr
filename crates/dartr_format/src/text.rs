// Dart source: none (Rust helpers for Dart `String` semantics).

//! Dart strings are sequences of UTF-16 code units: `String.length`, offsets
//! and `substring` count code units. The formatter measures line widths with
//! `length`, and selection offsets are code unit offsets. Rust strings are
//! UTF-8, so the formatter uses these helpers wherever the Dart code uses
//! `length` or offsets.

/// Dart `string.length`: the number of UTF-16 code units of [s].
#[inline]
pub fn utf16_len(s: &str) -> usize {
    if s.is_ascii() {
        return s.len();
    }
    s.chars().map(char::len_utf16).sum()
}

/// The byte offset in [s] of the UTF-16 code unit offset [offset]. An offset
/// inside a surrogate pair maps to the start of the character. An offset past
/// the end maps to `s.len()`.
pub fn byte_offset_of_utf16(s: &str, offset: usize) -> usize {
    if s.is_ascii() {
        return offset.min(s.len());
    }
    let mut units = 0;
    for (byte, c) in s.char_indices() {
        if units >= offset {
            return byte;
        }
        units += c.len_utf16();
        if units > offset {
            return byte;
        }
    }
    s.len()
}

/// Dart `s.substring(start, end)` with UTF-16 code unit offsets.
pub fn substring_utf16(s: &str, start: usize, end: usize) -> &str {
    let b0 = byte_offset_of_utf16(s, start);
    let b1 = byte_offset_of_utf16(s, end);
    &s[b0..b1]
}

/// Maps UTF-16 offsets of one source text to byte offsets, in O(1) for ASCII
/// text and O(log n) otherwise.
#[derive(Clone, Debug)]
pub struct Utf16Index {
    /// `None` when the text is ASCII (offsets are equal). Otherwise, for each
    /// non-ASCII character: (utf16 offset after it, byte offset after it).
    marks: Option<Vec<(u32, u32)>>,
    len_bytes: usize,
    len_utf16: usize,
}

impl Utf16Index {
    pub fn new(s: &str) -> Utf16Index {
        if s.is_ascii() {
            return Utf16Index {
                marks: None,
                len_bytes: s.len(),
                len_utf16: s.len(),
            };
        }
        let mut marks = Vec::new();
        let mut units = 0u32;
        for (byte, c) in s.char_indices() {
            units += c.len_utf16() as u32;
            if !c.is_ascii() {
                marks.push((units, (byte + c.len_utf8()) as u32));
            }
        }
        Utf16Index {
            marks: Some(marks),
            len_bytes: s.len(),
            len_utf16: units as usize,
        }
    }

    /// The length of the text in UTF-16 code units.
    pub fn len_utf16(&self) -> usize {
        self.len_utf16
    }

    /// The byte offset of UTF-16 offset [offset] (clamped to the text).
    pub fn byte_offset(&self, offset: usize) -> usize {
        let Some(marks) = &self.marks else {
            return offset.min(self.len_bytes);
        };
        if offset >= self.len_utf16 {
            return self.len_bytes;
        }
        // The last mark at or before `offset`.
        let i = marks.partition_point(|&(u, _)| (u as usize) <= offset);
        if i == 0 {
            return offset;
        }
        let (u, b) = marks[i - 1];
        b as usize + (offset - u as usize)
    }

    /// The UTF-16 offset of byte offset [byte] (which must be a char
    /// boundary).
    pub fn utf16_offset(&self, byte: usize) -> usize {
        let Some(marks) = &self.marks else {
            return byte;
        };
        let i = marks.partition_point(|&(_, b)| (b as usize) <= byte);
        if i == 0 {
            return byte;
        }
        let (u, b) = marks[i - 1];
        u as usize + (byte - b as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trip() {
        let s = "aé😀b\nc";
        let index = Utf16Index::new(s);
        assert_eq!(index.len_utf16(), utf16_len(s));
        assert_eq!(index.byte_offset(0), 0);
        assert_eq!(index.byte_offset(1), 1);
        assert_eq!(index.byte_offset(2), 3);
        assert_eq!(index.byte_offset(4), 7);
        assert_eq!(index.byte_offset(5), 8);
        assert_eq!(index.utf16_offset(7), 4);
        assert_eq!(substring_utf16(s, 2, 5), "😀b");
        assert_eq!(byte_offset_of_utf16(s, 4), 7);
    }
}
