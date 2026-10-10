// Dart source: pkg/analyzer/lib/src/dart/analysis/driver.dart
// (AnalysisDriver.getErrors, _computeErrors, _computeAnalysisResult: one
// driver per analysis context, the library of a file, the diagnostics of the
// units of the library), pkg/analysis_server/lib/src/context_manager.dart
// (one driver per context root)

//! [DriverProvider]: the full diagnostics of `dartr analyze`, from the
//! analysis driver (`dartr_driver`): parse, resolution, constant and (when
//! `dartr_resolver` reports them) verifier diagnostics, then the same steps
//! as [crate::provider::ParseOnlyProvider]: the AST-only lint rules, the
//! ignore-comment diagnostics and the `// ignore:` filtering.
//!
//! [DriverSession] keeps one [Driver] per analysis context of the
//! collection. `dartr analyze` uses a new session for each run; the language
//! server keeps the session and reports file changes with
//! [DriverSession::change_file] (design `docs/design/semantics.md` §4.2).
//!
//! Per context (design §2.5): read and parse the requested files and every
//! file they reference in parallel, find the library of each file, link the
//! library cycles with the DAG scheduler (a cycle starts when its
//! dependencies are linked), then analyze all libraries in parallel.
//!
//! Fallbacks to the parse-only pipeline (the file still gets its parse
//! diagnostics, lints and ignore handling): a part file without a library
//! (Dart analyzes it as a library of its own), a context whose linking
//! panics, a library whose analysis panics. `DARTR_DEBUG_DRIVER=1` writes
//! these cases and the resolver panics of units to stderr.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use dartr_diagnostics::Diagnostic;
use dartr_driver::driver::Driver;
use dartr_driver::file_state::FileId;
use dartr_element::Generation;
use dartr_lints::{RuleContextUnit, lint_library_unfiltered};
use dartr_project::AnalysisContextCollection;
use dartr_project::paths;
use dartr_resolver::library_analyzer::{ResolvedLibrary, UnitInput};
use dartr_resolver::options::AnalysisOptions as ResolverOptions;
use indexmap::IndexMap;

use crate::provider::{
    AnalyzedFile, DiagnosticsProvider, FileDiagnostics, FileSettings, diagnostics_with_reader,
    finish_file, pool,
};

/// The diagnostics of the analysis driver. See the module documentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriverProvider;

impl DiagnosticsProvider for DriverProvider {
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics> {
        self.diagnostics_for_files(collection, std::slice::from_ref(file))
            .pop()
    }

    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        DriverSession::default().diagnostics(collection, files)
    }
}

fn debug() -> bool {
    std::env::var_os("DARTR_DEBUG_DRIVER").is_some()
}

/// The settings of the files of one context, by analysis options folder
/// (Dart `AnalysisOptionsMap.getOptions`), owned so that the analysis
/// threads can read them.
struct ContextOptions {
    /// The options folders, deepest first, with their settings.
    folders: Vec<(String, Arc<FileSettings>, ResolverOptions)>,
    default: (Arc<FileSettings>, ResolverOptions),
}

impl ContextOptions {
    fn new(collection: &AnalysisContextCollection, context: usize) -> ContextOptions {
        let map = collection.options_map(&collection.contexts[context]);
        let settings = |o: &dartr_project::AnalysisOptions| {
            (
                Arc::new(FileSettings::new(o)),
                ResolverOptions {
                    strict_casts: o.strict_casts,
                    strict_inference: o.strict_inference,
                    strict_raw_types: o.strict_raw_types,
                },
            )
        };
        ContextOptions {
            folders: map
                .entries()
                .iter()
                .map(|(folder, o)| {
                    let (s, r) = settings(o);
                    (folder.clone(), s, r)
                })
                .collect(),
            default: settings(map.default_options()),
        }
    }

    fn for_path(&self, path: &str) -> (&Arc<FileSettings>, ResolverOptions) {
        for (folder, settings, options) in &self.folders {
            if paths::is_within(folder, path) {
                return (settings, *options);
            }
        }
        (&self.default.0, self.default.1)
    }
}

