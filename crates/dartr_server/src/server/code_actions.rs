// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_code_actions.dart (CodeActionHandler)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/code_actions/code_action_computer.dart (CodeActionComputer, _CodeActionSorter)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/code_actions/abstract_code_actions_producer.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/code_actions/dart.dart (DartCodeActionsProducer: getFixActions, getSourceActions)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_execute_command.dart (ExecuteCommandHandler)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/commands/organize_imports.dart, sort_members.dart, simple_edit_handler.dart, log_action.dart, send_workspace_edit.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (toCodeActionKind, createWorkspaceEdit, createPlainWorkspaceEdit, toWorkspaceEdit, toTextDocumentEdit, toSnippetTextEdits, toLegacySnippetTextEdits)
// Dart source: pkg/analysis_server/lib/src/lsp/snippets.dart (buildSnippetStringForEditGroups, SnippetBuilder)

//! `textDocument/codeAction` (source actions and fixes) and
//! `workspace/executeCommand` (the commands of the source actions).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dartr_cli::DriverSession;
use dartr_cli::provider::AnalyzedFile;
use dartr_cli::server::processed_diagnostics;
use dartr_diagnostics::{Diagnostic, DiagnosticType};
use dartr_element::NoopSink;
use dartr_project::{AnalysisContextCollection, AnalysisOptions};
use dartr_syntax::LineInfo;
use serde_json::{Map, Value, json};

use super::{Document, Pending, ResolvedUnitRef, Server};
use crate::correction::change::{
    LinkedEditGroup, SourceChange, SourceEdit, SourceFileEdit, utf16_len,
};
use crate::correction::change_builder::{ChangeWorkspace, TopLevelDeclaration};
use crate::correction::fix_processor::{FixUnit, compute_fixes};
use crate::correction::organize_imports::{ImportOrganizer, Resolution};
use crate::correction::producers::import_library::{element_kind, exported_element};
use crate::correction::sort_members::MemberSorter;
use crate::correction::utils::CorrectionUtils;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::uri::path_to_uri;

/// Dart `Commands`.
pub mod commands {
    pub const APPLY_CODE_ACTION: &str = "dart.edit.codeAction.apply";
    pub const SORT_MEMBERS: &str = "dart.edit.sortMembers";
    pub const ORGANIZE_IMPORTS: &str = "dart.edit.organizeImports";
    #[allow(dead_code)]
    pub const FIX_ALL: &str = "dart.edit.fixAll";
    pub const SEND_WORKSPACE_EDIT: &str = "dart.edit.sendWorkspaceEdit";
    pub const LOG_ACTION: &str = "dart.logAction";

    /// The commands that dartr executes (a subset of Dart
    /// `Commands.serverSupportedCommands`, in Dart order).
    pub const SUPPORTED: &[&str] = &[
        SORT_MEMBERS,
        ORGANIZE_IMPORTS,
        SEND_WORKSPACE_EDIT,
        LOG_ACTION,
    ];
}

/// The code action kinds of `CodeActionOptions` (Dart
/// `DartCodeActionKind.serverSupportedKinds`).
pub const SERVER_SUPPORTED_KINDS: &[&str] = &[
    "source",
    "source.organizeImports",
    "source.fixAll",
    "source.sortMembers",
    "quickfix",
    "refactor",
];

/// The workspace of the change builder: resolved units from the driver
/// session, options from the collection, contents from the overlays.
pub(crate) struct ServerWorkspace<'s> {
    pub session: &'s mut DriverSession,
    pub collection: &'s AnalysisContextCollection,
    pub overlays: &'s HashMap<String, Document>,
}

impl ChangeWorkspace for ServerWorkspace<'_> {
    fn resolved_unit(&mut self, path: &str) -> Option<ResolvedUnitRef> {
        if !path.ends_with(".dart") || self.collection.context_for(path).is_none() {
            return None;
        }
        let library = self.session.resolved_library(self.collection, path)?;
        let index = library.unit_index(path)?;
        Some(ResolvedUnitRef { library, index })
    }

    fn analysis_options(&self, path: &str) -> Rc<AnalysisOptions> {
        options_for(self.collection, path)
    }

    fn content(&self, path: &str) -> Option<String> {
        match self.overlays.get(path) {
            Some(d) => Some(d.content.clone()),
            None => super::read_file(path),
        }
    }

    fn fix_data_files(&self, path: &str) -> Vec<(String, Option<String>)> {
        let Some(context) = self.collection.context_for(path) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for package in context.packages.packages() {
            out.push((
                format!("{}/fix_data.yaml", package.lib),
                Some(package.name.clone()),
            ));
            let mut files = Vec::new();
            yaml_files_recursively(&format!("{}/fix_data", package.lib), &mut files);
            out.extend(files.into_iter().map(|f| (f, Some(package.name.clone()))));
        }
        if let Some(sdk) = context.sdk.as_ref().or(self.collection.sdk.as_ref()) {
            out.push((format!("{}/_internal/fix_data.yaml", sdk.lib_path()), None));
        }
        out
    }

    fn top_level_declarations(&mut self, path: &str, name: &str) -> Vec<TopLevelDeclaration> {
        let collection = self.collection;
        let Some(context) = collection.context_for(path) else {
            return Vec::new();
        };
        let index = collection
            .contexts
            .iter()
            .position(|x| std::ptr::eq(x, context))
            .unwrap_or(0);
        let filter = FileFilter::new(context, path);
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for candidate in available_files(self.session, context, index) {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let Some(uri) = file_uri(context, &candidate) else {
                continue;
            };
            if !filter.should_include(&candidate, &uri) {
                continue;
            }
            // Dart `getLibraryByUri`: only libraries.
            let Some(linked) = self
                .session
                .linked_library_in(collection, index, &candidate)
            else {
                continue;
            };
            if linked.library_path != candidate {
                continue;
            }
            let sink = NoopSink;
            let features = dartr_element::FeatureSet::new(Vec::<std::sync::Arc<str>>::new());
            let ctx = linked.ctx(&sink, &features);
            let Some(library) = ctx.library_by_uri(&linked.uri) else {
                continue;
            };
            let element = exported_element(&ctx, library, name)
                .or_else(|| exported_element(&ctx, library, &format!("{name}=")));
            let Some(mut element) = element else { continue };
            if matches!(
                element.tag(),
                dartr_element::Tag::Getter | dartr_element::Tag::Setter
            ) {
                if let Some(v) =
                    dartr_resolver::element_metadata::accessor_variable_any(&ctx, element)
                {
                    element = v;
                }
            }
            let declared_in_library =
                dartr_resolver::error::support::library_of(&ctx, element) == Some(library);
            result.push(TopLevelDeclaration {
                library_uri: linked.uri.clone(),
                library_path: linked.library_path.clone(),
                kind: element_kind(element),
                declared_in_library,
            });
        }
        result
    }
}

