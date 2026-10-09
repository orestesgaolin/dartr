//! The analysis context collection: context roots plus, for each context, the
//! SDK, the packages, and the analysis options of each folder.
//!
//! Ports `pkg/analyzer/lib/src/dart/analysis/analysis_context_collection.dart`,
//! the project-model parts of `context_builder.dart` (`ContextBuilderImpl`),
//! `analysis_options_map.dart` (`AnalysisOptionsMap`), and the language
//! version and URI of a file (`FileSystemState._newFile`,
//! `FeatureSetProvider.getLanguageVersion`).

use crate::analysis_options::{AnalysisOptions, OptionsParseSession};
use crate::context_locator::locate_context_roots;
use crate::context_root::ContextRoot;
use crate::experiments::CURRENT_LANGUAGE_VERSION;
use crate::package_config::{self, LanguageVersion, Packages};
use crate::sdk::DartSdk;
use crate::{fs, paths};
use std::rc::Rc;

/// Options of a collection.
#[derive(Clone, Debug, Default)]
pub struct CollectionOptions {
    /// An options file to use instead of the `analysis_options.yaml` files
    /// (`dart analyze --options`).
    pub options_file: Option<String>,
    /// A package config file to use instead of the
    /// `.dart_tool/package_config.json` files (`--packages`).
    pub package_config_file: Option<String>,
    /// The SDK folder. If `None`, the SDK of `dart` on `PATH`.
    pub sdk_path: Option<String>,
    /// `--enable-experiment` flags. Only used with [options_file].
    pub enabled_experiments: Vec<String>,
}

/// Maps folders to their analysis options (`AnalysisOptionsMap`).
#[derive(Clone, Debug)]
pub struct AnalysisOptionsMap {
    /// Entries sorted by folder path, in reverse order (deeper folders
    /// first).
    entries: Vec<(String, Rc<AnalysisOptions>)>,
    default_options: Rc<AnalysisOptions>,
}

impl AnalysisOptionsMap {
    fn new() -> Self {
        AnalysisOptionsMap {
            entries: Vec::new(),
            default_options: Rc::new(AnalysisOptions::default()),
        }
    }

    /// One set of options for every file (`AnalysisOptionsMap.forSharedOptions`).
    fn shared(options: Rc<AnalysisOptions>) -> Self {
        let mut entries = Vec::new();
        if let Some(file) = &options.file {
            entries.push((paths::dirname(file).to_string(), options.clone()));
        }
        AnalysisOptionsMap {
            entries,
            default_options: options,
        }
    }

    fn insert(&mut self, folder: &str, options: Rc<AnalysisOptions>) {
        match self.entries.binary_search_by(|(f, _)| folder.cmp(f)) {
            Ok(index) => self.entries[index].1 = options,
            Err(index) => self.entries.insert(index, (folder.to_string(), options)),
        }
    }

    /// The folders, deepest first.
    pub fn folders(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(f, _)| f.as_str())
    }

    /// The entries (folder and options), deepest folder first.
    pub fn entries(&self) -> &[(String, Rc<AnalysisOptions>)] {
        &self.entries
    }

    /// The options for the file at [path]: the options of the first folder
    /// that contains it, or the default options.
    pub fn options_for(&self, path: &str) -> &Rc<AnalysisOptions> {
        for (folder, options) in &self.entries {
            if paths::is_within(folder, path) {
                return options;
            }
        }
        &self.default_options
    }

    /// The default options.
    pub fn default_options(&self) -> &Rc<AnalysisOptions> {
        &self.default_options
    }
}

/// An analysis context.
#[derive(Debug)]
pub struct AnalysisContext {
    pub root: ContextRoot,
    /// The packages of [ContextRoot::packages_file] (`_createPackageMap`).
    pub packages: Packages,
    /// The SDK of the context: the folder SDK, or the embedder SDK of
    /// `package:sky_engine/_embedder.yaml`.
    pub sdk: Option<Rc<DartSdk>>,
    /// `None` if the context uses the options map shared by the collection.
    own_options: Option<AnalysisOptionsMap>,
}

