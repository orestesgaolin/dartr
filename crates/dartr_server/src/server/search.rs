// Dart source: pkg/analyzer/lib/src/dart/analysis/search.dart (Search.references, directSubtypeReferences, _LocalReferencesVisitor)
// Dart source: pkg/analysis_server/lib/src/services/search/search_engine_internal.dart (appendAllSubtypes)
// Dart source: pkg/analysis_server/lib/src/services/search/hierarchy.dart
// Dart source: pkg/analysis_server/lib/src/services/search/element_references.dart
// Dart source: pkg/analysis_server/lib/src/search/type_hierarchy.dart (TypeHierarchyComputerHelper)
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_references.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_implementation.dart

//! `textDocument/references` and `textDocument/implementation`: the search
//! engine over the analyzed files of all contexts, with the index of each
//! unit ([`crate::index`]).

use std::collections::HashSet;
use std::sync::Arc;

use dartr_ast::*;
use dartr_cli::driver_provider::ResolvedLibraryResult;
use dartr_element::{Ctx, ElemRef, ElementId, FragmentFlags, NoopSink, Tag, TypeKind};
use dartr_resolver::error::support;
use dartr_typesystem::member;
use serde_json::{Value, json};

use super::Server;
use super::nav::ResolvedUnitRef;
use crate::index::{ElementKey, RelationKind, UnitIndex};
use crate::mapping::{self, ErrorOr};
use crate::uri::path_to_uri;

/// An element with the resolved library whose element model it is in.
#[derive(Clone)]
pub(crate) struct SElem {
    pub lib: Arc<ResolvedLibraryResult>,
    pub unit: usize,
    pub id: ElementId,
}

impl SElem {
    fn with<R>(&self, f: impl FnOnce(&Ctx<'_>) -> R) -> R {
        let sink = NoopSink;
        let ctx = self.lib.ctx(self.unit, &sink);
        f(&ctx)
    }

    fn key(&self) -> Option<ElementKey> {
        self.with(|ctx| crate::index::element_key(ctx, self.id))
    }

    /// The identity of the element in Dart: the same element of the same
    /// driver (each driver has its own element model).
    fn identity(&self) -> Option<(usize, ElementKey)> {
        self.key().map(|k| (self.lib.context, k))
    }

    fn same(&self, other: ElementId) -> SElem {
        SElem {
            lib: self.lib.clone(),
            unit: self.unit,
            id: other,
        }
    }
}

/// A search match: the file, offset and length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Match {
    pub path: String,
    pub offset: u32,
    pub length: u32,
    /// The context of the driver that found the match.
    pub context: usize,
}

const REFERENCES: &[RelationKind] = &[
    RelationKind::IsReadWrittenBy,
    RelationKind::IsInvokedBy,
    RelationKind::IsReadBy,
    RelationKind::IsWrittenBy,
    RelationKind::IsReferencedByNamedArgument,
    RelationKind::IsReferencedBy,
];

const CONSTRUCTOR: &[RelationKind] = &[
    RelationKind::IsInvokedBy,
    RelationKind::IsInvokedByDotShorthandsConstructor,
    RelationKind::IsInvokedByEnumConstantWithoutArguments,
    RelationKind::IsReferencedBy,
    RelationKind::IsReferencedByConstructorTearOff,
    RelationKind::IsReferencedByDotShorthandConstructorTearOff,
];

const GETTER: &[RelationKind] = &[
    RelationKind::IsReferencedBy,
    RelationKind::IsReferencedByPatternField,
    RelationKind::IsInvokedBy,
];

const FIELD: &[RelationKind] = &[
    RelationKind::IsWrittenBy,
    RelationKind::IsReferencedBy,
    RelationKind::IsReferencedByPatternField,
];

const SETTER_OF_FIELD: &[RelationKind] = &[RelationKind::IsReferencedBy];

const FUNCTION: &[RelationKind] = &[
    RelationKind::IsReferencedByPatternField,
    RelationKind::IsReferencedBy,
    RelationKind::IsInvokedBy,
];

const SUBTYPES: &[RelationKind] = &[
    RelationKind::IsExtendedBy,
    RelationKind::IsMixedInBy,
    RelationKind::IsImplementedBy,
    RelationKind::Constrains,
];

/// Dart VM `String.hashCode` (`StringHasher`: one-at-a-time over the
/// UTF-16 code units, 30 bits, 0 is 1).
pub(crate) fn dart_string_hash(s: &str) -> u32 {
    let mut h: u32 = 0;
    for c in s.encode_utf16() {
        h = h.wrapping_add(c as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h = h.wrapping_add(h << 15);
    h &= (1 << 30) - 1;
    if h == 0 { 1 } else { h }
}

/// The iteration order of a Dart VM `HashMap` (`_HashMap`: 8 buckets at
/// first, new entries first in their bucket, twice the buckets when more
/// than 3/4 full) after inserting the distinct [hashes] in order: indexes
/// into [hashes].
pub(crate) fn dart_hash_map_order(hashes: &[u32]) -> Vec<usize> {
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); 8];
    let mut count = 0usize;
    for (i, &h) in hashes.iter().enumerate() {
        let length = buckets.len();
        buckets[h as usize & (length - 1)].insert(0, i);
        count += 1;
        if (count << 2) > ((length << 1) + length) {
            let new_length = length << 1;
            let mut new_buckets: Vec<Vec<usize>> = vec![Vec::new(); new_length];
            for bucket in &buckets {
                for &e in bucket {
                    new_buckets[hashes[e] as usize & (new_length - 1)].insert(0, e);
                }
            }
            buckets = new_buckets;
        }
    }
    buckets.into_iter().flatten().collect()
}

