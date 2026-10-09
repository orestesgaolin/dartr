//! The "short" style (dart_style `lib/src/short`), used for language
//! versions before 3.7.

use dartr_ast::{Ast, NodeId};
use dartr_syntax::LineInfo;

use crate::dart_formatter::DartFormatter;
use crate::exceptions::FormatError;
use crate::source_code::SourceCode;

/// Formats [node] (a compilation unit or a statement) in the short style.
/// This is the part of Dart `DartFormatter.formatSource` that creates
/// `SourceVisitor(this, lineInfo, unitSourceCode)` and calls
/// `visitor.run(node, inferredLineEnding)`.
pub fn format_short(
    formatter: &DartFormatter,
    ast: &Ast,
    line_info: &LineInfo,
    source: &SourceCode,
    node: NodeId,
    line_ending: &str,
) -> Result<SourceCode, FormatError> {
    let _ = (formatter, ast, line_info, source, node, line_ending);
    Err(FormatError::Other("short style: not ported yet".into()))
}
