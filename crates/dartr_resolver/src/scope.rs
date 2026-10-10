// Dart source: pkg/analyzer/lib/src/dart/element/scope.dart
// (ScopeLookupResult, LibraryDeclarations, LibraryFragmentScope,
// PrefixScope, EnclosedScope, TypeParameterScope, InstanceScope,
// ExtensionScope, FormalParameterScope, LocalScope,
// ConstructorInitializerScope, PrimaryParameterScope,
// DocumentationCommentScope)
// Dart source: pkg/analyzer/lib/src/dart/resolver/scope.dart (LabelScope,
// BlockScope, UnlabeledBreakContinueContext)

//! The scopes of body analysis.
//!
//! - [`LibraryScopes`]: the scopes of one library that do not change during
//!   analysis (the library declarations, the prefix scopes and the scope of
//!   each library fragment). Built once per library from the linked element
//!   model ([`Ctx`]) and shared (read-only, `Sync`) by the unit resolvers.
//! - [`EnclosedScope`]: the Dart `EnclosedScope` subclasses (type
//!   parameters, formal parameters, local scopes, instance members, ...).
//!   The scope context of the resolution visitor
//!   ([`crate::scope_context::ScopeContext`]) keeps them in a stack on top
//!   of the library fragment scope.
//! - [`NameScope`]: Dart `Scope.lookup` of the current lexical scope.
//! - [`ScopeLookupResult`]: Dart `ScopeLookupResult`.
//! - [`LabelScope`], [`UnlabeledBreakContinueContext`]: the jump targets
//!   (`dart/resolver/scope.dart`).
//!
//! # Multiply defined names
//!
//! Dart `PrefixScope._merge` creates a `MultiplyDefinedElementImpl` when two
//! imports give different elements for the same name. The prefix scopes are
//! built once per library and shared by the unit resolvers, which run in
//! parallel, and each unit resolver writes only into its own local arena.
//! So the scope keeps the conflicting elements (in Dart order), and a lookup
//! creates the `Tag::MultiplyDefined` element in the local arena of the
//! lookup context (`ctx.local`) the first time a unit asks for it. A cache
//! keyed by the local store gives the same element for each later lookup of
//! the same unit (Dart identity within a unit). A lookup without a local
//! arena creates the element in the synthetic store of the generation.
//!
//! # Not ported
//!
//! - Import usage tracking (`ImportsTracking`, `ImportsTrackingOfPrefix`,
//!   `importsTrackingActive`): it is only used by the unused import
//!   diagnostics of the library-wide steps (wave D).
//! - Requirement recording (`globalResultRequirements`): v1 uses
//!   `NoopSink`.
//! - Deprecated exports (`PrefixScopeLookupResult.getterIsFromDeprecatedExport`):
//!   the export namespace of a linked library does not keep the export
//!   locations. A lookup never reports a deprecated export.
//! - Doc imports (`DocumentationCommentScope` with `@docImport`
//!   libraries): the caller gives the doc import libraries
//!   (`LibraryAnalysisInput::doc_import_libraries`); [`LibraryScopes`] keeps
//!   their export entries.

use std::sync::Mutex;

use dartr_ast::NodeId;
use dartr_element::{
    ConstructorElement, Ctx, DirectiveUri, EId, ElemRef, ElementData, ElementId, ExtensionElement,
    FId, FormalParameterFragment, FragmentData, InstanceElement, LibraryElement, LibraryFragment,
    LookupMap, MultiplyDefinedElement, MultiplyDefinedFragment, Name, NamespaceCombinator,
    PrefixElement, StoreId, Tag, TypeParameterElement,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;
use indexmap::{IndexMap, IndexSet};

/// Dart `ScopeLookupResult`: the getter and the setter found for a name.
///
/// A Dart `MultiplyDefinedElementImpl` is an element with
/// `Tag::MultiplyDefined` (see the module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopeLookupResult {
    pub getter: Option<ElementId>,
    pub setter: Option<ElementId>,
}

impl ScopeLookupResult {
    fn is_found(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

/// Dart `Scope.lookup(id)` of a lexical scope.
pub trait NameScope {
    fn lookup(&self, id: &str) -> ScopeLookupResult;
}

/// Whether [flag] is enabled for [library] (Dart
/// `library.featureSet.isEnabled(feature)`).
pub fn library_feature_enabled(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    flag: ExperimentalFlag,
) -> bool {
    ctx.get(library).feature_set.is_enabled(flag.name())
}

/// Dart `Element.lookupName` of the elements that scopes hold: the name,
/// `name=` for a setter (here: the name without `=`, see the callers),
/// `unary-` for the unary minus operator.
fn lookup_name<'a>(ctx: &Ctx<'a>, e: ElementId) -> Option<&'a str> {
    match e.tag() {
        Tag::Dynamic => return Some("dynamic"),
        Tag::Never => return Some("Never"),
        _ => {}
    }
    let name = ctx.name_str(ctx.element_data(e)?.name?);
    if e.tag() == Tag::Method && name == "-" {
        if let dartr_element::AnyElement::Method(m) = ctx.any(e) {
            if m.formal_params.is_empty() {
                return Some("unary-");
            }
        }
    }
    Some(name)
}

