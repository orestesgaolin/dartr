// Dart source: pkg/analyzer/lib/src/dart/analysis/search.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/index.dart
// Dart source: pkg/analysis_server/lib/src/services/search/search_engine_internal.dart
// Dart source: pkg/analysis_server/lib/src/services/search/hierarchy.dart
// Dart source: pkg/analysis_server/lib/src/search/element_references.dart
// Dart source: pkg/analysis_server/lib/src/search/type_hierarchy.dart

//! Search engine and handlers for the `search.*` domain and `analysis.implemented`.

use std::collections::HashSet;
use std::sync::Arc;

use dartr_ast::*;
use dartr_cli::DriverSession;
use dartr_cli::driver_provider::ResolvedLibraryResult;
use dartr_element::{
    AnyElement, Ctx, DisplayOptions, EId, ElemRef, ElementId, FId, FormalParameterElement,
    FragmentFlags, FragmentId, InstanceElement, InterfaceElement, LibraryElement, LibraryFragment,
    NoopSink, Tag, TypeId, TypeKind, type_display_string_with,
};
use dartr_project::{AnalysisContextCollection, fs, paths};
use dartr_resolver::error::support;
use dartr_server::index::{ElementKey, RelationKind, UnitIndex, dart_sort, element_key};
use dartr_syntax::LineInfo;
use dartr_typesystem::member;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::convert::{
    compute_element_path, convert_element, find_library_fragment, location_from_starts,
};
use crate::protocol;

/// An element with the resolved library whose element model it is in.
#[derive(Clone)]
pub struct SElem {
    pub lib: Arc<ResolvedLibraryResult>,
    pub unit: usize,
    pub id: ElementId,
}

impl SElem {
    pub fn with<R>(&self, f: impl FnOnce(&Ctx<'_>) -> R) -> R {
        let sink = NoopSink;
        let ctx = self.lib.ctx(self.unit, &sink);
        f(&ctx)
    }

    pub fn key(&self) -> Option<ElementKey> {
        self.with(|ctx| element_key(ctx, self.id))
    }

    pub fn identity(&self) -> Option<(usize, ElementKey)> {
        self.key().map(|k| (self.lib.context, k))
    }

    pub fn same(&self, other: ElementId) -> SElem {
        SElem {
            lib: self.lib.clone(),
            unit: self.unit,
            id: other,
        }
    }
}

/// A resolved unit reference (`ResolvedUnitResult`).
#[derive(Clone)]
pub struct ResolvedUnitRef {
    pub library: Arc<ResolvedLibraryResult>,
    pub index: usize,
}

impl ResolvedUnitRef {
    pub fn unit(&self) -> &dartr_resolver::library_analyzer::ResolvedUnit {
        &self.library.library.units[self.index]
    }

    pub fn line_info(&self) -> &LineInfo {
        &self.library.inputs[self.index].parsed.line_info
    }

    pub fn ctx<'a>(&'a self, sink: &'a NoopSink) -> Ctx<'a> {
        self.library.ctx(self.index, sink)
    }

    pub fn path(&self) -> &str {
        &self.library.inputs[self.index].path
    }
}

/// Dart `OwnedFiles`: the driver (context) of each file that a search looks at.
#[derive(Default)]
pub struct OwnedFiles {
    pub added: IndexMap<String, usize>,
    pub known: IndexMap<String, usize>,
    pub discovered: bool,
}

impl OwnedFiles {
    pub fn add_known(&mut self, path: String, context: usize) -> bool {
        if self.added.contains_key(&path) || self.known.contains_key(&path) {
            return false;
        }
        self.known.insert(path, context);
        true
    }
}

pub type SearchScope = Vec<(usize, Vec<String>)>;

/// A search match with its `SearchResultKind` and whether it is potential.
#[derive(Clone, Debug)]
pub struct LegacyMatch {
    pub path: String,
    pub offset: u32,
    pub length: u32,
    pub context: usize,
    pub kind: protocol::SearchResultKind,
    pub is_potential: bool,
}

/// Unresolved member reference in a unit (`AnalysisDriverUnitIndex.usedNames`).
#[derive(Clone, Debug)]
pub struct UnresolvedNameRelation {
    pub name: String,
    pub kind: protocol::SearchResultKind,
    pub offset: u32,
    pub length: u32,
}

/// Combined index for a unit: resolved element relations and unresolved member names.
#[derive(Debug, Default)]
pub struct LegacyUnitIndex {
    pub unit_index: UnitIndex,
    pub unresolved_names: Vec<UnresolvedNameRelation>,
}

impl LegacyUnitIndex {
    pub fn build(
        ctx: &Ctx<'_>,
        ast: &Ast,
        tables: &dartr_element::ResolutionTables,
        unit: Id<CompilationUnit>,
    ) -> Self {
        let unit_index = dartr_server::index::index_unit(ctx, ast, tables, unit);
        let mut visitor = UnresolvedNameVisitor {
            ctx,
            tables,
            relations: Vec::new(),
        };
        ast.accept(unit, &mut visitor);
        let mut relations = visitor.relations;
        let keys: Vec<Vec<u16>> = relations
            .iter()
            .map(|r| r.name.encode_utf16().collect())
            .collect();
        let mut order: Vec<usize> = (0..relations.len()).collect();
        dart_sort(&mut order, &|a, b| keys[*a].cmp(&keys[*b]) as i64);
        let unresolved_names = order.into_iter().map(|i| relations[i].clone()).collect();
        let _ = &mut relations;
        LegacyUnitIndex {
            unit_index,
            unresolved_names,
        }
    }

    pub fn unresolved_for(&self, name: &str) -> Vec<&UnresolvedNameRelation> {
        self.unresolved_names
            .iter()
            .filter(|r| r.name == name)
            .collect()
    }
}

struct UnresolvedNameVisitor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    relations: Vec<UnresolvedNameRelation>,
}

impl UnresolvedNameVisitor<'_, '_> {
    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.tables
            .element
            .get(node.into())
            .map(|&e| member::base_element(self.ctx, e))
    }
}

impl AstVisitor for UnresolvedNameVisitor<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        if let Some(element) = self.element(node)
            && element.tag() == Tag::Constructor
        {
            let name = ast[node].name;
            let identifier_is_constructor = ast.cast::<PrefixedIdentifier>(name).is_some_and(|p| {
                self.element(ast[p].identifier)
                    .is_some_and(|e| e.tag() == Tag::Constructor)
            });
            if identifier_is_constructor {
                let p = ast.cast::<PrefixedIdentifier>(name).unwrap();
                ast.accept(ast[p].prefix, self);
            } else {
                ast.accept(name, self);
            }
            if let Some(t) = ast[node].type_arguments {
                ast.accept(t, self);
            }
            if let Some(a) = ast[node].arguments {
                ast.accept(a, self);
            }
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_comment_reference(&mut self, ast: &Ast, node: Id<CommentReference>) {
        let expression = ast[node].expression;
        let element = self.element(expression).or_else(|| {
            ast.cast::<PrefixedIdentifier>(expression)
                .and_then(|p| self.element(ast[p].identifier))
        });
        if ast.is::<Identifier>(expression)
            && let Some(element) = element
            && element.tag() == Tag::Constructor
        {
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_field_initializer(
        &mut self,
        ast: &Ast,
        node: Id<ConstructorFieldInitializer>,
    ) {
        ast.accept(ast[node].expression, self);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        ast.accept(ast[node].type_, self);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        _ast: &Ast,
        _node: Id<DotShorthandPropertyAccess>,
    ) {
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_label_reference(&mut self, _ast: &Ast, _node: Id<LabelReference>) {}

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let name = ast[node].method_name;
        if self.element(name).is_none() {
            let token = ast[name].token;
            let lexeme = ast.tokens.lexeme(token).to_string();
            let offset = ast.offset(name);
            let length = lexeme.encode_utf16().count() as u32;
            self.relations.push(UnresolvedNameRelation {
                name: lexeme,
                kind: protocol::SearchResultKind::INVOCATION,
                offset,
                length,
            });
        }
        if let Some(t) = ast[node].target {
            ast.accept(t, self);
        }
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if support::in_declaration_context(ast, node) {
            return;
        }
        let element = support::write_or_read_element(self.ctx, ast, self.tables, node)
            .or_else(|| support::read_element(self.ctx, self.tables, node));
        if element.is_none() {
            let in_getter = dartr_resolver::ast_ext::simple_identifier_in_getter_context(ast, node);
            let in_setter = dartr_resolver::ast_ext::simple_identifier_in_setter_context(ast, node);
            let kind = if in_getter && in_setter {
                protocol::SearchResultKind::ReadWrite
            } else if in_getter {
                protocol::SearchResultKind::READ
            } else {
                protocol::SearchResultKind::WRITE
            };
            let token = ast[node].token;
            let lexeme = ast.tokens.lexeme(token).to_string();
            let offset = ast.offset(node);
            let length = lexeme.encode_utf16().count() as u32;
            self.relations.push(UnresolvedNameRelation {
                name: lexeme,
                kind,
                offset,
                length,
            });
        }
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        ast.accept(ast[node].argument_list, self);
    }
}

pub const REFERENCES: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsReadWrittenBy,
        protocol::SearchResultKind::ReadWrite,
    ),
    (
        RelationKind::IsInvokedBy,
        protocol::SearchResultKind::INVOCATION,
    ),
    (RelationKind::IsReadBy, protocol::SearchResultKind::READ),
    (RelationKind::IsWrittenBy, protocol::SearchResultKind::WRITE),
    (
        RelationKind::IsReferencedByNamedArgument,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
];

pub const CONSTRUCTOR: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsInvokedBy,
        protocol::SearchResultKind::INVOCATION,
    ),
    (
        RelationKind::IsInvokedByDotShorthandsConstructor,
        protocol::SearchResultKind::INVOCATION,
    ),
    (
        RelationKind::IsInvokedByEnumConstantWithoutArguments,
        protocol::SearchResultKind::INVOCATION,
    ),
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedByConstructorTearOff,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedByDotShorthandConstructorTearOff,
        protocol::SearchResultKind::REFERENCE,
    ),
];

pub const GETTER: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedByPatternField,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsInvokedBy,
        protocol::SearchResultKind::INVOCATION,
    ),
];

