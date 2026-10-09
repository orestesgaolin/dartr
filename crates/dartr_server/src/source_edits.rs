// Dart source: pkg/analysis_server/lib/src/lsp/source_edits.dart (applyAndConvertEditsToServer, generateEditsForFormatting, generateFullEdit, generateMinimalEdits, _MinimalEditComputer)

//! Incremental document changes (`textDocument/didChange`) and the edits of
//! the formatting requests: the formatted source is compared with the
//! unformatted source token by token, and only the whitespace (and the
//! commas, semicolons and comments that the formatter may change) between
//! the tokens is replaced.

use dartr_syntax::{
    LineInfo, ScannerConfiguration, TokenId, TokenType, Tokens,
    analyzer_scanner::scan_for_analyzer_with_configuration,
};
use serde_json::{Value, json};

use crate::mapping::{ErrorOr, ResponseError, codes, read_position, to_offset, to_position};

/// The byte index of the UTF-16 offset [offset] in [content]. Offsets after
/// the end map to the end; an offset inside a surrogate pair maps to the
/// start of the character.
fn byte_index(content: &str, offset: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in content.char_indices() {
        let next = units + c.len_utf16() as u32;
        if next > offset {
            return i;
        }
        units = next;
    }
    content.len()
}

/// Dart `applyAndConvertEditsToServer(failureIsCritical: true)`: applies
/// the `contentChanges` of `didChange` to [content].
pub fn apply_changes(content: &str, changes: &[Value]) -> ErrorOr<String> {
    let mut new_content = content.to_string();
    for change in changes {
        let text = change
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| ResponseError::new(codes::INVALID_PARAMS, "Invalid change"))?;
        match change.get("range") {
            Some(range) => {
                let start = range.get("start").and_then(read_position);
                let end = range.get("end").and_then(read_position);
                let (Some(start), Some(end)) = (start, end) else {
                    return Err(ResponseError::new(codes::INVALID_PARAMS, "Invalid range"));
                };
                let lines = LineInfo::from_content(&new_content);
                let start = to_offset(&lines, start.0, start.1, true)?;
                let end = to_offset(&lines, end.0, end.1, true)?;
                let (start, end) = (
                    byte_index(&new_content, start),
                    byte_index(&new_content, end),
                );
                if start > end {
                    return Err(ResponseError::new(
                        codes::CLIENT_SERVER_INCONSISTENT_STATE,
                        "Invalid range",
                    ));
                }
                new_content.replace_range(start..end, text);
            }
            None => new_content = text.to_string(),
        }
    }
    Ok(new_content)
}

/// Dart `_isValidFormatterChange`: `RegExp(r'^[\s,;<>]*$')`, with the
/// JavaScript `\s` of Dart regular expressions.
fn is_valid_formatter_change(text: &[u16]) -> bool {
    text.iter().all(|&c| {
        matches!(
            c,
            0x09..=0x0d
                | 0x20
                | 0xa0
                | 0x1680
                | 0x2000..=0x200a
                | 0x2028
                | 0x2029
                | 0x202f
                | 0x205f
                | 0x3000
                | 0xfeff
        ) || c == b',' as u16
            || c == b';' as u16
            || c == b'<' as u16
            || c == b'>' as u16
    })
}

/// A `TextEdit` as LSP JSON.
fn text_edit(line_info: &LineInfo, start: u32, end: u32, new_text: &str) -> Value {
    json!({
        "range": {"start": to_position(line_info, start), "end": to_position(line_info, end)},
        "newText": new_text,
    })
}

/// The range of a formatting request as offsets (Dart `toOffset` of the
/// start and the end, not critical).
pub fn range_offsets(line_info: &LineInfo, range: &Value) -> ErrorOr<(u32, u32)> {
    let position = |key: &str| {
        range
            .get(key)
            .and_then(read_position)
            .ok_or_else(|| ResponseError::new(codes::INVALID_PARAMS, "Invalid range"))
    };
    let (start, end) = (position("start")?, position("end")?);
    let start = to_offset(line_info, start.0, start.1, false)?;
    let end = to_offset(line_info, end.0, end.1, false)?;
    Ok((start, end))
}

