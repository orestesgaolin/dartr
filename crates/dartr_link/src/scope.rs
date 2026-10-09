// Dart source: pkg/analyzer/lib/src/dart/element/scope.dart
// (LibraryDeclarations, LibraryFragmentScope, PrefixScope, InstanceScope,
// ExtensionScope, TypeParameterScope, EnclosedScope)

//! The scopes that linking needs to resolve type names: the scope of a
//! library fragment (the library declarations, the import prefixes and the
//! imported names of the fragment and its enclosing fragments), and the
//! enclosed scopes of type parameters and of instance members.
//!
//! Differences: a `MultiplyDefinedElementImpl` is not created; a lookup
//! returns [`ScopeElement::MultiplyDefined`] with the conflicting elements.
//! Import tracking and requirement recording are not ported (they are used
//! by analysis, not by linking).

use std::sync::Arc;

use dartr_element::*;
use indexmap::IndexMap;

use crate::export::{ExportEntry, combinators_allow};
use crate::link::Linker;

/// An element found by a scope lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeElement {
    Element(ElementId),
    /// An import prefix and its prefix scope (index in [`LibraryScopes`]).
    Prefix(EId<PrefixElement>, usize),
    /// Dart `MultiplyDefinedElementImpl` (the conflicting elements).
    MultiplyDefined(Vec<ElementId>),
}

impl ScopeElement {
    pub fn element(&self) -> Option<ElementId> {
        match self {
            ScopeElement::Element(e) => Some(*e),
            _ => None,
        }
    }
}

/// Dart `ScopeLookupResult`.
#[derive(Clone, Debug, Default)]
pub struct ScopeLookupResult {
    pub getter: Option<ScopeElement>,
    pub setter: Option<ScopeElement>,
}

/// Dart `LibraryDeclarations` (names to elements, first one wins).
#[derive(Clone, Debug, Default)]
pub struct LibraryDeclarations {
    pub getters: IndexMap<Arc<str>, ElementId>,
    pub setters: IndexMap<Arc<str>, ElementId>,
}

/// Dart `PrefixScope`.
#[derive(Clone, Debug, Default)]
pub struct PrefixScope {
    pub parent: Option<usize>,
    pub getters: IndexMap<Arc<str>, ScopeElement>,
    pub setters: IndexMap<Arc<str>, ScopeElement>,
}

/// Dart `LibraryFragmentScope`.
#[derive(Clone, Debug)]
pub struct FragmentScope {
    pub library: usize,
    pub parent: Option<FId<LibraryFragment>>,
    pub no_prefix_scope: usize,
    /// Dart `_prefixElements`: name to prefix and its scope.
    pub prefix_elements: IndexMap<Arc<str>, (EId<PrefixElement>, usize)>,
    pub wildcard_variables: bool,
}

/// The scopes of the libraries of a cycle.
#[derive(Debug, Default)]
pub struct LibraryScopes {
    /// Per library builder.
    pub declarations: Vec<LibraryDeclarations>,
    pub prefix_scopes: Vec<PrefixScope>,
    pub fragment_scopes: IndexMap<FId<LibraryFragment>, FragmentScope>,
}

/// The store of an element: the cycle being linked or a frozen cycle.
pub fn store_of<'a>(linker: &'a Linker<'_>, id: StoreId) -> &'a ElementStore {
    if id == linker.core.store.id {
        &linker.core.store
    } else {
        linker.core.world.store(id).expect("store")
    }
}

/// Dart `Element.lookupName` of the elements that scopes hold.
pub fn lookup_name(linker: &Linker<'_>, e: ElementId) -> Option<String> {
    if e == ElementId::DYNAMIC {
        return Some("dynamic".into());
    }
    if e == ElementId::NEVER {
        return Some("Never".into());
    }
    let store = store_of(linker, e.store());
    let name = store.element_data(e)?.name?;
    let text = linker.core.name_str(name);
    Some(if e.tag() == Tag::Setter {
        format!("{text}=")
    } else {
        text.to_string()
    })
}

