// Dart source: pkg/analyzer/lib/src/dart/analysis/library_context.dart
// (LibraryContext.load: link the cycles of a library in dependency order),
// design docs/design/semantics.md §2.5 (parallel linking of independent
// cycles)

//! [`Driver`]: the in-memory "driver lite" of unit B6. It owns the file
//! state of one analysis context, computes library cycles, builds the link
//! inputs and links the cycles that a set of libraries needs, in
//! dependency order: a cycle starts as soon as all its direct dependencies
//! are linked (a DAG scheduler on rayon), and each linked cycle is
//! published into the [`WorldSnapshot`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};

use dartr_element::{Generation, StoreId, WorldSnapshot};
use dartr_link::input::{
    LinkCombinator, LinkDirectiveUri, LinkExport, LinkImport, LinkImportPrefix, LinkLibraryInput,
    LinkPart, LinkPartUri, LinkUnitInput,
};
use dartr_link::link::{ConstExprs, LinkedCycle, LinkedLibraries, LinkedLibrary, link_cycle};
use indexmap::{IndexMap, IndexSet};

use crate::file_state::{DirectiveUri, FileId, FileSystemState, RefreshData, SdkFileKey};
use crate::library_graph::{CycleId, LibraryGraph};
use crate::unlinked_data::UnlinkedCombinator;

/// Cache key for a linked library cycle within a [`Generation`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct SdkCycleKey {
    pub sdk_path: String,
    pub api_signature: String,
    pub manifest_signature: String,
    pub non_transitive_api_signature: String,
    pub unlinked_signatures: Vec<[u8; 16]>,
    pub dependency_stores: Vec<StoreId>,
}

/// Shared cache of parsed files and linked library cycles
/// across all [`Driver`]s that share the same [`Generation`].
#[derive(Default)]
pub(crate) struct SdkCache {
    files: RwLock<HashMap<SdkFileKey, Arc<RefreshData>>>,
    cycles: RwLock<HashMap<SdkCycleKey, (Arc<str>, Arc<LinkedCycle>)>>,
}

impl SdkCache {
    pub(crate) fn for_generation(generation: &Arc<Generation>) -> Arc<SdkCache> {
        static TABLE: LazyLock<Mutex<Vec<(Weak<Generation>, Arc<SdkCache>)>>> =
            LazyLock::new(|| Mutex::new(Vec::new()));
        let mut table = TABLE.lock().unwrap();
        table.retain(|(weak, _)| weak.strong_count() > 0);
        for (weak, cache) in table.iter() {
            if let Some(existing) = weak.upgrade()
                && Arc::ptr_eq(&existing, generation)
            {
                return cache.clone();
            }
        }
        let cache = Arc::new(SdkCache::default());
        table.push((Arc::downgrade(generation), cache.clone()));
        cache
    }

    pub(crate) fn get_file(&self, key: &SdkFileKey) -> Option<Arc<RefreshData>> {
        self.files.read().unwrap().get(key).cloned()
    }

    pub(crate) fn insert_file(&self, key: SdkFileKey, data: Arc<RefreshData>) {
        self.files.write().unwrap().insert(key, data);
    }

    pub(crate) fn invalidate_file(&self, path: &str) {
        self.files.write().unwrap().retain(|k, _| &*k.path != path);
    }

    pub(crate) fn get_cycle(&self, key: &SdkCycleKey) -> Option<Arc<LinkedCycle>> {
        self.cycles
            .read()
            .unwrap()
            .get(key)
            .map(|(_, cycle)| cycle.clone())
    }

    pub(crate) fn insert_cycle(
        &self,
        key: SdkCycleKey,
        first_path: Arc<str>,
        cycle: Arc<LinkedCycle>,
    ) {
        self.cycles
            .write()
            .unwrap()
            .insert(key, (first_path, cycle));
    }

    pub(crate) fn invalidate_store(&self, store: StoreId) {
        self.cycles
            .write()
            .unwrap()
            .retain(|k, (_, v)| v.store.id != store && !k.dependency_stores.contains(&store));
    }

