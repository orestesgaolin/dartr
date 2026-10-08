//! Port of `pkg/analysis_server/lib/src/context_manager.dart` non-Dart file
//! dispatch and `pkg/analyzer_plugin/lib/utilities/analyzer_converter.dart`
//! diagnostic severity processing. CLI JSON selection follows
//! `pkg/dartdev/lib/src/commands/analyze.dart`.

use crate::{
    AnalysisContext, AnalysisContextCollection, AnalysisOptions, FileKind, OptionsParseSession,
    paths,
};
use dartr_diagnostics::{Diagnostic, DiagnosticSeverity};
use serde_json::{Value, json};

/// Effective options for a file, using the deepest applicable options folder.
pub fn options_for_file(context: &AnalysisContext, path: &str) -> AnalysisOptions {
    let file = context
        .root
        .options_file_map
        .iter()
        .flat_map(|(file, folders)| folders.iter().map(move |folder| (file, folder)))
        .filter(|(_, folder)| paths::is_or_within(folder, path))
        .max_by_key(|(_, folder)| folder.len())
        .map(|(file, _)| file)
        .or(context.root.options_file.as_ref());
    file.map(|file| {
        OptionsParseSession::new()
            .parse(&context.root.workspace, file)
            .options
    })
    .unwrap_or_default()
}

/// Validate an analyzed non-Dart file and apply configured error severities.
/// Manifests are analyzed in every context; Chrome OS checks are opt-in.
/// Excluded and hidden files follow `ContextRoot.isAnalyzed`.
pub fn diagnostics_for_file(context: &AnalysisContext, path: &str) -> Vec<Diagnostic> {
    if !context.root.is_analyzed(path) {
        return Vec::new();
    }
    let diagnostics = match FileKind::of(path) {
        FileKind::AnalysisOptions => {
            crate::options_validator::validate_analysis_options(context, path)
        }
        FileKind::Pubspec => crate::pubspec_validator::validate_pubspec(context, path),
        FileKind::AndroidManifest => crate::manifest_validator::validate_manifest(context, path),
        _ => return Vec::new(),
    };
    let options = if FileKind::of(path) == FileKind::AnalysisOptions {
        // ContextManager uses the options returned by parsing this file,
        // even when the driver uses a shared command-line options override.
        OptionsParseSession::new()
            .parse(&context.root.workspace, path)
            .options
    } else {
        options_for_file(context, path)
    };
    apply_error_processors(&options, diagnostics)
}

pub fn apply_error_processors(
    options: &AnalysisOptions,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|mut diagnostic| {
            if let Some(processor) = options.processor_for(&diagnostic.code.name.to_lowercase()) {
                diagnostic.severity = match processor.severity? {
                    crate::analysis_options::DiagnosticSeverity::Info => DiagnosticSeverity::Info,
                    crate::analysis_options::DiagnosticSeverity::Warning => {
                        DiagnosticSeverity::Warning
                    }
                    crate::analysis_options::DiagnosticSeverity::Error => DiagnosticSeverity::Error,
                };
            }
            Some(diagnostic)
        })
        .collect()
}

/// Stable comparison representation. Source paths belong to the file result,
/// because `Diagnostic` itself stores source ranges only.
pub fn diagnostic_json(path: &str, diagnostic: &Diagnostic) -> Value {
    json!({"file":path, "code":diagnostic.code.name.to_lowercase(),
        "severity":diagnostic.severity.name(), "offset":diagnostic.offset,
        "length":diagnostic.length, "message":diagnostic.message,
        "correction":diagnostic.correction})
}

/// Non-Dart files selected by the server's context-root traversal.
pub fn analyzed_files(collection: &AnalysisContextCollection) -> Vec<String> {
    let mut files: Vec<_> = collection
        .contexts
        .iter()
        .flat_map(|context| context.root.analyzed_files())
        .filter(|path| {
            matches!(
                FileKind::of(path),
                FileKind::AnalysisOptions | FileKind::Pubspec | FileKind::AndroidManifest
            )
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

pub fn collection_diagnostics_json(collection: &AnalysisContextCollection) -> Value {
    collection_json(collection, false)
}

/// Match Dart 3.13.3 `analyze --format=json`, including its omission of
/// error-severity diagnostics from options and pubspec files.
pub fn collection_cli_diagnostics_json(collection: &AnalysisContextCollection) -> Value {
    collection_json(collection, true)
}

fn collection_json(collection: &AnalysisContextCollection, for_cli: bool) -> Value {
    let mut diagnostics = Vec::new();
    for path in analyzed_files(collection) {
        if let Some(context) = collection.context_for(&path) {
            diagnostics.extend(
                diagnostics_for_file(context, &path)
                    .iter()
                    // Dart 3.13.3 analyze.dart partitions ERROR diagnostics in
                    // options/pubspec into priorityErrors, then omits that list
                    // from JSON and machine output. Server APIs retain them.
                    .filter(|d| {
                        !(for_cli
                            && d.severity == DiagnosticSeverity::Error
                            && matches!(
                                FileKind::of(&path),
                                FileKind::AnalysisOptions | FileKind::Pubspec
                            ))
                    })
                    .map(|d| diagnostic_json(&path, d)),
            );
        }
    }
    sort_diagnostics(&mut diagnostics);
    Value::Array(diagnostics)
}

pub fn sort_diagnostics(diagnostics: &mut [Value]) {
    diagnostics.sort_by_key(|d| {
        (
            d["file"].as_str().unwrap_or_default().to_owned(),
            d["offset"].as_u64().unwrap_or_default(),
            d["code"].as_str().unwrap_or_default().to_owned(),
            d.to_string(),
        )
    });
}
