// Dart source: pkg/analyzer/lib/src/ignore_comments/ignore_info.dart

//! `// ignore:` and `// ignore_for_file:` comments (`IgnoreInfo`).
//!
//! Offsets are UTF-16 code unit offsets, like the analyzer.

use std::collections::BTreeMap;

use dartr_diagnostics::{Diagnostic, DiagnosticCode, DiagnosticType};
use dartr_syntax::{LineInfo, TokenId, Tokens};

/// Dart `IgnoredElement`: a name, a type, or trailing comment text of an
/// ignore comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IgnoredElement {
    /// Dart `IgnoredDiagnosticName`. [name] is lower case.
    Name {
        name: String,
        plugin_name: Option<String>,
        offset: u32,
    },
    /// Dart `IgnoredDiagnosticType` (`type=lint`). [type_] is lower case.
    Type { type_: String, offset: u32, length: u32 },
    /// Dart `IgnoredDiagnosticComment`.
    Comment { text: String, offset: u32 },
}

impl IgnoredElement {
    /// Dart `_matches`.
    fn matches(&self, code: &DiagnosticCode, plugin_name: Option<&str>) -> bool {
        match self {
            IgnoredElement::Name {
                name,
                plugin_name: own_plugin,
                ..
            } => {
                if own_plugin.as_deref() != plugin_name {
                    return false;
                }
                if name == code.lower_case_name() {
                    return true;
                }
                let unique = code.lower_case_unique_name();
                let unique = match unique.find('.') {
                    Some(period) => &unique[period + 1..],
                    None => unique,
                };
                *name == unique.to_lowercase()
            }
            IgnoredElement::Type { type_, .. } => match code.diagnostic_type {
                DiagnosticType::Hint => type_ == "hint",
                DiagnosticType::Lint => type_ == "lint",
                DiagnosticType::StaticWarning => type_ == "warning",
                _ => false,
            },
            IgnoredElement::Comment { .. } => false,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            IgnoredElement::Name { name, .. } => Some(name),
            _ => None,
        }
    }
}

/// Dart `IgnoreInfo`.
#[derive(Clone, Debug, Default)]
pub struct IgnoreInfo {
    /// Line number (1-based) to the elements ignored on that line.
    ignored_on_line: BTreeMap<u32, Vec<IgnoredElement>>,
    ignored_for_file: Vec<IgnoredElement>,
}

impl IgnoreInfo {
    /// Dart `IgnoreInfo.forDart`: the ignore comments of the token stream
    /// that starts at [first] (`CompilationUnit.beginToken`).
    pub fn for_dart(tokens: &Tokens, first: TokenId, line_info: &LineInfo, content: &str) -> Self {
        let mut info = IgnoreInfo::default();
        for comment in ignore_comments(tokens, first) {
            let lexeme = tokens.lexeme(comment);
            if lexeme.contains("ignore:") {
                let offset = tokens.offset(comment);
                let mut line_number = line_info.get_location(offset).line_number;
                // `beforeMatch.trim().isEmpty`: the text between the start of
                // the line and the comment.
                let byte_offset = tokens.byte_offset(comment) as usize;
                let line_start = content[..byte_offset]
                    .rfind(['\n', '\r'])
                    .map_or(0, |i| i + 1);
                if is_dart_blank(&content[line_start..byte_offset]) {
                    // The comment is on its own line, so it refers to the next
                    // line.
                    line_number += 1;
                }
                info.ignored_on_line
                    .entry(line_number)
                    .or_default()
                    .extend(ignored_elements(lexeme, offset));
            } else if lexeme.contains("ignore_for_file:") {
                info.ignored_for_file
                    .extend(ignored_elements(lexeme, tokens.offset(comment)));
            }
        }
        info
    }

    /// Dart `hasIgnores`.
    pub fn has_ignores(&self) -> bool {
        !self.ignored_on_line.is_empty() || !self.ignored_for_file.is_empty()
    }

    /// Dart `ignoredForFile` (a copy).
    pub fn ignored_for_file(&self) -> Vec<IgnoredElement> {
        self.ignored_for_file.clone()
    }