    pub(crate) fn retain_for_roots(&self, roots: &[String]) {
        let in_roots = |path: &str| {
            roots
                .iter()
                .any(|r| dartr_project::paths::is_within(r, path))
        };
        self.files.write().unwrap().retain(|k, _| in_roots(&k.path));
        let mut cycles = self.cycles.write().unwrap();
        cycles.retain(|_, (path, _)| in_roots(path));
        loop {
            let live: std::collections::HashSet<StoreId> =
                cycles.values().map(|(_, v)| v.store.id).collect();
            let before = cycles.len();
            cycles.retain(|k, _| k.dependency_stores.iter().all(|d| live.contains(d)));
            if cycles.len() == before {
                break;
            }
        }
    }

    pub(crate) fn clear(&self) {
        self.files.write().unwrap().clear();
        self.cycles.write().unwrap().clear();
    }

    pub(crate) fn clear_files(&self) {
        self.files.write().unwrap().clear();
    }

    pub(crate) fn drop_all_parsed(&self) {
        let mut files = self.files.write().unwrap();
        for data in files.values_mut() {
            if data.parsed.is_some() {
                let mut cloned = (**data).clone();
                cloned.parsed = None;
                *data = Arc::new(cloned);
            }
        }
    }
}

/// Invalidates cached parsed file data for [path] in [generation].
pub fn invalidate_cached_file(generation: &Arc<Generation>, path: &str) {
    SdkCache::for_generation(generation).invalidate_file(path);
}

/// Drops any cached [` dartr_ast_builder::ParsedUnit`] ASTs in [generation]'s shared cache.
pub fn drop_cached_parsed(generation: &Arc<Generation>) {
    SdkCache::for_generation(generation).drop_all_parsed();
}

/// The libraries linked so far, by URI (lookups only).
#[derive(Clone, Default)]
pub struct LinkedRegistry {
    pub libraries: imbl::HashMap<Arc<str>, Arc<LinkedLibrary>>,
    /// The `ConstExprs` of each linked cycle, by raw store id.
    pub const_exprs: imbl::HashMap<u32, Arc<ConstExprs>>,
}

impl LinkedLibraries for LinkedRegistry {
    fn library(&self, uri: &str) -> Option<Arc<LinkedLibrary>> {
        self.libraries.get(uri).cloned()
    }

    fn const_exprs(&self, store: dartr_element::StoreId) -> Option<Arc<ConstExprs>> {
        self.const_exprs.get(&store.raw()).cloned()
    }
}

/// The world and the linked libraries, updated as cycles are linked.
#[derive(Clone)]
pub struct LinkState {
    pub world: WorldSnapshot,
    pub registry: LinkedRegistry,
}

/// The result of [`Driver::change_file`].
#[derive(Debug, Default)]
pub struct FileChange {
    /// Whether the API of the file changed (cycles were removed).
    pub api_changed: bool,
    /// The libraries to analyze again (defining units). After an API
    /// change they must be linked first ([`Driver::link_libraries`]).
    pub libraries: Vec<FileId>,
}

/// Dart `AnalysisDriver` (the linking part only).
pub struct Driver {
    pub fs: FileSystemState,
    pub graph: LibraryGraph,
    pub state: LinkState,
    pub cycles: IndexMap<CycleId, Arc<LinkedCycle>>,
    sdk_cache: Arc<SdkCache>,
    pub clear_sdk_cache_after_lookup: bool,
}

impl Driver {
    pub fn new(mut fs: FileSystemState, generation: Arc<Generation>) -> Driver {
        let sdk_cache = SdkCache::for_generation(&generation);
        fs.sdk_cache = Some(sdk_cache.clone());
        Driver {
            fs,
            graph: LibraryGraph::default(),
            state: LinkState {
                world: WorldSnapshot::new(generation),
                registry: LinkedRegistry::default(),
            },
            cycles: IndexMap::new(),
            sdk_cache,
            clear_sdk_cache_after_lookup: false,
        }
    }

    /// Evicts cached SDK/package files and cycles that do not reside under any
    /// folder in [roots].
    pub fn retain_sdk_cache_for_roots(&self, roots: &[String]) {
        self.sdk_cache.retain_for_roots(roots);
    }

