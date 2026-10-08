// Ported from package:yaml 3.1.4 lib/src/loader.dart.
// Copyright (c) 2014, the Dart project authors. MIT (see ../LICENSE).

use crate::Scalar;

pub(crate) fn parse_null(value: &str) -> Option<Scalar> {
    matches!(value, "" | "null" | "Null" | "NULL" | "~").then_some(Scalar::Null)
}

pub(crate) fn parse_bool(value: &str) -> Option<Scalar> {
    match value {
        "true" | "True" | "TRUE" => Some(Scalar::Bool(true)),
        "false" | "False" | "FALSE" => Some(Scalar::Bool(false)),
        _ => None,
    }
}

/// `_tryParseScalar` of `package:yaml`, falling back to a string.
pub(crate) fn parse_plain_scalar(value: &str) -> Scalar {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Scalar::Null;
    }
    let length = value.chars().count();
    let parsed = match bytes[0] {
        b'.' | b'+' | b'-' => parse_number(value, true, true),
        b'n' | b'N' if length == 4 => parse_null(value),
        b't' | b'T' if length == 4 => parse_bool(value),
        b'f' | b'F' if length == 5 => parse_bool(value),
        b'~' if length == 1 => Some(Scalar::Null),
        b'0'..=b'9' => parse_number(value, true, true),
        _ => None,
    };
    parsed.unwrap_or_else(|| Scalar::String(value.to_string()))
}

/// `_parseNumberValue` of `package:yaml`.
pub(crate) fn parse_number(contents: &str, allow_int: bool, allow_float: bool) -> Option<Scalar> {
    let bytes = contents.as_bytes();
    let first = *bytes.first()?;
    if allow_int && bytes.len() == 1 {
        return first
            .is_ascii_digit()
            .then(|| Scalar::Int((first - b'0') as i64));
    }
    let second = *bytes.get(1)?;
    if allow_int && first == b'0' {
        if second == b'x' {
            return i64::from_str_radix(&contents[2..], 16)
                .ok()
                .map(Scalar::Int);
        }
        if second == b'o' {
            return i64::from_str_radix(&contents[2..], 8).ok().map(Scalar::Int);
        }
    }
    if first.is_ascii_digit() || ((first == b'+' || first == b'-') && second.is_ascii_digit()) {
        if allow_int && let Some(value) = dart_int_try_parse(contents) {
            return Some(Scalar::Int(value));
        }
        if allow_float {
            return dart_double_try_parse(contents).map(Scalar::Float);
        }
        return None;
    }
    if !allow_float {
        return None;
    }
    if (first == b'.' && second.is_ascii_digit())
        || ((first == b'-' || first == b'+') && second == b'.')
    {
        if bytes.len() == 5 {
            match contents {
                "+.inf" | "+.Inf" | "+.INF" => return Some(Scalar::Float(f64::INFINITY)),
                "-.inf" | "-.Inf" | "-.INF" => return Some(Scalar::Float(f64::NEG_INFINITY)),
                _ => {}
            }
        }
        return dart_double_try_parse(contents).map(Scalar::Float);
    }
    if bytes.len() == 4 && first == b'.' {
        return match contents {
            ".inf" | ".Inf" | ".INF" => Some(Scalar::Float(f64::INFINITY)),
            ".nan" | ".NaN" | ".NAN" => Some(Scalar::Float(f64::NAN)),
            _ => None,
        };
    }
    None
}

/// `int.tryParse(contents, radix: 10)`: optional sign, decimal digits.
fn dart_int_try_parse(contents: &str) -> Option<i64> {
    let digits = contents.strip_prefix(['+', '-']).unwrap_or(contents);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    contents.parse::<i64>().ok()
}

/// `double.tryParse`: a decimal floating point literal, or `NaN`/`Infinity`.
fn dart_double_try_parse(contents: &str) -> Option<f64> {
    let body = contents.strip_prefix(['+', '-']).unwrap_or(contents);
    match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => {
            return Some(if contents.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }
        _ => {}
    }
    let valid = body
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !valid {
        return None;
    }
    contents.parse::<f64>().ok()
}
