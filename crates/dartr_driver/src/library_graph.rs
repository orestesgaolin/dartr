// Dart source: pkg/analyzer/lib/src/dart/analysis/library_graph.dart,
// pkg/_fe_analyzer_shared/lib/src/util/dependency_walker.dart

//! Library cycles: the strongly connected components of the graph of
//! libraries with import and export edges (Tarjan, Dart
//! `DependencyWalker`), with their API signatures.

use indexmap::{IndexMap, IndexSet};

use crate::api_signature::ApiSignature;
use crate::file_state::{FileId, FileSystemState};

/// Index of a cycle in [`LibraryGraph`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CycleId(pub u32);

/// Dart `LibraryCycle`.
#[derive(Clone, Debug)]
pub struct LibraryCycle {
    pub id: CycleId,
    /// The libraries of the cycle, sorted by path.
    pub libraries: Vec<FileId>,
    /// Dart `transitivePackages`.
    pub transitive_packages: IndexSet<String>,
    /// Dart `directDependencies`.
    pub direct_dependencies: Vec<CycleId>,
    /// Dart `directUsers`.
    pub direct_users: Vec<CycleId>,
    /// Dart `apiSignature` (transitive).
    pub api_signature: String,
    /// Dart `manifestSignature`.
    pub manifest_signature: String,
    /// Dart `nonTransitiveApiSignature`.
    pub non_transitive_api_signature: String,
}

/// The library cycles computed so far (Dart: the `_libraryCycle` fields of
/// the `LibraryFileKind`s).
#[derive(Debug, Default)]
pub struct LibraryGraph {
    pub cycles: Vec<LibraryCycle>,
    pub cycle_of_library: IndexMap<FileId, CycleId>,
}

impl LibraryGraph {
    pub fn cycle(&self, id: CycleId) -> &LibraryCycle {
        &self.cycles[id.0 as usize]
    }

    /// Dart `LibraryFileKind.libraryCycle` (`computeLibraryCycle`).
    pub fn library_cycle(&mut self, fs: &FileSystemState, library: FileId) -> CycleId {
        if let Some(&c) = self.cycle_of_library.get(&library) {
            return c;
        }
        let mut walker = LibraryWalker {
            fs,
            graph: self,
            nodes: IndexMap::new(),
            index: 1,
            stack: Vec::new(),
        };
        walker.walk(library);
        self.cycle_of_library[&library]
    }
}

/// Dart `_LibraryNode` state of `Node`.
struct NodeState {
    index: u32,
    low_link: u32,
    dependencies: Option<Vec<FileId>>,
}

/// Dart `_LibraryWalker`.
struct LibraryWalker<'a> {
    fs: &'a FileSystemState,
    graph: &'a mut LibraryGraph,
    nodes: IndexMap<FileId, NodeState>,
    index: u32,
    stack: Vec<FileId>,
}