    /// Computes the [`SdkCycleKey`] of a cycle whose direct dependencies
    /// are already linked in [`Self::cycles`].
    fn sdk_cycle_key(&self, c: CycleId) -> SdkCycleKey {
        let sdk_path = self
            .fs
            .source_factory
            .sdk
            .as_ref()
            .map(|s| s.path().to_string())
            .unwrap_or_default();
        let cycle = self.graph.cycle(c);
        let mut unlinked_signatures = Vec::new();
        for &l in &cycle.libraries {
            for f in self.fs.library_files(l) {
                unlinked_signatures.push(self.fs.file(f).c().unlinked_signature);
            }
        }
        let dependency_stores = cycle
            .direct_dependencies
            .iter()
            .map(|d| self.cycles[d].store.id)
            .collect();
        SdkCycleKey {
            sdk_path,
            api_signature: cycle.api_signature.clone(),
            manifest_signature: cycle.manifest_signature.clone(),
            non_transitive_api_signature: cycle.non_transitive_api_signature.clone(),
            unlinked_signatures,
            dependency_stores,
        }
    }

    fn publish_linked_cycle(&mut self, c: CycleId, linked: Arc<LinkedCycle>) {
        let libraries = linked.libraries.iter().map(|l| (l.uri.clone(), l.element));
        self.state.world.add_store(linked.store.clone(), libraries);
        for l in &linked.libraries {
            self.state
                .registry
                .libraries
                .insert(l.uri.clone(), l.clone());
        }
        self.state
            .registry
            .const_exprs
            .insert(linked.store.id.raw(), linked.const_exprs.clone());
        self.cycles.insert(c, linked);
    }

    fn finish_link_parsed(&self, libraries: &[FileId], keep_targets: bool) {
        if keep_targets {
            let keep: std::collections::HashSet<FileId> = libraries
                .iter()
                .flat_map(|&l| self.fs.library_file_kinds(l))
                .collect();
            self.fs.drop_parsed_except_ids(&keep);
        } else {
            self.fs.clear_all_parsed();
        }
        if crate::analysis::keep_parsed_in_sdk_cache() {
            self.sdk_cache.drop_all_parsed();
        }
    }

    /// Links the cycles of [libraries] and of everything they depend on.
    /// The files must be discovered ([`FileSystemState::discover`]).
    pub fn link_libraries(&mut self, libraries: &[FileId]) {
        self.link_libraries_with_keep(libraries, crate::analysis::keep_parsed_in_link());
    }