    /// Dart `ignoredOnLine` (a copy), ordered by line.
    pub fn ignored_on_line(&self) -> BTreeMap<u32, Vec<IgnoredElement>> {
        self.ignored_on_line.clone()
    }

    /// Dart `ignored`: whether [diagnostic] is ignored by an ignore comment.
    pub fn ignored(&self, diagnostic: &Diagnostic, line_info: &LineInfo) -> bool {
        let line = line_info.get_location(diagnostic.offset as u32).line_number;
        self.ignored_at(diagnostic.code, line, None)
    }

    /// Dart `_ignoredAt`.
    fn ignored_at(&self, code: &DiagnosticCode, line: u32, plugin_name: Option<&str>) -> bool {
        let on_line = self.ignored_on_line.get(&line);
        if self.ignored_for_file.is_empty() && on_line.is_none() {
            return false;
        }
        if self
            .ignored_for_file
            .iter()
            .any(|e| e.matches(code, plugin_name))
        {
            return true;
        }
        on_line.is_some_and(|elements| elements.iter().any(|e| e.matches(code, plugin_name)))
    }

    /// Dart `isIgnoreComment`.
    pub fn is_ignore_comment(s: &str) -> bool {
        let s = s.as_bytes();
        let end = s.len();
        if end < 9 || s[0] != b'/' || s[1] != b'/' {
            return false;
        }
        let mut from = 2;
        while from < end && s[from] == b'/' {
            from += 1;
        }
        while from < end && s[from] == b' ' {
            from += 1;
        }
        end - from >= 7 && &s[from..from + 7] == b"ignore:"
    }

    /// Dart `isIgnoreForFileComment`.
    pub fn is_ignore_for_file_comment(s: &str) -> bool {
        let s = s.as_bytes();
        let end = s.len();
        if end < 18 || s[0] != b'/' || s[1] != b'/' {
            return false;
        }
        let mut from = 2;
        while from < end && s[from] == b' ' {
            from += 1;
        }
        end - from >= 16 && &s[from..from + 16] == b"ignore_for_file:"
    }
}

/// Dart `String.trim().isEmpty`: Dart trims the Unicode whitespace of
/// `String.trim` (which also contains U+FEFF).
fn is_dart_blank(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || c == '\u{feff}')
}

/// Dart `CompilationUnitExtension.ignoreComments`.
fn ignore_comments(tokens: &Tokens, first: TokenId) -> Vec<TokenId> {
    let mut result = Vec::new();
    for token in tokens.iter_from(first) {
        for comment in tokens.comments(token) {
            let lexeme = tokens.lexeme(comment);
            if IgnoreInfo::is_ignore_comment(lexeme) || IgnoreInfo::is_ignore_for_file_comment(lexeme)
            {
                result.push(comment);
            }
        }
    }
    result
}

fn is_letter(c: u16) -> bool {
    (0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c)
}

fn is_digit(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
}

fn is_whitespace(c: u16) -> bool {
    c == 0x20 || c == 0x09 || c == 0x0D || c == 0x0A
}

fn is_space(c: u16) -> bool {
    c == 0x20 || c == 0x09
}

const COMMA: u16 = 0x2C;
const UNDERSCORE: u16 = 0x5F;