/// The yaml files under [folder] (Dart `_loadTransforms` order).
fn yaml_files_recursively(folder: &str, out: &mut Vec<String>) {
    let Some(children) = dartr_project::fs::children(folder) else {
        return;
    };
    for child in children {
        match child.kind {
            dartr_project::fs::ResourceKind::File => {
                if child.path.ends_with(".yaml") {
                    out.push(child.path);
                }
            }
            dartr_project::fs::ResourceKind::Folder => yaml_files_recursively(&child.path, out),
        }
    }
}

/// Dart `AnalysisDriver.discoverAvailableFiles` and `knownFiles`: the files
/// that the driver knows, then the SDK libraries, then the Dart files of
/// the `lib` folders of the packages.
fn available_files(
    session: &DriverSession,
    context: &dartr_project::AnalysisContext,
    index: usize,
) -> Vec<String> {
    let mut out = session.known_files(index);
    let added: Vec<String> = context
        .root
        .analyzed_files()
        .into_iter()
        .filter(|f| f.ends_with(".dart"))
        .collect();
    out.extend(added);
    if let Some(sdk) = context.sdk.as_ref() {
        out.extend(
            sdk.libraries()
                .iter()
                .filter_map(|l| sdk.map_dart_uri(&l.short_name)),
        );
    }
    for package in context.packages.packages() {
        dart_files_recursively(&package.lib, &mut out);
    }
    out
}

/// The Dart files under [folder], in directory order.
fn dart_files_recursively(folder: &str, out: &mut Vec<String>) {
    let Some(children) = dartr_project::fs::children(folder) else {
        return;
    };
    for child in children {
        match child.kind {
            dartr_project::fs::ResourceKind::File => {
                if child.path.ends_with(".dart") {
                    out.push(child.path);
                }
            }
            dartr_project::fs::ResourceKind::Folder => dart_files_recursively(&child.path, out),
        }
    }
}

/// The URI of [path] in [context]: `dart:`, `package:` or `file:`.
fn file_uri(context: &dartr_project::AnalysisContext, path: &str) -> Option<String> {
    if let Some(sdk) = context.sdk.as_ref() {
        for library in sdk.libraries() {
            if sdk.map_dart_uri(&library.short_name).as_deref() == Some(path) {
                return Some(library.short_name.clone());
            }
        }
    }
    if let Some(uri) = context.packages.path_to_package_uri(path) {
        return Some(uri);
    }
    Some(path_to_uri(path))
}

/// Dart `FileStateFilter` (`_PubFilter` for a file in a pub package,
/// `_AnyFilter` otherwise).
struct FileFilter {
    /// The root of the pub package of the target file, its name and
    /// dependencies; `None` for `_AnyFilter`.
    package: Option<(String, Option<String>, HashSet<String>)>,
    in_lib_or_entry_point: bool,
}

impl FileFilter {
    fn new(context: &dartr_project::AnalysisContext, path: &str) -> FileFilter {
        let _ = context;
        // The nearest folder with a pubspec.yaml.
        let mut folder = path
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default();
        loop {
            let pubspec = format!("{folder}/pubspec.yaml");
            if let Some(text) = super::read_file(&pubspec) {
                let in_lib = ["lib", "bin", "web"]
                    .iter()
                    .any(|f| dartr_project::paths::is_within(&format!("{folder}/{f}"), path));
                let (name, dependencies) = pubspec_dependencies(&text, in_lib);
                return FileFilter {
                    package: Some((folder, name, dependencies)),
                    in_lib_or_entry_point: in_lib,
                };
            }
            match folder.rsplit_once('/') {
                Some((parent, _)) if !parent.is_empty() => folder = parent.to_string(),
                _ => break,
            }
        }
        FileFilter {
            package: None,
            in_lib_or_entry_point: false,
        }
    }

    /// Dart `shouldInclude`.
    fn should_include(&self, path: &str, uri: &str) -> bool {
        if let Some(rest) = uri.strip_prefix("dart:") {
            // Dart `shouldIncludeSdk`.
            if rest.starts_with('_') {
                return false;
            }
            return !matches!(
                uri,
                "dart:html"
                    | "dart:indexed_db"
                    | "dart:js"
                    | "dart:js_util"
                    | "dart:svg"
                    | "dart:web_audio"
                    | "dart:web_gl"
            );
        }
        let Some((root, name, dependencies)) = &self.package else {
            return true;
        };
        let Some(rest) = uri.strip_prefix("package:") else {
            if self.in_lib_or_entry_point {
                return false;
            }
            return dartr_project::paths::is_within(root, path);
        };
        let package_name = rest.split('/').next().unwrap_or("");
        if Some(package_name) == name.as_deref() {
            return true;
        }
        let is_src = rest.split('/').nth(1) == Some("src");
        if is_src {
            let friend = matches!(name.as_deref(), Some("analysis_server") | Some("linter"));
            return friend && package_name == "analyzer";
        }
        dependencies.contains(package_name)
    }
}

