// Dart source: the Dart VM number semantics that value.dart gets from the VM
// it runs on:
// - runtime/vm/double_conversion.cc (DoubleToCString: `double.toString()`),
// - runtime/lib/double.cc + runtime/vm/runtime_entry.cc (DartModulo:
//   `double.%`), sdk/lib/_internal/vm/lib/double.dart (`toInt`),
// - sdk/lib/_internal/vm/lib/integers.dart (`int` operators),
// - sdk/lib/_internal/vm_shared/lib/integers_patch.dart (`int.parse`),
// - sdk/lib/_internal/vm/lib/string_patch.dart (`_isWhitespace`).

//! Dart `int` (64-bit, wrapping) and `double` semantics.
//!
//! The analyzer runs on the Dart VM, so its constant arithmetic uses VM
//! `int` semantics (no JavaScript number semantics; the analyzer has no
//! JS-compat flag in value.dart). Every function here is checked against
//! the VM by `tests/differential.rs`.

use std::cmp::Ordering;

/// Dart `a ~/ b` for `int`s; `None` when `b == 0`
/// (`IntegerDivisionByZeroException`). `minInt ~/ -1` wraps to `minInt`.
pub fn int_truncating_div(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    Some(a.wrapping_div(b))
}

/// Dart `a % b` for `int`s (Euclidean modulo: the result is never
/// negative); `None` when `b == 0`.
pub fn int_modulo(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    let r = a.wrapping_rem(b);
    if r < 0 {
        // |r| < |b|, so this does not overflow.
        Some(if b < 0 { r - b } else { r + b })
    } else {
        Some(r)
    }
}

/// Dart `a.remainder(b)` for `int`s (truncating); `None` when `b == 0`.
pub fn int_remainder(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    Some(a.wrapping_rem(b))
}

/// Dart `a << b` for `b >= 0`.
pub fn int_shl(a: i64, b: i64) -> i64 {
    debug_assert!(b >= 0);
    if b >= 64 { 0 } else { a.wrapping_shl(b as u32) }
}

/// Dart `a >> b` for `b >= 0`.
pub fn int_shr(a: i64, b: i64) -> i64 {
    debug_assert!(b >= 0);
    if b >= 64 {
        if a < 0 { -1 } else { 0 }
    } else {
        a >> b
    }
}

/// Dart `a >>> b` for `b >= 0`.
pub fn int_ushr(a: i64, b: i64) -> i64 {
    debug_assert!(b >= 0);
    if b >= 64 { 0 } else { ((a as u64) >> b) as i64 }
}

/// Dart `int.bitLength`.
pub fn int_bit_length(a: i64) -> u32 {
    let v = if a < 0 { !a } else { a };
    64 - v.leading_zeros()
}

/// Dart `int.compareTo`.
pub fn int_compare(a: i64, b: i64) -> Ordering {
    a.cmp(&b)
}

/// Dart `int.toDouble()` (round to nearest, ties to even).
pub fn int_to_double(a: i64) -> f64 {
    a as f64
}

/// Dart `double.toInt()`; `None` for NaN and infinities
/// (`UnsupportedError`). Finite values outside the `int` range saturate,
/// as on the VM.
pub fn double_to_int(a: f64) -> Option<i64> {
    if !a.is_finite() {
        return None;
    }
    // `as` truncates toward zero and saturates.
    Some(a as i64)
}

/// Dart `a % b` for `double`s (DartModulo).
pub fn double_modulo(a: f64, b: f64) -> f64 {
    let mut remainder = a % b; // fmod
    if remainder == 0.0 {
        // The VM switches to the positive 0.0.
        remainder = 0.0;
    } else if remainder < 0.0 {
        if b < 0.0 {
            remainder -= b;
        } else {
            remainder += b;
        }
    }
    remainder
}

/// Dart `a.remainder(b)` for `double`s (fmod).
pub fn double_remainder(a: f64, b: f64) -> f64 {
    a % b
}

