// Dart source: pkg/analyzer/lib/src/error/imports_verifier.dart
// Dart source: pkg/analyzer/lib/src/dart/element/scope.dart (ImportsTracking,
// ImportsTrackingOfPrefix)
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart
// (_hasDiagnosticReportedThatPreventsImportWarnings)

//! Unused, duplicate and unnecessary imports: [`verify`] (Dart
//! `ImportsVerifier`, `addImports` and the `generate*` steps).
//!
//! # Import usage tracking
//!
//! Dart records the usage of the imports while it resolves the files of a
//! library: every lookup in an import scope (`PrefixScope.lookup`) records
//! the found getter and setter for the imports that provide them
//! (`ImportsTrackingOfPrefix.lookupResult`), and the extension member
//! resolution records the used extensions (`notifyExtensionUsed`).
//! The scopes of this port do not track (see [`crate::scope`]), so
//! [`compute_imports_tracking`] replays the lookups after resolution from
//! the resolved units of the library:
//!
//! - every `SimpleIdentifier` with a lexical lookup result
//!   (`ResolverTables.scope_lookup_result`, the identifiers that the Dart
//!   resolution visitor looks up) whose result is not a declaration of the
//!   library (so the lookup reached the import scopes), except the names of
//!   combinators (Dart deactivates the tracking there);
//! - every unprefixed `NamedType` and `ExtensionOverride` whose element is
//!   not a declaration of the library;
//! - the names after an import prefix (`p.x`, `p.f()`, `p.T`), looked up in
//!   the scope of the prefix;
//! - the implicit uses of the instance members of extensions of other
//!   libraries (the receiver is not an extension override);
//! - an import prefix alone in a comment reference
//!   (`notifyPrefixUsedInCommentReference`).
//!
//! The replayed lookup walks the import scopes like Dart
//! `LibraryFragmentScope._lookupCombined` / `PrefixScope.lookup`, and
//! records into the tracker of the scope that answers.
//!
//! Not ported: `elementsOf2` (the elements from deprecated exports are not
//! excluded; the export namespace does not keep deprecated exports).