/// One driver per analysis context, kept between requests.
#[derive(Default)]
pub struct DriverSession {
    generation: Option<Arc<Generation>>,
    /// By context index of the collection.
    drivers: IndexMap<usize, Driver>,
    /// Resolved libraries for the navigation features, by the path of the
    /// defining unit (Dart `AnalysisDriver.getResolvedLibrary` results);
    /// cleared when a file changes.
    resolved: HashMap<(usize, String), Arc<ResolvedLibraryResult>>,
    /// The contexts whose driver ran `discoverAvailableFiles`.
    discovered: std::collections::HashSet<usize>,
}

/// A library file that a driver knows (Dart `FileSystemState.knownFiles`
/// with a library kind).
#[derive(Clone, Debug)]
pub struct KnownLibrary {
    pub path: String,
    pub uri: String,
    pub is_dart: bool,
    pub is_dart_internal: bool,
    pub is_src: bool,
    pub package_name: Option<String>,
}

/// A resolved library with the element model it was resolved against
/// (Dart `ResolvedLibraryResult`).
pub struct ResolvedLibraryResult {
    /// The index of the context whose driver resolved the library.
    pub context: usize,
    /// A snapshot of the linked element model.
    pub world: dartr_element::WorldSnapshot,
    pub type_provider: dartr_element::TypeProvider,
    /// The features of the library.
    pub features: dartr_element::FeatureSet,
    pub library: ResolvedLibrary,
    /// The unit inputs (parsed units), in the order of `library.units`.
    pub inputs: Vec<UnitInput>,
}

impl ResolvedLibraryResult {
    /// The lookup context for unit [index] (with its local elements).
    pub fn ctx<'a>(
        &'a self,
        index: usize,
        sink: &'a dartr_element::NoopSink,
    ) -> dartr_element::Ctx<'a> {
        dartr_element::Ctx {
            world: &self.world,
            current: None,
            local: self.library.units.get(index).map(|u| &u.local),
            tp: &self.type_provider,
            features: &self.features,
            req: sink,
        }
    }

    /// The index of the unit of [path].
    pub fn unit_index(&self, path: &str) -> Option<usize> {
        self.library.units.iter().position(|u| &*u.path == path)
    }
}

/// A linked library (Dart `LibraryElementResult`): the element model of
/// the driver after linking the library.
pub struct LinkedLibraryResult {
    pub context: usize,
    pub world: dartr_element::WorldSnapshot,
    pub type_provider: dartr_element::TypeProvider,
    /// The path of the defining unit of the library.
    pub library_path: String,
    pub uri: String,
}

impl LinkedLibraryResult {
    /// A lookup context without a unit.
    pub fn ctx<'a>(
        &'a self,
        sink: &'a dartr_element::NoopSink,
        features: &'a dartr_element::FeatureSet,
    ) -> dartr_element::Ctx<'a> {
        dartr_element::Ctx {
            world: &self.world,
            current: None,
            local: None,
            tp: &self.type_provider,
            features,
            req: sink,
        }
    }
}

/// What the analysis of one context gives for one requested file.
enum Outcome {
    Done(FileDiagnostics),
    /// Use the parse-only pipeline.
    Fallback(AnalyzedFile),
}

