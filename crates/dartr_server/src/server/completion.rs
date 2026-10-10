// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_completion.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_completion_resolve.dart

//! `textDocument/completion` and `completionItem/resolve`.

#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, NoopSink, Tag};
use dartr_typesystem::member;
use serde_json::{Map, Value, json};

use super::Server;
use crate::completion::candidate::Kind;
use crate::completion::lsp::{ItemCapabilities, ItemContext};
use crate::completion::target::TokenExt;
use crate::completion::{self as c, CodeStyle, KnownLibrary, RequestInputs};
use crate::mapping::{ErrorOr, to_range};

/// Dart `maxDocSizeForInlining`.
const MAX_DOC_SIZE_FOR_INLINING: usize = 25;

impl Server {
    fn bool_cap(&self, pointer: &str) -> bool {
        self.client.raw.pointer(pointer).and_then(Value::as_bool).unwrap_or(false)
    }

    fn int_set_cap(&self, pointer: &str) -> Option<Vec<i64>> {
        self.client
            .raw
            .pointer(pointer)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
    }

    /// The completion capabilities of the client (Dart
    /// `LspClientCapabilities`).
    fn completion_capabilities(&self) -> ItemCapabilities {
        let item = "/textDocument/completion/completionItem";
        let defaults: Vec<String> = self
            .client
            .raw
            .pointer("/textDocument/completion/completionList/itemDefaults")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        ItemCapabilities {
            snippets: self.bool_cap(&format!("{item}/snippetSupport")),
            insert_replace: self.bool_cap(&format!("{item}/insertReplaceSupport")),
            deprecated_flag: self.bool_cap(&format!("{item}/deprecatedSupport")),
            deprecated_tag: self
                .int_set_cap(&format!("{item}/tagSupport/valueSet"))
                .is_some_and(|s| s.contains(&1)),
            label_details: self.bool_cap(&format!("{item}/labelDetailsSupport")),
            as_is_insert_mode: self
                .int_set_cap(&format!("{item}/insertTextModeSupport/valueSet"))
                .is_some_and(|s| s.contains(&1)),
            item_kinds: self
                .int_set_cap("/textDocument/completion/completionItemKind/valueSet")
                .unwrap_or_else(|| vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]),
            documentation_formats: self.client_formats(&format!("{item}/documentationFormat")),
            default_edit_range: defaults.iter().any(|d| d == "editRange"),
            default_text_mode: defaults.iter().any(|d| d == "insertTextMode"),
            default_data: self.bool_cap("/textDocument/completion/completionList/applyKindSupport"),
        }
    }

    /// The code style options of [path] (Dart `CodeStyleOptions`).
    fn code_style(&self, path: &str) -> CodeStyle {
        let Some(collection) = &self.collection else {
            return CodeStyle::default();
        };
        let Some(context) = collection.context_for(path) else {
            return CodeStyle::default();
        };
        let options = collection.options_for(context, path);
        let has = |name: &str| options.lint_rules.iter().any(|r| r == name);
        let quote = if has("prefer_single_quotes") {
            '\''
        } else if has("prefer_double_quotes") {
            '"'
        } else {
            '\''
        };
        CodeStyle {
            specify_types: has("always_specify_types"),
            make_locals_final: has("prefer_final_locals"),
            quote,
        }
    }

    /// Dart `CompletionHandler.handle`.
    pub(crate) fn completion(&mut self, params: &Value) -> ErrorOr<Value> {
        let path = self.path_of_doc(params)?;
        if !path.ends_with(".dart") {
            return Ok(json!({"isIncomplete": false, "items": []}));
        }
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let trigger = params
            .pointer("/context/triggerCharacter")
            .and_then(Value::as_str)
            .map(str::to_string);
        let caps = self.completion_capabilities();
        let config = self.client_configuration.for_resource(&path);
        let max_items = config.max_completion_items();
        let enable_snippets = config.enable_snippets();
        let global = self.client_configuration.global();
        let complete_function_calls_setting = global.complete_function_calls();
        let commit_characters = global.preview_commit_characters();
        let documentation_preference = global.preferred_documentation();
        let apply_edit = self.bool_cap("/workspace/applyEdit");
        let use_not_imported = self.init.suggest_from_unimported_libraries && apply_edit;
        let budget_ms = self.init.completion_budget_ms.unwrap_or(100);
        let templates = self.dartdoc_templates(&path);
        let style = self.code_style(&path);

        // The package of the file and the SDK and packages of its context.
        let context_index = resolved.library.context;
        let (package, sdk_libraries, packages) = match &self.collection {
            Some(collection) => {
                let context = &collection.contexts[context_index];
                let package = match context.root.workspace.find_package_for(&path) {
                    Some(dartr_project::workspace::WorkspacePackage::Pub { root, name, .. }) => Some((root, name)),
                    _ => None,
                };
                let sdk = context.sdk.as_ref().or(collection.sdk.as_ref());
                let sdk_libraries = sdk
                    .map(|s| {
                        s.libraries()
                            .iter()
                            .map(|l| (l.short_name.clone(), l.is_internal(), l.implementation))
                            .collect()
                    })
                    .unwrap_or_default();
                let packages = context
                    .packages
                    .packages()
                    .iter()
                    .map(|p| (p.name.clone(), p.lib.clone()))
                    .collect();
                (package, sdk_libraries, packages)
            }
            None => (None, Vec::new(), Vec::new()),
        };

        // The libraries of the not-imported pass (Dart `knownFiles`).
        let known = if use_not_imported {
            match &self.collection {
                Some(collection) => self.session.link_known_libraries(collection, context_index).map(|(world, libraries)| {
                    let filter = c::FileFilter::new(package.clone(), &path);
                    let workspace = collection.contexts[context_index].root.workspace.clone();
                    let root_of = |p: &str| match workspace.find_package_for(p) {
                        Some(dartr_project::workspace::WorkspacePackage::Pub { root, .. }) => Some(root),
                        _ => None,
                    };
                    if std::env::var_os("DARTR_DEBUG_COMPLETION").is_some() {
                        for l in &libraries {
                            eprintln!("KNOWN {}", l.uri);
                        }
                    }
                    let mut included: Vec<String> = libraries
                        .iter()
                        .filter(|l| filter.should_include(l, &root_of))
                        .map(|l| l.uri.clone())
                        .collect();
                    // Dart `AnalysisDriver._discoverDartCore`: `dart:core` and
                    // the files it references are known before the analyzed
                    // files.
                    let sink = NoopSink;
                    let core_ctx = Ctx {
                        world: &world,
                        current: None,
                        local: None,
                        tp: &resolved.library.type_provider,
                        features: &resolved.library.features,
                        req: &sink,
                    };
                    let mut front = vec!["dart:core".to_string()];
                    if let Some(core) = core_ctx.library_by_uri("dart:core") {
                        let first = core_ctx.get(core).first_fragment();
                        let f = core_ctx.fragment(first);
                        let uris = f
                            .library_exports
                            .iter()
                            .map(|e| &e.directive.uri)
                            .chain(f.library_imports.iter().filter(|i| !i.is_synthetic).map(|i| &i.directive.uri));
                        for uri in uris {
                            if let dartr_element::DirectiveUri::Library { library, .. } = uri {
                                let u = c::elem::library_uri(&core_ctx, *library);
                                if !front.contains(&u) {
                                    front.push(u);
                                }
                            }
                        }
                    }
                    let mut ordered: Vec<String> = front.into_iter().filter(|u| included.contains(u)).collect();
                    included.retain(|u| !ordered.contains(u));
                    ordered.append(&mut included);
                    (world, ordered)
                }),
                None => None,
            }
        } else {
            None
        };

        let unit = resolved.unit();
        let sink = NoopSink;
        let world = known.as_ref().map(|k| &k.0).unwrap_or(&resolved.library.world);
        let ctx = Ctx {
            world,
            current: None,
            local: Some(&unit.local),
            tp: &resolved.library.type_provider,
            features: &resolved.library.features,
            req: &sink,
        };
        let content = unit.ast.tokens.source.clone();
        let package_root = package.as_ref().map(|(root, _)| root.clone());
        let in_test_directory = package_root
            .as_ref()
            .is_some_and(|root| path.starts_with(&format!("{root}/test/")));
        let Some(q) = c::build_request(RequestInputs {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
            root: unit.unit,
            content: &content,
            path: &path,
            offset,
            line_info: &line_info,
            style,
            package_root,
            in_test_directory,
            sdk_libraries,
            packages,
        }) else {
            return Ok(json!({"isIncomplete": false, "items": []}));
        };
        if let Some(t) = &trigger {
            if !trigger_character_valid(&q, offset, t) {
                return Ok(json!({"isIncomplete": false, "items": []}));
            }
        }
        let known_list = || -> Vec<KnownLibrary> {
            let Some((world, uris)) = &known else {
                return Vec::new();
            };
            uris.iter()
                .filter_map(|u| world.libraries.get(u.as_str()).copied())
                .map(|element| KnownLibrary { element })
                .collect()
        };
        let result = c::compute(
            &q,
            budget_ms,
            max_items,
            if use_not_imported {
                Some(&known_list as &dyn Fn() -> Vec<KnownLibrary>)
            } else {
                None
            },
        );

        // Dart `_getServerDartItems` after the candidates.
        let (r_offset, r_length) = q.replacement;
        let insert_length = offset.saturating_sub(r_offset).min(r_length);
        let complete_function_calls = !has_existing_arg_list(&q) && complete_function_calls_setting;
        let default_replacement = to_range(&line_info, r_offset, r_length);
        let default_insertion = to_range(&line_info, r_offset, insert_length);
        let mut defaults: Option<Value> = None;
        if caps.default_edit_range || caps.default_text_mode || caps.default_data {
            let mut d = Map::new();
            if caps.default_text_mode {
                d.insert("insertTextMode".into(), json!(1));
            }
            if caps.default_edit_range {
                let range = if !caps.insert_replace || default_insertion == default_replacement {
                    default_replacement.clone()
                } else {
                    json!({"insert": default_insertion, "replace": default_replacement})
                };
                d.insert("editRange".into(), range);
            }
            if caps.default_data {
                d.insert("data".into(), json!({"file": path}));
            }
            defaults = Some(Value::Object(d));
        }
        let has_default_edit_range = defaults.as_ref().is_some_and(|d| d.get("editRange").is_some());
        let has_default_text_mode = defaults.as_ref().is_some_and(|d| d.get("insertTextMode").is_some());
        let ic = ItemContext {
            caps: &caps,
            commit_characters_enabled: commit_characters,
            complete_function_calls,
            has_default_text_mode,
            file_path: &path,
        };
        let mut ranked: Vec<(Value, f64)> = Vec::new();
        for mut candidate in result.candidates {
            match &candidate.kind {
                Kind::Override { .. } => {
                    let data = c::overrides::override_data(&q, &candidate);
                    if let Kind::Override { data: d, .. } = &mut candidate.kind {
                        *d = data;
                    }
                }
                _ => {
                    if candidate.typed().is_some() {
                        let data = c::overrides::typed_data(&q, &candidate);
                        if let Some(t) = candidate.typed_mut() {
                            t.data = data;
                        }
                    }
                }
            }
            let mut item_length = r_length;
            let mut item_insert = insert_length;
            if let Kind::NamedArgument { replacement_length, .. } = &candidate.kind {
                if let Some(l) = replacement_length {
                    item_length = *l;
                }
                item_insert = offset.saturating_sub(r_offset).min(item_insert);
            }
            let replacement_range = to_range(&line_info, r_offset, item_length);
            let insertion_range = to_range(&line_info, r_offset, item_insert);
            let element = candidate.element().map(|e| member::base_element(&ctx, e));
            let location = element.and_then(|e| c::lsp::element_location(&ctx, e));
            let mut import_uris: Vec<String> = Vec::new();
            if candidate.is_importable() {
                if let Some(d) = &candidate.import_data {
                    if d.is_not_imported {
                        import_uris.push(d.library_uri.clone());
                    }
                }
            } else {
                let data = match &candidate.kind {
                    Kind::Override { data, .. } => data.as_ref(),
                    _ => candidate.typed().and_then(|t| t.data.as_ref()),
                };
                if let Some(d) = data {
                    import_uris.extend(d.imports.iter().cloned());
                }
            }
            let mut resolution: Option<Value> = None;
            if !import_uris.is_empty() {
                let mut d = Map::new();
                if !caps.default_data {
                    d.insert("file".into(), json!(path));
                }
                d.insert("importUris".into(), json!(import_uris));
                if let Some(l) = &location {
                    d.insert("ref".into(), json!(l));
                }
                resolution = Some(Value::Object(d));
            }
            let mut cleaned_doc = None;
            if resolution.is_none() {
                if let Some(e) = element {
                    if documentation_preference != "none" {
                        cleaned_doc = clean_documentation(&ctx, e, &templates, documentation_preference);
                    }
                    if cleaned_doc.as_ref().is_some_and(|d| d.encode_utf16().count() > MAX_DOC_SIZE_FOR_INLINING) {
                        if let Some(l) = &location {
                            let mut d = Map::new();
                            if !caps.default_data {
                                d.insert("file".into(), json!(path));
                            }
                            d.insert("ref".into(), json!(l));
                            resolution = Some(Value::Object(d));
                        }
                    }
                }
            }
            let has_default = has_default_edit_range
                && insertion_range == default_insertion
                && replacement_range == default_replacement;
            let doc = if resolution.is_none() { cleaned_doc } else { None };
            if let Some(item) = c::lsp::to_item(
                &ctx,
                &ic,
                &candidate,
                replacement_range,
                insertion_range,
                has_default,
                resolution,
                doc,
            ) {
                ranked.push((item, candidate.matcher_score));
            }
        }
        let prefix = c::target::target_prefix(q.ast, &q.target, offset);
        let mut unranked: Vec<Value> = Vec::new();
        if caps.snippets && enable_snippets {
            let default_range = defaults
                .as_ref()
                .and_then(|d| d.get("editRange"))
                .filter(|r| r.get("insert").is_none());
            unranked = c::snippets::snippet_items(
                &q,
                &prefix,
                &default_replacement,
                default_range,
                &caps.documentation_formats,
                caps.as_is_insert_mode,
            );
        }
        let mut is_incomplete = result.is_incomplete;
        let total = ranked.len();
        let max_ranked = (max_items.max(0) as usize).saturating_sub(unranked.len());
        let items: Vec<Value> = if total <= max_ranked {
            ranked.into_iter().map(|(i, _)| i).collect()
        } else {
            truncate_results(ranked, &prefix, max_ranked)
        };
        if items.len() != total {
            is_incomplete = true;
        }
        let mut items = items;
        items.extend(unranked);
        let mut out = Map::new();
        out.insert("isIncomplete".into(), json!(is_incomplete));
        out.insert("items".into(), Value::Array(items));
        if let Some(d) = defaults {
            out.insert("itemDefaults".into(), d);
        }
        Ok(Value::Object(out))
    }

    /// Dart `CompletionResolveHandler.handle`.
    pub(crate) fn completion_resolve(&mut self, params: &Value) -> ErrorOr<Value> {
        let Some(data) = params.get("data").filter(|d| d.is_object()) else {
            return Ok(params.clone());
        };
        let Some(file) = data.get("file").and_then(Value::as_str).map(str::to_string) else {
            return Ok(params.clone());
        };
        let import_uris: Vec<String> = data
            .get("importUris")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let reference = data.get("ref").and_then(Value::as_str).map(str::to_string);
        if import_uris.is_empty() && params.get("documentation").is_some() {
            return Ok(params.clone());
        }
        let resolved = self.require_resolved_unit(&file)?;
        let line_info = resolved.line_info().clone();
        let templates = self.dartdoc_templates(&file);
        let preference = self.client_configuration.global().preferred_documentation();
        let formats = self.client_formats("/textDocument/completion/completionItem/documentationFormat");
        let context_index = resolved.library.context;
        let mut world = None;
        if let Some(collection) = &self.collection {
            world = self.session.link_known_libraries(collection, context_index).map(|w| w.0);
        }
        let unit = resolved.unit();
        let sink = NoopSink;
        let ctx = Ctx {
            world: world.as_ref().unwrap_or(&resolved.library.world),
            current: None,
            local: Some(&unit.local),
            tp: &resolved.library.type_provider,
            features: &resolved.library.features,
            req: &sink,
        };
        let element = reference.as_deref().and_then(|r| locate_element(&ctx, r));
        let mut item = params.as_object().cloned().unwrap_or_default();
        let mut documentation = None;
        if let Some(e) = element {
            if preference != "none" {
                let doc = crate::hover::documentation(&ctx, e, &templates);
                let doc = if preference == "summary" {
                    doc.map(|d| summary_of(&d))
                } else {
                    doc
                };
                if let Some(d) = doc.filter(|d| !d.is_empty()) {
                    documentation = Some(Self::markup_content_or_string(&formats, d));
                }
            }
        }
        let mut edits = Vec::new();
        if !import_uris.is_empty() {
            edits = import_edits(&unit.ast, unit.unit, &line_info, &import_uris);
        }
        let mut detail = item.get("detail").and_then(Value::as_str).map(str::to_string);
        if !edits.is_empty() && !import_uris.is_empty() {
            let rest = detail.clone().unwrap_or_default();
            detail = Some(if import_uris.len() == 1 {
                let display = if import_uris[0].starts_with("file://") {
                    import_uris[0].clone()
                } else {
                    import_uris[0].clone()
                };
                format!("Auto import from '{display}'\n\n{rest}").trim().to_string()
            } else {
                format!("Auto import required URIs\n\n{rest}").trim().to_string()
            });
        }
        match detail {
            Some(d) => {
                item.insert("detail".into(), json!(d));
            }
            None => {
                item.remove("detail");
            }
        }
        match documentation {
            Some(d) => {
                item.insert("documentation".into(), d);
            }
            None => {
                item.remove("documentation");
            }
        }
        item.insert("additionalTextEdits".into(), Value::Array(edits));
        Ok(Value::Object(item))
    }
}

