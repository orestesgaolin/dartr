//! Diagnostics.
//!
//! Port of `Diagnostic` (`pkg/analyzer/lib/diagnostic/diagnostic.dart`),
//! `DiagnosticMessage`, `LocatableDiagnostic` and `LocatedDiagnostic`
//! (`pkg/analyzer/lib/src/diagnostic/diagnostic.dart`).

use crate::args::{DiagnosticArg, convert_type_names};
use crate::code::{DiagnosticCode, DiagnosticSeverity};
use crate::format::format_list;

/// A message attached to a diagnostic, with its own location
/// (`DiagnosticMessage`). Used for context messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticMessage {
    /// The absolute, normalized path of the file.
    pub file_path: String,
    pub offset: i64,
    pub length: i64,
    pub message: String,
    pub url: Option<String>,
}

impl DiagnosticMessage {
    /// `messageText(includeUrl: ...)`.
    pub fn message_text(&self, include_url: bool) -> String {
        match (&self.url, include_url) {
            (Some(url), true) => {
                let mut result = self.message.clone();
                if !result.ends_with('.') {
                    result.push('.');
                }
                result.push_str("  See ");
                result.push_str(url);
                result
            }
            _ => self.message.clone(),
        }
    }
}

/// A diagnostic reported for a source range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static DiagnosticCode,
    /// Offset in UTF-16 code units, like the analyzer.
    pub offset: usize,
    /// Length in UTF-16 code units.
    pub length: usize,
    /// The formatted problem message.
    pub message: String,
    /// The formatted correction message.
    pub correction: Option<String>,
    /// The severity. Starts as the default severity of the code; analysis
    /// options can change it later.
    pub severity: DiagnosticSeverity,
    pub context_messages: Vec<DiagnosticMessage>,
}

impl Diagnostic {
    /// Creates a diagnostic and formats its messages (`Diagnostic.tmp`). The
    /// arguments must already be text.
    pub fn new<S: AsRef<str>>(
        code: &'static DiagnosticCode,
        offset: usize,
        length: usize,
        arguments: &[S],
        context_messages: Vec<DiagnosticMessage>,
    ) -> Diagnostic {
        Diagnostic {
            code,
            offset,
            length,
            message: format_list(code.problem_message, arguments),
            correction: code.correction_message.map(|c| format_list(c, arguments)),
            severity: code.severity(),
            context_messages,
        }
    }

    /// Creates a diagnostic from typed arguments. Type and element arguments
    /// are converted like `DiagnosticReporter._createDiagnostic` does
    /// (`convertTypeNames`), which can add context messages.
    pub fn with_arguments(
        code: &'static DiagnosticCode,
        offset: usize,
        length: usize,
        arguments: &[DiagnosticArg],
        mut context_messages: Vec<DiagnosticMessage>,
    ) -> Diagnostic {
        let (texts, extra) = convert_type_names(arguments);
        context_messages.extend(extra);
        Diagnostic::new(code, offset, length, &texts, context_messages)
    }

    /// The end offset (`offset + length`).
    pub fn end(&self) -> usize {
        self.offset + self.length
    }
}

/// A diagnostic code with its arguments, but without a location
/// (`LocatableDiagnostic`). The generated functions in [`crate::diag`]
/// return this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatableDiagnostic {
    pub code: &'static DiagnosticCode,
    pub arguments: Vec<DiagnosticArg>,
    pub context_messages: Vec<DiagnosticMessage>,
}

impl LocatableDiagnostic {
    pub fn new(code: &'static DiagnosticCode, arguments: Vec<DiagnosticArg>) -> Self {
        LocatableDiagnostic {
            code,
            arguments,
            context_messages: Vec::new(),
        }
    }

    /// Adds context messages (`withContextMessages`).
    pub fn with_context_messages(
        mut self,
        messages: impl IntoIterator<Item = DiagnosticMessage>,
    ) -> Self {
        self.context_messages.extend(messages);
        self
    }

    /// Gives the diagnostic a location (`atOffset`).
    pub fn at_offset(self, offset: usize, length: usize) -> LocatedDiagnostic {
        LocatedDiagnostic {
            diagnostic: self,
            offset,
            length,
        }
    }

    /// Creates the [`Diagnostic`] at the given location.
    pub fn to_diagnostic(&self, offset: usize, length: usize) -> Diagnostic {
        Diagnostic::with_arguments(
            self.code,
            offset,
            length,
            &self.arguments,
            self.context_messages.clone(),
        )
    }
}

impl From<&'static DiagnosticCode> for LocatableDiagnostic {
    fn from(code: &'static DiagnosticCode) -> Self {
        LocatableDiagnostic::new(code, Vec::new())
    }
}

/// A [`LocatableDiagnostic`] with a location (`LocatedDiagnostic`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedDiagnostic {
    pub diagnostic: LocatableDiagnostic,
    pub offset: usize,
    pub length: usize,
}

impl LocatedDiagnostic {
    pub fn into_diagnostic(self) -> Diagnostic {
        let LocatableDiagnostic {
            code,
            arguments,
            context_messages,
        } = self.diagnostic;
        Diagnostic::with_arguments(code, self.offset, self.length, &arguments, context_messages)
    }
}
