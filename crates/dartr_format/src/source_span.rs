// Dart source: dart_style lib/src/exceptions.dart (FormatterException.message)
// Dart source: package:source_span (SourceSpan.message, highlighter)

//! The error message of [FormatterException]: Dart
//! `span.message(error.message, color: color)` of `package:source_span`.

use crate::exceptions::FormatterException;

/// Dart `FormatterException.message({bool? color})`.
///
/// TODO(port): port `SourceSpan.message` and the `Highlighter` of
/// `package:source_span` faithfully (line/column header, the `╷ │ ^` code
/// excerpt, colors). This placeholder writes the header line only.
pub fn formatter_exception_message(exception: &FormatterException, _color: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("Could not format because the source could not be parsed:\n");

    // In case we get a huge series of cascaded errors, just show the first few.
    let shown = exception.errors.len().min(10);
    for error in &exception.errors[..shown] {
        let line_info = dartr_syntax::LineInfo::from_content(&error.source);
        let location = line_info.get_location(error.offset as u32);
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(&format!(
            "line {}, column {} of {}: {}",
            location.line_number, location.column_number, error.path, error.message
        ));
    }

    if shown != exception.errors.len() {
        buffer.push('\n');
        buffer.push_str(&format!(
            "({} more errors...)",
            exception.errors.len() - shown
        ));
    }
    buffer
}
