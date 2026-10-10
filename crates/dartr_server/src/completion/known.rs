// Dart source: pkg/analyzer/lib/src/dart/analysis/driver.dart (_discoverDartCore, _discoverLibraries, discoverAvailableFiles)
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_graph.dart (_LibraryNode.computeDependencies)
// Dart source: pkg/analyzer/lib/src/dart/analysis/file_state.dart (FileKind.discoverReferencedFiles)

//! The order of the library files that the driver of Dart knows
//! (`FileSystemState.knownFiles`) when the not-imported pass runs: the
//! order in which the files were created.
//!
//! The Dart driver creates `dart:core` and the files it references first,
//! then the added (analyzed) files, then the files that the library cycle
//! walks of the analysis find (for each library: the imports and exports
//! of the library and of its parts, then each new dependency, depth
//! first), then the files of `discoverAvailableFiles` (the SDK libraries,
//! then the Dart files of the package `lib` folders).

use std::collections::{HashMap, HashSet};

use dartr_element::{Ctx, DirectiveUri, EId, LibraryElement};
use indexmap::IndexSet;

use super::elem;

/// The inputs of [`dart_known_order`].
pub struct KnownOrderInputs<'i> {
    /// The analyzed files of the context, in the order the server adds
    /// them.
    pub added_files: &'i [String],
    /// The SDK library files and the files of the package `lib` folders,
    /// in `discoverAvailableFiles` order.
    pub available_files: &'i [String],
    /// The URI of each library file (path → URI).
    pub uri_of_path: &'i HashMap<String, String>,
}

struct Walker<'c, 'a> {
    ctx: &'c Ctx<'a>,
    created: IndexSet<String>,
    visited: HashSet<String>,
}

impl Walker<'_, '_> {
    fn library(&self, uri: &str) -> Option<EId<LibraryElement>> {
        self.ctx.library_by_uri(uri)
    }

    /// The fragments of a library: the defining unit, then the parts
    /// (Dart `LibraryFileKind.fileKinds`).
    fn fragments(&self, library: EId<LibraryElement>) -> Vec<dartr_element::FId<dartr_element::LibraryFragment>> {
        let mut out = Vec::new();
        let mut stack = vec![self.ctx.get(library).first_fragment()];
        while let Some(f) = stack.pop() {
            out.push(f);
            for part in self.ctx.fragment(f).parts.iter().rev() {
                if let DirectiveUri::Unit { library_fragment, .. } = &part.directive.uri {
                    stack.push(*library_fragment);
                }
            }
        }
        out
    }

    fn target(&self, uri: &DirectiveUri) -> Option<String> {
        match uri {
            DirectiveUri::Library { library, .. } => Some(elem::library_uri(self.ctx, *library)),
            _ => None,
        }
    }

    /// Dart `_LibraryNode.computeDependencies`: creates the referenced
    /// libraries and returns them in order.
    fn dependencies(&mut self, uri: &str) -> Vec<String> {
        let Some(library) = self.library(uri) else {
            return Vec::new();
        };
        let mut deps: IndexSet<String> = IndexSet::new();
        for f in self.fragments(library) {
            let data = self.ctx.fragment(f);
            let mut explicit: Vec<String> = Vec::new();
            let mut implicit: Vec<String> = Vec::new();
            for import in &data.library_imports {
                if let Some(t) = self.target(&import.directive.uri) {
                    if import.is_synthetic {
                        implicit.push(t);
                    } else {
                        explicit.push(t);
                    }
                }
            }
            // The implicit `dart:core` import is the last import.
            for t in explicit.into_iter().chain(implicit) {
                deps.insert(t);
            }
            for export in &data.library_exports {
                if let Some(t) = self.target(&export.directive.uri) {
                    deps.insert(t);
                }
            }
        }
        for d in &deps {
            self.created.insert(d.clone());
        }
        deps.into_iter().collect()
    }

    /// Dart `DependencyWalker.walk` (the files that it creates).
    fn walk(&mut self, uri: &str) {
        if !self.visited.insert(uri.to_string()) {
            return;
        }
        for d in self.dependencies(uri) {
            if !self.visited.contains(&d) {
                self.walk(&d);
            }
        }
    }
}

/// The library URIs in the order the Dart driver creates them.
pub fn dart_known_order(ctx: &Ctx<'_>, inputs: &KnownOrderInputs<'_>) -> Vec<String> {
    let mut w = Walker {
        ctx,
        created: IndexSet::new(),
        visited: HashSet::new(),
    };
    // Dart `_discoverDartCore`: `discoverReferencedFiles` of `dart:core`
    // (exports, imports, parts, doc imports).
    w.created.insert("dart:core".to_string());
    if let Some(core) = w.library("dart:core") {
        let first = ctx.get(core).first_fragment();
        let data = ctx.fragment(first);
        let mut referenced = Vec::new();
        for e in &data.library_exports {
            referenced.extend(w.target(&e.directive.uri));
        }
        for i in data.library_imports.iter().filter(|i| !i.is_synthetic) {
            referenced.extend(w.target(&i.directive.uri));
        }
        for r in referenced {
            w.created.insert(r);
        }
    }
    // Dart `_discoverLibraries`: the added files.
    for path in inputs.added_files {
        if let Some(uri) = inputs.uri_of_path.get(path) {
            w.created.insert(uri.clone());
        }
    }
    // The analysis: the library cycle walk of each added library.
    for path in inputs.added_files {
        if let Some(uri) = inputs.uri_of_path.get(path).cloned() {
            w.walk(&uri);
        }
    }
    // Dart `discoverAvailableFiles`.
    for path in inputs.available_files {
        if let Some(uri) = inputs.uri_of_path.get(path) {
            w.created.insert(uri.clone());
        }
    }
    w.created.into_iter().collect()
}
