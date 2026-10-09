// Dart source: dart_style lib/src/cli/summary.dart

//! The kind of summary shown after all formatting is complete.

use std::io::Write;
use std::time::{Duration, Instant};

/// The kind of summary shown after all formatting is complete.
#[derive(Debug)]
pub enum Summary {
    None,

    /// Tracks how many files were formatted and the total time
    /// (Dart `_LineSummary`).
    Line {
        start: Instant,

        /// The number of processed files.
        files: usize,

        /// The number of changed files.
        changed: usize,
    },

    /// Reports how long it took for format each file (Dart `_ProfileSummary`).
    Profile {
        /// The elapsed time it took to format each completed file, in
        /// completion order.
        elapsed: Vec<(String, Duration)>,

        /// The number of files that completed so fast that they aren't worth
        /// tracking.
        elided: usize,
    },
}

impl Summary {
    /// Creates a Summary that tracks how many files were formatted and the total
    /// time.
    pub fn line(start: Instant) -> Summary {
        Summary::Line {
            start,
            files: 0,
            changed: 0,
        }
    }

    /// Creates a Summary that captures profiling information.
    ///
    /// Mostly for internal use.
    pub fn profile() -> Summary {
        Summary::Profile {
            elapsed: Vec::new(),
            elided: 0,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Summary::None)
    }

    /// Describe the processed file at [display_path].
    ///
    /// If the contents of the file are the same as the formatted output,
    /// [changed] will be false. [elapsed] is the time from Dart `beforeFile`
    /// to `afterFile` (used by the profile summary).
    pub fn after_file(&mut self, display_path: &str, changed: bool, elapsed: Duration) {
        match self {
            Summary::None => {}
            Summary::Line {
                files,
                changed: changed_count,
                ..
            } => {
                *files += 1;
                if changed {
                    *changed_count += 1;
                }
            }
            Summary::Profile {
                elapsed: times,
                elided,
            } => {
                if elapsed.as_millis() >= 10 {
                    // Dart stores the times in a map keyed by path.
                    times.retain(|(path, _)| path != display_path);
                    times.push((display_path.to_string(), elapsed));
                } else {
                    *elided += 1;
                }
            }
        }
    }

    /// Show the summary.
    pub fn show(&self, stdout: &mut dyn Write) {
        match self {
            Summary::None => {}
            Summary::Line {
                start,
                files,
                changed,
            } => {
                let elapsed = start.elapsed();
                let time = format!("{:.2}", elapsed.as_millis() as f64 / 1000.0);

                if *files == 0 {
                    let _ = writeln!(stdout, "Formatted no files in {time} seconds.");
                } else if *files == 1 {
                    let _ = writeln!(
                        stdout,
                        "Formatted {files} file ({changed} changed) in {time} seconds."
                    );
                } else {
                    let _ = writeln!(
                        stdout,
                        "Formatted {files} files ({changed} changed) in {time} seconds."
                    );
                }
            }
            Summary::Profile { elapsed, elided } => {
                let mut files: Vec<&(String, Duration)> = elapsed.iter().collect();
                files.sort_by_key(|f| std::cmp::Reverse(f.1));

                for (file, time) in files {
                    let _ = writeln!(stdout, "{}: {file}", dart_duration_to_string(*time));
                }

                if *elided >= 1 {
                    let s = if *elided > 1 { "s" } else { "" };
                    let _ = writeln!(stdout, "...{elided} more file{s} each took less than 10ms.");
                }

                // Dart `Profile.report()` prints nothing: `Profile.enabled` is
                // `false`.
            }
        }
    }
}

/// Dart `Duration.toString()`: `H:MM:SS.mmmmmm`.
pub fn dart_duration_to_string(duration: Duration) -> String {
    let micros = duration.as_micros();
    let hours = micros / 3_600_000_000;
    let minutes = micros / 60_000_000 % 60;
    let seconds = micros / 1_000_000 % 60;
    let fraction = micros % 1_000_000;
    format!("{hours}:{minutes:02}:{seconds:02}.{fraction:06}")
}