impl DriverSession {
    /// The diagnostics of [files], in any order. Files that cannot be read
    /// give no result.
    pub fn diagnostics(
        &mut self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut by_context: IndexMap<usize, Vec<AnalyzedFile>> = IndexMap::new();
        for f in files {
            by_context.entry(f.context).or_default().push(f.clone());
        }
        let mut result = Vec::new();
        let mut fallback = Vec::new();
        for (context, files) in by_context {
            let options = ContextOptions::new(collection, context);
            let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
                dartr_driver::project::context_driver(collection, context, generation.clone())
            });
            let outcome = pool().install(|| {
                catch_unwind(AssertUnwindSafe(|| {
                    analyze_context(&mut driver, &options, &files)
                }))
            });
            match outcome {
                Ok(outcomes) => {
                    for o in outcomes {
                        match o {
                            Outcome::Done(d) => result.push(d),
                            Outcome::Fallback(f) => fallback.push(f),
                        }
                    }
                    self.drivers.insert(context, driver);
                }
                Err(e) => {
                    // The driver state is not consistent after a panic:
                    // drop it.
                    if debug() {
                        eprintln!(
                            "dartr: driver panic in context {context}: {}",
                            panic_message(&*e)
                        );
                    }
                    fallback.extend(files);
                }
            }
        }
        if !fallback.is_empty() {
            if debug() {
                eprintln!(
                    "dartr: {} files use the parse-only pipeline",
                    fallback.len()
                );
            }
            result.extend(diagnostics_with_reader(
                collection,
                &fallback,
                &dartr_project::fs::read_string,
            ));
        }
        result
    }

    /// Dart `AnalysisDriver.getResolvedLibrary` for the library of [path]
    /// (a library or part file of a context of [collection]); `None` when
    /// the file is not in a library that can be analyzed.
    pub fn resolved_library(
        &mut self,
        collection: &AnalysisContextCollection,
        path: &str,
    ) -> Option<Arc<ResolvedLibraryResult>> {
        let context = collection
            .context_for(path)
            .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))
            .unwrap_or(0);
        self.resolved_library_in(collection, context, path)
    }

    /// [Self::resolved_library] with the driver of [context] (Dart: the
    /// driver that owns [path] in a search, which can be a file of the SDK
    /// or of a package that the context depends on).
    pub fn resolved_library_in(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
        path: &str,
    ) -> Option<Arc<ResolvedLibraryResult>> {
        if context >= collection.contexts.len() {
            return None;
        }
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
            dartr_driver::project::context_driver(collection, context, generation.clone())
        });
        let options = ContextOptions::new(collection, context);
        let resolved = &mut self.resolved;
        let result = pool().install(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let id = driver.fs.get_file_for_path(path);
                driver.fs.discover();
                if !driver.fs.file(id).exists() {
                    return None;
                }
                let library = driver.fs.library_of(id)?;
                let library_path = driver.fs.file(library).path.to_string();
                let key = (context, library_path.clone());
                if let Some(r) = resolved.get(&key) {
                    return Some(r.clone());
                }
                driver.link_libraries(&[library]);
                let (unignorable, resolver_options) = {
                    let (settings, options) = options.for_path(&library_path);
                    let mut names: Vec<String> =
                        settings.unignorable_names.iter().cloned().collect();
                    names.sort();
                    (names, options)
                };
                let jobs = [(library, resolver_options, &unignorable[..])];
                let world = driver.state.world.clone();
                let type_provider = dartr_link::types_builder::world_type_provider(&world);
                let mut out = driver.analyze_libraries(&jobs, |_, units, result| {
                    result.ok().map(|library| (library, units.to_vec()))
                });
                let (library, inputs) = out.pop().flatten()?;
                let features = {
                    let sink = dartr_element::NoopSink;
                    let features = dartr_element::FeatureSet::default();
                    let ctx = dartr_element::Ctx {
                        world: &world,
                        current: None,
                        local: None,
                        tp: &type_provider,
                        features: &features,
                        req: &sink,
                    };
                    ctx.get(library.library).feature_set.clone()
                };
                let entry = Arc::new(ResolvedLibraryResult {
                    context,
                    world,
                    type_provider,
                    features,
                    library,
                    inputs,
                });
                resolved.insert(key, entry.clone());
                Some(entry)
            }))
        });
        match result {
            Ok(r) => {
                self.drivers.insert(context, driver);
                r
            }
            Err(e) => {
                if debug() {
                    eprintln!(
                        "dartr: driver panic in context {context}: {}",
                        panic_message(&*e)
                    );
                }
                None
            }
        }
    }

    /// The parsed units of all files that the driver of the context of
    /// [path] knows (Dart `FileSystemState` files), with their paths.
    pub fn known_parsed_units(
        &mut self,
        collection: &AnalysisContextCollection,
        path: &str,
    ) -> Vec<(Arc<str>, Arc<dartr_ast_builder::ParsedUnit>)> {
        let Some(context) = collection
            .context_for(path)
            .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))
        else {
            return Vec::new();
        };
        let Some(driver) = self.drivers.get(&context) else {
            return Vec::new();
        };
        driver
            .fs
            .files()
            .iter()
            .filter_map(|f| {
                let content = f.content.as_ref()?;
                Some((f.path.clone(), content.parsed.clone()))
            })
            .collect()
    }

    /// Dart `AnalysisDriver.getLibraryByUri` for the library of [path] in
    /// the driver of [context]: the linked element model (no resolution of
    /// the bodies) and the URI of the library; `None` when the file does
    /// not exist or linking panics.
    pub fn linked_library_in(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
        path: &str,
    ) -> Option<LinkedLibraryResult> {
        if context >= collection.contexts.len() {
            return None;
        }
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
            dartr_driver::project::context_driver(collection, context, generation.clone())
        });
        let result = pool().install(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let id = driver.fs.get_file_for_path(path);
                driver.fs.discover();
                if !driver.fs.file(id).exists() {
                    return None;
                }
                let library = driver.fs.library_of(id)?;
                let library_path = driver.fs.file(library).path.to_string();
                let uri = driver.fs.file(library).uri_str.to_string();
                driver.link_libraries(&[library]);
                let world = driver.state.world.clone();
                let type_provider = dartr_link::types_builder::world_type_provider(&world);
                Some(LinkedLibraryResult {
                    context,
                    world,
                    type_provider,
                    library_path,
                    uri,
                })
            }))
        });
        match result {
            Ok(r) => {
                self.drivers.insert(context, driver);
                r
            }
            Err(_) => None,
        }
    }

    /// Dart `AnalysisDriver.discoverAvailableFiles` for the driver of
    /// [context] (once: the SDK libraries and the Dart files of the package
    /// `lib` folders), then links every library file that the driver knows
    /// (Dart `knownFiles` + `getLibraryByUri` of each). Returns the element
    /// model after linking and the library files in the order the driver
    /// found them; `None` when linking panics.
    pub fn link_known_libraries(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
    ) -> Option<(dartr_element::WorldSnapshot, Vec<KnownLibrary>)> {
        if context >= collection.contexts.len() {
            return None;
        }
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
            dartr_driver::project::context_driver(collection, context, generation.clone())
        });
        let discover = self.discovered.insert(context);
        let analysis_context = &collection.contexts[context];
        let sdk = analysis_context.sdk.as_ref().or(collection.sdk.as_ref());
        let mut available: Vec<String> = Vec::new();
        if discover {
            if let Some(sdk) = sdk {
                available.extend(
                    sdk.libraries()
                        .iter()
                        .filter_map(|l| sdk.map_dart_uri(&l.short_name)),
                );
            }
            fn recurse(folder: &str, out: &mut Vec<String>) {
                let Some(children) = dartr_project::fs::children(folder) else {
                    return;
                };
                for child in children {
                    match child.kind {
                        dartr_project::fs::ResourceKind::File => {
                            if child.path.ends_with(".dart") {
                                out.push(child.path);
                            }
                        }
                        dartr_project::fs::ResourceKind::Folder => recurse(&child.path, out),
                    }
                }
            }
            for package in analysis_context.packages.packages() {
                recurse(&package.lib, &mut available);
            }
        }
        let result = pool().install(|| {
            catch_unwind(AssertUnwindSafe(|| {
                for path in &available {
                    driver.fs.get_file_for_path(path);
                }
                driver.fs.discover();
                let mut libraries = Vec::new();
                let mut ids = Vec::new();
                for f in driver.fs.files() {
                    let Some(content) = f.content.as_ref() else {
                        continue;
                    };
                    if !content.exists || !f.path.ends_with(".dart") || !content.kind.is_library() {
                        continue;
                    }
                    ids.push(f.id);
                    libraries.push(KnownLibrary {
                        path: f.path.to_string(),
                        uri: f.uri_str.to_string(),
                        is_dart: f.uri_properties.is_dart,
                        is_dart_internal: f.uri_properties.is_dart_internal,
                        is_src: f.uri_properties.is_src,
                        package_name: f.uri_properties.package_name.clone(),
                    });
                }
                driver.link_libraries(&ids);
                (driver.state.world.clone(), libraries)
            }))
        });
        match result {
            Ok(r) => {
                self.drivers.insert(context, driver);
                Some(r)
            }
            Err(_) => None,
        }
    }

    /// Dart `VariableElement.computeConstantValue()` of [elements] with the
    /// driver of [context] (the units of other libraries are resolved on
    /// demand), each mapped with [f]; `None` for a variable without a valid
    /// constant value. [library] is the library of the request.
    pub fn constant_values_of<T>(
        &self,
        context: usize,
        library: dartr_element::EId<dartr_element::LibraryElement>,
        elements: &[dartr_element::ElementId],
        f: impl Fn(&dartr_constant::DartObjectImpl) -> Option<T>,
    ) -> Vec<Option<T>> {
        let Some(driver) = self.drivers.get(&context) else {
            return elements.iter().map(|_| None).collect();
        };
        static DECLARED_VARIABLES: std::sync::LazyLock<dartr_constant::DeclaredVariables> =
            std::sync::LazyLock::new(dartr_constant::DeclaredVariables::new);
        let world = &driver.state.world;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let external = dartr_resolver::library_analyzer::ExternalUnitCache::new(
            world,
            &tp,
            dartr_resolver::options::AnalysisOptions::default(),
            driver.unit_sources(),
        );
        let engine = dartr_resolver::constant::evaluation::ConstantEvaluationEngine::new(
            world,
            &tp,
            library,
            &DECLARED_VARIABLES,
            &[],
            Some(&external),
        );
        elements
            .iter()
            .map(|&e| {
                catch_unwind(AssertUnwindSafe(|| {
                    engine.compute_constant_value_of(e).and_then(|v| f(&v))
                }))
                .ok()
                .flatten()
            })
            .collect()
    }

    /// The paths of the library files that the driver of [context] knows,
    /// in the order in which the Dart driver creates their `FileState`s
    /// (the order of `FileSystemState.knownFiles`): `dart:core` and the
    /// files it references (`_discoverDartCore`), the added files
    /// (`_discoverLibraries`), the files that the library cycle walk of the
    /// analysis of each added file creates (`_LibraryNode.computeDependencies`
    /// and the doc imports of `addDirectivesSignature` when a cycle is
    /// evaluated), then [available] (`discoverAvailableFiles`). Call after
    /// [Self::link_known_libraries].
    pub fn dart_known_order(
        &mut self,
        context: usize,
        added: &[String],
        available: &[String],
    ) -> Vec<String> {
        let Some(driver) = self.drivers.get_mut(&context) else {
            return Vec::new();
        };
        let fs = &mut driver.fs;
        let mut order = KnownOrder {
            created: indexmap::IndexSet::new(),
            evaluated: std::collections::HashSet::new(),
        };
        // `_discoverDartCore`.
        let core = fs
            .files()
            .iter()
            .find(|f| &*f.uri_str == "dart:core")
            .map(|f| f.id);
        if let Some(core) = core {
            order.insert(fs, core);
            if fs.file(core).content.is_some() {
                let c = fs.file(core).c();
                let mut uris: Vec<dartr_driver::file_state::DirectiveUris> = Vec::new();
                uris.extend(c.library_exports.iter().map(|d| d.uris.clone()));
                uris.extend(c.library_imports.iter().map(|d| d.uris.clone()));
                uris.extend(c.part_includes.iter().map(|d| d.uris.clone()));
                uris.extend(c.doc_library_imports.iter().map(|d| d.uris.clone()));
                for u in &uris {
                    order.create(fs, u);
                }
            }
        }
        // `_discoverLibraries`.
        let added_ids: Vec<FileId> = added
            .iter()
            .filter_map(|p| fs.get_existing_from_path(p))
            .collect();
        for &id in &added_ids {
            order.insert(fs, id);
        }
        // The analysis of each added file: the walk of its library.
        for &id in &added_ids {
            if fs.file(id).content.is_none() {
                continue;
            }
            if let Some(library) = fs.library_of(id) {
                order.walk(fs, library);
            }
        }
        // `discoverAvailableFiles`.
        for p in available {
            if let Some(id) = fs.get_existing_from_path(p) {
                order.insert(fs, id);
            }
        }
        order
            .created
            .into_iter()
            .filter(|&id| {
                let f = fs.file(id);
                f.content
                    .as_ref()
                    .is_some_and(|c| c.exists && c.kind.is_library())
                    && f.path.ends_with(".dart")
            })
            .map(|id| fs.file(id).path.to_string())
            .collect()
    }

    /// The existing Dart files that the driver of [context] knows (Dart
    /// `FileSystemState.knownFiles`), in the order the driver found them;
    /// empty when the context has no driver yet.
    pub fn known_files(&self, context: usize) -> Vec<String> {
        let Some(driver) = self.drivers.get(&context) else {
            return Vec::new();
        };
        driver
            .fs
            .files()
            .iter()
            .filter(|f| f.content.is_some() && f.exists() && f.path.ends_with(".dart"))
            .map(|f| f.path.to_string())
            .collect()
    }

    /// The library file of the part [path] in the driver of [context] when
    /// the part names its library with a URI (Dart creates that library
    /// file when it creates the part: `PartOfUriKnownFileKind`).
    pub fn part_of_uri_library(&self, context: usize, path: &str) -> Option<String> {
        let driver = self.drivers.get(&context)?;
        let id = driver.fs.get_existing_from_path(path)?;
        let file = driver.fs.file(id);
        file.content.as_ref()?;
        match file.kind() {
            dartr_driver::file_state::FileKind::PartOfUriKnown { uri_file } => {
                Some(driver.fs.file(*uri_file).path.to_string())
            }
            _ => None,
        }
    }

    /// Dart `AnalysisDriver.changeFile` for every driver: reads [path] again
    /// and invalidates what depends on it (design §4.2). Returns the paths
    /// of the units of the libraries to analyze again.
    pub fn change_file(&mut self, path: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for (&context, driver) in self.drivers.iter_mut() {
            if driver.fs.get_existing_from_path(path).is_none() {
                continue;
            }
            let change = driver.change_file(path);
            for library in change.libraries {
                // The resolved library is not valid anymore (Dart: the
                // library signature changed).
                self.resolved
                    .remove(&(context, driver.fs.file(library).path.to_string()));
                for unit in driver.fs.library_file_kinds(library) {
                    let p = driver.fs.file(unit).path.to_string();
                    if !result.contains(&p) {
                        result.push(p);
                    }
                }
            }
        }
        result
    }
}