/// The identifier-like words of [content] (a superset of Dart
/// `FileState.referencedNames`).
fn words(content: &str) -> HashSet<String> {
    content
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Dart `Folder.getChildren` recursively: the Dart files under [folder], in
/// directory order (`discoverAvailableFiles.discoverRecursively`).
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

/// The paths of the files of the library of [e] (Dart
/// `LibraryFileKind.files`: the library, then its parts, depth first).
fn library_file_paths(element: &SElem, e: ElementId) -> Vec<String> {
    element.with(|ctx| {
        let Some(library) = support::library_of(ctx, e) else {
            return Vec::new();
        };
        let first = ctx.get(library).first_fragment();
        let mut out = Vec::new();
        let mut stack = vec![first];
        while let Some(f) = stack.pop() {
            out.push(ctx.fragment(f).source.path.to_string());
            for part in ctx.fragment(f).parts.iter().rev() {
                if let dartr_element::DirectiveUri::Unit { library_fragment, .. } = &part.directive.uri {
                    stack.push(*library_fragment);
                }
            }
        }
        out
    })
}

/// Dart `OwnedFiles`: the driver (context) of each file that a search
/// looks at.
#[derive(Default)]
pub(crate) struct OwnedFiles {
    /// The analyzed files, by the context that adds them.
    pub added: indexmap::IndexMap<String, usize>,
    /// Other files, by the first context that knows them.
    pub known: indexmap::IndexMap<String, usize>,
    /// Whether `discoverAvailableFiles` ran.
    pub discovered: bool,
}

impl OwnedFiles {
    /// Dart `addKnown`; returns whether the file is new.
    fn add_known(&mut self, path: String, context: usize) -> bool {
        if self.added.contains_key(&path) || self.known.contains_key(&path) {
            return false;
        }
        self.known.insert(path, context);
        true
    }
}

/// The files that a search looks at (Dart `OwnedFiles.filesFor(driver)` of
/// each driver after `discoverAvailableFiles`): by context, in the order of
/// `SearchEngineImpl._drivers`.
pub(crate) type SearchScope = Vec<(usize, Vec<String>)>;

impl Server {
    /// Dart `OwnedFiles` after `discoverAvailableFiles` of all drivers. A
    /// file is owned by the driver to which it is added (the analyzed files
    /// of the context), else by the first driver that knows it: the files
    /// that the analysis found (contexts in collection order), then the
    /// SDK libraries and the Dart files of the package `lib` folders
    /// (drivers in search order, once), then the files that later
    /// resolutions find. The ownership only grows (Dart: the maps are
    /// append-only), so the files of a search depend on the earlier
    /// requests, as in Dart. The drivers are in the order of the
    /// `HashMap<Folder, AnalysisDriver>` of the context manager.
    fn search_scope(&mut self) -> Arc<SearchScope> {
        if let Some(scope) = &self.search_scope {
            return scope.clone();
        }
        let Some(collection) = &self.collection else {
            return Arc::new(Vec::new());
        };
        let count = collection.contexts.len();
        if self.owned.added.is_empty() {
            for (index, context) in collection.contexts.iter().enumerate() {
                let files: Vec<String> = context
                    .root
                    .analyzed_files()
                    .into_iter()
                    .filter(|f| f.ends_with(".dart") && !self.is_excluded(f))
                    .collect();
                // Dart creates the library of a part with a URI before the
                // part is known (`_newFile` calls `onNewFile` after
                // `refresh`).
                for f in &files {
                    if let Some(library) = self.session.part_of_uri_library(index, f) {
                        if files.contains(&library) {
                            self.owned.added.entry(library).or_insert(index);
                        }
                    }
                    self.owned.added.entry(f.clone()).or_insert(index);
                }
            }
        }
        for index in 0..count {
            for f in self.session.known_files(index) {
                self.owned.add_known(f, index);
            }
        }
        let hashes: Vec<u32> = collection
            .contexts
            .iter()
            .map(|c| dart_string_hash(&c.root.root))
            .collect();
        let order = dart_hash_map_order(&hashes);
        if !self.owned.discovered {
            self.owned.discovered = true;
            for &index in &order {
                let context = &collection.contexts[index];
                let sdk = context.sdk.as_ref().or(collection.sdk.as_ref());
                let mut discovered = Vec::new();
                if let Some(sdk) = sdk {
                    discovered.extend(sdk.libraries().iter().filter_map(|l| sdk.map_dart_uri(&l.short_name)));
                }
                for package in context.packages.packages() {
                    dart_files_recursively(&package.lib, &mut discovered);
                }
                for f in discovered {
                    if std::path::Path::new(&f).is_file() {
                        self.owned.add_known(f, index);
                    }
                }
            }
        }
        let scope: SearchScope = order
            .iter()
            .map(|&index| {
                let files = self
                    .owned
                    .added
                    .iter()
                    .chain(self.owned.known.iter())
                    .filter(|(_, owner)| **owner == index)
                    .map(|(f, _)| f.clone())
                    .collect();
                (index, files)
            })
            .collect();
        if std::env::var_os("DARTR_DEBUG_SEARCH").is_some() {
            for (index, files) in &scope {
                let root = &collection.contexts[*index].root.root;
                eprintln!("dartr: search scope of context {index} ({root}): {} files", files.len());
            }
        }
        let scope = Arc::new(scope);
        self.search_scope = Some(scope.clone());
        scope
    }

    /// The context that owns [path] in a search.
    fn owner_of(&mut self, path: &str) -> Option<usize> {
        self.search_scope()
            .iter()
            .find(|(_, files)| files.iter().any(|f| f == path))
            .map(|(index, _)| *index)
    }

    /// Whether [content] of [path] has one of [names] (Dart
    /// `FileState.referencedNames`); cached until a file changes.
    fn mentions_any(&mut self, path: &str, names: &[String]) -> bool {
        let words = match self.search_words.get(path) {
            Some(w) => w.clone(),
            None => {
                let w = Arc::new(words(&self.content(path).unwrap_or_default()));
                self.search_words.insert(path.to_string(), w.clone());
                w
            }
        };
        names.iter().any(|n| words.contains(n))
    }

    /// The index of the unit [path] in the driver of [context] (cached
    /// until a file changes).
    fn unit_index(&mut self, path: &str, context: usize) -> Option<Arc<UnitIndex>> {
        let key = (context, path.to_string());
        if let Some(i) = self.indexes.get(&key) {
            return Some(i.clone());
        }
        let resolved = self.require_resolved_unit_in(path, Some(context)).ok()?;
        // Dart: the resolution creates the files of the library and of the
        // libraries it depends on in the driver (known to it unless another
        // driver knows them).
        let mut grew = false;
        for input in &resolved.library.inputs {
            grew |= self.owned.add_known(input.path.to_string(), context);
        }
        for f in self.session.known_files(context) {
            grew |= self.owned.add_known(f, context);
        }
        if grew {
            self.search_scope = None;
        }
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let index = Arc::new(crate::index::index_unit(&ctx, &unit.ast, &unit.tables, unit.unit));
        self.indexes.insert(key, index.clone());
        Some(index)
    }

    /// Dart `Search._addResults`: the relations with [kinds] of [element]
    /// in the files of each driver that can reference it.
    fn search_index(&mut self, element: &SElem, kinds: &[RelationKind]) -> Vec<Match> {
        let Some(name) = element.with(|ctx| ctx.element_data(element.id).and_then(|d| d.name).map(|n| ctx.name_str(n).to_string())) else {
            return Vec::new();
        };
        let Some(key) = element.key() else {
            return Vec::new();
        };
        let mut reference_names = vec![name.clone()];
        // The library files of the declaring library (and of the class for
        // an unnamed constructor).
        let mut library_files: Vec<Vec<String>> = Vec::new();
        let library_files_of = |e: ElementId| library_file_paths(element, e);
        library_files.push(library_files_of(element.id));
        if element.id.tag() == Tag::Constructor && name == "new" {
            let class = element.with(|ctx| ctx.element_data(element.id).and_then(|d| d.enclosing));
            if let Some(class) = class {
                if let Some(class_name) = element.with(|ctx| ctx.element_data(class).and_then(|d| d.name).map(|n| ctx.name_str(n).to_string())) {
                    reference_names.push(class_name);
                }
                library_files.push(library_files_of(class));
            }
        }
        let mut results = Vec::new();
        let scope = self.search_scope();
        for (context, owned) in scope.iter() {
            let mut files: Vec<String> = Vec::new();
            for lib in &library_files {
                if lib.first().is_some_and(|f| owned.contains(f)) {
                    for f in lib {
                        if !files.contains(f) {
                            files.push(f.clone());
                        }
                    }
                }
            }
            if !name.starts_with('_') {
                for f in owned {
                    if files.contains(f) {
                        continue;
                    }
                    if self.mentions_any(f, &reference_names) {
                        files.push(f.clone());
                    }
                }
            }
            for f in files {
                let Some(index) = self.unit_index(&f, *context) else { continue };
                for r in index.relations_of(&key, |k| kinds.contains(&k)) {
                    results.push(Match {
                        path: f.clone(),
                        offset: r.offset,
                        length: r.length,
                        context: *context,
                    });
                }
            }
        }
        results
    }

    /// Dart `Search.references(element)` over all drivers.
    pub(crate) fn search_references(&mut self, element: &SElem) -> Vec<Match> {
        let id = element.id;
        match id.tag() {
            Tag::Extension | Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Setter | Tag::TypeAlias => {
                self.search_index(element, REFERENCES)
            }
            Tag::Constructor => self.search_index(element, CONSTRUCTOR),
            Tag::Getter => self.search_index(element, GETTER),
            Tag::Field | Tag::TopLevelVariable => {
                let (getter, setter, origin) = element.with(|ctx| {
                    let (getter, setter) = match ctx.any(id) {
                        dartr_element::AnyElement::Field(f) => (f.getter.map(|g| g.raw()), f.setter.map(|s| s.raw())),
                        dartr_element::AnyElement::TopLevelVariable(v) => (v.getter.map(|g| g.raw()), v.setter.map(|s| s.raw())),
                        _ => (None, None),
                    };
                    let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, id);
                    (getter, setter, flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION))
                });
                let mut results = Vec::new();
                if origin {
                    results.extend(self.search_index(element, FIELD));
                }
                if let Some(g) = getter {
                    results.extend(self.search_index(&element.same(g), GETTER));
                }
                if let Some(s) = setter {
                    results.extend(self.search_index(&element.same(s), SETTER_OF_FIELD));
                }
                results
            }
            Tag::LocalFunction => self.search_local(element, &|ast, n| ast.is::<Block>(n)),
            Tag::Method | Tag::TopLevelFunction => self.search_index(element, FUNCTION),
            Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable => {
                self.search_pattern_variable(element)
            }
            Tag::Label | Tag::LocalVariable => self.search_local(element, &|ast, n| {
                ast.is::<Block>(n)
                    || ast.is::<ForElement>(n)
                    || ast.is::<FunctionBody>(n)
                    || ast.is::<TopLevelVariableDeclaration>(n)
                    || ast.is::<SwitchExpression>(n)
                    || ast.parent(n).is_some_and(|p| ast.is::<CompilationUnit>(p))
            }),
            Tag::Library => self.search_library(element),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                let enclosing = element.with(|ctx| ctx.element_data(id).and_then(|d| d.enclosing));
                if enclosing.is_some_and(|e| e.tag() == Tag::LocalFunction) {
                    self.search_local(element, &|ast, n| {
                        ast.is::<Block>(n) || ast.parent(n).is_some_and(|p| ast.is::<CompilationUnit>(p))
                    })
                } else {
                    self.search_index(element, REFERENCES)
                }
            }
            Tag::Prefix => self.search_prefix(element),
            Tag::TypeParameter => self.search_local(element, &|ast, n| {
                ast.parent(n).is_some_and(|p| ast.is::<CompilationUnit>(p))
            }),
            _ => Vec::new(),
        }
    }

    /// The resolved unit of the file of [element]'s first fragment, when the
    /// file is analyzed (Dart `_ownsSource`), with the fragment.
    fn element_unit(&mut self, element: &SElem) -> Option<(ResolvedUnitRef, u32)> {
        let (path, name_offset) = element.with(|ctx| {
            let first = ctx.element_data(element.id)?.first_fragment;
            let path = crate::navigation::fragment_path(ctx, first)?;
            let data = ctx.fragment_data(first)?;
            let offset = data.name_offset.or(if element.id.tag() == Tag::Label {
                data.first_token_offset
            } else {
                None
            });
            Some((path, offset))
        })?;
        let owner = self.owner_of(&path)?;
        let resolved = self.require_resolved_unit_in(&path, Some(owner)).ok()?;
        Some((resolved, name_offset?))
    }

    /// Dart `_searchReferences_Local`.
    fn search_local(&mut self, element: &SElem, is_root: &dyn Fn(&Ast, NodeId) -> bool) -> Vec<Match> {
        let Some((resolved, name_offset)) = self.element_unit(element) else {
            return Vec::new();
        };
        let unit = resolved.unit();
        let ast = &unit.ast;
        let Some(node) = ast.node_covering(unit.unit, name_offset, 0) else {
            return Vec::new();
        };
        let mut current = Some(node);
        let mut root = None;
        while let Some(n) = current {
            if is_root(ast, n) || ast.is::<CompilationUnit>(n) {
                root = Some(n);
                break;
            }
            current = ast.parent(n);
        }
        let Some(root) = root else { return Vec::new() };
        if ast.is::<CompilationUnit>(root) {
            return Vec::new();
        }
        // Elements are compared within the same resolved library.
        let elements = vec![element.id];
        self.local_references(&resolved, root, &elements)
    }

    fn local_references(&self, resolved: &ResolvedUnitRef, root: NodeId, elements: &[ElementId]) -> Vec<Match> {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let mut visitor = LocalReferencesVisitor {
            ctx: &ctx,
            tables: &unit.tables,
            elements,
            results: Vec::new(),
        };
        unit.ast.accept(root, &mut visitor);
        let path = resolved.library.inputs[resolved.index].path.to_string();
        visitor
            .results
            .into_iter()
            .map(|(offset, length)| Match {
                path: path.clone(),
                offset,
                length,
                context: resolved.library.context,
            })
            .collect()
    }

    /// Dart `_searchReferences_PatternVariable`.
    fn search_pattern_variable(&mut self, element: &SElem) -> Vec<Match> {
        let Some((resolved, _)) = self.element_unit(element) else {
            return Vec::new();
        };
        let variables = element.with(|ctx| {
            // `rootVariable`: the outermost join.
            let mut root = element.id;
            while let Some(j) = dartr_resolver::element_ext::pattern_variable_join(ctx, root) {
                root = j;
            }
            // `transitiveVariables`.
            let mut out = Vec::new();
            let mut stack = vec![root];
            while let Some(v) = stack.pop() {
                if v.tag() == Tag::JoinPatternVariable {
                    let mut components = dartr_resolver::element_ext::join_pattern_variable_components(ctx, v);
                    components.reverse();
                    stack.extend(components);
                } else {
                    out.push(v);
                }
            }
            if root.tag() == Tag::JoinPatternVariable {
                out
            } else {
                vec![root]
            }
        });
        let bind = variables.iter().copied().find(|v| v.tag() == Tag::BindPatternVariable);
        let Some(bind) = bind else { return Vec::new() };
        let Some(node) = element.with(|ctx| dartr_resolver::element_ext::bind_pattern_variable_node(ctx, bind)) else {
            return Vec::new();
        };
        let unit = resolved.unit();
        let ast = &unit.ast;
        let mut current = Some(node);
        let mut root = None;
        while let Some(n) = current {
            if ast.is::<SwitchExpression>(n) || ast.is::<Block>(n) || ast.is::<ExpressionFunctionBody>(n) {
                root = Some(n);
                break;
            }
            current = ast.parent(n);
        }
        let Some(root) = root else { return Vec::new() };
        self.local_references(&resolved, root, &variables)
    }

    /// Dart `_searchReferences_Prefix`.
    fn search_prefix(&mut self, element: &SElem) -> Vec<Match> {
        let mut results = Vec::new();
        let paths: Vec<String> = element.lib.inputs.iter().map(|u| u.path.to_string()).collect();
        for path in paths {
            if let Ok(resolved) = self.require_resolved_unit_in(&path, Some(element.lib.context)) {
                let root = resolved.unit().unit.raw();
                results.extend(self.local_references(&resolved, root, &[element.id]));
            }
        }
        results
    }

    /// Dart `_searchReferences_Library`: the `part of` directives in the
    /// files of the library, when a driver owns the library.
    fn search_library(&mut self, element: &SElem) -> Vec<Match> {
        let paths = library_file_paths(element, element.id);
        let Some(first) = paths.first() else {
            return Vec::new();
        };
        let Some(owner) = self.owner_of(first) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for path in paths {
            let Ok(resolved) = self.require_resolved_unit_in(&path, Some(owner)) else {
                continue;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
            for &d in ast.list_raw(ast[unit.unit].directives) {
                if let Some(p) = ast.cast::<PartOfDirective>(d) {
                    let target = ast[p].library_name.map(|n| n.raw()).or(ast[p].uri.map(|u| u.raw()));
                    if let Some(t) = target {
                        results.push(Match {
                            path: path.clone(),
                            offset: ast.offset(t),
                            length: ast.length(t),
                            context: owner,
                        });
                    }
                }
            }
        }
        results
    }

    /// Dart `directSubtypeReferences` over all drivers: the classes whose
    /// declarations extend, mix in, implement or constrain [class].
    fn direct_subtypes(&mut self, class: &SElem) -> Vec<SElem> {
        let matches = self.search_index(class, SUBTYPES);
        let mut out = Vec::new();
        for m in matches {
            let Ok(resolved) = self.require_resolved_unit_in(&m.path, Some(m.context)) else {
                continue;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
            // The enclosing fragment: the innermost type declaration.
            let mut node = ast.node_covering(unit.unit, m.offset, 0);
            let mut found = None;
            while let Some(n) = node {
                if ast.is::<ClassDeclaration>(n)
                    || ast.is::<MixinDeclaration>(n)
                    || ast.is::<EnumDeclaration>(n)
                    || ast.is::<ExtensionTypeDeclaration>(n)
                    || ast.is::<ClassTypeAlias>(n)
                {
                    found = Some(n);
                    break;
                }
                node = ast.parent(n);
            }
            let Some(declaration) = found else { continue };
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            if let Some(e) = support::declared_element(&ctx, &unit.tables, declaration) {
                out.push(SElem {
                    lib: resolved.library.clone(),
                    unit: resolved.index,
                    id: e,
                });
            }
        }
        out
    }

    /// Dart `SearchEngine.appendAllSubtypes(type, allSubtypes)`.
    fn append_all_subtypes(&mut self, class: &SElem, all: &mut Vec<SElem>, keys: &mut Vec<(usize, ElementKey)>) {
        for sub in self.direct_subtypes(class) {
            let Some(key) = sub.identity() else { continue };
            if keys.contains(&key) {
                continue;
            }
            keys.push(key);
            all.push(sub.clone());
            self.append_all_subtypes(&sub, all, keys);
        }
    }

    /// Dart `getHierarchyMembersAndParameters(member,
    /// includeParametersForFields: true)`.
    fn hierarchy_members_and_parameters(&mut self, member: &SElem) -> (Vec<SElem>, Vec<SElem>) {
        let id = member.id;
        let mut members = Vec::new();
        let mut parameters = Vec::new();
        let enclosing = member.with(|ctx| ctx.element_data(id).and_then(|d| d.enclosing));
        if enclosing.is_some_and(|e| e.tag() == Tag::Extension) {
            members.push(member.clone());
            return (members, parameters);
        }
        let is_static = member.with(|ctx| member::is_static(ctx, ElemRef::Base(id)));
        if id.tag() == Tag::Constructor || (matches!(id.tag(), Tag::Field | Tag::Method) && is_static) {
            members.push(member.clone());
            return (members, parameters);
        }
        let Some(class) = enclosing.and_then(|e| e.cast::<dartr_element::InterfaceElement>()) else {
            return (members, parameters);
        };
        let name = member.with(|ctx| support::display_name(ctx, id));
        let is_private = name.starts_with('_');
        let member_library = member.with(|ctx| support::library_of(ctx, id));
        let search_classes: Vec<ElementId> = member.with(|ctx| {
            let mut out: Vec<ElementId> = dartr_typesystem::class_hierarchy::implemented_interfaces(ctx, class)
                .iter()
                .filter_map(|&t| match ctx.ty(t) {
                    TypeKind::Interface { element, .. } => Some(element.raw()),
                    _ => None,
                })
                .filter(|&e| !is_private || support::library_of(ctx, e) == member_library)
                .collect();
            out.push(class.raw());
            out
        });
        let mut sub_classes: Vec<SElem> = Vec::new();
        let mut keys: Vec<(usize, ElementKey)> = Vec::new();
        for super_class in search_classes {
            let declares = member.with(|ctx| !class_members(ctx, super_class, Some(&name)).is_empty());
            if !declares {
                continue;
            }
            let sc = member.same(super_class);
            self.append_all_subtypes(&sc, &mut sub_classes, &mut keys);
            if let Some(key) = sc.identity()
                && !keys.contains(&key)
            {
                keys.push(key);
                sub_classes.push(sc);
            }
        }
        if is_private {
            sub_classes.retain(|s| {
                let library_path = s.with(|ctx| {
                    support::library_of(ctx, s.id).map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.path.to_string())
                });
                let member_path = member.with(|ctx| {
                    member_library.map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.path.to_string())
                });
                library_path == member_path
            });
        }
        let member_key = member.key();
        let mut member_keys: Vec<(usize, ElementKey)> = Vec::new();
        for sub in &sub_classes {
            let children = sub.with(|ctx| children_named(ctx, sub.id, &name));
            for c in children {
                if matches!(c.tag(), Tag::Field | Tag::Method) {
                    let e = sub.same(c);
                    if let Some(k) = e.identity()
                        && !member_keys.contains(&k)
                    {
                        member_keys.push(k);
                        members.push(e);
                    }
                }
            }
            if id.tag() == Tag::Field && sub.lib.context == member.lib.context {
                let field_formals: Vec<ElementId> = sub.with(|ctx| {
                    let Some(interface) = sub.id.cast::<dartr_element::InterfaceElement>() else {
                        return Vec::new();
                    };
                    let mut out = Vec::new();
                    for &c in &ctx.interface(interface).constructors {
                        for &p in &ctx.executable(c.upcast()).formal_params {
                            if p.raw().tag() == Tag::FieldFormalParameter
                                && let dartr_element::AnyElement::FormalParameter(fp) = ctx.any(p.raw())
                                && let Some(f) = fp.field.get()
                                && crate::index::element_key(ctx, f.raw()) == member_key
                            {
                                out.push(p.raw());
                            }
                        }
                    }
                    out
                });
                for p in field_formals {
                    parameters.push(sub.same(p));
                }
            }
        }
        (members, parameters)
    }

    /// Dart `getHierarchyNamedParameters`.
    fn hierarchy_named_parameters(&mut self, parameter: &SElem) -> Vec<SElem> {
        let (named, name, enclosing) = parameter.with(|ctx| {
            let data = ctx.get(dartr_element::EId::<dartr_element::FormalParameterElement>::from_raw(parameter.id));
            (
                data.kind.is_named(),
                data.name.map(|n| ctx.name_str(n).to_string()),
                ctx.element_data(parameter.id).and_then(|d| d.enclosing),
            )
        });
        if named
            && let Some(method) = enclosing.filter(|e| e.tag() == Tag::Method)
        {
            let (members, _) = self.hierarchy_members_and_parameters(&parameter.same(method));
            let mut out = Vec::new();
            for m in members {
                if m.id.tag() != Tag::Method {
                    continue;
                }
                let found = m.with(|ctx| {
                    ctx.executable(m.id.cast::<dartr_element::ExecutableElement>()?)
                        .formal_params
                        .iter()
                        .copied()
                        .find(|&p| {
                            let d = ctx.get(p);
                            d.kind.is_named() && d.name.map(|n| ctx.name_str(n).to_string()) == name
                        })
                        .map(|p| p.raw())
                });
                if let Some(p) = found {
                    out.push(m.same(p));
                }
            }
            return out;
        }
        vec![parameter.clone()]
    }

    /// Dart `ReferencesHandler.handle`.
    pub(crate) fn references(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let element = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            let ast = &unit.ast;
            let u = crate::element_locator::Unit {
                ctx: &ctx,
                ast,
                tables: &unit.tables,
            };
            let mut node = ast.node_covering(unit.unit, offset, 0);
            if let Some(n) = node
                && ast.is::<TypeParameterList>(n)
            {
                node = ast.parent(n);
            }
            node.and_then(|n| crate::element_locator::get_element(&u, n)).map(|e| match e.tag() {
                Tag::FieldFormalParameter => match ctx.any(e) {
                    dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()).unwrap_or(e),
                    _ => e,
                },
                Tag::Getter | Tag::Setter => dartr_resolver::element_metadata::accessor_variable_any(&ctx, e).unwrap_or(e),
                _ => e,
            })
        };
        let Some(element) = element else {
            return Ok(Value::Null);
        };
        let element = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        // Dart `_getRefElements`.
        let is_named_parameter = matches!(
            element.id.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) && element.with(|ctx| {
            ctx.get(dartr_element::EId::<dartr_element::FormalParameterElement>::from_raw(element.id))
                .kind
                .is_named()
        });
        let ref_elements: Vec<SElem> = if is_named_parameter {
            self.hierarchy_named_parameters(&element)
        } else if matches!(element.id.tag(), Tag::Method | Tag::Field | Tag::Constructor) {
            let (mut members, parameters) = self.hierarchy_members_and_parameters(&element);
            members.extend(parameters);
            members
        } else {
            vec![element.clone()]
        };
        let mut locations = Vec::new();
        for e in &ref_elements {
            for m in self.search_references(e) {
                if let Some(lines) = self.line_info_of(&m.path) {
                    locations.push(json!({
                        "uri": path_to_uri(&m.path),
                        "range": mapping::to_range(&lines, m.offset, m.length),
                    }));
                }
            }
        }
        let include_declaration = params
            .pointer("/context/includeDeclaration")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if include_declaration {
            for location in self.declarations(&element) {
                locations.push(location);
            }
        }
        Ok(Value::Array(locations))
    }

    /// Dart `_getDeclarations(element)`: `fragmentToLocation` of each
    /// fragment of `element.nonSynthetic`.
    fn declarations(&self, element: &SElem) -> Vec<Value> {
        let fragments: Vec<(String, u32, u32)> = element.with(|ctx| {
            let e = dartr_element::diagnostics::non_synthetic(ctx, element.id);
            let mut out = Vec::new();
            let mut f = ctx.element_data(e).map(|d| d.first_fragment);
            while let Some(id) = f {
                let Some(data) = ctx.fragment_data(id) else { break };
                let path = crate::navigation::fragment_path(ctx, id);
                let mut name_offset = data.name_offset.or(if id.tag() == Tag::Label {
                    data.first_token_offset
                } else {
                    None
                });
                let mut name_length = data.name.map(|n| ctx.name_str(n).encode_utf16().count() as u32);
                if name_offset.is_none()
                    && let Some(c) = id.cast::<dartr_element::ConstructorFragment>()
                {
                    let c = ctx.fragment(c);
                    name_offset = c.type_name_offset;
                    name_length = c.type_name.map(|n| ctx.name_str(n).encode_utf16().count() as u32);
                }
                if let (Some(path), Some(o), Some(l)) = (path, name_offset, name_length) {
                    out.push((path, o, l));
                }
                f = data.next_fragment;
            }
            out
        });
        fragments
            .into_iter()
            .filter_map(|(path, o, l)| {
                let lines = self.line_info_of(&path)?;
                Some(json!({"uri": path_to_uri(&path), "range": mapping::to_range(&lines, o, l)}))
            })
            .collect()
    }

    /// Dart `ImplementationHandler.handle`.
    pub(crate) fn implementation(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let element = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            let u = crate::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            unit.ast
                .node_covering(unit.unit, offset, 0)
                .and_then(|n| crate::element_locator::get_element(&u, n))
        };
        let Some(element) = element else {
            return Ok(json!([]));
        };
        let pivot = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        let helper = pivot.with(|ctx| HierarchyHelper::from_element(ctx, pivot.lib.context, element));
        let Some(pivot_class) = helper.pivot_class else {
            return Ok(json!([]));
        };
        let pivot_class = pivot.same(pivot_class);
        let needs_member = pivot_class
            .with(|ctx| helper.find_member(ctx, pivot_class.lib.context, pivot_class.id))
            .is_some();
        let mut all = Vec::new();
        let mut keys = Vec::new();
        self.append_all_subtypes(&pivot_class, &mut all, &mut keys);
        let mut seen: Vec<(usize, ElementKey)> = Vec::new();
        let mut locations = Vec::new();
        for sub in all {
            let target = if needs_member {
                let found = sub.with(|ctx| {
                    helper
                        .find_member(ctx, sub.lib.context, sub.id)
                        .map(|m| dartr_element::diagnostics::non_synthetic(ctx, m))
                });
                match found {
                    Some(m) => sub.same(m),
                    None => continue,
                }
            } else {
                sub
            };
            if let Some(k) = target.identity() {
                if seen.contains(&k) {
                    continue;
                }
                seen.push(k);
            }
            let location = target.with(|ctx| {
                let first = ctx.element_data(target.id)?.first_fragment;
                let path = crate::navigation::fragment_path(ctx, first)?;
                let data = ctx.fragment_data(first)?;
                let name = data.name?;
                Some((path, data.name_offset?, ctx.name_str(name).encode_utf16().count() as u32))
            });
            if let Some((path, o, l)) = location
                && let Some(lines) = self.line_info_of(&path)
            {
                locations.push(json!({"uri": path_to_uri(&path), "range": mapping::to_range(&lines, o, l)}));
            }
        }
        Ok(Value::Array(locations))
    }
}