/// Dart `LibraryElement.isInSdk`.
pub(crate) fn library_is_in_sdk(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> bool {
    ctx.library_uri(library).starts_with("dart:")
}

// ------------------------------------------------------------------ library scopes

/// An element in a library-level scope: an element, or a multiply defined
/// name (an index in [`LibraryScopes::conflicts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeEntry {
    Element(ElementId),
    Conflict(u32),
}

/// The data of a Dart `MultiplyDefinedElementImpl`.
#[derive(Debug)]
struct Conflict {
    /// `conflictingElements.first.name`.
    name: Name,
    /// Dart `conflictingElements`, in Dart set order.
    elements: Vec<ElementId>,
    /// Dart `libraryFragment`.
    library_fragment: FId<LibraryFragment>,
}

/// Dart `LibraryDeclarations`.
#[derive(Debug, Default)]
struct LibraryDeclarations {
    getters: IndexMap<Box<str>, ElementId>,
    setters: IndexMap<Box<str>, ElementId>,
    /// Dart `extensions`.
    extensions: Vec<EId<ExtensionElement>>,
}

/// Dart `PrefixScope`.
#[derive(Debug, Default)]
struct PrefixScopeData {
    parent: Option<u32>,
    /// Dart `_importedLibraries`.
    imported_libraries: Vec<EId<LibraryElement>>,
    getters: IndexMap<Box<str>, ScopeEntry>,
    setters: IndexMap<Box<str>, ScopeEntry>,
    /// Dart `_extensions`.
    extensions: IndexSet<EId<ExtensionElement>>,
    /// Dart `_deferredLibrary`.
    deferred_library: Option<EId<LibraryElement>>,
}

/// Dart `LibraryFragmentScope`.
#[derive(Debug)]
struct FragmentScopeData {
    parent: Option<FId<LibraryFragment>>,
    /// Dart `noPrefixScope`.
    no_prefix_scope: u32,
    /// Dart `_prefixElements`.
    prefix_elements: IndexMap<Box<str>, EId<PrefixElement>>,
    /// Dart `accessibleExtensions`.
    accessible_extensions: Vec<EId<ExtensionElement>>,
}

/// The library-level scopes of one library (Dart `LibraryFragmentImpl.scope`
/// of each fragment, `PrefixElementImpl.scope` of each prefix).
pub struct LibraryScopes {
    library: Option<EId<LibraryElement>>,
    /// `library.featureSet.isEnabled(Feature.wildcard_variables)`.
    wildcard_variables: bool,
    /// Dart `LibraryElementImpl.libraryDeclarations`.
    declarations: LibraryDeclarations,
    prefix_scopes: Vec<PrefixScopeData>,
    /// Dart `PrefixElementImpl.scope`.
    prefix_scope_of: IndexMap<EId<PrefixElement>, u32>,
    fragment_scopes: IndexMap<FId<LibraryFragment>, FragmentScopeData>,
    conflicts: Vec<Conflict>,
    /// The multiply defined elements created so far, by the store they are
    /// in and the conflict.
    multiply_defined: Mutex<LookupMap<(StoreId, u32), ElementId>>,
    /// The getters and setters of Dart `DocumentationCommentScope` from the
    /// `@docImport` libraries ([`LibraryScopes::set_doc_import_libraries`]).
    doc_import_getters: IndexMap<Box<str>, ElementId>,
    doc_import_setters: IndexMap<Box<str>, ElementId>,
}

impl Default for LibraryScopes {
    fn default() -> Self {
        LibraryScopes {
            library: None,
            wildcard_variables: false,
            declarations: LibraryDeclarations::default(),
            prefix_scopes: Vec::new(),
            prefix_scope_of: IndexMap::new(),
            fragment_scopes: IndexMap::new(),
            conflicts: Vec::new(),
            multiply_defined: Mutex::new(LookupMap::new()),
            doc_import_getters: IndexMap::new(),
            doc_import_setters: IndexMap::new(),
        }
    }
}

impl std::fmt::Debug for LibraryScopes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryScopes")
            .field("library", &self.library)
            .field("fragments", &self.fragment_scopes.len())
            .field("prefix_scopes", &self.prefix_scopes.len())
            .finish()
    }
}

/// The fragments of [library] with the fragment that includes each one
/// (Dart `LibraryFragmentImpl.enclosingFragment`): the defining unit, then
/// the parts, depth first in `part` directive order (so that a fragment
/// comes after the fragment that includes it).
pub(crate) fn library_fragments(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
) -> Vec<(FId<LibraryFragment>, Option<FId<LibraryFragment>>)> {
    fn visit(
        ctx: &Ctx<'_>,
        unit: FId<LibraryFragment>,
        out: &mut Vec<(FId<LibraryFragment>, Option<FId<LibraryFragment>>)>,
    ) {
        for part in &ctx.fragment(unit).parts {
            if let DirectiveUri::Unit {
                library_fragment, ..
            } = &part.directive.uri
            {
                if !out.iter().any(|(f, _)| f == library_fragment) {
                    out.push((*library_fragment, Some(unit)));
                    visit(ctx, *library_fragment, out);
                }
            }
        }
    }
    let first = ctx.get(library).first_fragment();
    let mut out = vec![(first, None)];
    visit(ctx, first, &mut out);
    out
}

