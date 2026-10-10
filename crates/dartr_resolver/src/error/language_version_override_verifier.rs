// Dart source: pkg/analyzer/lib/src/error/language_version_override_verifier.dart

//! Finds invalid or misplaced language version override comments (Dart
//! `LanguageVersionOverrideVerifier`).

use dartr_ast::{Ast, CompilationUnit, Id};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_syntax::{TokenId, TokenType, token::flags};

use super::{UnitVerifier, VerifierHost};

/// Dart `LanguageVersionOverrideVerifier(diagnosticReporter).verify(unit)`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    verify_misplaced(v);

    let ast = v.ast;
    let mut begin_token = ast[v.unit].begin_token;
    if !begin_token.is_none() && ast.tokens.ty(begin_token) == TokenType::SCRIPT_TAG {
        begin_token = ast.tokens.next(begin_token);
    }
    if begin_token.is_none() {
        return;
    }
    let mut comment_token = ast.tokens.get(begin_token).preceding_comments;
    while !comment_token.is_none() {
        if find_language_version_override_comment(v, comment_token) {
            // A valid language version override was found. Do not search
            // for any later invalid language version comments.
            return;
        }
        comment_token = ast.tokens.next(comment_token);
    }
}

/// Dart `CompilationUnit.languageVersionToken`: the language version
/// override comment (`// @dart = x.y`) before the first token (after the
/// script tag), with its major and minor version.
pub fn language_version_token(ast: &Ast, unit: Id<CompilationUnit>) -> Option<(TokenId, u64, u64)> {
    let mut target_token = ast[unit].begin_token;
    if !target_token.is_none() && ast.tokens.ty(target_token) == TokenType::SCRIPT_TAG {
        target_token = ast.tokens.next(target_token);
    }
    if target_token.is_none() {
        return None;
    }
    let mut comment = ast.tokens.get(target_token).preceding_comments;
    while !comment.is_none() {
        if ast.tokens.get(comment).flags & flags::LANGUAGE_VERSION != 0 {
            let (major, minor) = parse_language_version(ast.tokens.lexeme(comment))?;
            return Some((comment, major, minor));
        }
        comment = ast.tokens.next(comment);
    }
    None
}

/// The major and minor version of a `LanguageVersionToken` lexeme
/// (`// @dart = 2.12`): the first two digit sequences after `@dart`.
fn parse_language_version(lexeme: &str) -> Option<(u64, u64)> {
    let rest = &lexeme[lexeme.find("@dart")? + 5..];
    let mut numbers = rest
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().unwrap_or(u64::MAX));
    Some((numbers.next()?, numbers.next()?))
}

/// Dart `_findLanguageVersionOverrideComment(commentToken)`: looks for
/// comments which look almost like a Dart language version override, and
/// reports them. Returns whether [comment_token] is a valid override.
fn find_language_version_override_comment(
    v: &mut UnitVerifier<'_>,
    comment_token: TokenId,
) -> bool {
    let ast = v.ast;
    // Dart strings are UTF-16.
    let comment: Vec<u16> = ast.tokens.lexeme(comment_token).encode_utf16().collect();
    let offset = ast.tokens.offset(comment_token) as usize;
    let length = comment.len();
    let mut index = 0;

    // TODO(srawlins): Actual whitespace.
    let is_whitespace = |c: u16| c == 0x09 || c == 0x20;
    let is_numeric = |c: u16| (0x30..=0x39).contains(&c);
    let is_alphabetical = |c: u16| (0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c);
    let skip_whitespaces = |index: &mut usize| {
        while *index < length && is_whitespace(comment[*index]) {
            *index += 1;
        }
    };

    // Count the number of `/` characters at the beginning.
    while index < length && comment[index] == 0x2F {
        index += 1;
    }
    let slash_count = index;

    skip_whitespaces(&mut index);
    if index == length {
        return false;
    }

    let at_sign_present = comment[index] == 0x40;
    if at_sign_present {
        index += 1;
    }
    if length - index < 4 {
        return false;
    }

    let possible_dart = &comment[index..index + 4];
    let lower: Vec<u16> = possible_dart
        .iter()
        .map(|&c| {
            if (0x41..=0x5A).contains(&c) {
                c + 0x20
            } else {
                c
            }
        })
        .collect();
    if lower != "dart".encode_utf16().collect::<Vec<_>>() {
        return false;
    }
    let is_lower_dart = possible_dart == lower.as_slice();

    index += 4;
    skip_whitespaces(&mut index);
    if index == length {
        return false;
    }

    // The separator between "@dart" and the version number.
    let dart_version_separator_start_index = index;
    // Move through any other consecutive punctuation, whitespace,
    while index < length {
        let c = comment[index];
        if is_numeric(c) || is_alphabetical(c) || is_whitespace(c) {
            break;
        }
        index += 1;
    }
    if index == length {
        return false;
    }

    let dart_version_separator_length = index - dart_version_separator_start_index;
    skip_whitespaces(&mut index);
    if index == length {
        return false;
    }

    let mut contains_invalid_version_number_prefix = false;
    if is_alphabetical(comment[index]) {
        contains_invalid_version_number_prefix = true;
        index += 1;
        if index == length {
            return false;
        }
    }

    if !is_numeric(comment[index]) {
        return false;
    }

    if index + 1 < length && is_alphabetical(comment[index + 1]) {
        return false;
    }

    if !at_sign_present && dart_version_separator_length == 0 {
        // The comment is too different from a valid language version
        // override comment, like "/// dart2 is great".
        return false;
    }

    // At this point, the comment is considered an "attempted" language
    // version override comment. Check for all issues which would make it an
    // invalid language version override comment.
    let report = |v: &mut UnitVerifier<'_>, d: LocatableDiagnostic| {
        v.report(d.at_offset(offset, length));
    };

    if slash_count > 2 {
        report(v, diag::invalid_language_version_override_two_slashes());
        return false;
    }

    if !at_sign_present {
        report(v, diag::invalid_language_version_override_at_sign());
        return false;
    }

    if !is_lower_dart {
        // The 4 characters after `@` are "dart", but in the wrong case.
        report(v, diag::invalid_language_version_override_lower_case());
        return false;
    }

    if dart_version_separator_length != 1 || comment[dart_version_separator_start_index] != 0x3D {
        // The separator between "@dart" and the version number is either
        // not present, or is not a single "=" character.
        report(v, diag::invalid_language_version_override_equals());
        return false;
    }

    if contains_invalid_version_number_prefix {
        report(v, diag::invalid_language_version_override_prefix());
        return false;
    }

    // Nothing preceding the version number makes this comment invalid.
    // Check the format of the version number, and trailing characters.

    // Skip major version.
    while index < length && is_numeric(comment[index]) {
        index += 1;
    }

    // Skip '.' separator.
    if index == length || comment[index] != 0x2E {
        report(v, diag::invalid_language_version_override_number());
        return false;
    }
    index += 1;

    // Skip minor version.
    while index < length && is_numeric(comment[index]) {
        index += 1;
    }

    skip_whitespaces(&mut index);

    // OK, no trailing characters.
    if index == length {
        return true;
    }

    // This comment is a valid language version override, except for
    // trailing characters.
    report(
        v,
        diag::invalid_language_version_override_trailing_characters(),
    );
    false
}