/// Dart `getClassMembers(clazz, name)`: the non-constructor executables
/// and fields of [class] with the display name [name].
fn class_members(ctx: &Ctx<'_>, class: ElementId, name: Option<&str>) -> Vec<ElementId> {
    let Some(instance) = class.cast::<dartr_element::InstanceElement>() else {
        return Vec::new();
    };
    let data = ctx.instance(instance);
    let mut out = Vec::new();
    let children: Vec<ElementId> = data
        .fields
        .iter()
        .map(|f| f.raw())
        .chain(data.getters.iter().map(|g| g.raw()))
        .chain(data.setters.iter().map(|s| s.raw()))
        .chain(data.methods.iter().map(|m| m.raw()))
        .collect();
    for c in children {
        if name.is_some_and(|n| support::display_name(ctx, c) != n) {
            continue;
        }
        out.push(c);
    }
    out
}

/// Dart `getChildren(parent, name)`: the children of [parent] (fields,
/// getters, setters, methods, constructors) with the lookup name [name].
fn children_named(ctx: &Ctx<'_>, parent: ElementId, name: &str) -> Vec<ElementId> {
    let Some(instance) = parent.cast::<dartr_element::InstanceElement>() else {
        return Vec::new();
    };
    let data = ctx.instance(instance);
    let mut children: Vec<ElementId> = data
        .fields
        .iter()
        .map(|f| f.raw())
        .chain(data.getters.iter().map(|g| g.raw()))
        .chain(data.setters.iter().map(|s| s.raw()))
        .chain(data.methods.iter().map(|m| m.raw()))
        .collect();
    if let Some(interface) = parent.cast::<dartr_element::InterfaceElement>() {
        children.extend(ctx.interface(interface).constructors.iter().map(|c| c.raw()));
    }
    children
        .into_iter()
        .filter(|&c| member::lookup_name(ctx, ElemRef::Base(c)).as_deref() == Some(name))
        .collect()
}

