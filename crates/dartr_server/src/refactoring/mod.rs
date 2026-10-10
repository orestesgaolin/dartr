// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/

//! The refactorings of Dart's legacy refactoring framework: the status of
//! a refactoring and the naming conventions. The refactorings themselves
//! are in [`crate::server::rename`] and [`crate::server::refactor`].

pub mod host;
pub mod convert;
pub mod naming;
pub mod rename;
pub mod status;

use crate::correction::change::SourceChange;
use crate::server::{ResolvedUnitRef, SElem};
use rename::Engine;
use status::RefactoringStatus;

/// Dart `RefactoringKind` of the legacy refactorings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefactoringKind {
    ConvertGetterToMethod,
    ConvertMethodToGetter,
    ExtractLocalVariable,
    ExtractMethod,
    ExtractWidget,
    InlineLocalVariable,
    InlineMethod,
    MoveFile,
    Rename,
}

impl RefactoringKind {
    /// Dart `RefactoringKind.values.byName`.
    pub fn by_name(name: &str) -> Option<RefactoringKind> {
        Some(match name {
            "CONVERT_GETTER_TO_METHOD" => RefactoringKind::ConvertGetterToMethod,
            "CONVERT_METHOD_TO_GETTER" => RefactoringKind::ConvertMethodToGetter,
            "EXTRACT_LOCAL_VARIABLE" => RefactoringKind::ExtractLocalVariable,
            "EXTRACT_METHOD" => RefactoringKind::ExtractMethod,
            "EXTRACT_WIDGET" => RefactoringKind::ExtractWidget,
            "INLINE_LOCAL_VARIABLE" => RefactoringKind::InlineLocalVariable,
            "INLINE_METHOD" => RefactoringKind::InlineMethod,
            "MOVE_FILE" => RefactoringKind::MoveFile,
            "RENAME" => RefactoringKind::Rename,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            RefactoringKind::ConvertGetterToMethod => "CONVERT_GETTER_TO_METHOD",
            RefactoringKind::ConvertMethodToGetter => "CONVERT_METHOD_TO_GETTER",
            RefactoringKind::ExtractLocalVariable => "EXTRACT_LOCAL_VARIABLE",
            RefactoringKind::ExtractMethod => "EXTRACT_METHOD",
            RefactoringKind::ExtractWidget => "EXTRACT_WIDGET",
            RefactoringKind::InlineLocalVariable => "INLINE_LOCAL_VARIABLE",
            RefactoringKind::InlineMethod => "INLINE_METHOD",
            RefactoringKind::MoveFile => "MOVE_FILE",
            RefactoringKind::Rename => "RENAME",
        }
    }
}

/// A legacy refactoring of a selection (Dart `Refactoring`), created for
/// the commands `refactor.perform` and `refactor.validate` and the code
/// actions.
pub enum Refactoring {
    ConvertGetterToMethod(SElem),
    ConvertMethodToGetter(SElem),
}

impl Engine<'_> {
    /// Dart `Refactoring.checkInitialConditions`.
    pub fn initial_conditions(&mut self, r: &Refactoring) -> RefactoringStatus {
        match r {
            Refactoring::ConvertGetterToMethod(e) => self.getter_to_method_initial(e),
            Refactoring::ConvertMethodToGetter(e) => self.method_to_getter_initial(e),
        }
    }

    /// Dart `Refactoring.checkFinalConditions`.
    pub fn final_conditions(&mut self, r: &Refactoring) -> RefactoringStatus {
        match r {
            Refactoring::ConvertGetterToMethod(e) => self.getter_to_method_final(e),
            Refactoring::ConvertMethodToGetter(_) => RefactoringStatus::new(),
        }
    }

    /// Dart `Refactoring.checkAllConditions`.
    pub fn all_conditions(&mut self, r: &Refactoring) -> RefactoringStatus {
        let mut result = self.initial_conditions(r);
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.final_conditions(r));
        result
    }

    /// Dart `Refactoring.createChange`.
    pub fn change(&mut self, r: &Refactoring) -> SourceChange {
        match r {
            Refactoring::ConvertGetterToMethod(e) => self.getter_to_method_change(e),
            Refactoring::ConvertMethodToGetter(e) => self.method_to_getter_change(e),
        }
    }

    /// The getter at [offset] (Dart `nodeCovering(offset)?.getElement()` is
    /// a `GetterElement`).
    pub fn getter_at(&mut self, resolved: &ResolvedUnitRef, offset: u32) -> Option<SElem> {
        self.element_at(resolved, offset).filter(|e| e.id.tag() == dartr_element::Tag::Getter)
    }

    /// The executable at [offset] (an `ExecutableElement`).
    pub fn executable_at(&mut self, resolved: &ResolvedUnitRef, offset: u32) -> Option<SElem> {
        self.element_at(resolved, offset)
            .filter(|e| crate::element_locator::is_executable(e.id))
    }
}