/// Dart `Combinator.matches` / `List<Combinator>.allows`.
pub(crate) fn combinators_allow(
    ctx: &Ctx<'_>,
    combinators: &[NamespaceCombinator],
    name: &str,
) -> bool {
    let name = name.strip_suffix('=').unwrap_or(name);
    let matches = |names: &[Name]| names.iter().any(|&n| ctx.name_str(n) == name);
    for c in combinators {
        match c {
            NamespaceCombinator::Show { shown_names, .. } => {
                if !matches(shown_names) {
                    return false;
                }
            }
            NamespaceCombinator::Hide { hidden_names, .. } => {
                if matches(hidden_names) {
                    return false;
                }
            }
        }
    }
    true
}

impl LibraryScopes {
    /// Builds the scopes of [library] from its linked elements.
    pub fn build(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> LibraryScopes {
        let mut scopes = LibraryScopes {
            library: Some(library),
            wildcard_variables: library_feature_enabled(
                ctx,
                library,
                ExperimentalFlag::WildcardVariables,
            ),
            declarations: library_declarations(ctx, library),
            ..LibraryScopes::default()
        };
        for (fragment, parent) in library_fragments(ctx, library) {
            scopes.build_fragment_scope(ctx, library, fragment, parent);
        }
        scopes
    }

    /// Dart `DocumentationCommentScope(innerScope, docImportLibraries)`, the
    /// constructor: the export entries of each doc import library (Dart
    /// ignores the combinators).
    pub fn set_doc_import_libraries(&mut self, ctx: &Ctx<'_>, libraries: &[EId<LibraryElement>]) {
        for &library in libraries {
            let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
                continue;
            };
            for &element in namespace.defined_names.values() {
                let Some(id) = lookup_name(ctx, element) else {
                    continue;
                };
                let map = if element.tag() == Tag::Setter {
                    &mut self.doc_import_setters
                } else {
                    &mut self.doc_import_getters
                };
                map.entry(id.into()).or_insert(element);
            }
        }
    }

    /// Dart `DocumentationCommentScope.lookup`, the part after the inner
    /// scope.
    pub fn doc_import_lookup(&self, id: &str) -> ScopeLookupResult {
        ScopeLookupResult {
            getter: self.doc_import_getters.get(id).copied(),
            setter: self.doc_import_setters.get(id).copied(),
        }
    }

    /// Dart `LibraryFragmentScope(fragment)`.
    fn build_fragment_scope(
        &mut self,
        ctx: &Ctx<'_>,
        library: EId<LibraryElement>,
        fragment: FId<LibraryFragment>,
        parent: Option<FId<LibraryFragment>>,
    ) {
        let f = ctx.fragment(fragment);
        // Dart `fragment.enclosingFragment?.scope`.
        let parent = parent.filter(|p| self.fragment_scopes.contains_key(p));
        let parent_no_prefix = parent.map(|p| self.fragment_scopes[&p].no_prefix_scope);
        let no_prefix_scope =
            self.build_prefix_scope(ctx, library, fragment, None, parent_no_prefix);

        let mut prefix_elements: IndexMap<Box<str>, EId<PrefixElement>> = IndexMap::new();
        for &prefix in &f.library_import_prefixes {
            let name = ctx.get(prefix).name.map(|n| ctx.name_str(n));
            if let Some(name) = name {
                prefix_elements.insert(name.into(), prefix);
            }
            // Dart `_getParentPrefixScope`.
            let is_deferred = f.library_imports.iter().any(|import| {
                import.prefix.is_some_and(|pf| {
                    let p = ctx.fragment(pf);
                    p.element.try_get() == Some(&prefix.raw()) && p.is_deferred
                })
            });
            let mut parent_scope = None;
            if !is_deferred {
                let mut scope = parent;
                while let Some(s) = scope {
                    let data = &self.fragment_scopes[&s];
                    if let Some(parent_prefix) = name.and_then(|n| data.prefix_elements.get(n)) {
                        parent_scope = self.prefix_scope_of.get(parent_prefix).copied();
                        break;
                    }
                    scope = data.parent;
                }
            }
            let scope = self.build_prefix_scope(ctx, library, fragment, Some(prefix), parent_scope);
            self.prefix_scope_of.insert(prefix, scope);
        }

        // Dart `accessibleExtensions`.
        let mut extensions: IndexSet<EId<ExtensionElement>> = IndexSet::new();
        extensions.extend(self.declarations.extensions.iter().copied());
        extensions.extend(
            self.prefix_scopes[no_prefix_scope as usize]
                .extensions
                .iter()
                .copied(),
        );
        for prefix in prefix_elements.values() {
            if let Some(&s) = self.prefix_scope_of.get(prefix) {
                extensions.extend(self.prefix_scopes[s as usize].extensions.iter().copied());
            }
        }
        if let Some(p) = parent {
            extensions.extend(
                self.fragment_scopes[&p]
                    .accessible_extensions
                    .iter()
                    .copied(),
            );
        }

        self.fragment_scopes.insert(
            fragment,
            FragmentScopeData {
                parent,
                no_prefix_scope,
                prefix_elements,
                accessible_extensions: extensions.into_iter().collect(),
            },
        );
    }

