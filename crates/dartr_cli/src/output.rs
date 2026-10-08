// Dart source: pkg/dartdev/lib/src/commands/analyze.dart (emitDefaultFormat, emitJsonFormat, emitMachineFormat)
// Dart source: pkg/dartdev/lib/src/utils.dart (wrapText, trimEnd)
// Dart source: third_party/pkg/cli_util/lib/cli_logging.dart (Ansi)

//! The three output formats of `dart analyze`.

use std::fmt::Write as _;

use dartr_diagnostics::DiagnosticSeverity;
use dartr_project::paths;

use crate::server::{AnalysisError, Location};

/// `Ansi` of `cli_logging`: escape codes when stdout is a terminal that
/// supports them, otherwise empty strings.
#[derive(Clone, Copy, Debug)]
pub struct Ansi {
    pub use_ansi: bool,
}

impl Ansi {
    fn code(&self, code: &'static str) -> &'static str {
        if self.use_ansi { code } else { "" }
    }
    pub fn bullet(&self) -> &'static str {
        if self.use_ansi { "\u{2022}" } else { "-" }
    }
    pub fn green(&self) -> &'static str {
        self.code("\x1b[32m")
    }
    pub fn none(&self) -> &'static str {
        self.code("\x1b[0m")
    }
    pub fn error(&self, msg: &str) -> String {
        format!(
            "{}{}{msg}{}",
            self.code("\x1b[31m"),
            self.code("\x1b[1m"),
            self.none()
        )
    }
    pub fn emphasized(&self, msg: &str) -> String {
        format!("{}{msg}{}", self.code("\x1b[1m"), self.none())
    }
}

const SEVERITY_WIDTH: usize = 7;
const BODY_INDENT_WIDTH: usize = SEVERITY_WIDTH + 3;
const BODY_INDENT: &str = "          ";

/// The severity name of the protocol (`INFO`, `WARNING`, `ERROR`).
fn severity_name(s: DiagnosticSeverity) -> &'static str {
    s.name()
}

/// dartdev `_relativePath`: [given] relative to [from] (the current
/// directory if `None`), if that is not longer.
fn relative_path(given: &str, from: &str) -> String {
    let relative = paths::relative(given, from);
    if relative.encode_utf16().count() <= given.encode_utf16().count() {
        relative
    } else {
        given.to_string()
    }
}

/// dartdev `trimEnd`.
fn trim_end<'a>(s: &'a str, suffix: &str) -> &'a str {
    s.strip_suffix(suffix).unwrap_or(s)
}

