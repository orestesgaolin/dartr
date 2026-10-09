// Dart source: dart_style lib/src/exceptions.dart

//! Errors of the formatter.

use std::fmt;

/// One error of a [FormatterException] (Dart `Diagnostic`): offsets are in
/// UTF-16 code units of [source].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatterError {
    /// Dart `diagnosticCode.uniqueName`, for example
    /// `ParserErrorCode.EXPECTED_TOKEN`.
    pub unique_name: String,
    pub offset: usize,
    pub length: usize,
    /// Dart `Diagnostic.message`.
    pub message: String,
    /// Dart `error.source.contents.data`: the text that was parsed (for a
    /// statement, the text with the wrapper function).
    pub source: String,
    /// Dart `error.source.fullName`: the URI of the file, or the empty
    /// string.
    pub path: String,
}

/// Thrown when one or more errors occurs while parsing the code to be
/// formatted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatterException {
    /// The [Diagnostic]s that occurred.
    pub errors: Vec<FormatterError>,
}

impl FormatterException {
    /// Creates a human-friendly representation of the analysis errors.
    pub fn message(&self, color: bool) -> String {
        crate::source_span::formatter_exception_message(self, color)
    }
}

impl fmt::Display for FormatterException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(false))
    }
}

/// Exception thrown when the internal sanity check that only whitespace
/// changes are made fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnexpectedOutputException {
    /// The source being formatted.
    pub input: String,
    /// The resulting output.
    pub output: String,
}

impl fmt::Display for UnexpectedOutputException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The formatter produced unexpected output. Input was:\n{}\nWhich formatted to:\n{}",
            self.input, self.output
        )
    }
}

/// The errors that [crate::DartFormatter::format_source] can return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// The source has syntax errors.
    Formatter(FormatterException),
    /// The formatter changed more than whitespace (a bug in the formatter).
    UnexpectedOutput(UnexpectedOutputException),
    /// Dart `ArgumentError` and other internal errors.
    Other(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Formatter(e) => e.fmt(f),
            FormatError::UnexpectedOutput(e) => e.fmt(f),
            FormatError::Other(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for FormatError {}
