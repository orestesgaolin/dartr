// Dart source: pkg/analysis_server/lib/src/protocol_server.dart (mapEngineErrors, newAnalysisError_fromEngine)
// Dart source: pkg/analyzer/lib/source/error_processor.dart (ErrorProcessor.getProcessor)
// Dart source: pkg/dartdev/lib/src/analysis_server.dart (AnalysisError)

//! The server side of `dart analyze`: engine diagnostics become protocol
//! `AnalysisError`s, with the `errors:` processors of `analysis_options.yaml`
//! applied.

use std::cmp::Ordering;
use std::collections::HashMap;

use dartr_diagnostics::{Diagnostic, DiagnosticSeverity};
use dartr_project::AnalysisContextCollection;
use dartr_project::analysis_options::DiagnosticSeverity as OptionsSeverity;
use dartr_syntax::LineInfo;

use crate::provider::FileDiagnostics;

/// Protocol `Location`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    pub offset: usize,
    pub length: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Protocol `DiagnosticMessage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextMessage {
    pub message: String,
    pub location: Location,
}

/// Protocol `AnalysisError` (the fields that `dart analyze` reads).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisError {
    pub severity: DiagnosticSeverity,
    /// `DiagnosticType.name`, for example `SYNTACTIC_ERROR`.
    pub type_: &'static str,
    pub location: Location,
    pub message: String,
    pub correction: Option<String>,
    /// Lower case name of the code.
    pub code: String,
    pub url: Option<String>,
    pub context_messages: Vec<ContextMessage>,
}

impl AnalysisError {
    /// dartdev `_AnalysisSeverity.index`: error, warning, info, none.
    fn severity_index(&self) -> u8 {
        match self.severity {
            DiagnosticSeverity::Error => 0,
            DiagnosticSeverity::Warning => 1,
            DiagnosticSeverity::Info => 2,
            DiagnosticSeverity::None => 3,
        }
    }

    /// dartdev `AnalysisError.compareTo`: severity, file, offset, message.
    pub fn compare(&self, other: &AnalysisError) -> Ordering {
        self.severity_index()
            .cmp(&other.severity_index())
            .then_with(|| compare_utf16(&self.location.file, &other.location.file))
            .then_with(|| self.location.offset.cmp(&other.location.offset))
            .then_with(|| compare_utf16(&self.message, &other.message))
    }
}

/// Dart `String.compareTo`: compares UTF-16 code units.
pub fn compare_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn location(file: &str, offset: usize, length: usize, line_info: &LineInfo) -> Location {
    let start = line_info.get_location(offset as u32);
    let end = line_info.get_location((offset + length) as u32);
    Location {
        file: file.to_string(),
        offset,
        length,
        start_line: start.line_number,
        start_column: start.column_number,
        end_line: end.line_number,
        end_column: end.column_number,
    }
}

/// Line infos of files that are only referenced by context messages.
#[derive(Default)]
pub struct LineInfoCache {
    files: HashMap<String, Option<LineInfo>>,
}

impl LineInfoCache {
    fn get(&mut self, path: &str) -> Option<&LineInfo> {
        self.files
            .entry(path.to_string())
            .or_insert_with(|| {
                let bytes = std::fs::read(path).ok()?;
                let text = String::from_utf8_lossy(&bytes);
                let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                Some(LineInfo::from_content(text))
            })
            .as_ref()
    }
}

/// Dart `mapEngineErrors` + `newAnalysisError_fromEngine` for the
/// diagnostics of one file.
pub fn analysis_errors(
    collection: &AnalysisContextCollection,
    file: &FileDiagnostics,
    line_infos: &mut LineInfoCache,
) -> Vec<AnalysisError> {
    let options = collection
        .context_for(&file.path)
        .map(|context| collection.options_for(context, &file.path).clone());
    let mut result = Vec::new();
    for diagnostic in &file.diagnostics {
        let processor = options
            .as_ref()
            .and_then(|o| o.processor_for(diagnostic.code.lower_case_name()));
        let severity = match processor {
            // Errors with null severity are filtered out.
            Some(p) => match p.severity {
                None => continue,
                Some(OptionsSeverity::Error) => DiagnosticSeverity::Error,
                Some(OptionsSeverity::Warning) => DiagnosticSeverity::Warning,
                Some(OptionsSeverity::Info) => DiagnosticSeverity::Info,
            },
            None => diagnostic.code.severity(),
        };
        result.push(new_analysis_error(file, diagnostic, severity, line_infos));
    }
    result
}

fn new_analysis_error(
    file: &FileDiagnostics,
    diagnostic: &Diagnostic,
    severity: DiagnosticSeverity,
    line_infos: &mut LineInfoCache,
) -> AnalysisError {
    let code = diagnostic.code;
    let context_messages = diagnostic
        .context_messages
        .iter()
        .filter_map(|m| {
            let offset = m.offset.max(0) as usize;
            let length = m.length.max(0) as usize;
            let line_info = if m.file_path == file.path {
                &file.line_info
            } else {
                line_infos.get(&m.file_path)?
            };
            Some(ContextMessage {
                message: m.message.clone(),
                location: location(&m.file_path, offset, length, line_info),
            })
        })
        .collect();
    AnalysisError {
        severity,
        type_: code.diagnostic_type.name(),
        location: location(&file.path, diagnostic.offset, diagnostic.length, &file.line_info),
        message: diagnostic.message.clone(),
        correction: diagnostic.correction.clone(),
        code: code.lower_case_name().to_string(),
        url: code.url(),
        context_messages,
    }
}
