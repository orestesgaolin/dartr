//! The front end of the "tall" style (dart_style `lib/src/front_end`): AST
//! to pieces.

pub mod ast_node_visitor;
pub mod chain_builder;
pub mod comment_writer;
pub mod delimited_list_builder;
pub mod expression_contents;
pub mod formatting_style;
pub mod piece_factory;
pub mod piece_writer;
pub mod sequence_builder;
pub mod type_builder;

use dartr_ast::{Ast, NodeId};
use dartr_syntax::LineInfo;

use crate::dart_formatter::DartFormatter;
use crate::dart_version_history::Version;
use crate::exceptions::FormatError;
use crate::source_code::SourceCode;

use ast_node_visitor::AstNodeVisitor;
use formatting_style::FormattingStyle;

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
    let style = FormattingStyle::new(
        formatter,
        line_ending,
        Some(language_version),
        page_width_from_comment,
    );
    let visitor = AstNodeVisitor::new(style, ast, line_info, source);
    Ok(visitor.run(source, node))
}
