// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_completion.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_completion_resolve.dart

//! `textDocument/completion` and `completionItem/resolve`.

use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, NoopSink, Tag};
#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
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
    pub(super) fn bool_cap(&self, pointer: &str) -> bool {
        self.client
            .raw
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
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
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
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
                .unwrap_or_else(|| {
                    vec![
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                    ]
                }),
            documentation_formats: self.client_formats(&format!("{item}/documentationFormat")),
            default_edit_range: defaults.iter().any(|d| d == "editRange"),
            default_text_mode: defaults.iter().any(|d| d == "insertTextMode"),
            default_data: self.bool_cap("/textDocument/completion/completionList/applyKindSupport"),
        }
    }

    /// The quote of the `prefer_single_quotes` or `prefer_double_quotes`
    /// lint of [path] (Dart `CodeStyleOptions._lintQuote`).
    fn lint_quote(&self, path: &str) -> Option<char> {
        let collection = self.collection.as_ref()?;
        let context = collection.context_for(path)?;
        let options = collection.options_for(context, path);
        let has = |name: &str| options.lint_rules.iter().any(|r| r == name);
        if has("prefer_single_quotes") {
            Some('\'')
        } else if has("prefer_double_quotes") {
            Some('"')
        } else {
            None
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
        let lint_quote = if has("prefer_single_quotes") {
            Some('\'')
        } else if has("prefer_double_quotes") {
            Some('"')
        } else {
            None
        };
        let quote = lint_quote.unwrap_or('\'');
        CodeStyle {
            lint_quote,
            specify_types: has("always_specify_types"),
            make_locals_final: has("prefer_final_locals"),
            quote,
        }
    }

    fn completion_known_for(
        &mut self,
        context_index: usize,
        package: Option<(String, Option<String>)>,
        path: &str,
    ) -> Option<(
        dartr_element::WorldSnapshot,
        std::sync::Arc<Vec<KnownLibrary>>,
    )> {
        let collection = self.collection.as_ref()?;
        let excluded = &self.excluded;
        let is_excluded_fn = |f: &str| {
            excluded
                .iter()
                .any(|e| f == e || f.starts_with(&format!("{e}/")))
        };
        let pkg_root = package.as_ref().map(|(r, _)| r.clone());
        let pkg_name = package.as_ref().and_then(|(_, n)| n.clone());
        let cached_added = self.context_added_files.get(context_index).cloned();
        let pkg_lib_files = &mut self.pkg_lib_files;
        let (world, ordered_libraries) = self.session.link_known_libraries_ordered(
            collection,
            context_index,
            pkg_root.clone(),
            pkg_name.clone(),
            |coll, ctx_idx| {
                let context = &coll.contexts[ctx_idx];
                let added_files: Vec<String> = cached_added.unwrap_or_else(|| {
                    context
                        .root
                        .analyzed_files()
                        .into_iter()
                        .filter(|f| f.ends_with(".dart") && !is_excluded_fn(f))
                        .collect()
                });
                let mut available_files: Vec<String> = Vec::new();
                if let Some(sdk) = context.sdk.as_ref().or(coll.sdk.as_ref()) {
                    available_files.extend(
                        sdk.libraries()
                            .iter()
                            .filter_map(|l| sdk.map_dart_uri(&l.short_name)),
                    );
                }
                fn recurse_pkg(folder: &str, skip_src: bool, out: &mut Vec<String>) {
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
                            dartr_project::fs::ResourceKind::Folder => {
                                if skip_src
                                    && (child.path.ends_with("/src")
                                        || child.path.ends_with("\\src"))
                                {
                                    continue;
                                }
                                recurse_pkg(&child.path, false, out);
                            }
                        }
                    }
                }
                let friend_of_analyzer =
                    matches!(pkg_name.as_deref(), Some("analysis_server" | "linter"));
                let allowed_pkgs: Option<std::collections::HashSet<String>> =
                    if let Some((root, name)) = &package {
                        dartr_project::pubspec::Pubspec::read(&format!("{root}/pubspec.yaml")).map(
                            |pubspec| {
                                let mut set: std::collections::HashSet<String> = pubspec
                                    .dependencies
                                    .into_iter()
                                    .chain(pubspec.dev_dependencies)
                                    .collect();
                                if let Some(n) = name {
                                    set.insert(n.clone());
                                }
                                if friend_of_analyzer {
                                    set.insert("analyzer".to_string());
                                }
                                set
                            },
                        )
                    } else {
                        None
                    };
                for pkg in context.packages.packages() {
                    if let Some(allowed) = &allowed_pkgs
                        && !allowed.contains(&pkg.name)
                    {
                        continue;
                    }
                    let skip_src = package.as_ref().is_some_and(|(root, _)| &pkg.root != root)
                        && !(friend_of_analyzer && pkg.name == "analyzer");
                    let files = pkg_lib_files
                        .entry((pkg.lib.clone(), skip_src))
                        .or_insert_with(|| {
                            let mut v = Vec::new();
                            recurse_pkg(&pkg.lib, skip_src, &mut v);
                            std::sync::Arc::new(v)
                        });
                    available_files.extend(files.iter().cloned());
                }
                (
                    added_files,
                    available_files,
                    allowed_pkgs,
                    friend_of_analyzer,
                )
            },
        )?;
        let in_lib = package.as_ref().is_some_and(|(root, _)| {
            ["lib", "bin", "web"]
                .iter()
                .any(|f| path.starts_with(&format!("{root}/{f}/")))
        });
        let cache_key = (context_index, pkg_root, in_lib);
        let ordered_ptr = std::sync::Arc::as_ptr(&ordered_libraries) as usize;
        let stores_len = world.stores.len();
        if std::env::var_os("DARTR_DEBUG_COMPLETION").is_none()
            && let Some((cached_ptr, cached_stores_len, cached_list, cached_indices)) =
                self.completion_known_cache.get_mut(&cache_key)
            && *cached_ptr == ordered_ptr
        {
            if *cached_stores_len == stores_len {
                return Some((world, cached_list.clone()));
            }
            let filtered: Vec<KnownLibrary> = cached_indices
                .iter()
                .filter_map(|&i| {
                    let l = &ordered_libraries[i];
                    world
                        .libraries
                        .get(l.uri.as_str())
                        .copied()
                        .map(|element| KnownLibrary { element })
                })
                .collect();
            let filtered = std::sync::Arc::new(filtered);
            *cached_stores_len = stores_len;
            *cached_list = filtered.clone();
            return Some((world, filtered));
        }
        let filter = c::FileFilter::new(package, path);
        let workspace = &collection.contexts[context_index].root.workspace;
        let dir_cache: std::cell::RefCell<std::collections::HashMap<String, Option<String>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
        let root_of = |p: &str| {
            let dir = dartr_project::paths::dirname(p);
            if let Some(cached) = dir_cache.borrow().get(dir) {
                return cached.clone();
            }
            let found = workspace.find_pub_package_root_for(p);
            dir_cache
                .borrow_mut()
                .insert(dir.to_string(), found.clone());
            found
        };
        if std::env::var_os("DARTR_DEBUG_COMPLETION").is_some() {
            for l in ordered_libraries.iter() {
                eprintln!("KNOWN {}", l.uri);
            }
        }
        let mut indices = Vec::new();
        let mut filtered = Vec::new();
        for (i, l) in ordered_libraries.iter().enumerate() {
            if !filter.should_include(l, &root_of) {
                continue;
            }
            indices.push(i);
            if std::env::var_os("DARTR_DEBUG_COMPLETION").is_some() {
                eprintln!("ORDER {}", l.uri);
            }
            if let Some(&element) = world.libraries.get(l.uri.as_str()) {
                filtered.push(KnownLibrary { element });
            }
        }
        let filtered = std::sync::Arc::new(filtered);
        self.completion_known_cache.insert(
            cache_key,
            (ordered_ptr, stores_len, filtered.clone(), indices),
        );
        Some((world, filtered))
    }

    /// Pre-warms `completion_known_for` for a newly opened priority `.dart` file
    /// when `suggestFromUnimportedLibraries` is active.
    pub(super) fn warm_completion_for_open_file(&mut self, path: &str) {
        let apply_edit = self.bool_cap("/workspace/applyEdit");
        let use_not_imported = self.init.suggest_from_unimported_libraries && apply_edit;
        if !use_not_imported {
            return;
        }
        let Some((context_index, package)) = self.collection.as_ref().and_then(|collection| {
            let context = collection
                .context_for(path)
                .or_else(|| collection.contexts.first())?;
            let context_index = collection
                .contexts
                .iter()
                .position(|x| std::ptr::eq(x, context))?;
            let package = match context.root.workspace.find_package_for(path) {
                Some(dartr_project::workspace::WorkspacePackage::Pub { root, name, .. }) => {
                    Some((root, name))
                }
                _ => None,
            };
            Some((context_index, package))
        }) else {
            return;
        };
        let _ = self.completion_known_for(context_index, package, path);
        let _ = self.dartdoc_templates(path);
    }

    /// Dart `CompletionHandler.handle`.
    pub(crate) fn completion(&mut self, params: &Value) -> ErrorOr<Value> {
        let path = self.path_of_doc(params)?;
        if !path.ends_with(".dart") {
            let Some(content) = self.content(&path) else {
                return Err(crate::mapping::ResponseError::with_data(
                    crate::mapping::codes::FILE_NOT_ANALYZED,
                    "File is not being analyzed",
                    &path,
                ));
            };
            let line_info = dartr_syntax::LineInfo::from_content(&content);
            let offset = self.position_offset(&line_info, params)?;
            if path.ends_with(".yaml")
                && let Some(kind) = c::yaml::YamlFileKind::from_path(&path)
            {
                let caps = self.completion_capabilities();
                let preferred_documentation =
                    self.client_configuration.global().preferred_documentation();
                let items = c::yaml::yaml_completion_items(
                    kind,
                    &path,
                    &content,
                    &line_info,
                    offset as usize,
                    &caps,
                    preferred_documentation,
                );
                return Ok(json!({"isIncomplete": false, "items": items}));
            }
            return Ok(json!({"isIncomplete": false, "items": []}));
        }
        let resolved = match self.require_resolved_unit(&path) {
            Ok(r) => r,
            Err(e) if e.code == crate::mapping::codes::FILE_NOT_ANALYZED => {
                // Dart takes the line info from the file when the file is
                // not analyzed, and then returns no items.
                let Some(content) = self.content(&path) else {
                    return Err(e);
                };
                let line_info = dartr_syntax::LineInfo::from_content(&content);
                self.position_offset(&line_info, params)?;
                return Ok(json!({"isIncomplete": false, "items": []}));
            }
            Err(e) => return Err(e),
        };
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
        let style = self.code_style(&path);

        // The package of the file and the SDK and packages of its context.
        let context_index = resolved.library.context;
        let (package, sdk_libraries, packages) = match &self.collection {
            Some(collection) => {
                let context = &collection.contexts[context_index];
                let package = match context.root.workspace.find_package_for(&path) {
                    Some(dartr_project::workspace::WorkspacePackage::Pub {
                        root, name, ..
                    }) => Some((root, name)),
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

        let unit = resolved.unit();
        let sink = NoopSink;
        let ctx = Ctx {
            world: &resolved.library.world,
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
        let Some(mut q) = c::build_request(RequestInputs {
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
        let in_scope = c::compute_in_scope(&q, budget_ms, max_items);

        // The libraries of the not-imported pass (Dart `knownFiles`), ordered
        // in `dart_known_order` and filtered by `FileFilter`, cached across
        // requests when the linked world and file set have not changed.
        // Only link and discover unimported libraries when the in-scope pass
        // actually produced not-imported operations!
        let known = if use_not_imported && in_scope.needs_not_imported() {
            self.completion_known_for(context_index, package, &path)
        } else {
            None
        };
        let ctx_with_known;
        if let Some((world, _)) = &known {
            ctx_with_known = Ctx { world, ..ctx };
            q.ctx = &ctx_with_known;
        }
        let ctx = *q.ctx;
        let known_list = || -> Vec<KnownLibrary> {
            let Some((_, list)) = &known else {
                return Vec::new();
            };
            (**list).clone()
        };
        let result = c::finalize(
            &q,
            in_scope,
            max_items,
            if known.is_some() {
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
        let has_default_edit_range = defaults
            .as_ref()
            .is_some_and(|d| d.get("editRange").is_some());
        let has_default_text_mode = defaults
            .as_ref()
            .is_some_and(|d| d.get("insertTextMode").is_some());
        let ic = ItemContext {
            caps: &caps,
            commit_characters_enabled: commit_characters,
            complete_function_calls,
            has_default_text_mode,
            file_path: &path,
        };
        // Dart `getColorHexString` of the variables of the candidates (when
        // the client supports the kind `Color`).
        let mut color_hex: std::collections::HashMap<dartr_element::ElementId, String> =
            std::collections::HashMap::new();
        if caps.item_kinds.contains(&16) {
            let mut elements: Vec<dartr_element::ElementId> = Vec::new();
            for candidate in &result.candidates {
                let Some(e) = candidate.element() else {
                    continue;
                };
                let b = member::base_element(&ctx, e);
                if !matches!(b.tag(), Tag::Field | Tag::TopLevelVariable)
                    || color_hex.contains_key(&b)
                    || elements.contains(&b)
                {
                    continue;
                }
                if !c::lsp::is_color(&ctx, member::type_(&ctx, ElemRef::Base(b))) {
                    continue;
                }
                if let Some(cached) = self.color_hex_cache.get(&(context_index, b)) {
                    if let Some(hex) = cached {
                        color_hex.insert(b, hex.clone());
                    }
                } else {
                    elements.push(b);
                }
            }
            if !elements.is_empty() {
                let values =
                    self.session
                        .constant_values_of(context_index, q.library, &elements, |v| {
                            c::lsp::color_hex_string(&ctx, v)
                        });
                for (e, v) in elements.into_iter().zip(values) {
                    self.color_hex_cache.insert((context_index, e), v.clone());
                    if let Some(v) = v {
                        color_hex.insert(e, v);
                    }
                }
            }
        }
        let mut lazy_templates: Option<std::sync::Arc<std::collections::HashMap<String, String>>> =
            None;
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
            if let Kind::NamedArgument {
                replacement_length, ..
            } = &candidate.kind
            {
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
                        cleaned_doc = clean_documentation_with(
                            &ctx,
                            e,
                            || {
                                if lazy_templates.is_none() {
                                    lazy_templates = Some(self.dartdoc_templates(&path));
                                }
                                lazy_templates.as_ref().unwrap()
                            },
                            documentation_preference,
                        );
                    }
                    if cleaned_doc
                        .as_ref()
                        .is_some_and(|d| d.encode_utf16().count() > MAX_DOC_SIZE_FOR_INLINING)
                    {
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
            let doc = if resolution.is_none() {
                cleaned_doc
            } else {
                None
            };
            if let Some(item) = c::lsp::to_item(
                &ctx,
                &ic,
                &candidate,
                replacement_range,
                insertion_range,
                has_default,
                resolution,
                doc,
                element.and_then(|b| color_hex.get(&b).cloned()),
            ) {
                ranked.push((item, candidate.matcher_score));
            }
        }
        let prefix = c::target::target_prefix(q.ast, &q.target, offset);
        let mut unranked: Vec<Value> = Vec::new();
        // Dart `SnippetProducer.isValid`: the file must be analyzed in the
        // root of its context (no snippets for a file outside the roots).
        let in_context_root = self
            .collection
            .as_ref()
            .is_some_and(|collection| collection.contexts[context_index].root.is_analyzed(&path));
        if caps.snippets && enable_snippets && in_context_root {
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
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let reference = data.get("ref").and_then(Value::as_str).map(str::to_string);
        if import_uris.is_empty() && params.get("documentation").is_some() {
            return Ok(params.clone());
        }
        let resolved = self.require_resolved_unit(&file)?;
        let line_info = resolved.line_info().clone();
        let preference = self.client_configuration.global().preferred_documentation();
        let formats =
            self.client_formats("/textDocument/completion/completionItem/documentationFormat");
        let context_index = resolved.library.context;
        let mut world = None;
        if let Some(collection) = &self.collection {
            world = self
                .session
                .link_known_libraries(collection, context_index)
                .map(|w| w.0);
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
                let mut lazy_templates = None;
                let doc = crate::hover::documentation_with(&ctx, e, || {
                    lazy_templates.insert(self.dartdoc_templates(&file))
                });
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
            let content = self.content(&file).unwrap_or_default();
            let lint_quote = self.lint_quote(&file);
            edits = c::imports::import_edits(
                &unit.ast,
                unit.unit,
                &line_info,
                &content,
                lint_quote,
                &import_uris,
            );
        }
        let mut detail = item
            .get("detail")
            .and_then(Value::as_str)
            .map(str::to_string);
        if !edits.is_empty() && !import_uris.is_empty() {
            let rest = detail.clone().unwrap_or_default();
            detail = Some(if import_uris.len() == 1 {
                let display = if import_uris[0].starts_with("file://") {
                    import_uris[0].clone()
                } else {
                    import_uris[0].clone()
                };
                format!("Auto import from '{display}'\n\n{rest}")
                    .trim()
                    .to_string()
            } else {
                format!("Auto import required URIs\n\n{rest}")
                    .trim()
                    .to_string()
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
fn clean_documentation_with<'a>(
    ctx: &Ctx<'_>,
    element: ElementId,
    get_templates: impl FnOnce() -> &'a std::collections::HashMap<String, String>,
    preference: &str,
) -> Option<String> {
    let full = crate::hover::documentation_with(ctx, element, get_templates)?;
    let doc = c::lsp::remove_dart_doc_delimiters(&full);
    let raw = if preference == "summary" {
        summary_of(&doc)
    } else {
        doc
    };
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
    let top = children
        .into_iter()
        .find(|c| lookup(*c).as_deref() == Some(parts[1]))?;
    if parts.len() == 2 {
        return Some(top);
    }
    member_children(ctx, top)
        .into_iter()
        .find(|c| lookup(*c).as_deref() == Some(parts[2]))
}

/// Dart `LibraryElement.children`.
fn library_children(
    ctx: &Ctx<'_>,
    library: dartr_element::EId<dartr_element::LibraryElement>,
) -> Vec<ElementId> {
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

/// Dart `_truncateResults`.
fn truncate_results(items: Vec<(Value, f64)>, prefix: &str, max: usize) -> Vec<Value> {
    let prefix_lower = prefix.to_lowercase();
    let items = c::dart_sort_vec(items, |a, b| {
        if a.1 != b.1 {
            return if b.1 > a.1 { 1 } else { -1 };
        }
        let a_text =
            a.0.get("sortText")
                .or(a.0.get("label"))
                .and_then(Value::as_str)
                .unwrap_or("");
        let b_text =
            b.0.get("sortText")
                .or(b.0.get("label"))
                .and_then(Value::as_str)
                .unwrap_or("");
        if a_text == b_text {
            let a_label =
                a.0.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .encode_utf16()
                    .count() as i64;
            let b_label =
                b.0.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .encode_utf16()
                    .count() as i64;
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