/// The name of a pubspec and its dependencies (with dev dependencies
/// when [in_lib] is false).
fn pubspec_dependencies(text: &str, in_lib: bool) -> (Option<String>, HashSet<String>) {
    let mut name = None;
    let mut dependencies = HashSet::new();
    let Ok(node) = dartr_project::yaml::load_yaml_node(text) else {
        return (name, dependencies);
    };
    let Some(entries) = node.as_map() else {
        return (name, dependencies);
    };
    for (key, value) in entries {
        let Some(key) = key.scalar().map(|k| k.to_dart_string()) else {
            continue;
        };
        match key.as_str() {
            "name" => name = value.scalar().map(|s| s.to_dart_string()),
            "dependencies" | "dev_dependencies" => {
                if key == "dev_dependencies" && in_lib {
                    continue;
                }
                if let Some(map) = value.as_map() {
                    for (k, _) in map {
                        if let Some(k) = k.scalar() {
                            dependencies.insert(k.to_dart_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (name, dependencies)
}

/// The analysis options of [path].
pub(crate) fn options_for(
    collection: &AnalysisContextCollection,
    path: &str,
) -> Rc<AnalysisOptions> {
    match collection.context_for(path) {
        Some(context) => collection.options_for(context, path).clone(),
        None => Rc::new(AnalysisOptions::default()),
    }
}

/// A code action with the priority of its fix (Dart
/// `CodeActionWithPriority`).
struct ActionWithPriority {
    action: Value,
    priority: i64,
}

/// Dart `toCodeActionKind`.
fn to_code_action_kind(id: Option<&str>, fallback: &str) -> String {
    let Some(id) = id else {
        return fallback.to_string();
    };
    let new_id = id
        .replace("dart.assist", "refactor")
        .replace("dart.fix", "quickfix")
        .replace("analysisOptions.assist", "refactor")
        .replace("analysisOptions.fix", "quickfix")
        .replace("pubspec.assist", "refactor")
        .replace("pubspec.fix", "quickfix");
    if !new_id.starts_with("refactor") && !new_id.starts_with("quickfix") {
        return format!("{fallback}.{new_id}");
    }
    new_id
}

/// The kind filters of a request (Dart `CodeActionComputer.only` and
/// `supportedKinds`).
struct KindFilter {
    only: Option<Vec<String>>,
    supported: Option<Vec<String>>,
}

impl KindFilter {
    /// Dart `shouldIncludeAnyOfKind`.
    fn include_any_of_kind(&self, kind: &str) -> bool {
        match &self.only {
            Some(only) => only
                .iter()
                .any(|wanted| wanted == kind || wanted.starts_with(&format!("{kind}."))),
            None => true,
        }
    }

    /// Dart `shouldIncludeKind`.
    fn include_kind(&self, kind: &str) -> bool {
        let is_match = |wanted: &String| kind == wanted || kind.starts_with(&format!("{wanted}."));
        if let Some(only) = &self.only {
            return only.iter().any(is_match);
        }
        if let Some(supported) = &self.supported {
            return supported.iter().any(is_match);
        }
        true
    }
}

impl Server {
    /// Dart `getVersionedDocumentIdentifier`.
    fn versioned_document(&self, path: &str) -> Value {
        let version = self.overlays.get(path).and_then(|d| d.version);
        json!({"uri": path_to_uri(path), "version": version})
    }

    fn client_flag(&self, pointer: &str) -> bool {
        self.client
            .raw
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Dart `LspClientCapabilities.applyEdit`.
    fn editor_supports_apply_edit(&self) -> bool {
        self.client_flag("/workspace/applyEdit")
    }

    /// The processed diagnostics of the Dart file [path] (Dart
    /// `unitResult.diagnostics`): the engine diagnostics and their
    /// severities after the `errors:` processors.
    fn unit_diagnostics(&mut self, path: &str) -> Vec<Diagnostic> {
        let Some(collection) = &self.collection else {
            return Vec::new();
        };
        let context = collection
            .context_for(path)
            .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))
            .unwrap_or(0);
        let files = [AnalyzedFile {
            path: path.to_string(),
            context,
        }];
        let results = self.session.diagnostics(collection, &files);
        let mut out = Vec::new();
        let mut lints = Vec::new();
        for file in &results {
            if file.path != path {
                continue;
            }
            for (d, severity) in processed_diagnostics(collection, file) {
                let mut d = d.clone();
                d.severity = severity;
                if d.code.diagnostic_type == DiagnosticType::Lint {
                    lints.push(d);
                } else {
                    out.push(d);
                }
            }
        }
        // Dart reports the lints after the other diagnostics, in the order
        // of one traversal of the unit (the rules of a node in name order);
        // the offset order is close to it.
        lints.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.code.name.cmp(b.code.name)));
        out.extend(lints);
        out
    }

    /// Dart `CodeActionHandler.handle` and `CodeActionComputer.compute`.
    pub(crate) fn code_action(&mut self, params: &Value) -> ErrorOr<Value> {
        let path = self.path_of_doc(params)?;
        let uri = params
            .pointer("/textDocument/uri")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !self.is_analyzed(&path) || !uri.starts_with("file:") {
            return Ok(json!([]));
        }
        let literal = self
            .client
            .raw
            .pointer("/textDocument/codeAction/codeActionLiteralSupport")
            .is_some();
        let supported = if literal {
            Some(
                self.client_formats(
                    "/textDocument/codeAction/codeActionLiteralSupport/codeActionKind/valueSet",
                )
                .unwrap_or_default(),
            )
        } else {
            None
        };
        let only = params
            .pointer("/context/only")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            });
        let auto_triggered = params
            .pointer("/context/triggerKind")
            .and_then(Value::as_i64)
            == Some(2);
        let filter = KindFilter { only, supported };
        let range = params.get("range").cloned().unwrap_or(Value::Null);
        let is_dart = path.ends_with(".dart");
        let resolved = if is_dart {
            self.require_resolved_unit(&path).ok()
        } else {
            None
        };
        let line_info = match &resolved {
            Some(r) => r.line_info().clone(),
            None => match self.line_info_of(&path) {
                Some(l) => l,
                None => return Ok(json!([])),
            },
        };
        let start = range.get("start").and_then(mapping::read_position);
        let end = range.get("end").and_then(mapping::read_position);
        let (Some(start), Some(end)) = (start, end) else {
            return Ok(json!([]));
        };
        let (Ok(start_offset), Ok(end_offset)) = (
            mapping::to_offset(&line_info, start.0, start.1, false),
            mapping::to_offset(&line_info, end.0, end.1, false),
        ) else {
            return Ok(json!([]));
        };
        let mut actions: Vec<Value> = Vec::new();
        let Some(resolved) = resolved else {
            return Ok(json!([]));
        };
        if filter.include_any_of_kind("source") && self.editor_supports_apply_edit() {
            let create_action = |kind: &str, title: &str, command: &str| {
                let mut argument = json!({"path": path});
                if auto_triggered {
                    argument["autoTriggered"] = json!(true);
                }
                let command = json!({"title": title, "command": command, "arguments": [argument]});
                if literal {
                    json!({"title": title, "kind": kind, "command": command})
                } else {
                    command
                }
            };
            if filter.include_kind("source.sortMembers") {
                actions.push(create_action(
                    "source.sortMembers",
                    "Sort Members",
                    commands::SORT_MEMBERS,
                ));
            }
            if filter.include_kind("source.organizeImports") {
                actions.push(create_action(
                    "source.organizeImports",
                    "Organize Imports",
                    commands::ORGANIZE_IMPORTS,
                ));
            }
        }
        if filter.include_any_of_kind("quickfix") {
            let fixes = self.fix_actions(&path, &resolved, &range, &filter, literal);
            actions.extend(sort_actions(fixes, &range));
        }
        if filter.include_any_of_kind("refactor") {
            let assists = self.assist_actions(
                &path,
                &resolved,
                &range,
                start_offset,
                end_offset.saturating_sub(start_offset),
                &filter,
                literal,
            );
            actions.extend(sort_actions(assists, &range));
        }
        Ok(Value::Array(actions))
    }

    /// Dart `DartCodeActionsProducer.getFixActions`.
    fn fix_actions(
        &mut self,
        path: &str,
        resolved: &ResolvedUnitRef,
        range: &Value,
        filter: &KindFilter,
        literal: bool,
    ) -> Vec<ActionWithPriority> {
        let diagnostics = self.unit_diagnostics(path);
        let Some(collection) = &self.collection else {
            return Vec::new();
        };
        let options = options_for(collection, path);
        let diagnostic_options = self.client.diagnostic_options();
        let resolved = Rc::new(ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        });
        let line_info = resolved.line_info().clone();
        let range_start_line = range
            .pointer("/start/line")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let range_end_line = range
            .pointer("/end/line")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let utils = CorrectionUtils::new(&unit.ast, &line_info);
        // Dart processes the diagnostics with their default severity; the
        // LSP diagnostic has the processed severity.
        let engine: Vec<Diagnostic> = diagnostics
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d.severity = d.code.severity();
                d
            })
            .collect();
        let fix_unit = FixUnit {
            resolved: &resolved,
            ctx: &ctx,
            utils: &utils,
            path,
            options: &options,
            diagnostics: &engine,
        };
        let overlays = &self.overlays;
        let mut workspace = ServerWorkspace {
            session: &mut self.session,
            collection,
            overlays,
        };
        let mut already_calculated: HashSet<String> = HashSet::new();
        let mut actions = Vec::new();
        let mut pending: Vec<(usize, Vec<crate::correction::fix_processor::Fix>)> = Vec::new();
        for (index, d) in engine.iter().enumerate() {
            let start_line = line_info.get_location(d.offset as u32).line_number - 1;
            let end_line = line_info
                .get_location((d.offset + d.length) as u32)
                .line_number
                - 1;
            if range_end_line < start_line || range_start_line > end_line {
                continue;
            }
            let fixes = compute_fixes(&fix_unit, d, &mut workspace, Some(&mut already_calculated));
            if !fixes.is_empty() {
                pending.push((index, fixes));
            }
        }
        drop(workspace);
        for (index, fixes) in pending {
            let d = &diagnostics[index];
            let lsp_diagnostic =
                mapping::to_diagnostic(&line_info, path, d, &diagnostic_options, &|f| {
                    super::read_file(f).map(|c| LineInfo::from_content(&c))
                });
            for fix in fixes {
                let change = &fix.change;
                let kind = to_code_action_kind(change.id.as_deref(), "quickfix");
                if !filter.include_kind(&kind) {
                    continue;
                }
                let action = self.change_action(
                    change,
                    &kind,
                    path,
                    range,
                    &line_info,
                    literal,
                    Some(lsp_diagnostic.clone()),
                );
                actions.push(ActionWithPriority {
                    action,
                    priority: fix.kind.priority,
                });
            }
        }
        actions
    }

    /// Dart `createCodeActionLiteralOrApplyCommand`.
    #[allow(clippy::too_many_arguments)]
    fn change_action(
        &self,
        change: &crate::correction::change::SourceChange,
        kind: &str,
        path: &str,
        range: &Value,
        line_info: &LineInfo,
        literal: bool,
        diagnostic: Option<Value>,
    ) -> Value {
        if literal {
            let mut m = Map::new();
            m.insert("title".into(), json!(change.message));
            m.insert("kind".into(), json!(kind));
            // Dart `createCodeActionLiteral`: `diagnostics: [?diagnostic]`.
            m.insert(
                "diagnostics".into(),
                json!(diagnostic.map(|d| vec![d]).unwrap_or_default()),
            );
            if let Some(id) = &change.id {
                m.insert(
                    "command".into(),
                    json!({"command": commands::LOG_ACTION, "title": "Log Action", "arguments": [{"action": id}]}),
                );
            }
            m.insert(
                "edit".into(),
                self.create_workspace_edit(change, true, path, line_info),
            );
            Value::Object(m)
        } else {
            json!({
                "title": change.message,
                "command": commands::APPLY_CODE_ACTION,
                "arguments": [{
                    "textDocument": self.versioned_document(path),
                    "range": range,
                    "kind": kind,
                    "loggedAction": change.id,
                }],
            })
        }
    }

    /// Dart `DartCodeActionsProducer.getAssistActions`.
    #[allow(clippy::too_many_arguments)]
    fn assist_actions(
        &mut self,
        path: &str,
        resolved: &ResolvedUnitRef,
        range: &Value,
        offset: u32,
        length: u32,
        filter: &KindFilter,
        literal: bool,
    ) -> Vec<ActionWithPriority> {
        let diagnostics = self.unit_diagnostics(path);
        let Some(collection) = &self.collection else {
            return Vec::new();
        };
        let options = options_for(collection, path);
        let resolved = Rc::new(ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        });
        let line_info = resolved.line_info().clone();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let utils = CorrectionUtils::new(&unit.ast, &line_info);
        let fix_unit = FixUnit {
            resolved: &resolved,
            ctx: &ctx,
            utils: &utils,
            path,
            options: &options,
            diagnostics: &diagnostics,
        };
        let overlays = &self.overlays;
        let mut workspace = ServerWorkspace {
            session: &mut self.session,
            collection,
            overlays,
        };
        let assists = crate::correction::assist_processor::compute_assists(
            &fix_unit,
            offset,
            length,
            &mut workspace,
        );
        drop(workspace);
        let mut actions = Vec::new();
        for assist in assists {
            let change = &assist.change;
            let kind = to_code_action_kind(change.id.as_deref(), "refactor");
            if !filter.include_kind(&kind) {
                continue;
            }
            let action = self.change_action(change, &kind, path, range, &line_info, literal, None);
            actions.push(ActionWithPriority {
                action,
                priority: assist.kind.priority,
            });
        }
        actions
    }

    /// The line info of [file] for edits (Dart `server.getLineInfo`).
    fn edit_line_info(&self, file: &str, file_path: &str, line_info: &LineInfo) -> LineInfo {
        if file == file_path {
            return line_info.clone();
        }
        self.line_info_of(file)
            .unwrap_or_else(|| LineInfo::from_content(""))
    }

    /// Dart `createWorkspaceEdit`.
    pub(crate) fn create_workspace_edit(
        &self,
        change: &SourceChange,
        allow_snippets: bool,
        file_path: &str,
        line_info: &LineInfo,
    ) -> Value {
        let legacy = self.client_flag("/experimental/snippetTextEdit");
        let snippet = self.client_flag("/workspace/workspaceEdit/snippetEditSupport");
        let document_changes = self.client_flag("/workspace/workspaceEdit/documentChanges");
        let single = change.edits.len() == 1
            && change.edits[0].file_stamp != -1
            && change.edits[0].file == file_path
            && change.edits[0].edits.len() == 1;
        if !allow_snippets
            || (!legacy && !snippet)
            || !document_changes
            || !single
            || (change.selection.is_none() && change.linked_edit_groups.is_empty())
        {
            return self.create_plain_workspace_edit(&change.edits, file_path, line_info);
        }
        let file_edit = &change.edits[0];
        let edits = to_snippet_text_edits(
            file_edit,
            &change.linked_edit_groups,
            line_info,
            change.selection.as_ref().map(|p| p.offset),
            change.selection_length,
            snippet,
        );
        json!({"documentChanges": [{
            "textDocument": self.versioned_document(&file_edit.file),
            "edits": edits,
        }]})
    }

    /// Dart `createPlainWorkspaceEdit` and `toWorkspaceEdit`.
    pub(crate) fn create_plain_workspace_edit(
        &self,
        edits: &[SourceFileEdit],
        file_path: &str,
        line_info: &LineInfo,
    ) -> Value {
        let document_changes = self.client_flag("/workspace/workspaceEdit/documentChanges");
        let supports_create = self
            .client
            .raw
            .pointer("/workspace/workspaceEdit/resourceOperations")
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("create")));
        let text_edits = |e: &SourceFileEdit| -> Vec<Value> {
            let li = if e.file_stamp == -1 {
                LineInfo::from_content("")
            } else {
                self.edit_line_info(&e.file, file_path, line_info)
            };
            e.edits
                .iter()
                .rev()
                .map(|edit| json!({"range": mapping::to_range(&li, edit.offset, edit.length), "newText": edit.replacement}))
                .collect()
        };
        if document_changes {
            let mut changes = Vec::new();
            for e in edits {
                if supports_create && e.file_stamp == -1 {
                    changes.push(json!({"kind": "create", "uri": path_to_uri(&e.file)}));
                }
                changes.push(json!({"textDocument": self.versioned_document(&e.file), "edits": text_edits(e)}));
            }
            json!({"documentChanges": changes})
        } else {
            let mut map = Map::new();
            for e in edits {
                map.insert(path_to_uri(&e.file), Value::Array(text_edits(e)));
            }
            json!({"changes": map})
        }
    }

    // ---------------------------------------------------------------------
    // workspace/executeCommand

    /// Dart `ExecuteCommandHandler.handle`. Returns `None` when the response
    /// is sent later (after `workspace/applyEdit`).
    pub(crate) fn execute_command(&mut self, id: &Value, params: &Value) -> Option<ErrorOr<Value>> {
        let command = params.get("command").and_then(Value::as_str).unwrap_or("");
        if !commands::SUPPORTED.contains(&command) {
            return Some(Err(ResponseError::new(
                codes::UNKNOWN_COMMAND,
                format!("{command} is not a valid command identifier"),
            )));
        }
        let arguments = params
            .get("arguments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let parameters = if arguments.is_empty() {
            json!({})
        } else if arguments.len() == 1 && arguments[0].is_object() {
            arguments[0].clone()
        } else {
            return Some(Err(ResponseError::new(
                codes::INVALID_COMMAND_ARGUMENTS,
                format!("{command} requires a single Map argument"),
            )));
        };
        match command {
            commands::LOG_ACTION => Some(Ok(Value::Null)),
            commands::ORGANIZE_IMPORTS => self.organize_imports_command(id, &parameters),
            commands::SORT_MEMBERS => self.sort_members_command(id, &parameters),
            commands::SEND_WORKSPACE_EDIT => {
                let Some(edit) = parameters.get("edit").filter(|e| e.is_object()).cloned() else {
                    return Some(Err(ResponseError::new(
                        codes::INVALID_COMMAND_ARGUMENTS,
                        "Send Workspace Edit requires a Map argument containing an \"edit\"",
                    )));
                };
                self.send_workspace_edit_to_client(id, "Send Workspace Edit", edit)
            }
            _ => Some(Ok(Value::Null)),
        }
    }

    /// Dart `OrganizeImportsCommandHandler.handle`.
    fn organize_imports_command(
        &mut self,
        id: &Value,
        parameters: &Value,
    ) -> Option<ErrorOr<Value>> {
        const NAME: &str = "Organize Imports";
        let Some(path) = parameters
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Some(Err(ResponseError::new(
                codes::INVALID_COMMAND_ARGUMENTS,
                format!("{NAME} requires a Map argument containing a \"path\""),
            )));
        };
        let doc = self.versioned_document(&path);
        let auto_triggered = parameters
            .get("autoTriggered")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let resolved = match self.require_resolved_unit(&path) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        let diagnostics = self.unit_diagnostics(&path);
        if diagnostics
            .iter()
            .any(|d| d.code.diagnostic_type == DiagnosticType::SyntacticError)
        {
            if auto_triggered {
                return Some(Ok(Value::Null));
            }
            return Some(Err(ResponseError::with_data(
                codes::FILE_HAS_ERRORS,
                format!("Unable to {NAME} because the file contains parse errors"),
                &path,
            )));
        }
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let organizer = ImportOrganizer {
            resolution: Some(Resolution {
                ctx: &ctx,
                tables: &unit.tables,
                fragment: unit.fragment,
            }),
            ast: &unit.ast,
            unit: unit.unit,
            line_info: resolved.line_info(),
            diagnostics: &diagnostics,
            remove_unused: true,
        };
        let edits = organizer.organize();
        let line_info = resolved.line_info().clone();
        self.send_source_edits_to_client(id, NAME, doc, &line_info, edits)
    }

    /// Dart `SortMembersCommandHandler.handle`.
    fn sort_members_command(&mut self, id: &Value, parameters: &Value) -> Option<ErrorOr<Value>> {
        const NAME: &str = "Sort Members";
        let Some(path) = parameters
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Some(Err(ResponseError::new(
                codes::INVALID_COMMAND_ARGUMENTS,
                format!("{NAME} requires a Map argument containing a \"path\""),
            )));
        };
        let doc = self.versioned_document(&path);
        let auto_triggered = parameters
            .get("autoTriggered")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // Dart `getAnalysisSession(path)`: the context whose root contains
        // the file (also an excluded file).
        let in_context = self.collection.as_ref().is_some_and(|c| {
            c.contexts
                .iter()
                .any(|x| dartr_project::paths::is_within(&x.root.root, &path))
        });
        let file = if path.ends_with(".dart") && in_context {
            self.parsed_unit(&path)
        } else {
            None
        };
        let Some(file) = file else {
            if auto_triggered {
                return Some(Ok(Value::Null));
            }
            return Some(Err(ResponseError::new(
                codes::FILE_NOT_ANALYZED,
                format!("{NAME} is only available for analyzed files"),
            )));
        };
        if !file.unit.diagnostics.is_empty() {
            if auto_triggered {
                return Some(Ok(Value::Null));
            }
            return Some(Err(ResponseError::with_data(
                codes::FILE_HAS_ERRORS,
                format!("Unable to {NAME} because the file contains parse errors"),
                &path,
            )));
        }
        let sort_constructors_first = self
            .collection
            .as_ref()
            .map(|c| options_for(c, &path))
            .is_some_and(|o| o.lint_rules.iter().any(|r| r == "sort_constructors_first"));
        let ast = &file.unit.ast;
        let unit = file.unit.unit;
        let line_info = file.unit.line_info.clone();
        let edits = MemberSorter::new(ast, unit, &line_info, sort_constructors_first).sort();
        self.send_source_edits_to_client(id, NAME, doc, &line_info, edits)
    }

    /// Dart `SimpleEditCommandHandler.sendSourceEditsToClient`.
    fn send_source_edits_to_client(
        &mut self,
        id: &Value,
        name: &str,
        doc: Value,
        line_info: &LineInfo,
        edits: Vec<SourceEdit>,
    ) -> Option<ErrorOr<Value>> {
        if edits.is_empty() {
            return Some(Ok(Value::Null));
        }
        let document_changes = self.client_flag("/workspace/workspaceEdit/documentChanges");
        let text_edits: Vec<Value> = edits
            .iter()
            .rev()
            .map(|e| json!({"range": mapping::to_range(line_info, e.offset, e.length), "newText": e.replacement}))
            .collect();
        let edit = if document_changes {
            json!({"documentChanges": [{"textDocument": doc, "edits": text_edits}]})
        } else {
            let mut map = Map::new();
            map.insert(
                doc["uri"].as_str().unwrap_or("").to_string(),
                Value::Array(text_edits),
            );
            json!({"changes": map})
        };
        self.send_workspace_edit_to_client(id, name, edit)
    }

    /// Dart `sendWorkspaceEditToClient`: sends `workspace/applyEdit`; the
    /// response of the command is sent when the client answers.
    fn send_workspace_edit_to_client(
        &mut self,
        id: &Value,
        name: &str,
        edit: Value,
    ) -> Option<ErrorOr<Value>> {
        self.send_request(
            "workspace/applyEdit",
            json!({"label": name, "edit": edit.clone()}),
            Pending::ApplyEdit {
                request_id: id.clone(),
                command_name: name.to_string(),
                edit,
            },
        );
        None
    }

    /// The response of the client to `workspace/applyEdit` (the end of
    /// Dart `sendWorkspaceEditToClient`).
    pub(crate) fn apply_edit_response(
        &mut self,
        request_id: &Value,
        command_name: &str,
        edit: &Value,
        message: &Value,
    ) {
        let result = if let Some(error) = message.get("error") {
            Err(ResponseError::with_data(
                codes::CLIENT_FAILED_TO_APPLY_EDIT,
                format!("Client failed to apply workspace edit for {command_name}"),
                error.to_string(),
            ))
        } else {
            let result = message.get("result").cloned().unwrap_or(Value::Null);
            let applied = result
                .get("applied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            match result.get("failureReason").and_then(Value::as_str) {
                Some(reason) if !applied => Err(ResponseError::with_data(
                    codes::CLIENT_FAILED_TO_APPLY_EDIT,
                    format!(
                        "Client failed to apply workspace edit for {command_name} (reason: {reason})"
                    ),
                    edit.to_string(),
                )),
                _ => Ok(Value::Null),
            }
        };
        self.send_response(request_id, result);
    }
}

