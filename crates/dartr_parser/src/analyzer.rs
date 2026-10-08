// Dart source: pkg/analyzer/lib/dart/analysis/utilities.dart (parseString)
// Dart source: pkg/analyzer/lib/src/generated/parser.dart (Parser)

//! Scanning and parsing a file the way the analyzer does.
//!
//! `parseString` (and file analysis) scans with the analyzer `Scanner`
//! configured for the feature set of the package
//! (`FeatureSet.latestLanguageVersion()` here), then creates the fasta
//! parser with `ExperimentalFeaturesStatus(scanner.featureSet)`: the package
//! features restricted to the language version of a `// @dart = x.y`
//! comment, if the file has one. The parser starts at the first token of the
//! scanner, before the error tokens (it skips them and reports them through
//! `Listener::handle_error_token` at the end).
//!
//! The analyzer `Parser` wrapper also sets `astBuilder.allowNativeClause =
//! true` and `astBuilder.parser = fastaParser`, and creates the `AstBuilder`
//! with `isFullAst = true`; these are options of the listener (the AST
//! builder), not of the parser.
//!
//! # Notes for the AST builder (`pkg/analyzer/lib/src/fasta/ast_builder.dart`)
//!
//! Where the builder reaches back into the parser or changes its input:
//!
//! - Token rewriting from inside events, with `parser.rewriter`: Rust
//!   listeners get `&mut Tokens` in every event and use
//!   `TokenStreamRewriter::new(tokens)`. The undoable rewriter is only
//!   active while events are dropped (look-ahead), so the builder always
//!   sees the plain rewriter. Uses in Dart: `endNamedMixinApplication`
//!   (`replaceNextTokensWithSyntheticToken`), `endRecordLiteral` and
//!   `endRecordType` (`insertSyntheticIdentifier`), `endVariablesDeclaration`
//!   (`insertSyntheticIdentifier`, `insertToken`, `insertSyntheticToken` for
//!   `await for` recovery), `handleForInLoopParts`
//!   (`insertSyntheticIdentifier`), `_ensureSetterFormalParameter`
//!   (`insertSyntheticIdentifier`), `_syntheticArgumentList` and
//!   `_syntheticFormalParameterList` (`insertParens`). These changes can
//!   change the tokens the parser reads next.
//! - `parser.findDartDoc(token)` in `_findComment`: the free function
//!   `parser_impl::find_dart_doc(tokens, token)`.
//! - Doc comments (`DocCommentBuilder`): comment references are scanned
//!   with a separate `scanString` (a separate token arena) and rewritten
//!   with `parser.rewriter.replaceTokenFollowing` /
//!   `insertSyntheticIdentifier` and `parser.syntheticPreviousToken`
//!   (`TokenStreamRewriter::new(&mut other_tokens)`,
//!   `parser_impl::synthetic_previous_token`). `@docImport` creates a new
//!   `DocImportStringScanner`, a new `AstBuilder` and a new `Parser`
//!   (`Parser::new` over the new arena) and calls `parseUnit`.
//! - Builder options that the parser does not see: `isFullAst` (true),
//!   `allowNativeClause` (true for `parseString`), `parseFunctionBodies`,
//!   the feature flags of the builder (from the same feature set).
//! - `handleErrorToken`: the builder translates scanner error tokens with
//!   `translateErrorToken` (in Rust `dartr_syntax::analyzer_scanner`).
//!   The parser reports them at the end of `parse_unit`, after all other
//!   events.
//! - `addProblem` turns `NonPartOfDirectiveInPart` into
//!   `DirectiveAfterDeclaration` when no directive was seen yet: builder
//!   state, not parser state.
//! - Listener defaults: `handle_experiment_not_enabled`,
//!   `handle_error_token`, `handle_unescape_error` and
//!   `handle_invalid_statement` forward to `handle_recoverable_error` like
//!   Dart; the builder overrides some of them.

use dartr_syntax::analyzer_scanner::{AnalyzerScanResult, scan_for_analyzer};
use dartr_syntax::{Diagnostic, ScannerResult, TokenId, Tokens};

use crate::experimental_features::ExperimentalFeatures;
use crate::listener::Listener;
use crate::parser_impl::Parser;

/// The result of [`parse_for_analyzer`].
pub struct AnalyzerParseResult<L> {
    /// The token arena after parsing (the parser may have inserted tokens).
    pub tokens: Tokens,
    pub listener: L,
    /// The first token that the scanner returned (error tokens first).
    pub scanner_first: TokenId,
    /// The first token after the error tokens (where `beginCompilationUnit`
    /// starts).
    pub first: TokenId,
    /// The token before [`Self::first`] at the start (the scanner's head
    /// token or the last error token). Its `next` is the start of the
    /// token stream after parsing (the parser can insert tokens before
    /// `first`).
    pub before_first: TokenId,
    /// The EOF token returned by `parse_unit`.
    pub eof: TokenId,
    /// Scanner diagnostics (language version comments, error tokens), as
    /// [`scan_for_analyzer`] reports them.
    pub scanner_diagnostics: Vec<Diagnostic>,
    /// Dart `Scanner.overrideVersion`.
    pub override_version: Option<(i64, i64)>,
    /// The features the parser was created with.
    pub features: ExperimentalFeatures,
    /// Dart `lineStarts` of the scanner (with the additional line start
    /// after the end of the file).
    pub line_starts: Vec<u32>,
}

/// The parser features for a file: [package_features] restricted to the
/// language version of the `// @dart = x.y` comment, if any (Dart
/// `Scanner.configureFeatures` + `FeatureSet.restrictToVersion`).
pub fn features_for_file(override_version: Option<(i64, i64)>) -> ExperimentalFeatures {
    match override_version {
        Some((major, minor)) => {
            ExperimentalFeatures::for_language_version(major as u32, minor as u32, &[])
        }
        None => ExperimentalFeatures::latest(),
    }
}

/// Scans and parses [source] (without byte order mark) like the analyzer
/// `parseString` with the latest language version, sending the events to
/// [listener].
pub fn parse_for_analyzer<L: Listener>(source: &str, listener: L) -> AnalyzerParseResult<L> {
    let AnalyzerScanResult {
        scan,
        first,
        diagnostics,
        override_version,
    } = scan_for_analyzer(source);
    let ScannerResult {
        tokens,
        first: scanner_first,
        line_starts,
        ..
    } = scan;
    let before_first = tokens.previous(first);
    let features = features_for_file(override_version);
    let mut parser = Parser::new(listener, tokens, true, features);
    let eof = parser.parse_unit(scanner_first);
    let (tokens, listener) = parser.into_parts();
    AnalyzerParseResult {
        tokens,
        listener,
        scanner_first,
        first,
        before_first,
        eof,
        scanner_diagnostics: diagnostics,
        override_version,
        features,
        line_starts,
    }
}