pub const GETTER_OF_FIELD: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::READ,
    ),
    (
        RelationKind::IsReferencedByPatternField,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsInvokedBy,
        protocol::SearchResultKind::INVOCATION,
    ),
];

pub const FIELD: &[(RelationKind, protocol::SearchResultKind)] = &[
    (RelationKind::IsWrittenBy, protocol::SearchResultKind::WRITE),
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedByPatternField,
        protocol::SearchResultKind::REFERENCE,
    ),
];

pub const SETTER_OF_FIELD: &[(RelationKind, protocol::SearchResultKind)] = &[(
    RelationKind::IsReferencedBy,
    protocol::SearchResultKind::WRITE,
)];

pub const FUNCTION: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsReferencedByPatternField,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsReferencedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsInvokedBy,
        protocol::SearchResultKind::INVOCATION,
    ),
];

pub const SUBTYPES: &[(RelationKind, protocol::SearchResultKind)] = &[
    (
        RelationKind::IsExtendedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsMixedInBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::IsImplementedBy,
        protocol::SearchResultKind::REFERENCE,
    ),
    (
        RelationKind::Constrains,
        protocol::SearchResultKind::REFERENCE,
    ),
];

pub fn dart_string_hash(s: &str) -> u32 {
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

pub fn dart_hash_map_order(hashes: &[u32]) -> Vec<usize> {
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

pub fn words(content: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for w in content.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')) {
        if w.is_empty() {
            continue;
        }
        if w.contains('$') {
            for part in w.split('$').filter(|p| !p.is_empty()) {
                out.insert(part.to_string());
            }
        }
        out.insert(w.to_string());
    }
    out
}

pub fn dart_files_recursively(folder: &str, out: &mut Vec<String>) {
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

pub fn library_file_paths(element: &SElem, e: ElementId) -> Vec<String> {
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
                if let dartr_element::DirectiveUri::Unit {
                    library_fragment, ..
                } = &part.directive.uri
                {
                    stack.push(*library_fragment);
                }
            }
        }
        out
    })
}

/// Children of a `FragmentId` in the order of Dart `Fragment.children`.
pub fn fragment_children(ctx: &Ctx<'_>, fragment: FragmentId) -> Vec<FragmentId> {
    match fragment.tag() {
        Tag::Library => {
            let Some(lib_frag) = fragment.cast::<LibraryFragment>() else {
                return Vec::new();
            };
            let d = ctx.fragment(lib_frag);
            let mut out = Vec::new();
            out.extend(d.classes.iter().map(|f| f.raw()));
            out.extend(d.enums.iter().map(|f| f.raw()));
            out.extend(d.extensions.iter().map(|f| f.raw()));
            out.extend(d.extension_types.iter().map(|f| f.raw()));
            out.extend(d.functions.iter().map(|f| f.raw()));
            out.extend(d.getters.iter().map(|f| f.raw()));
            out.extend(d.mixins.iter().map(|f| f.raw()));
            out.extend(d.setters.iter().map(|f| f.raw()));
            out.extend(d.type_aliases.iter().map(|f| f.raw()));
            out.extend(d.variables.iter().map(|f| f.raw()));
            out
        }
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType => {
            let mut out = Vec::new();
            if let Some(inst) = fragment.cast::<dartr_element::InstanceFragment>() {
                let d = ctx.store(inst.store()).instance_fragment(inst);
                out.extend(d.type_params.iter().map(|f| f.raw()));
                out.extend(d.fields.iter().map(|f| f.raw()));
                out.extend(d.getters.iter().map(|f| f.raw()));
                out.extend(d.setters.iter().map(|f| f.raw()));
                let constructors = match fragment.tag() {
                    Tag::Class => fragment
                        .cast::<dartr_element::ClassFragment>()
                        .map(|f| &ctx.fragment(f).constructors),
                    Tag::Enum => fragment
                        .cast::<dartr_element::EnumFragment>()
                        .map(|f| &ctx.fragment(f).constructors),
                    Tag::Mixin => fragment
                        .cast::<dartr_element::MixinFragment>()
                        .map(|f| &ctx.fragment(f).constructors),
                    Tag::ExtensionType => fragment
                        .cast::<dartr_element::ExtensionTypeFragment>()
                        .map(|f| &ctx.fragment(f).constructors),
                    _ => None,
                };
                if let Some(constructors) = constructors {
                    out.extend(constructors.iter().map(|f| f.raw()));
                }
                out.extend(d.methods.iter().map(|f| f.raw()));
            }
            out
        }
        Tag::Extension => {
            let mut out = Vec::new();
            if let Some(inst) = fragment.cast::<dartr_element::InstanceFragment>() {
                let d = ctx.store(inst.store()).instance_fragment(inst);
                out.extend(d.type_params.iter().map(|f| f.raw()));
                out.extend(d.fields.iter().map(|f| f.raw()));
                out.extend(d.getters.iter().map(|f| f.raw()));
                out.extend(d.setters.iter().map(|f| f.raw()));
                out.extend(d.methods.iter().map(|f| f.raw()));
            }
            out
        }
        Tag::Constructor
        | Tag::Method
        | Tag::TopLevelFunction
        | Tag::LocalFunction
        | Tag::Getter
        | Tag::Setter => {
            let mut out = Vec::new();
            if let Some(exec) = fragment.cast::<dartr_element::ExecutableFragment>() {
                let d = ctx.store(exec.store()).executable_fragment(exec);
                out.extend(d.type_params.iter().map(|f| f.raw()));
                out.extend(d.formal_params.iter().map(|f| f.raw()));
            }
            out
        }
        Tag::TypeAlias => {
            let Some(ta) = fragment.cast::<dartr_element::TypeAliasFragment>() else {
                return Vec::new();
            };
            ctx.fragment(ta)
                .type_params
                .iter()
                .map(|f| f.raw())
                .collect()
        }
        Tag::GenericFunctionType => {
            let Some(gft) = fragment.cast::<dartr_element::GenericFunctionTypeFragment>() else {
                return Vec::new();
            };
            let d = ctx.fragment(gft);
            let mut out: Vec<FragmentId> = d.type_params.iter().map(|f| f.raw()).collect();
            out.extend(d.formal_params.iter().map(|f| f.raw()));
            out
        }
        _ => Vec::new(),
    }
}

/// Dart `_getEnclosingFragment(libraryFragment, offset)`.
pub fn get_enclosing_fragment(
    ctx: &Ctx<'_>,
    library_fragment: FId<LibraryFragment>,
    offset: u32,
) -> FragmentId {
    fn visit(ctx: &Ctx<'_>, fragment: FragmentId, offset: u32) -> Option<FragmentId> {
        if fragment.tag() != Tag::Library {
            let data = ctx.fragment_data(fragment)?;
            let code_offset = data.code_offset?;
            let code_length = data.code_length?;
            let code_end = code_offset + code_length;
            if !(code_offset <= offset && offset <= code_end) {
                return None;
            }
        }
        for child in fragment_children(ctx, fragment) {
            if let Some(res) = visit(ctx, child, offset) {
                return Some(res);
            }
        }
        Some(fragment)
    }
    visit(ctx, library_fragment.raw(), offset).unwrap_or(library_fragment.raw())
}

/// Dart `findFragmentByNameOffset(libraryFragment, offset)`.
pub fn find_fragment_by_name_offset(
    ctx: &Ctx<'_>,
    library_fragment: FId<LibraryFragment>,
    offset: u32,
) -> Option<FragmentId> {
    fn visit(ctx: &Ctx<'_>, fragment: FragmentId, offset: u32) -> Option<FragmentId> {
        let data = ctx.fragment_data(fragment)?;
        if data.name_offset == Some(offset) {
            return Some(fragment);
        }
        for child in fragment_children(ctx, fragment) {
            if let Some(res) = visit(ctx, child, offset) {
                return Some(res);
            }
        }
        None
    }
    visit(ctx, library_fragment.raw(), offset)
}

/// Dart `_LocalReferencesVisitor`: references to local elements with `SearchResultKind`.
pub struct LocalReferencesVisitor<'c, 'a> {
    pub ctx: &'c Ctx<'a>,
    pub tables: &'c dartr_element::ResolutionTables,
    pub elements: &'c [ElementId],
    pub results: Vec<(u32, u32, protocol::SearchResultKind)>,
}

impl LocalReferencesVisitor<'_, '_> {
    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        support::element_of(self.ctx, self.tables, node)
    }

    fn contains(&self, e: Option<ElementId>) -> bool {
        e.is_some_and(|e| self.elements.contains(&e))
    }

    fn add_token(
        &mut self,
        ast: &Ast,
        token: dartr_syntax::TokenId,
        kind: protocol::SearchResultKind,
    ) {
        let t = ast.tokens.get(token);
        self.results.push((t.offset, t.end() - t.offset, kind));
    }
}

impl AstVisitor for LocalReferencesVisitor<'_, '_> {
    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        if self.contains(self.element(node)) {
            self.results.push((
                ast.offset(node),
                ast.length(node),
                protocol::SearchResultKind::WRITE,
            ));
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
            self.add_token(ast, ast[node].name, protocol::SearchResultKind::REFERENCE);
        }
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        if self.contains(self.element(node)) {
            self.results.push((
                ast.offset(node),
                ast.length(node),
                protocol::SearchResultKind::REFERENCE,
            ));
        }
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let e = support::corresponding_parameter(self.ctx, ast, self.tables, node.raw());
        if self.contains(e) {
            self.add_token(ast, ast[node].name, protocol::SearchResultKind::REFERENCE);
        }
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if self.contains(self.element(node)) {
            self.add_token(ast, ast[node].name, protocol::SearchResultKind::REFERENCE);
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
            let e = e.unwrap();
            let parent = ast.parent(node);
            let mut kind = protocol::SearchResultKind::REFERENCE;
            if e.tag() == Tag::LocalFunction {
                if parent
                    .and_then(|p| ast.cast::<MethodInvocation>(p))
                    .is_some_and(|m| ast[m].method_name == node)
                {
                    kind = protocol::SearchResultKind::INVOCATION;
                }
            } else if matches!(
                e.tag(),
                Tag::LocalVariable
                    | Tag::FormalParameter
                    | Tag::FieldFormalParameter
                    | Tag::SuperFormalParameter
                    | Tag::PatternVariable
                    | Tag::BindPatternVariable
                    | Tag::JoinPatternVariable
            ) {
                let is_get =
                    dartr_resolver::ast_ext::simple_identifier_in_getter_context(ast, node);
                let is_set =
                    dartr_resolver::ast_ext::simple_identifier_in_setter_context(ast, node);
                if is_get && is_set {
                    kind = protocol::SearchResultKind::ReadWrite;
                } else if is_get {
                    if parent
                        .and_then(|p| ast.cast::<MethodInvocation>(p))
                        .is_some_and(|m| ast[m].method_name == node)
                    {
                        kind = protocol::SearchResultKind::INVOCATION;
                    } else {
                        kind = protocol::SearchResultKind::READ;
                    }
                } else if is_set {
                    kind = protocol::SearchResultKind::WRITE;
                }
            }
            self.results
                .push((ast.offset(node), ast.length(node), kind));
        }
    }
}