/// Dart `_CodeActionSorter.sort`: dedupes actions by title (keeping the
/// one nearest the start of [range], merging the diagnostics of the same
/// edits) and sorts by priority (stable).
fn sort_actions(actions: Vec<ActionWithPriority>, range: &Value) -> Vec<Value> {
    let start_character = range
        .pointer("/start/character")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    // groupBy keeps the order of the first occurrence of each title.
    let mut groups: Vec<(String, Vec<ActionWithPriority>)> = Vec::new();
    for a in actions {
        let title = a
            .action
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match groups.iter_mut().find(|(t, _)| *t == title) {
            Some((_, list)) => list.push(a),
            None => groups.push((title, vec![a])),
        }
    }
    let pos_of = |a: &Value| -> i64 {
        a.get("diagnostics")
            .and_then(Value::as_array)
            .and_then(|d| d.first())
            .and_then(|d| d.pointer("/range/start/character"))
            .and_then(Value::as_i64)
            .unwrap_or(start_character)
    };
    let mut deduped: Vec<ActionWithPriority> = Vec::new();
    for (_, mut list) in groups {
        if list.len() == 1 {
            deduped.push(list.pop().unwrap());
            continue;
        }
        // Dart `List.sort` with the column distance (merge sort is stable
        // for short lists).
        list.sort_by_key(|a| (pos_of(&a.action) - start_character).abs());
        let first = list.remove(0);
        let is_literal = first.action.get("kind").is_some() || first.action.get("edit").is_some();
        if !is_literal {
            deduped.push(first);
            continue;
        }
        let mut merged = first.action.clone();
        let mut diagnostics: Vec<Value> = merged
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for other in &list {
            let same = match first.action.get("edit") {
                Some(edit) => other.action.get("edit") == Some(edit),
                None => {
                    first.action.get("command").is_some()
                        && first.action.get("command") == other.action.get("command")
                }
            };
            if same {
                if let Some(d) = other.action.get("diagnostics").and_then(Value::as_array) {
                    diagnostics.extend(d.iter().cloned());
                }
            }
        }
        merged["diagnostics"] = Value::Array(diagnostics);
        deduped.push(ActionWithPriority {
            action: merged,
            priority: first.priority,
        });
    }
    let mut indexed: Vec<(usize, ActionWithPriority)> = deduped.into_iter().enumerate().collect();
    indexed.sort_by(|(ia, a), (ib, b)| b.priority.cmp(&a.priority).then(ia.cmp(ib)));
    indexed.into_iter().map(|(_, a)| a.action).collect()
}