/// Dart `identical(a, b)` for two doubles: the same bits.
pub fn double_identical(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// Dart `double.toString()` on the VM: the shortest digits that round-trip,
/// laid out by double-conversion with `kDecimalLow = -6`,
/// `kDecimalHigh = 21` and the flags `EMIT_POSITIVE_EXPONENT_SIGN |
/// EMIT_TRAILING_DECIMAL_POINT | EMIT_TRAILING_ZERO_AFTER_POINT`.
///
/// `1.0`, `0.1`, `1e-7`, `0.000001`, `1e+21`, `123456789012345680000.0`,
/// `-0.0`, `NaN`, `Infinity`, `-Infinity`, `5e-324`.
pub fn double_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    if value == 0.0 {
        out.push_str("0.0");
        return out;
    }
    let (digits, exponent) = shortest_digits(value.abs());
    let n = digits.len() as i32;
    let decimal_point = exponent + 1;
    if (-6..21).contains(&exponent) {
        if decimal_point <= 0 {
            out.push_str("0.");
            for _ in 0..-decimal_point {
                out.push('0');
            }
            out.push_str(&digits);
        } else if decimal_point >= n {
            out.push_str(&digits);
            for _ in 0..decimal_point - n {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            let (int_part, frac_part) = digits.split_at(decimal_point as usize);
            out.push_str(int_part);
            out.push('.');
            out.push_str(frac_part);
        }
    } else {
        let (first, rest) = digits.split_at(1);
        out.push_str(first);
        if !rest.is_empty() {
            out.push('.');
            out.push_str(rest);
        }
        out.push('e');
        if exponent >= 0 {
            out.push('+');
        }
        out.push_str(&exponent.to_string());
    }
    out
}

/// Splits `{:e}` output (`d.ddde<exp>`) into the digits and the exponent.
fn split_sci(sci: &str) -> (String, i32) {
    let (mantissa, exp) = sci.split_once('e').expect("`{:e}` has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    (digits, exp.parse().expect("`{:e}` exponent is an integer"))
}

/// The shortest digits that round-trip to [value] (finite, positive) and the
/// decimal exponent of the first digit, as double-conversion's SHORTEST
/// mode gives them: of the shortest candidates, the closest one; an exact
/// tie rounds the last digit to even (bignum-dtoa: "round towards even,
/// since this is what Gay seems to do").
///
/// Rust's `{:e}` gives the shortest length, but rounds a tie up
/// (`2^-25` is `2.98023223876953125e-8`: Rust `...313e-8`, Dart
/// `...312e-8`). `{:.Ne}` rounds exactly with ties to even, so the digits
/// are taken from it when they round-trip at the same length.
fn shortest_digits(value: f64) -> (String, i32) {
    let shortest = split_sci(&format!("{value:e}"));
    let precision = shortest.0.len() - 1;
    let exact = format!("{value:.precision$e}");
    if exact.parse::<f64>() == Ok(value) {
        let rounded = split_sci(&exact);
        if rounded.0.len() == shortest.0.len() {
            return rounded;
        }
    }
    shortest
}

/// Dart `String._isWhitespace` on the VM (Unicode 6.3 White_Space and BOM).
pub fn is_dart_whitespace(code_unit: u16) -> bool {
    matches!(
        code_unit,
        0x09..=0x0D
            | 0x20
            | 0x85
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

/// Dart `int.parse(source)` without a radix on the VM; `None` for a
/// `FormatException`.
///
/// Leading and trailing whitespace is ignored; an optional sign; decimal
/// digits, or `0x`/`0X` and hex digits. Decimal values must be in the
/// 64-bit range; positive hex values may use all 64 bits (they wrap to a
/// negative `int`, `0xFFFFFFFFFFFFFFFF == -1`).
pub fn int_parse(source: &str) -> Option<i64> {
    let units: Vec<u16> = source.encode_utf16().collect();
    let start = units.iter().position(|&c| !is_dart_whitespace(c))?;
    let end = units.iter().rposition(|&c| !is_dart_whitespace(c))? + 1;
    let mut i = start;
    let mut negative = false;
    if units[i] == u16::from(b'+') || units[i] == u16::from(b'-') {
        negative = units[i] == u16::from(b'-');
        i += 1;
        if i == end {
            return None;
        }
    }
    let mut radix = 10u32;
    if units[i] == u16::from(b'0') {
        i += 1;
        if i == end {
            return Some(0);
        }
        if units[i] | 0x20 == u16::from(b'x') {
            i += 1;
            if i == end {
                return None;
            }
            radix = 16;
        }
    }
    // Accumulate the magnitude; anything above 2^64 is out of range.
    let mut magnitude: u128 = 0;
    for &c in &units[i..end] {
        let digit = char::from_u32(u32::from(c))?.to_digit(radix)?;
        magnitude = magnitude * u128::from(radix) + u128::from(digit);
        if magnitude > u128::from(u64::MAX) {
            magnitude = u128::from(u64::MAX) + 1;
        }
    }
    if negative {
        if magnitude > 1u128 << 63 {
            return None;
        }
        Some((magnitude as u64 as i64).wrapping_neg())
    } else if radix == 16 {
        if magnitude > u128::from(u64::MAX) {
            return None;
        }
        Some(magnitude as u64 as i64)
    } else {
        if magnitude > i64::MAX as u128 {
            return None;
        }
        Some(magnitude as i64)
    }
}

/// Dart `String.compareTo`: UTF-16 code unit order.
pub fn string_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Dart `String.length`: the number of UTF-16 code units.
pub fn string_length(s: &str) -> i64 {
    s.encode_utf16().count() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_to_string_layout() {
        let cases: &[(f64, &str)] = &[
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1e-7, "1e-7"),
            (1e-6, "0.000001"),
            (1.5e-7, "1.5e-7"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000.0"),
            (1.2345678901234568e20, "123456789012345680000.0"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (123.456, "123.456"),
            // Exact ties round the last digit to even.
            (2f64.powi(-25), "2.9802322387695312e-8"),
            (f64::from_bits(0x4302aeee03761efa), "657360858039263.2"),
        ];
        for (value, expected) in cases {
            assert_eq!(double_to_string(*value), *expected, "{value:e}");
        }
    }

    #[test]
    fn int_parse_ranges() {
        assert_eq!(int_parse(" 42 "), Some(42));
        assert_eq!(int_parse("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(int_parse("9223372036854775808"), None);
        assert_eq!(int_parse("0xFFFFFFFFFFFFFFFF"), Some(-1));
        assert_eq!(int_parse("-0x8000000000000000"), Some(i64::MIN));
        assert_eq!(int_parse("-0x8000000000000001"), None);
        assert_eq!(int_parse("0x"), None);
        assert_eq!(int_parse(""), None);
        assert_eq!(int_parse("1_000"), None);
    }
}
