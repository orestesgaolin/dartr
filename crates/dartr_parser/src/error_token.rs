// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/error_token.dart (assertionMessage)

//! The CFE message of an error token (Dart `ErrorToken.assertionMessage`).

use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::error_token::{ErrorKind, ScannerMessageCode, close_brace_for, close_quote_for};
use dartr_syntax::{TokenId, Tokens};

/// Dart `ErrorToken.assertionMessage` of the error token [token].
pub fn error_token_assertion_message(tokens: &Tokens, token: TokenId) -> CfeMessage {
    let error = tokens.error(token).expect("not an error token");
    match &error.kind {
        ErrorKind::Encoding => diag::encoding(),
        ErrorKind::NonAsciiIdentifier { character } => {
            let s = char_string(*character);
            diag::non_ascii_identifier(&s, *character as i64)
        }
        ErrorKind::NonAsciiWhitespace { character } => {
            diag::non_ascii_whitespace(*character as i64)
        }
        ErrorKind::AsciiControlCharacter { character } => {
            diag::ascii_control_character(*character as i64)
        }
        ErrorKind::UnsupportedOperator { token } => {
            diag::unsupported_operator(tokens.lexeme(*token))
        }
        ErrorKind::UnterminatedString { start, .. } => {
            diag::unterminated_string(start, close_quote_for(start))
        }
        ErrorKind::UnterminatedToken { code, .. } => match code {
            ScannerMessageCode::UnterminatedComment => diag::unterminated_comment(),
            ScannerMessageCode::MissingExponent => diag::missing_exponent(),
            ScannerMessageCode::ExpectedHexDigit => diag::expected_hex_digit(),
            ScannerMessageCode::UnexpectedDollarInString => diag::unexpected_dollar_in_string(),
            ScannerMessageCode::UnexpectedSeparatorInNumber => {
                diag::unexpected_separator_in_number()
            }
            ScannerMessageCode::Encoding => diag::encoding(),
            code => panic!("unexpected UnterminatedToken code {code:?}"),
        },
        ErrorKind::UnmatchedToken { begin } => {
            let lexeme = tokens.lexeme(*begin);
            diag::unmatched_token(close_brace_for(lexeme), lexeme)
        }
    }
}

/// Dart `String.fromCharCodes([character])`.
fn char_string(character: i32) -> String {
    match char::from_u32(character as u32) {
        Some(c) => c.to_string(),
        None => String::from(char::REPLACEMENT_CHARACTER),
    }
}
