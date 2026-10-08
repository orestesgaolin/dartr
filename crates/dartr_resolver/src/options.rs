// Dart source: pkg/analyzer/lib/src/dart/analysis/analysis_options.dart
// (the fields of AnalysisOptionsImpl that the resolver reads)

//! [`AnalysisOptions`]: the analysis options that resolution reads.

/// The resolver part of `AnalysisOptionsImpl`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalysisOptions {
    /// `strict-casts`.
    pub strict_casts: bool,
    /// `strict-inference`.
    pub strict_inference: bool,
    /// `strict-raw-types`.
    pub strict_raw_types: bool,
}
