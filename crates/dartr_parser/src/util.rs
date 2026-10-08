// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/util.dart

//! Token helpers of the parser.

use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

/// Dart `noLength` (`messages/codes.dart`).
pub const NO_LENGTH: u32 = 1;

/// Returns true if [token] is the symbol or keyword [value].
#[inline]
pub fn optional(tokens: &Tokens, value: &str, token: TokenId) -> bool {
    tokens.ty(token).string_value() == Some(value)
}

/// Returns the token before the close brace, bracket, or parenthesis
/// associated with [left]. For '<', it may return `None`.
pub fn before_close_brace_token_for(tokens: &Tokens, left: TokenId) -> Option<TokenId> {
    let end_token = tokens.get(left).end_group.get()?;
    let mut token = left;
    let mut next = tokens.next(token);
    while next != end_token && next != tokens.next(next) {
        token = next;
        next = tokens.next(token);
    }
    Some(token)
}

/// Return [token] or a token before [token] which is either not synthetic or
/// synthetic with non-zero length.
pub fn find_previous_non_zero_length_token(tokens: &Tokens, mut token: TokenId) -> TokenId {
    loop {
        let t = tokens.get(token);
        if !(t.is_synthetic() && t.length == 0) {
            break;
        }
        // Dart `beforeSynthetic` is null for tokens that are not synthetic
        // tokens (a zero length non-synthetic token).
        let previous = if t.flags & dartr_syntax::token::flags::SYNTHETIC != 0 {
            t.before_synthetic
        } else {
            TokenId::NONE
        };
        if previous == token {
            panic!("token == token.beforeSynthetic");
        }
        if previous.is_none() {
            break;
        }
        token = previous;
    }
    token
}

/// Return [token] or a token after [token] which is either not synthetic or
/// synthetic with non-zero length. This may return EOF if there are no more
/// non-synthetic tokens in the stream.
pub fn find_non_zero_length_token(tokens: &Tokens, mut token: TokenId) -> TokenId {
    loop {
        let t = tokens.get(token);
        if !(t.is_synthetic() && t.length == 0 && !t.is_eof()) {
            return token;
        }
        token = t.next;
    }
}

pub fn is_digit(c: i32) -> bool {
    (0x30..=0x39).contains(&c)
}

pub fn is_letter(c: i32) -> bool {
    (0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c)
}

pub fn is_letter_or_digit(c: i32) -> bool {
    is_letter(c) || is_digit(c)
}

pub fn is_whitespace(c: i32) -> bool {
    c == 0x20 || c == 0xA || c == 0xD || c == 0x9
}

pub fn is_any_of(tokens: &Tokens, token: TokenId, values: &[TokenType]) -> bool {
    values.contains(&tokens.ty(token))
}

/// A null-aware alternative to `token.length`. If [token] is `None`,
/// returns [`NO_LENGTH`].
pub fn length_for_token(tokens: &Tokens, token: Option<TokenId>) -> u32 {
    match token {
        None => NO_LENGTH,
        Some(t) => tokens.get(t).length,
    }
}

/// Returns the length of the span from [begin] to [end] (inclusive). If both
/// tokens are null, return [`NO_LENGTH`]. If one of the tokens are null,
/// return the length of the other token.
pub fn length_of_span(tokens: &Tokens, begin: Option<TokenId>, end: Option<TokenId>) -> u32 {
    let Some(begin) = begin else {
        return length_for_token(tokens, end);
    };
    let Some(end) = end else {
        return length_for_token(tokens, Some(begin));
    };
    let e = tokens.get(end);
    e.offset
        .wrapping_add(e.length)
        .wrapping_sub(tokens.get(begin).offset)
}

pub fn skip_metadata(tokens: &Tokens, mut token: TokenId) -> TokenId {
    token = tokens.next(token);
    debug_assert!(tokens.ty(token) == TokenType::AT);
    let mut next = tokens.next(token);
    // Corresponds to 'ensureIdentifier' in [parseMetadata].
    if tokens.get(next).is_identifier() {
        token = next;
        next = tokens.next(token);
        // Corresponds to 'parseQualifiedRestOpt' in [parseMetadata].
        if tokens.ty(next) == TokenType::PERIOD {
            token = next;
            next = tokens.next(token);
            if tokens.get(next).is_identifier() {
                token = next;
                next = tokens.next(token);
            }
        }
        // Corresponds to 'computeTypeParamOrArg' in [parseMetadata].
        if tokens.ty(next) == TokenType::LT
            && !tokens.get(tokens.get(next).end_group).is_synthetic()
        {
            token = tokens.get(next).end_group;
            next = tokens.next(token);
        }

        // The extra .identifier after arguments in [parseMetadata].
        if tokens.ty(next) == TokenType::PERIOD {
            token = next;
            next = tokens.next(token);
            if tokens.get(next).is_identifier() {
                token = next;
                next = tokens.next(token);
            }
        }

        // Corresponds to 'parseArgumentsOpt' in [parseMetadata].
        if tokens.ty(next) == TokenType::OPEN_PAREN
            && !tokens.get(tokens.get(next).end_group).is_synthetic()
        {
            token = tokens.get(next).end_group;
        }
    }
    token
}

