// Dart source: pkg/analysis_server/lib/src/lsp/handlers/code_actions/dart.dart (DartCodeActionsProducer.getRefactorActions, createRefactor)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/commands/abstract_refactor.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/commands/perform_refactor.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/commands/validate_refactor.dart

//! The refactorings of the legacy framework in LSP: the `refactor.*` code
//! actions and the commands `refactor.perform` and `refactor.validate`.

use serde_json::{Value, json};

use super::code_actions::{KindFilter, commands};
use super::{Pending, ResolvedUnitRef, Server};
use crate::mapping::{ErrorOr, ResponseError, codes};
use crate::refactoring::rename::Engine;
use crate::refactoring::{Refactoring, RefactoringKind};

/// Dart `UserPromptActions`.
const REFACTOR_ANYWAY: &str = "Refactor Anyway";
const CANCEL: &str = "Cancel";

/// Dart `ErrorCodes.ContentModified`.
const CONTENT_MODIFIED: i64 = -32801;

impl Server {
    /// Dart `DartCodeActionsProducer.getRefactorActions` (the legacy
    /// refactorings).
    pub(crate) fn refactor_actions(
        &mut self,
        path: &str,
        resolved: &ResolvedUnitRef,
        offset: u32,
        length: u32,
        filter: &KindFilter,
        literal: bool,
    ) -> Vec<Value> {
        if !self.editor_supports_apply_edit() {
            return Vec::new();
        }
        let version = self.versioned_document(path)["version"].clone();
        let create = |kind: &str, title: &str, refactoring: RefactoringKind| {
            let command = json!({
                "title": title,
                "command": commands::PERFORM_REFACTOR,
                "arguments": [refactoring.name(), path, version, offset, length, Value::Null],
            });
            if literal {
                json!({"title": title, "kind": kind, "command": command})
            } else {
                command
            }
        };
        let mut actions = Vec::new();
        if filter.include_kind("refactor.rewrite") {
            let mut engine = Engine { host: self };
            let getter = engine
                .getter_at(resolved, offset)
                .filter(|g| !engine.getter_to_method_initial(g).has_fatal_error());
            let executable = engine
                .executable_at(resolved, offset)
                .filter(|e| !engine.method_to_getter_initial(e).has_fatal_error());
            if getter.is_some() {
                actions.push(create(
                    "refactor.rewrite",
                    "Convert Getter to Method",
                    RefactoringKind::ConvertGetterToMethod,
                ));
            }
            if executable.is_some() {
                actions.push(create(
                    "refactor.rewrite",
                    "Convert Method to Getter",
                    RefactoringKind::ConvertMethodToGetter,
                ));
            }
        }
        actions
    }

