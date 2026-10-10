// Dart source: pkg/analysis_server_plugin/lib/edit/dart/correction_producer.dart (CorrectionProducer, CorrectionProducerContext, CorrectionApplicability, MultiCorrectionProducer)

//! The correction producers: a producer computes the edits of one fix for
//! one diagnostic (or selection) in a resolved unit.

use std::rc::Rc;

use dartr_ast::*;
use dartr_diagnostics::Diagnostic;
use dartr_element::{Ctx, ElementId, ResolutionTables};
use dartr_project::AnalysisOptions;
use dartr_syntax::{LineInfo, TokenId};

use super::change_builder::ChangeBuilder;
use super::code_style::CodeStyleOptions;
use super::fix_kind::FixKind;
use super::utils::{CorrectionUtils, RangeFactory};
use crate::server::ResolvedUnitRef;

/// Dart `CorrectionApplicability`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    SingleLocation,
    AcrossSingleFile,
    AcrossFiles,
    AutomaticallyButOncePerFile,
}

/// Dart `CorrectionProducerContext` and the getters of
/// `ResolvedCorrectionProducer`.
pub struct ProducerContext<'a> {
    pub resolved: &'a Rc<ResolvedUnitRef>,
    pub ctx: &'a Ctx<'a>,
    pub ast: &'a Ast,
    pub unit: Id<CompilationUnit>,
    pub tables: &'a ResolutionTables,
    pub utils: &'a CorrectionUtils<'a>,
    pub line_info: &'a LineInfo,
    /// The path of the unit (Dart `file`).
    pub path: &'a str,
    pub options: &'a Rc<AnalysisOptions>,
    pub diagnostic: Option<&'a Diagnostic>,
    /// The diagnostics of the unit (Dart `unitResult.diagnostics`).
    pub diagnostics: &'a [Diagnostic],
    pub selection_offset: u32,
    pub selection_length: u32,
    /// Dart `node`: the node covering the selection.
    pub node: NodeId,
    /// Dart `token`.
    pub token: TokenId,
    pub applying_bulk_fixes: bool,
}

impl<'a> ProducerContext<'a> {
    /// Dart `CorrectionProducerContext.createResolved`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        resolved: &'a Rc<ResolvedUnitRef>,
        ctx: &'a Ctx<'a>,
        utils: &'a CorrectionUtils<'a>,
        path: &'a str,
        options: &'a Rc<AnalysisOptions>,
        diagnostic: Option<&'a Diagnostic>,
        diagnostics: &'a [Diagnostic],
        selection_offset: u32,
        selection_length: u32,
        applying_bulk_fixes: bool,
    ) -> Self {
        let unit_ref = resolved.unit();
        let ast = &unit_ref.ast;
        let unit = unit_ref.unit;
        let node = ast
            .node_covering(unit, selection_offset, selection_length)
            .unwrap_or(unit.raw());
        let token = token_at(ast, node, selection_offset).unwrap_or_else(|| ast.begin_token(node));
        ProducerContext {
            resolved,
            ctx,
            ast,
            unit,
            tables: &unit_ref.tables,
            utils,
            line_info: resolved.line_info(),
            path,
            options,
            diagnostic,
            diagnostics,
            selection_offset,
            selection_length,
            node,
            token,
            applying_bulk_fixes,
        }
    }

    /// The element locator of the unit.
    pub fn locator(&self) -> crate::element_locator::Unit<'_, 'a> {
        crate::element_locator::Unit {
            ctx: self.ctx,
            ast: self.ast,
            tables: self.tables,
        }
    }

    pub fn range(&self) -> RangeFactory<'a> {
        RangeFactory::new(self.ast)
    }

    pub fn code_style(&self) -> CodeStyleOptions<'_> {
        CodeStyleOptions {
            options: self.options,
        }
    }

    /// Dart `getCodeStyleOptions(...).isLintEnabled` / `isLintEnabled`.
    pub fn is_lint_enabled(&self, name: &str) -> bool {
        self.code_style().is_lint_enabled(name)
    }

    /// Dart `diagnosticOffset`/`diagnosticLength`.
    pub fn diagnostic_range(&self) -> Option<(u32, u32)> {
        self.diagnostic.map(|d| (d.offset as u32, d.length as u32))
    }

    /// Dart `coveringNode` of the diagnostic (`coveringNode` getter of
    /// `ResolvedCorrectionProducer`).
    pub fn covering_node(&self) -> Option<NodeId> {
        let (offset, length) = self.diagnostic_range()?;
        self.ast.node_covering(self.unit, offset, length)
    }

    /// The element of [node] (Dart `ElementLocator.locate`).
    pub fn element_of(&self, node: NodeId) -> Option<ElementId> {
        self.locator().element(node)
    }

    /// The source text of the unit.
    pub fn content(&self) -> &str {
        &self.ast.tokens.source
    }

    /// Dart `eol` (`utils.endOfLine`).
    pub fn eol(&self) -> &str {
        &self.utils.end_of_line
    }

    pub fn token_offset(&self, t: TokenId) -> u32 {
        self.ast.tokens.get(t).offset
    }

    pub fn token_end(&self, t: TokenId) -> u32 {
        self.ast.tokens.get(t).end()
    }

    pub fn lexeme(&self, t: TokenId) -> &str {
        self.ast.tokens.lexeme(t)
    }
}

/// Dart `CorrectionProducerContext._tokenAt`.
fn token_at(ast: &Ast, node: NodeId, offset: u32) -> Option<TokenId> {
    for entity in ast.child_entities(node) {
        match entity {
            Entity::Node(n) => {
                if ast.offset(n) <= offset && offset <= ast.end(n) {
                    return token_at(ast, n, offset);
                }
            }
            Entity::Token(t) => {
                let token = ast.tokens.get(t);
                if token.offset <= offset && offset <= token.end() {
                    return Some(t);
                }
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
    None
}

/// Dart `CorrectionProducer`.
pub trait CorrectionProducer {
    /// Dart `fixKind`; `None` when the producer is not a fix.
    fn fix_kind(&self) -> Option<&'static FixKind>;

    /// Dart `multiFixKind`.
    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        None
    }

    /// Dart `fixArguments`.
    fn fix_arguments(&self) -> Vec<String> {
        Vec::new()
    }

    /// Dart `applicability`.
    fn applicability(&self) -> Applicability;

    /// Dart `canBeAppliedAcrossSingleFile`.
    fn can_be_applied_across_single_file(&self) -> bool {
        self.applicability() != Applicability::SingleLocation
    }

    /// Dart `compute`.
    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>);
}

/// Dart `ProducerGenerator`.
pub type ProducerGenerator = fn(&ProducerContext<'_>) -> Box<dyn CorrectionProducer>;

/// Dart `MultiProducerGenerator` (`MultiCorrectionProducer.producers`).
pub type MultiProducerGenerator = fn(
    &ProducerContext<'_>,
    &mut dyn super::change_builder::ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>>;