pub fn class_members(ctx: &Ctx<'_>, class: ElementId, name: Option<&str>) -> Vec<ElementId> {
    let Some(instance) = class.cast::<InstanceElement>() else {
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

pub fn children_named(ctx: &Ctx<'_>, parent: ElementId, name: &str) -> Vec<ElementId> {
    let Some(instance) = parent.cast::<InstanceElement>() else {
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
    if let Some(interface) = parent.cast::<InterfaceElement>() {
        children.extend(
            ctx.interface(interface)
                .constructors
                .iter()
                .map(|c| c.raw()),
        );
    }
    children
        .into_iter()
        .filter(|&c| member::lookup_name(ctx, ElemRef::Base(c)).as_deref() == Some(name))
        .collect()
}

/// Dart `TypeHierarchyComputerHelper`.
pub struct HierarchyHelper {
    pub pivot_context: usize,
    pub pivot_key: Option<ElementKey>,
    pub pivot_library_path: Option<String>,
    pub pivot_library_uri: Option<String>,
    pub pivot_tag: Tag,
    pub pivot_name: Option<String>,
    pub pivot_field_final: bool,
    pub pivot_class: Option<ElementId>,
}

impl HierarchyHelper {
    pub fn from_element(ctx: &Ctx<'_>, context: usize, element: ElementId) -> HierarchyHelper {
        let mut pivot = element;
        let mut current = Some(element);
        let mut field_final = false;
        let field = match element.tag() {
            Tag::FieldFormalParameter => match ctx.any(element) {
                AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
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
        if dartr_server::element_locator::is_executable(pivot) {
            current = ctx.element_data(pivot).and_then(|d| d.enclosing);
        }
        let pivot_class = current.filter(|e| e.cast::<InterfaceElement>().is_some());
        let pivot_library_path = support::library_of(ctx, pivot).map(|l| {
            ctx.fragment(ctx.get(l).first_fragment())
                .source
                .path
                .to_string()
        });
        let pivot_library_uri = support::library_of(ctx, pivot).map(|l| {
            ctx.fragment(ctx.get(l).first_fragment())
                .source
                .uri
                .to_string()
        });
        HierarchyHelper {
            pivot_context: context,
            pivot_key: element_key(ctx, pivot),
            pivot_library_path,
            pivot_library_uri,
            pivot_tag: pivot.tag(),
            pivot_name: ctx
                .element_data(pivot)
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n).to_string()),
            pivot_field_final: field_final,
            pivot_class,
        }
    }

    pub fn find_member(
        &self,
        ctx: &Ctx<'_>,
        context: usize,
        class: ElementId,
    ) -> Option<ElementId> {
        if self
            .pivot_class
            .is_some_and(|c| c.tag() == Tag::ExtensionType)
            || class.tag() == Tag::ExtensionType
        {
            return None;
        }
        let name = self.pivot_name.as_deref()?;
        let library_path = self.pivot_library_path.as_deref()?;
        let instance = class.cast::<InstanceElement>()?;
        let data = ctx.instance(instance);
        let named = |list: Vec<ElementId>| -> Option<ElementId> {
            list.into_iter()
                .find(|&e| support::display_name(ctx, e) == name)
        };
        let methods = || named(data.methods.iter().map(|m| m.raw()).collect());
        let getters = || named(data.getters.iter().map(|g| g.raw()).collect());
        let setters = || named(data.setters.iter().map(|s| s.raw()).collect());
        let result = match self.pivot_tag {
            Tag::Method => methods(),
            Tag::Getter => getters(),
            Tag::Setter => setters(),
            Tag::Field => getters().or_else(|| {
                if self.pivot_field_final {
                    None
                } else {
                    setters()
                }
            }),
            _ => None,
        };
        let accessible = |e: ElementId| {
            !name.starts_with('_')
                || support::library_of(ctx, e)
                    .map(|l| {
                        ctx.fragment(ctx.get(l).first_fragment())
                            .source
                            .path
                            .to_string()
                    })
                    .as_deref()
                    == Some(library_path)
        };
        if let Some(r) = result
            && accessible(r)
        {
            return Some(r);
        }
        let interface = class.cast::<InterfaceElement>()?;
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
                let manager =
                    dartr_typesystem::inheritance_manager3::InheritanceManager3::new(ctx.global());
                let n = if kind == Tag::Setter {
                    format!("{name}=")
                } else {
                    name.to_string()
                };
                let library = self
                    .pivot_library_uri
                    .as_deref()
                    .and_then(|uri| ctx.world.libraries.get(uri).copied());
                let key = dartr_typesystem::inheritance_manager3::Name::new(ctx, library, &n);
                let interface = mixin.cast::<InterfaceElement>()?;
                let found = manager
                    .get_member(interface, key)
                    .map(|m| member::base_element(ctx, m))?;
                (found.tag() == kind).then_some(found)
            };
            let result = match self.pivot_tag {
                Tag::Method => lookup(Tag::Method),
                Tag::Getter => lookup(Tag::Getter),
                Tag::Setter => lookup(Tag::Setter),
                Tag::Field => lookup(Tag::Getter).or_else(|| {
                    if self.pivot_field_final {
                        None
                    } else {
                        lookup(Tag::Setter)
                    }
                }),
                _ => None,
            };
            if result.is_some()
                && context == self.pivot_context
                && result.and_then(|r| element_key(ctx, r)) == self.pivot_key
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

/// Computes the super hierarchy items for `TypeHierarchyComputer`.
pub struct SuperHierarchyBuilder<'a> {
    pub helper: &'a HierarchyHelper,
    pub items: Vec<protocol::TypeHierarchyItem>,
    pub item_elements: Vec<SElem>,
    pub keys: Vec<(usize, ElementKey)>,
}

impl<'a> SuperHierarchyBuilder<'a> {
    pub fn new(helper: &'a HierarchyHelper) -> Self {
        Self {
            helper,
            items: Vec::new(),
            item_elements: Vec::new(),
            keys: Vec::new(),
        }
    }

    pub fn create_super_item(&mut self, class: &SElem, type_args: &[TypeId]) -> usize {
        if let Some(key) = class.identity()
            && let Some(idx) = self.keys.iter().position(|k| *k == key)
        {
            return idx;
        }
        let (class_element, display_name, member_element, supertype, mixins, interfaces) = class
            .with(|ctx| {
                let class_el = convert_element(ctx, class.id, None);
                let display_name = if !type_args.is_empty() {
                    let args_str = type_args
                        .iter()
                        .map(|&t| type_display_string_with(ctx, t, DisplayOptions::default()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Some(format!(
                        "{}<{args_str}>",
                        support::display_name(ctx, class.id)
                    ))
                } else {
                    None
                };
                let member_el = self
                    .helper
                    .find_member(ctx, class.lib.context, class.id)
                    .map(|m| dartr_element::diagnostics::non_synthetic(ctx, m))
                    .map(|m| convert_element(ctx, m, None));

                let mut supertype = None;
                let mut mixins = Vec::new();
                let mut interfaces = Vec::new();
                if let Some(intf) = class.id.cast::<InterfaceElement>() {
                    let d = ctx.interface(intf);
                    if let Some(st) = d.supertype.get()
                        && let TypeKind::Interface { element, args, .. } = ctx.ty(st)
                    {
                        supertype = Some((element.raw(), ctx.list(*args).to_vec()));
                    }
                    if let Some(m_list) = d.mixins.get() {
                        for &t in ctx.list(m_list) {
                            if let TypeKind::Interface { element, args, .. } = ctx.ty(t) {
                                mixins.push((element.raw(), ctx.list(*args).to_vec()));
                            }
                        }
                    }
                    if let Some(i_list) = d.interfaces.get() {
                        for &t in ctx.list(i_list) {
                            if let TypeKind::Interface { element, args, .. } = ctx.ty(t) {
                                interfaces.push((element.raw(), ctx.list(*args).to_vec()));
                            }
                        }
                    }
                }
                (
                    class_el,
                    display_name,
                    member_el,
                    supertype,
                    mixins,
                    interfaces,
                )
            });

        let item_id = self.items.len();
        if let Some(key) = class.identity() {
            self.keys.push(key);
        } else {
            self.keys.push((
                usize::MAX,
                ElementKey {
                    library_path: String::new(),
                    unit_path: String::new(),
                    unit_member: Some(format!("unknown-{item_id}")),
                    class_member: None,
                    parameter: None,
                    kind: dartr_server::index::SyntheticKind::NotSynthetic,
                },
            ));
        }
        self.item_elements.push(class.clone());
        self.items.push(protocol::TypeHierarchyItem {
            class_element,
            display_name,
            member_element,
            superclass: None,
            interfaces: Vec::new(),
            mixins: Vec::new(),
            subclasses: Vec::new(),
        });

        if let Some((st_el, st_args)) = supertype {
            let st_id = self.create_super_item(&class.same(st_el), &st_args);
            self.items[item_id].superclass = Some(st_id as i64);
        }
        for (m_el, m_args) in mixins {
            let m_id = self.create_super_item(&class.same(m_el), &m_args);
            self.items[item_id].mixins.push(m_id as i64);
        }
        for (i_el, i_args) in interfaces {
            let i_id = self.create_super_item(&class.same(i_el), &i_args);
            self.items[item_id].interfaces.push(i_id as i64);
        }
        item_id
    }
}

/// Converts a declaration `SElem` into a `protocol::SearchResult`.
pub fn search_result_for_element(elem: &SElem) -> Option<protocol::SearchResult> {
    elem.with(|ctx| {
        let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, elem.id);
        let first = ctx.element_data(non_synthetic)?.first_fragment;
        let lib_frag = find_library_fragment(ctx, first)?;
        let lib_frag_data = ctx.fragment(lib_frag);
        let data = ctx.fragment_data(first)?;
        let name_offset = data.name_offset?;
        let name_len = data.name.map(|n| ctx.name_str(n).encode_utf16().count())?;
        let location = location_from_starts(
            &lib_frag_data.source.path,
            &lib_frag_data.line_starts,
            name_offset,
            name_len as u32,
        );
        let path = compute_element_path(ctx, elem.id);
        Some(protocol::SearchResult {
            location,
            kind: protocol::SearchResultKind::DECLARATION,
            is_potential: false,
            path,
        })
    })
}

/// Checks if a parsed unit has any top-level declaration whose name matches `regex`.
pub fn parsed_has_matching_top_level(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    regex: &regress::Regex,
) -> bool {
    for &decl in ast.list_raw(ast[unit].declarations) {
        if let Some(c) = ast.cast::<ClassDeclaration>(decl) {
            let name = ast
                .tokens
                .lexeme(support::class_name_token(ast, ast[c].name_part));
            if regex.find(name).is_some() {
                return true;
            }
        } else if let Some(c) = ast.cast::<ClassTypeAlias>(decl) {
            if regex.find(ast.tokens.lexeme(ast[c].name)).is_some() {
                return true;
            }
        } else if let Some(e) = ast.cast::<EnumDeclaration>(decl) {
            let name = ast
                .tokens
                .lexeme(support::class_name_token(ast, ast[e].name_part));
            if regex.find(name).is_some() {
                return true;
            }
        } else if let Some(e) = ast.cast::<ExtensionDeclaration>(decl) {
            let name = ast[e].name.map(|t| ast.tokens.lexeme(t)).unwrap_or("");
            if regex.find(name).is_some() {
                return true;
            }
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(decl) {
            let name = ast
                .tokens
                .lexeme(support::class_name_token(ast, ast[e].name_part));
            if regex.find(name).is_some() {
                return true;
            }
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(decl) {
            if regex.find(ast.tokens.lexeme(ast[f].name)).is_some() {
                return true;
            }
        } else if let Some(f) = ast.cast::<FunctionTypeAlias>(decl) {
            if regex.find(ast.tokens.lexeme(ast[f].name)).is_some() {
                return true;
            }
        } else if let Some(g) = ast.cast::<GenericTypeAlias>(decl) {
            if regex.find(ast.tokens.lexeme(ast[g].name)).is_some() {
                return true;
            }
        } else if let Some(m) = ast.cast::<MixinDeclaration>(decl) {
            if regex.find(ast.tokens.lexeme(ast[m].name)).is_some() {
                return true;
            }
        } else if let Some(v) = ast.cast::<TopLevelVariableDeclaration>(decl) {
            let list = ast[v].variables;
            for &var in ast.list(ast[list].variables) {
                if regex.find(ast.tokens.lexeme(ast[var].name)).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

/// Checks if a parsed unit defines any class/enum/extensionType/mixin member with `target_name`.
pub fn parsed_defines_class_member(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    target_name: &str,
) -> bool {
    let check_members = |members: NodeList<ClassMember>| -> bool {
        for &m in ast.list_raw(members) {
            if let Some(method) = ast.cast::<MethodDeclaration>(m) {
                if ast.tokens.lexeme(ast[method].name) == target_name {
                    return true;
                }
            } else if let Some(field_decl) = ast.cast::<FieldDeclaration>(m) {
                let list = ast[field_decl].fields;
                for &v in ast.list(ast[list].variables) {
                    if ast.tokens.lexeme(ast[v].name) == target_name {
                        return true;
                    }
                }
            }
        }
        false
    };
    let check_name_part = |name_part: Id<ClassNamePart>| -> bool {
        if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(name_part) {
            let params = ast[ast[p].formal_parameters].parameters;
            for &param in ast.list(params) {
                if let Some(s) = ast.cast::<RegularFormalParameter>(param)
                    && (ast[s].covariant_keyword.is_some()
                        || ast[s].const_final_or_var_keyword.is_some()
                        || ast[s].type_.is_some())
                    && let Some(name_tok) = ast[s].name
                    && ast.tokens.lexeme(name_tok) == target_name
                {
                    return true;
                }
                if let Some(f) = ast.cast::<FieldFormalParameter>(param)
                    && ast.tokens.lexeme(ast[f].name) == target_name
                {
                    return true;
                }
            }
        }
        false
    };
    let check_body = |body: Id<ClassBody>| -> bool {
        ast.cast::<BlockClassBody>(body)
            .is_some_and(|b| check_members(ast[b].members))
    };
    for &decl in ast.list_raw(ast[unit].declarations) {
        if let Some(c) = ast.cast::<ClassDeclaration>(decl) {
            if check_name_part(ast[c].name_part) || check_body(ast[c].body) {
                return true;
            }
        } else if let Some(e) = ast.cast::<EnumDeclaration>(decl) {
            if check_name_part(ast[e].name_part) {
                return true;
            }
            if let Some(body) = ast.cast::<BlockEnumBody>(ast[e].body) {
                for &constant in ast.list(ast[body].constants) {
                    if ast.tokens.lexeme(ast[constant].name) == target_name {
                        return true;
                    }
                }
                if check_members(ast[body].members) {
                    return true;
                }
            }
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(decl) {
            if check_name_part(ast[e].name_part) || check_body(ast[e].body) {
                return true;
            }
        } else if let Some(m) = ast.cast::<MixinDeclaration>(decl) {
            if check_body(ast[m].body) {
                return true;
            }
        }
    }
    false
}

/// Collects matching class members from a `LibraryElement` (`Search.classMembers`).
pub fn collect_library_class_members(lib: &Arc<ResolvedLibraryResult>, name: &str) -> Vec<SElem> {
    let sink = NoopSink;
    let ctx = lib.ctx(0, &sink);
    let lib_el: EId<LibraryElement> = lib.library.library;
    let data = ctx.get(lib_el);
    let mut out = Vec::new();
    let mut check_interface = |intf_id: ElementId| {
        let Some(intf) = intf_id.cast::<InterfaceElement>() else {
            return;
        };
        let d = ctx.interface(intf);
        for &g in &d.getters {
            let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, g.raw());
            if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
                && support::display_name(&ctx, g.raw()) == name
            {
                out.push(SElem {
                    lib: lib.clone(),
                    unit: 0,
                    id: g.raw(),
                });
            }
        }
        for &s in &d.setters {
            let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, s.raw());
            if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
                && support::display_name(&ctx, s.raw()) == name
            {
                out.push(SElem {
                    lib: lib.clone(),
                    unit: 0,
                    id: s.raw(),
                });
            }
        }
        for &f in &d.fields {
            let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, f.raw());
            if (flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
                || flags
                    .contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER))
                && support::display_name(&ctx, f.raw()) == name
            {
                out.push(SElem {
                    lib: lib.clone(),
                    unit: 0,
                    id: f.raw(),
                });
            }
        }
        for &m in &d.methods {
            if support::display_name(&ctx, m.raw()) == name {
                out.push(SElem {
                    lib: lib.clone(),
                    unit: 0,
                    id: m.raw(),
                });
            }
        }
    };
    for &c in &data.classes {
        check_interface(c.raw());
    }
    for &e in &data.enums {
        check_interface(e.raw());
    }
    for &et in &data.extension_types {
        check_interface(et.raw());
    }
    for &m in &data.mixins {
        check_interface(m.raw());
    }
    out
}

/// Collects matching top-level elements from a `LibraryElement` (`Search.topLevelElements`).
pub fn collect_library_top_level_elements(
    lib: &Arc<ResolvedLibraryResult>,
    regex: &regress::Regex,
) -> Vec<SElem> {
    let sink = NoopSink;
    let ctx = lib.ctx(0, &sink);
    let lib_el: EId<LibraryElement> = lib.library.library;
    let data = ctx.get(lib_el);
    let mut out = Vec::new();
    let mut add = |id: ElementId| {
        let name = support::display_name(&ctx, id);
        if regex.find(&name).is_some() {
            out.push(SElem {
                lib: lib.clone(),
                unit: 0,
                id,
            });
        }
    };
    for &g in &data.getters {
        let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, g.raw());
        if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
            add(g.raw());
        }
    }
    for &c in &data.classes {
        add(c.raw());
    }
    for &e in &data.enums {
        add(e.raw());
    }
    for &ext in &data.extensions {
        add(ext.raw());
    }
    for &et in &data.extension_types {
        add(et.raw());
    }
    for &f in &data.top_level_functions {
        add(f.raw());
    }
    for &m in &data.mixins {
        add(m.raw());
    }
    for &s in &data.setters {
        let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, s.raw());
        if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
            add(s.raw());
        }
    }
    for &v in &data.top_level_variables {
        let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, v.raw());
        if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION) {
            add(v.raw());
        }
    }
    for &ta in &data.type_aliases {
        add(ta.raw());
    }
    out
}

/// Checks if `element` is a member of an `InterfaceElement` (excluding constructors)
/// for `ElementReferencesComputer._isMemberElement`.
pub fn is_member_element(element: &SElem) -> bool {
    if element.id.tag() == Tag::Constructor {
        return false;
    }
    element.with(|ctx| {
        ctx.element_data(element.id)
            .and_then(|d| d.enclosing)
            .is_some_and(|enc| enc.cast::<InterfaceElement>().is_some())
    })
}

/// Checks if `element` is a named formal parameter.
pub fn is_named_parameter(element: &SElem) -> bool {
    matches!(
        element.id.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) && element.with(|ctx| {
        ctx.get(EId::<FormalParameterElement>::from_raw(element.id))
            .kind
            .is_named()
    })
}

/// Search engine over the analyzed contexts of a legacy server.
pub struct SearchEngine<'a> {
    pub collection: &'a AnalysisContextCollection,
    pub excluded: &'a [String],
    pub session: &'a mut DriverSession,
    pub owned: &'a mut OwnedFiles,
    pub search_scope: &'a mut Option<Arc<SearchScope>>,
    pub search_words: &'a mut FxHashMap<String, Arc<HashSet<String>>>,
    pub indexes: &'a mut FxHashMap<(usize, String), Arc<LegacyUnitIndex>>,
}

