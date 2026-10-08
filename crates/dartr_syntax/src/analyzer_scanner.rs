// Dart source: pkg/analyzer/lib/src/dart/scanner/scanner.dart
// Dart source: pkg/analyzer/lib/src/dart/scanner/translate_error_token.dart

//! The scanner as the analyzer uses it (`parseString`, file analysis): the
//! language version override handling of the analyzer `Scanner`, and the
//! translation of error tokens to analyzer diagnostics, which the analyzer
//! does when the parser skips the error tokens at the start of the stream
//! (`Parser.parseUnit` -> `AstBuilder.handleErrorToken`).

use std::collections::HashSet;

use crate::abstract_scanner::{LanguageVersionInfo, ScannerConfiguration};
use dartr_diagnostics::diag;

use crate::diagnostic::Diagnostic;
use crate::error_token::{ErrorKind, ScannerMessageCode};
use crate::scanner::{ScannerResult, scan_string};
use crate::token::{TokenId, Tokens};
use crate::token_constants::BAD_INPUT_TOKEN;
use crate::token_type::TokenType;

/// Dart `ExperimentStatus.currentVersion` of the pinned SDK (3.13).
pub const CURRENT_LANGUAGE_VERSION: (i64, i64) = (3, 13);

/// The language version where `>>>` was released (`triple_shift`).
const TRIPLE_SHIFT_VERSION: (i64, i64) = (2, 14);

/// The result of [`scan_for_analyzer`].
#[derive(Clone, Debug)]
pub struct AnalyzerScanResult {
    pub scan: ScannerResult,
    /// The first token that is not an error token: the token where the
    /// parser starts (`CompilationUnit.beginToken`).
    pub first: TokenId,
    /// Diagnostics in the order the analyzer reports them when there are no
    /// parse errors: the language version diagnostics (reported while
    /// scanning), then the error tokens.
    pub diagnostics: Vec<Diagnostic>,
    /// Dart `Scanner.overrideVersion` (major, minor).
    pub override_version: Option<(i64, i64)>,
    /// The number of [`Self::diagnostics`] that the scanner reported while
    /// scanning (language version comments); the rest are the translated
    /// error tokens, which the parser reports through
    /// `Listener.handleErrorToken` when the AST is built.
    pub scan_diagnostic_count: usize,
    /// The version that the feature set of the file is restricted to (Dart
    /// `Scanner.featureSet`): the last valid `// @dart = x.y` comment. A
    /// later comment with a too high version resets
    /// [`Self::override_version`] but not the feature set.
    pub feature_version: Option<(i64, i64)>,
}

impl AnalyzerScanResult {
    pub fn tokens(&self) -> &Tokens {
        &self.scan.tokens
    }

    /// Dart `Scanner.lineStarts`: without the additional line after the end.
    pub fn line_starts(&self) -> &[u32] {
        let l = &self.scan.line_starts;
        &l[..l.len().saturating_sub(1)]
    }
}

/// Scans [source] like the analyzer `Scanner` configured for the latest
/// language version (`FeatureSet.latestLanguageVersion()`), with comments.
/// [source] must not start with a byte order mark (see
/// [`crate::scanner::strip_bom`]).
pub fn scan_for_analyzer(source: &str) -> AnalyzerScanResult {
    let mut diagnostics = RecordingDiagnosticListener::default();
    let mut override_version = None;
    let mut feature_version = None;
    let mut callback = |info: LanguageVersionInfo| -> Option<ScannerConfiguration> {
        // Dart `Scanner._languageVersionChanged`.
        if info.major < 0 || info.minor < 0 {
            return None;
        }
        let version = (info.major, info.minor);
        override_version = Some(version);
        if version > CURRENT_LANGUAGE_VERSION {
            diagnostics.on_diagnostic(
                diag::invalid_language_version_override_greater(
                    CURRENT_LANGUAGE_VERSION.0,
                    CURRENT_LANGUAGE_VERSION.1,
                )
                .at_offset(info.offset as usize, info.length as usize)
                .into_diagnostic(),
            );
            override_version = None;
            None
        } else {
            feature_version = Some(version);
            // `_featureSetForOverriding.restrictToVersion(overrideVersion)`.
            Some(ScannerConfiguration {
                enable_triple_shift: version >= TRIPLE_SHIFT_VERSION,
                enable_augmentations: false,
            })
        }
    };
    let scan = scan_string(
        source,
        Some(ScannerConfiguration::default()),
        true,
        Some(&mut callback),
    );

    let scan_diagnostic_count = diagnostics.diagnostics.len();
    let tokens = &scan.tokens;
    let mut first = scan.first;
    let mut error_tokens = Vec::new();
    while tokens.get(first).is_error() {
        error_tokens.push(first);
        first = tokens.next(first);
    }
    for token in error_tokens {
        translate_error_token(tokens, token, &mut |d| diagnostics.on_diagnostic(d));
    }
    AnalyzerScanResult {
        scan,
        first,
        diagnostics: diagnostics.diagnostics,
        override_version,
        scan_diagnostic_count,
        feature_version,
    }
}

