// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring.dart (RefactoringWorkspace)

//! The legacy server as the host of the refactorings of `dartr_server`
//! (one refactoring engine for LSP and the legacy protocol).

use std::sync::Arc;

use dartr_server::correction::change_builder::ChangeWorkspace;
use dartr_server::index::ElementKey;
use dartr_server::refactoring::host::RefactoringHost;
use dartr_server::refactoring::status::{RefactoringStatus, Severity};
use dartr_server::server as srv;

use crate::protocol;
use crate::search::{LegacyMatch, SElem, SearchEngine, SearchScope};

fn to_server(e: &SElem) -> srv::SElem {
    srv::SElem {
        lib: e.lib.clone(),
        unit: e.unit,
        id: e.id,
    }
}

fn to_legacy(e: &srv::SElem) -> SElem {
    SElem {
        lib: e.lib.clone(),
        unit: e.unit,
        id: e.id,
    }
}

fn to_match(m: LegacyMatch) -> srv::Match {
    srv::Match {
        path: m.path,
        offset: m.offset,
        length: m.length,
        context: m.context,
        kind: m.relation,
        is_qualified: m.is_qualified,
    }
}

impl RefactoringHost for SearchEngine<'_> {
    fn collection(&self) -> Option<&dartr_project::AnalysisContextCollection> {
        Some(self.collection)
    }

    fn resolved_unit_in(
        &mut self,
        path: &str,
        context: Option<usize>,
    ) -> Option<srv::ResolvedUnitRef> {
        let r = SearchEngine::resolved_unit_in(self, path, context)?;
        Some(srv::ResolvedUnitRef {
            library: r.library,
            index: r.index,
        })
    }

    fn search_references(&mut self, element: &srv::SElem) -> Vec<srv::Match> {
        SearchEngine::search_references(self, &to_legacy(element))
            .into_iter()
            .map(to_match)
            .collect()
    }

    fn hierarchy_members_and_parameters(
        &mut self,
        member: &srv::SElem,
    ) -> (Vec<srv::SElem>, Vec<srv::SElem>) {
        let (m, p) = SearchEngine::hierarchy_members_and_parameters(self, &to_legacy(member));
        (
            m.iter().map(to_server).collect(),
            p.iter().map(to_server).collect(),
        )
    }

    fn hierarchy_named_parameters(&mut self, parameter: &srv::SElem) -> Vec<srv::SElem> {
        SearchEngine::hierarchy_named_parameters(self, &to_legacy(parameter))
            .iter()
            .map(to_server)
            .collect()
    }

    fn append_all_subtypes(
        &mut self,
        class: &srv::SElem,
        all: &mut Vec<srv::SElem>,
        keys: &mut Vec<(usize, ElementKey)>,
    ) {
        let mut legacy: Vec<SElem> = Vec::new();
        SearchEngine::append_all_subtypes(self, &to_legacy(class), &mut legacy, keys);
        all.extend(legacy.iter().map(to_server));
    }

    fn search_scope(&mut self) -> Arc<SearchScope> {
        SearchEngine::search_scope(self)
    }

    fn mentions_any(&mut self, path: &str, names: &[String]) -> bool {
        SearchEngine::mentions_any(self, path, names)
    }

    fn with_change_workspace(&mut self, f: &mut dyn FnMut(&mut dyn ChangeWorkspace)) {
        let mut workspace = crate::edit::LegacyWorkspace {
            session: self.session,
            collection: self.collection,
        };
        f(&mut workspace);
    }
}

/// The problems of a status (Dart `RefactoringStatus.problems`).
pub fn problems(status: &RefactoringStatus) -> Vec<protocol::RefactoringProblem> {
    status
        .problems
        .iter()
        .map(|p| protocol::RefactoringProblem {
            severity: match p.severity {
                Severity::Info => protocol::RefactoringProblemSeverity::INFO,
                Severity::Warning => protocol::RefactoringProblemSeverity::WARNING,
                Severity::Error => protocol::RefactoringProblemSeverity::ERROR,
                Severity::Fatal => protocol::RefactoringProblemSeverity::FATAL,
            },
            message: p.message.clone(),
            location: None,
        })
        .collect()
}

/// A change of the refactorings as a protocol change.
pub fn change(change: &dartr_server::correction::change::SourceChange) -> protocol::SourceChange {
    use dartr_server::correction::change::LinkedEditSuggestionKind;
    protocol::SourceChange {
        message: change.message.clone(),
        edits: change
            .edits
            .iter()
            .map(|f| protocol::SourceFileEdit {
                file: f.file.clone(),
                file_stamp: f.file_stamp,
                edits: f
                    .edits
                    .iter()
                    .map(|e| protocol::SourceEdit {
                        offset: e.offset as i64,
                        length: e.length as i64,
                        replacement: e.replacement.clone(),
                        id: None,
                        description: None,
                    })
                    .collect(),
            })
            .collect(),
        linked_edit_groups: change
            .linked_edit_groups
            .iter()
            .map(|g| protocol::LinkedEditGroup {
                positions: g
                    .positions
                    .iter()
                    .map(|p| protocol::Position {
                        file: p.file.clone(),
                        offset: p.offset as i64,
                    })
                    .collect(),
                length: g.length as i64,
                suggestions: g
                    .suggestions
                    .iter()
                    .map(|s| protocol::LinkedEditSuggestion {
                        value: s.value.clone(),
                        kind: match s.kind {
                            LinkedEditSuggestionKind::Method => {
                                protocol::LinkedEditSuggestionKind::METHOD
                            }
                            LinkedEditSuggestionKind::Parameter => {
                                protocol::LinkedEditSuggestionKind::PARAMETER
                            }
                            LinkedEditSuggestionKind::Type => {
                                protocol::LinkedEditSuggestionKind::TYPE
                            }
                            LinkedEditSuggestionKind::Variable => {
                                protocol::LinkedEditSuggestionKind::VARIABLE
                            }
                        },
                    })
                    .collect(),
            })
            .collect(),
        selection: change.selection.as_ref().map(|p| protocol::Position {
            file: p.file.clone(),
            offset: p.offset as i64,
        }),
        selection_length: change.selection_length.map(|l| l as i64),
        id: change.id.clone(),
    }
}
