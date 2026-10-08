// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/error_token.dart
// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/recover.dart

//! Error tokens. The scanner puts them at the start of the token stream
//! (see `AbstractScanner::prepend_error_token`).

use crate::token::TokenId;

/// The front end message code of an error token (Dart `ErrorToken.errorCode`,
/// from `_fe_analyzer_shared/messages.yaml`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ScannerMessageCode {
    AsciiControlCharacter,
    Encoding,
    ExpectedHexDigit,
    MissingExponent,
    NonAsciiIdentifier,
    NonAsciiWhitespace,
    UnexpectedDollarInString,
    UnexpectedSeparatorInNumber,
    UnmatchedToken,
    UnsupportedOperator,
    UnterminatedComment,
    UnterminatedString,
}

impl ScannerMessageCode {
    /// Dart `Code.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::AsciiControlCharacter => "AsciiControlCharacter",
            Self::Encoding => "Encoding",
            Self::ExpectedHexDigit => "ExpectedHexDigit",
            Self::MissingExponent => "MissingExponent",
            Self::NonAsciiIdentifier => "NonAsciiIdentifier",
            Self::NonAsciiWhitespace => "NonAsciiWhitespace",
            Self::UnexpectedDollarInString => "UnexpectedDollarInString",
            Self::UnexpectedSeparatorInNumber => "UnexpectedSeparatorInNumber",
            Self::UnmatchedToken => "UnmatchedToken",
            Self::UnsupportedOperator => "UnsupportedOperator",
            Self::UnterminatedComment => "UnterminatedComment",
            Self::UnterminatedString => "UnterminatedString",
        }
    }
}

/// The subclass of Dart `ErrorToken` and its fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// `EncodingErrorToken`.
    Encoding,
    /// `NonAsciiIdentifierToken`.
    NonAsciiIdentifier { character: i32 },
    /// `NonAsciiWhitespaceToken`.
    NonAsciiWhitespace { character: i32 },
    /// `AsciiControlCharacterToken`.
    AsciiControlCharacter { character: i32 },
    /// `UnsupportedOperator`: `token` is the `===` or `!==` token.
    UnsupportedOperator { token: TokenId },
    /// `UnterminatedString`: `start` is the opening quote, with `r` if raw.
    UnterminatedString {
        start: &'static str,
        end_offset: u32,
    },
    /// `UnterminatedToken`.
    UnterminatedToken {
        code: ScannerMessageCode,
        end_offset: u32,
    },
    /// `UnmatchedToken`: `begin` is the begin token without a matching end.
    UnmatchedToken { begin: TokenId },
}

/// Dart `ErrorToken`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorToken {
    pub kind: ErrorKind,
    pub char_offset: u32,
}

impl ErrorToken {
    /// Dart `ErrorToken.errorCode`.
    pub fn error_code(&self) -> ScannerMessageCode {
        match &self.kind {
            ErrorKind::Encoding => ScannerMessageCode::Encoding,
            ErrorKind::NonAsciiIdentifier { .. } => ScannerMessageCode::NonAsciiIdentifier,
            ErrorKind::NonAsciiWhitespace { .. } => ScannerMessageCode::NonAsciiWhitespace,
            ErrorKind::AsciiControlCharacter { .. } => ScannerMessageCode::AsciiControlCharacter,
            ErrorKind::UnsupportedOperator { .. } => ScannerMessageCode::UnsupportedOperator,
            ErrorKind::UnterminatedString { .. } => ScannerMessageCode::UnterminatedString,
            ErrorKind::UnterminatedToken { code, .. } => *code,
            ErrorKind::UnmatchedToken { .. } => ScannerMessageCode::UnmatchedToken,
        }
    }

    /// Dart `ErrorToken.character`.
    pub fn character(&self) -> Option<i32> {
        match self.kind {
            ErrorKind::NonAsciiIdentifier { character }
            | ErrorKind::NonAsciiWhitespace { character }
            | ErrorKind::AsciiControlCharacter { character } => Some(character),
            _ => None,
        }
    }

    /// Dart `ErrorToken.endOffset`.
    pub fn end_offset(&self) -> Option<u32> {
        match self.kind {
            ErrorKind::UnterminatedString { end_offset, .. }
            | ErrorKind::UnterminatedToken { end_offset, .. } => Some(end_offset),
            _ => None,
        }
    }

    /// Dart `ErrorToken.begin`.
    pub fn begin(&self) -> Option<TokenId> {
        match self.kind {
            ErrorKind::UnmatchedToken { begin } => Some(begin),
            _ => None,
        }
    }

    /// Dart `ErrorToken.length`.
    pub fn length(&self) -> u32 {
        match self.kind {
            ErrorKind::UnterminatedString { end_offset, .. } => end_offset - self.char_offset,
            _ => 1,
        }
    }
}

/// Dart `buildUnexpectedCharacterToken`.
pub fn build_unexpected_character_token(character: i32, char_offset: u32) -> ErrorToken {
    let kind = if character < 0x1f {
        ErrorKind::AsciiControlCharacter { character }
    } else {
        match character {
            // unicodeReplacementCharacter
            0xFFFD => ErrorKind::Encoding,
            0x00A0 | 0x1680 | 0x180E | 0x2000 | 0x2001 | 0x2002 | 0x2003 | 0x2004 | 0x2005
            | 0x2006 | 0x2007 | 0x2008 | 0x2009 | 0x200A | 0x200B | 0x2028 | 0x2029 | 0x202F
            | 0x205F | 0x3000 | 0xFEFF => ErrorKind::NonAsciiWhitespace { character },
            _ => ErrorKind::NonAsciiIdentifier { character },
        }
    };
    ErrorToken { kind, char_offset }
}

/// Dart `closeBraceFor` (recover.dart).
pub fn close_brace_for(open_brace: &str) -> &'static str {
    match open_brace {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        "<" => ">",
        "${" => "}",
        _ => panic!("no close brace for {open_brace}"),
    }
}

/// Dart `closeQuoteFor` (recover.dart).
pub fn close_quote_for(open_quote: &str) -> &'static str {
    match open_quote {
        "\"" | "r\"" => "\"",
        "'" | "r'" => "'",
        "\"\"\"" | "r\"\"\"" => "\"\"\"",
        "'''" | "r'''" => "'''",
        _ => panic!("no close quote for {open_quote}"),
    }
}
