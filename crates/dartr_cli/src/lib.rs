//! `dartr analyze`: a port of `dart analyze` (`pkg/dartdev/lib/src/commands/analyze.dart`)
//! and of the parts of the analysis server and the analyzer that decide which
//! diagnostics it reports:
//!
//! - [args]: the options and usage errors of `dart analyze` (`package:args` rules);
//! - [provider]: [provider::DiagnosticsProvider], the source of the diagnostics of
//!   each file, and [provider::ParseOnlyProvider] (parse diagnostics and the
//!   AST-only lint rules of `dartr_lints`, run per library with its parts;
//!   enabled rules that need resolution are skipped);
//! - [ignore_info], [ignore_validator]: `// ignore:` and `// ignore_for_file:`
//!   comments (`IgnoreInfo`, `IgnoreValidator`, `_filterIgnoredDiagnostics`);
//! - [server]: protocol `AnalysisError`s with the `errors:` processors of
//!   `analysis_options.yaml` (`mapEngineErrors`);
//! - [output]: the `default`, `json` and `machine` formats;
//! - [analyze]: the command (targets, progress, sorting, exit codes).
//!
//! Not done yet (later phases): the `unnecessary_ignore` lint; plugin
//! diagnostics; `server.error` (exit code 4). Diagnostics of
//! `analysis_options.yaml`, `pubspec.yaml`, `AndroidManifest.xml` and
//! `fix_data.yaml` files come from `dartr_project::non_dart`.

pub mod analyze;
pub mod args;
pub mod ignore_info;
pub mod ignore_validator;
pub mod output;
pub mod provider;
pub mod server;

pub use analyze::{Terminal, USAGE_EXIT_CODE, run};
pub use provider::{AnalyzedFile, DiagnosticsProvider, FileDiagnostics, ParseOnlyProvider};