    /// Dart `PrefixScope(libraryFragment:, parent:, libraryImports:,
    /// prefix:)`.
    fn build_prefix_scope(
        &mut self,
        ctx: &Ctx<'_>,
        library: EId<LibraryElement>,
        fragment: FId<LibraryFragment>,
        prefix: Option<EId<PrefixElement>>,
        parent: Option<u32>,
    ) -> u32 {
        let records_enabled = library_feature_enabled(ctx, library, ExperimentalFlag::Records);
        let mut scope = PrefixScopeData {
            parent,
            ..PrefixScopeData::default()
        };
        for import in &ctx.fragment(fragment).library_imports {
            let DirectiveUri::Library {
                library: imported_library,
                ..
            } = &import.directive.uri
            else {
                continue;
            };
            let import_prefix = import
                .prefix
                .and_then(|p| ctx.fragment(p).element.try_get().copied());
            if import_prefix != prefix.map(|p| p.raw()) {
                continue;
            }
            let imported_library = *imported_library;
            scope.imported_libraries.push(imported_library);
            if let Some(namespace) = ctx.get(imported_library).export_namespace.try_get() {
                for (&name, &element) in &namespace.defined_names {
                    let name = ctx.name_str(name);
                    if !combinators_allow(ctx, &import.combinators, name) {
                        continue;
                    }
                    // Dart `_shouldAdd`.
                    if !records_enabled
                        && library_is_in_sdk(ctx, imported_library)
                        && element.tag() == Tag::Class
                        && ctx.is_element(element, "dart.core", "Record")
                    {
                        continue;
                    }
                    self.add_to_prefix_scope(ctx, fragment, &mut scope, name, element);
                }
            }
            if let Some(pf) = import.prefix {
                if ctx.fragment(pf).is_deferred && scope.deferred_library.is_none() {
                    scope.deferred_library = Some(imported_library);
                }
            }
        }
        self.prefix_scopes.push(scope);
        (self.prefix_scopes.len() - 1) as u32
    }

    /// Dart `PrefixScope._add` / `_addTo`.
    fn add_to_prefix_scope(
        &mut self,
        ctx: &Ctx<'_>,
        fragment: FId<LibraryFragment>,
        scope: &mut PrefixScopeData,
        name: &str,
        element: ElementId,
    ) {
        let is_setter = element.tag() == Tag::Setter;
        // Dart `element.displayName`: the name without `=`.
        let id = name.strip_suffix('=').unwrap_or(name);
        if !is_setter {
            if let Some(e) = element.cast::<ExtensionElement>() {
                scope.extensions.insert(e);
            }
        }
        let map = if is_setter {
            &mut scope.setters
        } else {
            &mut scope.getters
        };
        let entry = ScopeEntry::Element(element);
        match map.get(id).copied() {
            None => {
                map.insert(id.into(), entry);
            }
            Some(existing) if existing == entry => {}
            Some(existing) => {
                let merged = self.merge(ctx, fragment, existing, element);
                let map = if is_setter {
                    &mut scope.setters
                } else {
                    &mut scope.getters
                };
                map.insert(id.into(), merged);
            }
        }
    }

    /// Dart `PrefixScope._merge`.
    fn merge(
        &mut self,
        ctx: &Ctx<'_>,
        fragment: FId<LibraryFragment>,
        existing: ScopeEntry,
        other: ElementId,
    ) -> ScopeEntry {
        let other_entry = ScopeEntry::Element(other);
        if self.is_sdk_element(ctx, existing) {
            if !self.is_sdk_element(ctx, other_entry) {
                return other_entry;
            }
        } else if self.is_sdk_element(ctx, other_entry) {
            return existing;
        }

        // Dart `_addElement` into an insertion ordered set.
        let mut conflicting: IndexSet<ElementId> = IndexSet::new();
        match existing {
            ScopeEntry::Conflict(c) => {
                conflicting.extend(self.conflicts[c as usize].elements.iter().copied())
            }
            ScopeEntry::Element(e) => {
                conflicting.insert(e);
            }
        }
        conflicting.insert(other);
        let first = conflicting[0];
        let name = ctx
            .element_data(first)
            .and_then(|d| d.name)
            .unwrap_or_else(|| ctx.name(lookup_name(ctx, first).unwrap_or("")));
        self.conflicts.push(Conflict {
            name,
            elements: conflicting.into_iter().collect(),
            library_fragment: fragment,
        });
        ScopeEntry::Conflict((self.conflicts.len() - 1) as u32)
    }

