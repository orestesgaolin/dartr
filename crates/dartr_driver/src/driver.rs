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

use std::path::{Path, PathBuf};

use crate::disk_cache::{CachedSdkFileSummary, DiskCycleKey};
use crate::file_state::{
    DirectiveUri, FileConfig, FileId, FileSystemState, LazyParsed, RefreshData, SdkFileKey,
};
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

type BundleKey = (PathBuf, FileConfig, Vec<u32>);
type BundleMap = HashMap<Arc<str>, CachedSdkFileSummary>;

/// Shared cache of parsed files and linked library cycles
/// across all [`Driver`]s that share the same [`Generation`].
#[derive(Default)]
pub(crate) struct SdkCache {
    files: RwLock<HashMap<SdkFileKey, Arc<RefreshData>>>,
    cycles: RwLock<HashMap<SdkCycleKey, Arc<LinkedCycle>>>,
    unlinked_bundles: RwLock<HashMap<BundleKey, Arc<RwLock<BundleMap>>>>,
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

    fn get_or_load_bundle(
        &self,
        dir: &Path,
        config: &FileConfig,
        salt: &[u32],
    ) -> Arc<RwLock<BundleMap>> {
        let bkey = (dir.to_path_buf(), config.clone(), salt.to_vec());
        if let Some(b) = self.unlinked_bundles.read().unwrap().get(&bkey) {
            return b.clone();
        }
        let mut guard = self.unlinked_bundles.write().unwrap();
        guard
            .entry(bkey)
            .or_insert_with(|| {
                let loaded =
                    crate::disk_cache::load_unlinked_bundle(dir, config, salt).unwrap_or_default();
                Arc::new(RwLock::new(loaded))
            })
            .clone()
    }

    pub(crate) fn get_disk_sdk_file(&self, dir: &Path, key: &SdkFileKey) -> Option<RefreshData> {
        let bundle = self.get_or_load_bundle(dir, &key.config, &key.salt);
        let guard = bundle.read().unwrap();
        let entry = guard.get(&key.path)?;
        if entry.is_dart_core != key.is_dart_core {
            return None;
        }
        let (mtime_secs, mtime_nanos, file_size) = crate::disk_cache::file_stamp(&key.path)?;
        if (mtime_secs, mtime_nanos, file_size)
            != (entry.mtime_secs, entry.mtime_nanos, entry.file_size)
        {
            return None;
        }
        Some(RefreshData {
            exists: entry.exists,
            content_hash: entry.content_hash.clone(),
            unlinked_signature: entry.unlinked_signature,
            parsed: LazyParsed::lazy(key.path.clone(), key.config.clone()),
            unlinked: entry.unlinked.clone(),
        })
    }

    pub(crate) fn save_disk_sdk_files(
        &self,
        dir: &Path,
        items: Vec<(FileConfig, Vec<u32>, CachedSdkFileSummary)>,
    ) {
        let mut grouped: HashMap<(FileConfig, Vec<u32>), Vec<CachedSdkFileSummary>> =
            HashMap::new();
        for (config, salt, summary) in items {
            grouped.entry((config, salt)).or_default().push(summary);
        }
        for ((config, salt), summaries) in grouped {
            let bundle = self.get_or_load_bundle(dir, &config, &salt);
            let mut guard = bundle.write().unwrap();
            for s in summaries {
                guard.insert(s.path.clone(), s);
            }
            crate::disk_cache::save_unlinked_bundle(dir, &config, &salt, &guard);
        }
    }

    pub(crate) fn get_cycle(&self, key: &SdkCycleKey) -> Option<Arc<LinkedCycle>> {
        self.cycles.read().unwrap().get(key).cloned()
    }

