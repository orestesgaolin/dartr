//! Normalizes `saphyr-parser` 0.1.0 errors to `package:yaml` 3.1.3.
//!
//! Sources:
//! - `package:yaml/src/parser.dart` at version 3.1.3
//! - `package:yaml/src/scanner.dart` at version 3.1.3
//! - `$CARGO_HOME/registry/src/*/saphyr-parser-0.1.0/src/{parser,scanner}.rs`
//!
//! Saphyr exposes a single character-index marker. `yaml.rs` converts that
//! marker to a UTF-8 byte offset before calling this function. Package YAML
//! sometimes reports a different point or a non-empty span, so the mappings
//! below rescan only the malformed token involved in a confirmed difference.

use crate::yaml::{Span, YamlError};

pub(crate) fn normalize_error(text: &str, message: &str, span: Span) -> YamlError {
    let (message, span) = match message {
        "while parsing a node, did not find expected node content" => {
            ("Expected node content.", point(text, span.start))
        }
        "while parsing a flow sequence, expected ',' or ']'" => (
            "While parsing a flow sequence, expected ',' or ']'.",
            point(text, span.start),
        ),
        "while parsing a flow mapping, did not find expected ',' or '}'" => (
            "While parsing a flow mapping, expected ',' or '}'.",
            point(text, span.start),
        ),
        "while parsing a block collection, did not find expected '-' indicator" => (
            "While parsing a block collection, expected '-'.",
            point(text, span.start),
        ),
        "while parsing a block mapping, did not find expected key" => (
            "Expected a key while parsing a block mapping.",
            point(text, span.start),
        ),
        "simple key expect ':'" => ("Expected ':'.", point(text, span.start)),
        "while scanning a quoted scalar, found unexpected end of stream" => {
            ("Unexpected end of file.", point(text, text.len()))
        }
        "tabs disallowed within this context (block indentation)" => {
            let offset = indentation_tab(text, span.start).unwrap_or(span.start);
            (
                "Tab characters are not allowed as indentation.",
                char_range(text, offset),
            )
        }
        "while scanning a plain scalar, found a tab" => {
            let offset = nearby_tab(text, span.start).unwrap_or(span.start);
            (
                "Expected a space but found a tab.",
                char_range(text, offset),
            )
        }
        "while scanning an anchor or alias, did not find expected alphabetic or numeric character" =>
        {
            let offset = indicator_end(text, span.start);
            ("Expected alphanumeric character.", point(text, offset))
        }
        "while parsing node, found unknown anchor" => {
            let end = token_end(text, span.start);
            ("Undefined alias.", range(text, span.start, end))
        }
        "the handle wasn't declared" => {
            let end = token_end(text, span.start);
            ("Undefined tag handle.", range(text, span.start, end))
        }
        "while scanning a directive, could not find expected directive name" => {
            ("Expected directive name.", point(text, span.start))
        }
        "while scanning a YAML directive, did not find expected version number" => (
            "Expected version number.",
            point(text, yaml_version_number_position(text, span.start)),
        ),
        "while scanning a YAML directive, did not find expected digit or '.' character" => (
            "expected \".\".",
            point(text, yaml_version_dot_position(text, span.start)),
        ),
        "while scanning a directive, did not find expected comment or line break" => {
            let start = line_start(text, span.start);
            let end = directive_consumed_end(text, start);
            (
                "Expected comment or line break after directive.",
                range(text, start, end),
            )
        }
        "while scanning a tag, did not find expected '!'" => {
            let offset = tag_handle_position(text, span.start);
            ("expected \"!\".", point(text, offset))
        }
        "invalid global tag character" => {
            let Some(offset) = tag_directive_handle_end(text, span.start) else {
                return unchanged(message, span);
            };
            ("Expected whitespace.", point(text, offset))
        }
        "while parsing a quoted scalar, found unknown escape character" => {
            let offset = quoted_escape(text, span.start).map_or(span.start, |error| error.start);
            ("Unknown escape character.", point(text, offset))
        }
        "while parsing a quoted scalar, did not find expected hexadecimal number" => {
            if let Some(error) = quoted_escape(text, span.start) {
                let digits = error.digits.unwrap_or(0);
                let message = format!("Expected {digits}-digit hexidecimal number.");
                return YamlError {
                    message,
                    span: Some(range(text, error.start, error.end)),
                };
            }
            return unchanged(message, span);
        }
        "while parsing a quoted scalar, found invalid Unicode character escape code" => {
            if let Some(error) = quoted_escape(text, span.start) {
                return YamlError {
                    message: "Invalid Unicode character escape code.".into(),
                    span: Some(range(text, error.start, error.end)),
                };
            }
            return unchanged(message, span);
        }
        "while scanning a quoted scalar, found unexpected document indicator" => {
            let offset = document_indicator(text, span.start).unwrap_or(span.start);
            (
                "Unexpected document indicator.",
                range(text, offset, offset + 3),
            )
        }
        "while scanning a block scalar, found an indentation indicator equal to 0" => (
            "0 may not be used as an indentation indicator.",
            char_range(text, span.start),
        ),
        "unexpected character: `@'" | "unexpected character: `%'" | "unexpected character: ``'" => {
            ("Unexpected character.", char_range(text, span.start))
        }
        "misplaced bracket" => ("Expected node content.", point(text, span.start)),
        _ => return unchanged(message, span),
    };

    YamlError {
        message: message.into(),
        span: Some(span),
    }
}