impl LibraryWalker<'_> {
    fn is_evaluated(&self, library: FileId) -> bool {
        self.graph.cycle_of_library.contains_key(&library)
    }

    fn node(&mut self, library: FileId) -> &mut NodeState {
        self.nodes.entry(library).or_insert(NodeState {
            index: 0,
            low_link: 0,
            dependencies: None,
        })
    }

    /// Dart `Node.getDependencies` + `_LibraryNode.computeDependencies`: the
    /// imported and exported libraries of all files of the library, as a
    /// set in first-seen order.
    fn dependencies(&mut self, library: FileId) -> Vec<FileId> {
        if let Some(d) = &self.node(library).dependencies {
            return d.clone();
        }
        let fs = self.fs;
        let mut referenced = IndexSet::new();
        for kind in fs.library_file_kinds(library) {
            let c = fs.file(kind).c();
            let imports = c.library_imports.iter().map(|d| &d.uris.selected);
            let exports = c.library_exports.iter().map(|d| &d.uris.selected);
            for uri in imports.chain(exports) {
                if let Some(l) = fs.library_of_uri(uri) {
                    referenced.insert(l);
                }
            }
        }
        let result: Vec<FileId> = referenced.into_iter().collect();
        self.node(library).dependencies = Some(result.clone());
        result
    }

    /// Dart `DependencyWalker.walk`.
    fn walk(&mut self, start: FileId) {
        if self.is_evaluated(start) {
            return;
        }
        self.strong_connect(start);
    }

    fn strong_connect(&mut self, node: FileId) {
        let mut has_trivial_cycle = false;
        let index = self.index;
        self.index += 1;
        {
            let n = self.node(node);
            n.index = index;
            n.low_link = index;
        }
        self.stack.push(node);
        for dependency in self.dependencies(node) {
            if self.is_evaluated(dependency) {
                continue;
            }
            if dependency == node {
                has_trivial_cycle = true;
            } else if self.node(dependency).index == 0 {
                self.strong_connect(dependency);
                let dep_low = self.node(dependency).low_link;
                let n = self.node(node);
                if dep_low < n.low_link {
                    n.low_link = dep_low;
                }
            } else {
                let dep_index = self.node(dependency).index;
                let n = self.node(node);
                if dep_index < n.low_link {
                    n.low_link = dep_index;
                }
            }
        }
        let n = self.node(node);
        if n.low_link == n.index {
            if *self.stack.last().unwrap() == node {
                self.stack.pop();
                // Dart `evaluate` and `evaluateScc` do the same here.
                let _ = has_trivial_cycle;
                self.evaluate_scc(vec![node]);
            } else {
                let mut scc = Vec::new();
                loop {
                    let other = self.stack.pop().unwrap();
                    scc.push(other);
                    if other == node {
                        break;
                    }
                }
                self.evaluate_scc(scc);
            }
        }
    }

    /// Dart `_LibraryWalker.evaluateScc`.
    fn evaluate_scc(&mut self, mut scc: Vec<FileId>) {
        let fs = self.fs;
        let mut api_signature = ApiSignature::new();
        api_signature.add_uint32_list(&fs.salt_for_elements);

        scc.sort_by(|a, b| fs.file(*a).path.cmp(&fs.file(*b).path));

        let mut direct_dependencies: IndexSet<CycleId> = IndexSet::new();
        for &node in &scc {
            let deps = self.dependencies(node);
            api_signature.add_int(deps.len() as u32);
            for referenced in deps {
                let Some(&cycle) = self.graph.cycle_of_library.get(&referenced) else {
                    continue;
                };
                if direct_dependencies.insert(cycle) {
                    api_signature.add_string(&self.graph.cycle(cycle).api_signature);
                }
            }
        }

        let mut libraries = Vec::new();
        for &node in &scc {
            let file = fs.file(node);
            libraries.push(node);
            let (major, minor) = file.config.package_language_version;
            api_signature.add_language_version(major, minor);
            api_signature.add_string(&file.uri_str);
            let library_files = fs.library_files(node);
            api_signature.add_int(library_files.len() as u32);
            for f in library_files {
                let f = fs.file(f);
                api_signature.add_bool(f.exists());
                api_signature.add_bytes(f.api_signature());
            }
        }

        let mut transitive_packages = IndexSet::new();
        for &d in &direct_dependencies {
            for p in &self.graph.cycle(d).transitive_packages {
                transitive_packages.insert(p.clone());
            }
        }
        for &l in &libraries {
            if let Some(p) = &fs.file(l).uri_properties.package_name {
                transitive_packages.insert(p.clone());
            }
        }

        let (manifest_signature, non_transitive_api_signature) = {
            let mut manifest = ApiSignature::new();
            let mut api = ApiSignature::new();
            // Dart: `manifestBuilder.addBytes(saltForElements)` (the list
            // of ints as bytes) and `_addUriResolutionToSignature`.
            for v in &fs.salt_for_elements {
                manifest.add_bytes(&[*v as u8]);
            }
            if let Some(sdk) = &fs.source_factory.sdk {
                manifest.add_string(sdk.path());
            }
            let mut package_paths: Vec<String> = transitive_packages
                .iter()
                .filter_map(|name| fs.source_factory.workspace.packages.get(name))
                .map(|p| p.lib.clone())
                .collect();
            package_paths.sort();
            manifest.add_string_list(&package_paths);
            let mut sorted_files: Vec<FileId> =
                libraries.iter().flat_map(|&l| fs.library_files(l)).collect();
            sorted_files.sort_by(|a, b| fs.file(*a).path.cmp(&fs.file(*b).path));
            for f in sorted_files {
                let file = fs.file(f);
                manifest.add_string(&file.path);
                manifest.add_string(&file.uri_str);
                api.add_bytes(file.api_signature());
                fs.add_directives_signature(f, &mut api);
            }
            (manifest.to_hex(), api.to_hex())
        };

        let id = CycleId(self.graph.cycles.len() as u32);
        let direct_dependencies: Vec<CycleId> = direct_dependencies.into_iter().collect();
        for &d in &direct_dependencies {
            self.graph.cycles[d.0 as usize].direct_users.push(id);
        }
        self.graph.cycles.push(LibraryCycle {
            id,
            libraries,
            transitive_packages,
            direct_dependencies,
            direct_users: Vec::new(),
            api_signature: api_signature.to_hex(),
            manifest_signature,
            non_transitive_api_signature,
        });
        for node in scc {
            self.graph.cycle_of_library.insert(node, id);
        }
    }
}