    pub(crate) fn insert_cycle(&self, key: SdkCycleKey, cycle: Arc<LinkedCycle>) {
        self.cycles.write().unwrap().insert(key, cycle);
    }
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
        }
    }

    /// Whether `c` consists exclusively of SDK (`dart:*`) libraries and none of
    /// its files has an in-memory overlay.
    fn is_sdk_cycle(&self, c: CycleId) -> bool {
        let cycle = self.graph.cycle(c);
        !cycle.libraries.is_empty()
            && cycle.libraries.iter().all(|&l| {
                self.fs.file(l).uri_properties.is_dart
                    && self
                        .fs
                        .library_files(l)
                        .into_iter()
                        .all(|f| !dartr_project::fs::has_overlay(&self.fs.file(f).path))
            })
    }

    fn disk_cycle_key(&self, c: CycleId) -> DiskCycleKey {
        let sdk_path = self
            .fs
            .source_factory
            .sdk
            .as_ref()
            .map(|s| s.path().to_string())
            .unwrap_or_default();
        let cycle = self.graph.cycle(c);
        let mut unlinked_signatures = Vec::new();
        for &d in &cycle.direct_dependencies {
            for &l in &self.graph.cycle(d).libraries {
                for f in self.fs.library_files(l) {
                    unlinked_signatures.push(self.fs.file(f).c().unlinked_signature);
                }
            }
        }
        for &l in &cycle.libraries {
            for f in self.fs.library_files(l) {
                unlinked_signatures.push(self.fs.file(f).c().unlinked_signature);
            }
        }
        DiskCycleKey {
            sdk_path,
            api_signature: cycle.api_signature.clone(),
            manifest_signature: cycle.manifest_signature.clone(),
            non_transitive_api_signature: cycle.non_transitive_api_signature.clone(),
            unlinked_signatures,
        }
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
        self.state.world = self.state.world.with_store(linked.store.clone(), libraries);
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

    /// Links the cycles of [libraries] and of everything they depend on.
    /// The files must be discovered ([`FileSystemState::discover`]).
    pub fn link_libraries(&mut self, libraries: &[FileId]) {
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
            return;
        }
        let disk_dir = crate::disk_cache::cache_dir();
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
                if self.is_sdk_cycle(c)
                    && let Some(dir) = disk_dir.as_deref()
                {
                    let disk_key = self.disk_cycle_key(c);
                    if let Some(linked) =
                        crate::disk_cache::load_linked_cycle(dir, &disk_key, &self.state.world)
                    {
                        let linked = Arc::new(linked);
                        self.sdk_cache.insert_cycle(key, linked.clone());
                        self.publish_linked_cycle(c, linked);
                        continue;
                    }
                }
            }
            remaining_order.insert(c);
        }
        let order = remaining_order;
        if order.is_empty() {
            return;
        }
        // The link inputs: built here, because the file state is not
        // shared between threads.
        let inputs: Vec<Vec<LinkLibraryInput>> = order
            .iter()
            .map(|&c| {
                self.graph
                    .cycle(c)
                    .libraries
                    .iter()
                    .map(|&l| self.link_input(l))
                    .collect()
            })
            .collect();
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
        let shared = Mutex::new((self.state.clone(), vec![None; order.len()]));
        let ready: Vec<usize> = (0..order.len())
            .filter(|&i| remaining[i].load(Ordering::Relaxed) == 0)
            .collect();
        let job = Job {
            inputs: &inputs,
            remaining: &remaining,
            users: &users,
            shared: &shared,
        };
        rayon::scope(|scope| {
            for i in ready {
                job.spawn(scope, i);
            }
        });
        let (state, results) = shared.into_inner().unwrap();
        self.state = state;
        for (i, result) in results.into_iter().enumerate() {
            let c = order[i];
            let linked = result.expect("cycle linked");
            self.cycles.insert(c, linked.clone());
            let key = self.sdk_cycle_key(c);
            self.sdk_cache.insert_cycle(key, linked.clone());
            if self.is_sdk_cycle(c)
                && let Some(dir) = disk_dir.as_deref()
            {
                let disk_key = self.disk_cycle_key(c);
                crate::disk_cache::save_linked_cycle(dir, &disk_key, &self.state.world, &linked);
            }
        }
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
            self.cycles.shift_remove(c);
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
    /// that reference it.
    fn change_seeds(&mut self, file: FileId, seeds: &mut IndexSet<FileId>) {
        if let Some(l) = self.fs.library_of(file) {
            seeds.insert(l);
        }
        if self.fs.file(file).kind().is_library() {
            seeds.insert(file);
        }
        let referencing: Vec<FileId> = self
            .fs
            .file(file)
            .referencing_files
            .iter()
            .copied()
            .collect();
        for r in referencing {
            if let Some(l) = self.fs.library_of(r) {
                seeds.insert(l);
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
            unit: self.unit_input(library, true, &mut visited),
        }
    }

    fn unit_input(
        &self,
        file: FileId,
        is_library: bool,
        visited: &mut IndexSet<FileId>,
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
                        unit: Box::new(self.unit_input(part, false, visited)),
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
        LinkUnitInput {
            path: f.path.clone(),
            uri: f.uri_str.clone(),
            exists: c.exists,
            parsed: c.parsed.clone(),
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
    inputs: &'a [Vec<LinkLibraryInput>],
    remaining: &'a [AtomicUsize],
    users: &'a [Vec<usize>],
    shared: &'a Mutex<(LinkState, Vec<Option<Arc<LinkedCycle>>>)>,
}

impl<'a> Job<'a> {
    fn spawn<'s>(&'s self, scope: &rayon::Scope<'s>, i: usize)
    where
        'a: 's,
    {
        scope.spawn(move |scope| {
            // A snapshot that has all direct dependencies (they published
            // before this cycle became ready).
            let state = self.shared.lock().unwrap().0.clone();
            let linked = link_cycle(
                &state.world,
                &state.registry,
                &crate::link_resolver::ResolverForLinking,
                &self.inputs[i],
            );
            let linked = Arc::new(linked);
            {
                let mut shared = self.shared.lock().unwrap();
                let (state, results) = &mut *shared;
                let libraries = linked.libraries.iter().map(|l| (l.uri.clone(), l.element));
                state.world = state.world.with_store(linked.store.clone(), libraries);
                for l in &linked.libraries {
                    state.registry.libraries.insert(l.uri.clone(), l.clone());
                }
                state
                    .registry
                    .const_exprs
                    .insert(linked.store.id.raw(), linked.const_exprs.clone());
                results[i] = Some(linked);
            }
            for &user in &self.users[i] {
                if self.remaining[user].fetch_sub(1, Ordering::AcqRel) == 1 {
                    self.spawn(scope, user);
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

        // ParsedUnit and LinkedCycle for dart:core are shared by pointer when SDK and options match.
        assert!(Arc::ptr_eq(
            &d1.fs.file(c1).c().parsed,
            &d2.fs.file(c2).c().parsed
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

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let id = SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "dartr-test-{}-{}-{}",
                label,
                std::process::id(),
                id
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn disk_cache_produces_identical_diagnostics() {
        let Some(sdk) = DartSdk::find() else {
            return;
        };
        let work_dir = TempDir::new("work-diag");
        let cache_dir = TempDir::new("cache-diag");
        let main_path = work_dir.path().join("main.dart");
        std::fs::write(
            &main_path,
            r#"
import 'dart:async';
import 'dart:collection';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

const double twoPi = pi * 2;
const Duration d = Duration.zero;

void f() {
  int x = 'not an int';
  List<int> list = <String>['a'];
  HashMap<String, int> map = HashMap();
  map[123] = 'wrong';
  Future<int> fut = Future.value('bad');
  var encoded = jsonEncode({'k': twoPi, 'd': d.inSeconds});
  File f = File(encoded);
  undefinedFunction(f);
}
"#,
        )
        .unwrap();
        let main_path_str = main_path.to_string_lossy().to_string();

        let run_once = |gen_id: u32| {
            crate::disk_cache::with_cache_dir_override(Some(cache_dir.path().to_path_buf()), || {
                let generation = Arc::new(Generation::new(gen_id));
                let sf = SourceFactory {
                    workspace: Workspace::basic(
                        Packages::empty(),
                        &work_dir.path().to_string_lossy(),
                    ),
                    sdk: Some(sdk.clone()),
                };
                let fs = FileSystemState::new(
                    sf,
                    Box::new(|_, _| FileConfig {
                        package_language_version: (3, 7),
                        experiments: Vec::new(),
                    }),
                );
                let mut driver = Driver::new(fs, generation);
                let file_id = driver.fs.get_file_for_path(&main_path_str);
                driver.fs.discover();
                let core_uri = Uri::try_parse("dart:core").unwrap();
                let core_id = driver.fs.get_file_for_uri(&core_uri).unwrap();
                let core_parsed_after_discover =
                    driver.fs.file(core_id).c().parsed.get_if_ready().is_some();
                driver.link_libraries(&[file_id]);
                let core_parsed_after_link =
                    driver.fs.file(core_id).c().parsed.get_if_ready().is_some();
                let resolved = driver
                    .analyze_library(file_id, dartr_resolver::options::AnalysisOptions::default())
                    .expect("library analyzed");
                let diags: Vec<(String, usize, usize, String)> = resolved.units[0]
                    .diagnostics
                    .iter()
                    .map(|d| {
                        (
                            d.code.name.to_string(),
                            d.offset,
                            d.length,
                            d.message.clone(),
                        )
                    })
                    .collect();
                (core_parsed_after_discover, core_parsed_after_link, diags)
            })
        };

        // Cold run populates the disk cache.
        let (cold_disc, cold_link, cold_diags) = run_once(100);
        assert!(cold_disc);
        assert!(cold_link);
        assert!(!cold_diags.is_empty());

        let entries: Vec<_> = std::fs::read_dir(cache_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(
            entries
                .iter()
                .any(|e| e.file_name().to_string_lossy().starts_with("unlinked_"))
        );
        assert!(
            entries
                .iter()
                .any(|e| e.file_name().to_string_lossy().starts_with("cycle_"))
        );

        // Warm run in a fresh Generation loads unlinked summaries and linked SDK cycles from disk
        // without parsing dart:core during discover or link_libraries, and yields identical diagnostics.
        let (warm_disc, warm_link, warm_diags) = run_once(101);
        assert!(!warm_disc);
        assert!(!warm_link);
        assert_eq!(cold_diags, warm_diags);
    }

    #[test]
    fn disk_cache_key_changes_with_language_version_and_experiments() {
        assert_eq!(crate::disk_cache::parse_cache_dir_value("none"), None);
        assert_eq!(crate::disk_cache::parse_cache_dir_value("NONE"), None);
        assert_eq!(crate::disk_cache::parse_cache_dir_value("  "), None);

        let Some(sdk) = DartSdk::find() else {
            return;
        };
        let cache_dir = TempDir::new("cache-keys");

        let link_core = |gen_id: u32, version: (u32, u32), experiments: Vec<ExperimentalFlag>| {
            crate::disk_cache::with_cache_dir_override(Some(cache_dir.path().to_path_buf()), || {
                let generation = Arc::new(Generation::new(gen_id));
                let sf = SourceFactory {
                    workspace: Workspace::basic(Packages::empty(), "/tmp"),
                    sdk: Some(sdk.clone()),
                };
                let fs = FileSystemState::new(
                    sf,
                    Box::new(move |_, _| FileConfig {
                        package_language_version: version,
                        experiments: experiments.clone(),
                    }),
                );
                let mut driver = Driver::new(fs, generation);
                let core_uri = Uri::try_parse("dart:core").unwrap();
                let core_id = driver.fs.get_file_for_uri(&core_uri).unwrap();
                driver.fs.discover();
                let was_parsed = driver.fs.file(core_id).c().parsed.get_if_ready().is_some();
                driver.link_libraries(&[core_id]);
                was_parsed
            })
        };

        let count_files = || -> (usize, usize) {
            let mut unlinked = 0;
            let mut cycles = 0;
            for entry in std::fs::read_dir(cache_dir.path()).unwrap().flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("unlinked_") {
                    unlinked += 1;
                } else if name.starts_with("cycle_") {
                    cycles += 1;
                }
            }
            (unlinked, cycles)
        };

        // 1. Cold run with (3, 7), no experiments.
        assert!(link_core(200, (3, 7), Vec::new()));
        let (u1, c1) = count_files();
        assert_eq!(u1, 1);
        assert!(c1 >= 1);

        // 2. Warm run with identical (3, 7), no experiments -> hits cache (not re-parsed, no new files).
        assert!(!link_core(201, (3, 7), Vec::new()));
        assert_eq!(count_files(), (u1, c1));

        // 3. Different language version (3, 5) -> misses cache, parses and writes new entries.
        assert!(link_core(202, (3, 5), Vec::new()));
        let (u2, c2) = count_files();
        assert_eq!(u2, u1 + 1);
        assert!(c2 > c1);

        // 4. Different experiment flag -> misses cache, parses and writes new entries.
        assert!(link_core(203, (3, 7), vec![ExperimentalFlag::Macros]));
        let (u3, c3) = count_files();
        assert_eq!(u3, u2 + 1);
        assert!(c3 > c2);
    }

    #[test]
    fn disk_cache_ignores_and_rebuilds_corrupt_entries() {
        let Some(sdk) = DartSdk::find() else {
            return;
        };
        let work_dir = TempDir::new("work-corrupt");
        let cache_dir = TempDir::new("cache-corrupt");
        let main_path = work_dir.path().join("main.dart");
        std::fs::write(
            &main_path,
            "import 'dart:math';\nvoid main() { int a = sin(1.0); }\n",
        )
        .unwrap();
        let main_path_str = main_path.to_string_lossy().to_string();

        let analyze = |gen_id: u32| -> Vec<(String, usize, usize)> {
            crate::disk_cache::with_cache_dir_override(Some(cache_dir.path().to_path_buf()), || {
                let generation = Arc::new(Generation::new(gen_id));
                let sf = SourceFactory {
                    workspace: Workspace::basic(
                        Packages::empty(),
                        &work_dir.path().to_string_lossy(),
                    ),
                    sdk: Some(sdk.clone()),
                };
                let fs = FileSystemState::new(
                    sf,
                    Box::new(|_, _| FileConfig {
                        package_language_version: (3, 7),
                        experiments: Vec::new(),
                    }),
                );
                let mut driver = Driver::new(fs, generation);
                let file_id = driver.fs.get_file_for_path(&main_path_str);
                driver.fs.discover();
                driver.link_libraries(&[file_id]);
                let resolved = driver
                    .analyze_library(file_id, dartr_resolver::options::AnalysisOptions::default())
                    .expect("library analyzed");
                resolved.units[0]
                    .diagnostics
                    .iter()
                    .map(|d| (d.code.name.to_string(), d.offset, d.length))
                    .collect()
            })
        };

        let expected = analyze(300);
        assert!(!expected.is_empty());

        let cache_files = || -> Vec<PathBuf> {
            std::fs::read_dir(cache_dir.path())
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "bin"))
                .collect()
        };

        // 1. Corrupt by truncating to 5 bytes.
        for path in cache_files() {
            std::fs::write(&path, b"SHORT").unwrap();
        }
        assert_eq!(analyze(301), expected);

        // 2. Corrupt by flipping a payload byte (checksum failure).
        for path in cache_files() {
            let mut bytes = std::fs::read(&path).unwrap();
            assert!(bytes.len() > 40);
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
            std::fs::write(&path, &bytes).unwrap();
        }
        assert_eq!(analyze(302), expected);

        // 3. Corrupt by writing a valid header & checksum over a 3-byte garbage payload.
        for path in cache_files() {
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.len() > 40);
            let bad_payload = b"bad";
            let mut header = bytes[..28].to_vec();
            header.extend_from_slice(&(bad_payload.len() as u32).to_le_bytes());
            let mut hash: u64 = 0xcbf29ce484222325;
            for &b in bad_payload {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            header.extend_from_slice(&hash.to_le_bytes());
            header.extend_from_slice(bad_payload);
            std::fs::write(&path, &header).unwrap();
        }
        assert_eq!(analyze(303), expected);

        // 4. Subsequent warm run succeeds using the rebuilt cache files.
        assert_eq!(analyze(304), expected);
    }
}
