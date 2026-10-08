// Dart source: pkg/analyzer/lib/diagnostic/diagnostic.dart (Diagnostic)

//! The diagnostics that the scanner reports are analyzer diagnostics of
//! `dartr_diagnostics` (the codes of `pkg/analyzer/messages.yaml`).

pub use dartr_diagnostics::{Diagnostic, DiagnosticSeverity};

/// Dart `Diagnostic.severity` as the lower case name of the public
/// `Severity` enum (`error`, `warning`, `info`).
pub fn severity_lower_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::None => "none",
    }
}