use dartr_ast::{
    Ast, AstVisitor, CommentReference, ExtensionOverride, HideCombinator, Id, ImportDirective,
    ImportPrefixReference, NamedType, NodeId, NodeKind, PrefixedIdentifier, ShowCombinator,
    SimpleIdentifier,
};
use dartr_diagnostics::diag;
use dartr_element::{
    Ctx, DirectiveUri, EId, ElemRef, ElementId, FId, LibraryElement, LibraryFragment,
    NamespaceCombinator, PrefixElement, Tag,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::member;
use indexmap::{IndexMap, IndexSet};

use super::dead_code_verifier::{directive_library, is_origin_not_existing_file, library_uri};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext;
use crate::scope::LibraryScopes;

/// Dart `LibraryAnalyzer._hasDiagnosticReportedThatPreventsImportWarnings`
/// over the diagnostics of all units so far.
pub fn has_diagnostic_reported_that_prevents_import_warnings(units: &[UnitVerifier<'_>]) -> bool {
    const CODES: &[&str] = &[
        "ambiguous_import",
        "const_with_non_type",
        "extends_non_class",
        "implements_non_class",
        "mixin_of_non_class",
        "new_with_non_type",
        "not_a_type",
        "prefix_identifier_not_followed_by_dot",
        "undefined_annotation",
        "undefined_class",
        "undefined_function",
        "undefined_identifier",
        "undefined_prefixed_name",
        "deprecated_export_use",
    ];
    units.iter().any(|u| {
        u.diagnostics
            .iter()
            .any(|d| CODES.contains(&d.code.unique_name))
    })
}

// ============================================================ ImportsTracking

/// The key of an import scope: the library fragment and the import prefix
/// (`None`: the imports without prefix).
type ScopeKey = (FId<LibraryFragment>, Option<EId<PrefixElement>>);

/// The getters or setters of an import scope with one name: each element
/// with the indexes (in `LibraryFragment.library_imports`) of the imports
/// that provide it.
type ScopeEntry = IndexMap<ElementId, Vec<usize>>;

/// Dart `ImportsTracking` of all fragments of a library: the tracking of
/// each import scope (Dart `PrefixScope` with `ImportsTrackingOfPrefix`).
#[derive(Default)]
pub struct ImportsTracking {
    scopes: IndexMap<ScopeKey, PrefixScopeTracking>,
}

/// Dart `PrefixScope` with its `ImportsTrackingOfPrefix`.
#[derive(Default)]
struct PrefixScopeTracking {
    /// Dart `PrefixScope.parent`.
    parent: Option<ScopeKey>,
    /// The getters of the scope by name.
    getters: IndexMap<String, ScopeEntry>,
    /// The setters of the scope by name (without `=`).
    setters: IndexMap<String, ScopeEntry>,
    /// Dart `_elementImports`: the imports that provide each element.
    element_imports: IndexMap<ElementId, Vec<usize>>,
    /// Dart `importToUsedElements`.
    import_to_used_elements: IndexMap<usize, IndexSet<ElementId>>,
    /// Dart `hasPrefixUsedInCommentReference`.
    has_prefix_used_in_comment_reference: bool,
}

impl PrefixScopeTracking {
    /// Dart `lookupResult(element)` for the elements of a found name: a
    /// name with more than one element is a `MultiplyDefinedElement`, which
    /// is not recorded.
    fn record(&mut self, found: Option<ScopeEntry>) {
        let Some(found) = found else {
            return;
        };
        if found.len() != 1 {
            return;
        }
        for (element, imports) in found {
            for import in imports {
                self.import_to_used_elements
                    .entry(import)
                    .or_default()
                    .insert(element);
            }
        }
    }
}

impl ImportsTracking {
    /// Dart `ImportsTracking.elementsOf(import)` of the import with
    /// [index] in the scope [key].
    fn elements_of(&self, key: ScopeKey, index: usize) -> IndexSet<ElementId> {
        self.scopes
            .get(&key)
            .and_then(|s| s.import_to_used_elements.get(&index))
            .cloned()
            .unwrap_or_default()
    }

    /// Dart `PrefixScope.lookup(id)` of the scope [key] (and its parents).
    fn prefix_scope_lookup(&mut self, key: ScopeKey, id: &str) -> bool {
        let mut current = Some(key);
        while let Some(k) = current {
            let Some(scope) = self.scopes.get_mut(&k) else {
                return false;
            };
            let getter = scope.getters.get(id).cloned();
            let setter = scope.setters.get(id).cloned();
            if getter.is_some() || setter.is_some() {
                scope.record(getter);
                scope.record(setter);
                return true;
            }
            current = scope.parent;
        }
        false
    }

    /// Dart `ImportsTrackingOfPrefix.notifyExtensionUsed(element)` of the
    /// scope [key].
    fn notify_extension_used_in(&mut self, key: ScopeKey, element: ElementId) {
        let mut current = Some(key);
        while let Some(k) = current {
            let Some(scope) = self.scopes.get_mut(&k) else {
                return;
            };
            if let Some(imports) = scope.element_imports.get(&element).cloned() {
                for import in imports {
                    scope
                        .import_to_used_elements
                        .entry(import)
                        .or_default()
                        .insert(element);
                }
                return;
            }
            // The element is from the accessible extensions of a parent.
            current = scope.parent;
        }
    }

    /// Dart `LibraryFragmentScope.notifyExtensionUsed(element)`.
    fn notify_extension_used(&mut self, fragment: FId<LibraryFragment>, element: ElementId) {
        let keys: Vec<ScopeKey> = self
            .scopes
            .keys()
            .copied()
            .filter(|(f, _)| *f == fragment)
            .collect();
        for key in keys {
            self.notify_extension_used_in(key, element);
        }
    }
}

/// Builds the import scopes of the fragments of the units, and replays the
/// lookups of the resolved units (see the module documentation).
pub fn compute_imports_tracking(units: &[UnitVerifier<'_>]) -> ImportsTracking {
    let mut tracking = ImportsTracking::default();
    let Some(first) = units.first() else {
        return tracking;
    };
    let ctx = first.ctx;
    let scopes = first.scopes;
    for v in units {
        build_fragment_scopes(&ctx, scopes, v.fragment, &mut tracking);
    }
    for v in units {
        let mut replay = Replay {
            v,
            tracking: &mut tracking,
        };
        v.ast.accept(v.unit, &mut replay);
    }
    tracking
}

/// Dart `LibraryFragmentScope(fragment)`: the import scopes of [fragment]
/// (after the scopes of its enclosing fragment).
fn build_fragment_scopes(
    ctx: &Ctx<'_>,
    scopes: &LibraryScopes,
    fragment: FId<LibraryFragment>,
    tracking: &mut ImportsTracking,
) {
    if tracking.scopes.contains_key(&(fragment, None)) {
        return;
    }
    let parent = scopes.enclosing_fragment(fragment);
    if let Some(p) = parent {
        build_fragment_scopes(ctx, scopes, p, tracking);
    }
    let f = ctx.fragment(fragment);
    let scope = build_prefix_scope(ctx, fragment, None, parent.map(|p| (p, None)));
    tracking.scopes.insert((fragment, None), scope);
    for &prefix in &f.library_import_prefixes {
        // Dart `_getParentPrefixScope`.
        let is_deferred = f.library_imports.iter().any(|import| {
            import.prefix.is_some_and(|pf| {
                let p = ctx.fragment(pf);
                p.element.try_get() == Some(&prefix.raw()) && p.is_deferred
            })
        });
        let name = ctx.get(prefix).name;
        let mut parent_scope = None;
        if !is_deferred {
            let mut current = parent;
            while let Some(s) = current {
                let parent_prefix = name.and_then(|n| {
                    ctx.fragment(s)
                        .library_import_prefixes_by_id
                        .get(&n)
                        .copied()
                });
                if let Some(parent_prefix) = parent_prefix {
                    parent_scope = Some((s, Some(parent_prefix)));
                    break;
                }
                current = scopes.enclosing_fragment(s);
            }
        }
        let scope = build_prefix_scope(ctx, fragment, Some(prefix), parent_scope);
        tracking.scopes.insert((fragment, Some(prefix)), scope);
    }
}

/// Dart `PrefixScope(libraryFragment:, parent:, libraryImports:, prefix:)`
/// with `ImportsTrackingOfPrefix._buildElementToImportsMap`.
fn build_prefix_scope(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    prefix: Option<EId<PrefixElement>>,
    parent: Option<ScopeKey>,
) -> PrefixScopeTracking {
    let mut scope = PrefixScopeTracking {
        parent,
        ..PrefixScopeTracking::default()
    };
    for (index, import) in ctx.fragment(fragment).library_imports.iter().enumerate() {
        let Some(imported_library) = directive_library(&import.directive.uri) else {
            continue;
        };
        let import_prefix = import
            .prefix
            .and_then(|p| ctx.fragment(p).element.try_get().copied());
        if import_prefix != prefix.map(|p| p.raw()) {
            continue;
        }
        let Some(namespace) = ctx.get(imported_library).export_namespace.try_get() else {
            continue;
        };
        for (&name, &element) in &namespace.defined_names {
            let name = ctx.name_str(name);
            if !combinators_allow(ctx, &import.combinators, name) {
                continue;
            }
            let imports = scope.element_imports.entry(element).or_default();
            if !imports.contains(&index) {
                imports.push(index);
            }
            let (map, id) = match name.strip_suffix('=') {
                Some(id) => (&mut scope.setters, id),
                None => (&mut scope.getters, name),
            };
            let imports = map
                .entry(id.to_string())
                .or_default()
                .entry(element)
                .or_default();
            if !imports.contains(&index) {
                imports.push(index);
            }
        }
    }
    scope
}

/// Dart `NamespaceCombinator.allows(name)` of a list of combinators.
fn combinators_allow(ctx: &Ctx<'_>, combinators: &[NamespaceCombinator], name: &str) -> bool {
    let name = name.strip_suffix('=').unwrap_or(name);
    let matches = |names: &[dartr_element::Name]| names.iter().any(|&n| ctx.name_str(n) == name);
    combinators.iter().all(|c| match c {
        NamespaceCombinator::Show { shown_names, .. } => matches(shown_names),
        NamespaceCombinator::Hide { hidden_names, .. } => !matches(hidden_names),
    })
}

/// Replays the import scope lookups of one resolved unit.
struct Replay<'v, 't, 'a> {
    v: &'v UnitVerifier<'a>,
    tracking: &'t mut ImportsTracking,
}

impl Replay<'_, '_, '_> {
    /// Whether [element] is a declaration of the library (or a local
    /// element), so that a lookup that found it did not reach the import
    /// scopes.
    fn is_library_element(&self, element: ElementId) -> bool {
        if element.tag() == Tag::MultiplyDefined {
            return false;
        }
        self.v
            .ctx
            .element_data(element)
            .and_then(|d| d.library)
            .is_some_and(|l| l == self.v.library)
    }

    /// Dart `LibraryFragmentScope.lookup(id)` from the fragment of the unit,
    /// after the enclosed scopes did not find [id].
    fn fragment_lookup(&mut self, id: &str) {
        let ctx = self.v.ctx;
        let scopes = self.v.scopes;
        // Dart `_lookupLibrary`.
        if scopes.library_declaration_with_name(id).is_some() {
            return;
        }
        // Dart `_lookupCombined`.
        let wildcard_variables = crate::scope::library_feature_enabled(
            &ctx,
            self.v.library,
            ExperimentalFlag::WildcardVariables,
        );
        let mut current = Some(self.v.fragment);
        while let Some(fragment) = current {
            // Dart `_shouldTryPrefixElement`.
            if (id != "_" || !wildcard_variables)
                && ctx
                    .fragment(fragment)
                    .library_import_prefixes_by_id
                    .contains_key(&ctx.name(id))
            {
                return;
            }
            if self.tracking.prefix_scope_lookup((fragment, None), id) {
                return;
            }
            current = scopes.enclosing_fragment(fragment);
        }
    }

    /// Dart `prefixElement.scope.lookup(id)`.
    fn prefix_lookup(&mut self, prefix: EId<PrefixElement>, id: &str) {
        if let Some(key) = self.prefix_scope_key(prefix) {
            self.tracking.prefix_scope_lookup(key, id);
        }
    }

    /// The import scope of [prefix]: the fragment that declares it.
    fn prefix_scope_key(&self, prefix: EId<PrefixElement>) -> Option<ScopeKey> {
        let ctx = self.v.ctx;
        let mut current = Some(self.v.fragment);
        while let Some(fragment) = current {
            if ctx
                .fragment(fragment)
                .library_import_prefixes
                .contains(&prefix)
            {
                return Some((fragment, Some(prefix)));
            }
            current = self.v.scopes.enclosing_fragment(fragment);
        }
        None
    }

    /// The prefix element that the identifier [node] was looked up as.
    fn looked_up_prefix(&self, node: Id<SimpleIdentifier>) -> Option<EId<PrefixElement>> {
        let result = self.v.rt.scope_lookup_result.get(node)?;
        result.getter?.cast::<PrefixElement>()
    }

    /// The prefix of an `ImportPrefixReference`.
    fn import_prefix_element(&self, node: Id<ImportPrefixReference>) -> Option<EId<PrefixElement>> {
        if let Some(ElemRef::Base(e)) = self.v.tables.element.get(node) {
            return e.cast::<PrefixElement>();
        }
        let ctx = self.v.ctx;
        let name = ctx.name(self.v.ast.tokens.lexeme(self.v.ast[node].name));
        let mut current = Some(self.v.fragment);
        while let Some(fragment) = current {
            if let Some(&p) = ctx
                .fragment(fragment)
                .library_import_prefixes_by_id
                .get(&name)
            {
                return Some(p);
            }
            current = self.v.scopes.enclosing_fragment(fragment);
        }
        None
    }

    /// A type name (`NamedType`, `ExtensionOverride`) with an optional
    /// import prefix and the resolved [element].
    fn type_name(
        &mut self,
        import_prefix: Option<Id<ImportPrefixReference>>,
        name: &str,
        element: Option<ElemRef>,
    ) {
        match import_prefix {
            Some(prefix) => {
                if let Some(prefix) = self.import_prefix_element(prefix) {
                    self.prefix_lookup(prefix, name);
                }
            }
            None => {
                let ctx = self.v.ctx;
                let Some(element) = element.map(|e| member::base_element(&ctx, e)) else {
                    return;
                };
                if element.tag() == Tag::TypeParameter || self.is_library_element(element) {
                    return;
                }
                self.fragment_lookup(name);
            }
        }
    }

    /// Dart `notifyExtensionUsed` of the extension member resolution: the
    /// element of [node] is an instance member of an extension of another
    /// library, and the receiver is not an extension override.
    fn check_extension_use(&mut self, node: NodeId) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        for element in [
            self.v.tables.element.get(node).copied(),
            self.v.tables.read_element.get(node).copied(),
            self.v.tables.write_element.get(node).copied(),
        ]
        .into_iter()
        .flatten()
        {
            let base = member::base_element(&ctx, element);
            if !matches!(base.tag(), Tag::Method | Tag::Getter | Tag::Setter) {
                continue;
            }
            let Some(extension) = ctx.element_data(base).and_then(|d| d.enclosing) else {
                continue;
            };
            if extension.tag() != Tag::Extension
                || self.is_library_element(extension)
                || member::is_static(&ctx, ElemRef::Base(base))
            {
                continue;
            }
            if receiver_is_extension_override(ast, node) {
                continue;
            }
            self.tracking
                .notify_extension_used(self.v.fragment, extension);
        }
    }
}