/// Whether the library of [e] is an SDK library (Dart `_isSdkElement`).
fn is_sdk_element(linker: &Linker<'_>, e: &ScopeElement) -> bool {
    match e {
        ScopeElement::Element(id) if *id == ElementId::DYNAMIC || *id == ElementId::NEVER => true,
        ScopeElement::Element(id) => {
            let store = store_of(linker, id.store());
            let Some(library) = store.element_data(*id).and_then(|d| d.library) else {
                return false;
            };
            let lib_store = store_of(linker, library.store());
            lib_store
                .fragment(lib_store.get(library).first_fragment())
                .source
                .uri
                .starts_with("dart:")
        }
        _ => false,
    }
}

/// The export entries of the library [library] (this cycle or a linked
/// cycle).
pub fn export_entries(linker: &Linker<'_>, library: EId<LibraryElement>) -> Vec<ExportEntry> {
    let uri = crate::library_builder::library_uri(linker, library);
    if let Some(&b) = linker.builder_by_uri.get(uri.as_ref()) {
        return linker.builders[b].export_scope.to_export_entries();
    }
    linker
        .core
        .deps
        .library(&uri)
        .map(|l| l.export_entries.clone())
        .unwrap_or_default()
}

impl LibraryScopes {
    /// Builds the declarations and the fragment scopes of every library of
    /// the cycle.
    pub fn build(linker: &Linker<'_>) -> LibraryScopes {
        let mut scopes = LibraryScopes::default();
        for (index, builder) in linker.builders.iter().enumerate() {
            scopes
                .declarations
                .push(library_declarations(linker, builder.element, &builder.uri));
            for unit in &builder.units {
                scopes.build_fragment_scope(linker, index, unit.fragment);
            }
        }
        scopes
    }

    fn build_fragment_scope(
        &mut self,
        linker: &Linker<'_>,
        library: usize,
        fragment: FId<LibraryFragment>,
    ) {
        let store = &linker.core.store;
        let f = store.fragment(fragment);
        // Dart `fragment.enclosingFragment?.scope`: the unit that has the
        // `part` directive.
        let parent = enclosing_unit(linker, library, fragment);
        let parent_no_prefix = parent
            .and_then(|p| self.fragment_scopes.get(&p))
            .map(|s| s.no_prefix_scope);
        let no_prefix = self.prefix_scope(linker, &f.library_imports, None, parent_no_prefix);
        let wildcard_variables = linker.builders[library]
            .is_enabled(dartr_parser::experimental_flags::ExperimentalFlag::WildcardVariables);
        let mut prefix_elements = IndexMap::new();
        for &prefix in &f.library_import_prefixes {
            let p = store.get(prefix);
            let is_deferred = f.library_imports.iter().any(|i| {
                i.prefix.is_some_and(|pf| {
                    store.fragment(pf).element.try_get() == Some(&prefix.raw())
                        && store.fragment(pf).is_deferred
                })
            });
            let name = p.name.map(|n| Arc::<str>::from(linker.core.name_str(n)));
            // Dart `_getParentPrefixScope`.
            let mut parent_scope = None;
            if !is_deferred && let Some(name) = &name {
                let mut scope = parent;
                while let Some(s) = scope {
                    let fs = &self.fragment_scopes[&s];
                    if let Some(&(_, ps)) = fs.prefix_elements.get(name) {
                        parent_scope = Some(ps);
                        break;
                    }
                    scope = fs.parent;
                }
            }
            let scope = self.prefix_scope(linker, &f.library_imports, Some(prefix), parent_scope);
            if let Some(name) = name {
                prefix_elements.insert(name, (prefix, scope));
            }
        }
        self.fragment_scopes.insert(
            fragment,
            FragmentScope {
                library,
                parent,
                no_prefix_scope: no_prefix,
                prefix_elements,
                wildcard_variables,
            },
        );
    }