    /// Like [`Self::link_libraries`], and drops the cached [`ParsedUnit`] of
    /// target [libraries] as well when [keep_targets] is `false`.
    pub fn link_libraries_with_keep(&mut self, libraries: &[FileId], keep_targets: bool) {
        self.fs.discover_doc_imports(libraries);
        // Library graph and cycles: sequential.
        let mut targets = Vec::new();
        for &library in libraries {
            targets.push(self.graph.library_cycle(&self.fs, library));
            // Dart `AnalysisDriver._analyzeFileImpl` also loads the
            // `@docImport` libraries of the library it analyzes.
            if self.fs.file(library).kind().is_library() {
                for doc_import in self.doc_import_files(library) {
                    targets.push(self.graph.library_cycle(&self.fs, doc_import));
                }
            }
        }
        // The cycles to link, dependencies first.
        let mut order: IndexSet<CycleId> = IndexSet::new();
        fn visit(
            graph: &LibraryGraph,
            linked: &IndexMap<CycleId, Arc<LinkedCycle>>,
            c: CycleId,
            order: &mut IndexSet<CycleId>,
        ) {
            if linked.contains_key(&c) || order.contains(&c) {
                return;
            }
            for &d in &graph.cycle(c).direct_dependencies {
                visit(graph, linked, d, order);
            }
            order.insert(c);
        }
        for &t in &targets {
            visit(&self.graph, &self.cycles, t, &mut order);
        }
        if order.is_empty() {
            self.finish_link_parsed(libraries, keep_targets);
            return;
        }
        let mut remaining_order: IndexSet<CycleId> = IndexSet::new();
        for &c in &order {
            if self
                .graph
                .cycle(c)
                .direct_dependencies
                .iter()
                .all(|d| self.cycles.contains_key(d))
            {
                let key = self.sdk_cycle_key(c);
                if let Some(linked) = self.sdk_cache.get_cycle(&key) {
                    self.publish_linked_cycle(c, linked);
                    continue;
                }
            }
            remaining_order.insert(c);
        }
        let order = remaining_order;
        if self.clear_sdk_cache_after_lookup {
            self.sdk_cache.clear();
        }
        if order.is_empty() {
            self.finish_link_parsed(libraries, keep_targets);
            return;
        }
        let keep_files: std::collections::HashSet<FileId> = if keep_targets {
            libraries
                .iter()
                .flat_map(|&l| self.fs.library_file_kinds(l))
                .collect()
        } else {
            std::collections::HashSet::new()
        };
        let mut needed_or_keep = keep_files.clone();
        for &c in &order {
            for &l in &self.graph.cycle(c).libraries {
                needed_or_keep.extend(self.fs.library_file_kinds(l));
            }
        }
        self.fs.drop_parsed_except_ids(&needed_or_keep);
        let position: IndexMap<CycleId, usize> =
            order.iter().enumerate().map(|(i, &c)| (c, i)).collect();
        // Dependencies that are not linked yet, and the users of each cycle.
        let mut remaining: Vec<AtomicUsize> = Vec::new();
        let mut users: Vec<Vec<usize>> = vec![Vec::new(); order.len()];
        for (i, &c) in order.iter().enumerate() {
            let mut count = 0;
            for d in &self.graph.cycle(c).direct_dependencies {
                if let Some(&p) = position.get(d) {
                    count += 1;
                    users[p].push(i);
                }
            }
            remaining.push(AtomicUsize::new(count));
        }
        let generation = self.state.world.generation.clone();
        let prev_state = std::mem::replace(
            &mut self.state,
            LinkState {
                world: WorldSnapshot::new(generation),
                registry: LinkedRegistry::default(),
            },
        );
        let shared = Mutex::new((prev_state, vec![None; order.len()]));
        let ready: Vec<usize> = (0..order.len())
            .filter(|&i| remaining[i].load(Ordering::Relaxed) == 0)
            .collect();
        let max_workers = crate::analysis::max_parallel_libraries()
            .min(rayon::current_num_threads().max(1))
            .min(order.len())
            .max(1);
        let initial_workers = max_workers.min(ready.len());
        let sched = Mutex::new((std::collections::VecDeque::from(ready), initial_workers));
        let job = Job {
            driver: self,
            order: &order,
            keep_files: &keep_files,
            remaining: &remaining,
            users: &users,
            shared: &shared,
            sched: &sched,
            max_workers,
        };
        rayon::scope(|scope| {
            for _ in 0..initial_workers {
                job.spawn_worker(scope);
            }
        });
        let (state, results) = shared.into_inner().unwrap();
        self.state = state;
        for (i, result) in results.into_iter().enumerate() {
            let c = order[i];
            let linked = result.expect("cycle linked");
            self.cycles.insert(c, linked.clone());
            if !self.clear_sdk_cache_after_lookup {
                let cycle_libs = &self.graph.cycle(c).libraries;
                let is_shared = cycle_libs.iter().all(|&l| {
                    let p = &self.fs.file(l).uri_properties;
                    p.is_dart || p.package_name.is_some()
                });
                if is_shared && let Some(&first_lib) = cycle_libs.first() {
                    let first_path = self.fs.file(first_lib).path.clone();
                    let key = self.sdk_cycle_key(c);
                    self.sdk_cache.insert_cycle(key, first_path, linked);
                }
            }
        }
        self.finish_link_parsed(libraries, keep_targets);
    }