/// Analyzes the requested [files] of one context with its [driver].
fn analyze_context(
    driver: &mut Driver,
    options: &ContextOptions,
    files: &[AnalyzedFile],
) -> Vec<Outcome> {
    let ids: Vec<FileId> = files
        .iter()
        .map(|f| driver.fs.get_file_for_path(&f.path))
        .collect();
    driver.fs.discover();

    let mut outcomes = Vec::new();
    // The requested files of each library (defining unit).
    let mut libraries: IndexMap<FileId, Vec<FileId>> = IndexMap::new();
    let mut requested: HashMap<FileId, &AnalyzedFile> = HashMap::new();
    for (file, &id) in files.iter().zip(&ids) {
        if !driver.fs.file(id).exists() {
            // Like the parse-only provider: no result for a file that
            // cannot be read.
            continue;
        }
        match driver.fs.library_of(id) {
            Some(library) => {
                libraries.entry(library).or_default().push(id);
                requested.insert(id, file);
            }
            None => outcomes.push(Outcome::Fallback(file.clone())),
        }
    }
    let library_ids: Vec<FileId> = libraries.keys().copied().collect();
    driver.link_libraries(&library_ids);

    // The unignorable codes of each library: the library analyzer keeps
    // them in its ignore filtering, like `finish_file` does.
    let unignorable: Vec<Vec<String>> = library_ids
        .iter()
        .map(|&l| {
            let mut names: Vec<String> = options
                .for_path(&driver.fs.file(l).path)
                .0
                .unignorable_names
                .iter()
                .cloned()
                .collect();
            names.sort();
            names
        })
        .collect();
    // The rules that need resolution run on the resolved units (Dart
    // `_computeLints`); the parse-only rules keep running on the parsed units.
    let parsed_rules = dartr_lints::rules::parsed_rules();
    let jobs: Vec<dartr_driver::analysis::LintJob<'_>> = library_ids
        .iter()
        .zip(&unignorable)
        .map(|(&l, names)| {
            let path = &driver.fs.file(l).path;
            let resolved_rules = options
                .for_path(path)
                .0
                .lint_rules
                .iter()
                .filter(|rule| !parsed_rules.contains(rule))
                .map(|rule| rule.to_string())
                .collect();
            (l, options.for_path(path).1, &names[..], resolved_rules)
        })
        .collect();
    let requested = &requested;
    let libraries = &libraries;
    let analyzed = driver.analyze_libraries_with_lints(&jobs, |library, units, result, lints| {
        library_outcomes(
            options,
            &libraries[&library],
            requested,
            units,
            result,
            lints,
        )
    });
    outcomes.extend(analyzed.into_iter().flatten());
    outcomes
}