/// Dart `generateEditsForFormatting`: the edits that format [unformatted]
/// (the content of a parsed unit with [line_info]) to [formatted] (the
/// result of `formatSafely`), or `null` if the formatter changed nothing.
/// With [range], only the edits inside the range.
pub fn generate_edits_for_formatting(
    unformatted: &str,
    line_info: &LineInfo,
    formatted: &str,
    scanner_configuration: ScannerConfiguration,
    range: Option<&Value>,
) -> ErrorOr<Value> {
    if formatted == unformatted {
        return Ok(Value::Null);
    }
    let range = match range {
        Some(r) => Some(range_offsets(line_info, r)?),
        None => None,
    };
    Ok(Value::Array(generate_minimal_edits(
        unformatted,
        line_info,
        formatted,
        scanner_configuration,
        range,
    )))
}

/// Dart `generateFullEdit`: one edit that replaces the whole document.
pub fn generate_full_edit(line_info: &LineInfo, unformatted: &str, formatted: &str) -> Vec<Value> {
    let end = utf16_len(unformatted);
    vec![text_edit(line_info, 0, end, formatted)]
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Dart `generateMinimalEdits` (after the conversion of the range to
/// offsets): edits that change only whitespace, commas, semicolons and
/// comments of [unformatted] to get [formatted]. [range] is the start and
/// end offset of a range formatting request.
pub fn generate_minimal_edits(
    unformatted: &str,
    line_info: &LineInfo,
    formatted: &str,
    scanner_configuration: ScannerConfiguration,
    range: Option<(u32, u32)>,
) -> Vec<Value> {
    MinimalEditComputer::new(
        line_info,
        unformatted,
        formatted,
        scanner_configuration,
        range,
    )
    .compute_minimal_edits()
}

/// A scanned source: the UTF-16 text, the tokens and the first token.
struct Parsed {
    text: Vec<u16>,
    tokens: Tokens,
    first: TokenId,
}

impl Parsed {
    /// Dart `_MinimalEditComputer._parse`: scans [s] with the features of the
    /// unit (Dart `Scanner.configureFeatures(featureSetForOverriding:
    /// featureSet, featureSet: featureSet)`; the feature set of the unit is
    /// already restricted to its language version override).
    fn new(s: &str, configuration: ScannerConfiguration) -> Parsed {
        let result = scan_for_analyzer_with_configuration(s, configuration, |_| configuration);
        Parsed {
            text: s.encode_utf16().collect(),
            first: result.first,
            tokens: result.scan.tokens,
        }
    }

    /// Dart `_iterateAllTokens`: all tokens and comments, without EOF.
    fn all_tokens(&self) -> Vec<TokenId> {
        let mut out = Vec::new();
        let mut token = self.first;
        loop {
            // EOF is handled inside the loop: it can have preceding comments.
            out.extend(self.tokens.comments(token));
            if self.tokens.get(token).is_eof() {
                break;
            }
            out.push(token);
            token = self.tokens.next(token);
        }
        out
    }

    fn ty(&self, id: TokenId) -> TokenType {
        self.tokens.ty(id)
    }

    fn offset(&self, id: TokenId) -> u32 {
        self.tokens.get(id).offset
    }

    fn end(&self, id: TokenId) -> u32 {
        self.tokens.get(id).end()
    }

    fn lexeme(&self, id: TokenId) -> &str {
        self.tokens.lexeme(id)
    }
}

/// A token stream as a Dart `Iterator`: `move_next` and `current`.
struct TokenCursor {
    tokens: Vec<TokenId>,
    next: usize,
    current: TokenId,
}

impl TokenCursor {
    fn new(tokens: Vec<TokenId>) -> TokenCursor {
        TokenCursor {
            tokens,
            next: 0,
            current: TokenId::NONE,
        }
    }

    fn move_next(&mut self) -> bool {
        match self.tokens.get(self.next) {
            Some(&t) => {
                self.current = t;
                self.next += 1;
                true
            }
            None => false,
        }
    }
}

/// Dart `_MinimalEditComputer.allowedAddOrRemovedTokens`: tokens that the
/// formatter may add or remove.
const ALLOWED_ADD_OR_REMOVED_TOKENS: [TokenType; 2] = [TokenType::COMMA, TokenType::SEMICOLON];

/// Dart `allowedLexemeDifferences`: comments may have trailing spaces
/// removed from lines.
const ALLOWED_LEXEME_DIFFERENCES: [TokenType; 2] = [
    TokenType::MULTI_LINE_COMMENT,
    TokenType::SINGLE_LINE_COMMENT,
];

/// Dart `allowedSubstitutions`: tokens that the formatter may replace with
/// a sequence of other tokens.
fn allowed_substitutions(ty: TokenType) -> &'static [&'static [TokenType]] {
    use TokenType as T;
    match ty {
        T::GT_GT_GT => &[
            &[T::GT, T::GT, T::GT],
            &[T::GT, T::GT_GT],
            &[T::GT_GT, T::GT],
        ],
        T::GT_GT => &[&[T::GT, T::GT]],
        T::LT_LT => &[&[T::LT, T::LT]],
        T::INDEX => &[&[T::OPEN_SQUARE_BRACKET, T::CLOSE_SQUARE_BRACKET]],
        _ => &[],
    }
}

/// Dart `_MinimalEditComputer`: computes minimal edits to translate a set
/// of parseable source code into another (usually produced by the
/// formatter).
struct MinimalEditComputer<'a> {
    line_info: &'a LineInfo,
    unformatted_source: &'a str,
    formatted_source: &'a str,
    range_start: Option<u32>,
    range_end: Option<u32>,
    unformatted: Parsed,
    formatted: Parsed,
    /// The edits being built.
    edits: Vec<Value>,
}

