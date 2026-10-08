// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/quote.dart

//! String literal unescaping (Dart `quote.dart`), used by the AST builder to
//! compute the values of string literals.
//!
//! # Differences from Dart
//!
//! - Dart strings are UTF-16. The functions here take and return Rust
//!   strings, but all indices and lengths are computed on UTF-16 code units
//!   as in Dart: the offsets and lengths given to the error callback are
//!   UTF-16 code unit offsets into the string that `unescape` received
//!   (the same values Dart passes to `handleUnescapeError`).
//! - Dart `UnescapeErrorListener listener` + `Object location` is one
//!   callback `listener: &mut impl FnMut(CfeMessage, u32, u32)`. Dart
//!   `listener.handleUnescapeError(message, location, offset, length)` is
//!   `listener(message, offset, length)`; the caller captures `location`
//!   in the closure (for example the string token, so that the error
//!   offset is `token.charOffset + offset` as in Dart `StackListener`).
//! - Unpaired surrogates. An escape like `\uD800` (or `\u{D800}`) gives a
//!   lone surrogate code unit in the Dart string. Two escapes that give a
//!   high and a low surrogate (`😀`) give a valid surrogate pair
//!   in Dart, which is the same string as the character U+1F600; this port
//!   gives the same result. A Rust `String` cannot hold a lone surrogate:
//!   the `String` results replace each lone surrogate with U+FFFD (the
//!   result of `String::from_utf16_lossy`). This is also what Dart gives
//!   when it encodes such a string as UTF-8 (`utf8.encode`). Use
//!   [`unescape_code_units_utf16`] to get the exact Dart code units.
//! - Dart `considerCanonicalizeString(result)` only interns the string; it
//!   has no effect on the value and is not ported.

use std::borrow::Cow;

use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::characters::{
    BACKSLASH, BS, CLOSE_CURLY_BRACKET, CR, FF, LC_b, LC_f, LC_n, LC_r, LC_t, LC_u, LC_v, LC_x, LF,
    OPEN_CURLY_BRACKET, SPACE, TAB, VTAB, hex_digit_value, is_hex_digit,
};

/// Dart (line 34): `enum Quote`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Quote {
    Single,
    Double,
    MultiLineSingle,
    MultiLineDouble,
    RawSingle,
    RawDouble,
    RawMultiLineSingle,
    RawMultiLineDouble,
}

/// Dart (line 45): `Quote analyzeQuote(String first)`.
pub fn analyze_quote(first: &str) -> Quote {
    if first.starts_with("\"\"\"") {
        return Quote::MultiLineDouble;
    }
    if first.starts_with("r\"\"\"") {
        return Quote::RawMultiLineDouble;
    }
    if first.starts_with("'''") {
        return Quote::MultiLineSingle;
    }
    if first.starts_with("r'''") {
        return Quote::RawMultiLineSingle;
    }
    if first.starts_with('"') {
        return Quote::Double;
    }
    if first.starts_with("r\"") {
        return Quote::RawDouble;
    }
    if first.starts_with('\'') {
        return Quote::Single;
    }
    if first.starts_with("r'") {
        return Quote::RawSingle;
    }
    panic!("Unsupported operation: '{first}' in analyzeQuote");
}

/// Dart (line 59): `int lengthOfOptionalWhitespacePrefix(String first, int start)`.
///
/// Note: based on [StringValidator.quotingFromString]
/// (pkg/compiler/lib/src/string_validator.dart).
///
/// [start] and the result are UTF-16 code unit offsets.
pub fn length_of_optional_whitespace_prefix(first: &str, start: usize) -> usize {
    let code_units: Vec<u16> = first.encode_utf16().collect();
    let mut i = start;
    while i < code_units.len() {
        let mut code = code_units[i] as i32;
        if code == BACKSLASH {
            i += 1;
            if i < code_units.len() {
                code = code_units[i] as i32;
            } else {
                break;
            }
        }
        if code == TAB || code == SPACE {
            i += 1;
            continue;
        }
        if code == CR {
            if i + 1 < code_units.len() && code_units[i + 1] as i32 == LF {
                i += 1;
            }
            return i + 1;
        }
        if code == LF {
            return i + 1;
        }
        break; // Not a white-space character.
    }
    start
}

