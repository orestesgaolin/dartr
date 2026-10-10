// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (DartFileEditBuilderImpl.importLibrary, importLibraryElement, importsLibrary, _importLibrary, _getLibraryUriText, _handleCombinators, _addLibraryImports, _LibraryImport)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/directive_sort.dart (compareDirectiveUri, DirectiveSortPriority)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/library.dart (canBeRelativeImport)

//! The imports that a Dart file builder adds: the same insertion logic as
//! the Dart change builder (between the existing imports by kind and URI,
//! after the library directive, before exports and parts, or before the
//! first declaration).

use std::cmp::Ordering;

use dartr_ast::*;
use dartr_element::{Ctx, EId, LibraryElement, NamespaceCombinator, NoopSink};
use indexmap::IndexSet;

use super::change_builder::{EditBuilder, FileEditBuilder};
use super::code_style::CodeStyleOptions;
use super::utils::first_token_after_comment_and_metadata;

/// Dart `compareDirectiveUri`.
pub fn compare_directive_uri(a: &str, b: &str) -> Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:"))
        && !a.starts_with('/')
        && !b.starts_with('/')
    {
        return dart_compare(a, b);
    }
    let (Some(index_a), Some(index_b)) = (a.find('/'), b.find('/')) else {
        return dart_compare(a, b);
    };
    let result = dart_compare(&a[..index_a], &b[..index_b]);
    if result != Ordering::Equal {
        return result;
    }
    dart_compare(&a[index_a + 1..], &b[index_b + 1..])
}

/// Dart `String.compareTo` (UTF-16 code units).
pub fn dart_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Dart `DirectiveSortKind`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DirectiveSortKind {
    Import,
    Export,
    Part,
}

/// Dart `DirectiveSortPriority`: the ordinal.
pub fn directive_sort_priority(uri: &str, kind: DirectiveSortKind) -> u32 {
    let base = match kind {
        DirectiveSortKind::Import => 0,
        DirectiveSortKind::Export => 4,
        DirectiveSortKind::Part => return 8,
    };
    base + if uri.starts_with("dart:") {
        0
    } else if uri.starts_with("package:") {
        1
    } else if uri.contains("://") {
        2
    } else {
        3
    }
}