/// Dart `CommentTokenExtension.ignoredElements` for a comment with the
/// given [lexeme] at [token_offset].
pub fn ignored_elements(lexeme: &str, token_offset: u32) -> Vec<IgnoredElement> {
    let units: Vec<u16> = lexeme.encode_utf16().collect();
    let len = units.len();
    let text = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to]);
    let mut result = Vec::new();
    let mut offset = units.iter().position(|&c| c == b':' as u16).map_or(0, |i| i + 1);

    let skip_whitespace = |offset: &mut usize| {
        while *offset < len && is_whitespace(units[*offset]) {
            *offset += 1;
        }
    };
    let read_word = |offset: &mut usize| {
        if !is_letter(units[*offset]) && units[*offset] != UNDERSCORE {
            return;
        }
        *offset += 1;
        while *offset < len {
            let c = units[*offset];
            if !(is_letter(c) || is_digit(c) || c == UNDERSCORE) {
                return;
            }
            *offset += 1;
        }
    };
    let at = |o: usize| token_offset + o as u32;

    let mut has_ignored_elements = false;
    loop {
        skip_whitespace(&mut offset);
        if offset == len {
            return result;
        }
        let word_offset = offset;
        read_word(&mut offset);
        if word_offset == offset {
            if has_ignored_elements {
                result.push(IgnoredElement::Comment {
                    text: text(offset, len),
                    offset: at(word_offset),
                });
            }
            return result;
        }
        let mut word = text(word_offset, offset);
        if word.to_lowercase() == "type" {
            skip_whitespace(&mut offset);
            if offset == len {
                return result;
            }
            if units[offset] != b'=' as u16 {
                return result;
            }
            offset += 1;
            skip_whitespace(&mut offset);
            if offset == len {
                return result;
            }
            let type_offset = offset;
            read_word(&mut offset);
            if type_offset == offset {
                if has_ignored_elements {
                    result.push(IgnoredElement::Comment {
                        text: text(offset, len),
                        offset: at(word_offset),
                    });
                }
                return result;
            }
            if offset < len {
                let next = units[offset];
                if !is_space(next) && next != COMMA {
                    if has_ignored_elements {
                        result.push(IgnoredElement::Comment {
                            text: text(word_offset, len),
                            offset: at(word_offset),
                        });
                    }
                    return result;
                }
            }
            has_ignored_elements = true;
            result.push(IgnoredElement::Type {
                type_: text(type_offset, offset).to_lowercase(),
                offset: at(word_offset),
                length: (offset - word_offset) as u32,
            });
        } else {
            let mut plugin_name = None;
            if offset < len && units[offset] == b'/' as u16 {
                plugin_name = Some(word.clone());
                offset += 1;
                if offset == len {
                    return result;
                }
                let name_offset = offset;
                read_word(&mut offset);
                word = text(name_offset, offset);
                if name_offset == offset {
                    if has_ignored_elements {
                        result.push(IgnoredElement::Comment {
                            text: text(offset, len),
                            offset: at(name_offset),
                        });
                    }
                    return result;
                }
            }
            if offset < len {
                let next = units[offset];
                if !is_space(next) && next != COMMA {
                    if has_ignored_elements {
                        result.push(IgnoredElement::Comment {
                            text: text(word_offset, len),
                            offset: at(word_offset),
                        });
                    }
                    return result;
                }
            }
            has_ignored_elements = true;
            result.push(IgnoredElement::Name {
                name: word.to_lowercase(),
                plugin_name,
                offset: at(word_offset),
            });
        }

        if offset == len {
            return result;
        }
        skip_whitespace(&mut offset);
        if offset == len {
            return result;
        }
        if units[offset] != COMMA {
            if has_ignored_elements {
                result.push(IgnoredElement::Comment {
                    text: text(offset, len),
                    offset: at(word_offset),
                });
            }
            return result;
        }
        offset += 1;
        if offset == len {
            return result;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(lexeme: &str) -> Vec<String> {
        ignored_elements(lexeme, 0)
            .iter()
            .map(|e| match e {
                IgnoredElement::Name {
                    name, plugin_name, ..
                } => match plugin_name {
                    Some(p) => format!("{p}/{name}"),
                    None => name.clone(),
                },
                IgnoredElement::Type { type_, .. } => format!("type={type_}"),
                IgnoredElement::Comment { text, .. } => format!("#{text}"),
            })
            .collect()
    }

    #[test]
    fn parses_ignore_comments() {
        assert_eq!(names("// ignore: a, B_c"), ["a", "b_c"]);
        assert_eq!(names("// ignore: type=lint, x"), ["type=lint", "x"]);
        assert_eq!(names("// ignore: p/x some text"), ["p/x", "#some text"]);
        assert_eq!(names("// ignore: http://google.com"), Vec::<String>::new());
        assert_eq!(names("// ignore: a, http://x"), ["a", "#http://x"]);
        assert!(IgnoreInfo::is_ignore_comment("/// ignore: a"));
        assert!(!IgnoreInfo::is_ignore_for_file_comment("/// ignore_for_file: a"));
        assert!(IgnoreInfo::is_ignore_for_file_comment("//ignore_for_file: a"));
    }
}