    /// Dart `AnalysisDriver.changeFile` with the v1 invalidation of design
    /// §4.2: reads [path] again. If only function bodies changed, every
    /// linked cycle is kept and only the library of the file must be
    /// analyzed again. If the API changed (declarations, directives, the
    /// file was created or deleted), the cycles of the libraries that
    /// contain or reference the file and all their transitive users are
    /// removed (Dart `LibraryCycle.dispose` through `directUsers`); the
    /// next [`Driver::link_libraries`] links them again, and unaffected
    /// cycles are reused.
    pub fn change_file(&mut self, path: &str) -> FileChange {
        let Some(file) = self.fs.get_existing_from_path(path) else {
            let file = self.fs.get_file_for_path(path);
            self.fs.discover();
            let libraries = self.fs.library_of(file).into_iter().collect();
            return FileChange {
                api_changed: true,
                libraries,
            };
        };
        let mut seeds: IndexSet<FileId> = IndexSet::new();
        self.change_seeds(file, &mut seeds);
        let api_changed = self.fs.change_file(file);
        if !api_changed {
            let libraries = self.fs.library_of(file).into_iter().collect();
            return FileChange {
                api_changed,
                libraries,
            };
        }
        self.change_seeds(file, &mut seeds);
        // The cycles of the seeds and their transitive users. Cycles that
        // were invalidated before stay in `graph.cycles` (ids are indices)
        // but no library points to them any more; skip them.
        let mut invalid: IndexSet<CycleId> = IndexSet::new();
        let mut stack: Vec<CycleId> = seeds
            .iter()
            .filter_map(|l| self.graph.cycle_of_library.get(l).copied())
            .collect();
        while let Some(c) = stack.pop() {
            if !self.is_live(c) || !invalid.insert(c) {
                continue;
            }
            stack.extend(self.graph.cycle(c).direct_users.iter().copied());
        }
        let mut libraries: IndexSet<FileId> = IndexSet::new();
        for &c in &invalid {
            libraries.extend(self.graph.cycle(c).libraries.iter().copied());
        }
        for l in &libraries {
            self.graph.cycle_of_library.shift_remove(l);
        }
        for c in &invalid {
            if let Some(linked) = self.cycles.shift_remove(c) {
                let store_id = linked.store.id;
                self.state
                    .world
                    .remove_store(store_id, linked.libraries.iter().map(|l| &l.uri));
                self.state.registry.const_exprs.remove(&store_id.raw());
                for l in &linked.libraries {
                    self.state.registry.libraries.remove(&l.uri);
                }
                self.sdk_cache.invalidate_store(store_id);
            }
        }
        libraries.extend(seeds);
        let libraries = libraries
            .into_iter()
            .filter(|&l| self.fs.file(l).content.is_some() && self.fs.file(l).kind().is_library())
            .collect();
        FileChange {
            api_changed,
            libraries,
        }
    }

    /// The library of [file] (or the file) and the libraries of the files
    /// that transitively reference it (Dart `FileSystemState.collectAffected`).
    fn change_seeds(&mut self, file: FileId, seeds: &mut IndexSet<FileId>) {
        let mut affected: IndexSet<FileId> = IndexSet::new();
        let mut stack = vec![file];
        while let Some(f) = stack.pop() {
            if !affected.insert(f) {
                continue;
            }
            stack.extend(self.fs.file(f).referencing_files.iter().copied());
        }
        for f in affected {
            if let Some(l) = self.fs.library_of(f) {
                seeds.insert(l);
            }
            if self.fs.file(f).kind().is_library() {
                seeds.insert(f);
            }
        }
    }

    /// Whether [cycle] is the current cycle of its libraries.
    fn is_live(&self, cycle: CycleId) -> bool {
        self.graph
            .cycle(cycle)
            .libraries
            .first()
            .is_some_and(|l| self.graph.cycle_of_library.get(l) == Some(&cycle))
    }

    /// The link input of a library file (Dart `LibraryFileKind` as
    /// `LibraryBuilder` reads it).
    pub fn link_input(&self, library: FileId) -> LinkLibraryInput {
        let mut visited = IndexSet::new();
        LinkLibraryInput {
            unit: self.unit_input(library, true, &mut visited, None),
        }
    }

    fn link_input_for_cycle(
        &self,
        library: FileId,
        keep_files: &std::collections::HashSet<FileId>,
    ) -> LinkLibraryInput {
        let mut visited = IndexSet::new();
        LinkLibraryInput {
            unit: self.unit_input(library, true, &mut visited, Some(keep_files)),
        }
    }

