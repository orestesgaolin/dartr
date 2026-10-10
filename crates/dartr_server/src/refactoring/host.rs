// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring.dart (RefactoringWorkspace)
// Dart source: pkg/analysis_server/lib/src/services/search/search_engine.dart (SearchEngine)

//! What the refactorings need from a server: the resolved units, the
//! search engine and the change workspace (Dart `RefactoringWorkspace`).
//! The LSP server and the legacy server implement it.

use std::sync::Arc;

use dartr_project::AnalysisContextCollection;

use crate::correction::change_builder::ChangeWorkspace;
use crate::index::ElementKey;
use crate::server::{Match, ResolvedUnitRef, SElem, SearchScope};

/// Dart `RefactoringWorkspace` and `SearchEngine`.
pub trait RefactoringHost {
    /// The contexts of the workspace.
    fn collection(&self) -> Option<&AnalysisContextCollection>;

    /// The resolved unit of [path] in the driver of [context] (the context
    /// of the path when `None`).
    fn resolved_unit_in(&mut self, path: &str, context: Option<usize>) -> Option<ResolvedUnitRef>;

    /// Dart `SearchEngine.searchReferences`.
    fn search_references(&mut self, element: &SElem) -> Vec<Match>;

    /// Dart `getHierarchyMembersAndParameters(includeParametersForFields:
    /// true)`.
    fn hierarchy_members_and_parameters(&mut self, member: &SElem) -> (Vec<SElem>, Vec<SElem>);

    /// Dart `getHierarchyNamedParameters`.
    fn hierarchy_named_parameters(&mut self, parameter: &SElem) -> Vec<SElem>;

    /// Dart `SearchEngine.appendAllSubtypes`.
    fn append_all_subtypes(
        &mut self,
        class: &SElem,
        all: &mut Vec<SElem>,
        keys: &mut Vec<(usize, ElementKey)>,
    );

    /// The files that a search looks at, by driver.
    fn search_scope(&mut self) -> Arc<SearchScope>;

    /// Whether the file [path] mentions one of [names].
    fn mentions_any(&mut self, path: &str, names: &[String]) -> bool;

    /// Calls [f] with the workspace of a change builder.
    fn with_change_workspace(&mut self, f: &mut dyn FnMut(&mut dyn ChangeWorkspace));
}