/// Dart `canBeRelativeImport`.
pub fn can_be_relative_import(a: &str, b: &str) -> bool {
    let package = |u: &str| -> Option<String> {
        let rest = u.strip_prefix("package:")?;
        let first = rest.split('/').next()?;
        Some(first.to_string())
    };
    match (package(a), package(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// The relative path from the folder [from_dir] to [to] (Dart
/// `pathContext.relative`), with `/` separators.
pub fn relative_path(to: &str, from_dir: &str) -> String {
    let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let from_parts: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let mut common = 0;
    while common < to_parts.len()
        && common < from_parts.len()
        && to_parts[common] == from_parts[common]
    {
        common += 1;
    }
    let mut out: Vec<&str> = Vec::new();
    for _ in common..from_parts.len() {
        out.push("..");
    }
    out.extend(&to_parts[common..]);
    if out.is_empty() {
        ".".to_string()
    } else {
        out.join("/")
    }
}

fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => ".",
    }
}

/// Dart `_LibraryImport`.
#[derive(Clone, Debug)]
pub struct LibraryImport {
    pub uri_text: String,
    sort_priority: u32,
    /// Dart `prefixes` (a set in insertion order).
    pub prefixes: IndexSet<String>,
    pub shown_names: Vec<Vec<String>>,
    pub hidden_names: Vec<Vec<String>>,
    pub is_explicitly_imported: bool,
}

impl LibraryImport {
    pub fn new(
        uri_text: String,
        prefix: String,
        is_explicitly_imported: bool,
        shown_names: Vec<Vec<String>>,
        hidden_names: Vec<Vec<String>>,
    ) -> Self {
        let sort_priority = directive_sort_priority(&uri_text, DirectiveSortKind::Import);
        let mut prefixes = IndexSet::new();
        prefixes.insert(prefix);
        LibraryImport {
            uri_text,
            sort_priority,
            prefixes,
            shown_names,
            hidden_names,
            is_explicitly_imported,
        }
    }

    /// Dart `prefix`: the first prefix.
    pub fn prefix(&self) -> &str {
        self.prefixes.first().map(String::as_str).unwrap_or("")
    }

    /// Dart `allShownNames`.
    pub fn all_shown_names(&self) -> IndexSet<String> {
        self.shown_names.iter().flatten().cloned().collect()
    }

    /// Dart `allHiddenNames`.
    pub fn all_hidden_names(&self) -> IndexSet<String> {
        self.hidden_names.iter().flatten().cloned().collect()
    }

    /// Dart `compareTo`.
    fn compare(&self, other: &LibraryImport) -> Ordering {
        if self.sort_priority == other.sort_priority {
            return compare_directive_uri(&self.uri_text, &other.uri_text);
        }
        self.sort_priority.cmp(&other.sort_priority)
    }

    /// Dart `_ensureShown`.
    pub fn ensure_shown(&mut self, name: &str, use_show: bool) {
        if self.shown_names.is_empty() && use_show {
            self.shown_names.push(vec![name.to_string()]);
        } else if let Some(last) = self.shown_names.last_mut() {
            last.push(name.to_string());
        }
        for list in &mut self.hidden_names {
            list.retain(|n| n != name);
        }
        self.hidden_names.retain(|l| !l.is_empty());
    }
}

/// An import of the library element of a unit (Dart `LibraryImport` of
/// `libraryElement.firstFragment.libraryImports`).
#[derive(Clone, Debug)]
pub struct ExistingImport {
    pub library: Option<EId<LibraryElement>>,
    pub library_uri: Option<String>,
    pub prefix: Option<String>,
    /// The combinators: `true` for `show`, and the names.
    pub combinators: Vec<(bool, Vec<String>)>,
    /// The offsets of the combinators (`offset`, `end`).
    pub combinator_ranges: Vec<(u32, u32)>,
}

/// The URI of [library] (Dart `library.uri`).
pub fn library_uri(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> String {
    let first = ctx.get(library).first_fragment();
    ctx.fragment(first).source.uri.to_string()
}

/// The path of [library] (Dart `library.firstFragment.source.fullName`).
pub fn library_path(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> String {
    let first = ctx.get(library).first_fragment();
    ctx.fragment(first).source.path.to_string()
}

/// The imports of the defining unit of the library of [ctx]'s unit.
pub fn existing_imports(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> Vec<ExistingImport> {
    let first = ctx.get(library).first_fragment();
    let data = ctx.fragment(first);
    data.library_imports
        .iter()
        .map(|i| {
            let library = dartr_resolver::error::support::uri_library(&i.directive.uri);
            let mut combinators = Vec::new();
            let mut ranges = Vec::new();
            for c in &i.combinators {
                match c {
                    NamespaceCombinator::Show {
                        shown_names,
                        offset,
                        end,
                    } => {
                        combinators.push((
                            true,
                            shown_names
                                .iter()
                                .map(|n| ctx.name_str(*n).to_string())
                                .collect(),
                        ));
                        ranges.push((*offset, *end as u32));
                    }
                    NamespaceCombinator::Hide {
                        hidden_names,
                        offset,
                        end,
                    } => {
                        combinators.push((
                            false,
                            hidden_names
                                .iter()
                                .map(|n| ctx.name_str(*n).to_string())
                                .collect(),
                        ));
                        ranges.push((*offset, *end as u32));
                    }
                }
            }
            ExistingImport {
                library,
                library_uri: library.map(|l| library_uri(ctx, l)),
                prefix: i
                    .prefix
                    .and_then(|p| ctx.fragment(p).name)
                    .map(|n| ctx.name_str(n).to_string()),
                combinators,
                combinator_ranges: ranges,
            }
        })
        .collect()
}

impl<'c, 'w> FileEditBuilder<'c, 'w> {
    /// The path of the builder that keeps the imports (Dart
    /// `libraryChangeBuilder ?? this`).
    fn imports_owner(&self) -> String {
        self.data()
            .dart
            .as_ref()
            .and_then(|d| d.library_path.clone())
            .unwrap_or_else(|| self.path.clone())
    }

    /// Facts about the resolved unit: the library, its URI and path, the
    /// unit URI and the existing imports.
    fn library_facts(&self) -> LibraryFacts {
        let resolved = self.resolved();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let library = resolved.library.library.library;
        LibraryFacts {
            library,
            library_uri: library_uri(&ctx, library),
            library_path: library_path(&ctx, library),
            unit_uri: resolved.unit().uri.to_string(),
            imports: existing_imports(&ctx, library),
        }
    }

    /// The path of the library with [uri] in the element model, if known
    /// (Dart `uriConverter.uriToPath`).
    fn uri_to_path(&self, uri: &str) -> Option<String> {
        if let Some(path) = uri.strip_prefix("file://") {
            return Some(path.to_string());
        }
        let resolved = self.resolved();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let library = ctx.library_by_uri(uri)?;
        Some(library_path(&ctx, library))
    }

    /// Dart `_getLibraryUriText`.
    fn get_library_uri_text(
        &self,
        uri: &str,
        force_absolute: bool,
        force_relative: bool,
    ) -> String {
        let facts = self.library_facts();
        let get_relative_path = |what: &str| relative_path(what, dirname(&facts.library_path));
        if uri.starts_with("file:") {
            if let Some(path) = self.uri_to_path(uri) {
                return get_relative_path(&path);
            }
        }
        let options = self.change.workspace.analysis_options(&self.path);
        let prefer_relative = CodeStyleOptions { options: &options }.use_relative_uris();
        if force_relative || (prefer_relative && !force_absolute) {
            if can_be_relative_import(uri, &facts.unit_uri) {
                if let Some(what) = self.uri_to_path(uri) {
                    return get_relative_path(&what);
                }
            }
        }
        uri.to_string()
    }

    /// Dart `importLibrary`: adds an import of [uri] (when the change is
    /// built) and returns the URI text of the import.
    pub fn import_library(
        &mut self,
        uri: &str,
        prefix: Option<&str>,
        show_name: Option<&str>,
        use_show: bool,
    ) -> String {
        self.import_library_impl(uri, prefix, show_name, true, use_show, false, false)
    }

    /// Dart `importLibraryWithAbsoluteUri`.
    pub fn import_library_with_absolute_uri(
        &mut self,
        uri: &str,
        prefix: Option<&str>,
        show_name: Option<&str>,
        use_show: bool,
    ) -> String {
        self.import_library_impl(uri, prefix, show_name, true, use_show, true, false)
    }

    /// Dart `importLibraryWithRelativeUri`.
    pub fn import_library_with_relative_uri(
        &mut self,
        uri: &str,
        prefix: Option<&str>,
        show_name: Option<&str>,
        use_show: bool,
    ) -> String {
        self.import_library_impl(uri, prefix, show_name, true, use_show, true, true)
    }

    /// Dart `importsLibrary`.
    pub fn imports_library(&self, uri: &str) -> bool {
        let facts = self.library_facts();
        if facts.library_uri == uri {
            return false;
        }
        if facts
            .imports
            .iter()
            .any(|i| i.library_uri.as_deref() == Some(uri))
        {
            return true;
        }
        let owner = self.imports_owner();
        self.change.data.files[&owner]
            .dart
            .as_ref()
            .is_some_and(|d| d.libraries_to_import.contains_key(uri))
    }

    /// Dart `importLibraryElement`: the prefix of an existing import of
    /// [uri] (`Some(None)` for no prefix), or adds an import.
    pub fn import_library_element(
        &mut self,
        uri: &str,
        prefix: Option<&str>,
        show_name: Option<&str>,
        use_show: bool,
    ) -> Option<String> {
        let facts = self.library_facts();
        if facts.library_uri == uri {
            return None;
        }
        for (index, import) in facts.imports.iter().enumerate() {
            if import.library_uri.as_deref() == Some(uri) {
                let import_prefix = import.prefix.clone();
                if !import.combinators.is_empty() {
                    if import_prefix.is_none() {
                        if let Some(show_name) = show_name {
                            self.handle_combinators(&facts.imports[index], show_name);
                            return import_prefix;
                        }
                    }
                } else {
                    return import_prefix;
                }
            }
        }
        self.import_library(uri, prefix, show_name, use_show);
        None
    }

    /// Dart `_handleCombinators`.
    fn handle_combinators(&mut self, import: &ExistingImport, show_name: &str) {
        for (i, (is_show, names)) in import.combinators.iter().enumerate() {
            let (offset, end) = import.combinator_ranges[i];
            let mut names = names.clone();
            if *is_show {
                if !names.iter().any(|n| n == show_name) {
                    names.push(show_name.to_string());
                    names.sort_by(|a, b| dart_compare(a, b));
                    self.add_simple_replacement(
                        offset,
                        end - offset,
                        &format!("show {}", names.join(", ")),
                    );
                }
            }
        }
        for (i, (is_show, names)) in import.combinators.iter().enumerate() {
            let (offset, end) = import.combinator_ranges[i];
            let mut names = names.clone();
            if !*is_show && names.iter().any(|n| n == show_name) {
                names.retain(|n| n != show_name);
                if names.is_empty() {
                    self.add_simple_replacement(offset - 1, end - offset + 1, "");
                } else {
                    self.add_simple_replacement(
                        offset,
                        end - offset,
                        &format!("hide {}", names.join(", ")),
                    );
                }
            }
        }
    }

    /// Dart `_importLibrary`.
    #[allow(clippy::too_many_arguments)]
    pub fn import_library_impl(
        &mut self,
        uri: &str,
        prefix: Option<&str>,
        show_name: Option<&str>,
        is_explicit_import: bool,
        use_show: bool,
        force_absolute: bool,
        force_relative: bool,
    ) -> String {
        let owner = self.imports_owner();
        let existing = self.change.data.files[&owner]
            .dart
            .as_ref()
            .and_then(|d| d.libraries_to_import.get(uri))
            .cloned();
        if let Some(mut import) = existing {
            if let Some(prefix) = prefix {
                import.prefixes.insert(prefix.to_string());
            }
            if let Some(show_name) = show_name {
                import.ensure_shown(show_name, use_show);
            }
            if is_explicit_import {
                import.is_explicitly_imported = true;
            }
            let text = import.uri_text.clone();
            self.change
                .data
                .files
                .get_mut(&owner)
                .unwrap()
                .dart
                .as_mut()
                .unwrap()
                .libraries_to_import
                .insert(uri.to_string(), import);
            return text;
        }
        let uri_text = self.get_library_uri_text(uri, force_absolute, force_relative);
        let facts = self.library_facts();
        let mut shown = Vec::new();
        let mut hidden = Vec::new();
        for import in &facts.imports {
            let Some(library_uri) = &import.library_uri else {
                continue;
            };
            if import.prefix.as_deref().unwrap_or("") != prefix.unwrap_or("") {
                continue;
            }
            let element_uri_text =
                self.get_library_uri_text(library_uri, force_absolute, force_relative);
            if uri_text != element_uri_text {
                continue;
            }
            for (is_show, names) in &import.combinators {
                if *is_show {
                    shown.push(names.clone());
                } else {
                    hidden.push(names.clone());
                }
            }
        }
        let mut import = LibraryImport::new(
            uri_text.clone(),
            prefix.unwrap_or("").to_string(),
            is_explicit_import,
            shown,
            hidden,
        );
        if let Some(show_name) = show_name {
            import.ensure_shown(show_name, use_show);
        }
        if let Some(d) = self
            .change
            .data
            .files
            .get_mut(&owner)
            .and_then(|f| f.dart.as_mut())
        {
            d.libraries_to_import.insert(uri.to_string(), import);
        }
        uri_text
    }
}

struct LibraryFacts {
    #[allow(dead_code)]
    library: EId<LibraryElement>,
    library_uri: String,
    library_path: String,
    unit_uri: String,
    imports: Vec<ExistingImport>,
}

/// The string value of the URI of a directive.
fn directive_uri(ast: &Ast, directive: NodeId) -> Option<String> {
    let uri = if let Some(i) = ast.cast::<ImportDirective>(directive) {
        ast[i].uri.raw()
    } else if let Some(e) = ast.cast::<ExportDirective>(directive) {
        ast[e].uri.raw()
    } else {
        return None;
    };
    dartr_resolver::element_metadata::string_value(ast, uri)
}

/// Dart `_addLibraryImports`.
pub fn add_library_imports(builder: &mut FileEditBuilder<'_, '_>, imports: Vec<LibraryImport>) {
    let resolved = builder.resolved();
    let unit_ref = resolved.unit();
    let ast = &unit_ref.ast;
    let unit = unit_ref.unit;
    let line_info = resolved.line_info().clone();
    let mut library_directive: Option<NodeId> = None;
    let mut import_directives: Vec<NodeId> = Vec::new();
    let mut first_export: Option<NodeId> = None;
    let mut first_part: Option<NodeId> = None;
    let directives: Vec<NodeId> = ast.list_raw(ast[unit].directives).to_vec();
    for &d in &directives {
        if ast.is::<LibraryDirective>(d) {
            library_directive = Some(d);
        } else if ast.is::<ImportDirective>(d) {
            import_directives.push(d);
        } else if ast.is::<ExportDirective>(d) {
            first_export.get_or_insert(d);
        } else if ast.is::<PartDirective>(d) {
            first_part.get_or_insert(d);
        }
    }
    let mut import_list = imports;
    import_list.sort_by(|a, b| a.compare(b));
    let options = builder.change.workspace.analysis_options(&builder.path);
    let style = CodeStyleOptions { options: &options };
    let sort_combinators = style.sort_combinators();
    let quote = style.preferred_quote_for_uris(ast, &import_directives);
    let write_import = |b: &mut EditBuilder<'_, '_, '_>, import: &LibraryImport| {
        let mut prefixes: Vec<&String> = import.prefixes.iter().collect();
        prefixes.sort_by(|a, b| dart_compare(a, b));
        let mut is_first = true;
        for prefix in prefixes {
            if !is_first {
                b.newline();
            }
            is_first = false;
            b.write(&format!("import {quote}"));
            b.write(&import.uri_text);
            b.write(quote);
            if !prefix.is_empty() {
                b.write(" as ");
                b.write(prefix);
            }
            if !import.shown_names.is_empty() {
                b.write(" show ");
                let mut names: Vec<String> = import.all_shown_names().into_iter().collect();
                if sort_combinators {
                    names.sort_by(|a, b| dart_compare(a, b));
                }
                b.write(&names.join(", "));
            }
            if !import.hidden_names.is_empty() {
                b.write(" hide ");
                let mut names: Vec<String> = import.all_hidden_names().into_iter().collect();
                if sort_combinators {
                    names.sort_by(|a, b| dart_compare(a, b));
                }
                b.write(&names.join(", "));
            }
            b.write(";");
        }
    };
    let names_of = |list: NodeList<SimpleIdentifier>| -> Vec<String> {
        ast.list(list)
            .iter()
            .map(|n| ast.tokens.lexeme(ast[*n].token).to_string())
            .collect()
    };

    // Insert imports: between existing imports.
    if !import_directives.is_empty() {
        for import in &import_list {
            let is_dart = import.uri_text.starts_with("dart:");
            let is_package = import.uri_text.starts_with("package:");
            let mut inserted = false;

            let update_show_combinators =
                |builder: &mut FileEditBuilder<'_, '_>, replace: NodeId| {
                    if import.shown_names.is_empty() {
                        return;
                    }
                    let directive = ast.cast::<ImportDirective>(replace).unwrap();
                    let shows: Vec<Id<ShowCombinator>> = ast
                        .list_raw(ast[directive].combinators)
                        .iter()
                        .filter_map(|c| ast.cast::<ShowCombinator>(*c))
                        .collect();
                    let Some(&last) = shows.last() else {
                        return;
                    };
                    let mut existing: IndexSet<String> = IndexSet::new();
                    for s in &shows {
                        for n in names_of(ast[*s].shown_names) {
                            existing.insert(n);
                        }
                    }
                    let added: Vec<String> = import
                        .all_shown_names()
                        .into_iter()
                        .filter(|n| !existing.contains(n))
                        .collect();
                    if !added.is_empty() {
                        let existing_sorted = existing
                            .iter()
                            .collect::<Vec<_>>()
                            .windows(2)
                            .all(|w| dart_compare(w[0], w[1]) != Ordering::Greater);
                        if sort_combinators || existing_sorted {
                            let mut combined: IndexSet<String> =
                                names_of(ast[last].shown_names).into_iter().collect();
                            combined.extend(added.iter().cloned());
                            let mut combined: Vec<String> = combined.into_iter().collect();
                            combined.sort_by(|a, b| dart_compare(a, b));
                            builder.add_simple_replacement(
                                ast.offset(last),
                                ast.length(last),
                                &format!("show {}", combined.join(", ")),
                            );
                        } else {
                            let mut added = added.clone();
                            added.sort_by(|a, b| dart_compare(a, b));
                            builder.add_insertion(ast.end(last), |b| {
                                b.write(&format!(", {}", added.join(", ")));
                            });
                        }
                    }
                };

            let update_hide_combinators =
                |builder: &mut FileEditBuilder<'_, '_>, replace: NodeId| {
                    let directive = ast.cast::<ImportDirective>(replace).unwrap();
                    for &c in ast.list_raw(ast[directive].combinators) {
                        let Some(hide) = ast.cast::<HideCombinator>(c) else {
                            continue;
                        };
                        let offset = ast.offset(hide);
                        let length = ast.end(hide) - offset;
                        let hidden_list = names_of(ast[hide].hidden_names);
                        let hidden_names: IndexSet<String> = hidden_list.iter().cloned().collect();
                        let all_hidden = import.all_hidden_names();
                        let all_shown = import.all_shown_names();
                        let new_names: IndexSet<String> = hidden_names
                            .iter()
                            .filter(|n| all_hidden.contains(*n) && !all_shown.contains(*n))
                            .cloned()
                            .collect();
                        if new_names.is_empty() {
                            let previous = ast.tokens.previous(ast.begin_token(hide));
                            let previous_end = if previous.is_some() {
                                ast.tokens.get(previous).end()
                            } else {
                                0
                            };
                            builder.add_deletion(previous_end, ast.end(hide) - previous_end);
                        } else if hidden_names
                            .iter()
                            .filter(|n| new_names.contains(*n))
                            .count()
                            != hidden_names.len()
                        {
                            let ordered: Vec<String> = if sort_combinators {
                                let mut v: Vec<String> = new_names.iter().cloned().collect();
                                v.sort_by(|a, b| dart_compare(a, b));
                                v
                            } else {
                                hidden_list
                                    .iter()
                                    .filter(|n| new_names.contains(*n))
                                    .cloned()
                                    .collect()
                            };
                            builder.add_simple_replacement(
                                offset,
                                length,
                                &format!("hide {}", ordered.join(", ")),
                            );
                        }
                    }
                };

            let mut insert = |builder: &mut FileEditBuilder<'_, '_>,
                              prev: Option<NodeId>,
                              replace: Option<NodeId>,
                              next: Option<NodeId>,
                              trailing_new_line: bool| {
                if let Some(prev) = prev {
                    let mut offset = ast.end(prev);
                    let next_token = ast.tokens.next(ast.end_token(prev));
                    let mut comment = ast.tokens.get(next_token).preceding_comments;
                    while comment.is_some() {
                        let c = ast.tokens.get(comment);
                        if line_info.on_same_line(c.offset, offset) {
                            offset = c.end();
                        }
                        comment = c.next;
                    }
                    builder.add_insertion(offset, |b| {
                        b.newline();
                        write_import(b, import);
                    });
                } else if let Some(replace) = replace {
                    update_hide_combinators(builder, replace);
                    update_show_combinators(builder, replace);
                } else if let Some(next) = next {
                    let is_first = directives.first() == Some(&next);
                    let offset = if is_first {
                        ast.tokens
                            .get(first_token_after_comment_and_metadata(ast, next))
                            .offset
                    } else {
                        ast.offset(next)
                    };
                    builder.add_insertion(offset, |b| {
                        write_import(b, import);
                        b.newline();
                        if trailing_new_line {
                            b.newline();
                        }
                    });
                }
                inserted = true;
            };

            let mut last_existing: Option<NodeId> = None;
            let mut last_existing_dart: Option<NodeId> = None;
            let mut last_existing_package: Option<NodeId> = None;
            let mut is_last_existing_dart = false;
            let mut is_last_existing_package = false;
            for &existing_import in &import_directives {
                let existing_uri = directive_uri(ast, existing_import).unwrap_or_default();
                let is_existing_dart = existing_uri.starts_with("dart:");
                let is_existing_package = existing_uri.starts_with("package:");
                let is_existing_relative = !existing_uri.contains(':');
                let existing_prefix = ast
                    .cast::<ImportDirective>(existing_import)
                    .and_then(|i| ast[i].prefix)
                    .map(|p| ast.tokens.lexeme(ast[p].token).to_string())
                    .unwrap_or_default();
                let is_replacement =
                    import.uri_text == existing_uri && import.prefix() == existing_prefix;
                let is_new_before_existing =
                    dart_compare(&import.uri_text, &existing_uri) == Ordering::Less;

                if is_replacement {
                    insert(builder, None, Some(existing_import), None, false);
                    break;
                } else if is_dart {
                    if !is_existing_dart || is_new_before_existing {
                        insert(
                            builder,
                            last_existing_dart,
                            None,
                            Some(existing_import),
                            !is_existing_dart,
                        );
                        break;
                    }
                } else if is_package {
                    if is_existing_relative || is_new_before_existing {
                        insert(
                            builder,
                            last_existing_package,
                            None,
                            Some(existing_import),
                            is_existing_relative,
                        );
                        break;
                    }
                } else if !is_existing_dart && !is_existing_package && is_new_before_existing {
                    insert(builder, None, None, Some(existing_import), false);
                    break;
                }

                last_existing = Some(existing_import);
                if is_existing_dart {
                    last_existing_dart = Some(existing_import);
                } else if is_existing_package {
                    last_existing_package = Some(existing_import);
                }
                is_last_existing_dart = is_existing_dart;
                is_last_existing_package = is_existing_package;
            }
            if !inserted {
                let last = last_existing.unwrap();
                builder.add_insertion(ast.end(last), |b| {
                    if is_package {
                        if is_last_existing_dart {
                            b.newline();
                        }
                    } else if !is_dart && (is_last_existing_dart || is_last_existing_package) {
                        b.newline();
                    }
                    b.newline();
                    write_import(b, import);
                });
            }
        }
        return;
    }

    // Insert imports: after the library directive.
    if let Some(library_directive) = library_directive {
        builder.add_insertion(ast.end(library_directive), |b| {
            b.newline();
            b.newline();
            for (i, import) in import_list.iter().enumerate() {
                write_import(b, import);
                if i != import_list.len() - 1 {
                    b.newline();
                }
            }
        });
        return;
    }

    // Insert imports: before any export directives, then part directives.
    if let Some(first) = first_export.or(first_part) {
        builder.add_insertion(ast.offset(first), |b| {
            for import in &import_list {
                write_import(b, import);
                b.newline();
            }
            b.newline();
        });
        return;
    }

    // If still at the beginning of the file, add before the first
    // declaration.
    let declarations = ast.list_raw(ast[unit].declarations);
    let (offset, insert_empty_line_after) = if let Some(&first) = declarations.first() {
        (ast.offset(first), true)
    } else if let Some(last) = builder.data().file_edit.edits.last() {
        // Edits are sorted by decreasing offset: the last has the lowest.
        (last.offset, true)
    } else {
        (ast.end(unit), false)
    };
    builder.add_insertion_with(offset, true, |b| {
        for (i, import) in import_list.iter().enumerate() {
            write_import(b, import);
            b.newline();
            if i == import_list.len() - 1 && insert_empty_line_after {
                b.newline();
            }
        }
    });
}
