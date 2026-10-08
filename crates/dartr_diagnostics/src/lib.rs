//! Diagnostic codes and diagnostic reporting.
//!
//! Generated from the `messages.yaml` files of `pkg/_fe_analyzer_shared`,
//! `pkg/analyzer`, `pkg/linter` and `pkg/analysis_server` (Dart SDK 3.13.3)
//! by `tools/diagnostics_gen`. Regenerate with `tools/regen_diagnostics.sh`.
//!
//! Reporting a diagnostic:
//!
//! ```
//! use dartr_diagnostics::{diag, DiagnosticReporter, RecordingDiagnosticListener, TypeArg};
//!
//! let mut listener = RecordingDiagnosticListener::new();
//! let mut reporter = DiagnosticReporter::new(&mut listener);
//! reporter.report(diag::undefined_class("Foo").at_offset(10, 3));
//! reporter.report(diag::abstract_class_member().at_offset(0, 8));
//! reporter.report(
//!     diag::argument_type_not_assignable(TypeArg::new("int"), TypeArg::new("String"), "")
//!         .at_offset(20, 1),
//! );
//! let d = &listener.diagnostics[0];
//! assert_eq!(d.code, &diag::UNDEFINED_CLASS);
//! assert_eq!(d.message, "Undefined class 'Foo'.");
//! ```

mod args;
pub mod cfe;
mod code;
mod diagnostic;
mod format;
mod generated;
mod reporter;

pub use args::{DiagnosticArg, ElementArg, ElementRef, TypeArg, convert_type_names};
pub use code::{
    DiagnosticCode, DiagnosticSeverity, DiagnosticType, Origin, Parameter, ParameterType,
    RemovedDiagnosticCode, all_codes, code_by_unique_name, codes_by_name, removed_codes,
};
pub use diagnostic::{Diagnostic, DiagnosticMessage, LocatableDiagnostic, LocatedDiagnostic};
pub use format::format_list;
pub use reporter::{
    BooleanDiagnosticListener, DiagnosticListener, DiagnosticReporter, NullDiagnosticListener,
    RecordingDiagnosticListener,
};

/// Analyzer diagnostic codes: `diag::UNDEFINED_CLASS` (the descriptor) and
/// `diag::undefined_class(name)` (a typed constructor).
pub mod diag {
    pub use crate::generated::diag::*;
}

/// CFE codes of the shared scanner and parser: `cfe_codes::ASCII_CONTROL_CHARACTER`
/// and `cfe_codes::ascii_control_character(code_point)`.
pub mod cfe_codes {
    pub use crate::generated::cfe_codes::*;
}