    /// Dart `PrefixScope._isSdkElement`.
    fn is_sdk_element(&self, ctx: &Ctx<'_>, entry: ScopeEntry) -> bool {
        match entry {
            ScopeEntry::Conflict(_) => false,
            ScopeEntry::Element(e) => {
                if matches!(e.tag(), Tag::Dynamic | Tag::Never) {
                    return true;
                }
                ctx.element_data(e)
                    .and_then(|d| d.library)
                    .is_some_and(|l| library_is_in_sdk(ctx, l))
            }
        }
    }

    /// The element of [entry]; creates the multiply defined element of a
    /// conflict (see the module documentation).
    fn entry_element(&self, ctx: &Ctx<'_>, entry: ScopeEntry) -> ElementId {
        match entry {
            ScopeEntry::Element(e) => e,
            ScopeEntry::Conflict(c) => self.multiply_defined_element(ctx, c),
        }
    }

    fn multiply_defined_element(&self, ctx: &Ctx<'_>, conflict: u32) -> ElementId {
        let store = match ctx.local {
            Some(local) => &local.store,
            None => &ctx.generation().synthetic,
        };
        let key = (store.id, conflict);
        let mut cache = self
            .multiply_defined
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(&e) = cache.get(&key) {
            return e;
        }
        let c = &self.conflicts[conflict as usize];
        let fragment = store.add_fragment::<MultiplyDefinedFragment>(MultiplyDefinedFragment {
            fragment: FragmentData::new(Some(c.name), None),
        });
        let mut data = ElementData::new(Some(c.name), fragment.raw());
        data.library = self.library;
        let element = store.add::<MultiplyDefinedElement>(MultiplyDefinedElement {
            element: data,
            library_fragment: c.library_fragment,
            conflicting_elements: c.elements.clone(),
        });
        store.fragment(fragment).element.set_once(element.raw());
        cache.insert(key, element.raw());
        element.raw()
    }

    fn result(
        &self,
        ctx: &Ctx<'_>,
        getter: Option<ScopeEntry>,
        setter: Option<ScopeEntry>,
    ) -> ScopeLookupResult {
        ScopeLookupResult {
            getter: getter.map(|e| self.entry_element(ctx, e)),
            setter: setter.map(|e| self.entry_element(ctx, e)),
        }
    }

    /// Dart `PrefixScope.lookup` of the prefix scope [scope].
    fn prefix_scope_lookup(&self, ctx: &Ctx<'_>, scope: u32, id: &str) -> ScopeLookupResult {
        let mut current = Some(scope);
        while let Some(s) = current {
            let data = &self.prefix_scopes[s as usize];
            if let Some(deferred) = data.deferred_library {
                if id == "loadLibrary" {
                    if let Some(&f) = ctx.get(deferred).load_library_function.try_get() {
                        return ScopeLookupResult {
                            getter: Some(f.raw()),
                            setter: None,
                        };
                    }
                }
            }
            let getter = data.getters.get(id).copied();
            let setter = data.setters.get(id).copied();
            if getter.is_some() || setter.is_some() {
                return self.result(ctx, getter, setter);
            }
            current = data.parent;
        }
        ScopeLookupResult::default()
    }

    /// Dart `LibraryFragmentImpl.scope.lookup(id)`.
    pub fn fragment_lookup(
        &self,
        ctx: &Ctx<'_>,
        fragment: FId<LibraryFragment>,
        id: &str,
    ) -> ScopeLookupResult {
        // Dart `_lookupLibrary`.
        let getter = self.declarations.getters.get(id).copied();
        let setter = self.declarations.setters.get(id).copied();
        if getter.is_some() || setter.is_some() {
            return ScopeLookupResult { getter, setter };
        }
        self.lookup_combined(ctx, fragment, id).unwrap_or_default()
    }

    /// Dart `LibraryFragmentScope._lookupCombined`.
    fn lookup_combined(
        &self,
        ctx: &Ctx<'_>,
        fragment: FId<LibraryFragment>,
        id: &str,
    ) -> Option<ScopeLookupResult> {
        let scope = self.fragment_scopes.get(&fragment)?;
        // Dart `_shouldTryPrefixElement`.
        let try_prefix = id != "_" || !self.wildcard_variables;
        if try_prefix {
            if let Some(&prefix) = scope.prefix_elements.get(id) {
                return Some(ScopeLookupResult {
                    getter: Some(prefix.raw()),
                    setter: None,
                });
            }
        }
        let result = self.prefix_scope_lookup(ctx, scope.no_prefix_scope, id);
        if result.is_found() {
            return Some(result);
        }
        scope.parent.and_then(|p| self.lookup_combined(ctx, p, id))
    }

