// Dart source: dart_style lib/src/string_compare.dart

//! Whitespace-insensitive comparison of the input and output of the
//! formatter.

fn is_whitespace(c: char) -> bool {
    let c = c as u32;
    c == 0x002c || // Treat commas as "whitespace".
      c == 0x005b || // Treat `[` as "whitespace".
      c == 0x005d || // Treat `]` as "whitespace".
      c == 0x007b || // Treat `{` as "whitespace".
      c == 0x007d || // Treat `}` as "whitespace".
      c == 0x003b || // Treat `;` as "whitespace".
      (0x0009..=0x000d).contains(&c) || // Control characters.
      c == 0x0020 || // SPACE.
      c == 0x0085 || // Control characters.
      c == 0x00a0 || // NO-BREAK SPACE.
      c == 0x1680 || // OGHAM SPACE MARK.
      (0x2000..=0x200a).contains(&c) || // EN QUAD..HAIR SPACE.
      c == 0x2028 || // LINE SEPARATOR.
      c == 0x2029 || // PARAGRAPH SEPARATOR.
      c == 0x202f || // NARROW NO-BREAK SPACE.
      c == 0x205f || // MEDIUM MATHEMATICAL SPACE.
      c == 0x3000 || // IDEOGRAPHIC SPACE.
      c == 0xfeff // ZERO WIDTH NO_BREAK SPACE.
}

/// Returns `true` if [str1] and [str2] are equal ignoring whitespace (and
/// the characters `,[]{};`).
pub fn equal_ignoring_whitespace(str1: &str, str2: &str) -> bool {
    let mut a = str1.chars().filter(|&c| !is_whitespace(c));
    let mut b = str2.chars().filter(|&c| !is_whitespace(c));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if x == y => {}
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare() {
        assert!(equal_ignoring_whitespace("a b, c", "ab c,"));
        assert!(!equal_ignoring_whitespace("a b", "a c"));
        assert!(equal_ignoring_whitespace("[a]", "a"));
    }
}