    fn unit_input(
        &self,
        file: FileId,
        is_library: bool,
        visited: &mut IndexSet<FileId>,
        keep_files: Option<&std::collections::HashSet<FileId>>,
    ) -> LinkUnitInput {
        visited.insert(file);
        let fs = &self.fs;
        let f = fs.file(file);
        let c = f.c();
        let _ = is_library;
        let imports = c
            .library_imports
            .iter()
            .map(|s| LinkImport {
                uri: self.directive_uri(&s.uris.selected),
                is_synthetic: s.unlinked.is_synthetic_dart_core,
                combinators: combinators(&s.unlinked.combinators),
                import_keyword_offset: s.unlinked.import_keyword_offset,
                prefix: s.unlinked.prefix.as_ref().map(|p| LinkImportPrefix {
                    name: p.name.as_ref().map(|n| Arc::from(n.name.as_str())),
                    name_offset: p.name_offset,
                    is_deferred: p.deferred_offset.is_some(),
                }),
            })
            .collect();
        let exports = c
            .library_exports
            .iter()
            .map(|s| LinkExport {
                uri: self.directive_uri(&s.uris.selected),
                combinators: combinators(&s.unlinked.combinators),
                export_keyword_offset: s.unlinked.export_keyword_offset,
            })
            .collect();
        let parts = c
            .part_includes
            .iter()
            .map(|s| {
                let uri = match (&s.uris.selected, fs.included_part(file, s)) {
                    (
                        DirectiveUri::WithFile {
                            relative_uri_str,
                            relative_uri,
                            ..
                        },
                        Some(part),
                    ) if !visited.contains(&part) => LinkPartUri::Unit {
                        relative_uri_string: relative_uri_str.clone(),
                        relative_uri: relative_uri.clone(),
                        unit: Box::new(self.unit_input(part, false, visited, keep_files)),
                    },
                    (
                        DirectiveUri::WithFile {
                            relative_uri_str,
                            relative_uri,
                            file: target,
                        },
                        _,
                    ) => {
                        let t = fs.file(*target);
                        LinkPartUri::Other(LinkDirectiveUri::Source {
                            relative_uri_string: relative_uri_str.clone(),
                            relative_uri: relative_uri.clone(),
                            path: t.path.clone(),
                            uri: t.uri_str.clone(),
                        })
                    }
                    (other, _) => LinkPartUri::Other(self.directive_uri(other)),
                };
                LinkPart {
                    uri,
                    part_keyword_offset: s.unlinked.part_keyword_offset,
                }
            })
            .collect();
        let parsed = match keep_files {
            Some(keep) if !keep.contains(&file) => f.take_or_parse(),
            _ => f.parsed(),
        };
        LinkUnitInput {
            path: f.path.clone(),
            uri: f.uri_str.clone(),
            exists: c.exists,
            parsed,
            imports,
            exports,
            parts,
        }
    }

    /// The selected URI of an import or export.
    fn directive_uri(&self, uri: &DirectiveUri) -> LinkDirectiveUri {
        match uri {
            DirectiveUri::WithoutString => LinkDirectiveUri::None,
            DirectiveUri::WithString { relative_uri_str } => LinkDirectiveUri::RelativeUriString {
                relative_uri_string: relative_uri_str.clone(),
            },
            DirectiveUri::WithUri {
                relative_uri_str,
                relative_uri,
            } => LinkDirectiveUri::RelativeUri {
                relative_uri_string: relative_uri_str.clone(),
                relative_uri: relative_uri.clone(),
            },
            DirectiveUri::WithFile {
                relative_uri_str,
                relative_uri,
                file,
            } => {
                let f = self.fs.file(*file);
                if f.kind().is_library() {
                    LinkDirectiveUri::Library {
                        relative_uri_string: relative_uri_str.clone(),
                        relative_uri: relative_uri.clone(),
                        library_uri: f.uri_str.clone(),
                    }
                } else {
                    LinkDirectiveUri::Source {
                        relative_uri_string: relative_uri_str.clone(),
                        relative_uri: relative_uri.clone(),
                        path: f.path.clone(),
                        uri: f.uri_str.clone(),
                    }
                }
            }
        }
    }
}

fn combinators(list: &[UnlinkedCombinator]) -> Vec<LinkCombinator> {
    list.iter()
        .map(|c| LinkCombinator {
            is_show: c.is_show,
            names: c.names.iter().map(|n| Arc::from(n.as_str())).collect(),
            keyword_offset: c.keyword_offset,
            end_offset: c.end_offset,
        })
        .collect()
}

/// The shared data of the scheduler.
struct Job<'a> {
    driver: &'a Driver,
    order: &'a IndexSet<CycleId>,
    keep_files: &'a std::collections::HashSet<FileId>,
    remaining: &'a [AtomicUsize],
    users: &'a [Vec<usize>],
    shared: &'a Mutex<(LinkState, Vec<Option<Arc<LinkedCycle>>>)>,
    sched: &'a Mutex<(std::collections::VecDeque<usize>, usize)>,
    max_workers: usize,
}