/// The diagnostics of the [wanted] units of one analyzed library.
fn library_outcomes(
    options: &ContextOptions,
    wanted: &[FileId],
    requested: &HashMap<FileId, &AnalyzedFile>,
    units: &[UnitInput],
    result: Result<ResolvedLibrary, String>,
    resolved_lints: Vec<Vec<Diagnostic>>,
) -> Vec<Outcome> {
    let fallback = || {
        wanted
            .iter()
            .map(|id| Outcome::Fallback(requested[id].clone()))
            .collect()
    };
    let Some(first) = units.first() else {
        return fallback();
    };
    let mut resolved = match result {
        Ok(resolved) => resolved,
        Err(message) => {
            if debug() {
                eprintln!("dartr: analysis panic in {}: {message}", first.path);
            }
            return fallback();
        }
    };
    let (library_settings, _) = options.for_path(&first.path);
    let parsed_rules = dartr_lints::rules::parsed_rules();
    let parsed_lint_rules: Vec<&str> = library_settings
        .lint_rules
        .iter()
        .copied()
        .filter(|rule| parsed_rules.contains(rule))
        .collect();
    let mut lints: Vec<Vec<Diagnostic>> = if parsed_lint_rules.is_empty() {
        vec![Vec::new(); units.len()]
    } else {
        let context_units: Vec<RuleContextUnit<'_>> = units
            .iter()
            .map(|u| RuleContextUnit {
                parsed: &u.parsed,
                source: &u.parsed.ast.tokens.source,
                path: &u.path,
            })
            .collect();
        lint_library_unfiltered(&context_units, &parsed_lint_rules)
    };
    for (unit_lints, resolved) in lints.iter_mut().zip(resolved_lints) {
        unit_lints.extend(resolved);
    }
    let mut outcomes = Vec::new();
    for &id in wanted {
        let file = requested[&id];
        let Some(index) = units.iter().position(|u| *u.path == *file.path) else {
            // Not a unit of its library (for example a part that the
            // library includes twice).
            outcomes.push(Outcome::Fallback(file.clone()));
            continue;
        };
        let unit = &mut resolved.units[index];
        if let Some(panic) = &unit.panic
            && debug()
        {
            eprintln!("dartr: resolver panic in {}: {panic}", unit.path);
        }
        let parsed = &units[index].parsed;
        let (settings, _) = options.for_path(&file.path);
        outcomes.push(Outcome::Done(finish_file(
            &file.path,
            &parsed.ast.tokens.source,
            parsed,
            [
                std::mem::take(&mut unit.diagnostics),
                std::mem::take(&mut unit.ignored_diagnostics),
            ]
            .concat(),
            lints.get(index).cloned().unwrap_or_default(),
            settings,
        )));
    }
    outcomes
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}