/// Dart (line 86): `int firstQuoteLength(String first, Quote quote)`.
///
/// The result is a UTF-16 code unit count.
pub fn first_quote_length(first: &str, quote: Quote) -> usize {
    match quote {
        Quote::Single | Quote::Double => 1,

        Quote::MultiLineSingle | Quote::MultiLineDouble => {
            length_of_optional_whitespace_prefix(first, /* start = */ 3)
        }

        Quote::RawSingle | Quote::RawDouble => 2,

        Quote::RawMultiLineSingle | Quote::RawMultiLineDouble => {
            length_of_optional_whitespace_prefix(first, /* start = */ 4)
        }
    }
}

/// Dart (line 106): `int lastQuoteLength(Quote quote)`.
pub fn last_quote_length(quote: Quote) -> usize {
    match quote {
        Quote::Single | Quote::Double | Quote::RawSingle | Quote::RawDouble => 1,

        Quote::MultiLineSingle
        | Quote::MultiLineDouble
        | Quote::RawMultiLineSingle
        | Quote::RawMultiLineDouble => 3,
    }
}

/// Dart (line 122): `String unescapeFirstStringPart(String first, Quote quote,
/// Object location, UnescapeErrorListener listener)`.
pub fn unescape_first_string_part(
    first: &str,
    quote: Quote,
    listener: &mut impl FnMut(CfeMessage, u32, u32),
) -> String {
    let start = first_quote_length(first, quote);
    unescape(&substring(first, start, None), quote, listener)
}

/// Dart (line 136): `String unescapeLastStringPart(String last, Quote quote,
/// Object location, bool isLastQuoteSynthetic, UnescapeErrorListener
/// listener)`.
pub fn unescape_last_string_part(
    last: &str,
    quote: Quote,
    is_last_quote_synthetic: bool,
    listener: &mut impl FnMut(CfeMessage, u32, u32),
) -> String {
    let length = utf16_length(last);
    let quote_length = if is_last_quote_synthetic {
        0
    } else {
        last_quote_length(quote)
    };
    // Dart `substring` throws a `RangeError` when `end` is negative.
    let end = length.checked_sub(quote_length).unwrap_or_else(|| {
        panic!(
            "RangeError: end {} is negative",
            length as i64 - quote_length as i64
        )
    });
    unescape(
        &substring(last, /* start = */ 0, Some(end)),
        quote,
        listener,
    )
}

/// Dart (line 152): `String unescapeString(String string, Object location,
/// UnescapeErrorListener listener)`.
pub fn unescape_string(string: &str, listener: &mut impl FnMut(CfeMessage, u32, u32)) -> String {
    let quote = analyze_quote(string);
    let start_index = first_quote_length(string, quote) as i64;
    let end_index = utf16_length(string) as i64 - last_quote_length(quote) as i64;
    if start_index > end_index {
        // An error has already been signaled.
        return String::new();
    }
    unescape(
        &substring(string, start_index as usize, Some(end_index as usize)),
        quote,
        listener,
    )
}

/// Dart (line 172): `String unescape(String string, Quote quote, Object
/// location, UnescapeErrorListener listener)`.
pub fn unescape(
    string: &str,
    quote: Quote,
    listener: &mut impl FnMut(CfeMessage, u32, u32),
) -> String {
    let result: String = match quote {
        Quote::Single | Quote::Double => {
            if !string.contains('\\') {
                string.to_owned()
            } else {
                unescape_code_units(&utf16(string), /* isRaw = */ false, listener)
            }
        }
        Quote::MultiLineSingle | Quote::MultiLineDouble => {
            if !string.contains('\\') && !string.contains('\r') {
                string.to_owned()
            } else {
                unescape_code_units(&utf16(string), /* isRaw = */ false, listener)
            }
        }
        Quote::RawSingle | Quote::RawDouble => string.to_owned(),
        Quote::RawMultiLineSingle | Quote::RawMultiLineDouble => {
            if !string.contains('\r') {
                string.to_owned()
            } else {
                unescape_code_units(&utf16(string), /* isRaw = */ true, listener)
            }
        }
    };
    // Dart: `return considerCanonicalizeString(result);` (interning only).
    result
}

