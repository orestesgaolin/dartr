// Dart source: pkg/analyzer/lib/src/dart/analysis/context_builder.dart
// (ContextBuilderImpl.createContext: the source factory and the feature set
// provider of the driver of a context),
// pkg/analyzer/lib/src/dart/analysis/feature_set_provider.dart
// (FeatureSetProvider.getLanguageVersion, getFeatureSet)

//! [`context_driver`]: the [`Driver`] of one analysis context of a
//! `dartr_project` collection: the source factory of the context (SDK,
//! workspace and packages), and for each file the language version of its
//! package and the experiments of its analysis options.

use std::sync::Arc;

use dartr_element::Generation;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_project::collection::language_version_in;
use dartr_project::{AnalysisContextCollection, AnalysisOptions, paths};

use crate::driver::Driver;
use crate::file_state::{FileConfig, FileSystemState, SourceFactory};

/// The enabled experiments of [options] as parser flags.
pub fn experiment_flags(options: &AnalysisOptions) -> Vec<ExperimentalFlag> {
    let names = options.enabled_experiments();
    ExperimentalFlag::VALUES
        .iter()
        .copied()
        .filter(|f| names.contains(&f.name()))
        .collect()
}

/// A driver for the context [context_index] of [collection]. The driver
/// owns a copy of the data it needs (packages, the experiments of each
/// options folder), so it does not borrow the collection.
pub fn context_driver(
    collection: &AnalysisContextCollection,
    context_index: usize,
    generation: Arc<Generation>,
) -> Driver {
    let context = &collection.contexts[context_index];
    let source_factory = SourceFactory {
        workspace: context.root.workspace.clone(),
        sdk: context.sdk.as_deref().cloned(),
    };
    let packages = context.packages.clone();
    let sdk_version = context.sdk_language_version();
    let map = collection.options_map(context);
    // `AnalysisOptionsMap.getOptions`: the first (deepest) folder that
    // contains the file, or the default options.
    let folders: Vec<(String, Vec<ExperimentalFlag>)> = map
        .entries()
        .iter()
        .map(|(folder, options)| (folder.clone(), experiment_flags(options)))
        .collect();
    let default = experiment_flags(map.default_options());
    let config_for = Box::new(move |path: &str, uri: &str| {
        let version = language_version_in(&packages, sdk_version, path, uri);
        let experiments = folders
            .iter()
            .find(|(folder, _)| paths::is_within(folder, path))
            .map(|(_, e)| e.clone())
            .unwrap_or_else(|| default.clone());
        FileConfig {
            package_language_version: (version.major, version.minor),
            experiments,
        }
    });
    Driver::new(FileSystemState::new(source_factory, config_for), generation)
}