/// The state of [`DriverSession::dart_known_order`].
struct KnownOrder {
    created: indexmap::IndexSet<FileId>,
    /// The libraries whose cycle is computed (Dart `isEvaluated`).
    evaluated: std::collections::HashSet<FileId>,
}

impl KnownOrder {
    /// Dart `_buildConfigurableDirectiveUris`: the primary URI, then the
    /// URIs of the configurations.
    fn create(
        &mut self,
        fs: &dartr_driver::file_state::FileSystemState,
        uris: &dartr_driver::file_state::DirectiveUris,
    ) {
        use dartr_driver::file_state::DirectiveUri;
        for u in std::iter::once(&uris.primary).chain(&uris.configurations) {
            if let DirectiveUri::WithFile { file, .. } = u {
                self.insert(fs, *file);
            }
        }
    }

    /// Dart `FileSystemState._newFile`: the file, then (when the kind of
    /// the file is `PartOfUriKnownFileKind`) the file of its library.
    fn insert(&mut self, fs: &dartr_driver::file_state::FileSystemState, file: FileId) {
        if !self.created.insert(file) {
            return;
        }
        if let Some(c) = fs.file(file).content.as_ref() {
            if let dartr_driver::file_state::FileKind::PartOfUriKnown { uri_file } = c.kind {
                self.insert(fs, uri_file);
            }
        }
    }