/// Dart (line 223): `String unescapeCodeUnits(List<int> codeUnits, bool isRaw,
/// Object location, UnescapeErrorListener listener)`.
///
/// Lone surrogates in the result are replaced with U+FFFD (see the module
/// documentation); [`unescape_code_units_utf16`] gives the exact code units.
pub fn unescape_code_units(
    code_units: &[u16],
    is_raw: bool,
    listener: &mut impl FnMut(CfeMessage, u32, u32),
) -> String {
    String::from_utf16_lossy(&unescape_code_units_utf16(code_units, is_raw, listener))
}

/// Dart (line 223): `unescapeCodeUnits`, with the result as the UTF-16 code
/// units of the Dart string (lone surrogates are kept).
///
/// Note: based on
/// [StringValidator.validateString](pkg/compiler/lib/src/string_validator.dart).
pub fn unescape_code_units_utf16(
    code_units: &[u16],
    is_raw: bool,
    listener: &mut impl FnMut(CfeMessage, u32, u32),
) -> Vec<u16> {
    let len = code_units.len();
    let at = |i: usize| code_units[i] as i32;
    // Can't use Uint8List or Uint16List here, the code units may be larger.
    // (Dart `List<int>.filled(codeUnits.length, 0)`; here a `Vec` that
    // grows to `resultOffset`.)
    let mut result: Vec<i32> = Vec::with_capacity(len);

    let mut i: usize = 0;
    while i < len {
        let mut code = at(i);
        if code == CR {
            if i + 1 < len && at(i + 1) == LF {
                i += 1;
            }
            code = LF;
        } else if !is_raw && code == BACKSLASH {
            i += 1;
            if len == i {
                // This should only be reachable in error cases.
                listener(
                    diag::invalid_escape_started(),
                    i as u32,
                    /* length = */ 1,
                );
                return code_units.to_vec();
            }
            code = at(i);

            // `\n` for newline, equivalent to `\x0A`.
            // `\r` for carriage return, equivalent to `\x0D`.
            // `\f` for form feed, equivalent to `\x0C`.
            // `\b` for backspace, equivalent to `\x08`.
            // `\t` for tab, equivalent to `\x09`.
            // `\v` for vertical tab, equivalent to `\x0B`.
            // `\xXX` for hex escape.
            // `\uXXXX` or `\u{XX?X?X?X?X?}` for Unicode hex escape.
            if code == LC_n {
                code = LF;
            } else if code == LC_r {
                code = CR;
            } else if code == LC_f {
                code = FF;
            } else if code == LC_b {
                code = BS;
            } else if code == LC_t {
                code = TAB;
            } else if code == LC_v {
                code = VTAB;
            } else if code == LC_x {
                // Expect exactly 2 hex digits.
                let begin = i;
                if len <= i + 2 {
                    listener(
                        diag::invalid_hex_escape(),
                        begin as u32,
                        (len + 1 - begin) as u32,
                    );
                    return code_units.to_vec();
                }
                code = 0;
                for _j in 0..2 {
                    i += 1;
                    let digit = at(i);
                    if !is_hex_digit(digit) {
                        listener(
                            diag::invalid_hex_escape(),
                            begin as u32,
                            (i + 1 - begin) as u32,
                        );
                        return code_units.to_vec();
                    }
                    code = (code << 4) + hex_digit_value(digit);
                }
            } else if code == LC_u {
                let begin = i;
                if len == i + 1 {
                    listener(
                        diag::invalid_unicode_escape_u_started(),
                        begin as u32,
                        (len + 1 - begin) as u32,
                    );
                    return code_units.to_vec();
                }
                code = at(i + 1);
                let mut found_end_bracket = false;
                if code == OPEN_CURLY_BRACKET {
                    // Expect 1-6 hex digits followed by '}'.
                    i += 1;
                    if len == i {
                        listener(
                            diag::invalid_unicode_escape_u_bracket(),
                            begin as u32,
                            (i + 1 - begin) as u32,
                        );
                        return code_units.to_vec();
                    }
                    code = 0;
                    for j in 0..7 {
                        i += 1;
                        if len == i {
                            listener(
                                diag::invalid_unicode_escape_u_bracket(),
                                begin as u32,
                                (i + 1 - begin) as u32,
                            );
                            return code_units.to_vec();
                        }
                        let digit = at(i);
                        if j != 0 && digit == CLOSE_CURLY_BRACKET {
                            found_end_bracket = true;
                            break;
                        } else if j == 6 {
                            break;
                        }
                        if !is_hex_digit(digit) {
                            listener(
                                diag::invalid_unicode_escape_u_bracket(),
                                begin as u32,
                                (i + 2 - begin) as u32,
                            );
                            return code_units.to_vec();
                        }
                        code = (code << 4) + hex_digit_value(digit);
                    }
                    if !found_end_bracket {
                        listener(
                            diag::invalid_unicode_escape_u_bracket(),
                            begin as u32,
                            (i + 1 - begin) as u32,
                        );
                    }
                } else {
                    // Expect exactly 4 hex digits.
                    if len <= i + 4 {
                        listener(
                            diag::invalid_unicode_escape_u_no_bracket(),
                            begin as u32,
                            (len + 1 - begin) as u32,
                        );
                        return code_units.to_vec();
                    }
                    code = 0;
                    for _j in 0..4 {
                        i += 1;
                        let digit = at(i);
                        if !is_hex_digit(digit) {
                            listener(
                                diag::invalid_unicode_escape_u_no_bracket(),
                                begin as u32,
                                (i + 1 - begin) as u32,
                            );
                            return code_units.to_vec();
                        }
                        code = (code << 4) + hex_digit_value(digit);
                    }
                }
                if code > 0x10FFFF {
                    listener(
                        diag::invalid_code_point(),
                        begin as u32,
                        (i + 1 - begin) as u32,
                    );
                    return code_units.to_vec();
                }
            } else {
                // Nothing, escaped character is passed through;
            }
        }
        result.push(code);
        i += 1;
    }
    from_char_codes(&result)
}

