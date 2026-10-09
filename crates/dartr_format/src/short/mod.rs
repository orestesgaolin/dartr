// Dart source: dart_style lib/src/short

//! The "short" style (dart_style `lib/src/short`), used for language
//! versions before 3.7.
//!
//! [source_visitor::SourceVisitor] walks the AST and feeds the
//! [chunk_builder::ChunkBuilder], which produces a tree of chunks with rules
//! and spans. [line_writer::LineWriter] divides the chunks into ranges and
//! runs the [line_splitting::line_splitter::LineSplitter] on each.
//!
//! The objects that Dart links by reference (chunks, rules, spans, nesting
//! levels) are in an [arena::Arena].

pub mod argument_list_visitor;
pub mod arena;
pub mod call_chain_visitor;
pub mod chunk;
pub mod chunk_builder;
pub mod line_splitting;
pub mod line_writer;
pub mod nesting_builder;
pub mod nesting_level;
pub mod rule;
pub mod source_comment;
pub mod source_visitor;

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
    // Dart throws for unsupported nodes (`ThrowingAstVisitor`) and in a few
    // "unreachable" states; the port panics there. Report those as errors.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let visitor = source_visitor::SourceVisitor::new(formatter, ast, line_info, source);
        visitor.run(node, line_ending)
    }));
    result.map_err(|error| {
        let message = if let Some(message) = error.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = error.downcast_ref::<&str>() {
            message.to_string()
        } else {
            "short style formatter failed".to_string()
        };
        FormatError::Other(message)
    })
}