/// Dart `RecordingDiagnosticListener`: keeps the diagnostics in a (linked)
/// set, so a diagnostic equal to one reported before (same code, offset,
/// length and message) is dropped.
#[derive(Default, Debug)]
pub struct RecordingDiagnosticListener {
    pub diagnostics: Vec<Diagnostic>,
    seen: HashSet<(&'static str, usize, usize, String)>,
}

impl RecordingDiagnosticListener {
    pub fn on_diagnostic(&mut self, diagnostic: Diagnostic) {
        let key = (
            diagnostic.code.unique_name,
            diagnostic.offset,
            diagnostic.length,
            diagnostic.message.clone(),
        );
        if self.seen.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }
}

/// Dart `translateErrorToken`: translates the error [token] into an analyzer
/// diagnostic and reports it with [report].
pub fn translate_error_token(tokens: &Tokens, token: TokenId, report: &mut dyn FnMut(Diagnostic)) {
    let error = tokens.error(token).expect("not an error token");
    let char_offset = error.char_offset;
    let end_offset = error.end_offset().unwrap_or(char_offset);
    // `makeError`: a diagnostic at `charOffset` with length 1, but never past
    // the end of the input (an error there would not be visible in an
    // editor).
    let make_error = |char_offset: u32, diagnostic: dartr_diagnostics::LocatableDiagnostic| {
        let offset = if is_at_end(tokens, token, char_offset) {
            char_offset.wrapping_sub(1)
        } else {
            char_offset
        };
        diagnostic.at_offset(offset as usize, 1).into_diagnostic()
    };
    let at = |offset: u32, diagnostic: dartr_diagnostics::LocatableDiagnostic| {
        diagnostic.at_offset(offset as usize, 1).into_diagnostic()
    };

    let diagnostic = match error.error_code() {
        ScannerMessageCode::Encoding => at(char_offset, diag::encoding()),
        // Fasta reports the error location as the entire string or comment;
        // analyzer expects the end.
        ScannerMessageCode::UnterminatedString => at(
            end_offset.wrapping_sub(1),
            diag::unterminated_string_literal(),
        ),
        ScannerMessageCode::UnterminatedComment => at(
            end_offset.wrapping_sub(1),
            diag::unterminated_multi_line_comment(),
        ),
        ScannerMessageCode::MissingExponent => {
            make_error(end_offset.wrapping_sub(1), diag::missing_digit())
        }
        ScannerMessageCode::ExpectedHexDigit => {
            make_error(end_offset.wrapping_sub(1), diag::missing_hex_digit())
        }
        ScannerMessageCode::NonAsciiIdentifier
        | ScannerMessageCode::NonAsciiWhitespace
        | ScannerMessageCode::AsciiControlCharacter => {
            let code_point = error.character().unwrap() as i64;
            make_error(char_offset, diag::illegal_character(code_point))
        }
        ScannerMessageCode::UnexpectedSeparatorInNumber => {
            make_error(char_offset, diag::unexpected_separator_in_number())
        }
        ScannerMessageCode::UnsupportedOperator => {
            let ErrorKind::UnsupportedOperator { token: operator } = error.kind else {
                unreachable!()
            };
            make_error(
                char_offset,
                diag::unsupported_operator(tokens.lexeme(operator)),
            )
        }
        ScannerMessageCode::UnmatchedToken => {
            let begin = error.begin().unwrap();
            let end_token = tokens.get(begin).end_group;
            let expected = match tokens.ty(begin) {
                TokenType::OPEN_CURLY_BRACKET | TokenType::STRING_INTERPOLATION_EXPRESSION => "}",
                TokenType::OPEN_SQUARE_BRACKET => "]",
                TokenType::OPEN_PAREN => ")",
                TokenType::LT => ">",
                ty => panic!("UnmatchedToken for {ty:?}"),
            };
            make_error(tokens.offset(end_token), diag::expected_token(expected))
        }
        ScannerMessageCode::UnexpectedDollarInString => {
            make_error(char_offset, diag::missing_identifier())
        }
    };
    report(diagnostic);
}

/// Dart `_isAtEnd`: whether [char_offset], which came from the non-EOF
/// token [token], is the end of the input.
fn is_at_end(tokens: &Tokens, mut token: TokenId, char_offset: u32) -> bool {
    loop {
        // Skip to the next token.
        token = tokens.next(token);
        let t = tokens.get(token);
        // If we've found an EOF token, its charOffset indicates where the end
        // of the input is.
        if t.is_eof() {
            return t.offset == char_offset;
        }
        // If we've found a non-error token, then we know there is additional
        // input text after [charOffset].
        if t.kind() != BAD_INPUT_TOKEN {
            return false;
        }
    }
}