impl<'a> MinimalEditComputer<'a> {
    fn new(
        line_info: &'a LineInfo,
        unformatted: &'a str,
        formatted: &'a str,
        configuration: ScannerConfiguration,
        range: Option<(u32, u32)>,
    ) -> Self {
        MinimalEditComputer {
            line_info,
            unformatted_source: unformatted,
            formatted_source: formatted,
            range_start: range.map(|r| r.0),
            range_end: range.map(|r| r.1),
            unformatted: Parsed::new(unformatted, configuration),
            formatted: Parsed::new(formatted, configuration),
            edits: Vec::new(),
        }
    }

    /// Dart `computeMinimalEdits`. If the edits cannot be computed: no edits
    /// for a range, one edit for the whole document otherwise.
    fn compute_minimal_edits(mut self) -> Vec<Value> {
        let mut unformatted_tokens = TokenCursor::new(self.unformatted.all_tokens());
        let mut formatted_tokens = TokenCursor::new(self.formatted.all_tokens());

        let mut unformatted_offset = 0;
        let mut formatted_offset = 0;

        // Walk through the token streams computing edits for the differences.
        let mut unformatted_has_more;
        let mut formatted_has_more;
        loop {
            // Don't short-circuit.
            unformatted_has_more = unformatted_tokens.move_next();
            formatted_has_more = formatted_tokens.move_next();
            if !(unformatted_has_more & formatted_has_more) {
                break;
            }
            let mut unformatted_token = unformatted_tokens.current;
            let mut formatted_token = formatted_tokens.current;

            // Compute the ranges from each side that we will produce an edit
            // for. This is usually just the whitespace from each side (the
            // range between the end of the previous token and the start of the
            // current), but in the case of commas will be expanded to include
            // the commas (and then the following whitespace).
            let unformatted_start = unformatted_offset;
            let mut unformatted_end = self.unformatted.offset(unformatted_token);
            let formatted_start = formatted_offset;
            let mut formatted_end = self.formatted.offset(formatted_token);
            let mut allow_any_content_differences = false;

            // We may need to advance multiple times if multiple allowed tokens
            // are added/removed consecutively.
            while unformatted_has_more && formatted_has_more {
                let same_token_types =
                    self.formatted.ty(formatted_token) == self.unformatted.ty(unformatted_token);

                // Helpers to advance a stream by `count` tokens if it is not
                // at the end (Dart `advanceFormatted`, `advanceUnformatted`).
                // Don't use the `next` token's offset, that would skip
                // comments.
                macro_rules! advance {
                    ($parsed:expr, $cursor:ident, $token:ident, $end:ident, $has_more:ident, $count:expr) => {{
                        $end = $parsed.end($token);
                        for _ in 0..$count {
                            $has_more = $cursor.move_next();
                            if $has_more {
                                $token = $cursor.current;
                                $end = $parsed.offset($token);
                            }
                        }
                    }};
                }

                // Handle differences allowed when tokens are different types.
                if !same_token_types {
                    // The formatter added an allowed token, advance over it.
                    if ALLOWED_ADD_OR_REMOVED_TOKENS.contains(&self.formatted.ty(formatted_token)) {
                        advance!(
                            self.formatted,
                            formatted_tokens,
                            formatted_token,
                            formatted_end,
                            formatted_has_more,
                            1
                        );
                        continue;
                    }

                    // The formatter removed an allowed token, advance over it.
                    if ALLOWED_ADD_OR_REMOVED_TOKENS
                        .contains(&self.unformatted.ty(unformatted_token))
                    {
                        advance!(
                            self.unformatted,
                            unformatted_tokens,
                            unformatted_token,
                            unformatted_end,
                            unformatted_has_more,
                            1
                        );
                        continue;
                    }

                    // The formatter substituted `unformatted_token` for some
                    // other tokens starting at `formatted_token`.
                    if let Some(num) = substitutes(
                        self.unformatted.ty(unformatted_token),
                        &self.formatted,
                        formatted_token,
                    ) {
                        advance!(
                            self.unformatted,
                            unformatted_tokens,
                            unformatted_token,
                            unformatted_end,
                            unformatted_has_more,
                            1
                        );
                        advance!(
                            self.formatted,
                            formatted_tokens,
                            formatted_token,
                            formatted_end,
                            formatted_has_more,
                            num
                        );
                        continue;
                    }

                    // The formatter collapsed tokens in `unformatted_token` for
                    // a new `formatted_token`. This is the opposite of the case
                    // above.
                    if let Some(num) = substitutes(
                        self.formatted.ty(formatted_token),
                        &self.unformatted,
                        unformatted_token,
                    ) {
                        advance!(
                            self.formatted,
                            formatted_tokens,
                            formatted_token,
                            formatted_end,
                            formatted_has_more,
                            1
                        );
                        advance!(
                            self.unformatted,
                            unformatted_tokens,
                            unformatted_token,
                            unformatted_end,
                            unformatted_has_more,
                            num
                        );
                        continue;
                    }
                }

                // Handle differences allowed when tokens are the same type.
                if same_token_types {
                    // The formatter made a change to the lexeme of a token
                    // type we allow.
                    if ALLOWED_LEXEME_DIFFERENCES.contains(&self.unformatted.ty(unformatted_token))
                        && self.unformatted.lexeme(unformatted_token)
                            != self.formatted.lexeme(formatted_token)
                    {
                        advance!(
                            self.unformatted,
                            unformatted_tokens,
                            unformatted_token,
                            unformatted_end,
                            unformatted_has_more,
                            1
                        );
                        advance!(
                            self.formatted,
                            formatted_tokens,
                            formatted_token,
                            formatted_end,
                            formatted_has_more,
                            1
                        );
                        allow_any_content_differences = true;
                        continue;
                    }
                }

                // If we didn't hit any `continue` above to restart the loop,
                // then we are done.
                break;
            }

            if self.unformatted.lexeme(unformatted_token) != self.formatted.lexeme(formatted_token)
                && !ALLOWED_LEXEME_DIFFERENCES.contains(&self.unformatted.ty(unformatted_token))
            {
                // If the token lexemes do not match (except where this is a
                // token allowed to have differences), there is an unexpected
                // difference in the parsed token streams (this should not
                // ordinarily happen) so use the fallback.
                return self.generate_fallback();
            }

            // Add edits for the computed ranges.
            self.add_edit_for(
                unformatted_start,
                unformatted_end,
                formatted_start,
                formatted_end,
                allow_any_content_differences,
            );

            // And move the pointers along to after these tokens.
            unformatted_offset = self.unformatted.end(unformatted_token);
            formatted_offset = self.formatted.end(formatted_token);

            // When range formatting, if we've processed a token that ends
            // after the range then there can't be any more relevant edits and
            // we can return early.
            if let Some(range_end) = self.range_end {
                if unformatted_offset > range_end {
                    return self.edits;
                }
            }
        }

        // If we got here and either of the streams still have tokens,
        // something did not match so use the fallback.
        if unformatted_has_more || formatted_has_more {
            return self.generate_fallback();
        }

        // Finally, handle any whitespace that was after the last token.
        let unformatted_length = self.unformatted.text.len() as u32;
        let formatted_length = self.formatted.text.len() as u32;
        self.add_edit_for(
            unformatted_offset,
            unformatted_length,
            formatted_offset,
            formatted_length,
            false,
        );

        self.edits
    }

