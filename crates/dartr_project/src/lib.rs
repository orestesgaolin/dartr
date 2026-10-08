//! dartr_project: the project model of dartr.
//!
//! Everything that the Dart analyzer does before it analyzes a file, so that
//! dartr finds the same analysis contexts and settings as `dart analyze`:
//!
//! - context discovery ([context_locator], [context_root], [collection]),
//!   workspaces ([workspace]): pub workspaces, package config workspaces,
//!   basic workspaces, and the discovery of Blaze and GN workspaces;
//! - `package_config.json` and `package:` URI resolution ([package_config]);
//! - `pubspec.yaml` ([pubspec]);
//! - `analysis_options.yaml` with includes and merging ([analysis_options]),
//!   YAML with spans ([yaml]), exclude globs ([glob]);
//! - the Dart SDK: location, version, `dart:` libraries ([sdk]);
//! - experiments ([experiments]) and the names of the lint rules
//!   ([lint_rules]);
//! - the JSON dump that the differential test compares with
//!   `tools/oracle/bin/contexts.dart` ([dump]).
//!
//! Paths are absolute, normalized posix paths (`String`), as in the analyzer.

pub mod analysis_options;
pub mod collection;
pub mod context_locator;
pub mod context_root;
pub mod dump;
pub mod experiments;
pub mod fs;
pub mod glob;
pub mod lint_rules;
pub mod package_config;
pub mod paths;
pub mod pubspec;
pub mod sdk;
pub mod workspace;
pub mod yaml;

pub use analysis_options::{AnalysisOptions, OptionsParseSession};
pub use collection::{
    AnalysisContext, AnalysisContextCollection, CollectionOptions, FileInfo, FileKind,
};
pub use context_root::ContextRoot;
pub use package_config::{LanguageVersion, Package, Packages};
pub use sdk::DartSdk;
pub use workspace::{Workspace, WorkspaceKind};
