// Dart source: dart_style lib/src/analysis_options/analysis_options_file.dart
// Dart source: dart_style lib/src/analysis_options/merge_options.dart

//! Reads the "formatter" options of "analysis_options.yaml" files, with
//! `include:` support.

pub mod analysis_options_file;
pub mod merge_options;

pub use analysis_options_file::{
    AnalysisOptions, AnalysisOptionsError, PackageResolutionException, Value,
    find_analysis_options, read_analysis_options,
};