/// Dart `new String.fromCharCodes(charCodes)`: a code above 0xFFFF becomes
/// a surrogate pair, other codes become one code unit (lone surrogates
/// stay).
fn from_char_codes(char_codes: &[i32]) -> Vec<u16> {
    let mut out = Vec::with_capacity(char_codes.len());
    for &code in char_codes {
        debug_assert!((0..=0x10FFFF).contains(&code));
        if code > 0xFFFF {
            let c = (code - 0x10000) as u32;
            out.push((0xD800 + (c >> 10)) as u16);
            out.push((0xDC00 + (c & 0x3FF)) as u16);
        } else {
            out.push(code as u16);
        }
    }
    out
}

/// Dart `string.codeUnits`.
fn utf16(string: &str) -> Vec<u16> {
    string.encode_utf16().collect()
}

/// Dart `string.length` (UTF-16 code units).
fn utf16_length(string: &str) -> usize {
    if string.is_ascii() {
        string.len()
    } else {
        string.chars().map(char::len_utf16).sum()
    }
}

/// Dart `string.substring(start, end)` with UTF-16 offsets. When an offset
/// falls between the two code units of a surrogate pair (Dart then makes a
/// lone surrogate), the lone surrogate becomes U+FFFD.
fn substring(string: &str, start: usize, end: Option<usize>) -> Cow<'_, str> {
    if string.is_ascii() {
        let end = end.unwrap_or(string.len());
        assert!(
            start <= end && end <= string.len(),
            "RangeError: substring({start}, {end})"
        );
        return Cow::Borrowed(&string[start..end]);
    }
    let byte_start = utf16_to_byte_offset(string, start);
    let byte_end = match end {
        None => Some(string.len()),
        Some(end) => utf16_to_byte_offset(string, end),
    };
    match (byte_start, byte_end) {
        (Some(s), Some(e)) => {
            assert!(s <= e, "RangeError: substring({start}, {end:?})");
            Cow::Borrowed(&string[s..e])
        }
        _ => {
            let units = utf16(string);
            let end = end.unwrap_or(units.len());
            assert!(
                start <= end && end <= units.len(),
                "RangeError: substring({start}, {end})"
            );
            Cow::Owned(String::from_utf16_lossy(&units[start..end]))
        }
    }
}

