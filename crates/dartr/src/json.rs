//! A small JSON writer that writes exactly what Dart `jsonEncode` writes
//! (same escaping, keys in insertion order, no spaces).

use std::fmt::Write;

/// Writes [s] as a JSON string like Dart `jsonEncode`.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x20 && b != b'"' && b != b'\\' {
            continue;
        }
        out.push_str(&s[start..i]);
        start = i + 1;
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            _ => {
                let _ = write!(out, "\\u{:04x}", b);
            }
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_dart_json_encode() {
        let mut out = String::new();
        write_string(&mut out, "a\"b\\c\n\t\r\u{8}\u{c}\u{1}\u{1f}\u{7f}é😀/");
        assert_eq!(
            out,
            "\"a\\\"b\\\\c\\n\\t\\r\\b\\f\\u0001\\u001f\u{7f}é😀/\""
        );
    }
}