/// Dart `getCleanElementDocumentation`.
fn clean_documentation(
    ctx: &Ctx<'_>,
    element: ElementId,
    templates: &std::collections::HashMap<String, String>,
    preference: &str,
) -> Option<String> {
    let full = crate::hover::documentation(ctx, element, templates)?;
    let doc = c::lsp::remove_dart_doc_delimiters(&full);
    let raw = if preference == "summary" { summary_of(&doc) } else { doc };
    Some(crate::hover::clean_dartdoc(&raw))
}

/// Dart `getDartDocSummary`: the first paragraph.
fn summary_of(doc: &str) -> String {
    let mut lines = Vec::new();
    for line in doc.split('\n') {
        if line.trim().is_empty() {
            break;
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Dart `ElementLocation.decode(ref).locateIn(session)`.
fn locate_element(ctx: &Ctx<'_>, reference: &str) -> Option<ElementId> {
    let parts: Vec<&str> = reference.split(';').collect();
    if parts.len() < 2 {
        return None;
    }
    let library = ctx.library_by_uri(parts[0])?;
    let children = library_children(ctx, library);
    let lookup = |e: ElementId| member::lookup_name(ctx, ElemRef::Base(e));
    let top = children.into_iter().find(|c| lookup(*c).as_deref() == Some(parts[1]))?;
    if parts.len() == 2 {
        return Some(top);
    }
    member_children(ctx, top)
        .into_iter()
        .find(|c| lookup(*c).as_deref() == Some(parts[2]))
}

/// Dart `LibraryElement.children`.
fn library_children(ctx: &Ctx<'_>, library: dartr_element::EId<dartr_element::LibraryElement>) -> Vec<ElementId> {
    let l = ctx.get(library);
    l.classes
        .iter()
        .map(|e| e.raw())
        .chain(l.enums.iter().map(|e| e.raw()))
        .chain(l.extensions.iter().map(|e| e.raw()))
        .chain(l.extension_types.iter().map(|e| e.raw()))
        .chain(l.getters.iter().map(|e| e.raw()))
        .chain(l.mixins.iter().map(|e| e.raw()))
        .chain(l.setters.iter().map(|e| e.raw()))
        .chain(l.top_level_functions.iter().map(|e| e.raw()))
        .chain(l.top_level_variables.iter().map(|e| e.raw()))
        .chain(l.type_aliases.iter().map(|e| e.raw()))
        .collect()
}

/// The members of an instance element (Dart `children`).
fn member_children(ctx: &Ctx<'_>, element: ElementId) -> Vec<ElementId> {
    let Some(instance) = element.cast::<dartr_element::InstanceElement>() else {
        return Vec::new();
    };
    let data = ctx.instance(instance);
    let mut out: Vec<ElementId> = Vec::new();
    if let Some(i) = element.cast::<dartr_element::InterfaceElement>() {
        out.extend(ctx.interface(i).constructors.iter().map(|c| c.raw()));
    }
    out.extend(data.fields.iter().map(|e| e.raw()));
    out.extend(data.getters.iter().map(|e| e.raw()));
    out.extend(data.methods.iter().map(|e| e.raw()));
    out.extend(data.setters.iter().map(|e| e.raw()));
    out.extend(data.type_params.iter().map(|e| e.raw()));
    out
}

/// The edits that import [uris] (a simplified Dart
/// `DartFileEditBuilder.importLibraryElement`): a new import directive in
/// the sorted position among the existing imports.
fn import_edits(ast: &Ast, unit: Id<CompilationUnit>, line_info: &dartr_syntax::LineInfo, uris: &[String]) -> Vec<Value> {
    let mut edits = Vec::new();
    let imports: Vec<Id<ImportDirective>> = ast
        .list_raw(ast[unit].directives)
        .iter()
        .filter_map(|d| ast.cast::<ImportDirective>(*d))
        .collect();
    let group = |uri: &str| -> u8 {
        if uri.starts_with("dart:") {
            0
        } else if uri.starts_with("package:") {
            1
        } else {
            2
        }
    };
    for uri in uris {
        let text = format!("import '{uri}';");
        let existing_uri = |i: Id<ImportDirective>| -> String {
            let u = ast[i].uri.raw();
            ast.cast::<SimpleStringLiteral>(u)
                .map(|s| ast[s].value.to_string())
                .unwrap_or_default()
        };
        if imports.is_empty() {
            let directives = ast.list_raw(ast[unit].directives);
            if let Some(&last) = directives.last() {
                let end = ast.end(last);
                edits.push(json!({"range": to_range(line_info, end, 0), "newText": format!("\n\n{text}")}));
            } else {
                edits.push(json!({"range": to_range(line_info, 0, 0), "newText": format!("{text}\n\n")}));
            }
            continue;
        }
        let key = (group(uri), uri.clone());
        let before = imports.iter().copied().find(|i| {
            let u = existing_uri(*i);
            (group(&u), u) > key
        });
        match before {
            Some(i) => {
                let offset = ast.offset(i);
                let u = existing_uri(i);
                let sep = if group(&u) != group(uri) { "\n\n" } else { "\n" };
                edits.push(json!({"range": to_range(line_info, offset, 0), "newText": format!("{text}{sep}")}));
            }
            None => {
                let last = *imports.last().unwrap();
                let u = existing_uri(last);
                let sep = if group(&u) != group(uri) { "\n\n" } else { "\n" };
                edits.push(json!({"range": to_range(line_info, ast.end(last), 0), "newText": format!("{sep}{text}")}));
            }
        }
    }
    edits
}

/// Dart `_truncateResults`.
fn truncate_results(items: Vec<(Value, f64)>, prefix: &str, max: usize) -> Vec<Value> {
    let prefix_lower = prefix.to_lowercase();
    let items = c::dart_sort_vec(items, |a, b| {
        if a.1 != b.1 {
            return if b.1 > a.1 { 1 } else { -1 };
        }
        let a_text = a.0.get("sortText").or(a.0.get("label")).and_then(Value::as_str).unwrap_or("");
        let b_text = b.0.get("sortText").or(b.0.get("label")).and_then(Value::as_str).unwrap_or("");
        if a_text == b_text {
            let a_label = a.0.get("label").and_then(Value::as_str).unwrap_or("").encode_utf16().count() as i64;
            let b_label = b.0.get("label").and_then(Value::as_str).unwrap_or("").encode_utf16().count() as i64;
            return a_label - b_label;
        }
        match a_text.cmp(b_text) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    });
    items
        .into_iter()
        .enumerate()
        .filter(|(i, (item, _))| {
            if prefix_lower.is_empty() {
                return *i < max;
            }
            let text = item
                .get("filterText")
                .or(item.get("label"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            *i < max || text == prefix_lower
        })
        .map(|(_, (item, _))| item)
        .collect()
}

/// Dart `_hasExistingArgList`.
fn has_existing_arg_list(q: &c::Request<'_, '_>) -> bool {
    let ast = q.ast;
    let Some(mut node) = q.target.entity_node() else {
        return false;
    };
    if let Some(s) = ast.cast::<ExpressionStatement>(node) {
        node = ast[s].expression.raw();
    }
    if ast.is::<SimpleIdentifier>(node) {
        match ast.parent(node) {
            Some(p) => node = p,
            None => return false,
        }
    }
    if ast.is::<ConstructorName>(node) {
        match ast.parent(node) {
            Some(p) => node = p,
            None => return false,
        }
    }
    let args = if let Some(m) = ast.cast::<MethodInvocation>(node) {
        Some(ast[m].argument_list)
    } else if let Some(f) = ast.cast::<FunctionExpressionInvocation>(node) {
        Some(ast[f].argument_list)
    } else if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        Some(ast[i].argument_list)
    } else {
        None
    };
    if let Some(a) = args {
        return !ast.t_synthetic(ast.begin(a.raw()));
    }
    if let Some(p) = ast.cast::<PropertyAccess>(node) {
        let name = ast[p].property_name;
        return ast.t_lexeme(ast[name].token).starts_with('(');
    }
    false
}

/// Dart `_triggerCharacterValid`.
fn trigger_character_valid(q: &c::Request<'_, '_>, offset: u32, trigger: &str) -> bool {
    let ast = q.ast;
    let node = q.target.containing_node;
    let in_directive = |n: NodeId| ast.parent(n).is_some_and(|p| ast.is::<Directive>(p));
    match trigger {
        "\"" | "'" => ast.cast::<SimpleStringLiteral>(node).is_some_and(|s| {
            in_directive(node) && offset == c::target::string_contents_range(ast, s).0
        }),
        "{" => ast
            .cast::<InterpolationExpression>(node)
            .is_some_and(|i| ast.offset(ast[i].expression) == offset),
        "/" => ast.cast::<SimpleStringLiteral>(node).is_some_and(|s| {
            let (start, end) = c::target::string_contents_range(ast, s);
            in_directive(node) && offset >= start && offset <= end
        }),
        ":" => !ast.is::<SwitchStatement>(node),
        _ => true,
    }
}

#[allow(dead_code)]
fn is_tag(e: ElementId, tag: Tag) -> bool {
    e.tag() == tag
}