impl<'a> Job<'a> {
    fn spawn_worker<'s>(&'s self, scope: &rayon::Scope<'s>)
    where
        'a: 's,
    {
        scope.spawn(move |scope| {
            loop {
                let i = {
                    let mut g = self.sched.lock().unwrap();
                    match g.0.pop_front() {
                        Some(i) => i,
                        None => {
                            g.1 -= 1;
                            return;
                        }
                    }
                };
                // A snapshot that has all direct dependencies (they published
                // before this cycle became ready).
                let state = self.shared.lock().unwrap().0.clone();
                let c = self.order[i];
                let cycle_inputs: Vec<LinkLibraryInput> = self
                    .driver
                    .graph
                    .cycle(c)
                    .libraries
                    .iter()
                    .map(|&l| self.driver.link_input_for_cycle(l, self.keep_files))
                    .collect();
                let linked = link_cycle(
                    &state.world,
                    &state.registry,
                    &crate::link_resolver::ResolverForLinking,
                    &cycle_inputs,
                );
                drop(cycle_inputs);
                drop(state);
                let linked = Arc::new(linked);
                {
                    let mut shared = self.shared.lock().unwrap();
                    let (state, results) = &mut *shared;
                    let libraries = linked.libraries.iter().map(|l| (l.uri.clone(), l.element));
                    state.world.add_store(linked.store.clone(), libraries);
                    for l in &linked.libraries {
                        state.registry.libraries.insert(l.uri.clone(), l.clone());
                    }
                    state
                        .registry
                        .const_exprs
                        .insert(linked.store.id.raw(), linked.const_exprs.clone());
                    results[i] = Some(linked);
                }
                let mut newly_ready = Vec::new();
                for &user in &self.users[i] {
                    if self.remaining[user].fetch_sub(1, Ordering::AcqRel) == 1 {
                        newly_ready.push(user);
                    }
                }
                if !newly_ready.is_empty() {
                    let to_spawn = {
                        let mut g = self.sched.lock().unwrap();
                        g.0.extend(newly_ready);
                        let can_spawn = (self.max_workers - g.1).min(g.0.len().saturating_sub(1));
                        g.1 += can_spawn;
                        can_spawn
                    };
                    for _ in 0..to_spawn {
                        self.spawn_worker(scope);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_state::{FileConfig, SourceFactory};
    use crate::uri::Uri;
    use dartr_parser::experimental_flags::ExperimentalFlag;
    use dartr_project::package_config::Packages;
    use dartr_project::{DartSdk, Workspace};

    #[test]
    fn shares_sdk_cycles_across_drivers_with_same_sdk_and_options() {
        let Some(sdk) = DartSdk::find() else {
            return;
        };
        let generation = Arc::new(Generation::new(0));
        let make_driver = |experiments: Vec<ExperimentalFlag>| {
            let sf = SourceFactory {
                workspace: Workspace::basic(Packages::empty(), "/tmp"),
                sdk: Some(sdk.clone()),
            };
            let fs = FileSystemState::new(
                sf,
                Box::new(move |_, _| FileConfig {
                    package_language_version: (3, 7),
                    experiments: experiments.clone(),
                }),
            );
            Driver::new(fs, generation.clone())
        };

        let core_uri = Uri::try_parse("dart:core").unwrap();
        let mut d1 = make_driver(Vec::new());
        let c1 = d1.fs.get_file_for_uri(&core_uri).unwrap();
        d1.fs.discover();
        d1.link_libraries(&[c1]);

        let mut d2 = make_driver(Vec::new());
        let c2 = d2.fs.get_file_for_uri(&core_uri).unwrap();
        d2.fs.discover();
        d2.link_libraries(&[c2]);

        // Source text and LinkedCycle for dart:core are shared by pointer when SDK and options match.
        assert!(Arc::ptr_eq(
            &d1.fs.file(c1).c().source,
            &d2.fs.file(c2).c().source
        ));
        let cycle1 = d1.graph.cycle_of_library[&c1];
        let cycle2 = d2.graph.cycle_of_library[&c2];
        assert!(Arc::ptr_eq(&d1.cycles[&cycle1], &d2.cycles[&cycle2]));
        assert_eq!(
            d1.state.world.libraries["dart:core"],
            d2.state.world.libraries["dart:core"]
        );

        // Different experiment flags produce a distinct linked cycle.
        let mut d3 = make_driver(vec![ExperimentalFlag::Macros]);
        let c3 = d3.fs.get_file_for_uri(&core_uri).unwrap();
        d3.fs.discover();
        d3.link_libraries(&[c3]);
        let cycle3 = d3.graph.cycle_of_library[&c3];
        assert!(!Arc::ptr_eq(&d1.cycles[&cycle1], &d3.cycles[&cycle3]));
    }

    #[test]
    fn shares_package_cycles_across_drivers_and_invalidates_on_change() {
        use dartr_project::package_config::Package;
        let Some(sdk) = DartSdk::find() else {
            return;
        };
        let generation = Arc::new(Generation::new(0));
        let shared_path = "/tmp/dartr_xctx_test_shared/lib/shared.dart";
        let doc_only_path = "/tmp/dartr_xctx_test_doc_only/lib/doc_only.dart";
        dartr_project::fs::set_overlay(
            shared_path,
            Some("/// @docImport 'package:doc_only/doc_only.dart';\nlibrary;\nint sharedValue() => 1;\n".to_string()),
        );
        dartr_project::fs::set_overlay(doc_only_path, Some("class DocOnly {}\n".to_string()));

        let shared_pkg = Package {
            name: "shared".to_string(),
            root: "/tmp/dartr_xctx_test_shared".to_string(),
            lib: "/tmp/dartr_xctx_test_shared/lib".to_string(),
            language_version: None,
        };
        let doc_only_pkg = Package {
            name: "doc_only".to_string(),
            root: "/tmp/dartr_xctx_test_doc_only".to_string(),
            lib: "/tmp/dartr_xctx_test_doc_only/lib".to_string(),
            language_version: None,
        };
        let make_driver = |packages: Vec<Package>| {
            let sf = SourceFactory {
                workspace: Workspace::basic(Packages::new(packages), "/tmp"),
                sdk: Some(sdk.clone()),
            };
            let fs = FileSystemState::new(
                sf,
                Box::new(|_, _| FileConfig {
                    package_language_version: (3, 7),
                    experiments: Vec::new(),
                }),
            );
            Driver::new(fs, generation.clone())
        };

        let shared_uri = Uri::try_parse("package:shared/shared.dart").unwrap();
        // Driver 1 has `doc_only` in its package config; Driver 2 does not.
        let mut d1 = make_driver(vec![shared_pkg.clone(), doc_only_pkg]);
        let f1 = d1.fs.get_file_for_uri(&shared_uri).unwrap();
        d1.fs.discover();
        d1.link_libraries(&[f1]);

        let mut d2 = make_driver(vec![shared_pkg]);
        let f2 = d2.fs.get_file_for_uri(&shared_uri).unwrap();
        d2.fs.discover();
        d2.link_libraries(&[f2]);

        assert!(Arc::ptr_eq(
            &d1.fs.file(f1).c().source,
            &d2.fs.file(f2).c().source
        ));
        assert!(Arc::ptr_eq(
            &d1.fs.file(f1).c().unlinked,
            &d2.fs.file(f2).c().unlinked
        ));
        let cycle1 = d1.graph.cycle_of_library[&f1];
        let cycle2 = d2.graph.cycle_of_library[&f2];
        let old_linked = d1.cycles[&cycle1].clone();
        assert!(Arc::ptr_eq(&old_linked, &d2.cycles[&cycle2]));

        // Edit `shared.dart` and notify d1: d1 gets a new cycle while d2 keeps the old snapshot
        // until d2 is notified.
        dartr_project::fs::set_overlay(
            shared_path,
            Some("/// @docImport 'package:doc_only/doc_only.dart';\nlibrary;\nString sharedValue() => 'x';\n".to_string()),
        );
        let change1 = d1.change_file(shared_path);
        assert!(change1.api_changed);
        d1.link_libraries(&[f1]);
        let new_cycle1 = d1.graph.cycle_of_library[&f1];
        assert!(!Arc::ptr_eq(&d1.cycles[&new_cycle1], &old_linked));
        assert!(Arc::ptr_eq(&d2.cycles[&cycle2], &old_linked));

        // Once d2 is notified of the file change, it reuses the new source, unlinked unit, and linked cycle.
        let change2 = d2.change_file(shared_path);
        assert!(change2.api_changed);
        d2.link_libraries(&[f2]);
        let new_cycle2 = d2.graph.cycle_of_library[&f2];
        assert!(Arc::ptr_eq(
            &d1.fs.file(f1).c().source,
            &d2.fs.file(f2).c().source
        ));
        assert!(Arc::ptr_eq(
            &d1.fs.file(f1).c().unlinked,
            &d2.fs.file(f2).c().unlinked
        ));
        assert!(Arc::ptr_eq(
            &d1.cycles[&new_cycle1],
            &d2.cycles[&new_cycle2]
        ));

        dartr_project::fs::set_overlay(shared_path, None);
        dartr_project::fs::set_overlay(doc_only_path, None);
    }
}
