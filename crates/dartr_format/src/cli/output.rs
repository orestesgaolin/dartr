// Dart source: dart_style lib/src/cli/output.dart

//! Where formatted code results should go.

use std::io::Write;

use crate::source_code::SourceCode;

/// Where formatted code results should go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Overwrite files on disc.
    Write,

    /// Print the code to the terminal as human-friendly text.
    Show,

    /// Print the code to the terminal as JSON.
    Json,

    /// Do nothing. (Used when the user just wants the list of files that would
    /// be changed.)
    None,
}

impl Output {
    /// Dart `Output.values.byName`.
    pub fn by_name(name: &str) -> Output {
        match name {
            "write" => Output::Write,
            "show" => Output::Show,
            "json" => Output::Json,
            "none" => Output::None,
            _ => panic!("Invalid Output name: {name}"),
        }
    }

    /// Write the file to disc.
    ///
    /// If stdin is being formatted, then [file] is `None`.
    pub fn write_file(
        self,
        stderr: &mut dyn Write,
        file: Option<&str>,
        display_path: &str,
        result: &SourceCode,
    ) -> bool {
        if self != Output::Write {
            return false;
        }

        let file = file.expect("files are written only when formatting paths");
        if let Err(err) = std::fs::write(file, &result.text) {
            let code = err.raw_os_error().unwrap_or(0);
            let _ = writeln!(
                stderr,
                "Could not overwrite {display_path}: {} (error code {code})",
                os_error_message(&err)
            );
        }

        true
    }

    /// Print the file to the terminal in some way.
    pub fn show_file(self, stdout: &mut dyn Write, path: &str, result: &SourceCode) {
        match self {
            Output::Show => {
                // Don't add an extra newline.
                let _ = stdout.write_all(result.text.as_bytes());
            }
            Output::Json => {
                // TODO(rnystrom): Put an empty selection in here to remain compatible
                // with the old formatter. Since there's no way to pass a selection on
                // the command line, this will never be used, which is why it's
                // hard-coded to -1, -1. If we add support for passing in a selection,
                // put the real result here.
                let offset = result.selection_start.map_or(-1, |s| s as i64);
                let length = result.selection_length.map_or(-1, |s| s as i64);
                let _ = writeln!(
                    stdout,
                    "{{\"path\":{},\"source\":{},\"selection\":{{\"offset\":{offset},\"length\":{length}}}}}",
                    json_encode_string(path),
                    json_encode_string(&result.text)
                );
            }
            Output::Write | Output::None => {
                // Do nothing.
            }
        }
    }
}

/// Dart `OSError.message` (the `strerror` text without Rust's
/// " (os error N)" suffix).
fn os_error_message(err: &std::io::Error) -> String {
    let text = err.to_string();
    match text.rfind(" (os error ") {
        Some(index) => text[..index].to_string(),
        None => text,
    }
}

/// Dart `jsonEncode` of a string (`_JsonStringifier.writeStringContent`):
/// escapes `"`, `\`, control characters (`\b \t \n \f \r`, others as
/// `\u00xx` with lowercase hex) and lone surrogates (not possible in Rust
/// strings).
pub fn json_encode_string(text: &str) -> String {
    let mut buffer = String::with_capacity(text.len() + 2);
    buffer.push('"');
    for c in text.chars() {
        match c {
            '"' => buffer.push_str("\\\""),
            '\\' => buffer.push_str("\\\\"),
            '\u{8}' => buffer.push_str("\\b"),
            '\t' => buffer.push_str("\\t"),
            '\n' => buffer.push_str("\\n"),
            '\u{c}' => buffer.push_str("\\f"),
            '\r' => buffer.push_str("\\r"),
            c if (c as u32) < 0x20 => buffer.push_str(&format!("\\u{:04x}", c as u32)),
            c => buffer.push(c),
        }
    }
    buffer.push('"');
    buffer
}
