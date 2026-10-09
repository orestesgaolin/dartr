//! The front end of the "tall" style (dart_style `lib/src/front_end`): AST
//! to pieces.

use dartr_ast::{Ast, NodeId};
use dartr_syntax::LineInfo;

use crate::dart_formatter::DartFormatter;
use crate::dart_version_history::Version;
use crate::exceptions::FormatError;
use crate::source_code::SourceCode;

/// Formats [node] (a compilation unit or a statement) in the tall style.
/// This is the part of Dart `DartFormatter.formatSource` that creates
/// `AstNodeVisitor(FormattingStyle(...), lineInfo, unitSourceCode)` and
/// calls `visitor.run(unitSourceCode, node)`.
#[allow(clippy::too_many_arguments)]
pub fn format_tall(
    formatter: &DartFormatter,
    line_ending: &str,
    language_version: Version,
    page_width_from_comment: Option<usize>,
    ast: &Ast,
    line_info: &LineInfo,
    source: &SourceCode,
    node: NodeId,
) -> Result<SourceCode, FormatError> {
    let _ = (
        formatter,
        line_ending,
        language_version,
        page_width_from_comment,
        ast,
        line_info,
        source,
        node,
    );
    Err(FormatError::Other("tall style: not ported yet".into()))
}