/// Dart `toSnippetTextEdits` / `toLegacySnippetTextEdits`.
fn to_snippet_text_edits(
    file_edit: &SourceFileEdit,
    groups: &[LinkedEditGroup],
    line_info: &LineInfo,
    selection_offset: Option<u32>,
    selection_length: Option<u32>,
    snippet_edit_support: bool,
) -> Vec<Value> {
    let mut out = Vec::new();
    let mut offset_delta: i64 = 0;
    for edit in file_edit.edits.iter().rev() {
        let edit_offset = edit.offset as i64 + offset_delta;
        let text = build_snippet_string_for_edit_groups(
            &edit.replacement,
            &file_edit.file,
            groups,
            edit_offset,
            selection_offset.map(|s| s as i64 - edit_offset),
            selection_length,
        );
        let range = mapping::to_range(line_info, edit.offset, edit.length);
        out.push(if snippet_edit_support {
            json!({"range": range, "snippet": {"kind": "snippet", "value": text}})
        } else {
            json!({"insertTextFormat": 2, "range": range, "newText": text})
        });
        offset_delta += utf16_len(&edit.replacement) as i64 - edit.length as i64;
    }
    out.reverse();
    out
}

struct Placeholder {
    offset: i64,
    length: i64,
    suggestions: Option<Vec<String>>,
    linked_group_id: Option<usize>,
    is_final: bool,
}