/// Facts about one analyzed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub path: String,
    /// The URI of the file (`dart:`, `package:` or `file:`).
    pub uri: String,
    /// The default language version of the file, before `// @dart=` comments.
    pub language_version: LanguageVersion,
}

/// The kind of an analyzed file, for the files the analysis server reports
/// diagnostics for (`ContextManagerImpl._createAnalysisContexts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Dart,
    AnalysisOptions,
    Pubspec,
    AndroidManifest,
    Other,
}

impl FileKind {
    pub fn of(path: &str) -> FileKind {
        match paths::basename(path) {
            "analysis_options.yaml" => FileKind::AnalysisOptions,
            "pubspec.yaml" => FileKind::Pubspec,
            "AndroidManifest.xml" => FileKind::AndroidManifest,
            _ if paths::extension(path) == ".dart" => FileKind::Dart,
            _ => FileKind::Other,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FileKind::Dart => "dart",
            FileKind::AnalysisOptions => "analysis_options",
            FileKind::Pubspec => "pubspec",
            FileKind::AndroidManifest => "android_manifest",
            FileKind::Other => "other",
        }
    }
}

/// All analysis contexts for a set of included paths
/// (`AnalysisContextCollectionImpl`).
#[derive(Debug)]
pub struct AnalysisContextCollection {
    pub sdk: Option<Rc<DartSdk>>,
    pub contexts: Vec<AnalysisContext>,
    /// The options map that the contexts share (`ContextBuilderImpl._optionsMap`).
    shared_options: AnalysisOptionsMap,
    session: OptionsParseSession,
}

impl AnalysisContextCollection {
    /// Builds the contexts for [included_paths]. Relative paths are made
    /// absolute against the current directory; all paths are normalized.
    pub fn new(
        included_paths: &[String],
        options: &CollectionOptions,
    ) -> AnalysisContextCollection {
        let included: Vec<String> = included_paths
            .iter()
            .map(|p| paths::absolute_normalized(p))
            .collect();
        let sdk_path = options.sdk_path.clone().or_else(crate::sdk::find_sdk_path);
        let sdk = sdk_path.map(|path| Rc::new(DartSdk::new(&path)));
        let session = OptionsParseSession::new();
        let roots = locate_context_roots(
            &included,
            options.options_file.as_deref(),
            options.package_config_file.as_deref(),
            &session,
        );

        let mut shared_options = AnalysisOptionsMap::new();
        let mut contexts = Vec::new();
        for root in roots {
            let workspace = &root.workspace;
            let context_sdk = create_sdk(workspace, sdk.as_ref());
            // A new parse session per context, as `createContext` does.
            let context_session = OptionsParseSession::new();
            let own_options = match (&options.options_file, &root.options_file) {
                (Some(_), Some(file)) => {
                    let mut parsed = context_session.parse(workspace, file).options;
                    // `_getAnalysisOptions` replaces the experiments with the
                    // command line flags.
                    parsed.enable_experiment_flags = Some(options.enabled_experiments.clone());
                    Some(AnalysisOptionsMap::shared(Rc::new(parsed)))
                }
                _ => {
                    for (file, folders) in &root.options_file_map {
                        let parsed = Rc::new(context_session.parse(workspace, file).options);
                        for folder in folders {
                            shared_options.insert(folder, parsed.clone());
                        }
                    }
                    if shared_options.entries.is_empty() {
                        Some(AnalysisOptionsMap::shared(Rc::new(
                            AnalysisOptions::default(),
                        )))
                    } else {
                        None
                    }
                }
            };
            let packages = match &root.packages_file {
                Some(file) => package_config::parse_package_config_file(file),
                None => Packages::empty(),
            };
            contexts.push(AnalysisContext {
                root,
                packages,
                sdk: context_sdk,
                own_options,
            });
        }
        AnalysisContextCollection {
            sdk,
            contexts,
            shared_options,
            session,
        }
    }

    /// The options parse session (file contents are cached).
    pub fn session(&self) -> &OptionsParseSession {
        &self.session
    }

    /// The options map of [context].
    pub fn options_map<'a>(&'a self, context: &'a AnalysisContext) -> &'a AnalysisOptionsMap {
        context.own_options.as_ref().unwrap_or(&self.shared_options)
    }