impl<'a> SearchEngine<'a> {
    fn is_excluded(&self, file: &str) -> bool {
        self.excluded
            .iter()
            .any(|root| paths::is_or_within(root, file))
    }

    pub fn resolved_unit_in(
        &mut self,
        path: &str,
        context: Option<usize>,
    ) -> Option<ResolvedUnitRef> {
        if !path.ends_with(".dart") {
            return None;
        }
        fs::read_string(path)?;
        let library = match context {
            Some(ctx_idx) => self
                .session
                .resolved_library_in(self.collection, ctx_idx, path),
            None => self.session.resolved_library(self.collection, path),
        }?;
        let index = library.unit_index(path)?;
        Some(ResolvedUnitRef { library, index })
    }

    pub fn search_scope(&mut self) -> Arc<SearchScope> {
        if let Some(scope) = self.search_scope.as_ref() {
            return scope.clone();
        }
        let count = self.collection.contexts.len();
        if self.owned.added.is_empty() {
            for (index, context) in self.collection.contexts.iter().enumerate() {
                let files: Vec<String> = context
                    .root
                    .analyzed_files()
                    .into_iter()
                    .filter(|f| f.ends_with(".dart") && !self.is_excluded(f))
                    .collect();
                for f in &files {
                    if let Some(library) = self.session.part_of_uri_library(index, f)
                        && files.contains(&library)
                    {
                        self.owned.added.entry(library).or_insert(index);
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
        let hashes: Vec<u32> = self
            .collection
            .contexts
            .iter()
            .map(|c| dart_string_hash(&c.root.root))
            .collect();
        let order = dart_hash_map_order(&hashes);
        if !self.owned.discovered {
            self.owned.discovered = true;
            for &index in &order {
                let context = &self.collection.contexts[index];
                let sdk = context.sdk.as_ref().or(self.collection.sdk.as_ref());
                let mut discovered = Vec::new();
                if let Some(sdk) = sdk {
                    discovered.extend(
                        sdk.libraries()
                            .iter()
                            .filter_map(|l| sdk.map_dart_uri(&l.short_name)),
                    );
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
        let scope = Arc::new(scope);
        *self.search_scope = Some(scope.clone());
        scope
    }

    pub fn owner_of(&mut self, path: &str) -> Option<usize> {
        self.search_scope()
            .iter()
            .find(|(_, files)| files.iter().any(|f| f == path))
            .map(|(index, _)| *index)
    }

    pub fn mentions_any(&mut self, path: &str, names: &[String]) -> bool {
        let w = match self.search_words.get(path) {
            Some(w) => w.clone(),
            None => {
                let w = Arc::new(words(&fs::read_string(path).unwrap_or_default()));
                self.search_words.insert(path.to_string(), w.clone());
                w
            }
        };
        names.iter().any(|n| w.contains(n))
    }

    pub fn unit_index(&mut self, path: &str, context: usize) -> Option<Arc<LegacyUnitIndex>> {
        let key = (context, path.to_string());
        if let Some(i) = self.indexes.get(&key) {
            return Some(i.clone());
        }
        let resolved = self.resolved_unit_in(path, Some(context))?;
        let mut grew = false;
        for input in &resolved.library.inputs {
            grew |= self.owned.add_known(input.path.to_string(), context);
        }
        for f in self.session.known_files(context) {
            grew |= self.owned.add_known(f, context);
        }
        if grew {
            *self.search_scope = None;
        }
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let index = Arc::new(LegacyUnitIndex::build(
            &ctx,
            &unit.ast,
            &unit.tables,
            unit.unit,
        ));
        self.indexes.insert(key, index.clone());
        Some(index)
    }

    pub fn search_index(
        &mut self,
        element: &SElem,
        kinds: &[(RelationKind, protocol::SearchResultKind)],
    ) -> Vec<LegacyMatch> {
        let Some(name) = element.with(|ctx| {
            ctx.element_data(element.id)
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n).to_string())
        }) else {
            return Vec::new();
        };
        let Some(key) = element.key() else {
            return Vec::new();
        };
        let mut reference_names = vec![name.clone()];
        let mut library_files: Vec<Vec<String>> = Vec::new();
        let library_files_of = |e: ElementId| library_file_paths(element, e);
        library_files.push(library_files_of(element.id));
        if element.id.tag() == Tag::Constructor && name == "new" {
            let class = element.with(|ctx| ctx.element_data(element.id).and_then(|d| d.enclosing));
            if let Some(class) = class {
                if let Some(class_name) = element.with(|ctx| {
                    ctx.element_data(class)
                        .and_then(|d| d.name)
                        .map(|n| ctx.name_str(n).to_string())
                }) {
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
                let Some(index) = self.unit_index(&f, *context) else {
                    continue;
                };
                for r in index
                    .unit_index
                    .relations_of(&key, |k| kinds.iter().any(|(rk, _)| *rk == k))
                {
                    let Some((_, result_kind)) = kinds.iter().find(|(rk, _)| *rk == r.kind) else {
                        continue;
                    };
                    results.push(LegacyMatch {
                        path: f.clone(),
                        offset: r.offset,
                        length: r.length,
                        context: *context,
                        kind: result_kind.clone(),
                        is_potential: false,
                    });
                }
            }
        }
        results
    }

    pub fn search_references(&mut self, element: &SElem) -> Vec<LegacyMatch> {
        let id = element.id;
        match id.tag() {
            Tag::Extension
            | Tag::Class
            | Tag::Enum
            | Tag::Mixin
            | Tag::ExtensionType
            | Tag::Setter
            | Tag::TypeAlias => self.search_index(element, REFERENCES),
            Tag::Constructor => self.search_index(element, CONSTRUCTOR),
            Tag::Getter => self.search_index(element, GETTER),
            Tag::Field | Tag::TopLevelVariable => {
                let (getter, setter, origin) = element.with(|ctx| {
                    let (getter, setter) = match ctx.any(id) {
                        AnyElement::Field(f) => {
                            (f.getter.map(|g| g.raw()), f.setter.map(|s| s.raw()))
                        }
                        AnyElement::TopLevelVariable(v) => {
                            (v.getter.map(|g| g.raw()), v.setter.map(|s| s.raw()))
                        }
                        _ => (None, None),
                    };
                    let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, id);
                    (
                        getter,
                        setter,
                        flags.contains(
                            FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
                        ),
                    )
                });
                let mut results = Vec::new();
                if origin {
                    results.extend(self.search_index(element, FIELD));
                }
                if let Some(g) = getter {
                    results.extend(self.search_index(&element.same(g), GETTER_OF_FIELD));
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
                        ast.is::<Block>(n)
                            || ast.parent(n).is_some_and(|p| ast.is::<CompilationUnit>(p))
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

    fn element_unit(&mut self, element: &SElem) -> Option<(ResolvedUnitRef, u32)> {
        let (path, name_offset) = element.with(|ctx| {
            let first = ctx.element_data(element.id)?.first_fragment;
            let lib_frag = find_library_fragment(ctx, first)?;
            let path = ctx.fragment(lib_frag).source.path.to_string();
            let data = ctx.fragment_data(first)?;
            let offset = data.name_offset.or(if element.id.tag() == Tag::Label {
                data.first_token_offset
            } else {
                None
            });
            Some((path, offset))
        })?;
        let owner = self.owner_of(&path)?;
        let resolved = self.resolved_unit_in(&path, Some(owner))?;
        Some((resolved, name_offset?))
    }

    fn search_local(
        &mut self,
        element: &SElem,
        is_root: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> Vec<LegacyMatch> {
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
        let elements = vec![element.id];
        self.local_references(&resolved, root, &elements)
    }

    fn local_references(
        &self,
        resolved: &ResolvedUnitRef,
        root: NodeId,
        elements: &[ElementId],
    ) -> Vec<LegacyMatch> {
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
            .map(|(offset, length, kind)| LegacyMatch {
                path: path.clone(),
                offset,
                length,
                context: resolved.library.context,
                kind,
                is_potential: false,
            })
            .collect()
    }

    fn search_pattern_variable(&mut self, element: &SElem) -> Vec<LegacyMatch> {
        let Some((resolved, _)) = self.element_unit(element) else {
            return Vec::new();
        };
        let variables = element.with(|ctx| {
            let mut root = element.id;
            while let Some(j) = dartr_resolver::element_ext::pattern_variable_join(ctx, root) {
                root = j;
            }
            let mut out = Vec::new();
            let mut stack = vec![root];
            while let Some(v) = stack.pop() {
                if v.tag() == Tag::JoinPatternVariable {
                    let mut components =
                        dartr_resolver::element_ext::join_pattern_variable_components(ctx, v);
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
        let bind = variables
            .iter()
            .copied()
            .find(|v| v.tag() == Tag::BindPatternVariable);
        let Some(bind) = bind else { return Vec::new() };
        let Some(node) =
            element.with(|ctx| dartr_resolver::element_ext::bind_pattern_variable_node(ctx, bind))
        else {
            return Vec::new();
        };
        let unit = resolved.unit();
        let ast = &unit.ast;
        let mut current = Some(node);
        let mut root = None;
        while let Some(n) = current {
            if ast.is::<SwitchExpression>(n)
                || ast.is::<Block>(n)
                || ast.is::<ExpressionFunctionBody>(n)
            {
                root = Some(n);
                break;
            }
            current = ast.parent(n);
        }
        let Some(root) = root else { return Vec::new() };
        self.local_references(&resolved, root, &variables)
    }

    fn search_prefix(&mut self, element: &SElem) -> Vec<LegacyMatch> {
        let mut results = Vec::new();
        let paths: Vec<String> = element
            .lib
            .inputs
            .iter()
            .map(|u| u.path.to_string())
            .collect();
        for path in paths {
            if let Some(resolved) = self.resolved_unit_in(&path, Some(element.lib.context)) {
                let root = resolved.unit().unit.raw();
                results.extend(self.local_references(&resolved, root, &[element.id]));
            }
        }
        results
    }

    fn search_library(&mut self, element: &SElem) -> Vec<LegacyMatch> {
        let paths = library_file_paths(element, element.id);
        let Some(first) = paths.first() else {
            return Vec::new();
        };
        let Some(owner) = self.owner_of(first) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for path in paths {
            let Some(resolved) = self.resolved_unit_in(&path, Some(owner)) else {
                continue;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
            for &d in ast.list_raw(ast[unit.unit].directives) {
                if let Some(p) = ast.cast::<PartOfDirective>(d) {
                    let target = ast[p]
                        .library_name
                        .map(|n| n.raw())
                        .or(ast[p].uri.map(|u| u.raw()));
                    if let Some(t) = target {
                        results.push(LegacyMatch {
                            path: path.clone(),
                            offset: ast.offset(t),
                            length: ast.length(t),
                            context: owner,
                            kind: protocol::SearchResultKind::REFERENCE,
                            is_potential: false,
                        });
                    }
                }
            }
        }
        results
    }

    pub fn direct_subtypes(&mut self, class: &SElem) -> Vec<SElem> {
        let matches = self.search_index(class, SUBTYPES);
        let mut out = Vec::new();
        for m in matches {
            let Some(resolved) = self.resolved_unit_in(&m.path, Some(m.context)) else {
                continue;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
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

    pub fn append_all_subtypes(
        &mut self,
        class: &SElem,
        all: &mut Vec<SElem>,
        keys: &mut Vec<(usize, ElementKey)>,
    ) {
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

    pub fn hierarchy_members_and_parameters(&mut self, member: &SElem) -> (Vec<SElem>, Vec<SElem>) {
        let id = member.id;
        let mut members = Vec::new();
        let mut parameters = Vec::new();
        let enclosing = member.with(|ctx| ctx.element_data(id).and_then(|d| d.enclosing));
        if enclosing.is_some_and(|e| e.tag() == Tag::Extension) {
            members.push(member.clone());
            return (members, parameters);
        }
        let is_static = member.with(|ctx| member::is_static(ctx, ElemRef::Base(id)));
        if id.tag() == Tag::Constructor
            || (matches!(id.tag(), Tag::Field | Tag::Method) && is_static)
        {
            members.push(member.clone());
            return (members, parameters);
        }
        let Some(class) = enclosing.and_then(|e| e.cast::<InterfaceElement>()) else {
            return (members, parameters);
        };
        let name = member.with(|ctx| support::display_name(ctx, id));
        let is_private = name.starts_with('_');
        let member_library = member.with(|ctx| support::library_of(ctx, id));
        let search_classes: Vec<ElementId> = member.with(|ctx| {
            let mut out: Vec<ElementId> =
                dartr_typesystem::class_hierarchy::implemented_interfaces(ctx, class)
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
            let declares =
                member.with(|ctx| !class_members(ctx, super_class, Some(&name)).is_empty());
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
                    support::library_of(ctx, s.id).map(|l| {
                        ctx.fragment(ctx.get(l).first_fragment())
                            .source
                            .path
                            .to_string()
                    })
                });
                let member_path = member.with(|ctx| {
                    member_library.map(|l| {
                        ctx.fragment(ctx.get(l).first_fragment())
                            .source
                            .path
                            .to_string()
                    })
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
                    let Some(interface) = sub.id.cast::<InterfaceElement>() else {
                        return Vec::new();
                    };
                    let mut out = Vec::new();
                    for &c in &ctx.interface(interface).constructors {
                        for &p in &ctx.executable(c.upcast()).formal_params {
                            if p.raw().tag() == Tag::FieldFormalParameter
                                && let AnyElement::FormalParameter(fp) = ctx.any(p.raw())
                                && let Some(f) = fp.field.get()
                                && element_key(ctx, f.raw()) == member_key
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

    pub fn hierarchy_named_parameters(&mut self, parameter: &SElem) -> Vec<SElem> {
        let (named, name, enclosing) = parameter.with(|ctx| {
            let data = ctx.get(EId::<FormalParameterElement>::from_raw(parameter.id));
            (
                data.kind.is_named(),
                data.name.map(|n| ctx.name_str(n).to_string()),
                ctx.element_data(parameter.id).and_then(|d| d.enclosing),
            )
        });
        if named && let Some(method) = enclosing.filter(|e| e.tag() == Tag::Method) {
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

    pub fn match_to_search_result(&mut self, m: &LegacyMatch) -> Option<protocol::SearchResult> {
        let resolved = self.resolved_unit_in(&m.path, Some(m.context))?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let enclosing_frag = get_enclosing_fragment(&ctx, unit.fragment, m.offset);
        let enclosing_element = *ctx.fragment_data(enclosing_frag)?.element.try_get()?;
        let loc_lib_frag = if let Some(lib_el) = enclosing_element.cast::<LibraryElement>() {
            ctx.get(lib_el).first_fragment()
        } else {
            let first_frag = ctx.element_data(enclosing_element)?.first_fragment;
            find_library_fragment(&ctx, first_frag)?
        };
        let loc_frag_data = ctx.fragment(loc_lib_frag);
        let location = location_from_starts(
            &loc_frag_data.source.path,
            &loc_frag_data.line_starts,
            m.offset,
            m.length,
        );
        let path = compute_element_path(&ctx, enclosing_element);
        Some(protocol::SearchResult {
            location,
            kind: m.kind.clone(),
            is_potential: m.is_potential,
            path,
        })
    }

    pub fn search_unresolved_member_references(&mut self, name: &str) -> Vec<LegacyMatch> {
        let mut results = Vec::new();
        let scope = self.search_scope();
        let names = vec![name.to_string()];
        for (context, owned) in scope.iter() {
            for f in owned {
                if !self.mentions_any(f, &names) {
                    continue;
                }
                let Some(index) = self.unit_index(f, *context) else {
                    continue;
                };
                for r in index.unresolved_for(name) {
                    results.push(LegacyMatch {
                        path: f.clone(),
                        offset: r.offset,
                        length: r.length,
                        context: *context,
                        kind: r.kind.clone(),
                        is_potential: true,
                    });
                }
            }
        }
        results
    }

    pub fn find_element_references(
        &mut self,
        element: &SElem,
        include_potential: bool,
    ) -> Vec<protocol::SearchResult> {
        let ref_elements: Vec<SElem> = if is_named_parameter(element) {
            self.hierarchy_named_parameters(element)
        } else if matches!(
            element.id.tag(),
            Tag::Method | Tag::Field | Tag::Constructor
        ) {
            let (mut members, parameters) = self.hierarchy_members_and_parameters(element);
            members.extend(parameters);
            members
        } else {
            vec![element.clone()]
        };
        let mut matches = Vec::new();
        for ref_el in &ref_elements {
            matches.extend(self.search_references(ref_el));
        }
        if include_potential && is_member_element(element) {
            let name = element.with(|ctx| support::display_name(ctx, element.id));
            matches.extend(self.search_unresolved_member_references(&name));
        }
        matches
            .iter()
            .filter_map(|m| self.match_to_search_result(m))
            .collect()
    }

    pub fn find_member_references(&mut self, name: &str) -> Vec<protocol::SearchResult> {
        let matches = self.search_unresolved_member_references(name);
        matches
            .iter()
            .filter_map(|m| self.match_to_search_result(m))
            .collect()
    }

    fn parse_file_in_context(
        &self,
        context_idx: usize,
        file: &str,
    ) -> Option<dartr_ast_builder::ParsedUnit> {
        let context = self.collection.contexts.get(context_idx)?;
        let content = fs::read_string(file)?;
        let content = dartr_syntax::strip_bom(&content);
        let version = context.file_info(file).language_version;
        let options = self.collection.options_for(context, file);
        let experiments: Vec<_> = options
            .enabled_experiments()
            .iter()
            .filter_map(|name| {
                dartr_parser::ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .find(|flag| flag.name() == *name)
            })
            .collect();
        Some(dartr_ast_builder::parse_file(
            content,
            file,
            (version.major, version.minor),
            &experiments,
        ))
    }

    pub fn find_member_declarations(&mut self, name: &str) -> Vec<protocol::SearchResult> {
        let mut results = Vec::new();
        let scope = self.search_scope();
        let is_ident = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        let names = vec![name.to_string()];
        for (context, owned) in scope.iter() {
            let mut checked_libraries: HashSet<String> = HashSet::new();
            for f in owned {
                if is_ident && !self.mentions_any(f, &names) {
                    continue;
                }
                let Some(parsed) = self.parse_file_in_context(*context, f) else {
                    continue;
                };
                if !parsed_defines_class_member(&parsed.ast, parsed.unit, name) {
                    continue;
                }
                let Some(resolved) = self.resolved_unit_in(f, Some(*context)) else {
                    continue;
                };
                let Some(first_input) = resolved.library.inputs.first() else {
                    continue;
                };
                let lib_path = first_input.path.to_string();
                if !checked_libraries.insert(lib_path) {
                    continue;
                }
                for elem in collect_library_class_members(&resolved.library, name) {
                    if let Some(res) = search_result_for_element(&elem) {
                        results.push(res);
                    }
                }
            }
        }
        results
    }

    pub fn find_top_level_declarations(
        &mut self,
        regex: &regress::Regex,
    ) -> Vec<protocol::SearchResult> {
        let mut results = Vec::new();
        let scope = self.search_scope();
        for (context, owned) in scope.iter() {
            let mut checked_libraries: HashSet<String> = HashSet::new();
            for f in owned {
                let is_added = self.owned.added.contains_key(f);
                if !is_added {
                    let Some(parsed) = self.parse_file_in_context(*context, f) else {
                        continue;
                    };
                    let has_parts = parsed
                        .ast
                        .list_raw(parsed.ast[parsed.unit].directives)
                        .iter()
                        .any(|&d| parsed.ast.is::<PartDirective>(d));
                    if !has_parts && !parsed_has_matching_top_level(&parsed.ast, parsed.unit, regex)
                    {
                        continue;
                    }
                }
                let Some(resolved) = self.resolved_unit_in(f, Some(*context)) else {
                    continue;
                };
                let Some(first_input) = resolved.library.inputs.first() else {
                    continue;
                };
                let lib_path = first_input.path.to_string();
                if !checked_libraries.insert(lib_path) {
                    continue;
                }
                for elem in collect_library_top_level_elements(&resolved.library, regex) {
                    if let Some(res) = search_result_for_element(&elem) {
                        results.push(res);
                    }
                }
            }
        }
        results
    }

    pub fn compute_type_hierarchy(
        &mut self,
        pivot: &SElem,
        super_only: bool,
    ) -> Option<Vec<protocol::TypeHierarchyItem>> {
        let helper =
            pivot.with(|ctx| HierarchyHelper::from_element(ctx, pivot.lib.context, pivot.id));
        let pivot_class = helper.pivot_class?;
        let pivot_class = pivot.same(pivot_class);
        let mut builder = SuperHierarchyBuilder::new(&helper);
        builder.create_super_item(&pivot_class, &[]);
        if !super_only {
            self.create_subclasses(&mut builder, 0);
        }
        Some(builder.items)
    }

    fn create_subclasses(&mut self, builder: &mut SuperHierarchyBuilder<'_>, item_id: usize) {
        let class_el = builder.item_elements[item_id].clone();
        let direct = self.direct_subtypes(&class_el);
        let mut seen_direct: Vec<(usize, ElementKey)> = Vec::new();
        let mut sub_item_ids = Vec::new();
        for sub in direct {
            if let Some(k) = sub.identity() {
                if seen_direct.contains(&k) {
                    continue;
                }
                seen_direct.push(k.clone());
                if let Some(existing_id) = builder.keys.iter().position(|ek| *ek == k) {
                    builder.items[item_id].subclasses.push(existing_id as i64);
                    continue;
                }
                builder.keys.push(k);
            } else {
                builder.keys.push((
                    usize::MAX,
                    ElementKey {
                        library_path: String::new(),
                        unit_path: String::new(),
                        unit_member: Some(format!("unknown-{}", builder.items.len())),
                        class_member: None,
                        parameter: None,
                        kind: dartr_server::index::SyntheticKind::NotSynthetic,
                    },
                ));
            }
            let (class_element, member_element) = sub.with(|ctx| {
                let c_el = convert_element(ctx, sub.id, None);
                let m_el = builder
                    .helper
                    .find_member(ctx, sub.lib.context, sub.id)
                    .map(|m| dartr_element::diagnostics::non_synthetic(ctx, m))
                    .map(|m| convert_element(ctx, m, None));
                (c_el, m_el)
            });
            let sub_item_id = builder.items.len();
            builder.item_elements.push(sub);
            builder.items.push(protocol::TypeHierarchyItem {
                class_element,
                display_name: None,
                member_element,
                superclass: Some(item_id as i64),
                interfaces: Vec::new(),
                mixins: Vec::new(),
                subclasses: Vec::new(),
            });
            builder.items[item_id].subclasses.push(sub_item_id as i64);
            sub_item_ids.push(sub_item_id);
        }
        for sub_item_id in sub_item_ids {
            self.create_subclasses(builder, sub_item_id);
        }
    }

    pub fn members_of_subtypes(&mut self, class: &SElem) -> Option<HashSet<String>> {
        let mut all = Vec::new();
        let mut keys = Vec::new();
        self.append_all_subtypes(class, &mut all, &mut keys);
        if all.is_empty() {
            return None;
        }
        let class_lib_path = class.with(|ctx| {
            support::library_of(ctx, class.id).map(|l| {
                ctx.fragment(ctx.get(l).first_fragment())
                    .source
                    .path
                    .to_string()
            })
        });
        let mut members = HashSet::new();
        for sub in all {
            sub.with(|ctx| {
                let sub_lib_path = support::library_of(ctx, sub.id).map(|l| {
                    ctx.fragment(ctx.get(l).first_fragment())
                        .source
                        .path
                        .to_string()
                });
                let same_lib = class_lib_path.is_some() && class_lib_path == sub_lib_path;
                for m in class_members(ctx, sub.id, None) {
                    if member::is_static(ctx, ElemRef::Base(m)) {
                        continue;
                    }
                    let name = support::display_name(ctx, m);
                    if !name.is_empty() && (same_lib || !name.starts_with('_')) {
                        members.insert(name);
                    }
                }
            });
        }
        Some(members)
    }

    /// Computes `search.getElementDeclarations` (`FindDeclarations`).
    pub fn get_element_declarations(
        &mut self,
        pattern: &str,
        max_results: Option<i64>,
        only_for_file: Option<&str>,
    ) -> protocol::SearchGetElementDeclarationsResult {
        let max = max_results.map(|m| usize::try_from(m.max(0)).unwrap_or(usize::MAX));
        if max == Some(0) {
            return protocol::SearchGetElementDeclarationsResult {
                declarations: Vec::new(),
                files: Vec::new(),
            };
        }
        self.search_scope();
        let mut entries: Vec<(String, usize)> = self
            .owned
            .added
            .iter()
            .map(|(f, c)| (f.clone(), *c))
            .collect();
        entries.extend(self.owned.known.iter().map(|(f, c)| (f.clone(), *c)));

        let mut matcher = dartr_server::fuzzy::FuzzyMatcher::new(pattern);
        let mut declarations = Vec::new();
        let mut files = Vec::new();
        let mut path_to_index: FxHashMap<String, i64> = FxHashMap::default();
        let mut processed: HashSet<(usize, String)> = HashSet::new();

        for (file, owner) in entries {
            let Some(linked) = self
                .session
                .linked_library_in(self.collection, owner, &file)
            else {
                continue;
            };
            let key = (owner, linked.library_path.clone());
            if !processed.insert(key) {
                continue;
            }
            if only_for_file.is_some_and(|only| linked.library_path != only) {
                continue;
            }
            let sink = NoopSink;
            let features = dartr_element::FeatureSet::default();
            let ctx = linked.ctx(&sink, &features);
            let Some(library) = ctx.world.libraries.get(linked.uri.as_str()).copied() else {
                continue;
            };
            let mut finder = LegacyLibraryDeclarations {
                matcher: &mut matcher,
                declarations: &mut declarations,
                files: &mut files,
                path_to_index: &mut path_to_index,
                max,
            };
            if finder.compute(&ctx, library).is_err() {
                break;
            }
        }

        protocol::SearchGetElementDeclarationsResult {
            declarations,
            files,
        }
    }
}

struct FullDeclarations;

struct LegacyLibraryDeclarations<'a> {
    matcher: &'a mut dartr_server::fuzzy::FuzzyMatcher,
    declarations: &'a mut Vec<protocol::ElementDeclaration>,
    files: &'a mut Vec<String>,
    path_to_index: &'a mut FxHashMap<String, i64>,
    max: Option<usize>,
}

impl LegacyLibraryDeclarations<'_> {
    fn is_full(&self) -> bool {
        self.max.is_some_and(|m| self.declarations.len() >= m)
    }

    fn name(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
        ctx.element_data(e)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).to_string())
    }

    fn first_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
        ctx.element_data(e)
            .and_then(|d| ctx.fragment_data(d.first_fragment))
            .map(|f| f.flags.get())
            .unwrap_or_default()
    }

    fn search_element_kind(ctx: &Ctx<'_>, e: ElementId) -> Option<protocol::ElementKind> {
        Some(match e.tag() {
            Tag::Enum => protocol::ElementKind::ENUM,
            Tag::ExtensionType => protocol::ElementKind::ExtensionType,
            Tag::Mixin => protocol::ElementKind::MIXIN,
            Tag::Class => {
                if Self::first_flags(ctx, e)
                    .contains(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
                {
                    protocol::ElementKind::ClassTypeAlias
                } else {
                    protocol::ElementKind::CLASS
                }
            }
            Tag::Constructor => protocol::ElementKind::CONSTRUCTOR,
            Tag::Extension => protocol::ElementKind::EXTENSION,
            Tag::Field => {
                if dartr_resolver::element_ext::is_enum_constant(ctx, e) {
                    protocol::ElementKind::EnumConstant
                } else {
                    protocol::ElementKind::FIELD
                }
            }
            Tag::LocalFunction | Tag::TopLevelFunction => protocol::ElementKind::FUNCTION,
            Tag::Method => protocol::ElementKind::METHOD,
            Tag::Getter => protocol::ElementKind::GETTER,
            Tag::Setter => protocol::ElementKind::SETTER,
            Tag::TypeAlias => protocol::ElementKind::TypeAlias,
            Tag::TopLevelVariable | Tag::LocalVariable | Tag::FormalParameter => {
                protocol::ElementKind::TopLevelVariable
            }
            _ => return None,
        })
    }

    fn add(&mut self, ctx: &Ctx<'_>, e: ElementId, name: String) -> Result<(), FullDeclarations> {
        if self.is_full() {
            return Err(FullDeclarations);
        }
        let enclosing = ctx.element_data(e).and_then(|d| d.enclosing);
        let (mut class_name, mut mixin_name) = (None, None);
        match enclosing.map(|x| x.tag()) {
            Some(Tag::Enum) => {}
            Some(Tag::Mixin) => mixin_name = enclosing.and_then(|x| Self::name(ctx, x)),
            Some(Tag::Class) | Some(Tag::ExtensionType) => {
                class_name = enclosing.and_then(|x| Self::name(ctx, x))
            }
            _ => {}
        }
        let filtered = if e.tag() == Tag::Constructor {
            let class = enclosing
                .and_then(|x| Self::name(ctx, x))
                .unwrap_or_else(|| "<null>".into());
            if name == "new" {
                class
            } else {
                format!("{class}.{name}")
            }
        } else {
            name.clone()
        };
        if self.matcher.score(&filtered) < 0.0 {
            return Ok(());
        }
        let Some(kind) = Self::search_element_kind(ctx, e) else {
            return Ok(());
        };
        let parameters = if dartr_server::element_locator::is_executable(e) {
            let display = dartr_element::display_string::element_display_string_with(
                ctx,
                e,
                DisplayOptions::default(),
            );
            match display.find('(') {
                Some(i) if i > 0 => Some(display[i..].to_string()),
                _ => None,
            }
        } else {
            None
        };
        let Some(first) = ctx.element_data(e).map(|d| d.first_fragment) else {
            return Ok(());
        };
        let Some(lib_frag_id) = find_library_fragment(ctx, first) else {
            return Ok(());
        };
        let lib_frag = ctx.fragment(lib_frag_id);
        let file_path = lib_frag.source.path.to_string();
        let Some(data) = ctx.fragment_data(first) else {
            return Ok(());
        };
        let mut location_offset = data.name_offset;
        if location_offset.is_none()
            && let Some(c) = first.cast::<dartr_element::ConstructorFragment>()
        {
            location_offset = ctx.fragment(c).type_name_offset;
        }
        let Some(location_offset) = location_offset else {
            return Ok(());
        };
        let (line, column) =
            crate::convert::line_col_from_starts(&lib_frag.line_starts, location_offset);
        let file_index = match self.path_to_index.get(&file_path) {
            Some(&idx) => idx,
            None => {
                let idx = self.files.len() as i64;
                self.files.push(file_path.clone());
                self.path_to_index.insert(file_path, idx);
                idx
            }
        };
        self.declarations.push(protocol::ElementDeclaration {
            name,
            kind,
            file_index,
            offset: location_offset as i64,
            line: line as i64,
            column: column as i64,
            code_offset: data.code_offset.unwrap_or(0) as i64,
            code_length: data.code_length.unwrap_or(0) as i64,
            class_name,
            mixin_name,
            parameters,
        });
        Ok(())
    }

    fn origin(ctx: &Ctx<'_>, e: ElementId) -> bool {
        let f = Self::first_flags(ctx, e);
        match e.tag() {
            Tag::Constructor => {
                f.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION)
            }
            Tag::Field | Tag::TopLevelVariable => {
                f.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
            }
            Tag::Getter | Tag::Setter => {
                f.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
            }
            _ => true,
        }
    }

    fn named(
        &mut self,
        ctx: &Ctx<'_>,
        elements: &[ElementId],
        check_origin: bool,
        display: bool,
    ) -> Result<(), FullDeclarations> {
        for &e in elements {
            if check_origin && !Self::origin(ctx, e) {
                continue;
            }
            let name = if display {
                Some(support::display_name(ctx, e))
            } else {
                Self::name(ctx, e)
            };
            if let Some(n) = name {
                self.add(ctx, e, n)?;
            }
        }
        Ok(())
    }

    fn members(
        &mut self,
        ctx: &Ctx<'_>,
        e: ElementId,
        with_constructors: bool,
    ) -> Result<(), FullDeclarations> {
        let Some(instance) = e.cast::<InstanceElement>() else {
            return Ok(());
        };
        let data = ctx.instance(instance);
        let getters: Vec<ElementId> = data.getters.iter().map(|x| x.raw()).collect();
        let fields: Vec<ElementId> = data.fields.iter().map(|x| x.raw()).collect();
        let methods: Vec<ElementId> = data.methods.iter().map(|x| x.raw()).collect();
        let setters: Vec<ElementId> = data.setters.iter().map(|x| x.raw()).collect();
        if with_constructors {
            self.named(ctx, &getters, true, true)?;
            if let Some(interface) = e.cast::<InterfaceElement>() {
                let constructors: Vec<ElementId> = ctx
                    .interface(interface)
                    .constructors
                    .iter()
                    .map(|x| x.raw())
                    .collect();
                self.named(ctx, &constructors, true, false)?;
            }
            self.named(ctx, &fields, true, false)?;
            self.named(ctx, &methods, false, false)?;
            self.named(ctx, &setters, true, true)?;
        } else {
            self.named(ctx, &fields, true, false)?;
            self.named(ctx, &getters, true, true)?;
            self.named(ctx, &methods, false, false)?;
            self.named(ctx, &setters, true, true)?;
        }
        Ok(())
    }

    fn classes(&mut self, ctx: &Ctx<'_>, elements: &[ElementId]) -> Result<(), FullDeclarations> {
        for &e in elements {
            if let Some(name) = Self::name(ctx, e) {
                self.add(ctx, e, name)?;
                self.members(ctx, e, true)?;
            }
        }
        Ok(())
    }

    fn compute(
        &mut self,
        ctx: &Ctx<'_>,
        library: EId<LibraryElement>,
    ) -> Result<(), FullDeclarations> {
        if self.is_full() {
            return Err(FullDeclarations);
        }
        let l = ctx.get(library);
        let ids = |v: Vec<ElementId>| v;
        self.classes(ctx, &ids(l.classes.iter().map(|x| x.raw()).collect()))?;
        self.named(
            ctx,
            &ids(l.getters.iter().map(|x| x.raw()).collect()),
            true,
            true,
        )?;
        self.classes(ctx, &ids(l.enums.iter().map(|x| x.raw()).collect()))?;
        self.classes(ctx, &ids(l.mixins.iter().map(|x| x.raw()).collect()))?;
        for &e in &l.extensions {
            let e = e.raw();
            if let Some(name) = Self::name(ctx, e) {
                self.add(ctx, e, name)?;
            }
            self.members(ctx, e, false)?;
        }
        self.classes(
            ctx,
            &ids(l.extension_types.iter().map(|x| x.raw()).collect()),
        )?;
        self.named(
            ctx,
            &ids(l.setters.iter().map(|x| x.raw()).collect()),
            true,
            true,
        )?;
        self.named(
            ctx,
            &ids(l.top_level_functions.iter().map(|x| x.raw()).collect()),
            false,
            false,
        )?;
        self.named(
            ctx,
            &ids(l.top_level_variables.iter().map(|x| x.raw()).collect()),
            true,
            false,
        )?;
        self.named(
            ctx,
            &ids(l.type_aliases.iter().map(|x| x.raw()).collect()),
            false,
            false,
        )?;
        Ok(())
    }
}

/// Computes `analysis.getImportedElements` (`ImportedElementsComputer`).
pub fn compute_imported_elements(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    unit: Id<CompilationUnit>,
    offset: i64,
    length: i64,
) -> Vec<protocol::ImportedElements> {
    let directives = ast.list_raw(ast[unit].directives);
    if let Some(&last) = directives.last()
        && offset < ast.end(last) as i64
    {
        return Vec::new();
    }
    let mut visitor = ImportedElementsVisitor {
        ctx,
        tables,
        start_offset: offset,
        end_offset: offset.saturating_add(length),
        imported_elements: IndexMap::new(),
    };
    visitor.visit_node(ast, unit.raw());
    visitor.imported_elements.into_values().collect()
}

struct ImportedElementsVisitor<'a, 'b> {
    ctx: &'a Ctx<'b>,
    tables: &'a dartr_element::ResolutionTables,
    start_offset: i64,
    end_offset: i64,
    imported_elements: IndexMap<String, protocol::ImportedElements>,
}

impl ImportedElementsVisitor<'_, '_> {
    fn overlaps(&self, ast: &Ast, node: impl Into<NodeId>) -> bool {
        let n = node.into();
        (ast.offset(n) as i64) <= self.end_offset && (ast.end(n) as i64) >= self.start_offset
    }

    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.tables
            .element
            .get(node.into())
            .map(|&e| member::base_element(self.ctx, e))
    }

    fn get_prefix_from(&self, ast: &Ast, identifier: Id<SimpleIdentifier>) -> String {
        if self.overlaps(ast, identifier)
            && let Some(el) = self.element(identifier)
            && el.tag() == Tag::Prefix
        {
            return self
                .ctx
                .element_data(el)
                .and_then(|d| d.name)
                .map(|n| self.ctx.name_str(n).to_string())
                .unwrap_or_default();
        }
        String::new()
    }

    fn add_element(&mut self, prefix: &str, element: Option<ElementId>) {
        let Some(element) = element else {
            return;
        };
        if element.tag() == Tag::Prefix {
            return;
        }
        if !self
            .ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .is_some_and(|e| e.tag() == Tag::Library)
        {
            return;
        }
        let Some(lib) = support::library_of(self.ctx, element) else {
            return;
        };
        let path = self
            .ctx
            .fragment(self.ctx.get(lib).first_fragment())
            .source
            .path
            .to_string();
        let key = format!("{prefix};{path}");
        let entry =
            self.imported_elements
                .entry(key)
                .or_insert_with(|| protocol::ImportedElements {
                    path,
                    prefix: prefix.to_string(),
                    elements: Vec::new(),
                });
        if let Some(element_name) = self
            .ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| self.ctx.name_str(n).to_string())
            && !entry.elements.contains(&element_name)
        {
            entry.elements.push(element_name);
        }
    }

    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        if !self.overlaps(ast, node) {
            return;
        }
        if let Some(named_type) = ast.cast::<NamedType>(node) {
            let prefix = ast[named_type]
                .import_prefix
                .and_then(|p| self.element(p))
                .and_then(|e| self.ctx.element_data(e)?.name)
                .map(|n| self.ctx.name_str(n).to_string())
                .unwrap_or_default();
            let el = self.element(named_type);
            self.add_element(&prefix, el);
            ast.visit_children(node, self);
        } else if let Some(ident) = ast.cast::<SimpleIdentifier>(node) {
            let is_ctor_return_type = ast
                .parent(ident)
                .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
                .is_some_and(|c| ast[c].type_name == Some(ident.into()));
            if !support::in_declaration_context(ast, ident) && !is_ctor_return_type {
                let node_element =
                    support::write_or_read_element(self.ctx, ast, self.tables, ident)
                        .or_else(|| support::read_element(self.ctx, self.tables, ident))
                        .or_else(|| self.element(ident));
                let mut prefix = String::new();
                if let Some(parent) = ast.parent(ident) {
                    if let Some(prefixed) = ast.cast::<PrefixedIdentifier>(parent)
                        && ast[prefixed].identifier == ident
                    {
                        prefix = self.get_prefix_from(ast, ast[prefixed].prefix);
                    } else if let Some(inv) = ast.cast::<MethodInvocation>(parent)
                        && ast[inv].method_name == ident
                        && let Some(target) = ast[inv].target
                        && let Some(target_ident) = ast.cast::<SimpleIdentifier>(target)
                    {
                        prefix = self.get_prefix_from(ast, target_ident);
                    }
                }
                self.add_element(&prefix, node_element);
            }
        } else {
            ast.visit_children(node, self);
        }
    }
}

impl AstVisitor for ImportedElementsVisitor<'_, '_> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        ImportedElementsVisitor::visit_node(self, ast, node);
    }
}
