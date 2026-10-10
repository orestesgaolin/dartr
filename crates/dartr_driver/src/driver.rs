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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dartr_element::{Generation, WorldSnapshot};
use dartr_link::input::{
    LinkCombinator, LinkDirectiveUri, LinkExport, LinkImport, LinkImportPrefix, LinkLibraryInput,
    LinkPart, LinkPartUri, LinkUnitInput,
};
use dartr_link::link::{ConstExprs, LinkedCycle, LinkedLibraries, LinkedLibrary, link_cycle};
use indexmap::{IndexMap, IndexSet};

use crate::file_state::{DirectiveUri, FileId, FileSystemState};
use crate::library_graph::{CycleId, LibraryGraph};
use crate::unlinked_data::UnlinkedCombinator;

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
}

impl Driver {
    pub fn new(fs: FileSystemState, generation: Arc<Generation>) -> Driver {
        Driver {
            fs,
            graph: LibraryGraph::default(),
            state: LinkState {
                world: WorldSnapshot::new(generation),
                registry: LinkedRegistry::default(),
            },
            cycles: IndexMap::new(),
        }
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
            self.cycles.insert(order[i], result.expect("cycle linked"));
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