    /// Dart `PrefixElementImpl.scope.lookup(id)`.
    pub fn prefix_lookup(
        &self,
        ctx: &Ctx<'_>,
        prefix: EId<PrefixElement>,
        id: &str,
    ) -> ScopeLookupResult {
        match self.prefix_scope_of.get(&prefix) {
            Some(&scope) => self.prefix_scope_lookup(ctx, scope, id),
            None => ScopeLookupResult::default(),
        }
    }

    /// Dart `LibraryFragmentScope.accessibleExtensions`.
    pub fn accessible_extensions(
        &self,
        fragment: FId<LibraryFragment>,
    ) -> &[EId<ExtensionElement>] {
        self.fragment_scopes
            .get(&fragment)
            .map(|s| s.accessible_extensions.as_slice())
            .unwrap_or(&[])
    }

    /// Dart `LibraryFragmentScope.importedLibrariesContributingExtensions`.
    pub fn imported_libraries_contributing_extensions(
        &self,
        fragment: FId<LibraryFragment>,
    ) -> Vec<EId<LibraryElement>> {
        let mut result: IndexSet<EId<LibraryElement>> = IndexSet::new();
        let mut current = Some(fragment);
        while let Some(f) = current {
            let Some(scope) = self.fragment_scopes.get(&f) else {
                break;
            };
            result.extend(
                self.prefix_scopes[scope.no_prefix_scope as usize]
                    .imported_libraries
                    .iter()
                    .copied(),
            );
            for prefix in scope.prefix_elements.values() {
                if let Some(&s) = self.prefix_scope_of.get(prefix) {
                    result.extend(
                        self.prefix_scopes[s as usize]
                            .imported_libraries
                            .iter()
                            .copied(),
                    );
                }
            }
            current = scope.parent;
        }
        result.into_iter().collect()
    }

    /// Dart `LibraryFragmentImpl.enclosingFragment` of a fragment of the
    /// library: the fragment with the `part` directive that includes it.
    pub fn enclosing_fragment(
        &self,
        fragment: FId<LibraryFragment>,
    ) -> Option<FId<LibraryFragment>> {
        self.fragment_scopes.get(&fragment)?.parent
    }

    /// Dart `LibraryDeclarations.withName(name)`.
    pub fn library_declaration_with_name(&self, name: &str) -> Option<ElementId> {
        self.declarations
            .getters
            .get(name)
            .or_else(|| self.declarations.setters.get(name))
            .copied()
    }
}

