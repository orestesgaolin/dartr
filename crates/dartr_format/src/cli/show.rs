// Dart source: dart_style lib/src/cli/show.dart

//! Which file paths should be printed.

use std::io::Write;

/// Which file paths should be printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// No files.
    None,

    /// All traversed files.
    All,

    /// Only files whose formatting changed.
    Changed,
}

impl Show {
    /// Dart `Show.values.byName`.
    pub fn by_name(name: &str) -> Show {
        match name {
            "none" => Show::None,
            "all" => Show::All,
            "changed" => Show::Changed,
            _ => panic!("Invalid Show name: {name}"),
        }
    }

    /// Describes a file that was processed.
    ///
    /// Returns whether or not this file should be displayed.
    pub fn file(
        self,
        stdout: &mut dyn Write,
        path: &str,
        changed: bool,
        overwritten: bool,
    ) -> bool {
        match self {
            Show::All => {
                if changed {
                    show_file_change(stdout, path, overwritten);
                } else {
                    let _ = writeln!(stdout, "Unchanged {path}");
                }
                true
            }
            Show::Changed => {
                if changed {
                    show_file_change(stdout, path, overwritten);
                }
                changed
            }
            Show::None => true,
        }
    }
}

fn show_file_change(stdout: &mut dyn Write, path: &str, overwritten: bool) {
    if overwritten {
        let _ = writeln!(stdout, "Formatted {path}");
    } else {
        let _ = writeln!(stdout, "Changed {path}");
    }
}
