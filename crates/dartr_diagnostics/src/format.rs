//! Message template formatting.
//!
//! Port of `formatList` in `pkg/_fe_analyzer_shared/lib/src/base/errors.dart`.

/// Inserts `arguments` into `pattern`, replacing each `{n}` with
/// `arguments[n]`.
///
/// `format_list("Hello, {0}!", &["John"]) == "Hello, John!"`
///
/// Same behavior as Dart: if `arguments` is empty, `pattern` is returned
/// unchanged; text such as `{}` or `{x}` is kept as is.
///
/// # Panics
///
/// If a placeholder index is out of range (Dart throws a `RangeError`).
pub fn format_list<S: AsRef<str>>(pattern: &str, arguments: &[S]) -> String {
    if arguments.is_empty() {
        return pattern.to_string();
    }
    let bytes = pattern.as_bytes();
    let mut buffer = String::with_capacity(pattern.len());
    let mut from = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let mut ch = bytes[i];
        i += 1;
        if ch == b'{' {
            let mut to = i - 1;
            let mut number: usize = 0;
            while i < bytes.len() {
                ch = bytes[i];
                i += 1;
                let digit = ch ^ b'0';
                if digit <= 9 {
                    number = number.wrapping_mul(10).wrapping_add(digit as usize);
                } else if ch == b'}' {
                    let end = i;
                    if end > to + 2 {
                        if to > from {
                            buffer.push_str(&pattern[from..to]);
                        }
                        buffer.push_str(arguments[number].as_ref());
                        from = i;
                    }
                    break;
                } else if ch == b'{' {
                    to = i - 1;
                    number = 0;
                } else {
                    break;
                }
            }
        }
    }
    if from == 0 {
        return pattern.to_string();
    }
    if from < bytes.len() {
        buffer.push_str(&pattern[from..]);
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::format_list;

    #[test]
    fn formats_like_dart() {
        assert_eq!(format_list("Hello, {0}!", &["John"]), "Hello, John!");
        assert_eq!(
            format_list("{0} are you {1}ing?", &["How", "do"]),
            "How are you doing?"
        );
        assert_eq!(format_list("a {} b {x} {{0}", &["z"]), "a {} b {x} {z");
        assert_eq!(format_list("{1}{0}", &["a", "b"]), "ba");
        assert_eq!(format_list("no args {0}", &[] as &[&str]), "no args {0}");
        assert_eq!(format_list("é{0}ü", &["x"]), "éxü");
    }
}
