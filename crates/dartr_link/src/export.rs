// Dart source: pkg/analyzer/lib/src/summary2/export.dart,
// pkg/analyzer/lib/src/summary2/combinator.dart

//! Export scopes: the names that a library exports, with the export
//! locations of re-exported names.
//!
//! Difference: Dart compares entries by `Reference` identity; here an entry
//! holds the element, and each declared reference has exactly one element,
//! so comparing elements is the same.

use std::sync::Arc;

use dartr_element::ElementId;
use indexmap::{IndexMap, IndexSet};

/// Dart `Combinator` (summary2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combinator {
    pub is_show: bool,
    pub names: IndexSet<Arc<str>>,
}

impl Combinator {
    /// Dart `Combinator.matches`: a setter name matches without its `=`.
    pub fn matches(&self, name: &str) -> bool {
        let name = name.strip_suffix('=').unwrap_or(name);
        self.names.contains(name)
    }
}

/// Dart `CombinatorListExtension.allows`.
pub fn combinators_allow(combinators: &[Combinator], name: &str) -> bool {
    for c in combinators {
        if c.is_show && !c.matches(name) {
            return false;
        }
        if !c.is_show && c.matches(name) {
            return false;
        }
    }
    true
}

/// Dart `ExportLocation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExportLocation {
    pub fragment_index: u32,
    pub export_index: u32,
}

/// Dart `ExportEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportEntry {
    pub name: Arc<str>,
    /// The element of the Dart `reference`.
    pub element: ElementId,
    pub locations: Vec<ExportLocation>,
}

impl ExportEntry {
    /// Dart `isDeclared`.
    pub fn is_declared(&self) -> bool {
        self.locations.is_empty()
    }

    /// Dart `isReExported`.
    pub fn is_re_exported(&self) -> bool {
        !self.locations.is_empty()
    }

    /// Dart `addLocation`.
    pub fn add_location(&mut self, location: ExportLocation) {
        if !self.locations.contains(&location) {
            self.locations.push(location);
        }
    }
}

/// Dart `ExportScope`.
#[derive(Clone, Debug, Default)]
pub struct ExportScope {
    pub entries_by_name: IndexMap<Arc<str>, ExportEntry>,
}

impl ExportScope {
    /// Dart `declare`.
    pub fn declare(&mut self, name: Arc<str>, element: ElementId) {
        self.entries_by_name.insert(
            name.clone(),
            ExportEntry {
                name,
                element,
                locations: Vec::new(),
            },
        );
    }

    /// Dart `export`: whether the entry was added.
    pub fn export(&mut self, location: ExportLocation, entry: &ExportEntry) -> bool {
        if let Some(existing) = self.entries_by_name.get_mut(&entry.name) {
            if existing.element == entry.element && existing.is_re_exported() {
                existing.add_location(location);
            }
            return false;
        }
        self.entries_by_name.insert(
            entry.name.clone(),
            ExportEntry {
                name: entry.name.clone(),
                element: entry.element,
                locations: vec![location],
            },
        );
        true
    }

    /// Dart `toExportEntries`.
    pub fn to_export_entries(&self) -> Vec<ExportEntry> {
        self.entries_by_name.values().cloned().collect()
    }
}

/// Dart `Export`: an export of a library of the cycle (`exporter`, an index
/// of a library builder) with its location and combinators.
#[derive(Clone, Debug)]
pub struct Export {
    pub exporter: usize,
    pub location: ExportLocation,
    pub combinators: Vec<Combinator>,
}

impl Export {
    /// Dart `addToExportScope`.
    pub fn add_to_export_scope(&self, scopes: &mut [ExportScope], entry: &ExportEntry) -> bool {
        if combinators_allow(&self.combinators, &entry.name) {
            return scopes[self.exporter].export(self.location, entry);
        }
        false
    }
}

/// Dart `Linker._buildExportScopes` after `buildInitialExportScope` and
/// `addExporters`: [scopes] are the export scopes of the libraries of the
/// cycle, [exports] for each library the exports of it by libraries of the
/// cycle (Dart `LibraryBuilder.exports`).
pub fn compute_export_scopes(scopes: &mut [ExportScope], exports: &[Vec<Export>]) {
    let mut exporting = IndexSet::new();
    let mut exported = IndexSet::new();
    for (library, library_exports) in exports.iter().enumerate() {
        if !library_exports.is_empty() {
            exported.insert(library);
            for export in library_exports {
                exporting.insert(export.exporter);
            }
        }
    }

    let mut both = IndexSet::new();
    for &e in &exported {
        if exporting.contains(&e) {
            both.insert(e);
        }
        for export in &exports[e] {
            // Dart iterates `exported.exportScope` while it may add to the
            // scope of the exporter (the same scope for a self-export);
            // iterate a snapshot.
            let entries: Vec<ExportEntry> = scopes[e].entries_by_name.values().cloned().collect();
            for entry in &entries {
                export.add_to_export_scope(scopes, entry);
            }
        }
    }

    let mut additional: Vec<(usize, ExportEntry)> = Vec::new();
    for &e in &both {
        for export in &exports[e] {
            let entries: Vec<ExportEntry> = scopes[e].entries_by_name.values().cloned().collect();
            for entry in entries {
                if export.add_to_export_scope(scopes, &entry) {
                    additional.push((export.exporter, entry));
                }
            }
        }
    }
    while let Some((exported_library, entry)) = additional.pop() {
        for export in &exports[exported_library] {
            if export.add_to_export_scope(scopes, &entry) {
                additional.push((export.exporter, entry.clone()));
            }
        }
    }
}
