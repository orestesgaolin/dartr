// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_rename.dart (PrepareRenameHandler, RenameHandler)

//! `textDocument/prepareRename` and `textDocument/rename` with the rename
//! refactorings of [`crate::refactoring::rename`]; the server is the
//! [`RefactoringHost`] of the refactorings.

use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Value, json};

use super::code_actions::ServerWorkspace;
use super::{Pending, ResolvedUnitRef, Server};
use crate::correction::change_builder::ChangeWorkspace;
use crate::index::ElementKey;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::refactoring::host::RefactoringHost;
use crate::refactoring::move_file::MoveFile;
use crate::refactoring::naming::to_file_name;
use crate::refactoring::rename::{Engine, Target};
use crate::refactoring::status::RefactoringStatus;
use crate::server::{Match, SElem, SearchScope};
use crate::uri::path_to_uri;

/// Dart `UserPromptActions`.
const RENAME_ANYWAY: &str = "Rename Anyway";
const CANCEL: &str = "Cancel";
const YES: &str = "Yes";
const NO: &str = "No";

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
        let client_version = params
            .pointer("/textDocument/version")
            .and_then(Value::as_i64)
            .or_else(|| self.overlays.get(&path).and_then(|d| d.version));
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
        if self.file_has_been_modified(&path, client_version) {
            return Err(ResponseError::new(
                codes::CONTENT_MODIFIED,
                "Document was modified before operation completed",
            ));
        }
        let mut edit = self.create_workspace_edit(&change, false, &path, &line_info);
        let mut class_file_rename: Option<(String, String, Value)> = None;
        if self.client_supports_rename_file()
            && rename.kind == crate::refactoring::rename::Kind::UnitMember
            && let Target::Element(elem) = &rename.target
            && elem.id.cast::<dartr_element::InterfaceElement>().is_some()
            && let Some(declaring_file) =
                elem.with(|ctx| crate::refactoring::rename::element_path(ctx, elem.id))
        {
            let folder = dartr_project::paths::dirname(&declaring_file);
            let actual_filename = dartr_project::paths::basename(&declaring_file).to_string();
            let old_computed_filename = to_file_name(&rename.old_name);
            let new_filename = to_file_name(&rename.new_name);
            if actual_filename == old_computed_filename {
                let new_path = dartr_project::paths::join(folder, &new_filename);
                let rename_op = json!({
                    "kind": "rename",
                    "oldUri": path_to_uri(&declaring_file),
                    "newUri": path_to_uri(&new_path),
                });
                let rename_config = self.client_configuration.rename_files_with_classes();
                if rename_config == "always" {
                    append_document_change(&mut edit, rename_op);
                } else if rename_config == "prompt" && self.supports_show_message_request() {
                    class_file_rename = Some((actual_filename, new_filename, rename_op));
                }
            }
        }
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
                    class_file_rename,
                },
            );
            return Ok(None);
        }
        if let Some((old_filename, new_filename, rename_op)) = class_file_rename {
            self.send_request(
                "window/showMessageRequest",
                json!({
                    "type": 3,
                    "message": format!("Rename '{old_filename}' to '{new_filename}'?"),
                    "actions": [{"title": YES}, {"title": NO}],
                }),
                Pending::RenameClassFile {
                    request_id: id.clone(),
                    edit,
                    rename_op,
                },
            );
            return Ok(None);
        }
        Ok(Some(edit))
    }

    fn client_supports_rename_file(&self) -> bool {
        self.client_flag("/workspace/workspaceEdit/documentChanges")
            && self
                .client
                .raw
                .pointer("/workspace/workspaceEdit/resourceOperations")
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("rename")))
    }

    /// The answer of the client to the prompt of a rename with warnings.
    pub(crate) fn rename_prompt_response(
        &mut self,
        request_id: &Value,
        edit: Value,
        class_file_rename: Option<(String, String, Value)>,
        message: &Value,
    ) {
        let choice = message
            .pointer("/result/title")
            .and_then(Value::as_str)
            .unwrap_or("");
        if choice != RENAME_ANYWAY {
            // Dart `emptyWorkspaceEdit`.
            self.send_response(request_id, Ok(json!({})));
            return;
        }
        if let Some((old_filename, new_filename, rename_op)) = class_file_rename {
            self.send_request(
                "window/showMessageRequest",
                json!({
                    "type": 3,
                    "message": format!("Rename '{old_filename}' to '{new_filename}'?"),
                    "actions": [{"title": YES}, {"title": NO}],
                }),
                Pending::RenameClassFile {
                    request_id: request_id.clone(),
                    edit,
                    rename_op,
                },
            );
            return;
        }
        self.send_response(request_id, Ok(edit));
    }

    /// The answer of the client to the prompt of renaming a class file.
    pub(crate) fn rename_class_file_response(
        &mut self,
        request_id: &Value,
        mut edit: Value,
        rename_op: Value,
        message: &Value,
    ) {
        let choice = message
            .pointer("/result/title")
            .and_then(Value::as_str)
            .unwrap_or("");
        if choice == YES {
            append_document_change(&mut edit, rename_op);
        }
        self.send_response(request_id, Ok(edit));
    }

    /// Dart `WillRenameFilesHandler.handle`.
    pub(crate) fn will_rename_files(&mut self, params: &Value) -> ErrorOr<Value> {
        let files = params
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| super::invalid_params("workspace/willRenameFiles"))?;
        let mut rename_mapping = IndexMap::new();
        for file in files {
            let old_uri = file
                .get("oldUri")
                .and_then(Value::as_str)
                .ok_or_else(|| super::invalid_params("workspace/willRenameFiles"))?;
            let new_uri = file
                .get("newUri")
                .and_then(Value::as_str)
                .ok_or_else(|| super::invalid_params("workspace/willRenameFiles"))?;
            let old_path = self.path_of_uri(old_uri)?;
            let new_path = self.path_of_uri(new_uri)?;
            rename_mapping.insert(old_path, new_path);
        }
        let refactoring = MoveFile::new_multi(rename_mapping);
        let status = refactoring.check_all_conditions(self);
        if status.has_fatal_error() {
            return Ok(Value::Null);
        }
        let change = refactoring.create_change(self);
        let dummy_line_info = dartr_syntax::LineInfo::from_content("");
        Ok(self.create_workspace_edit(&change, false, "", &dummy_line_info))
    }
}

fn append_document_change(edit: &mut Value, op: Value) {
    if let Some(changes) = edit
        .get_mut("documentChanges")
        .and_then(Value::as_array_mut)
    {
        changes.push(op);
    } else {
        edit["documentChanges"] = json!([op]);
    }
}
