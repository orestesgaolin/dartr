//! Diagnostic listeners and the diagnostic reporter.
//!
//! Port of `DiagnosticListener`, `RecordingDiagnosticListener`
//! (`pkg/analyzer/lib/error/listener.dart`) and `DiagnosticReporter`
//! (`pkg/analyzer/lib/src/error/listener.dart`).

use crate::args::DiagnosticArg;
use crate::code::DiagnosticCode;
use crate::diagnostic::{Diagnostic, DiagnosticMessage, LocatedDiagnostic};

/// Receives diagnostics (`DiagnosticListener`).
pub trait DiagnosticListener {
    fn on_diagnostic(&mut self, diagnostic: Diagnostic);
}

/// A listener that drops all diagnostics (`DiagnosticListener.nullListener`).
#[derive(Debug, Default)]
pub struct NullDiagnosticListener;

impl DiagnosticListener for NullDiagnosticListener {
    fn on_diagnostic(&mut self, _: Diagnostic) {}
}

/// A listener that keeps all diagnostics in report order
/// (`RecordingDiagnosticListener`).
#[derive(Debug, Default)]
pub struct RecordingDiagnosticListener {
    pub diagnostics: Vec<Diagnostic>,
}

impl RecordingDiagnosticListener {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DiagnosticListener for RecordingDiagnosticListener {
    fn on_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

/// A listener that only records whether a diagnostic was reported
/// (`BooleanDiagnosticListener`).
#[derive(Debug, Default)]
pub struct BooleanDiagnosticListener {
    pub diagnostic_reported: bool,
}

impl DiagnosticListener for BooleanDiagnosticListener {
    fn on_diagnostic(&mut self, _: Diagnostic) {
        self.diagnostic_reported = true;
    }
}

/// Any closure is a listener: `let mut f = |d| out.push(d);`.
impl<F: FnMut(Diagnostic)> DiagnosticListener for F {
    fn on_diagnostic(&mut self, diagnostic: Diagnostic) {
        self(diagnostic)
    }
}

/// Creates diagnostics and sends them to a listener (`DiagnosticReporter`).
///
/// One reporter is used for one source file. Scanner, parser and resolver
/// all report through this type.
pub struct DiagnosticReporter<'a> {
    listener: &'a mut dyn DiagnosticListener,
    /// If greater than zero, diagnostics are dropped (used to ignore
    /// diagnostics inside comments).
    pub lock_level: u32,
}

impl<'a> DiagnosticReporter<'a> {
    pub fn new(listener: &'a mut dyn DiagnosticListener) -> Self {
        DiagnosticReporter {
            listener,
            lock_level: 0,
        }
    }

    /// Reports a located diagnostic (`report(LocatedDiagnostic)`), usually
    /// made with a generated function:
    /// `reporter.report(diag::undefined_class("Foo").at_offset(10, 3))`.
    pub fn report(&mut self, located: LocatedDiagnostic) {
        if self.lock_level != 0 {
            return;
        }
        self.listener.on_diagnostic(located.into_diagnostic());
    }

    /// Reports `code` with untyped `arguments` (`atOffset`).
    pub fn at_offset(
        &mut self,
        offset: usize,
        length: usize,
        code: &'static DiagnosticCode,
        arguments: &[DiagnosticArg],
        context_messages: Vec<DiagnosticMessage>,
    ) {
        if self.lock_level != 0 {
            return;
        }
        self.listener.on_diagnostic(Diagnostic::with_arguments(
            code,
            offset,
            length,
            arguments,
            context_messages,
        ));
    }

    /// Reports an already created diagnostic (`reportError`).
    pub fn report_diagnostic(&mut self, diagnostic: Diagnostic) {
        if self.lock_level != 0 {
            return;
        }
        self.listener.on_diagnostic(diagnostic);
    }
}
