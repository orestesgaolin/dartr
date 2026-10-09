// Dart source: pkg/analyzer/lib/src/error/todo_finder.dart

//! `TodoFinder`: reports `TODO`, `FIXME`, `HACK` and `UNDONE` comments
//! (the CLI drops these diagnostics; the LSP shows them).
//!
//! Offsets and text positions are UTF-16 code units, like Dart strings.

use dartr_diagnostics::diag;
use dartr_syntax::{TokenId, TokenType};

use super::{UnitVerifier, VerifierHost};

/// Dart `TodoFinder(diagnosticReporter).findIn(unit)`.
pub fn find_in(v: &mut UnitVerifier<'_>) {
    let begin = v.ast[v.unit].begin_token;
    gather_todo_comments(v, begin);
}

/// Dart `_gatherTodoComments`.
fn gather_todo_comments(v: &mut UnitVerifier<'_>, begin: TokenId) {
    let tokens: Vec<TokenId> = v.ast.tokens.iter_from(begin).collect();
    for token in tokens {
        let mut comment = v.ast.tokens.get(token).preceding_comments.get();
        while let Some(c) = comment {
            let ty = v.ast.tokens.get(c).ty;
            if ty == TokenType::SINGLE_LINE_COMMENT || ty == TokenType::MULTI_LINE_COMMENT {
                comment = scrape_todo_comment(v, c);
            } else {
                comment = v.ast.tokens.get(c).next.get();
            }
        }
    }
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const LF: u16 = b'\n' as u16;
const CR: u16 = b'\r' as u16;
const STAR: u16 = b'*' as u16;
const SLASH: u16 = b'/' as u16;

/// Dart RegExp `\s` (ASCII part and the common Unicode spaces).
fn is_regexp_space(c: u16) -> bool {
    matches!(c, 0x09..=0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF)
}

/// Dart `todoText.replaceAll(RegExp('\\s*\\n\\s*\\*\\s*'), ' ')`.
fn replace_comment_newline_and_marker(s: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let mut matched = None;
        if is_regexp_space(s[i]) {
            let mut j = i;
            while j < s.len() && is_regexp_space(s[j]) {
                j += 1;
            }
            // Backtrack over the first `\s*` to the last `\n` that is
            // followed by `\s*\*`.
            let mut k = j;
            while k > i {
                k -= 1;
                if s[k] == LF {
                    let mut m = k + 1;
                    while m < s.len() && is_regexp_space(s[m]) {
                        m += 1;
                    }
                    if m < s.len() && s[m] == STAR {
                        m += 1;
                        while m < s.len() && is_regexp_space(s[m]) {
                            m += 1;
                        }
                        matched = Some(m);
                        break;
                    }
                }
            }
        }
        match matched {
            Some(end) => {
                out.push(SPACE);
                i = end;
            }
            None => {
                out.push(s[i]);
                i += 1;
            }
        }
    }
    out
}

/// Dart `lexeme.indexOf(RegExp('[^/ ]'))` (-1 when none).
fn index_of_non_whitespace_or_comment_marker(s: &[u16]) -> i64 {
    s.iter()
        .position(|&c| c != SLASH && c != SPACE)
        .map_or(-1, |i| i as i64)
}

/// Dart `String.trimRight`.
fn trim_right(s: &[u16]) -> &[u16] {
    let mut end = s.len();
    while end > 0 && (is_regexp_space(s[end - 1]) || s[end - 1] == 0x85) {
        end -= 1;
    }
    &s[..end]
}

/// Dart `_scrapeTodoComment`: returns the next comment token to search
/// from (after the continuations).
fn scrape_todo_comment(v: &mut UnitVerifier<'_>, comment_token: TokenId) -> Option<TokenId> {
    let tokens = &v.ast.tokens;
    let mut next_comment = tokens.get(comment_token).next.get();
    let comment = tokens.get(comment_token);
    let comment_ty = comment.ty;
    let comment_offset = comment.offset;
    let lexeme = utf16(tokens.lexeme(comment_token));
    let line_info = &v.parsed.line_info;
    let mut comment_location = None;

    let mut reports = Vec::new();
    let mut finder = TodoFinderImpl::new(&lexeme);
    while finder.move_next() {
        let match_offset = finder.offset as u32;
        let todo_kind = String::from_utf16_lossy(finder.todo_kind());
        let mut todo_text: Vec<u16> = finder.todo_text().to_vec();

        let location =
            *comment_location.get_or_insert_with(|| line_info.get_location(comment_offset));
        let offset = comment_offset + match_offset;
        let column = location.column_number + match_offset;
        let mut end = offset + todo_text.len() as u32;

        if comment_ty == TokenType::MULTI_LINE_COMMENT {
            // Remove any `*/` and trim any trailing whitespace.
            if todo_text.ends_with(&[STAR, SLASH]) {
                todo_text = trim_right(&todo_text[..todo_text.len() - 2]).to_vec();
                end = offset + todo_text.len() as u32;
            }
            // Unwrap multiple lines; the length keeps all characters.
            todo_text = replace_comment_newline_and_marker(&todo_text);
        } else if comment_ty == TokenType::SINGLE_LINE_COMMENT {
            // Append any indented lines onto the end.
            let mut line = location.line_number;
            while let Some(next) = next_comment {
                let next_token = tokens.get(next);
                let next_location = line_info.get_location(next_token.offset);
                let next_lexeme = utf16(tokens.lexeme(next));
                let column_of_first = next_location.column_number as i64
                    + index_of_non_whitespace_or_comment_marker(&next_lexeme);
                // Dart `line++ + 1`.
                let expected_line = line + 1;
                line += 1;
                let is_continuation = next_token.ty == TokenType::SINGLE_LINE_COMMENT
                    && !tokens.lexeme(next).starts_with("///")
                    && next_location.line_number == expected_line
                    && next_location.column_number == location.column_number
                    && column_of_first == column as i64 + 1
                    && !TodoFinderImpl::new(&next_lexeme).move_next();
                if !is_continuation {
                    break;
                }
                end = next_token.end();
                let lexeme_text_offset =
                    (column_of_first - next_location.column_number as i64) as usize;
                let continuation =
                    trim_right(&next_lexeme[lexeme_text_offset.min(next_lexeme.len())..]);
                todo_text.push(SPACE);
                todo_text.extend_from_slice(continuation);
                next_comment = next_token.next.get();
            }
        }

        // Dart `Todo.forKind(todoKind).withArguments(message: todoText)`.
        let message = String::from_utf16_lossy(&todo_text);
        let d = match todo_kind.as_str() {
            "TODO" => diag::todo(&message),
            "FIXME" => diag::fixme(&message),
            "HACK" => diag::hack(&message),
            _ => diag::undone(&message),
        };
        reports.push(d.at_offset(offset as usize, (end - offset) as usize));
    }
    for d in reports {
        v.report(d);
    }
    next_comment
}