    /// Dart `PrefixScope(...)`.
    fn prefix_scope(
        &mut self,
        linker: &Linker<'_>,
        imports: &[LibraryImport],
        prefix: Option<EId<PrefixElement>>,
        parent: Option<usize>,
    ) -> usize {
        let store = &linker.core.store;
        let mut scope = PrefixScope {
            parent,
            ..Default::default()
        };
        for import in imports {
            let DirectiveUri::Library { library, .. } = &import.directive.uri else {
                continue;
            };
            let import_prefix = import
                .prefix
                .and_then(|p| store.fragment(p).element.try_get().copied());
            if import_prefix != prefix.map(|p| p.raw()) {
                continue;
            }
            let combinators = crate::library_builder::combinators_of(linker, &import.combinators);
            for entry in export_entries(linker, *library) {
                if combinators_allow(&combinators, &entry.name) {
                    add_to_prefix_scope(linker, &mut scope, &entry);
                }
            }
        }
        self.prefix_scopes.push(scope);
        self.prefix_scopes.len() - 1
    }

    /// Dart `PrefixScope.lookup`.
    pub fn prefix_lookup(&self, scope: usize, id: &str) -> ScopeLookupResult {
        let mut current = Some(scope);
        while let Some(s) = current {
            let p = &self.prefix_scopes[s];
            let getter = p.getters.get(id).cloned();
            let setter = p.setters.get(id).cloned();
            if getter.is_some() || setter.is_some() {
                return ScopeLookupResult { getter, setter };
            }
            current = p.parent;
        }
        ScopeLookupResult::default()
    }

    /// Dart `LibraryFragmentScope.lookup`.
    pub fn lookup(&self, fragment: FId<LibraryFragment>, id: &str) -> ScopeLookupResult {
        let scope = &self.fragment_scopes[&fragment];
        let declarations = &self.declarations[scope.library];
        let getter = declarations.getters.get(id).copied();
        let setter = declarations.setters.get(id).copied();
        if getter.is_some() || setter.is_some() {
            return ScopeLookupResult {
                getter: getter.map(ScopeElement::Element),
                setter: setter.map(ScopeElement::Element),
            };
        }
        self.lookup_combined(fragment, id).unwrap_or_default()
    }

    /// Dart `_lookupCombined`.
    fn lookup_combined(
        &self,
        fragment: FId<LibraryFragment>,
        id: &str,
    ) -> Option<ScopeLookupResult> {
        let scope = &self.fragment_scopes[&fragment];
        let try_prefix = id != "_" || !scope.wildcard_variables;
        if try_prefix && let Some(&(prefix, s)) = scope.prefix_elements.get(id) {
            return Some(ScopeLookupResult {
                getter: Some(ScopeElement::Prefix(prefix, s)),
                setter: None,
            });
        }
        let r = self.prefix_lookup(scope.no_prefix_scope, id);
        if r.getter.is_some() || r.setter.is_some() {
            return Some(r);
        }
        scope.parent.and_then(|p| self.lookup_combined(p, id))
    }
}

/// The unit that includes [fragment] with a `part` directive.
fn enclosing_unit(
    linker: &Linker<'_>,
    library: usize,
    fragment: FId<LibraryFragment>,
) -> Option<FId<LibraryFragment>> {
    let store = &linker.core.store;
    for unit in &linker.builders[library].units {
        for part in &store.fragment(unit.fragment).parts {
            if let DirectiveUri::Unit {
                library_fragment, ..
            } = &part.directive.uri
                && *library_fragment == fragment
            {
                return Some(unit.fragment);
            }
        }
    }
    None
}