/// Dart `buildSnippetStringForEditGroups` (`_buildSnippetString`).
fn build_snippet_string_for_edit_groups(
    text: &str,
    file_path: &str,
    groups: &[LinkedEditGroup],
    edit_groups_offset: i64,
    selection_offset: Option<i64>,
    selection_length: Option<u32>,
) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut placeholders: Vec<Placeholder> = Vec::new();
    if let Some(s) = selection_offset {
        placeholders.push(Placeholder {
            offset: s,
            length: selection_length.unwrap_or(0) as i64,
            suggestions: None,
            linked_group_id: None,
            is_final: true,
        });
    }
    for (index, group) in groups.iter().enumerate() {
        for position in group.positions.iter().filter(|p| p.file == file_path) {
            placeholders.push(Placeholder {
                offset: position.offset as i64 - edit_groups_offset,
                length: group.length as i64,
                suggestions: Some(group.suggestions.iter().map(|s| s.value.clone()).collect()),
                linked_group_id: Some(index),
                is_final: selection_offset.is_none()
                    && groups.len() == 1
                    && group.suggestions.len() <= 1,
            });
        }
    }
    placeholders.retain(|p| p.offset >= 0 && p.offset + p.length <= units.len() as i64);
    if !groups.is_empty() {
        placeholders.sort_by_key(|p| p.offset);
    }
    let mut builder = SnippetBuilder::default();
    let mut ids: HashMap<usize, u32> = HashMap::new();
    let mut offset = 0usize;
    let slice = |a: usize, b: usize| String::from_utf16_lossy(&units[a..b]);
    for p in &placeholders {
        let start = p.offset as usize;
        let end = (p.offset + p.length) as usize;
        builder.append_text(&slice(offset.min(start), start));
        let mut number: Option<u32> = if p.is_final {
            Some(0)
        } else {
            p.linked_group_id.and_then(|g| ids.get(&g).copied())
        };
        let placeholder_text = slice(start, end);
        number = Some(match &p.suggestions {
            None => builder.append_placeholder(&placeholder_text, number),
            Some(suggestions) => {
                let mut choices: Vec<String> = Vec::new();
                if !placeholder_text.is_empty() {
                    choices.push(placeholder_text.clone());
                }
                for s in suggestions {
                    if !choices.contains(s) {
                        choices.push(s.clone());
                    }
                }
                builder.append_choice(&choices, number)
            }
        });
        offset = end;
        if let Some(g) = p.linked_group_id {
            ids.insert(g, number.unwrap());
        }
    }
    builder.append_text(&slice(offset.min(units.len()), units.len()));
    builder.buffer
}

