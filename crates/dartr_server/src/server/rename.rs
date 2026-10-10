// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_rename.dart (PrepareRenameHandler, RenameHandler)

//! `textDocument/prepareRename` and `textDocument/rename` with the rename
//! refactorings of [`crate::refactoring::rename`]; the server is the
//! [`RefactoringHost`] of the refactorings.

use std::sync::Arc;

use serde_json::{Value, json};

use super::code_actions::ServerWorkspace;
use super::{Pending, ResolvedUnitRef, Server};
use crate::correction::change_builder::ChangeWorkspace;
use crate::index::ElementKey;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::refactoring::host::RefactoringHost;
use crate::refactoring::rename::Engine;
use crate::refactoring::status::RefactoringStatus;
use crate::server::{Match, SElem, SearchScope};

/// Dart `UserPromptActions`.
const RENAME_ANYWAY: &str = "Rename Anyway";
const CANCEL: &str = "Cancel";

impl RefactoringHost for Server {
    fn collection(&self) -> Option<&dartr_project::AnalysisContextCollection> {
        self.collection.as_ref()
    }

    fn resolved_unit_in(&mut self, path: &str, context: Option<usize>) -> Option<ResolvedUnitRef> {
        self.require_resolved_unit_in(path, context).ok()
    }

    fn search_references(&mut self, element: &SElem) -> Vec<Match> {
        Server::search_references(self, element)
    }

    fn hierarchy_members_and_parameters(&mut self, member: &SElem) -> (Vec<SElem>, Vec<SElem>) {
        Server::hierarchy_members_and_parameters(self, member)
    }

    fn hierarchy_named_parameters(&mut self, parameter: &SElem) -> Vec<SElem> {
        Server::hierarchy_named_parameters(self, parameter)
    }

    fn append_all_subtypes(
        &mut self,
        class: &SElem,
        all: &mut Vec<SElem>,
        keys: &mut Vec<(usize, ElementKey)>,
    ) {
        Server::append_all_subtypes(self, class, all, keys)
    }

    fn search_scope(&mut self) -> Arc<SearchScope> {
        Server::search_scope(self)
    }

    fn mentions_any(&mut self, path: &str, names: &[String]) -> bool {
        Server::mentions_any(self, path, names)
    }

    fn with_change_workspace(&mut self, f: &mut dyn FnMut(&mut dyn ChangeWorkspace)) {
        let Some(collection) = self.collection.as_ref() else {
            return;
        };
        let mut workspace = ServerWorkspace {
            session: &mut self.session,
            collection,
            overlays: &self.overlays,
        };
        f(&mut workspace);
    }
}

impl Server {
    // ---------------------------------------------------------------------
    // Handlers.

    /// Dart `PrepareRenameHandler.handle`.
    pub(crate) fn prepare_rename(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let mut engine = Engine { host: self };
        let Some((target, name_offset, name_length)) = engine.element_to_rename(&resolved, offset)
        else {
            return Ok(Value::Null);
        };
        let Some(mut rename) = engine.create_rename(&resolved, target) else {
            return Ok(Value::Null);
        };
        let status = engine.check_initial_conditions(&mut rename);
        if status.has_fatal_error() {
            return Err(ResponseError::new(
                codes::RENAME_NOT_VALID,
                status.message().unwrap_or("").to_string(),
            ));
        }
        let start = if name_offset == u32::MAX {
            offset
        } else {
            name_offset
        };
        Ok(json!({
            "range": mapping::to_range(&line_info, start, name_length),
            "placeholder": rename.old_name,
        }))
    }

    /// Dart `RenameHandler.handle`. Returns `None` when the response is
    /// sent after the answer of the client to a prompt.
    pub(crate) fn rename(&mut self, id: &Value, params: &Value) -> Option<ErrorOr<Value>> {
        Some(match self.rename_inner(id, params) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => return None,
            Err(e) => Err(e),
        })
    }

    fn rename_inner(&mut self, id: &Value, params: &Value) -> ErrorOr<Option<Value>> {
        if !super::is_dart_document(params) {
            return Ok(Some(Value::Null));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let new_name = params
            .get("newName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut engine = Engine { host: self };
        let Some((target, _, _)) = engine.element_to_rename(&resolved, offset) else {
            return Ok(Some(Value::Null));
        };
        let Some(mut rename) = engine.create_rename(&resolved, target) else {
            return Ok(Some(Value::Null));
        };
        let not_valid = |s: &RefactoringStatus| {
            ResponseError::new(
                codes::RENAME_NOT_VALID,
                s.message().unwrap_or("").to_string(),
            )
        };
        let initial = engine.check_initial_conditions(&mut rename);
        if initial.has_fatal_error() {
            return Err(not_valid(&initial));
        }
        rename.new_name = new_name;
        let options = engine.check_new_name(&rename);
        if options.has_error() {
            return Err(not_valid(&options));
        }
        let final_status = engine.check_final_conditions(&mut rename);
        if final_status.has_fatal_error() {
            return Err(not_valid(&final_status));
        }
        let change = engine.create_change(&mut rename);
        let edit = self.create_workspace_edit(&change, false, &path, &line_info);
        if final_status.has_error() || final_status.has_warning() {
            if !self
                .client_flag("/window/showMessage/messageActionItem/additionalPropertiesSupport")
                && self.client.raw.pointer("/window/showMessage").is_none()
            {
                return Err(not_valid(&final_status));
            }
            self.send_request(
                "window/showMessageRequest",
                json!({
                    "type": 2,
                    "message": final_status.message().unwrap_or(""),
                    "actions": [{"title": RENAME_ANYWAY}, {"title": CANCEL}],
                }),
                Pending::RenamePrompt {
                    request_id: id.clone(),
                    edit,
                },
            );
            return Ok(None);
        }
        Ok(Some(edit))
    }

    /// The answer of the client to the prompt of a rename with warnings.
    pub(crate) fn rename_prompt_response(
        &mut self,
        request_id: &Value,
        edit: Value,
        message: &Value,
    ) {
        let choice = message
            .pointer("/result/title")
            .and_then(Value::as_str)
            .unwrap_or("");
        let result = if choice == RENAME_ANYWAY {
            edit
        } else {
            // Dart `emptyWorkspaceEdit`.
            json!({})
        };
        self.send_response(request_id, Ok(result));
    }
}