/// Dart `PrefixScope._add` / `_addTo` / `_merge`.
fn add_to_prefix_scope(linker: &Linker<'_>, scope: &mut PrefixScope, entry: &ExportEntry) {
    let is_setter = entry.element.tag() == Tag::Setter;
    let id: Arc<str> = if is_setter {
        entry.name.strip_suffix('=').unwrap_or(&entry.name).into()
    } else {
        entry.name.clone()
    };
    let map = if is_setter {
        &mut scope.setters
    } else {
        &mut scope.getters
    };
    let element = ScopeElement::Element(entry.element);
    match map.get(&id) {
        None => {
            map.insert(id, element);
        }
        Some(existing) if *existing == element => {}
        Some(existing) => {
            let existing = existing.clone();
            let merged = if is_sdk_element(linker, &existing) && !is_sdk_element(linker, &element) {
                element
            } else if !is_sdk_element(linker, &existing) && is_sdk_element(linker, &element) {
                existing
            } else {
                let mut conflicting: Vec<ElementId> = Vec::new();
                for e in [existing, element] {
                    match e {
                        ScopeElement::MultiplyDefined(list) => {
                            for x in list {
                                if !conflicting.contains(&x) {
                                    conflicting.push(x);
                                }
                            }
                        }
                        ScopeElement::Element(x) => {
                            if !conflicting.contains(&x) {
                                conflicting.push(x);
                            }
                        }
                        ScopeElement::Prefix(..) => {}
                    }
                }
                ScopeElement::MultiplyDefined(conflicting)
            };
            map.insert(id, merged);
        }
    }
}

/// Dart `LibraryDeclarations(library)`.
fn library_declarations(
    linker: &Linker<'_>,
    library: EId<LibraryElement>,
    uri: &str,
) -> LibraryDeclarations {
    let store = &linker.core.store;
    let l = store.get(library);
    let mut d = LibraryDeclarations::default();
    let add_getter = |d: &mut LibraryDeclarations, e: ElementId| {
        if let Some(name) = lookup_name(linker, e) {
            d.getters.entry(name.into()).or_insert(e);
        }
    };
    for &e in &l.getters {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.enums {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.extensions {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.extension_types {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.setters {
        if let Some(name) = lookup_name(linker, e.raw())
            && let Some(id) = name.strip_suffix('=')
        {
            d.setters.entry(id.into()).or_insert(e.raw());
        }
    }
    for &e in &l.top_level_functions {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.type_aliases {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.mixins {
        add_getter(&mut d, e.raw());
    }
    for &e in &l.classes {
        add_getter(&mut d, e.raw());
    }
    if uri == "dart:core" {
        add_getter(&mut d, ElementId::DYNAMIC);
        add_getter(&mut d, ElementId::NEVER);
    }
    d
}

/// An enclosed scope on top of a fragment scope (Dart `EnclosedScope`
/// subclasses that type resolution uses).
#[derive(Clone, Debug)]
pub enum EnclosedScope {
    /// Dart `TypeParameterScope`.
    TypeParameters(IndexMap<Arc<str>, ElementId>),
    /// Dart `InstanceScope`: getters (getters and methods) and setters,
    /// with whether each is static.
    Instance {
        getters: IndexMap<Arc<str>, (ElementId, bool)>,
        setters: IndexMap<Arc<str>, (ElementId, bool)>,
    },
    /// Dart `ExtensionScope`.
    Extension {
        getters: IndexMap<Arc<str>, ElementId>,
        setters: IndexMap<Arc<str>, ElementId>,
    },
}

impl EnclosedScope {
    /// The result of this scope, or `None` to ask the parent.
    pub fn lookup(&self, id: &str) -> Option<ScopeLookupResult> {
        match self {
            EnclosedScope::TypeParameters(map) => map.get(id).map(|&e| ScopeLookupResult {
                getter: Some(ScopeElement::Element(e)),
                setter: None,
            }),
            EnclosedScope::Instance { getters, setters } => {
                let g = getters.get(id).copied();
                let s = setters.get(id).copied();
                if g.is_none() && s.is_none() {
                    return None;
                }
                if g.is_some_and(|x| x.1) || s.is_some_and(|x| x.1) {
                    return Some(ScopeLookupResult {
                        getter: g.map(|x| ScopeElement::Element(x.0)),
                        setter: s.map(|x| ScopeElement::Element(x.0)),
                    });
                }
                Some(ScopeLookupResult::default())
            }
            EnclosedScope::Extension { getters, setters } => {
                let g = getters.get(id).copied();
                let s = setters.get(id).copied();
                if g.is_none() && s.is_none() {
                    return None;
                }
                Some(ScopeLookupResult {
                    getter: g.map(ScopeElement::Element),
                    setter: s.map(ScopeElement::Element),
                })
            }
        }
    }
}
