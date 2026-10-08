// Dart source: pkg/analyzer/lib/dart/analysis/utilities.dart (parseString)
// Dart source: pkg/analyzer/lib/src/generated/parser.dart (Parser)
// Dart source: pkg/analyzer/lib/src/dart/analysis/experiments.dart (LibraryLanguageVersion)

//! Scanning, parsing and building the AST of a file the way the analyzer
//! does it (`parseString`).

use dartr_ast::{Ast, CompilationUnit, Id, NodeId};
use dartr_diagnostics::Diagnostic;
use dartr_parser::Parser;
use dartr_parser::analyzer::features_for_file;
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::analyzer_scanner::{
    AnalyzerScanResult, CURRENT_LANGUAGE_VERSION, scan_for_analyzer,
};
use dartr_syntax::{LineInfo, ScannerResult};

use crate::ast_builder::AstBuilder;
use crate::error_converter::DiagnosticCollector;

/// Dart `LibraryLanguageVersion`: the language version of the package and
/// the `// @dart = x.y` override of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryLanguageVersion {
    pub package: (u32, u32),
    pub override_: Option<(u32, u32)>,
}

impl LibraryLanguageVersion {
    /// Dart `effective`: the override, or the package version.
    pub fn effective(&self) -> (u32, u32) {
        self.override_.unwrap_or(self.package)
    }
}

/// A parsed compilation unit: the AST and the data that Dart keeps in
/// `CompilationUnitImpl` (`lineInfo`, `languageVersion`, `featureSet`,
/// `invalidNodes`), and the parse diagnostics (Dart `ParseStringResult`).
#[derive(Debug)]
pub struct ParsedUnit {
    /// The token arena (after parsing) and the nodes.
    pub ast: Ast,
    /// The `CompilationUnit` node.
    pub unit: Id<CompilationUnit>,
    /// Dart `ParseStringResult.errors`: scanner and parser diagnostics in
    /// report order, without duplicates.
    pub diagnostics: Vec<Diagnostic>,
    /// Dart `CompilationUnit.lineInfo`.
    pub line_info: LineInfo,
    /// Dart `CompilationUnit.languageVersion`.
    pub language_version: LibraryLanguageVersion,
    /// Dart `CompilationUnit.featureSet`: the features of the file (the
    /// package features restricted to the language version override).
    pub feature_set: ExperimentalFeatures,
    /// Dart `CompilationUnitImpl.invalidNodes`: constructors in mixins and
    /// extensions, which are not in the AST.
    pub invalid_nodes: Vec<NodeId>,
    /// The expressions with `isDotShorthand = true`.
    pub dot_shorthands: Vec<NodeId>,
}

/// Dart `parseString(content: content, path: path, throwIfDiagnostics:
/// false)` with the feature set of the latest language version. [content]
/// must not start with a byte order mark.
pub fn parse_string(content: &str, path: &str) -> ParsedUnit {
    parse_impl(content, path, None)
}

/// Dart `FileState.parseCode`: parses [content] like the analysis driver,
/// with the language version of the package of the file
/// ([package_version], Dart `packageLanguageVersion`) and the experiments
/// that are enabled for the file ([experiments], Dart `featureSet`). The
/// features of the file are the features of `package_version` (or of the
/// `// @dart = x.y` comment) plus the experiments (Dart
/// `featureSet.restrictToVersion(version)`).
pub fn parse_file(
    content: &str,
    path: &str,
    package_version: (u32, u32),
    experiments: &[ExperimentalFlag],
) -> ParsedUnit {
    parse_impl(content, path, Some((package_version, experiments)))
}

fn parse_impl(
    content: &str,
    path: &str,
    package: Option<((u32, u32), &[ExperimentalFlag])>,
) -> ParsedUnit {
    let AnalyzerScanResult {
        scan,
        diagnostics: scan_diagnostics,
        override_version,
        scan_diagnostic_count,
        feature_version,
        ..
    } = scan_for_analyzer(content);
    let ScannerResult {
        tokens,
        first,
        mut line_starts,
        ..
    } = scan;
    // `Scanner.tokenize`: fasta pretends there is an additional line at
    // EOF, so the last line start is skipped.
    line_starts.pop();
    let line_info = LineInfo::new(line_starts);
    let override_ = override_version.map(|(major, minor)| (major as u32, minor as u32));
    let (language_version, feature_set) = match package {
        None => (
            LibraryLanguageVersion {
                package: (
                    CURRENT_LANGUAGE_VERSION.0 as u32,
                    CURRENT_LANGUAGE_VERSION.1 as u32,
                ),
                override_,
            },
            features_for_file(feature_version),
        ),
        Some((package_version, experiments)) => {
            let (major, minor) = match feature_version {
                Some((major, minor)) => (major as u32, minor as u32),
                None => package_version,
            };
            (
                LibraryLanguageVersion {
                    package: package_version,
                    override_,
                },
                ExperimentalFeatures::for_language_version(major, minor, experiments),
            )
        }
    };

    // Dart `Parser(diagnosticReporter, featureSet:, languageVersion:,
    // lineInfo:)` of `generated/parser.dart`.
    let mut builder = AstBuilder::new(file_uri(path), true, feature_set, language_version, None);
    builder.allow_native_clause = true;
    let mut parser = Parser::new(builder, tokens, true, feature_set);
    parser.parse_unit(first);
    let (tokens, mut builder) = parser.into_parts();
    builder.ast.tokens = tokens;
    let unit = builder.pop_node::<CompilationUnit>();

    let mut collector = DiagnosticCollector::new();
    for d in scan_diagnostics.into_iter().take(scan_diagnostic_count) {
        collector.on_diagnostic(d);
    }
    for d in std::mem::take(&mut builder.diagnostic_reporter.diagnostic_reporter.diagnostics) {
        collector.on_diagnostic(d);
    }

    ParsedUnit {
        ast: builder.ast,
        unit,
        diagnostics: collector.diagnostics,
        line_info,
        language_version,
        feature_set,
        invalid_nodes: builder.invalid_nodes,
        dot_shorthands: builder.dot_shorthands,
    }
}

/// Dart `Uri.file(path).toString()` for an absolute path (the URI of the
/// `StringSource` of `parseString`).
fn file_uri(path: &str) -> String {
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        path.to_string()
    }
}