/// Dart `_verifyMisplaced(unit)`: verifies that all language version
/// overrides are before declarations.
fn verify_misplaced(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let unit = &ast[v.unit];
    let first_meaningful_token = if let Some(&first) = ast.list_raw(unit.directives).first() {
        ast.begin_token(first)
    } else if let Some(&first) = ast.list_raw(unit.declarations).first() {
        ast.begin_token(first)
    } else {
        return;
    };
    let first_offset = ast.tokens.offset(first_meaningful_token);
    let at_dart: Vec<u16> = "@dart".encode_utf16().collect();

    let mut token = ast.tokens.next(first_meaningful_token);
    while !token.is_none() {
        if ast.tokens.offset(token) > first_offset {
            let mut comment_token = ast.tokens.get(token).preceding_comments;
            while !comment_token.is_none() {
                let units: Vec<u16> = ast.tokens.lexeme(comment_token).encode_utf16().collect();
                // Optimization: the regular expression is only tried when the
                // comment has an `@` before its last 5 code units.
                let end = units.len().saturating_sub(5);
                let has_at = units[..end].contains(&u16::from(b'@'));
                if has_at
                    && let Some(match_end) = match_override_comment_line(&units)
                    && let Some(at_dart_start) = index_of(&units, &at_dart)
                {
                    let offset = ast.tokens.offset(comment_token) as usize + at_dart_start;
                    let length = match_end.saturating_sub(at_dart_start);
                    v.report(
                        diag::invalid_language_version_override_location()
                            .at_offset(offset, length),
                    );
                }
                comment_token = ast.tokens.next(comment_token);
            }
        }

        let next = ast.tokens.next(token);
        if next == token {
            break;
        }
        token = next;
    }
}

/// The first index of [needle] in [haystack] (Dart `String.indexOf`).
fn index_of(haystack: &[u16], needle: &[u16]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Dart `RegExp(r'^\s*//\s*@dart\s*=\s*\d+\.\d+').firstMatch(lexeme)`: the
/// end of the match (in UTF-16 code units), if the comment matches.
fn match_override_comment_line(units: &[u16]) -> Option<usize> {
    // ECMAScript `\s`: white space and line terminators.
    fn is_space(c: u16) -> bool {
        matches!(
            c,
            0x09..=0x0D
                | 0x20
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
    let skip_spaces = |i: &mut usize| {
        while *i < units.len() && is_space(units[*i]) {
            *i += 1;
        }
    };
    let expect = |i: &mut usize, s: &str| -> bool {
        for c in s.encode_utf16() {
            if *i < units.len() && units[*i] == c {
                *i += 1;
            } else {
                return false;
            }
        }
        true
    };
    let digits = |i: &mut usize| -> bool {
        let start = *i;
        while *i < units.len() && (0x30..=0x39).contains(&units[*i]) {
            *i += 1;
        }
        *i > start
    };
    let mut i = 0;
    skip_spaces(&mut i);
    if !expect(&mut i, "//") {
        return None;
    }
    skip_spaces(&mut i);
    if !expect(&mut i, "@dart") {
        return None;
    }
    skip_spaces(&mut i);
    if !expect(&mut i, "=") {
        return None;
    }
    skip_spaces(&mut i);
    if !digits(&mut i) || !expect(&mut i, ".") || !digits(&mut i) {
        return None;
    }
    Some(i)
}
