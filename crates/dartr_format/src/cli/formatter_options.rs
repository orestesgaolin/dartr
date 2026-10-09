// Dart source: dart_style lib/src/cli/formatter_options.dart

//! Global options parsed from the command line that affect how the formatter
//! produces and uses its outputs.

use std::time::Duration;

use crate::cli::output::Output;
use crate::cli::show::Show;
use crate::cli::summary::Summary;
use crate::dart_formatter::TrailingCommas;
use crate::dart_version_history::Version;
use crate::io::Console;
use crate::source_code::SourceCode;

// Do not edit directly. Use tool/bump_version.dart.
pub const DART_STYLE_VERSION: &str = "3.1.13";

/// Global options parsed from the command line that affect how the formatter
/// produces and uses its outputs.
#[derive(Debug)]
pub struct FormatterOptions {
    /// The language version formatted code should be parsed at or `None` if not
    /// specified.
    pub language_version: Option<Version>,

    /// The number of spaces of indentation to prefix the output with.
    pub indent: usize,

    /// The number of columns that formatted output should be constrained to fit
    /// within or `None` if not specified.
    ///
    /// If omitted, the formatter defaults to a page width of
    /// [DartFormatter::DEFAULT_PAGE_WIDTH].
    pub page_width: Option<usize>,

    /// How trailing commas in the input source code affect formatting.
    pub trailing_commas: Option<TrailingCommas>,

    /// Whether symlinks should be traversed when formatting a directory.
    pub follow_links: bool,

    /// Which affected files should be shown.
    pub show: Show,

    /// Where formatted code should be output.
    pub output: Output,

    pub summary: Summary,

    /// Sets the exit code to 1 if any changes are made.
    pub set_exit_if_changed: bool,

    /// Flags to enable experimental language features.
    ///
    /// See dart.dev/go/experiments for details.
    pub experiment_flags: Vec<String>,

    /// Dart's global `exitCode`.
    pub exit_code: i32,
}

impl FormatterOptions {
    /// Describe the processed file at [file] with formatted [result]s.
    ///
    /// If the contents of the file are the same as the formatted output,
    /// [changed] will be false.
    ///
    /// If stdin is being formatted, then [file] is `None`. [elapsed] is the
    /// time it took to format the file (Dart `beforeFile` to `afterFile`).
    pub fn after_file(
        &mut self,
        console: &mut Console<'_>,
        file: Option<&str>,
        display_path: &str,
        result: &SourceCode,
        changed: bool,
        elapsed: Duration,
    ) {
        self.summary.after_file(display_path, changed, elapsed);

        // Save the results to disc.
        let mut overwritten = false;
        if changed {
            overwritten = self
                .output
                .write_file(console.stderr, file, display_path, result);
        }

        // Show the user.
        if self
            .show
            .file(console.stdout, display_path, changed, overwritten)
        {
            self.output.show_file(console.stdout, display_path, result);
        }

        // Set the exit code.
        if self.set_exit_if_changed && changed {
            self.exit_code = 1;
        }
    }
}