/// Dart `LibraryDeclarations(library)`.
fn library_declarations(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> LibraryDeclarations {
    let l = ctx.get(library);
    let mut d = LibraryDeclarations::default();
    fn add_getter(ctx: &Ctx<'_>, d: &mut LibraryDeclarations, e: ElementId) {
        if let Some(name) = lookup_name(ctx, e) {
            d.getters.entry(name.into()).or_insert(e);
        }
    }
    for &e in &l.getters {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.enums {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.extensions {
        add_getter(ctx, &mut d, e.raw());
        if !d.extensions.contains(&e) {
            d.extensions.push(e);
        }
    }
    for &e in &l.extension_types {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.setters {
        if let Some(name) = lookup_name(ctx, e.raw()) {
            d.setters.entry(name.into()).or_insert(e.raw());
        }
    }
    for &e in &l.top_level_functions {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.type_aliases {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.mixins {
        add_getter(ctx, &mut d, e.raw());
    }
    for &e in &l.classes {
        add_getter(ctx, &mut d, e.raw());
    }
    if ctx.library_uri(library) == "dart:core" {
        add_getter(ctx, &mut d, ElementId::DYNAMIC);
        add_getter(ctx, &mut d, ElementId::NEVER);
    }
    d
}

// ------------------------------------------------------------------ enclosed scopes

/// The kind of an [`EnclosedScope`] (the Dart class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnclosedScopeKind {
    /// Dart `TypeParameterScope`.
    TypeParameter,
    /// Dart `FormalParameterScope`.
    FormalParameter,
    /// Dart `LocalScope`.
    Local,
    /// Dart `InstanceScope`: an instance member hides the enclosing scopes
    /// and gives an empty result.
    Instance,
    /// Dart `ExtensionScope`.
    Extension,
    /// Dart `ConstructorInitializerScope`.
    ConstructorInitializer,
    /// Dart `PrimaryParameterScope`.
    PrimaryParameter,
    /// Dart `DocumentationCommentScope`: the enclosing scope first, then
    /// the doc imports.
    DocumentationComment,
}

/// A Dart `EnclosedScope` subclass (or `DocumentationCommentScope`): the
/// names that one scope declares. The enclosing scope is the scope below it
/// in the stack of [`crate::scope_context::ScopeContext`].
#[derive(Debug)]
pub struct EnclosedScope<'a> {
    pub kind: EnclosedScopeKind,
    getters: IndexMap<&'a str, ElementId>,
    setters: IndexMap<&'a str, ElementId>,
    /// `featureSet.isEnabled(Feature.wildcard_variables)` (local scopes).
    wildcard_variables: bool,
}

/// What a scope gives for a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnclosedLookup {
    /// The scope has the name: the result.
    Found(ScopeLookupResult),
    /// Ask the enclosing scope.
    NotFound,
}

impl<'a> EnclosedScope<'a> {
    fn new(kind: EnclosedScopeKind, wildcard_variables: bool) -> EnclosedScope<'a> {
        EnclosedScope {
            kind,
            getters: IndexMap::new(),
            setters: IndexMap::new(),
            wildcard_variables,
        }
    }

    /// Dart `_GettersAndSetters._addGetter`.
    fn add_getter(&mut self, ctx: &Ctx<'a>, element: ElementId) {
        if let Some(id) = lookup_name(ctx, element) {
            self.getters.entry(id).or_insert(element);
        }
    }

    /// Dart `_GettersAndSetters._addSetter` (the lookup name of a setter is
    /// `name=`; the id is the name).
    fn add_setter(&mut self, ctx: &Ctx<'a>, element: ElementId) {
        if element.tag() != Tag::Setter {
            return;
        }
        if let Some(id) = lookup_name(ctx, element) {
            self.setters.entry(id).or_insert(element);
        }
    }

    /// Dart `TypeParameterScope(parent, elements, featureSet:)`.
    pub fn type_parameter_scope(
        ctx: &Ctx<'a>,
        elements: &[EId<TypeParameterElement>],
        wildcard_variables: bool,
    ) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(EnclosedScopeKind::TypeParameter, wildcard_variables);
        for &e in elements {
            if wildcard_variables && lookup_name(ctx, e.raw()) == Some("_") {
                continue;
            }
            scope.add_getter(ctx, e.raw());
        }
        scope
    }

    /// Dart `FormalParameterScope(parent, elements, featureSet:)`.
    pub fn formal_parameter_scope(
        ctx: &Ctx<'a>,
        elements: &[EId<dartr_element::FormalParameterElement>],
        wildcard_variables: bool,
    ) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(EnclosedScopeKind::FormalParameter, wildcard_variables);
        for &e in elements {
            if matches!(
                e.raw().tag(),
                Tag::FieldFormalParameter | Tag::SuperFormalParameter
            ) {
                continue;
            }
            if wildcard_variables && lookup_name(ctx, e.raw()) == Some("_") {
                continue;
            }
            scope.add_getter(ctx, e.raw());
        }
        scope
    }

    /// Dart `LocalScope(parent, featureSet:)`.
    pub fn local_scope(wildcard_variables: bool) -> EnclosedScope<'a> {
        EnclosedScope::new(EnclosedScopeKind::Local, wildcard_variables)
    }

    /// Dart `LocalScope.add(element)`.
    pub fn add(&mut self, ctx: &Ctx<'a>, element: ElementId) {
        if lookup_name(ctx, element) == Some("_") {
            let is_pattern_variable = matches!(
                element.tag(),
                Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
            );
            if is_pattern_variable || self.wildcard_variables {
                return;
            }
        }
        self.add_getter(ctx, element);
    }

    /// Dart `InstanceScope(parent, element)`.
    pub fn instance_scope(ctx: &Ctx<'a>, element: EId<InstanceElement>) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(EnclosedScopeKind::Instance, false);
        let data = ctx.instance(element);
        for &e in &data.getters {
            scope.add_getter(ctx, e.raw());
        }
        for &e in &data.setters {
            scope.add_setter(ctx, e.raw());
        }
        for &e in &data.methods {
            scope.add_getter(ctx, e.raw());
        }
        scope
    }

    /// Dart `ExtensionScope(parent, element)`.
    pub fn extension_scope(ctx: &Ctx<'a>, element: EId<ExtensionElement>) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(EnclosedScopeKind::Extension, false);
        let data = ctx.instance(element.upcast());
        for &e in &data.getters {
            scope.add_getter(ctx, e.raw());
        }
        for &e in &data.setters {
            scope.add_setter(ctx, e.raw());
        }
        for &e in &data.methods {
            scope.add_getter(ctx, e.raw());
        }
        scope
    }

    /// Dart `ConstructorInitializerScope(parent, element)`.
    pub fn constructor_initializer_scope(
        ctx: &Ctx<'a>,
        element: EId<ConstructorElement>,
        wildcard_variables: bool,
    ) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(
            EnclosedScopeKind::ConstructorInitializer,
            wildcard_variables,
        );
        for &p in &ctx.get(element).formal_params {
            if wildcard_variables && lookup_name(ctx, p.raw()) == Some("_") {
                continue;
            }
            // A private named field formal parameter is referred to by its
            // private name in the initializer list.
            if p.raw().tag() == Tag::FieldFormalParameter {
                let first: FId<FormalParameterFragment> = ctx.get(p).first_fragment();
                if let Some(private_name) = ctx.fragment(first).private_name {
                    scope
                        .getters
                        .entry(ctx.name_str(private_name))
                        .or_insert(p.raw());
                    continue;
                }
            }
            scope.add_getter(ctx, p.raw());
        }
        scope
    }

    /// Dart `PrimaryParameterScope(parent, element)`.
    pub fn primary_parameter_scope(
        ctx: &Ctx<'a>,
        element: EId<ConstructorElement>,
        wildcard_variables: bool,
    ) -> EnclosedScope<'a> {
        let mut scope = EnclosedScope::new(EnclosedScopeKind::PrimaryParameter, wildcard_variables);
        for &p in &ctx.get(element).formal_params {
            if wildcard_variables && lookup_name(ctx, p.raw()) == Some("_") {
                continue;
            }
            if matches!(
                p.raw().tag(),
                Tag::FieldFormalParameter | Tag::SuperFormalParameter
            ) {
                continue;
            }
            scope.add_getter(ctx, p.raw());
        }
        scope
    }

    /// Dart `DocumentationCommentScope(innerScope, docImportLibraries)`
    /// without doc imports (see the module documentation).
    pub fn documentation_comment_scope() -> EnclosedScope<'a> {
        EnclosedScope::new(EnclosedScopeKind::DocumentationComment, false)
    }

    /// The lookup of this scope alone: Dart `EnclosedScope.lookup` (and
    /// `InstanceScope.lookup`) without the call to the parent.
    pub fn lookup_here(&self, ctx: &Ctx<'_>, id: &str) -> EnclosedLookup {
        let getter = self.getters.get(id).copied();
        let setter = self.setters.get(id).copied();
        if getter.is_none() && setter.is_none() {
            return EnclosedLookup::NotFound;
        }
        if self.kind == EnclosedScopeKind::Instance {
            // Dart `InstanceScope._isStatic`.
            let is_static = |e: Option<ElementId>| {
                e.is_some_and(|e| {
                    matches!(e.tag(), Tag::Getter | Tag::Setter | Tag::Method)
                        && dartr_typesystem::member::is_static(ctx, ElemRef::Base(e))
                })
            };
            if is_static(getter) || is_static(setter) {
                return EnclosedLookup::Found(ScopeLookupResult { getter, setter });
            }
            return EnclosedLookup::Found(ScopeLookupResult::default());
        }
        EnclosedLookup::Found(ScopeLookupResult { getter, setter })
    }
}