/// Dart `SnippetBuilder`.
#[derive(Default)]
struct SnippetBuilder {
    buffer: String,
    next_placeholder: u32,
}

impl SnippetBuilder {
    fn use_placeholder_number(&mut self, number: Option<u32>) -> u32 {
        if self.next_placeholder == 0 {
            self.next_placeholder = 1;
        }
        let number = number.unwrap_or(self.next_placeholder);
        self.next_placeholder = self.next_placeholder.max(number + 1);
        number
    }

    fn append_choice(&mut self, choices: &[String], number: Option<u32>) -> u32 {
        if choices.len() <= 1 {
            return self
                .append_placeholder(choices.first().map(String::as_str).unwrap_or(""), number);
        }
        let number = if number == Some(0) { None } else { number };
        let number = self.use_placeholder_number(number);
        let escaped: Vec<String> = choices
            .iter()
            .map(|c| escape(c, &['\\', '|', ',']))
            .collect();
        self.buffer
            .push_str(&format!("${{{number}|{}|}}", escaped.join(",")));
        number
    }

    fn append_placeholder(&mut self, text: &str, number: Option<u32>) -> u32 {
        if text.is_empty() {
            return self.append_tab_stop(number);
        }
        let number = self.use_placeholder_number(number);
        self.buffer.push_str(&format!(
            "${{{number}:{}}}",
            escape(text, &['$', '}', '\\'])
        ));
        number
    }

    fn append_tab_stop(&mut self, number: Option<u32>) -> u32 {
        let number = self.use_placeholder_number(number);
        self.buffer.push_str(&format!("${number}"));
        number
    }

    fn append_text(&mut self, text: &str) {
        self.buffer.push_str(&escape(text, &['$', '\\']));
    }
}

fn escape(s: &str, chars: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if chars.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
