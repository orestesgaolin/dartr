// Dart source: pkg/analyzer/messages.yaml (scanner codes only)
// Dart source: pkg/analyzer/lib/src/diagnostic/diagnostic.g.dart

//! The analyzer diagnostic codes that the scanner can report.
//!
//! This is a small local list. A later step replaces it with the codes
//! generated in `dartr_diagnostics`.

/// Dart `DiagnosticSeverity`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    /// Dart `DiagnosticSeverity.name`.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "INFO",
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
        }
    }

    /// Dart `Enum.name` of the severity (`error`, `warning`, `info`).
    pub fn lower_name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// Analyzer diagnostic codes reported by the scanner.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ScannerDiagnosticCode {
    Encoding,
    ExpectedToken,
    IllegalCharacter,
    InvalidLanguageVersionOverrideGreater,
    MissingDigit,
    MissingHexDigit,
    MissingIdentifier,
    UnexpectedSeparatorInNumber,
    UnsupportedOperator,
    UnterminatedMultiLineComment,
    UnterminatedStringLiteral,
}

impl ScannerDiagnosticCode {
    /// Dart `DiagnosticCode.lowerCaseName` (the shared name when there is
    /// one).
    pub fn lower_case_name(self) -> &'static str {
        match self {
            Self::Encoding => "encoding",
            Self::ExpectedToken => "expected_token",
            Self::IllegalCharacter => "illegal_character",
            Self::InvalidLanguageVersionOverrideGreater => "invalid_language_version_override",
            Self::MissingDigit => "missing_digit",
            Self::MissingHexDigit => "missing_hex_digit",
            Self::MissingIdentifier => "missing_identifier",
            Self::UnexpectedSeparatorInNumber => "unexpected_separator_in_number",
            Self::UnsupportedOperator => "unsupported_operator",
            Self::UnterminatedMultiLineComment => "unterminated_multi_line_comment",
            Self::UnterminatedStringLiteral => "unterminated_string_literal",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Self::InvalidLanguageVersionOverrideGreater => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// The `problemMessage` template; `#name` is a parameter.
    pub fn problem_message(self) -> &'static str {
        match self {
            Self::Encoding => "Unable to decode bytes as UTF-8.",
            Self::ExpectedToken => "Expected to find '#token'.",
            Self::IllegalCharacter => "Illegal character '#codePoint'.",
            Self::InvalidLanguageVersionOverrideGreater => {
                "The language version override can't specify a version greater than the latest known language version: #latestMajor.#latestMinor."
            }
            Self::MissingDigit => "Decimal digit expected.",
            Self::MissingHexDigit => "Hexadecimal digit expected.",
            Self::MissingIdentifier => "Expected an identifier.",
            Self::UnexpectedSeparatorInNumber => {
                "Digit separators ('_') in a number literal can only be placed between two digits."
            }
            Self::UnsupportedOperator => "The '#lexeme' operator is not supported.",
            Self::UnterminatedMultiLineComment => "Unterminated multi-line comment.",
            Self::UnterminatedStringLiteral => "Unterminated string literal.",
        }
    }

    /// The `correctionMessage` template, if any.
    pub fn correction_message(self) -> Option<&'static str> {
        match self {
            Self::InvalidLanguageVersionOverrideGreater => {
                Some("Try removing the language version override.")
            }
            Self::UnexpectedSeparatorInNumber => Some("Try removing the '_'."),
            Self::UnterminatedMultiLineComment => Some(
                "Try terminating the comment with '*/', or removing any unbalanced occurrences of '/*' (because comments nest in Dart).",
            ),
            _ => None,
        }
    }
}

/// A diagnostic at a location (Dart `Diagnostic`, without the source).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub code: ScannerDiagnosticCode,
    /// UTF-16 offset.
    pub offset: u32,
    /// UTF-16 length.
    pub length: u32,
    /// The problem message with the arguments filled in.
    pub message: String,
    pub correction: Option<String>,
}

impl Diagnostic {
    /// Creates a diagnostic; `arguments` are `(name, value)` pairs for the
    /// `#name` placeholders of the message templates.
    pub fn new(
        code: ScannerDiagnosticCode,
        offset: u32,
        length: u32,
        arguments: &[(&str, &str)],
    ) -> Diagnostic {
        let format = |template: &str| {
            let mut s = template.to_string();
            for (name, value) in arguments {
                s = s.replace(&format!("#{name}"), value);
            }
            s
        };
        Diagnostic {
            code,
            offset,
            length,
            message: format(code.problem_message()),
            correction: code.correction_message().map(format),
        }
    }

    pub fn severity(&self) -> Severity {
        self.code.severity()
    }
}
