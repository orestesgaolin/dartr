//! The source of the diagnostics of `dartr analyze`.
//!
//! [DiagnosticsProvider] returns, for each analyzed file, the diagnostics
//! that the analyzer reports for it (Dart `ErrorsResult.diagnostics` /
//! `ResolvedUnitResult.diagnostics`): after `// ignore:` filtering, before
//! the `errors:` severity processing of `analysis_options.yaml` (that is
//! server side, see [crate::server]).
//!
//! [ParseOnlyProvider] gives the parse diagnostics only. The analysis driver
//! (`dartr_driver`) will implement the trait with the full diagnostics.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dartr_ast_builder::parse::parse_file;
use dartr_diagnostics::{Diagnostic, all_codes};
use dartr_parser::ExperimentalFlag;
use dartr_project::analysis_options::DiagnosticSeverity as OptionsSeverity;
use dartr_project::{AnalysisContextCollection, AnalysisOptions};
use dartr_syntax::LineInfo;
use rayon::prelude::*;

use crate::ignore_info::IgnoreInfo;
use crate::ignore_validator::{filter_ignored_diagnostics, is_generated, validate_ignores};

/// A file to analyze: an analyzed `.dart` file of a context of the
/// collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzedFile {
    /// Absolute, normalized path.
    pub path: String,
    /// Index into [AnalysisContextCollection::contexts].
    pub context: usize,
}

/// The diagnostics of one file.
#[derive(Debug)]
pub struct FileDiagnostics {
    pub path: String,
    /// The line starts of the file, for line and column numbers.
    pub line_info: LineInfo,
    /// The reported diagnostics, without the ignored ones. The severity is
    /// the default severity of the code.
    pub diagnostics: Vec<Diagnostic>,
}

/// Produces the diagnostics of analyzed files.
pub trait DiagnosticsProvider {
    /// The diagnostics of one file (Dart `AnalysisDriver.getErrors`).
    /// Returns `None` if the file cannot be read.
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics>;

    /// The diagnostics of all [files], in any order. Implementations should
    /// analyze in parallel; the default analyzes one file after the other.
    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        files
            .iter()
            .filter_map(|f| self.diagnostics_for_file(collection, f))
            .collect()
    }
}

/// Parse diagnostics (scanner and parser) plus the ignore-comment
/// diagnostics (`duplicate_ignore`, `unignorable_ignore`), with `// ignore:`
/// filtering. Each file is parsed like the analysis driver parses it: with
/// the language version of its package and the experiments of its analysis
/// options.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOnlyProvider;

/// The per-file input of the parse step. Unlike the collection (which uses
/// `Rc`), it can be sent to other threads.
struct ParseTask {
    path: String,
    package_version: (u32, u32),
    settings: Arc<FileSettings>,
}

/// Settings derived from one [AnalysisOptions].
struct FileSettings {
    experiments: Vec<ExperimentalFlag>,
    /// Dart `unignorableDiagnosticCodeNames`.
    unignorable_names: HashSet<String>,
}

impl FileSettings {
    fn new(options: &AnalysisOptions) -> FileSettings {
        let experiments = options
            .enabled_experiments()
            .iter()
            .filter_map(|name| {
                ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .find(|f| f.name() == *name)
            })
            .collect();
        let codes: Vec<(&str, OptionsSeverity)> = all_codes()
            .iter()
            .filter_map(|c| {
                let severity = match c.severity() {
                    dartr_diagnostics::DiagnosticSeverity::Info => OptionsSeverity::Info,
                    dartr_diagnostics::DiagnosticSeverity::Warning => OptionsSeverity::Warning,
                    dartr_diagnostics::DiagnosticSeverity::Error => OptionsSeverity::Error,
                    dartr_diagnostics::DiagnosticSeverity::None => return None,
                };
                Some((c.lower_case_name(), severity))
            })
            .collect();
        let unignorable_names = options
            .unignorable_code_names(&codes)
            .into_iter()
            .map(|n| n.to_lowercase())
            .collect();
        FileSettings {
            experiments,
            unignorable_names,
        }
    }
}

impl ParseOnlyProvider {
    fn task(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
        cache: &mut HashMap<*const AnalysisOptions, Arc<FileSettings>>,
    ) -> ParseTask {
        let context = &collection.contexts[file.context];
        let info = context.file_info(&file.path);
        let options = collection.options_for(context, &file.path);
        let key = std::rc::Rc::as_ptr(options);
        let settings = cache
            .entry(key)
            .or_insert_with(|| Arc::new(FileSettings::new(options)))
            .clone();
        ParseTask {
            path: file.path.clone(),
            package_version: (info.language_version.major, info.language_version.minor),
            settings,
        }
    }
}

fn run_task(task: &ParseTask) -> Option<FileDiagnostics> {
    let content = std::fs::read(&task.path).ok()?;
    // The analyzer decodes with `allowMalformed: true`.
    let content = String::from_utf8_lossy(&content);
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
    let parsed = parse_file(
        content,
        &task.path,
        task.package_version,
        &task.settings.experiments,
    );
    let tokens = &parsed.ast.tokens;
    let first = parsed.ast.begin_token(dartr_ast::NodeId::from(parsed.unit));
    let ignore_info = IgnoreInfo::for_dart(tokens, first, &parsed.line_info, content);
    let unignorable = &task.settings.unignorable_names;
    let mut diagnostics = parsed.diagnostics;
    if !is_generated(&task.path) {
        diagnostics.extend(validate_ignores(&ignore_info, unignorable));
    }
    let diagnostics =
        filter_ignored_diagnostics(diagnostics, &ignore_info, &parsed.line_info, unignorable);
    Some(FileDiagnostics {
        path: task.path.clone(),
        line_info: parsed.line_info,
        diagnostics,
    })
}

/// Runs [f] on a thread with a large stack: the parser recurses deeply on
/// deeply nested code.
fn with_large_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(scope, f)
            .expect("spawn thread")
            .join()
            .expect("thread panicked")
    })
}

impl DiagnosticsProvider for ParseOnlyProvider {
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics> {
        let task = self.task(collection, file, &mut HashMap::new());
        with_large_stack(|| run_task(&task))
    }

    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        let mut cache = HashMap::new();
        let tasks: Vec<ParseTask> = files
            .iter()
            .map(|f| self.task(collection, f, &mut cache))
            .collect();
        let pool = rayon::ThreadPoolBuilder::new()
            .stack_size(256 << 20)
            .build()
            .expect("thread pool");
        pool.install(|| tasks.par_iter().filter_map(run_task).collect())
    }
}