/// dartdev `wrapText` (indices are UTF-16 code units, like Dart).
pub fn wrap_text(text: &str, width: Option<usize>) -> String {
    let Some(width) = width else {
        return text.to_string();
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    let sub = |a: usize, b: usize| String::from_utf16_lossy(&units[a..b]);
    let last_index_of_space = |from: usize| -> Option<usize> {
        let start = from.min(units.len().saturating_sub(1));
        (0..=start).rev().find(|&i| units[i] == b' ' as u16)
    };
    let index_of_space = |from: usize| (from..units.len()).find(|&i| units[i] == b' ' as u16);
    let mut buffer = String::new();
    let mut line_max_end = width;
    let mut line_start = 0;
    loop {
        if line_max_end >= units.len() {
            buffer.push_str(&sub(line_start, units.len()));
            break;
        }
        let mut last_space = last_index_of_space(line_max_end);
        if last_space.is_none_or(|i| i <= line_start) {
            last_space = index_of_space(line_max_end);
            if last_space.is_none() {
                buffer.push_str(&sub(line_start, units.len()));
                break;
            }
        }
        let last_space = last_space.unwrap();
        buffer.push_str(&sub(line_start, last_space));
        buffer.push('\n');
        line_start = last_space + 1;
        line_max_end = line_start + width;
    }
    buffer
}

/// dartdev `emitDefaultFormat`. [relative_to] is the absolute, resolved
/// directory for relative paths, or the current directory. [line_length]
/// is the terminal width (`dartdevUsageLineLength`), `None` if stdout is
/// not a terminal.
pub fn emit_default_format(
    out: &mut String,
    errors: &[AnalysisError],
    relative_to: &str,
    verbose: bool,
    ansi: Ansi,
    line_length: Option<usize>,
) {
    let bullet = ansi.bullet();
    out.push('\n');
    let wrap_width = line_length.map(|l| l.saturating_sub(BODY_INDENT_WIDTH));
    for error in errors {
        let mut severity = format!(
            "{:>width$}",
            severity_name(error.severity).to_lowercase(),
            width = SEVERITY_WIDTH
        );
        if error.severity == DiagnosticSeverity::Error {
            severity = ansi.error(&severity);
        }
        let file_path = relative_path(&error.location.file, relative_to);
        let code_ref = match (&error.url, verbose) {
            (Some(url), true) => url.as_str(),
            _ => error.code.as_str(),
        };
        let mut message = ansi.emphasized(&error.message);
        if let Some(correction) = &error.correction {
            message.push(' ');
            message.push_str(correction);
        }
        let location = format!(
            "{file_path}:{}:{}",
            error.location.start_line, error.location.start_column
        );
        let output = format!(
            "{location} {bullet} {message} {bullet} {}{code_ref}{}",
            ansi.green(),
            ansi.none()
        );
        let output = wrap_text(&output, wrap_width);
        let _ = writeln!(
            out,
            "{severity} {bullet} {}",
            output.replace('\n', &format!("\n{BODY_INDENT}"))
        );
        for message in &error.context_messages {
            let context_path = relative_path(&message.location.file, relative_to);
            let fragment = trim_end(&message.message, ".");
            let _ = writeln!(
                out,
                "{BODY_INDENT} - {fragment} at {context_path}:{}:{}.",
                message.location.start_line, message.location.start_column
            );
        }
    }
    out.push('\n');
}

/// Dart `json.encode` of a string.
fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_location(out: &mut String, location: &Location) {
    out.push_str("{\"file\":");
    json_string(out, &location.file);
    let _ = write!(
        out,
        ",\"range\":{{\"start\":{{\"offset\":{},\"line\":{},\"column\":{}}},\"end\":{{\"offset\":{},\"line\":{},\"column\":{}}}}}}}",
        location.offset,
        location.start_line,
        location.start_column,
        location.offset + location.length,
        location.end_line,
        location.end_column
    );
}

/// dartdev `emitJsonFormat` (one line, with the newline).
pub fn emit_json_format(out: &mut String, errors: &[AnalysisError], memory_kb: Option<u64>) {
    out.push_str("{\"version\":1,\"diagnostics\":[");
    for (i, error) in errors.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"code\":");
        json_string(out, &error.code);
        out.push_str(",\"severity\":");
        json_string(out, severity_name(error.severity));
        out.push_str(",\"type\":");
        json_string(out, error.type_);
        out.push_str(",\"location\":");
        json_location(out, &error.location);
        out.push_str(",\"problemMessage\":");
        json_string(out, &error.message);
        if let Some(correction) = &error.correction {
            out.push_str(",\"correctionMessage\":");
            json_string(out, correction);
        }
        if !error.context_messages.is_empty() {
            out.push_str(",\"contextMessages\":[");
            for (j, message) in error.context_messages.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str("{\"location\":");
                json_location(out, &message.location);
                out.push_str(",\"message\":");
                json_string(out, &message.message);
                out.push('}');
            }
            out.push(']');
        }
        if let Some(url) = &error.url {
            out.push_str(",\"documentation\":");
            json_string(out, url);
        }
        out.push('}');
    }
    out.push(']');
    if let Some(memory) = memory_kb {
        let _ = write!(out, ",\"memory\":{memory}");
    }
    out.push_str("}\n");
}

/// dartdev `_escapeForMachineMode`.
fn escape_for_machine_mode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\\' | '|' => {
                result.push('\\');
                result.push(c);
            }
            c => result.push(c),
        }
    }
    result
}

/// dartdev `emitMachineFormat`.
pub fn emit_machine_format(out: &mut String, errors: &[AnalysisError]) {
    for error in errors {
        let _ = writeln!(
            out,
            "{}|{}|{}|{}|{}|{}|{}|{}",
            severity_name(error.severity),
            error.type_,
            error.code.to_uppercase(),
            escape_for_machine_mode(&error.location.file),
            error.location.start_line,
            error.location.start_column,
            error.location.length,
            escape_for_machine_mode(&error.message)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_text() {
        assert_eq!(wrap_text("aaa bbb ccc", Some(5)), "aaa\nbbb\nccc");
        assert_eq!(wrap_text("aaaaaaa bb", Some(3)), "aaaaaaa\nbb");
        assert_eq!(wrap_text("abc", None), "abc");
    }

    #[test]
    fn escapes_machine_mode() {
        assert_eq!(
            escape_for_machine_mode("a|b\\c\nd\re"),
            "a\\|b\\\\c\\nd\\re"
        );
    }
}