/// Splits [token] into tokens of the types [parts], at consecutive offsets.
/// The first part keeps the comments of [token]; the last part's `next` is
/// `token.next` (without setting `previous` of that token). Returns the
/// first part. Dart `new SimpleToken(...)..setNext(...)`.
fn split(tokens: &mut Tokens, token: TokenId, parts: &[TokenType]) -> TokenId {
    let t = tokens.get(token);
    let offset = t.offset;
    let comments = t.preceding_comments;
    let next = t.next;
    let byte = tokens.byte_offset(token);
    let mut ids = Vec::with_capacity(parts.len());
    let mut delta = 0u32;
    for (i, &ty) in parts.iter().enumerate() {
        let id = tokens.push_simple(
            ty,
            offset + delta,
            byte + delta,
            if i == 0 { comments } else { TokenId::NONE },
        );
        delta += ty.lexeme().len() as u32;
        ids.push(id);
    }
    let last = *ids.last().unwrap();
    // Set next rather than calling Token.setNext so that the previous token
    // is not set.
    tokens.get_mut(last).next = next;
    for i in (0..ids.len() - 1).rev() {
        tokens.set_next(ids[i], ids[i + 1]);
    }
    ids[0]
}

/// Split `>=` into two separate tokens. Call `Tokens::set_next` to add the
/// token to the stream.
pub fn split_gt_eq(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_EQ);
    split(tokens, token, &[TokenType::GT, TokenType::EQ])
}

/// Split `>>` into two separate tokens.
pub fn split_gt_gt(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_GT);
    split(tokens, token, &[TokenType::GT, TokenType::GT])
}

/// Split `>>=` into three separate tokens.
pub fn split_gt_gt_eq(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_GT_EQ);
    split(tokens, token, &[TokenType::GT, TokenType::GT, TokenType::EQ])
}

/// Split `>>=` into two separate tokens... `>` followed by `>=`.
pub fn split_gt_from_gt_gt_eq(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_GT_EQ);
    split(tokens, token, &[TokenType::GT, TokenType::GT_EQ])
}

/// Split `>>>` into two separate tokens... `>` followed by `>>`.
pub fn split_gt_from_gt_gt_gt(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_GT_GT);
    split(tokens, token, &[TokenType::GT, TokenType::GT_GT])
}

/// Split `>>>=` into two separate tokens... `>` followed by `>>=`.
pub fn split_gt_from_gt_gt_gt_eq(tokens: &mut Tokens, token: TokenId) -> TokenId {
    debug_assert!(tokens.ty(token) == TokenType::GT_GT_GT_EQ);
    split(tokens, token, &[TokenType::GT, TokenType::GT_GT_EQ])
}

/// Strips separator characters (underscore) from [source].
///
/// No validation is performed on [source]; it could be a valid int, a valid
/// double, or invalid.
pub fn strip_separators(source: &str) -> String {
    source.chars().filter(|&c| c != '_').collect()
}

/// Return a synthetic `>` followed by [next]. Call `Tokens::set_next` to
/// add the token to the stream.
pub fn synthetic_gt(tokens: &mut Tokens, next: TokenId) -> TokenId {
    let offset = tokens.get(next).offset;
    let byte = tokens.byte_offset(next);
    let gt = tokens.push_synthetic(TokenType::GT, offset, byte);
    // Set next rather than calling Token.setNext so that the previous token
    // is not set.
    tokens.get_mut(gt).next = next;
    gt
}

/// Returns the boolean value from a 'true' or 'false' [token].
pub fn bool_from_token(tokens: &Tokens, token: TokenId) -> bool {
    let value = tokens.ty(token) == Keyword::TRUE;
    debug_assert!(value || tokens.ty(token) == Keyword::FALSE);
    value
}

/// Returns the integer value from an integer literal token (Dart
/// `int.tryParse`: decimal, or hexadecimal with `0x`; 64-bit).
///
/// If [has_separators], separator characters, '_', are stripped before
/// parsing the token text.
///
/// `None` is returned if the token text could not be parsed as an integer
/// value.
pub fn int_from_token(tokens: &Tokens, token: TokenId, has_separators: bool) -> Option<i64> {
    let lexeme = tokens.lexeme(token);
    let text = if has_separators {
        std::borrow::Cow::Owned(strip_separators(lexeme))
    } else {
        std::borrow::Cow::Borrowed(lexeme)
    };
    dart_int_try_parse(&text)
}

/// Dart `int.tryParse(source)` for the forms of integer literals: decimal
/// digits, or `0x`/`0X` and hexadecimal digits (hexadecimal values may use
/// all 64 bits, like Dart on the VM).
pub fn dart_int_try_parse(text: &str) -> Option<i64> {
    let (negative, body) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = u64::from_str_radix(hex, 16).ok()?;
        let value = value as i64;
        return Some(if negative { value.wrapping_neg() } else { value });
    }
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if negative {
        format!("-{body}").parse::<i64>().ok()
    } else {
        body.parse::<i64>().ok()
    }
}

/// Returns the double value from an double literal token.
///
/// If [has_separators], separator characters, '_', are stripped before
/// parsing the token text.
pub fn double_from_token(tokens: &Tokens, token: TokenId, has_separators: bool) -> f64 {
    let lexeme = tokens.lexeme(token);
    if has_separators {
        strip_separators(lexeme).parse().unwrap_or(f64::NAN)
    } else {
        lexeme.parse().unwrap_or(f64::NAN)
    }
}