    /// Dart `AbstractRefactorCommandHandler.handle` with
    /// `PerformRefactorCommandHandler.execute` or
    /// `ValidateRefactorCommandHandler.execute`. Returns `None` when the
    /// response is sent later (after a prompt or `workspace/applyEdit`).
    pub(crate) fn refactor_command(
        &mut self,
        id: &Value,
        parameters: &Value,
        perform: bool,
        progress_token: Option<Value>,
    ) -> Option<ErrorOr<Value>> {
        let command_name = if perform { "Perform Refactor" } else { "Validate Refactor" };
        let kind = parameters.get("kind").and_then(Value::as_str);
        let path = parameters.get("path").and_then(Value::as_str);
        let doc_version = parameters.get("docVersion");
        let offset = parameters.get("offset").and_then(Value::as_u64);
        let length = parameters.get("length").and_then(Value::as_u64);
        let options = parameters.get("options");
        let valid_version = doc_version.is_none_or(|v| v.is_null() || v.is_i64());
        let valid_options = options.is_none_or(|o| o.is_null() || o.is_object());
        let (Some(kind), Some(path), Some(offset), Some(length)) = (kind, path, offset, length) else {
            return Some(Err(invalid_arguments(command_name)));
        };
        if !valid_version || !valid_options {
            return Some(Err(invalid_arguments(command_name)));
        }
        let (kind, path) = (kind.to_string(), path.to_string());
        let doc_version = doc_version.and_then(Value::as_i64);
        let resolved = match self.require_resolved_unit(&path) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        // `getRefactoring`.
        let refactoring = {
            let mut engine = Engine { host: self };
            let offset = offset as u32;
            match RefactoringKind::by_name(&kind) {
                Some(RefactoringKind::ConvertGetterToMethod) => match engine.getter_at(&resolved, offset) {
                    Some(g) => Ok(Refactoring::ConvertGetterToMethod(g)),
                    None => Err(no_longer_valid(command_name, &kind)),
                },
                Some(RefactoringKind::ConvertMethodToGetter) => match engine.executable_at(&resolved, offset) {
                    Some(e) => Ok(Refactoring::ConvertMethodToGetter(e)),
                    None => Err(no_longer_valid(command_name, &kind)),
                },
                _ => Err(ResponseError::new(
                    codes::INVALID_COMMAND_ARGUMENTS,
                    format!("Unknown RefactoringKind {kind} was supplied to {command_name}"),
                )),
            }
        };
        let _ = length;
        let refactoring = match refactoring {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        let progress = self.begin_command_progress(
            progress_token,
            if perform { "Refactoring…" } else { "Preparing Refactor…" },
        );
        if !perform {
            let status = Engine { host: self }.initial_conditions(&refactoring);
            self.end_command_progress(progress);
            if status.has_fatal_error() {
                return Some(Ok(json!({"valid": false, "message": status.message().unwrap_or("")})));
            }
            return Some(Ok(json!({"valid": true})));
        }
        let mut engine = Engine { host: self };
        let status = engine.all_conditions(&refactoring);
        let change = if status.has_fatal_error() {
            None
        } else {
            Some(engine.change(&refactoring))
        };
        let warning = status.has_error() || status.has_warning();
        if warning && (status.has_fatal_error() || !self.supports_show_message_request()) {
            // Dart `showErrorMessageToUser`.
            self.end_command_progress(progress);
            self.send_notification(
                "window/showMessage",
                json!({"type": 1, "message": status.message().unwrap_or("")}),
            );
            return Some(Ok(Value::Null));
        }
        let Some(change) = change else {
            self.end_command_progress(progress);
            return Some(Ok(Value::Null));
        };
        if change.edits.is_empty() {
            self.end_command_progress(progress);
            return Some(Ok(Value::Null));
        }
        if self.file_has_been_modified(&path, doc_version) {
            self.end_command_progress(progress);
            return Some(Err(ResponseError::new(
                CONTENT_MODIFIED,
                "Document was modified before operation completed",
            )));
        }
        let line_info = resolved.line_info().clone();
        let edit = self.create_workspace_edit(&change, false, &path, &line_info);
        self.end_command_progress(progress);
        if warning {
            let mut message = status.message().unwrap_or("").to_string();
            let others = status.problems.len() - 1;
            if others > 0 {
                message.push_str(&if others == 1 {
                    " (+ 1 other error)".to_string()
                } else {
                    format!(" (+ {others} other errors)")
                });
            }
            self.send_request(
                "window/showMessageRequest",
                json!({
                    "type": 2,
                    "message": message,
                    "actions": [{"title": REFACTOR_ANYWAY}, {"title": CANCEL}],
                }),
                Pending::RefactorPrompt {
                    request_id: id.clone(),
                    edit,
                },
            );
            return None;
        }
        self.send_workspace_edit_to_client(id, command_name, edit)
    }

    /// The answer of the client to the prompt of a refactoring with
    /// problems.
    pub(crate) fn refactor_prompt_response(&mut self, request_id: &Value, edit: Value, message: &Value) {
        let choice = message.pointer("/result/title").and_then(Value::as_str).unwrap_or("");
        if choice == REFACTOR_ANYWAY {
            if let Some(result) = self.send_workspace_edit_to_client(request_id, "Perform Refactor", edit) {
                self.send_response(request_id, result);
            }
        } else {
            self.send_response(request_id, Ok(Value::Null));
        }
    }

    /// Dart `supportsShowMessageRequest`.
    fn supports_show_message_request(&self) -> bool {
        self.client.raw.pointer("/window/showMessage").is_some()
    }

    /// Dart `fileHasBeenModified(path, docVersion)`.
    fn file_has_been_modified(&self, path: &str, version: Option<i64>) -> bool {
        let Some(version) = version else { return false };
        self.overlays.get(path).and_then(|d| d.version) != Some(version)
    }

    /// The progress of a command (Dart `ProgressReporter`): the token of
    /// the client, else a progress that the server creates when the client
    /// supports it.
    fn begin_command_progress(&mut self, token: Option<Value>, title: &str) -> Option<Value> {
        let token = match token {
            Some(t) => t,
            None => {
                if !self.client_flag("/window/workDoneProgress") {
                    return None;
                }
                let token = json!(format!("dartr-progress-{}", self.next_request_id));
                self.send_request(
                    "window/workDoneProgress/create",
                    json!({"token": token}),
                    Pending::Unregistration,
                );
                token
            }
        };
        self.send_notification(
            "$/progress",
            json!({"token": token, "value": {"kind": "begin", "title": title}}),
        );
        Some(token)
    }

    fn end_command_progress(&mut self, token: Option<Value>) {
        if let Some(token) = token {
            self.send_notification("$/progress", json!({"token": token, "value": {"kind": "end"}}));
        }
    }
}

fn invalid_arguments(command_name: &str) -> ResponseError {
    ResponseError::new(
        codes::INVALID_COMMAND_ARGUMENTS,
        format!(
            "{command_name} requires 6 parameters: kind: String (RefactoringKind), filePath: String, \
             docVersion: int?, offset: int, length: int, options: Map<String, Object?>"
        ),
    )
}

fn no_longer_valid(command_name: &str, kind: &str) -> ResponseError {
    ResponseError::new(
        codes::INVALID_COMMAND_ARGUMENTS,
        format!("Location supplied to {command_name} {kind} is no longer valid"),
    )
}
