// Dart source: dart_style lib/src/cli/format_command.dart
// Dart source: dart_style lib/src/cli/formatter_options.dart

//! The command line interface of the formatter (`dart format`): the
//! `format` command, its options, where output goes, which file names are
//! shown and the summary.

pub mod format_command;
pub mod formatter_options;
pub mod output;
pub mod show;
pub mod summary;

pub use format_command::{CommandIo, run};
