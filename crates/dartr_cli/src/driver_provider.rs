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
#[derive(Clone, Debug, Default)]
pub struct DriverProvider {
    pub cache_dir: Option<String>,
}

impl DriverProvider {
    pub fn new(cache_dir: Option<String>) -> Self {
        Self { cache_dir }
    }
}

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
        let cache = crate::args::resolve_cache_dir(self.cache_dir.as_deref(), false);
        self.diagnostics_for_files_with_cache(collection, files, cache.as_deref())
    }

    fn diagnostics_for_files_with_cache(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
        cache_dir: Option<&str>,
    ) -> Vec<FileDiagnostics> {
        let cache = cache_dir
            .map(str::to_owned)
            .or_else(|| self.cache_dir.clone());
        DriverSession::one_shot_with_cache_dir(cache).diagnostics(collection, files)
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
pub struct DriverSession {
    generation: Option<Arc<Generation>>,
    /// By context index of the collection.
    drivers: IndexMap<usize, Driver>,
    /// Resolved libraries for the priority (open) files, by the path of the
    /// defining unit (Dart `AnalysisDriver._priorityResults`); cleared for a
    /// library when it or a dependency changes.
    resolved: HashMap<(usize, String), Arc<ResolvedLibraryResult>>,
    /// The priority (open) file paths (Dart `AnalysisDriver.priorityFiles`).
    priority_files: std::collections::HashSet<String>,
    /// The contexts whose driver ran `discoverAvailableFiles`.
    discovered: std::collections::HashSet<usize>,
    /// Cached `(world_valid, ordered_known_libraries)` per `(context, pkg_root)`
    /// for `link_known_libraries_ordered`.
    known_libraries_cache: HashMap<(usize, Option<String>), (bool, Arc<Vec<KnownLibrary>>)>,
    /// Whether to retain `Driver` instances in `self.drivers` after
    /// `diagnostics_with_index` (true for the language server, false for
    /// one-shot `dartr analyze`).
    retain_drivers: bool,
    /// Optional persistent result cache directory (`--cache` / `DARTR_CACHE_DIR`).
    cache_dir: Option<String>,
}

impl Default for DriverSession {
    fn default() -> Self {
        Self {
            generation: None,
            drivers: IndexMap::new(),
            resolved: HashMap::new(),
            priority_files: std::collections::HashSet::new(),
            discovered: std::collections::HashSet::new(),
            known_libraries_cache: HashMap::new(),
            retain_drivers: true,
            cache_dir: None,
        }
    }
}

/// A library file that a driver knows (Dart `FileSystemState.knownFiles`
/// with a library kind).
#[derive(Clone, Debug)]
pub struct KnownLibrary {
    pub id: FileId,
    pub should_link: bool,
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
    /// Creates a one-shot session for `dartr analyze` that drops each context's
    /// `Driver` once its diagnostics are computed.
    pub fn one_shot() -> Self {
        Self {
            retain_drivers: false,
            ..Self::default()
        }
    }

    /// Creates a one-shot session with the given optional result cache directory.
    pub fn one_shot_with_cache_dir(cache_dir: Option<String>) -> Self {
        Self {
            retain_drivers: false,
            cache_dir,
            ..Self::default()
        }
    }

    /// Creates a new session with the given optional result cache directory.
    pub fn with_cache_dir(cache_dir: Option<String>) -> Self {
        Self {
            cache_dir,
            ..Self::default()
        }
    }

    /// Returns the configured result cache directory, if any.
    pub fn cache_dir(&self) -> Option<&str> {
        self.cache_dir.as_deref()
    }

    /// Sets the result cache directory.
    pub fn set_cache_dir(&mut self, cache_dir: Option<String>) {
        self.cache_dir = cache_dir;
    }

    /// Updates the priority (open) files (Dart `AnalysisDriver.priorityFiles=`).
    /// Evicts cached resolved libraries that no longer contain any priority file.
    pub fn set_priority_files(&mut self, files: &[String]) {
        self.priority_files = files.iter().cloned().collect();
        let priority = &self.priority_files;
        self.resolved.retain(|_, lib| {
            lib.library
                .units
                .iter()
                .any(|u| priority.contains(&*u.path))
        });
    }

    /// Returns true if any driver already knows [path] with the exact [text].
    pub fn file_has_content(&self, path: &str, text: &str) -> bool {
        for driver in self.drivers.values() {
            if let Some(id) = driver.fs.get_existing_from_path(path)
                && let Some(content) = driver.fs.file(id).content.as_ref()
                && content.exists
                && content.source.as_ref() == text
            {
                return true;
            }
        }
        false
    }

    /// Returns the cached `LineInfo` of [path] from any driver if present
    /// (when [expected_content] is `Some`, only if the driver's content matches).
    pub fn line_info_matching(
        &self,
        path: &str,
        expected_content: Option<&str>,
    ) -> Option<dartr_syntax::LineInfo> {
        for driver in self.drivers.values() {
            if let Some(id) = driver.fs.get_existing_from_path(path)
                && let Some(content) = driver.fs.file(id).content.as_ref()
                && content.exists
                && expected_content.is_none_or(|exp| content.source.as_ref() == exp)
            {
                return Some(dartr_syntax::LineInfo::new(
                    content.unlinked.line_starts.clone(),
                ));
            }
        }
        None
    }

    /// Returns the cached content of [path] from any driver if present.
    pub fn file_content(&self, path: &str) -> Option<Arc<str>> {
        for driver in self.drivers.values() {
            if let Some(id) = driver.fs.get_existing_from_path(path)
                && let Some(content) = driver.fs.file(id).content.as_ref()
                && content.exists
            {
                return Some(content.source.clone());
            }
        }
        None
    }

    /// The diagnostics of [files], in any order. Files that cannot be read
    /// give no result.
    pub fn diagnostics(
        &mut self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        self.diagnostics_with_index::<
            (),
            fn(&dartr_element::Ctx<'_>, &dartr_resolver::library_analyzer::ResolvedUnit),
        >(collection, files, None)
            .0
    }

    /// Like [Self::diagnostics], and optionally computes an index entry `I`
    /// in parallel on each resolved unit while caching `ResolvedLibraryResult`
    /// for priority files.
    pub fn diagnostics_with_index<I, F>(
        &mut self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
        index_fn: Option<&F>,
    ) -> (Vec<FileDiagnostics>, Vec<((usize, String), I)>)
    where
        I: Send,
        F: Fn(&dartr_element::Ctx<'_>, &dartr_resolver::library_analyzer::ResolvedUnit) -> I + Sync,
    {
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut by_context: IndexMap<usize, Vec<AnalyzedFile>> = IndexMap::new();
        for f in files {
            by_context.entry(f.context).or_default().push(f.clone());
        }
        let retain_drivers = self.retain_drivers;
        if !retain_drivers && by_context.len() > 1 {
            // Process smaller contexts first so the largest context runs last,
            // reuses shared SdkCache entries, and frees all unused SdkCache
            // entries before linking and resolving its own libraries.
            by_context.sort_by(|_, a, _, b| a.len().cmp(&b.len()));
        }
        let mut result = Vec::new();
        let mut indexed = Vec::new();
        let mut fallback = Vec::new();
        let timings = std::env::var_os("DARTR_TIMINGS").is_some();
        let mut total_discover = std::time::Duration::ZERO;
        let mut total_link = std::time::Duration::ZERO;
        let mut total_resolve = std::time::Duration::ZERO;
        let num_contexts = by_context.len();
        let context_order: Vec<usize> = by_context.keys().copied().collect();
        let priority_files = &self.priority_files;
        for (idx, (context, files)) in by_context.into_iter().enumerate() {
            let options = ContextOptions::new(collection, context);
            let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
                dartr_driver::project::context_driver(collection, context, generation.clone())
            });
            if !retain_drivers && idx + 1 == num_contexts {
                driver.fs.sdk_cache_lookup_only = true;
                driver.clear_sdk_cache_after_lookup = true;
            }
            let outcome = pool().install(|| {
                catch_unwind(AssertUnwindSafe(|| {
                    analyze_context(
                        context,
                        &mut driver,
                        &options,
                        &files,
                        priority_files,
                        index_fn,
                    )
                }))
            });
            match outcome {
                Ok((
                    outcomes,
                    priority_resolved,
                    context_indexed,
                    (t_discover, t_link, t_resolve),
                )) => {
                    total_discover += t_discover;
                    total_link += t_link;
                    total_resolve += t_resolve;
                    if timings {
                        eprintln!(
                            "[driver] context {context}: discover {:?} ({} known files), link {:?} ({} stores), resolve+lint {:?}",
                            t_discover,
                            driver.fs.files().len(),
                            t_link,
                            driver.state.world.stores.len(),
                            t_resolve,
                        );
                    }
                    for o in outcomes {
                        match o {
                            Outcome::Done(d) => result.push(d),
                            Outcome::Fallback(f) => fallback.push(f),
                        }
                    }
                    for (key, entry) in priority_resolved {
                        self.resolved.insert(key, entry);
                    }
                    indexed.extend(context_indexed);
                    if retain_drivers {
                        self.drivers.insert(context, driver);
                    } else {
                        let mut remaining_roots: Vec<String> = Vec::new();
                        for &rem_ctx in &context_order[idx + 1..] {
                            let ac = &collection.contexts[rem_ctx];
                            if let Some(sdk) = ac.sdk.as_ref().or(collection.sdk.as_ref()) {
                                remaining_roots.push(sdk.path().to_string());
                            }
                            for pkg in ac.packages.packages() {
                                remaining_roots.push(pkg.lib.clone());
                            }
                        }
                        remaining_roots.sort();
                        remaining_roots.dedup();
                        driver.retain_sdk_cache_for_roots(&remaining_roots);
                        if std::env::var_os("DARTR_MEM_STATS").is_some() && idx + 1 == num_contexts
                        {
                            self.drivers.insert(context, driver);
                            self.print_mem_stats("after_diagnostics");
                            self.drivers.shift_remove(&context);
                        }
                    }
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
        dartr_driver::driver::drop_cached_parsed(&generation);
        if timings && num_contexts > 1 {
            eprintln!(
                "[driver] total ({num_contexts} contexts): discover {:?}, link {:?}, resolve+lint {:?}",
                total_discover, total_link, total_resolve,
            );
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
        if std::env::var_os("DARTR_MEM_STATS").is_some() && retain_drivers {
            self.print_mem_stats("after_diagnostics");
        }
        (result, indexed)
    }

    /// Resolves the unique libraries of [paths] in [context] in parallel and
    /// computes `index_fn` for every unit without retaining non-priority
    /// `ResolvedLibraryResult`s in memory. When `subtypes_of` is `Some(names)`,
    /// only files whose parsed AST actually contains a subtype clause
    /// (`ExtendsClause`, `WithClause`, `ImplementsClause`, `MixinOnClause`, or
    /// `ClassTypeAlias`) referencing one of `names` (or an implicit `Object`
    /// supertype) are resolved and indexed.
    pub fn index_files_in<I, F>(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
        paths: &[String],
        subtypes_of: Option<&[String]>,
        index_fn: &F,
    ) -> (Vec<((usize, String), I)>, Vec<String>)
    where
        I: Send,
        F: Fn(&dartr_element::Ctx<'_>, &dartr_resolver::library_analyzer::ResolvedUnit) -> I + Sync,
    {
        if context >= collection.contexts.len() || paths.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
            dartr_driver::project::context_driver(collection, context, generation.clone())
        });
        let options = ContextOptions::new(collection, context);
        let outcome = pool().install(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let ids: Vec<FileId> = paths
                    .iter()
                    .map(|p| driver.fs.get_file_for_path(p))
                    .collect();
                driver.fs.discover();
                let mut library_ids: indexmap::IndexSet<FileId> = indexmap::IndexSet::new();
                for id in ids {
                    let f = driver.fs.file(id);
                    let Some(c) = &f.content else {
                        continue;
                    };
                    if !c.exists {
                        continue;
                    }
                    if let Some(names) = subtypes_of {
                        let parsed = f.parsed();
                        if !unit_has_subtype_of(&parsed, names) {
                            f.drop_parsed();
                            continue;
                        }
                    }
                    if let Some(lib) = driver.fs.library_of(id) {
                        library_ids.insert(lib);
                    } else {
                        driver.fs.file(id).drop_parsed();
                    }
                }
                let library_ids: Vec<FileId> = library_ids.into_iter().collect();
                if library_ids.is_empty() {
                    driver.fs.clear_all_parsed();
                    return (Vec::new(), Vec::new());
                }
                driver.link_libraries(&library_ids);
                let unignorable: Vec<(Vec<String>, ResolverOptions)> = library_ids
                    .iter()
                    .map(|&l| {
                        let (settings, opts) = options.for_path(&driver.fs.file(l).path);
                        let mut names: Vec<String> =
                            settings.unignorable_names.iter().cloned().collect();
                        names.sort();
                        (names, opts)
                    })
                    .collect();
                let jobs: Vec<(FileId, ResolverOptions, &[String])> = library_ids
                    .iter()
                    .zip(&unignorable)
                    .map(|(&l, (names, opts))| (l, *opts, &names[..]))
                    .collect();
                let world = driver.state.world.clone();
                let type_provider = dartr_link::types_builder::world_type_provider(&world);
                let analyzed = driver.analyze_libraries(&jobs, |_, units, result| {
                    let Ok(resolved) = result else {
                        return (Vec::new(), Vec::new());
                    };
                    let sink = dartr_element::NoopSink;
                    let default_features = dartr_element::FeatureSet::default();
                    let base_ctx = dartr_element::Ctx {
                        world: &world,
                        current: None,
                        local: None,
                        tp: &type_provider,
                        features: &default_features,
                        req: &sink,
                    };
                    let features = base_ctx.get(resolved.library).feature_set.clone();
                    let unit_paths: Vec<String> =
                        units.iter().map(|u| u.path.to_string()).collect();
                    let mut out = Vec::with_capacity(resolved.units.len());
                    for unit in &resolved.units {
                        let unit_ctx = dartr_element::Ctx {
                            world: &world,
                            current: None,
                            local: Some(&unit.local),
                            tp: &type_provider,
                            features: &features,
                            req: &sink,
                        };
                        let idx = index_fn(&unit_ctx, unit);
                        out.push(((context, unit.path.to_string()), idx));
                    }
                    (out, unit_paths)
                });
                driver.fs.clear_all_parsed();
                let mut indexed = Vec::new();
                let mut unit_paths = Vec::new();
                for (i, u) in analyzed {
                    indexed.extend(i);
                    unit_paths.extend(u);
                }
                (indexed, unit_paths)
            }))
        });
        match outcome {
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
                (Vec::new(), Vec::new())
            }
        }
    }

    pub fn print_mem_stats(&self, phase: &str) {
        use std::collections::{HashMap, HashSet};
        use std::mem::size_of;

        let mut total_file_states = 0usize;
        let mut total_file_contents = 0usize;
        let mut unique_parsed_ptrs = HashSet::new();
        let mut total_ast = dartr_ast::AstMemStats::default();
        let mut unique_ast = dartr_ast::AstMemStats::default();
        let mut total_source_bytes = 0usize;
        let mut unique_source_ptrs = HashSet::new();
        let mut unique_source_ptr_bytes = 0usize;
        let mut unique_source_contents: HashMap<&str, usize> = HashMap::new();
        let mut total_line_info_cap = 0usize;
        let mut unique_line_info_cap = 0usize;
        let mut total_unlinked_info_used = 0usize;
        let mut total_unlinked_info_cap = 0usize;
        let mut unique_unlinked_ptrs = HashSet::new();
        let mut unique_unlinked_info_cap = 0usize;
        let mut fs_maps_cap = 0usize;

        let mut total_store_refs = 0usize;
        let mut unique_store_ptrs = HashSet::new();
        let mut elem_count = 0usize;
        let mut elem_used = 0usize;
        let mut elem_cap = 0usize;
        let mut frag_count = 0usize;
        let mut frag_used = 0usize;
        let mut frag_cap = 0usize;
        let mut store_inline_bytes = 0usize;

        let mut unique_const_ptrs = HashSet::new();
        let mut const_ast = dartr_ast::AstMemStats::default();
        let const_node_info_cap = 0usize;

        let mut unique_lib_ptrs = HashSet::new();
        let mut export_entries_count = 0usize;
        let mut export_entries_cap = 0usize;
        let mut ref_count = 0usize;
        let mut ref_used = 0usize;
        let mut ref_cap = 0usize;

        let mut all_str_instances = 0usize;
        let mut all_str_bytes = 0usize;
        let mut unique_str_ptrs = HashSet::new();
        let mut unique_str_ptr_bytes = 0usize;
        let mut unique_str_text = HashSet::new();
        let mut unique_str_text_bytes = 0usize;
        let mut record_arc_str = |s: &Arc<str>| {
            all_str_instances += 1;
            all_str_bytes += s.len();
            if unique_str_ptrs.insert(Arc::as_ptr(s) as *const u8 as usize) {
                unique_str_ptr_bytes += s.len();
            }
            if unique_str_text.insert(s.as_ref().to_owned()) {
                unique_str_text_bytes += s.len();
            }
        };

        for driver in self.drivers.values() {
            total_file_states += driver.fs.files().len();
            fs_maps_cap +=
                driver.fs.files().len() * (size_of::<dartr_driver::file_state::FileState>() + 64);
            for f in driver.fs.files() {
                record_arc_str(&f.path);
                record_arc_str(&f.uri_str);
                if let Some(c) = &f.content {
                    total_file_contents += 1;
                    record_arc_str(&c.source);
                    let src_len = c.source.len();
                    total_source_bytes += src_len;
                    if unique_source_ptrs.insert(Arc::as_ptr(&c.source) as *const u8 as usize) {
                        unique_source_ptr_bytes += src_len;
                    }
                    unique_source_contents.insert(&c.source, src_len);

                    let ui_used = c.unlinked.line_starts.len() * 4
                        + c.unlinked
                            .top_level_declarations
                            .iter()
                            .map(|s| s.len() + size_of::<String>())
                            .sum::<usize>();
                    let ui_cap = c.unlinked.line_starts.capacity() * 4
                        + c.unlinked.top_level_declarations.capacity() * (size_of::<String>() + 8)
                        + c.unlinked
                            .top_level_declarations
                            .iter()
                            .map(|s| s.capacity())
                            .sum::<usize>();
                    total_unlinked_info_used += ui_used;
                    total_unlinked_info_cap += ui_cap;
                    if unique_unlinked_ptrs.insert(Arc::as_ptr(&c.unlinked) as usize) {
                        unique_unlinked_info_cap += ui_cap;
                    }

                    if let Some(parsed) = c.cached_parsed() {
                        let m = parsed.ast.mem_stats();
                        total_ast.add(&m);
                        let li_cap = parsed.line_info.line_starts.capacity() * 4;
                        total_line_info_cap += li_cap;
                        if unique_parsed_ptrs.insert(Arc::as_ptr(&parsed) as usize) {
                            unique_ast.add(&m);
                            unique_line_info_cap += li_cap;
                        }
                    }
                }
            }

            for store_opt in driver.state.world.stores.iter() {
                let Some(store) = store_opt else {
                    continue;
                };
                total_store_refs += 1;
                if unique_store_ptrs.insert(Arc::as_ptr(store) as usize) {
                    store_inline_bytes += size_of::<dartr_element::ElementStore>();
                    let (ec, eu, ek) = store.elements.mem_bytes();
                    let (fc, fu, fk) = store.fragments.mem_bytes();
                    elem_count += ec;
                    elem_used += eu;
                    elem_cap += ek;
                    frag_count += fc;
                    frag_used += fu;
                    frag_cap += fk;
                }
            }

            for ce in driver.state.registry.const_exprs.values() {
                if unique_const_ptrs.insert(Arc::as_ptr(ce) as usize) {
                    const_ast.add(&ce.ast.mem_stats());
                }
            }

            for lib in driver.state.registry.libraries.values() {
                if unique_lib_ptrs.insert(Arc::as_ptr(lib) as usize) {
                    export_entries_count += lib.export_entries.len();
                    export_entries_cap += lib.export_entries.capacity()
                        * size_of::<dartr_link::export::ExportEntry>();
                    for e in &lib.export_entries {
                        record_arc_str(&e.name);
                    }
                    let (rc, ru, rk) = lib.reference.mem_bytes();
                    ref_count += rc;
                    ref_used += ru;
                    ref_cap += rk;
                }
            }
        }

        let (
            gen_intern_count,
            gen_intern_used,
            gen_intern_cap,
            gen_name_count,
            gen_name_used,
            gen_name_cap,
            syn_used,
            syn_cap,
        ) = if let Some(g) = &self.generation {
            let (ic, iu, ik) = g.interner.mem_bytes();
            let (nc, nu, nk) = g.names.mem_bytes();
            let (_, seu, sek) = g.synthetic.elements.mem_bytes();
            let (_, sfu, sfk) = g.synthetic.fragments.mem_bytes();
            (
                ic,
                iu,
                ik,
                nc,
                nu,
                nk,
                seu + sfu,
                size_of::<dartr_element::ElementStore>() + sek + sfk,
            )
        } else {
            (0, 0, 0, 0, 0, 0, 0, 0)
        };

        let mut res_lib_count = 0usize;
        let mut res_unit_count = 0usize;
        let mut res_ast = dartr_ast::AstMemStats::default();
        let mut res_tables_entries = 0usize;
        let mut res_tables_used = 0usize;
        let mut res_tables_cap = 0usize;
        let mut rt_tables_entries = 0usize;
        let mut rt_tables_used = 0usize;
        let mut rt_tables_cap = 0usize;
        let mut res_local_used = 0usize;
        let mut res_local_cap = 0usize;
        for r in self.resolved.values() {
            res_lib_count += 1;
            for u in &r.library.units {
                res_unit_count += 1;
                res_ast.add(&u.ast.mem_stats());
                let (re, ru, rc) = u.tables.mem_bytes();
                let (rte, rtu, rtc) = u.rt.mem_bytes();
                let (_, eu, ec) = u.local.store.elements.mem_bytes();
                let (_, fu, fc) = u.local.store.fragments.mem_bytes();
                let (_, ou, oc) = u.local.types.mem_bytes();
                res_tables_entries += re;
                res_tables_used += ru;
                res_tables_cap += rc;
                rt_tables_entries += rte;
                rt_tables_used += rtu;
                rt_tables_cap += rtc;
                res_local_used += eu + fu + ou;
                res_local_cap += size_of::<dartr_element::LocalArena>() + ec + fc + oc;
            }
        }

        let unique_source_dedup_bytes: usize = unique_source_contents.values().sum();
        let mb = |b: usize| b as f64 / (1024.0 * 1024.0);
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "=== DARTR_MEM_STATS ({phase}) drivers={} (prio_libs={} known_cache={}) ===",
            self.drivers.len(),
            self.resolved.len(),
            self.known_libraries_cache.len(),
        );
        let _ = writeln!(
            out,
            "  1. FileSystemState & ParsedUnits: file_states={} contents={} unique_parsed={} fs_maps={:.2} MiB",
            total_file_states,
            total_file_contents,
            unique_parsed_ptrs.len(),
            mb(fs_maps_cap)
        );
        let _ = writeln!(
            out,
            "     - AST total (across drivers): nodes={} tokens={} used={:.2} MiB cap={:.2} MiB (inline={:.2} tokens={:.2}/{:.2} spine={:.2}/{:.2} stores={:.2}/{:.2})",
            total_ast.node_count,
            total_ast.token_count,
            mb(total_ast.total_used()),
            mb(total_ast.total_cap()),
            mb(total_ast.inline_bytes),
            mb(total_ast.tokens_used),
            mb(total_ast.tokens_cap),
            mb(total_ast.spine_used),
            mb(total_ast.spine_cap),
            mb(total_ast.stores_used),
            mb(total_ast.stores_cap),
        );
        let _ = writeln!(
            out,
            "     - AST unique (in memory):     nodes={} tokens={} used={:.2} MiB cap={:.2} MiB (inline={:.2} tokens={:.2}/{:.2} spine={:.2}/{:.2} stores={:.2}/{:.2})",
            unique_ast.node_count,
            unique_ast.token_count,
            mb(unique_ast.total_used()),
            mb(unique_ast.total_cap()),
            mb(unique_ast.inline_bytes),
            mb(unique_ast.tokens_used),
            mb(unique_ast.tokens_cap),
            mb(unique_ast.spine_used),
            mb(unique_ast.spine_cap),
            mb(unique_ast.stores_used),
            mb(unique_ast.stores_cap),
        );
        let _ = writeln!(
            out,
            "     - Source text: total={:.2} MiB unique_arc={:.2} MiB unique_content={:.2} MiB | LineInfo cap={:.2} MiB (unique {:.2} MiB) | Unlinked informative={:.2}/{:.2} MiB (unique {:.2} MiB)",
            mb(total_source_bytes),
            mb(unique_source_ptr_bytes),
            mb(unique_source_dedup_bytes),
            mb(total_line_info_cap),
            mb(unique_line_info_cap),
            mb(total_unlinked_info_used),
            mb(total_unlinked_info_cap),
            mb(unique_unlinked_info_cap),
        );
        let _ = writeln!(
            out,
            "  2. ElementStores & LinkedLibraries: store_refs={} unique_stores={} unique_libs={}",
            total_store_refs,
            unique_store_ptrs.len(),
            unique_lib_ptrs.len()
        );
        let _ = writeln!(
            out,
            "     - ElementStore inline={:.2} MiB | elements={} used={:.2} MiB boxcar_cap={:.2} MiB | fragments={} used={:.2} MiB boxcar_cap={:.2} MiB",
            mb(store_inline_bytes),
            elem_count,
            mb(elem_used),
            mb(elem_cap),
            frag_count,
            mb(frag_used),
            mb(frag_cap),
        );
        let _ = writeln!(
            out,
            "     - ConstExprs (unique={}): nodes={} tokens={} used={:.2} MiB cap={:.2} MiB (inline={:.2} stores={:.2}/{:.2}) node_info={:.2} MiB",
            unique_const_ptrs.len(),
            const_ast.node_count,
            const_ast.token_count,
            mb(const_ast.total_used()),
            mb(const_ast.total_cap()),
            mb(const_ast.inline_bytes),
            mb(const_ast.stores_used),
            mb(const_ast.stores_cap),
            mb(const_node_info_cap),
        );
        let _ = writeln!(
            out,
            "     - LinkedLibrary: export_entries={} ({:.2} MiB) | LibraryReference refs={} used={:.2} MiB cap={:.2} MiB",
            export_entries_count,
            mb(export_entries_cap),
            ref_count,
            mb(ref_used),
            mb(ref_cap),
        );
        let _ = writeln!(
            out,
            "  3. Generation (Interner & NamePool): interner_items={} used={:.2} MiB cap={:.2} MiB | names={} used={:.2} MiB cap={:.2} MiB | synthetic={:.2}/{:.2} MiB",
            gen_intern_count,
            mb(gen_intern_used),
            mb(gen_intern_cap),
            gen_name_count,
            mb(gen_name_used),
            mb(gen_name_cap),
            mb(syn_used),
            mb(syn_cap),
        );
        let _ = writeln!(
            out,
            "  4. Session Cached ResolvedLibraries: libs={} units={} total_cap={:.2} MiB (ast={:.2}/{:.2} MiB, res_tables={} entries {:.2}/{:.2} MiB, rt_tables={} entries {:.2}/{:.2} MiB, local_arena={:.2}/{:.2} MiB)",
            res_lib_count,
            res_unit_count,
            mb(res_ast.total_cap() + res_tables_cap + rt_tables_cap + res_local_cap),
            mb(res_ast.total_used()),
            mb(res_ast.total_cap()),
            res_tables_entries,
            mb(res_tables_used),
            mb(res_tables_cap),
            rt_tables_entries,
            mb(rt_tables_used),
            mb(rt_tables_cap),
            mb(res_local_used),
            mb(res_local_cap),
        );
        let _ = writeln!(
            out,
            "  5. Strings (Arc<str> sampled): instances={} total={:.2} MiB unique_arc={} ({:.2} MiB) unique_text={} ({:.2} MiB)",
            all_str_instances,
            mb(all_str_bytes),
            unique_str_ptrs.len(),
            mb(unique_str_ptr_bytes),
            unique_str_text.len(),
            mb(unique_str_text_bytes),
        );
        let accounted_unique_cap = unique_ast.total_cap()
            + unique_source_ptr_bytes
            + unique_line_info_cap
            + unique_unlinked_info_cap
            + fs_maps_cap
            + store_inline_bytes
            + elem_cap
            + frag_cap
            + const_ast.total_cap()
            + const_node_info_cap
            + export_entries_cap
            + ref_cap
            + gen_intern_cap
            + gen_name_cap
            + syn_cap
            + res_ast.total_cap()
            + res_tables_cap
            + rt_tables_cap
            + res_local_cap;
        let _ = writeln!(
            out,
            "  => TOTAL ACCOUNTED RETAINED CAPACITY: {:.2} MiB",
            mb(accounted_unique_cap)
        );
        if let Ok(mut err) = std::fs::OpenOptions::new().write(true).open("/dev/stderr") {
            let _ = std::io::Write::write_all(&mut err, out.as_bytes());
        }
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
        // Fast path: if `path`'s defining library is already cached in
        // `self.resolved`, return it without rebuilding `ContextOptions` or
        // walking `discover()`.
        let key_guess = (context, path.to_string());
        if let Some(r) = self.resolved.get(&key_guess) {
            return Some(r.clone());
        }
        if let Some((_, r)) = self
            .resolved
            .iter()
            .find(|((c, _), r)| *c == context && r.library.units.iter().any(|u| &*u.path == path))
        {
            return Some(r.clone());
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
        let priority_files = &self.priority_files;
        let result = pool().install(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let id = driver.fs.get_file_for_path(path);
                if driver.fs.file(id).content.is_none() {
                    driver.fs.discover();
                }
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
                let (mut library, mut inputs) = out.pop().flatten()?;
                compact_unit_inputs(&mut inputs);
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
                let is_priority = library
                    .units
                    .iter()
                    .any(|u| priority_files.contains(&*u.path));
                if !is_priority {
                    for u in &mut library.units {
                        u.ast.shrink_to_fit();
                        u.rt = Default::default();
                    }
                }
                let entry = Arc::new(ResolvedLibraryResult {
                    context,
                    world,
                    type_provider,
                    features,
                    library,
                    inputs,
                });
                if is_priority {
                    resolved.insert(key, entry.clone());
                }
                Some(entry)
            }))
        });
        match result {
            Ok(r) => {
                self.drivers.insert(context, driver);
                if std::env::var_os("DARTR_MEM_STATS").is_some() {
                    self.print_mem_stats("after_resolved_library");
                }
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

    /// Returns `(context_index, driver_files_len)` for the context of [path],
    /// if a driver exists for that context.
    pub fn context_files_count(
        &self,
        collection: &AnalysisContextCollection,
        path: &str,
    ) -> Option<(usize, usize)> {
        let context = collection
            .context_for(path)
            .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))?;
        let driver = self.drivers.get(&context)?;
        Some((context, driver.fs.files().len()))
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
        let out = driver
            .fs
            .files()
            .iter()
            .filter_map(|f| {
                f.content.as_ref()?;
                Some((f.path.clone(), f.parsed()))
            })
            .collect();
        driver.fs.clear_all_parsed();
        out
    }

    /// For all files known by the driver of the context of [path], returns
    /// `(unit_path, Some(parsed))` only when `has_cached(&unit_path)` is false
    /// and the file's source text contains `"{@template"`; otherwise returns
    /// `(unit_path, None)`.
    pub fn uncached_template_units(
        &self,
        collection: &AnalysisContextCollection,
        path: &str,
        has_cached: impl Fn(&str) -> bool,
    ) -> Vec<(Arc<str>, Option<Arc<dartr_ast_builder::ParsedUnit>>)> {
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
                let parsed = if has_cached(&f.path) || !content.source.contains("{@template") {
                    None
                } else {
                    Some(f.take_or_parse())
                };
                Some((f.path.clone(), parsed))
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
                driver.link_libraries_with_keep(&[library], false);
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
    /// Clears the cached `KnownLibrary` lists across all contexts (for example
    /// when a file is added or removed on disk).
    pub fn clear_known_libraries_cache(&mut self) {
        self.known_libraries_cache.clear();
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
        if self.discovered.contains(&context)
            && let Some((true, ordered)) = self.known_libraries_cache.get(&(context, None))
            && let Some(driver) = self.drivers.get(&context)
        {
            return Some((driver.state.world.clone(), (**ordered).clone()));
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
                        id: f.id,
                        should_link: true,
                        path: f.path.to_string(),
                        uri: f.uri_str.to_string(),
                        is_dart: f.uri_properties.is_dart,
                        is_dart_internal: f.uri_properties.is_dart_internal,
                        is_src: f.uri_properties.is_src,
                        package_name: f.uri_properties.package_name.clone(),
                    });
                }
                driver.link_libraries_with_keep(&ids, false);
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

    /// Links the known libraries in [context] that can be included by a
    /// completion request in [pkg_root] and returns the `WorldSnapshot`
    /// together with the known libraries sorted in `dart_known_order`, caching
    /// the sorted list across requests and re-linking only invalidated cycles
    /// when a file's non-directive API changes.
    pub fn link_known_libraries_ordered(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
        pkg_root: Option<String>,
        pkg_name: Option<String>,
        added_and_available: impl FnOnce(
            &AnalysisContextCollection,
            usize,
        ) -> (
            Vec<String>,
            Vec<String>,
            Option<std::collections::HashSet<String>>,
            bool,
        ),
    ) -> Option<(dartr_element::WorldSnapshot, Arc<Vec<KnownLibrary>>)> {
        if context >= collection.contexts.len() {
            return None;
        }
        let cache_key = (context, pkg_root.clone());
        if let Some((world_valid, ordered)) = self.known_libraries_cache.get(&cache_key) {
            let world_valid = *world_valid;
            let ordered = ordered.clone();
            if let Some(driver) = self.drivers.get_mut(&context) {
                if !world_valid {
                    let unlinked: Vec<FileId> = ordered
                        .iter()
                        .filter(|l| {
                            l.should_link
                                && !driver
                                    .graph
                                    .cycle_of_library
                                    .get(&l.id)
                                    .is_some_and(|c| driver.cycles.contains_key(c))
                        })
                        .map(|l| l.id)
                        .collect();
                    if !unlinked.is_empty() {
                        let ok = pool().install(|| {
                            catch_unwind(AssertUnwindSafe(|| {
                                driver.link_libraries(&unlinked);
                            }))
                        });
                        if ok.is_err() {
                            self.drivers.shift_remove(&context);
                            self.known_libraries_cache.retain(|(c, _), _| *c != context);
                            return None;
                        }
                    }
                    self.known_libraries_cache
                        .insert(cache_key, (true, ordered.clone()));
                }
                return Some((driver.state.world.clone(), ordered));
            }
        }
        let (added, available, allowed_pkgs, friend_of_analyzer) =
            added_and_available(collection, context);
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
            dartr_driver::project::context_driver(collection, context, generation.clone())
        });
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
                    let is_other_pkg_src = pkg_root.is_some()
                        && f.uri_properties.is_src
                        && f.uri_properties.package_name.as_deref() != pkg_name.as_deref()
                        && !(friend_of_analyzer
                            && f.uri_properties.package_name.as_deref() == Some("analyzer"));
                    let is_disallowed_pkg = allowed_pkgs.as_ref().is_some_and(|allowed| {
                        f.uri_properties
                            .package_name
                            .as_ref()
                            .is_some_and(|p| !allowed.contains(p))
                    });
                    let should_link = !is_other_pkg_src && !is_disallowed_pkg;
                    if should_link
                        && !driver
                            .graph
                            .cycle_of_library
                            .get(&f.id)
                            .is_some_and(|c| driver.cycles.contains_key(c))
                    {
                        ids.push(f.id);
                    }
                    libraries.push(KnownLibrary {
                        id: f.id,
                        should_link,
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
        let (world, mut libraries) = match result {
            Ok(r) => {
                self.drivers.insert(context, driver);
                r
            }
            Err(_) => return None,
        };
        let order = self.dart_known_order(context, &added, &available);
        let rank: HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(i, p)| (p.as_str(), i))
            .collect();
        libraries.sort_by_key(|l| rank.get(l.path.as_str()).copied().unwrap_or(usize::MAX));
        let ordered = Arc::new(libraries);
        if self.drivers.contains_key(&context) {
            self.known_libraries_cache
                .insert(cache_key, (true, ordered.clone()));
        }
        Some((world, ordered))
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
        let external = dartr_resolver::library_analyzer::ExternalUnitCache::new_lazy(
            world,
            &tp,
            dartr_resolver::options::AnalysisOptions::default(),
            driver.lazy_unit_sources(),
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

    /// Returns the parsed unit sources for [context], for constructing an
    /// `ExternalUnitCache` when evaluating constants of AST expressions.
    pub fn unit_sources(
        &self,
        context: usize,
    ) -> IndexMap<Arc<str>, (Arc<str>, Arc<dartr_ast_builder::ParsedUnit>)> {
        self.drivers
            .get(&context)
            .map(|d| {
                let out = d.unit_sources();
                d.fs.clear_all_parsed();
                out
            })
            .unwrap_or_default()
    }

    /// Ensures the library at [path] is linked in the driver of [context] and
    /// invalidates cached resolved libraries for [context] so they see the new
    /// `WorldSnapshot`.
    pub fn ensure_library_linked_in(
        &mut self,
        collection: &AnalysisContextCollection,
        context: usize,
        path: &str,
    ) {
        let before_len = self.drivers.get(&context).map(|d| d.fs.files().len());
        if self.linked_library_in(collection, context, path).is_some() {
            self.resolved.retain(|(c, _), _| *c != context);
            let after_len = self.drivers.get(&context).map(|d| d.fs.files().len());
            if before_len.is_some() && before_len == after_len {
                for ((c, _), (world_valid, _)) in &mut self.known_libraries_cache {
                    if *c == context {
                        *world_valid = false;
                    }
                }
            } else {
                self.known_libraries_cache.retain(|(c, _), _| *c != context);
            }
        }
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
            fs.discover_doc_imports(&[core]);
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
        if let Some(g) = &self.generation {
            dartr_driver::driver::invalidate_cached_file(g, path);
        }
        let mut result: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (&context, driver) in self.drivers.iter_mut() {
            let before = file_directive_state(driver, path);
            driver.fs.invalidate_cached_file(path);
            if driver.fs.get_existing_from_path(path).is_none() {
                continue;
            }
            let change = driver.change_file(path);
            let after = file_directive_state(driver, path);
            if before.is_none() || before != after {
                self.known_libraries_cache.retain(|(c, _), _| *c != context);
            } else if change.api_changed {
                for ((c, _), (world_valid, _)) in &mut self.known_libraries_cache {
                    if *c == context {
                        *world_valid = false;
                    }
                }
            }
            for library in change.libraries {
                // `change.libraries` already contains the changed library and,
                // when `api_changed` is true, every transitively affected
                // library in this driver. Evict only those libraries.
                let lib_key = (context, driver.fs.file(library).path.to_string());
                self.resolved.remove(&lib_key);
                for unit in driver.fs.library_file_kinds(library) {
                    let p = driver.fs.file(unit).path.to_string();
                    if seen.insert(p.clone()) {
                        result.push(p);
                    }
                }
            }
        }
        result
    }
}

#[derive(PartialEq, Eq)]
struct FileDirectiveState {
    exists: bool,
    kind: dartr_driver::file_state::FileKind,
    imports: Vec<dartr_driver::file_state::DirectiveUris>,
    exports: Vec<dartr_driver::file_state::DirectiveUris>,
    parts: Vec<dartr_driver::file_state::DirectiveUris>,
    doc_imports: Vec<dartr_driver::file_state::DirectiveUris>,
}

fn file_directive_state(driver: &Driver, path: &str) -> Option<FileDirectiveState> {
    let id = driver.fs.get_existing_from_path(path)?;
    let c = driver.fs.file(id).content.as_ref()?;
    Some(FileDirectiveState {
        exists: c.exists,
        kind: c.kind.clone(),
        imports: c.library_imports.iter().map(|d| d.uris.clone()).collect(),
        exports: c.library_exports.iter().map(|d| d.uris.clone()).collect(),
        parts: c.part_includes.iter().map(|d| d.uris.clone()).collect(),
        doc_imports: c
            .doc_library_imports
            .iter()
            .map(|d| d.uris.clone())
            .collect(),
    })
}

/// Returns whether the parsed AST of `parsed` has any top-level class, mixin,
/// enum, extension type, or class type alias whose `extends`, `with`,
/// `implements`, or `on` clause names one of `names` (or has an implicit
/// `Object` supertype / constraint when `"Object"` is in `names`).
fn unit_has_subtype_of(parsed: &dartr_ast_builder::ParsedUnit, names: &[String]) -> bool {
    use dartr_ast::*;
    let ast = &parsed.ast;
    let matches_named = |t: Id<NamedType>| {
        let lex = ast.tokens.lexeme(ast[t].name);
        names.iter().any(|n| n == lex)
    };
    let matches_implements = |imp: Option<Id<ImplementsClause>>| {
        imp.is_some_and(|i| {
            ast.list(ast[i].interfaces)
                .iter()
                .copied()
                .any(matches_named)
        })
    };
    let matches_with = |w: Option<Id<WithClause>>| {
        w.is_some_and(|w| {
            ast.list(ast[w].mixin_types)
                .iter()
                .copied()
                .any(matches_named)
        })
    };
    let has_object = names.iter().any(|n| n == "Object");
    for &d in ast.list_raw(ast[parsed.unit].declarations) {
        if let Some(c) = ast.cast::<ClassDeclaration>(d) {
            let ext = ast[c].extends_clause;
            if ext.is_some_and(|e| matches_named(ast[e].superclass))
                || (ext.is_none() && has_object)
                || matches_with(ast[c].with_clause)
                || matches_implements(ast[c].implements_clause)
            {
                return true;
            }
        } else if let Some(c) = ast.cast::<ClassTypeAlias>(d) {
            if matches_named(ast[c].superclass)
                || matches_with(Some(ast[c].with_clause))
                || matches_implements(ast[c].implements_clause)
            {
                return true;
            }
        } else if let Some(m) = ast.cast::<MixinDeclaration>(d) {
            let on = ast[m].on_clause;
            if on.is_some_and(|o| {
                ast.list(ast[o].superclass_constraints)
                    .iter()
                    .copied()
                    .any(matches_named)
            }) || (on.is_none() && has_object)
                || matches_implements(ast[m].implements_clause)
            {
                return true;
            }
        } else if let Some(e) = ast.cast::<EnumDeclaration>(d) {
            if matches_with(ast[e].with_clause) || matches_implements(ast[e].implements_clause) {
                return true;
            }
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(d) {
            if matches_implements(ast[e].implements_clause) {
                return true;
            }
        }
    }
    false
}

/// Analyzes the requested [files] of one context with its [driver].
fn analyze_context<I, F>(
    context: usize,
    driver: &mut Driver,
    options: &ContextOptions,
    files: &[AnalyzedFile],
    priority_files: &std::collections::HashSet<String>,
    index_fn: Option<&F>,
) -> (
    Vec<Outcome>,
    Vec<((usize, String), Arc<ResolvedLibraryResult>)>,
    Vec<((usize, String), I)>,
    (
        std::time::Duration,
        std::time::Duration,
        std::time::Duration,
    ),
)
where
    I: Send,
    F: Fn(&dartr_element::Ctx<'_>, &dartr_resolver::library_analyzer::ResolvedUnit) -> I + Sync,
{
    let t0 = std::time::Instant::now();
    let ids: Vec<FileId> = files
        .iter()
        .map(|f| driver.fs.get_file_for_path(&f.path))
        .collect();
    driver.fs.discover();
    let t_discover = t0.elapsed();

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
    let t_before_link = t0.elapsed();
    driver.link_libraries(&library_ids);
    let t_link = t0.elapsed() - t_before_link;

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
    let mut pkg_cache = dartr_lints::LintPackageCache::default();
    let pkg_infos: HashMap<FileId, Arc<dartr_lints::LintPackageInfo>> = library_ids
        .iter()
        .map(|&l| {
            let path = &driver.fs.file(l).path;
            (l, pkg_cache.for_defining_path(path))
        })
        .collect();
    let world_and_tp = if index_fn.is_some() || !priority_files.is_empty() {
        let world = driver.state.world.clone();
        let tp = dartr_link::types_builder::world_type_provider(&world);
        Some((world, tp))
    } else {
        None
    };
    let requested = &requested;
    let libraries = &libraries;
    let pkg_infos = &pkg_infos;
    let world_and_tp = world_and_tp.as_ref();
    let t_before_resolve = t0.elapsed();
    let analyzed = driver.analyze_libraries_with_lints(&jobs, |library, units, result, lints| {
        library_outcomes(
            context,
            options,
            &libraries[&library],
            requested,
            units,
            result,
            lints,
            pkg_infos.get(&library).map(|p| &**p),
            priority_files,
            world_and_tp,
            index_fn,
        )
    });
    let t_resolve = t0.elapsed() - t_before_resolve;
    let mut priority_resolved = Vec::new();
    let mut indexed = Vec::new();
    for (lib_outcomes, prio_lib, unit_indexes) in analyzed {
        outcomes.extend(lib_outcomes);
        if let Some(p) = prio_lib {
            priority_resolved.push(p);
        }
        indexed.extend(unit_indexes);
    }
    (
        outcomes,
        priority_resolved,
        indexed,
        (t_discover, t_link, t_resolve),
    )
}

/// The diagnostics of the [wanted] units of one analyzed library, plus
/// optional cached `ResolvedLibraryResult` (when a unit is in `priority_files`)
/// and per-unit index entries.
#[allow(clippy::too_many_arguments)]
fn library_outcomes<I, F>(
    context: usize,
    options: &ContextOptions,
    wanted: &[FileId],
    requested: &HashMap<FileId, &AnalyzedFile>,
    units: &[UnitInput],
    result: Result<ResolvedLibrary, String>,
    resolved_lints: Vec<Vec<Diagnostic>>,
    package_info: Option<&dartr_lints::LintPackageInfo>,
    priority_files: &std::collections::HashSet<String>,
    world_and_tp: Option<&(dartr_element::WorldSnapshot, dartr_element::TypeProvider)>,
    index_fn: Option<&F>,
) -> (
    Vec<Outcome>,
    Option<((usize, String), Arc<ResolvedLibraryResult>)>,
    Vec<((usize, String), I)>,
)
where
    I: Send,
    F: Fn(&dartr_element::Ctx<'_>, &dartr_resolver::library_analyzer::ResolvedUnit) -> I + Sync,
{
    let fallback = || {
        (
            wanted
                .iter()
                .map(|id| Outcome::Fallback(requested[id].clone()))
                .collect(),
            None,
            Vec::new(),
        )
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
    let keep_priority =
        !priority_files.is_empty() && units.iter().any(|u| priority_files.contains(&*u.path));
    let mut unit_indexes = Vec::new();
    let mut features_opt = None;
    if let Some((world, tp)) = world_and_tp
        && (keep_priority || index_fn.is_some())
    {
        let sink = dartr_element::NoopSink;
        let default_features = dartr_element::FeatureSet::default();
        let base_ctx = dartr_element::Ctx {
            world,
            current: None,
            local: None,
            tp,
            features: &default_features,
            req: &sink,
        };
        let features = base_ctx.get(resolved.library).feature_set.clone();
        if let Some(f) = index_fn {
            unit_indexes.reserve(resolved.units.len());
            for unit in &resolved.units {
                let unit_ctx = dartr_element::Ctx {
                    world,
                    current: None,
                    local: Some(&unit.local),
                    tp,
                    features: &features,
                    req: &sink,
                };
                unit_indexes.push(((context, unit.path.to_string()), f(&unit_ctx, unit)));
            }
        }
        features_opt = Some(features);
    }
    let mut non_prio_unit_diags: Vec<(Option<String>, Vec<Diagnostic>)> = if !keep_priority {
        resolved
            .units
            .iter_mut()
            .map(|u| {
                let mut d = std::mem::take(&mut u.diagnostics);
                d.extend(std::mem::take(&mut u.ignored_diagnostics));
                (u.panic.take(), d)
            })
            .collect()
    } else {
        Vec::new()
    };
    let priority_entry = if keep_priority {
        if let Some((world, _)) = world_and_tp
            && let Some(features) = features_opt
        {
            let key = (context, first.path.to_string());
            let mut inputs = units.to_vec();
            compact_unit_inputs(&mut inputs);
            let entry = Arc::new(ResolvedLibraryResult {
                context,
                world: world.clone(),
                type_provider: dartr_link::types_builder::world_type_provider(world),
                features,
                library: resolved,
                inputs,
            });
            Some((key, entry))
        } else {
            None
        }
    } else {
        drop(resolved);
        None
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
                source_uri: Some(&u.uri),
                package_info,
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
        let diags = if let Some((_, prio)) = &priority_entry {
            let unit = &prio.library.units[index];
            if let Some(panic) = &unit.panic
                && debug()
            {
                eprintln!("dartr: resolver panic in {}: {panic}", unit.path);
            }
            [unit.diagnostics.clone(), unit.ignored_diagnostics.clone()].concat()
        } else if let Some((panic, diags)) = non_prio_unit_diags.get_mut(index) {
            if let Some(panic) = panic
                && debug()
            {
                eprintln!("dartr: resolver panic in {}: {panic}", units[index].path);
            }
            std::mem::take(diags)
        } else {
            Vec::new()
        };
        let parsed = &units[index].parsed;
        let (settings, _) = options.for_path(&file.path);
        outcomes.push(Outcome::Done(finish_file(
            &file.path,
            &parsed.ast.tokens.source,
            parsed,
            diags,
            lints.get(index).cloned().unwrap_or_default(),
            settings,
        )));
    }
    (outcomes, priority_entry, unit_indexes)
}

fn compact_unit_inputs(inputs: &mut [UnitInput]) {
    for u in inputs {
        u.directive_diagnostics = Vec::new();
        u.resolved_uris = dartr_ast::NodeMap::default();
        if let Some(p) = Arc::get_mut(&mut u.parsed) {
            p.ast = dartr_ast::Ast::new(p.ast.tokens.clone());
            p.invalid_nodes = Vec::new();
            p.dot_shorthands = Vec::new();
        } else {
            let p = &*u.parsed;
            u.parsed = Arc::new(dartr_ast_builder::ParsedUnit {
                ast: dartr_ast::Ast::new(p.ast.tokens.clone()),
                unit: p.unit,
                diagnostics: p.diagnostics.clone(),
                line_info: p.line_info.clone(),
                language_version: p.language_version,
                feature_set: p.feature_set.clone(),
                invalid_nodes: Vec::new(),
                dot_shorthands: Vec::new(),
            });
        }
    }
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
    fn evaluate_scc(&mut self, fs: &mut dartr_driver::file_state::FileSystemState, scc: &[FileId]) {
        fs.discover_doc_imports(scc);
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
    fn walk(&mut self, fs: &mut dartr_driver::file_state::FileSystemState, start: FileId) {
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
            fs: &mut dartr_driver::file_state::FileSystemState,
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