/// Whether the receiver of the member access [node] (or of the member access
/// whose name is [node]) is an extension override.
fn receiver_is_extension_override(ast: &Ast, node: NodeId) -> bool {
    let receiver = |n: NodeId| -> Option<NodeId> {
        if let Some(e) = ast.cast::<dartr_ast::MethodInvocation>(n) {
            return ast[e].target.map(|t| t.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::PropertyAccess>(n) {
            return ast[e].target.map(|t| t.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::IndexExpression>(n) {
            return ast[e].target.map(|t| t.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::BinaryExpression>(n) {
            return Some(ast[e].left_operand.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::PrefixExpression>(n) {
            return Some(ast[e].operand.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::PostfixExpression>(n) {
            return Some(ast[e].operand.raw());
        }
        if let Some(e) = ast.cast::<dartr_ast::AssignmentExpression>(n) {
            return Some(ast[e].left_hand_side.raw());
        }
        None
    };
    let mut target = receiver(node);
    if target.is_none()
        && ast.is::<SimpleIdentifier>(node)
        && let Some(parent) = ast.parent(node)
    {
        target = receiver(parent);
    }
    // A compound operator of `E(x).p op= v` / `E(x)[i]++`: the receiver of
    // the target.
    if let Some(t) = target
        && matches!(
            ast.kind(t),
            NodeKind::PropertyAccess | NodeKind::IndexExpression
        )
        && matches!(
            ast.kind(node),
            NodeKind::AssignmentExpression
                | NodeKind::PrefixExpression
                | NodeKind::PostfixExpression
        )
    {
        target = receiver(t);
    }
    target.is_some_and(|t| {
        let t = ast
            .cast::<dartr_ast::Expression>(t)
            .map(|e| ast_ext::un_parenthesized(ast, e).raw())
            .unwrap_or(t);
        ast.is::<ExtensionOverride>(t)
    })
}

impl AstVisitor for Replay<'_, '_, '_> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        self.check_extension_use(node);
        ast.visit_children(node, self);
    }

    // Dart: the tracking is not active while the combinators are resolved.
    fn visit_show_combinator(&mut self, _ast: &Ast, _node: Id<ShowCombinator>) {}

    fn visit_hide_combinator(&mut self, _ast: &Ast, _node: Id<HideCombinator>) {}

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        self.check_extension_use(node.raw());
        let Some(result) = self.v.rt.scope_lookup_result.get(node).copied() else {
            return;
        };
        let Some(found) = result.getter.or(result.setter) else {
            return;
        };
        if let Some(prefix) = found.cast::<PrefixElement>() {
            // Dart `CommentReferenceResolver._resolveSimpleIdentifier`:
            // `element.scope.notifyPrefixUsedInCommentReference()`.
            if ast
                .parent(node)
                .is_some_and(|p| ast.is::<CommentReference>(p))
                && let Some(key) = self.prefix_scope_key(prefix)
                && let Some(scope) = self.tracking.scopes.get_mut(&key)
            {
                scope.has_prefix_used_in_comment_reference = true;
            }
            return;
        }
        let from_imports = [result.getter, result.setter]
            .into_iter()
            .flatten()
            .any(|e| !self.is_library_element(e));
        if from_imports {
            let name = ast_ext::identifier_name(ast, node).to_string();
            self.fragment_lookup(&name);
        }
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        if let Some(prefix) = self.looked_up_prefix(ast[node].prefix) {
            let name = ast_ext::identifier_name(ast, ast[node].identifier).to_string();
            self.prefix_lookup(prefix, &name);
        }
        self.check_extension_use(node.raw());
        ast.visit_children(node, self);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<dartr_ast::MethodInvocation>) {
        if let Some(target) = ast[node].target
            && let Some(target) = ast.cast::<SimpleIdentifier>(target)
            && let Some(prefix) = self.looked_up_prefix(target)
        {
            let name = ast_ext::identifier_name(ast, ast[node].method_name).to_string();
            self.prefix_lookup(prefix, &name);
        }
        self.check_extension_use(node.raw());
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let name = ast.tokens.lexeme(ast[node].name).to_string();
        let element = self.v.tables.element.get(node).copied();
        self.type_name(ast[node].import_prefix, &name, element);
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let name = ast.tokens.lexeme(ast[node].name).to_string();
        let element = self.v.tables.element.get(node).copied();
        self.type_name(ast[node].import_prefix, &name, element);
        ast.visit_children(node, self);
    }
}

// ============================================================ ImportsVerifier

/// Dart `_computeWarnings` (imports): `ImportsVerifier(fileAnalysis:)`,
/// `addImports(unit)` and the `generate*` steps in Dart order. [tracking]
/// is the import usage of the library ([`compute_imports_tracking`], Dart
/// `fileAnalysis.importsTracking`).
pub fn verify(v: &mut UnitVerifier<'_>, tracking: &ImportsTracking) {
    let mut verifier = ImportsVerifier::default();
    verifier.add_imports(v);
    verifier.generate_duplicate_export_warnings(v);
    verifier.generate_duplicate_import_warnings(v);
    verifier.generate_duplicate_shown_hidden_name_warnings(v);
    verifier.generate_unused_import_warnings(v, tracking);
    verifier.generate_unused_shown_name_hints(v, tracking);
    verifier.generate_unnecessary_import_hints(v, tracking);
}

/// An import directive of the unit with its index in `library_imports`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ImportOfUnit {
    node: NodeId,
    index: usize,
}

/// Dart `ImportsVerifier`.
#[derive(Default)]
struct ImportsVerifier {
    /// Dart `_allImports`.
    all_imports: Vec<ImportOfUnit>,
    /// Dart `_unusedImports`.
    unused_imports: IndexSet<NodeId>,
    /// Dart `_duplicateImports`.
    duplicate_imports: Vec<NodeId>,
    /// Dart `_duplicateExports`.
    duplicate_exports: Vec<NodeId>,
    /// Dart `_duplicateHiddenNamesMap`.
    duplicate_hidden_names_map: IndexMap<NodeId, Vec<Id<SimpleIdentifier>>>,
    /// Dart `_duplicateShownNamesMap`.
    duplicate_shown_names_map: IndexMap<NodeId, Vec<Id<SimpleIdentifier>>>,
}

/// Dart `_NamespaceDirective`: a directive with its imported or exported
/// library.
struct NamespaceDirective {
    node: NodeId,
    /// Dart `libraryUriStr`.
    library_uri_str: String,
}

/// The import and export directives of the unit with their libraries
/// (Dart `node.libraryImport?.importedLibrary`,
/// `node.libraryExport?.exportedLibrary`), matched by position.
fn namespace_directives(v: &UnitVerifier<'_>) -> Vec<(NodeId, usize, Option<EId<LibraryElement>>)> {
    let ast = v.ast;
    let fragment = v.ctx.fragment(v.fragment);
    let mut import_index = 0;
    let mut export_index = 0;
    let mut result = Vec::new();
    for &directive in ast.list(ast[v.unit].directives) {
        match ast.kind(directive) {
            NodeKind::ImportDirective => {
                let library = fragment
                    .library_imports
                    .get(import_index)
                    .and_then(|i| directive_library(&i.directive.uri));
                result.push((directive.raw(), import_index, library));
                import_index += 1;
            }
            NodeKind::ExportDirective => {
                let library = fragment
                    .library_exports
                    .get(export_index)
                    .and_then(|e| directive_library(&e.directive.uri));
                result.push((directive.raw(), export_index, library));
                export_index += 1;
            }
            _ => {}
        }
    }
    result
}

impl ImportsVerifier {
    /// Dart `addImports(node)`.
    fn add_imports(&mut self, v: &UnitVerifier<'_>) {
        let ast = v.ast;
        let ctx = v.ctx;
        let mut imports_with_libraries = Vec::new();
        let mut exports_with_libraries = Vec::new();
        for (node, index, library) in namespace_directives(v) {
            if ast.is::<ImportDirective>(node) {
                if let Some(library) = library
                    && !is_origin_not_existing_file(&ctx, library)
                {
                    self.all_imports.push(ImportOfUnit { node, index });
                    imports_with_libraries.push(NamespaceDirective {
                        node,
                        library_uri_str: library_uri(&ctx, library),
                    });
                }
            } else if let Some(library) = library {
                exports_with_libraries.push(NamespaceDirective {
                    node,
                    library_uri_str: library_uri(&ctx, library),
                });
            }
            self.add_duplicate_shown_hidden_names(v, node, library);
        }
        self.duplicate_imports = duplicates(ast, imports_with_libraries);
        self.duplicate_exports = duplicates(ast, exports_with_libraries);
    }

    /// Dart `generateDuplicateExportWarnings`.
    fn generate_duplicate_export_warnings(&self, v: &mut UnitVerifier<'_>) {
        for &node in &self.duplicate_exports {
            let uri = directive_uri_node(v.ast, node);
            let d = v.at(diag::duplicate_export(), uri);
            v.report(d);
        }
    }

    /// Dart `generateDuplicateImportWarnings`.
    fn generate_duplicate_import_warnings(&self, v: &mut UnitVerifier<'_>) {
        for &node in &self.duplicate_imports {
            let uri = directive_uri_node(v.ast, node);
            let d = v.at(diag::duplicate_import(), uri);
            v.report(d);
        }
    }

    /// Dart `generateDuplicateShownHiddenNameWarnings`.
    fn generate_duplicate_shown_hidden_name_warnings(&self, v: &mut UnitVerifier<'_>) {
        for identifiers in self.duplicate_hidden_names_map.values() {
            for &identifier in identifiers {
                let d = v.at(diag::duplicate_hidden_name(), identifier);
                v.report(d);
            }
        }
        for identifiers in self.duplicate_shown_names_map.values() {
            for &identifier in identifiers {
                let d = v.at(diag::duplicate_shown_name(), identifier);
                v.report(d);
            }
        }
    }

    /// Dart `generateUnnecessaryImportHints`: an import is unnecessary if
    /// another import with the same prefix provides a proper superset of
    /// its used elements.
    fn generate_unnecessary_import_hints(
        &self,
        v: &mut UnitVerifier<'_>,
        tracking: &ImportsTracking,
    ) {
        let ctx = v.ctx;
        let fragment = ctx.fragment(v.fragment);
        let used_imports: Vec<ImportOfUnit> = self
            .all_imports
            .iter()
            .copied()
            .filter(|d| !self.unused_imports.contains(&d.node))
            .collect();
        for &first_directive in &used_imports {
            let Some(first_element) = fragment.library_imports.get(first_directive.index) else {
                continue;
            };
            let Some(tracker) = tracker_of(&ctx, v.fragment, first_element, tracking) else {
                continue;
            };
            // Ignore unresolved imports.
            let Some(imported_library) = directive_library(&first_element.directive.uri) else {
                continue;
            };
            // Ignore explicit dart:core import.
            if library_uri(&ctx, imported_library) == "dart:core" {
                continue;
            }
            for &second_directive in &used_imports {
                if second_directive == first_directive {
                    continue;
                }
                let Some(second_element) = fragment.library_imports.get(second_directive.index)
                else {
                    continue;
                };
                // Must be the same import prefix, so the same tracker.
                if tracker_of(&ctx, v.fragment, second_element, tracking) != Some(tracker) {
                    continue;
                }
                let first_set = tracking.elements_of(tracker, first_directive.index);
                let second_set = tracking.elements_of(tracker, second_directive.index);
                // The second must provide all elements of the first.
                if !first_set.iter().all(|e| second_set.contains(e)) {
                    continue;
                }
                // The second must provide strictly more than the first.
                if second_set.len() <= first_set.len() {
                    continue;
                }
                if let (
                    DirectiveUri::Library {
                        relative_uri_string: first_uri,
                        ..
                    },
                    DirectiveUri::Library {
                        relative_uri_string: second_uri,
                        ..
                    },
                ) = (&first_element.directive.uri, &second_element.directive.uri)
                {
                    let uri = directive_uri_node(v.ast, first_directive.node);
                    let d = v.at(diag::unnecessary_import(first_uri, second_uri), uri);
                    v.report(d);
                    // Now that we reported on the first, so we are done.
                    break;
                }
            }
        }
    }

    /// Dart `generateUnusedImportWarnings`.
    fn generate_unused_import_warnings(
        &mut self,
        v: &mut UnitVerifier<'_>,
        tracking: &ImportsTracking,
    ) {
        let ctx = v.ctx;
        let ast = v.ast;
        let fragment = ctx.fragment(v.fragment);
        for (node, index, _) in namespace_directives(v) {
            if !ast.is::<ImportDirective>(node) {
                continue;
            }
            let Some(import_element) = fragment.library_imports.get(index) else {
                continue;
            };
            let Some(key) = tracker_of(&ctx, v.fragment, import_element, tracking) else {
                continue;
            };
            let Some(scope) = tracking.scopes.get(&key) else {
                continue;
            };
            // Ignore the group of imports with a prefix in a comment
            // reference.
            if scope.has_prefix_used_in_comment_reference {
                continue;
            }
            if let DirectiveUri::Library {
                relative_uri_string,
                library,
                ..
            } = &import_element.directive.uri
            {
                // Ignore explicit dart:core import.
                if library_uri(&ctx, *library) == "dart:core" {
                    continue;
                }
                // The URI target does not exist, reported this elsewhere.
                if is_origin_not_existing_file(&ctx, *library) {
                    continue;
                }
                if !scope.import_to_used_elements.contains_key(&index) {
                    self.unused_imports.insert(node);
                    let uri = directive_uri_node(ast, node);
                    let d = v.at(diag::unused_import(relative_uri_string), uri);
                    v.report(d);
                }
            }
        }
    }

    /// Dart `generateUnusedShownNameHints`.
    fn generate_unused_shown_name_hints(
        &self,
        v: &mut UnitVerifier<'_>,
        tracking: &ImportsTracking,
    ) {
        let ctx = v.ctx;
        let ast = v.ast;
        let fragment = ctx.fragment(v.fragment);
        for (node, index, imported_library) in namespace_directives(v) {
            let Some(import_directive) = ast.cast::<ImportDirective>(node) else {
                continue;
            };
            // The whole import is unused, not just one or more shown names
            // from it, so an "unused_import" hint will be generated, making
            // it unnecessary to generate hints for the individual names.
            if self.unused_imports.contains(&node) {
                continue;
            }
            // Ignore unresolved imports.
            let Some(imported_library) = imported_library else {
                continue;
            };
            // Ignore explicit dart:core import.
            if library_uri(&ctx, imported_library) == "dart:core" {
                continue;
            }
            let Some(import_element) = fragment.library_imports.get(index) else {
                continue;
            };
            let import_elements = match tracker_of(&ctx, v.fragment, import_element, tracking) {
                Some(key) => tracking.elements_of(key, index),
                None => IndexSet::new(),
            };
            for &combinator in ast.list(ast[import_directive].combinators) {
                let Some(show) = ast.cast::<ShowCombinator>(combinator) else {
                    continue;
                };
                for &identifier in ast.list(ast[show].shown_names) {
                    let name = ast_ext::identifier_name(ast, identifier);
                    let Some(element) = combinator_name_element(&ctx, imported_library, name)
                    else {
                        continue;
                    };
                    let mut is_used = import_elements.contains(&element);
                    if let Some(property) = element.cast::<dartr_element::PropertyInducingElement>()
                    {
                        let property = ctx.property_inducing(property);
                        is_used = property
                            .getter
                            .is_some_and(|g| import_elements.contains(&g.raw()))
                            || property
                                .setter
                                .is_some_and(|s| import_elements.contains(&s.raw()));
                    }
                    if !is_used {
                        let d = v.at(diag::unused_shown_name(name), identifier);
                        v.report(d);
                    }
                }
            }
        }
    }

    /// Dart `_addDuplicateShownHiddenNames(directive)`.
    fn add_duplicate_shown_hidden_names(
        &mut self,
        v: &UnitVerifier<'_>,
        directive: NodeId,
        library: Option<EId<LibraryElement>>,
    ) {
        let ast = v.ast;
        let ctx = v.ctx;
        let combinators = if let Some(d) = ast.cast::<ImportDirective>(directive) {
            ast[d].combinators
        } else if let Some(d) = ast.cast::<dartr_ast::ExportDirective>(directive) {
            ast[d].combinators
        } else {
            return;
        };
        for &combinator in ast.list(combinators) {
            // Use a set to find duplicates.
            let mut identifiers: IndexSet<ElementId> = IndexSet::new();
            let (names, hide) = if let Some(c) = ast.cast::<HideCombinator>(combinator) {
                (ast[c].hidden_names, true)
            } else if let Some(c) = ast.cast::<ShowCombinator>(combinator) {
                (ast[c].shown_names, false)
            } else {
                continue;
            };
            for &name in ast.list(names) {
                // Dart `name.element` (`ElementResolver._resolveCombinators`).
                let element = library.and_then(|l| {
                    combinator_name_element(&ctx, l, ast_ext::identifier_name(ast, name))
                });
                if let Some(element) = element
                    && !identifiers.insert(element)
                {
                    // [name] is a duplicate.
                    let map = if hide {
                        &mut self.duplicate_hidden_names_map
                    } else {
                        &mut self.duplicate_shown_names_map
                    };
                    map.entry(directive).or_default().push(name);
                }
            }
        }
    }
}

/// Dart `importsTracking.trackerOf(import)`: the import scope of the prefix
/// of [import] in [fragment].
fn tracker_of(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    import: &dartr_element::LibraryImport,
    tracking: &ImportsTracking,
) -> Option<ScopeKey> {
    let prefix = import
        .prefix
        .and_then(|p| ctx.fragment(p).element.try_get().copied())
        .and_then(|e| e.cast::<PrefixElement>());
    let key = (fragment, prefix);
    tracking.scopes.contains_key(&key).then_some(key)
}

/// Dart `ElementResolver._resolveCombinators`: the element of a name of a
/// combinator, in the export namespace of [library] (the variable of an
/// accessor).
fn combinator_name_element(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    name: &str,
) -> Option<ElementId> {
    let namespace = ctx.get(library).export_namespace.try_get()?;
    let element = namespace
        .defined_names
        .get(&ctx.name(name))
        .or_else(|| namespace.defined_names.get(&ctx.name(&format!("{name}="))))
        .copied()?;
    if matches!(element.tag(), Tag::Getter | Tag::Setter) {
        return member::variable(ctx, ElemRef::Base(element)).map(|v| member::base_element(ctx, v));
    }
    Some(element)
}

/// The `uri` of the import or export directive [node].
fn directive_uri_node(ast: &Ast, node: NodeId) -> NodeId {
    if let Some(d) = ast.cast::<ImportDirective>(node) {
        ast[d].uri.raw()
    } else if let Some(d) = ast.cast::<dartr_ast::ExportDirective>(node) {
        ast[d].uri.raw()
    } else {
        node
    }
}

/// Dart `_duplicates(directives)`.
fn duplicates(ast: &Ast, mut directives: Vec<NamespaceDirective>) -> Vec<NodeId> {
    let mut duplicates = Vec::new();
    if directives.len() > 1 {
        // Order the list of directives to find duplicates in faster than
        // O(n^2) time.
        directives.sort_by(|a, b| a.library_uri_str.cmp(&b.library_uri_str));
        for pair in directives.windows(2) {
            let (current, next) = (&pair[0], &pair[1]);
            if current.library_uri_str == next.library_uri_str
                && are_syntactically_identical_except_uri(ast, current.node, next.node)
            {
                // Add either the current or the next directive depending on
                // which comes second, this guarantees that the first of the
                // duplicates won't be highlighted.
                if ast.offset(current.node) < ast.offset(next.node) {
                    duplicates.push(next.node);
                } else {
                    duplicates.push(current.node);
                }
            }
        }
    }
    duplicates
}

/// Dart `ImportDirectiveImpl.areSyntacticallyIdenticalExceptUri`.
fn are_syntactically_identical_except_uri(ast: &Ast, node1: NodeId, node2: NodeId) -> bool {
    if let (Some(i1), Some(i2)) = (
        ast.cast::<ImportDirective>(node1),
        ast.cast::<ImportDirective>(node2),
    ) {
        let name = |p: Option<Id<SimpleIdentifier>>| p.map(|p| ast_ext::identifier_name(ast, p));
        if name(ast[i1].prefix) != name(ast[i2].prefix) {
            return false;
        }
    }
    let combinators = |n: NodeId| {
        if let Some(d) = ast.cast::<ImportDirective>(n) {
            ast.list(ast[d].combinators).to_vec()
        } else if let Some(d) = ast.cast::<dartr_ast::ExportDirective>(n) {
            ast.list(ast[d].combinators).to_vec()
        } else {
            Vec::new()
        }
    };
    let combinators1 = combinators(node1);
    let combinators2 = combinators(node2);
    if combinators1.len() != combinators2.len() {
        return false;
    }
    let are_same_names = |names1: &[Id<SimpleIdentifier>], names2: &[Id<SimpleIdentifier>]| {
        names1.len() == names2.len()
            && names1.iter().zip(names2).all(|(&a, &b)| {
                ast_ext::identifier_name(ast, a) == ast_ext::identifier_name(ast, b)
            })
    };
    for (&c1, &c2) in combinators1.iter().zip(&combinators2) {
        if let (Some(h1), Some(h2)) = (
            ast.cast::<HideCombinator>(c1),
            ast.cast::<HideCombinator>(c2),
        ) {
            if !are_same_names(
                ast.list(ast[h1].hidden_names),
                ast.list(ast[h2].hidden_names),
            ) {
                return false;
            }
        } else if let (Some(s1), Some(s2)) = (
            ast.cast::<ShowCombinator>(c1),
            ast.cast::<ShowCombinator>(c2),
        ) {
            if !are_same_names(ast.list(ast[s1].shown_names), ast.list(ast[s2].shown_names)) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}