// ------------------------------------------------------------------ resolver/scope.dart

/// Dart `BlockScope.elementsInStatements` is in the resolution visitor
/// (`_defineLocalElements`); this is Dart `LabelScope`: a scope in which a
/// single label is defined, as a linked list in a vector.
#[derive(Clone, Copy, Debug)]
pub struct LabelScope {
    /// Dart `_outerScope` (an index in [`LabelScopes`]).
    pub outer: Option<usize>,
    /// Dart `element`.
    pub element: ElementId,
    /// Dart `node`: the target of a jump.
    pub node: NodeId,
}

/// The label scopes of one unit (the Dart objects are linked by
/// `_outerScope`).
#[derive(Debug, Default)]
pub struct LabelScopes {
    pub scopes: Vec<LabelScope>,
}

impl LabelScopes {
    /// Dart `LabelScope(outer, element, node)`; returns its index.
    pub fn push(&mut self, outer: Option<usize>, element: ElementId, node: NodeId) -> usize {
        self.scopes.push(LabelScope {
            outer,
            element,
            node,
        });
        self.scopes.len() - 1
    }

    /// Dart `LabelScope.lookup(targetLabel)`.
    pub fn lookup(
        &self,
        ctx: &Ctx<'_>,
        scope: Option<usize>,
        target_label: &str,
    ) -> Option<LabelScope> {
        let mut current = scope;
        while let Some(i) = current {
            let s = self.scopes[i];
            let name = ctx.element_data(s.element).and_then(|d| d.name);
            if name.is_some_and(|n| ctx.name_str(n) == target_label) {
                return Some(s);
            }
            current = s.outer;
        }
        None
    }
}

/// Dart `UnlabeledBreakContinueContext`: the targets of unlabeled `break`
/// and `continue` statements, as a stack of statements (the root context is
/// the empty stack).
#[derive(Clone, Debug, Default)]
pub struct UnlabeledBreakContinueContext {
    /// Dart `statement` of each nested context, outermost first.
    statements: Vec<NodeId>,
}

impl UnlabeledBreakContinueContext {
    /// Dart `UnlabeledBreakContinueContext.root`.
    pub fn root() -> Self {
        Self::default()
    }

    /// Dart `nest(statement)`.
    pub fn nest(&self, statement: NodeId) -> Self {
        let mut statements = self.statements.clone();
        statements.push(statement);
        UnlabeledBreakContinueContext { statements }
    }

    /// Dart `breakTarget`.
    pub fn break_target(&self) -> Option<NodeId> {
        self.statements.last().copied()
    }

    /// Dart `continueTarget`: the innermost statement that is not a
    /// `switch` statement.
    pub fn continue_target(&self, ast: &dartr_ast::Ast) -> Option<NodeId> {
        self.statements
            .iter()
            .rev()
            .copied()
            .find(|&s| !ast.is::<dartr_ast::SwitchStatement>(s))
    }
}