    /// The analysis options for the file at [path] in [context].
    pub fn options_for<'a>(
        &'a self,
        context: &'a AnalysisContext,
        path: &str,
    ) -> &'a Rc<AnalysisOptions> {
        self.options_map(context).options_for(path)
    }

    /// The context that analyzes [path] (`contextFor`).
    pub fn context_for(&self, path: &str) -> Option<&AnalysisContext> {
        self.contexts.iter().find(|c| c.root.is_analyzed(path))
    }
}

impl AnalysisContext {
    /// The URI and the default language version of the file at [path].
    pub fn file_info(&self, path: &str) -> FileInfo {
        let sdk = self.sdk.as_deref();
        let uri = self.root.workspace.path_to_uri(path, sdk);
        let language_version = self.language_version(path, &uri);
        FileInfo {
            path: path.to_string(),
            uri,
            language_version,
        }
    }

    /// `FeatureSetProvider.getLanguageVersion`: the language version of the
    /// package of the file, or of the SDK, or the current language version
    /// for files outside of packages.
    pub fn language_version(&self, path: &str, uri: &str) -> LanguageVersion {
        language_version_in(&self.packages, self.sdk_language_version(), path, uri)
    }

    /// The language version of the SDK of the context, or the current
    /// language version.
    pub fn sdk_language_version(&self) -> LanguageVersion {
        self.sdk
            .as_deref()
            .and_then(DartSdk::language_version)
            .unwrap_or(CURRENT_LANGUAGE_VERSION)
    }

    /// The `fix_data.yaml` files of the context that the analysis server
    /// analyzes: `lib/fix_data.yaml` and `lib/fix_data/**.yaml` of the root.
    pub fn fix_data_files(&self) -> Vec<String> {
        let lib = paths::join(&self.root.root, "lib");
        let mut result = Vec::new();
        let file = paths::join(&lib, "fix_data.yaml");
        if fs::file_exists(&file) {
            result.push(file);
        }
        fn walk(folder: &str, result: &mut Vec<String>) {
            for child in fs::children(folder).unwrap_or_default() {
                match child.kind {
                    fs::ResourceKind::File => {
                        if child.path.ends_with(".yaml") {
                            result.push(child.path);
                        }
                    }
                    fs::ResourceKind::Folder => walk(&child.path, result),
                }
            }
        }
        let folder = paths::join(&lib, "fix_data");
        if fs::folder_exists(&folder) {
            walk(&folder, &mut result);
        }
        result
    }
}

/// `ContextBuilderImpl._createSdk`: the embedder SDK of
/// `package:sky_engine/_embedder.yaml`, if any, else the folder SDK.
fn create_sdk(
    workspace: &crate::workspace::Workspace,
    sdk: Option<&Rc<DartSdk>>,
) -> Option<Rc<DartSdk>> {
    if let Some(embedder_path) = workspace.resolve_package_uri("package:sky_engine/_embedder.yaml")
    {
        let lib_folder = paths::dirname(&embedder_path);
        let language_version = sdk.and_then(|s| s.language_version());
        if let Some(embedder) = DartSdk::embedder(lib_folder, language_version) {
            return Some(Rc::new(embedder));
        }
    }
    sdk.cloned()
}

/// [AnalysisContext::language_version] with the [packages] and the
/// [sdk_version] of a context (for code that cannot hold the context).
pub fn language_version_in(
    packages: &Packages,
    sdk_version: LanguageVersion,
    path: &str,
    uri: &str,
) -> LanguageVersion {
    if uri.starts_with("dart:") {
        return sdk_version;
    }
    let package = if uri.starts_with("package:") {
        package_config::split_package_uri(uri).and_then(|(name, _)| packages.get(&name))
    } else if uri.starts_with("file:") {
        paths::file_uri_to_path(uri).and_then(|p| packages.package_for_path(&p))
    } else {
        None
    };
    let package = package.or_else(|| packages.package_for_path(path));
    match package {
        Some(package) => package.language_version.unwrap_or(sdk_version),
        None => CURRENT_LANGUAGE_VERSION,
    }
}