/// Dart `TypeHierarchyComputerHelper`.
struct HierarchyHelper {
    /// The context of the element model of the pivot (Dart compares the
    /// elements of a driver by identity).
    pivot_context: usize,
    pivot_key: Option<ElementKey>,
    pivot_library_path: Option<String>,
    pivot_library_uri: Option<String>,
    pivot_tag: Tag,
    pivot_name: Option<String>,
    pivot_field_final: bool,
    pivot_class: Option<ElementId>,
}

impl HierarchyHelper {
    fn from_element(ctx: &Ctx<'_>, context: usize, element: ElementId) -> HierarchyHelper {
        let mut pivot = element;
        let mut current = Some(element);
        let mut field_final = false;
        let field = match element.tag() {
            Tag::FieldFormalParameter => match ctx.any(element) {
                dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                _ => None,
            },
            Tag::Field => Some(element),
            _ => None,
        };
        if let Some(f) = field {
            pivot = f;
            field_final = dartr_resolver::element_ext::is_final(ctx, f);
            current = ctx.element_data(f).and_then(|d| d.enclosing);
        }
        if crate::element_locator::is_executable(pivot) {
            current = ctx.element_data(pivot).and_then(|d| d.enclosing);
        }
        let pivot_class = current.filter(|e| e.cast::<dartr_element::InterfaceElement>().is_some());
        let pivot_library_path = support::library_of(ctx, pivot)
            .map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.path.to_string());
        let pivot_library_uri = support::library_of(ctx, pivot)
            .map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.uri.to_string());
        HierarchyHelper {
            pivot_context: context,
            pivot_key: crate::index::element_key(ctx, pivot),
            pivot_library_path,
            pivot_library_uri,
            pivot_tag: pivot.tag(),
            pivot_name: ctx.element_data(pivot).and_then(|d| d.name).map(|n| ctx.name_str(n).to_string()),
            pivot_field_final: field_final,
            pivot_class,
        }
    }

    /// Dart `findMemberElement(clazz)`.
    fn find_member(&self, ctx: &Ctx<'_>, context: usize, class: ElementId) -> Option<ElementId> {
        if self.pivot_class.is_some_and(|c| c.tag() == Tag::ExtensionType) || class.tag() == Tag::ExtensionType {
            return None;
        }
        let name = self.pivot_name.as_deref()?;
        let library_path = self.pivot_library_path.as_deref()?;
        let instance = class.cast::<dartr_element::InstanceElement>()?;
        let data = ctx.instance(instance);
        let named = |list: Vec<ElementId>| -> Option<ElementId> {
            list.into_iter().find(|&e| support::display_name(ctx, e) == name)
        };
        let methods = || named(data.methods.iter().map(|m| m.raw()).collect());
        let getters = || named(data.getters.iter().map(|g| g.raw()).collect());
        let setters = || named(data.setters.iter().map(|s| s.raw()).collect());
        let result = match self.pivot_tag {
            Tag::Method => methods(),
            Tag::Getter => getters(),
            Tag::Setter => setters(),
            Tag::Field => getters().or_else(|| if self.pivot_field_final { None } else { setters() }),
            _ => None,
        };
        let accessible = |e: ElementId| {
            !name.starts_with('_')
                || support::library_of(ctx, e)
                    .map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.path.to_string())
                    .as_deref()
                    == Some(library_path)
        };
        if let Some(r) = result
            && accessible(r)
        {
            return Some(r);
        }
        let interface = class.cast::<dartr_element::InterfaceElement>()?;
        let mixins: Vec<ElementId> = ctx
            .interface(interface)
            .mixins
            .get()
            .map(|l| {
                ctx.list(l)
                    .iter()
                    .filter_map(|&t| match ctx.ty(t) {
                        TypeKind::Interface { element, .. } => Some(element.raw()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        for mixin in mixins.into_iter().rev() {
            let lookup = |kind: Tag| -> Option<ElementId> {
                let manager = dartr_typesystem::inheritance_manager3::InheritanceManager3::new(ctx.global());
                let n = if kind == Tag::Setter { format!("{name}=") } else { name.to_string() };
                // The pivot library in this element model (for a private
                // name).
                let library = self
                    .pivot_library_uri
                    .as_deref()
                    .and_then(|uri| ctx.world.libraries.get(uri).copied());
                let key = dartr_typesystem::inheritance_manager3::Name::new(ctx, library, &n);
                let interface = mixin.cast::<dartr_element::InterfaceElement>()?;
                let found = manager.get_member(interface, key).map(|m| member::base_element(ctx, m))?;
                (found.tag() == kind).then_some(found)
            };
            let result = match self.pivot_tag {
                Tag::Method => lookup(Tag::Method),
                Tag::Getter => lookup(Tag::Getter),
                Tag::Setter => lookup(Tag::Setter),
                Tag::Field => lookup(Tag::Getter).or_else(|| if self.pivot_field_final { None } else { lookup(Tag::Setter) }),
                _ => None,
            };
            if result.is_some()
                && context == self.pivot_context
                && result.and_then(|r| crate::index::element_key(ctx, r)) == self.pivot_key
            {
                return None;
            }
            if result.is_some() {
                return result;
            }
        }
        None
    }
}

/// Dart `_LocalReferencesVisitor`: the references to [elements] (by
/// identity, in one resolved library).
struct LocalReferencesVisitor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    elements: &'c [ElementId],
    results: Vec<(u32, u32)>,
}

impl LocalReferencesVisitor<'_, '_> {
    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        support::element_of(self.ctx, self.tables, node)
    }

    fn contains(&self, e: Option<ElementId>) -> bool {
        e.is_some_and(|e| self.elements.contains(&e))
    }

    fn add_token(&mut self, ast: &Ast, token: dartr_syntax::TokenId) {
        let t = ast.tokens.get(token);
        self.results.push((t.offset, t.end() - t.offset));
    }
}

impl AstVisitor for LocalReferencesVisitor<'_, '_> {
    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        if self.contains(self.element(node)) {
            self.results.push((ast.offset(node), ast.length(node)));
        }
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        if let Some(p) = ast[node].import_prefix {
            ast.accept(p, self);
        }
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        if self.contains(self.element(node)) {
            self.add_token(ast, ast[node].name);
        }
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        if self.contains(self.element(node)) {
            self.results.push((ast.offset(node), ast.length(node)));
        }
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let e = support::corresponding_parameter(self.ctx, ast, self.tables, node.raw());
        if self.contains(e) {
            self.add_token(ast, ast[node].name);
        }
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if self.contains(self.element(node)) {
            self.add_token(ast, ast[node].name);
        }
        if let Some(p) = ast[node].import_prefix {
            ast.accept(p, self);
        }
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if support::in_declaration_context(ast, node) {
            return;
        }
        let e = self
            .element(node)
            .or_else(|| support::write_or_read_element(self.ctx, ast, self.tables, node))
            .or_else(|| support::read_element(self.ctx, self.tables, node));
        if self.contains(e) {
            self.results.push((ast.offset(node), ast.length(node)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values that `dart` prints for the same strings and inserts.
    #[test]
    fn dart_hash_map_order_matches_the_vm() {
        assert_eq!(dart_string_hash(""), 1);
        assert_eq!(dart_string_hash("a"), 170824770);
        assert_eq!(dart_string_hash("hello world"), 1045060183);
        assert_eq!(dart_string_hash("/Users/a/ą€😀"), 667505471);
        let mut keys: Vec<String> = (0..20).map(|i| format!("/p/pkg_{i}")).collect();
        keys.push("/Users/a/ą€😀".to_string());
        let hashes: Vec<u32> = keys.iter().map(|k| dart_string_hash(k)).collect();
        assert_eq!(
            dart_hash_map_order(&hashes),
            vec![19, 17, 8, 0, 11, 9, 14, 7, 4, 1, 18, 3, 10, 12, 5, 13, 2, 16, 15, 6, 20]
        );
    }
}