/// The byte offset of the UTF-16 offset [offset] in [string], or `None`
/// when the offset is inside a surrogate pair. Panics (Dart `RangeError`)
/// when the offset is past the end.
fn utf16_to_byte_offset(string: &str, offset: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, c) in string.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += c.len_utf16();
        if units > offset {
            return None;
        }
    }
    assert!(units == offset, "RangeError: {offset} is out of range");
    Some(string.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs Dart `unescapeString(input)` and returns the UTF-16 code units
    /// of the result and the errors `(code name, offset, length)`.
    fn run(input: &str) -> (Vec<u16>, Vec<(String, u32, u32)>) {
        let mut errors = Vec::new();
        let result = unescape_string(input, &mut |m: CfeMessage, o, l| {
            errors.push((m.code.name.to_string(), o, l))
        });
        (result.encode_utf16().collect(), errors)
    }

    /// Like [run], but keeps lone surrogates (`unescapeCodeUnits` on the
    /// part between the quotes of a single quoted string).
    fn run_units(input: &str) -> (Vec<u16>, Vec<(String, u32, u32)>) {
        let mut errors = Vec::new();
        let quote = analyze_quote(input);
        let start = first_quote_length(input, quote);
        let end = utf16_length(input) - last_quote_length(quote);
        let inner = substring(input, start, Some(end));
        let result =
            unescape_code_units_utf16(&utf16(&inner), false, &mut |m: CfeMessage, o, l| {
                errors.push((m.code.name.to_string(), o, l))
            });
        (result, errors)
    }

    fn check(input: &str, expected_units: &[u16], expected_errors: &[(&str, u32, u32)]) {
        let expected_errors: Vec<(String, u32, u32)> = expected_errors
            .iter()
            .map(|(n, o, l)| (n.to_string(), *o, *l))
            .collect();
        let has_lone_surrogate =
            char::decode_utf16(expected_units.iter().copied()).any(|r| r.is_err());
        if has_lone_surrogate {
            // The `String` result replaces lone surrogates with U+FFFD.
            let (units, errors) = run(input);
            let lossy: Vec<u16> = String::from_utf16_lossy(expected_units)
                .encode_utf16()
                .collect();
            assert_eq!(units, lossy, "input: {input:?}");
            assert_eq!(errors, expected_errors, "input: {input:?}");
            let (units, errors) = run_units(input);
            assert_eq!(units, expected_units, "input: {input:?}");
            assert_eq!(errors, expected_errors, "input: {input:?}");
        } else {
            let (units, errors) = run(input);
            assert_eq!(units, expected_units, "input: {input:?}");
            assert_eq!(errors, expected_errors, "input: {input:?}");
        }
    }

    #[test]
    fn analyze_quote_kinds() {
        assert_eq!(analyze_quote("'a'"), Quote::Single);
        assert_eq!(analyze_quote("\"a\""), Quote::Double);
        assert_eq!(analyze_quote("'''a'''"), Quote::MultiLineSingle);
        assert_eq!(analyze_quote("\"\"\"a\"\"\""), Quote::MultiLineDouble);
        assert_eq!(analyze_quote("r'a'"), Quote::RawSingle);
        assert_eq!(analyze_quote("r\"a\""), Quote::RawDouble);
        assert_eq!(analyze_quote("r'''a'''"), Quote::RawMultiLineSingle);
        assert_eq!(analyze_quote("r\"\"\"a\"\"\""), Quote::RawMultiLineDouble);
    }

    #[test]
    fn first_and_last_parts() {
        // Expected values from Dart (see `dart_vectors` below for the
        // method).
        let mut no_errors = |_: CfeMessage, _: u32, _: u32| panic!("unexpected error");
        assert_eq!(
            unescape_first_string_part("'''  \nab\\n${", Quote::MultiLineSingle, &mut no_errors),
            "ab\n${"
        );
        assert_eq!(
            unescape_first_string_part("'é\\t", Quote::Single, &mut no_errors),
            "é\t"
        );
        assert_eq!(
            unescape_last_string_part(
                "}x\\u{1F600}'''",
                Quote::MultiLineSingle,
                false,
                &mut no_errors
            ),
            "}x😀"
        );
        assert_eq!(
            unescape_last_string_part("}x\\x41", Quote::Single, true, &mut no_errors),
            "}xA"
        );
        assert_eq!(
            unescape_last_string_part("😀'", Quote::Single, false, &mut no_errors),
            "😀"
        );
    }

    // The expected values below were produced by Dart SDK 3.13.3 with
    // `unescapeString` of `package:_fe_analyzer_shared/src/parser/quote.dart`
    // (result as UTF-16 code units, errors as `(code name, offset,
    // length)`).
    #[test]
    fn unescape_string_matches_dart() {
        check("'abc'", &[0x61, 0x62, 0x63], &[]);
        check("'a\\nb'", &[0x61, 0xa, 0x62], &[]);
        check("'\\n\\r\\f\\b\\t\\v'", &[0xa, 0xd, 0xc, 0x8, 0x9, 0xb], &[]);
        check("'\\x41'", &[0x41], &[]);
        check("'\\x4'", &[0x5c, 0x78, 0x34], &[("InvalidHexEscape", 1, 3)]);
        check(
            "'\\xg1'",
            &[0x5c, 0x78, 0x67, 0x31],
            &[("InvalidHexEscape", 1, 2)],
        );
        check("'\\x'", &[0x5c, 0x78], &[("InvalidHexEscape", 1, 2)]);
        check(
            "'\\x4g'",
            &[0x5c, 0x78, 0x34, 0x67],
            &[("InvalidHexEscape", 1, 3)],
        );
        check("'\\u0041'", &[0x41], &[]);
        check(
            "'\\u004'",
            &[0x5c, 0x75, 0x30, 0x30, 0x34],
            &[("InvalidUnicodeEscapeUNoBracket", 1, 5)],
        );
        check(
            "'\\u00g1'",
            &[0x5c, 0x75, 0x30, 0x30, 0x67, 0x31],
            &[("InvalidUnicodeEscapeUNoBracket", 1, 4)],
        );
        check(
            "'\\u'",
            &[0x5c, 0x75],
            &[("InvalidUnicodeEscapeUStarted", 1, 2)],
        );
        check("'\\u{41}'", &[0x41], &[]);
        check(
            "'\\u{}'",
            &[0x5c, 0x75, 0x7b, 0x7d],
            &[("InvalidUnicodeEscapeUBracket", 1, 4)],
        );
        check("'\\u{1F600}'", &[0xd83d, 0xde00], &[]);
        check("'\\u{10FFFF}'", &[0xdbff, 0xdfff], &[]);
        check(
            "'\\u{110000}'",
            &[0x5c, 0x75, 0x7b, 0x31, 0x31, 0x30, 0x30, 0x30, 0x30, 0x7d],
            &[("InvalidCodePoint", 1, 9)],
        );
        check(
            "'\\u{1234567}'",
            &[
                0x5c, 0x75, 0x7b, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x7d,
            ],
            &[
                ("InvalidUnicodeEscapeUBracket", 1, 9),
                ("InvalidCodePoint", 1, 9),
            ],
        );
        check(
            "'\\u{FFFFFF}'",
            &[0x5c, 0x75, 0x7b, 0x46, 0x46, 0x46, 0x46, 0x46, 0x46, 0x7d],
            &[("InvalidCodePoint", 1, 9)],
        );
        check("'\\u{00000F}'", &[0xf], &[]);
        check(
            "'\\u{0000001}'",
            &[0x0, 0x7d],
            &[("InvalidUnicodeEscapeUBracket", 1, 9)],
        );
        check(
            "'\\u{12'",
            &[0x5c, 0x75, 0x7b, 0x31, 0x32],
            &[("InvalidUnicodeEscapeUBracket", 1, 5)],
        );
        check(
            "'\\u{'",
            &[0x5c, 0x75, 0x7b],
            &[("InvalidUnicodeEscapeUBracket", 1, 3)],
        );
        check(
            "'\\u{g}'",
            &[0x5c, 0x75, 0x7b, 0x67, 0x7d],
            &[("InvalidUnicodeEscapeUBracket", 1, 4)],
        );
        check(
            "'\\u{1g}'",
            &[0x5c, 0x75, 0x7b, 0x31, 0x67, 0x7d],
            &[("InvalidUnicodeEscapeUBracket", 1, 5)],
        );
        check("'\\u{0}'", &[0x0], &[]);
        check("'\\uD800'", &[0xd800], &[]);
        check("'\\uD83D\\uDE00'", &[0xd83d, 0xde00], &[]);
        check("'\\u{D83D}\\u{DE00}'", &[0xd83d, 0xde00], &[]);
        check("'\\uDE00\\uD83D'", &[0xde00, 0xd83d], &[]);
        check("'\\uD800x\\u{DFFF}'", &[0xd800, 0x78, 0xdfff], &[]);
        check("'a\\'", &[0x61, 0x5c], &[("InvalidEscapeStarted", 2, 1)]);
        check("'\\$'", &[0x24], &[]);
        check("'\\q'", &[0x71], &[]);
        check("'\\''", &[0x27], &[]);
        check("'\u{e9}\\x41'", &[0xe9, 0x41], &[]);
        check(
            "'\u{1f600}\\x4g'",
            &[0xd83d, 0xde00, 0x5c, 0x78, 0x34, 0x67],
            &[("InvalidHexEscape", 3, 3)],
        );
        check(
            "'\u{1f600}\\u{1F600}x'",
            &[0xd83d, 0xde00, 0xd83d, 0xde00, 0x78],
            &[],
        );
        check("'x\r\ny'", &[0x78, 0xd, 0xa, 0x79], &[]);
        check(
            "'''\r\n abc\r\ndef'''",
            &[0x20, 0x61, 0x62, 0x63, 0xa, 0x64, 0x65, 0x66],
            &[],
        );
        check("'''  \\\n  x'''", &[0x20, 0x20, 0x78], &[]);
        check(
            "''' \\t \r\nx\ry'''",
            &[0x20, 0x9, 0x20, 0xa, 0x78, 0xa, 0x79],
            &[],
        );
        check("'''x\\\r\ny'''", &[0x78, 0xd, 0xa, 0x79], &[]);
        check(
            "r'''\r\nab\rc\\n'''",
            &[0x61, 0x62, 0xa, 0x63, 0x5c, 0x6e],
            &[],
        );
        check("r'''  \n\\n'''", &[0x5c, 0x6e], &[]);
        check("r'''\\\n x'''", &[0x20, 0x78], &[]);
        check("r'a\\n'", &[0x61, 0x5c, 0x6e], &[]);
        check("r\"a\\n\"", &[0x61, 0x5c, 0x6e], &[]);
        check("''''", &[], &[]);
        check("'''\n'''", &[], &[]);
        check("\"\"\"\n\"\"\"", &[], &[]);
        check("r\"\"\"\n\\n\"\"\"", &[0x5c, 0x6e], &[]);
        check("\"\\u{1F600}\"", &[0xd83d, 0xde00], &[]);
        check("'''\\u{1F600}\r\n'''", &[0xd83d, 0xde00, 0xa], &[]);
        check(
            "'''\u{1f600}\\u{1F60G}'''",
            &[
                0xd83d, 0xde00, 0x5c, 0x75, 0x7b, 0x31, 0x46, 0x36, 0x30, 0x47, 0x7d,
            ],
            &[("InvalidUnicodeEscapeUBracket", 3, 8)],
        );
    }
}