    /// Dart `_addEditFor`: compares the text between tokens and appends an
    /// edit.
    fn add_edit_for(
        &mut self,
        unformatted_start: u32,
        unformatted_end: u32,
        formatted_start: u32,
        formatted_end: u32,
        allow_any_content_differences: bool,
    ) {
        let unformatted_whitespace =
            &self.unformatted.text[unformatted_start as usize..unformatted_end as usize];
        let formatted_whitespace =
            &self.formatted.text[formatted_start as usize..formatted_end as usize];
        let newline = b'\n' as u16;

        if let (Some(range_start), Some(range_end)) = (self.range_start, self.range_end) {
            // If this change crosses over the start of the requested range,
            // discarding the change may result in leading whitespace of the
            // next line not being formatted correctly.
            //
            // To handle this, if both unformatted/formatted contain at least
            // one newline, split this change into two around the last newline
            // so that the final part (likely leading whitespace) can be
            // included without including the whole change. This cannot be done
            // if the newline is at the end of the source whitespace though, as
            // this would create a split where the first part is the same and
            // the second part is empty, resulting in an infinite loop/stack
            // overflow.
            //
            // Without this, functionality like VS Code's "format modified
            // lines" (which uses Git status to know which lines are edited) may
            // appear to fail to format the first newly added line in a range.
            if unformatted_start < range_start
                && unformatted_end > range_start
                && unformatted_whitespace.contains(&newline)
                && formatted_whitespace.contains(&newline)
                && unformatted_whitespace.last() != Some(&newline)
            {
                // Find the offsets of the character after the last newlines.
                let last_newline = |s: &[u16]| s.iter().rposition(|&c| c == newline).unwrap() + 1;
                let unformatted_offset = last_newline(unformatted_whitespace) as u32;
                let formatted_offset = last_newline(formatted_whitespace) as u32;
                // Call us again for the leading part.
                self.add_edit_for(
                    unformatted_start,
                    unformatted_start + unformatted_offset,
                    formatted_start,
                    formatted_start + formatted_offset,
                    false,
                );
                // Call us again for the trailing part.
                self.add_edit_for(
                    unformatted_start + unformatted_offset,
                    unformatted_end,
                    formatted_start + formatted_offset,
                    formatted_end,
                    false,
                );
                return;
            }

            // If we're formatting only a range, skip over any segments that
            // don't fall entirely within that range.
            if unformatted_start < range_start || unformatted_end > range_end {
                return;
            }
        }

        if unformatted_whitespace == formatted_whitespace {
            return;
        }

        let mut start_offset = unformatted_start;
        let mut end_offset = unformatted_end;
        let mut old_text = unformatted_whitespace;
        let mut new_text = formatted_whitespace;

        // Simplify some common cases where the new whitespace is a subset of
        // the old.
        // Remove common prefixes.
        let common_prefix_length = old_text
            .iter()
            .zip(new_text)
            .take_while(|(a, b)| a == b)
            .count();
        if common_prefix_length != 0 {
            old_text = &old_text[common_prefix_length..];
            new_text = &new_text[common_prefix_length..];
            start_offset += common_prefix_length as u32;
        }

        // Remove common suffixes.
        let common_suffix_length = old_text
            .iter()
            .rev()
            .zip(new_text.iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        if common_suffix_length != 0 {
            old_text = &old_text[..old_text.len() - common_suffix_length];
            new_text = &new_text[..new_text.len() - common_suffix_length];
            end_offset -= common_suffix_length as u32;
        }

        // Unless allowing any differences, validate that the replaced and
        // replacement text only contain characters that we expected the
        // formatter to have changed. If the change contains other characters,
        // it's likely the token offsets used were incorrect and it's better to
        // not modify the code than potentially corrupt it.
        if !allow_any_content_differences
            && (!is_valid_formatter_change(old_text) || !is_valid_formatter_change(new_text))
        {
            return;
        }

        // Finally, append the edit for this whitespace.
        // Note: As with all LSP edits, offsets are based on the original
        // location as they are applied in one shot. They should not account
        // for the previous edits in the same set.
        let new_text = String::from_utf16_lossy(new_text);
        self.edits.push(text_edit(
            self.line_info,
            start_offset,
            end_offset,
            &new_text,
        ));
    }

    /// Dart `_generateFallback`: the results if the edits cannot be
    /// minimized because the token streams differ in an unexpected way (a
    /// bug in the formatter, or in the assumptions about the tokens that
    /// may change).
    fn generate_fallback(self) -> Vec<Value> {
        if self.range_start.is_none() && self.range_end.is_none() {
            // A full document format falls back to one edit for the whole
            // document.
            generate_full_edit(
                self.line_info,
                self.unformatted_source,
                self.formatted_source,
            )
        } else {
            // A range format cannot reduce the edits to the range: format
            // nothing.
            Vec::new()
        }
    }
}

/// Dart `_MinimalEditComputer._substitutes`: whether a token of type [left]
/// has been substituted by the tokens starting at [right] (in [parsed]),
/// and the number of these tokens.
fn substitutes(left: TokenType, parsed: &Parsed, right: TokenId) -> Option<usize> {
    for possible_substitution in allowed_substitutions(left) {
        let mut current = Some(right);
        let mut matched = true;
        for &ty in possible_substitution.iter() {
            match current {
                Some(c) if parsed.ty(c) == ty => {
                    let next = parsed.tokens.next(c);
                    current = next.get();
                }
                _ => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            return Some(possible_substitution.len());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incremental_and_full() {
        let changes = [
            json!({"range": {"start": {"line": 1, "character": 1}, "end": {"line": 1, "character": 2}}, "text": "XY"}),
            json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "text": "😀"}),
            json!({"range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 3}}, "text": "-"}),
        ];
        assert_eq!(
            apply_changes("ab\ncde\n", &changes).unwrap(),
            "😀-b\ncXYe\n"
        );
        assert_eq!(apply_changes("x", &[json!({"text": "y"})]).unwrap(), "y");
        let bad = [
            json!({"range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 0}}, "text": ""}),
        ];
        assert_eq!(
            apply_changes("x", &bad).unwrap_err().code,
            codes::CLIENT_SERVER_INCONSISTENT_STATE
        );
    }
}