    /// Dart `LibraryFileKind.fileKinds`, with the files of the part
    /// directives created on the way.
    fn file_kinds(
        &mut self,
        fs: &dartr_driver::file_state::FileSystemState,
        library: FileId,
    ) -> Vec<FileId> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![library];
        // Depth first, in directive order.
        fn visit(
            o: &mut KnownOrder,
            fs: &dartr_driver::file_state::FileSystemState,
            kind: FileId,
            result: &mut Vec<FileId>,
            visited: &mut std::collections::HashSet<FileId>,
        ) {
            if !visited.insert(kind) || fs.file(kind).content.is_none() {
                return;
            }
            result.push(kind);
            let parts = fs.file(kind).c().part_includes.clone();
            for d in &parts {
                o.create(fs, &d.uris);
            }
            for d in &parts {
                if let Some(part) = fs.included_part(kind, d) {
                    visit(o, fs, part, result, visited);
                }
            }
        }
        while let Some(k) = stack.pop() {
            visit(self, fs, k, &mut result, &mut visited);
        }
        result
    }

    /// Dart `_LibraryNode.computeDependencies`.
    fn dependencies(
        &mut self,
        fs: &dartr_driver::file_state::FileSystemState,
        library: FileId,
    ) -> Vec<FileId> {
        let mut deps = indexmap::IndexSet::new();
        for kind in self.file_kinds(fs, library) {
            let c = fs.file(kind).c();
            let imports = c.library_imports.clone();
            let exports = c.library_exports.clone();
            for d in &imports {
                self.create(fs, &d.uris);
            }
            for d in &imports {
                if let Some(l) = fs.library_of_uri(&d.uris.selected) {
                    deps.insert(l);
                }
            }
            for d in &exports {
                self.create(fs, &d.uris);
            }
            for d in &exports {
                if let Some(l) = fs.library_of_uri(&d.uris.selected) {
                    deps.insert(l);
                }
            }
        }
        deps.into_iter().collect()
    }

    /// Dart `_LibraryWalker.evaluateScc`: the doc imports of the files of
    /// the cycle (sorted by path) are created by `addDirectivesSignature`.
    fn evaluate_scc(&mut self, fs: &dartr_driver::file_state::FileSystemState, scc: &[FileId]) {
        let mut files: Vec<FileId> = scc.iter().flat_map(|&l| fs.library_files(l)).collect();
        files.sort_by(|a, b| fs.file(*a).path.cmp(&fs.file(*b).path));
        for f in files {
            let docs = fs.file(f).c().doc_library_imports.clone();
            for d in &docs {
                self.create(fs, &d.uris);
            }
        }
        for &l in scc {
            self.evaluated.insert(l);
        }
    }

    /// Dart `DependencyWalker.walk` (Tarjan).
    fn walk(&mut self, fs: &dartr_driver::file_state::FileSystemState, start: FileId) {
        if self.evaluated.contains(&start) {
            return;
        }
        struct State {
            index: std::collections::HashMap<FileId, (u32, u32)>,
            next: u32,
            stack: Vec<FileId>,
        }
        fn strong_connect(
            o: &mut KnownOrder,
            fs: &dartr_driver::file_state::FileSystemState,
            st: &mut State,
            node: FileId,
        ) {
            st.index.insert(node, (st.next, st.next));
            st.next += 1;
            st.stack.push(node);
            for dep in o.dependencies(fs, node) {
                if o.evaluated.contains(&dep) || dep == node {
                    continue;
                }
                match st.index.get(&dep).copied() {
                    None => {
                        strong_connect(o, fs, st, dep);
                        let dep_low = st.index[&dep].1;
                        let e = st.index.get_mut(&node).unwrap();
                        if dep_low < e.1 {
                            e.1 = dep_low;
                        }
                    }
                    Some((dep_index, _)) => {
                        let e = st.index.get_mut(&node).unwrap();
                        if dep_index < e.1 {
                            e.1 = dep_index;
                        }
                    }
                }
            }
            let (index, low) = st.index[&node];
            if low == index {
                let mut scc = Vec::new();
                loop {
                    let other = st.stack.pop().unwrap();
                    scc.push(other);
                    if other == node {
                        break;
                    }
                }
                o.evaluate_scc(fs, &scc);
            }
        }
        let mut st = State {
            index: std::collections::HashMap::new(),
            next: 1,
            stack: Vec::new(),
        };
        strong_connect(self, fs, &mut st, start);
    }
}
