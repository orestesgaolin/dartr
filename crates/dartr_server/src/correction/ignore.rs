// Dart source: pkg/analysis_server_plugin/lib/src/correction/ignore_diagnostic.dart

//! The producers that ignore a diagnostic: on its line, in the whole file,
//! or in `analysis_options.yaml`.

use dartr_ast::*;
use dartr_ast_builder::ignore_info::IgnoreInfo;
use dartr_diagnostics::DiagnosticSeverity;
use dartr_project::analysis_options::DiagnosticSeverity as OptionsSeverity;

use super::change_builder::{ChangeBuilder, FileEditBuilder};
use super::fix_kind::*;
use super::producer::*;
use super::utils::documentation_comment;

/// Which ignore producer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IgnoreKind {
    Line,
    File,
    AnalysisOptions,
}

/// Dart `IgnoreDiagnosticOnLine`, `IgnoreDiagnosticInFile` and
/// `IgnoreDiagnosticInAnalysisOptionsFile`.
pub struct IgnoreDiagnostic {
    pub kind: IgnoreKind,
    code: String,
}

impl IgnoreDiagnostic {
    pub fn new(kind: IgnoreKind, c: &ProducerContext<'_>) -> Self {
        // Dart `_code` (dartr has no plugin registries).
        let code = c
            .diagnostic
            .map(|d| d.code.lower_case_name().to_string())
            .unwrap_or_default();
        IgnoreDiagnostic { kind, code }
    }

    /// Dart `_isCodeUnignorable`.
    fn is_code_unignorable(&self, c: &ProducerContext<'_>) -> bool {
        let Some(d) = c.diagnostic else {
            return true;
        };
        let severity = match d.code.severity() {
            DiagnosticSeverity::Info => Some(OptionsSeverity::Info),
            DiagnosticSeverity::Warning => Some(OptionsSeverity::Warning),
            DiagnosticSeverity::Error => Some(OptionsSeverity::Error),
            DiagnosticSeverity::None => None,
        };
        let codes: Vec<(&str, OptionsSeverity)> = severity
            .map(|s| vec![(self.code.as_str(), s)])
            .unwrap_or_default();
        let unignorable = c.options.unignorable_code_names(&codes);
        if unignorable.iter().any(|n| n.to_lowercase() == self.code) {
            return true;
        }
        c.options
            .error_processors
            .iter()
            .any(|p| p.severity.is_none() && p.code == self.code)
    }

    /// Dart `_DartIgnoreDiagnostic.insertAt`.
    fn insert_at(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut FileEditBuilder<'_, '_>,
        offset: u32,
        empty_line_before: bool,
        empty_line_after: bool,
    ) {
        let eol = builder.eol();
        let prefix = if empty_line_before { eol.as_str() } else { "" };
        let indent = c.utils.get_line_prefix(offset);
        let comment_prefix = if self.kind == IgnoreKind::Line {
            "ignore"
        } else {
            "ignore_for_file"
        };
        let comment = format!("// {comment_prefix}: {}", self.code);
        let suffix = if empty_line_after { eol.as_str() } else { "" };
        builder.add_simple_insertion(offset, &format!("{prefix}{indent}{comment}{eol}{suffix}"));
    }

    fn compute_line(&self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let Some(d) = c.diagnostic else { return };
        builder.add_dart_file_edit(c.path, |b| {
            let offset = d.offset as u32;
            let line_number = c.line_info.get_location(offset).line_number - 1;
            if line_number == 0 {
                self.insert_at(c, b, 0, false, false);
                return;
            }
            let previous_line_start = c.line_info.line_starts[line_number as usize - 1];
            let line_start = c.line_info.line_starts[line_number as usize];
            let line = c.utils.text.slice(previous_line_start, line_start);
            if IgnoreInfo::is_ignore_comment(line.trim()) {
                let colon = line
                    .encode_utf16()
                    .position(|u| u == b':' as u16)
                    .unwrap_or(0) as u32;
                b.add_simple_insertion(
                    previous_line_start + colon + 1,
                    &format!(" {},", self.code),
                );
            } else {
                self.insert_at(c, b, line_start, false, false);
            }
        });
    }

    fn compute_file(&self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        builder.add_dart_file_edit(c.path, |b| {
            let line_count = c.line_info.line_count();
            if line_count == 1 {
                self.insert_at(c, b, 0, false, true);
                return;
            }
            let mut last_blank_line_offset: Option<u32> = None;
            let mut line_start = 0;
            for line_number in 0..line_count - 1 {
                line_start = c.line_info.line_starts[line_number];
                let next_line_start = c.line_info.line_starts[line_number + 1];
                let line = c.utils.text.slice(line_start, next_line_start);
                let trimmed = line.trim();
                if IgnoreInfo::is_ignore_for_file_comment(trimmed) {
                    let colon = line
                        .encode_utf16()
                        .position(|u| u == b':' as u16)
                        .unwrap_or(0) as u32;
                    b.add_simple_insertion(line_start + colon + 1, &format!(" {},", self.code));
                    return;
                }
                if trimmed.is_empty() {
                    last_blank_line_offset = Some(line_start);
                    continue;
                }
                if trimmed.starts_with("#!") || trimmed.starts_with("//") {
                    continue;
                }
                if let Some(node) = c.utils.find_node(c.unit.raw(), line_start) {
                    let member = c.ast.this_or_ancestor_matching(node, |ast, n| {
                        ast.is::<CompilationUnitMember>(n)
                    });
                    let reference = member
                        .and_then(|m| documentation_comment(c.ast, m))
                        .unwrap_or(node);
                    line_start = c.ast.offset(reference);
                    break;
                }
                break;
            }
            if let Some(offset) = last_blank_line_offset {
                self.insert_at(c, b, offset, true, false);
            } else {
                self.insert_at(c, b, line_start, false, true);
            }
        });
    }
}

impl CorrectionProducer for IgnoreDiagnostic {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.kind {
            IgnoreKind::Line => &IGNORE_ERROR_LINE,
            IgnoreKind::File => &IGNORE_ERROR_FILE,
            IgnoreKind::AnalysisOptions => &IGNORE_ERROR_ANALYSIS_FILE,
        })
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.code.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        if self.is_code_unignorable(c) {
            return;
        }
        match self.kind {
            IgnoreKind::Line => self.compute_line(c, builder),
            IgnoreKind::File => self.compute_file(c, builder),
            IgnoreKind::AnalysisOptions => {
                super::yaml_edit::ignore_in_analysis_options(c, &self.code, builder)
            }
        }
    }
}