/// Dart `_TodoFinder`.
struct TodoFinderImpl<'s> {
    s: &'s [u16],
    start_at: usize,
    offset: usize,
    text: (usize, usize),
    kind: (usize, usize),
}

impl<'s> TodoFinderImpl<'s> {
    fn new(s: &'s [u16]) -> Self {
        // Start at 1 to allow for the char before the first find to be \s,
        // / or *.
        TodoFinderImpl {
            s,
            start_at: 1,
            offset: 0,
            text: (0, 0),
            kind: (0, 0),
        }
    }

    fn todo_text(&self) -> &'s [u16] {
        &self.s[self.text.0..self.text.1]
    }

    fn todo_kind(&self) -> &'s [u16] {
        &self.s[self.kind.0..self.kind.1]
    }

    fn is(&self, i: usize, word: &str) -> bool {
        word.bytes()
            .enumerate()
            .all(|(k, b)| self.s.get(i + k) == Some(&(b as u16)))
    }

    /// Dart `moveNext`: finds `TODO`, `HACK`, `FIXME` or `UNDONE`.
    fn move_next(&mut self) -> bool {
        // Stop 3 before the end so the next 3 chars can be checked.
        let end = self.s.len() as i64 - 3;
        let mut i = self.start_at as i64;
        while i < end {
            let iu = i as usize;
            let c = self.s[iu];
            if (b'A' as u16..=b'Z' as u16).contains(&c) {
                let found = if self.is(iu, "TODO") || self.is(iu, "HACK") {
                    Some(iu + 4)
                } else if self.s.len() > iu + 4 && self.is(iu, "FIXME") {
                    Some(iu + 5)
                } else if self.s.len() > iu + 5 && self.is(iu, "UNDONE") {
                    Some(iu + 6)
                } else {
                    None
                };
                if let Some(to) = found
                    && self.check(iu, to)
                {
                    return true;
                }
            }
            i += 1;
        }
        false
    }

    /// Dart `_check`.
    fn check(&mut self, from: usize, to: usize) -> bool {
        let s = self.s;
        let before = s[from - 1];
        if !matches!(before, SPACE | TAB | LF | CR | STAR | SLASH) {
            // Doesn't start with \s, / or *.
            return false;
        }
        if s.len() == to {
            // Line ends with this.
            self.do_match(from, to, to);
            return true;
        }
        let after = s[to];
        // Not allowed to match [A-Za-z0-9_].
        if (b'0' as u16..=b'9' as u16).contains(&after)
            || (b'a' as u16..=b'z' as u16).contains(&after)
            || (b'A' as u16..=b'Z' as u16).contains(&after)
            || after == b'_' as u16
        {
            return false;
        }
        let mut scan_from = to + 1;
        loop {
            let found = (scan_from..s.len()).find(|&i| s[i] == CR || s[i] == LF);
            let Some(found_linebreak_at) = found else {
                // No line breaks: match until the end.
                self.do_match(from, to, s.len());
                return true;
            };
            // Possibly match the next line too.
            let include_until = found_linebreak_at;
            let mut next_line_at = found_linebreak_at;
            if s[next_line_at] == CR && s.len() > next_line_at + 1 && s[next_line_at + 1] == LF {
                next_line_at += 1;
            }
            let mut i = next_line_at + 1;
            while i < s.len() && matches!(s[i], SPACE | TAB | LF | CR) {
                i += 1;
            }
            if !(s.len() > i + 2 && s[i] == STAR && s[i + 1] == SPACE && s[i + 2] == SPACE) {
                // Not whitespace, then `*` and two spaces: not included.
                self.do_match(from, to, include_until);
                return true;
            }
            // Include this line too.
            scan_from = i + 3;
        }
    }

    /// Dart `_match`.
    fn do_match(&mut self, from: usize, kind_to: usize, final_to: usize) {
        self.offset = from;
        self.text = (from, final_to);
        self.kind = (from, kind_to);
        self.start_at = final_to;
    }
}