fn unchanged(message: &str, span: Span) -> YamlError {
    YamlError {
        message: message.into(),
        span: Some(span),
    }
}

fn point(text: &str, offset: usize) -> Span {
    range(text, offset, offset)
}

fn char_range(text: &str, offset: usize) -> Span {
    let offset = clamp_boundary(text, offset);
    let end = text[offset..]
        .chars()
        .next()
        .map_or(offset, |character| offset + character.len_utf8());
    range(text, offset, end)
}

fn range(text: &str, start: usize, end: usize) -> Span {
    let start = clamp_boundary(text, start);
    let end = clamp_boundary(text, end.max(start));
    let prefix = &text[..start];
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    Span {
        start,
        end,
        line: prefix.bytes().filter(|byte| *byte == b'\n').count(),
        column: text[line_start..start].chars().count(),
    }
}

fn clamp_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn line_start(text: &str, offset: usize) -> usize {
    let offset = clamp_boundary(text, offset);
    text[..offset].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(text: &str, offset: usize) -> usize {
    let offset = clamp_boundary(text, offset);
    text[offset..]
        .find(['\r', '\n'])
        .map_or(text.len(), |relative| offset + relative)
}

fn indentation_tab(text: &str, offset: usize) -> Option<usize> {
    let start = line_start(text, offset);
    let end = line_end(text, offset);
    text[start..end]
        .char_indices()
        .take_while(|(_, character)| matches!(character, ' ' | '\t'))
        .find(|(_, character)| *character == '\t')
        .map(|(relative, _)| start + relative)
}

fn nearby_tab(text: &str, offset: usize) -> Option<usize> {
    let start = line_start(text, offset);
    let end = line_end(text, offset);
    text[start..end].find('\t').map(|relative| start + relative)
}

fn indicator_end(text: &str, offset: usize) -> usize {
    let offset = clamp_boundary(text, offset);
    match text[offset..].chars().next() {
        Some('*' | '&') => offset + 1,
        _ => offset,
    }
}

fn token_end(text: &str, offset: usize) -> usize {
    let offset = clamp_boundary(text, offset);
    text[offset..]
        .char_indices()
        .find(|(_, character)| {
            character.is_whitespace() || matches!(character, ',' | '[' | ']' | '{' | '}')
        })
        .map_or(text.len(), |(relative, _)| offset + relative)
}

fn directive_start(text: &str, offset: usize) -> usize {
    let start = line_start(text, offset);
    let end = line_end(text, offset);
    text[start..end]
        .find('%')
        .map_or(start, |relative| start + relative)
}

fn yaml_version_number_position(text: &str, offset: usize) -> usize {
    let start = directive_start(text, offset);
    let end = line_end(text, start);
    let mut cursor = (start + "%YAML".len()).min(end);
    while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    while cursor < end && text.as_bytes()[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor < end && text.as_bytes()[cursor] == b'.' {
        cursor += 1;
        while cursor < end && text.as_bytes()[cursor].is_ascii_digit() {
            cursor += 1;
        }
    }
    cursor
}

fn yaml_version_dot_position(text: &str, offset: usize) -> usize {
    let start = directive_start(text, offset);
    let end = line_end(text, start);
    let mut cursor = (start + "%YAML".len()).min(end);
    while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    while cursor < end && text.as_bytes()[cursor].is_ascii_digit() {
        cursor += 1;
    }
    cursor
}

fn tag_handle_position(text: &str, offset: usize) -> usize {
    let start = directive_start(text, offset);
    (start + "%TAG".len()).min(line_end(text, start))
}

fn directive_consumed_end(text: &str, start: usize) -> usize {
    let end = line_end(text, start);
    if text[start..end].starts_with("%YAML") {
        let mut cursor = (start + "%YAML".len()).min(end);
        while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        while cursor < end
            && (text.as_bytes()[cursor].is_ascii_digit() || text.as_bytes()[cursor] == b'.')
        {
            cursor += 1;
        }
        while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        return cursor;
    }
    if text[start..end].starts_with("%TAG") {
        let mut cursor = (start + "%TAG".len()).min(end);
        for _ in 0..2 {
            while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            while cursor < end && !text.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
        }
        while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        return cursor;
    }
    end
}

fn tag_directive_handle_end(text: &str, offset: usize) -> Option<usize> {
    let start = directive_start(text, offset);
    let end = line_end(text, start);
    if !text[start..end].starts_with("%TAG") {
        return None;
    }
    let mut cursor = start + "%TAG".len();
    while cursor < end && text.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    while cursor < end && !text.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    Some(cursor)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EscapeError {
    start: usize,
    end: usize,
    digits: Option<usize>,
}

fn quoted_escape(text: &str, quote_start: usize) -> Option<EscapeError> {
    let bytes = text.as_bytes();
    let mut cursor = clamp_boundary(text, quote_start);
    while cursor < bytes.len() && bytes[cursor] != b'"' {
        cursor += 1;
    }
    cursor = cursor.saturating_add(1);

    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            return None;
        }
        if bytes[cursor] != b'\\' {
            cursor += 1;
            continue;
        }

        let start = cursor;
        let &escape = bytes.get(cursor + 1)?;
        let digits = match escape {
            b'x' => 2,
            b'u' => 4,
            b'U' => 8,
            b'0' | b'a' | b'b' | b't' | b'n' | b'v' | b'f' | b'r' | b'e' | b' ' | b'"' | b'/'
            | b'\\' | b'N' | b'_' | b'L' | b'P' => {
                cursor += 2;
                continue;
            }
            _ => {
                return Some(EscapeError {
                    start,
                    end: start,
                    digits: None,
                });
            }
        };

        let digits_start = cursor + 2;
        for index in 0..digits {
            let position = digits_start + index;
            if bytes
                .get(position)
                .is_none_or(|byte| !byte.is_ascii_hexdigit())
            {
                return Some(EscapeError {
                    start,
                    end: (position + 1).min(bytes.len()),
                    digits: Some(digits),
                });
            }
        }

        let value = usize::from_str_radix(&text[digits_start..digits_start + digits], 16).ok()?;
        if (0xD800..=0xDFFF).contains(&value) || value > 0x10FFFF {
            return Some(EscapeError {
                start,
                end: digits_start + digits,
                digits: Some(digits),
            });
        }
        cursor = digits_start + digits;
    }
    None
}

fn document_indicator(text: &str, offset: usize) -> Option<usize> {
    let mut start = line_start(text, offset);
    while start < text.len() {
        let end = line_end(text, start);
        let line = &text[start..end];
        if line.starts_with("---") || line.starts_with("...") {
            return Some(start);
        }
        start = if end < text.len() {
            end + text[end..].chars().next()?.len_utf8()
        } else {
            break;
        };
    }
    None
}
