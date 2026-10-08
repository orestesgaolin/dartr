// Dart source: none (the lookup context replaces "an object knows its data",
// design §1.2 and §4.1); pkg/analyzer/lib/src/fine/requirements.dart for the
// requirement records of RequirementSink

//! The lookup context: [`Generation`], [`WorldSnapshot`], [`LocalArena`],
//! [`Ctx`], [`RequirementSink`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::LibraryElement;
use crate::element::{
    ElementData, ExecutableElementData, InstanceElementData, InterfaceElementData,
    PropertyAccessorElementData, PropertyInducingElementData, VariableElementData,
};
use crate::fragment::FragmentData;
use crate::ids::{
    EId, ElementId, ExecutableElement, FId, FragmentId, InstanceElement, InterfaceElement,
    PropertyAccessorElement, PropertyInducingElement, StoreId, StoreKind, VariableElement,
};
use crate::interner::{Interner, ListItem, TypeOverlay};
use crate::name::{Name, NamePool};
use crate::store::{AnyElement, ElementStore, StoredElement, StoredFragment};
use crate::type_provider::TypeProvider;
use crate::types::{
    AliasId, AliasRef, ListId, Member, MemberId, MentionsLocal, SubstId, SubstPair, TypeId,
    TypeKind,
};

/// The global state of one generation (design §4.2): the interner, the name
/// pool and the synthetic store. All snapshots of a generation share it. A
/// new generation starts when retired data is more than half of live data.
pub struct Generation {
    pub id: u32,
    pub interner: Interner,
    pub names: NamePool,
    /// Global, append-only: elements created inside shared caches.
    pub synthetic: ElementStore,
    next_cycle_store: AtomicU32,
    next_local_store: AtomicU32,
}

impl Generation {
    pub fn new(id: u32) -> Generation {
        Generation {
            id,
            interner: Interner::new(),
            names: NamePool::new(),
            synthetic: ElementStore::new(StoreId::SYNTHETIC),
            next_cycle_store: AtomicU32::new(1),
            next_local_store: AtomicU32::new(0),
        }
    }

    /// An empty store for a library cycle, with a new [`StoreId`].
    pub fn new_cycle_store(&self) -> ElementStore {
        let index = self.next_cycle_store.fetch_add(1, Ordering::Relaxed);
        ElementStore::new(StoreId::cycle(index))
    }

    /// An empty [`LocalArena`] for one analysis task, with a new [`StoreId`].
    pub fn new_local_arena(&self) -> LocalArena {
        let index = self.next_local_store.fetch_add(1, Ordering::Relaxed);
        LocalArena {
            store: ElementStore::new(StoreId::local(index)),
            types: TypeOverlay::new(),
        }
    }
}

/// The local elements and types of one analysis task (one unit's body
/// resolution). It adds through `&self` (append-only arenas), so the
/// resolver can create local elements while it holds a [`Ctx`].
pub struct LocalArena {
    pub store: ElementStore,
    pub types: TypeOverlay,
}

/// An immutable view of the world (design §4.1), shared with running tasks
/// through `Arc`. An edit makes a new snapshot; persistent maps make that
/// cheap.
#[derive(Clone)]
pub struct WorldSnapshot {
    pub generation: Arc<Generation>,
    /// Frozen cycle stores, indexed by [`StoreId::raw`].
    pub stores: imbl::Vector<Option<Arc<ElementStore>>>,
    /// Libraries by URI (lookups only; never iterate for output).
    pub libraries: imbl::HashMap<Arc<str>, EId<LibraryElement>>,
    /// Set once the `dart:core` / `dart:async` cycle is frozen.
    pub type_provider: Option<Arc<TypeProvider>>,
}

impl WorldSnapshot {
    pub fn new(generation: Arc<Generation>) -> WorldSnapshot {
        WorldSnapshot {
            generation,
            stores: imbl::Vector::new(),
            libraries: imbl::HashMap::new(),
            type_provider: None,
        }
    }

    /// A new snapshot that also has the frozen [store] and its [libraries]
    /// (URI, element).
    pub fn with_store(
        &self,
        store: Arc<ElementStore>,
        libraries: impl IntoIterator<Item = (Arc<str>, EId<LibraryElement>)>,
    ) -> WorldSnapshot {
        assert_eq!(
            store.id.kind(),
            StoreKind::Cycle,
            "only cycle stores are published"
        );
        let mut next = self.clone();
        let index = store.id.raw() as usize;
        while next.stores.len() <= index {
            next.stores.push_back(None);
        }
        next.stores.set(index, Some(store));
        for (uri, library) in libraries {
            next.libraries.insert(uri, library);
        }
        next
    }

    /// The frozen store [id], or the synthetic store.
    pub fn store(&self, id: StoreId) -> Option<&ElementStore> {
        match id.kind() {
            StoreKind::Synthetic => Some(&self.generation.synthetic),
            StoreKind::Cycle => self.stores.get(id.raw() as usize)?.as_deref(),
            StoreKind::Local => None,
        }
    }
}

/// A language feature set (placeholder for a port of `FeatureSet`,
/// `dart/analysis/features.dart`, unit B6): the enabled experiment flags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureSet {
    enabled: Vec<Arc<str>>,
}

impl FeatureSet {
    pub fn new(enabled: impl IntoIterator<Item = Arc<str>>) -> FeatureSet {
        let mut enabled: Vec<Arc<str>> = enabled.into_iter().collect();
        enabled.sort();
        enabled.dedup();
        FeatureSet { enabled }
    }

    /// Whether the feature with the `enable-experiment` flag [name] is
    /// enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.binary_search_by(|e| (**e).cmp(name)).is_ok()
    }
}

/// One requirement record (`RequirementsManifest.record_*`,
/// fine/requirements.dart): what a linking or analysis step looked up in
/// another library. v1 drops them ([`NoopSink`]); v2 (design §4.3) stores
/// them to decide what to relink or reanalyze.
#[derive(Clone, Copy, Debug)]
pub enum Requirement {
    ClassElementAllSubtypes {
        element: ElementId,
    },
    ClassElementDirectSubtypesOfSealed {
        element: ElementId,
    },
    FieldElementGetter {
        element: ElementId,
        name: Option<Name>,
    },
    FieldElementSetter {
        element: ElementId,
        name: Option<Name>,
    },
    /// `record_importPrefixScope_lookup` (the imported libraries are those
    /// of the prefix).
    ImportPrefixScopeLookup {
        prefix: ElementId,
        id: Name,
    },
    InstanceElementConstructors {
        element: ElementId,
    },
    InstanceElementFields {
        element: ElementId,
    },
    InstanceElementGetField {
        element: ElementId,
        name: Name,
    },
    InstanceElementGetGetter {
        element: ElementId,
        name: Name,
    },
    InstanceElementGetMethod {
        element: ElementId,
        name: Name,
    },
    InstanceElementGetSetter {
        element: ElementId,
        name: Name,
    },
    InstanceElementGetters {
        element: ElementId,
    },
    InstanceElementMethods {
        element: ElementId,
    },
    InstanceElementSetters {
        element: ElementId,
    },
    InterfaceAll {
        element: ElementId,
    },
    InterfaceGetMember {
        element: ElementId,
        name: Name,
        method_element: Option<ElementId>,
        concrete: bool,
        for_super: bool,
        for_mixin_index: i32,
    },
    InterfaceElementGetNamedConstructor {
        element: ElementId,
        name: Name,
    },
    InterfaceElementHasNonFinalField {
        element: ElementId,
    },
    LibraryAllClasses {
        library: EId<LibraryElement>,
    },
    LibraryAllEnums {
        library: EId<LibraryElement>,
    },
    LibraryAllExportedTopLevels {
        library: EId<LibraryElement>,
    },
    LibraryAllExtensions {
        library: EId<LibraryElement>,
    },
    LibraryAllExtensionTypes {
        library: EId<LibraryElement>,
    },
    LibraryAllGetters {
        library: EId<LibraryElement>,
    },
    LibraryAllMixins {
        library: EId<LibraryElement>,
    },
    LibraryAllSetters {
        library: EId<LibraryElement>,
    },
    LibraryAllTopLevelFunctions {
        library: EId<LibraryElement>,
    },
    LibraryAllTopLevelVariables {
        library: EId<LibraryElement>,
    },
    LibraryAllTypeAliases {
        library: EId<LibraryElement>,
    },
    LibraryEntryPoint {
        library: EId<LibraryElement>,
    },
    LibraryExportedLibraries {
        library: EId<LibraryElement>,
    },
    LibraryExportScopeGet {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryFeatureSet {
        library: EId<LibraryElement>,
    },
    LibraryGetClass {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetEnum {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetExtension {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetExtensionType {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetGetter {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetMixin {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetName {
        library: EId<LibraryElement>,
    },
    LibraryGetSetter {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetTopLevelFunction {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetTopLevelVariable {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryGetTypeAlias {
        library: EId<LibraryElement>,
        name: Name,
    },
    LibraryIsOriginNotExistingFile {
        library: EId<LibraryElement>,
    },
    LibraryIsSynthetic {
        library: EId<LibraryElement>,
    },
    LibraryLanguageVersion {
        library: EId<LibraryElement>,
    },
    LibraryMetadata {
        library: EId<LibraryElement>,
    },
    /// `record_libraryFragmentScope_accessibleExtensions`, for the imports
    /// of [fragment].
    LibraryFragmentScopeAccessibleExtensions {
        fragment: FragmentId,
    },
    PropertyAccessorElementVariable {
        element: ElementId,
        name: Option<Name>,
    },
    /// `recordOpaqueApiUse`: [target] is the element, [method] the API name.
    OpaqueApiUse {
        target: ElementId,
        method: &'static str,
    },
}

/// Receives [`Requirement`]s (design §4.3). Called at the same places as
/// `globalResultRequirements?.record_*` in the analyzer.
pub trait RequirementSink: Sync {
    fn record(&self, requirement: Requirement);
}

/// The v1 sink: drops every requirement.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopSink;

impl RequirementSink for NoopSink {
    #[inline(always)]
    fn record(&self, _: Requirement) {}
}

/// The lookup context, passed everywhere: the replacement for "an object
/// knows its data". Copy it freely.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    /// Frozen stores, the global interner, the synthetic store.
    pub world: &'a WorldSnapshot,
    /// The cycle being linked (shared borrow).
    pub current: Option<&'a ElementStore>,
    /// The local elements and type overlay of a body analysis task.
    pub local: Option<&'a LocalArena>,
    pub tp: &'a TypeProvider,
    pub features: &'a FeatureSet,
    pub req: &'a dyn RequirementSink,
}

impl<'a> Ctx<'a> {
    /// The same context without the local arena. Lazy shared caches
    /// (inheritance, member types) always run under `ctx.global()`, so they
    /// never see local elements or types.
    pub fn global(&self) -> Ctx<'a> {
        Ctx {
            local: None,
            ..*self
        }
    }

    #[inline]
    pub fn generation(&self) -> &'a Generation {
        &self.world.generation
    }

    // ---- stores and elements ----

    /// The store with [id]: the local arena, the current cycle, the
    /// synthetic store or a frozen cycle. Panics for an unknown store.
    #[inline]
    pub fn store(&self, id: StoreId) -> &'a ElementStore {
        if let Some(local) = self.local
            && local.store.id == id
        {
            return &local.store;
        }
        if let Some(current) = self.current
            && current.id == id
        {
            return current;
        }
        match self.world.store(id) {
            Some(store) => store,
            None => panic!("store {id:?} is not visible in this context"),
        }
    }

    /// The data of an element: `ctx.get(class_id).supertype`.
    #[inline]
    pub fn get<T: StoredElement + ?Sized>(&self, id: EId<T>) -> &'a T::Data {
        self.store(id.store()).get(id)
    }

    /// The data of a fragment.
    #[inline]
    pub fn fragment<T: StoredFragment + ?Sized>(&self, id: FId<T>) -> &'a T::Data {
        self.store(id.store()).fragment(id)
    }

    /// The element with [id], by kind (Dart `switch (element)`).
    #[inline]
    pub fn any(&self, id: ElementId) -> AnyElement<'a> {
        self.store(id.store()).any(id)
    }

    /// `ElementImpl` data; `None` for `dynamic` and `Never`.
    pub fn element_data(&self, id: ElementId) -> Option<&'a ElementData> {
        self.store(id.store()).element_data(id)
    }

    /// `FragmentImpl` data; `None` for `dynamic` and `Never`.
    pub fn fragment_data(&self, id: FragmentId) -> Option<&'a FragmentData> {
        self.store(id.store()).fragment_data(id)
    }

    pub fn instance(&self, id: EId<InstanceElement>) -> &'a InstanceElementData {
        self.store(id.store()).instance(id)
    }

    pub fn interface(&self, id: EId<InterfaceElement>) -> &'a InterfaceElementData {
        self.store(id.store()).interface(id)
    }

    pub fn executable(&self, id: EId<ExecutableElement>) -> &'a ExecutableElementData {
        self.store(id.store()).executable(id)
    }

    pub fn property_accessor(
        &self,
        id: EId<PropertyAccessorElement>,
    ) -> &'a PropertyAccessorElementData {
        self.store(id.store()).property_accessor(id)
    }

    pub fn variable(&self, id: EId<VariableElement>) -> &'a VariableElementData {
        self.store(id.store()).variable(id)
    }

    pub fn property_inducing(
        &self,
        id: EId<PropertyInducingElement>,
    ) -> &'a PropertyInducingElementData {
        self.store(id.store()).property_inducing(id)
    }

    /// The library with [uri], when it is in the world.
    pub fn library_by_uri(&self, uri: &str) -> Option<EId<LibraryElement>> {
        self.world.libraries.get(uri).copied()
    }

    // ---- names ----

    /// Interns a name.
    #[inline]
    pub fn name(&self, text: &str) -> Name {
        self.world.generation.names.intern(text)
    }

    /// The text of a name.
    #[inline]
    pub fn name_str(&self, name: Name) -> &'a str {
        self.world.generation.names.get(name)
    }

    // ---- types ----

    #[inline]
    fn overlay(&self) -> &'a TypeOverlay {
        &self
            .local
            .expect("a value with local ids needs a context with a LocalArena")
            .types
    }

    /// Interns a type: in the overlay when it mentions a local id, else
    /// globally.
    #[inline]
    pub fn intern(&self, kind: TypeKind) -> TypeId {
        if kind.mentions_local() {
            self.overlay().intern(kind)
        } else {
            self.world.generation.interner.intern(kind)
        }
    }

    /// The type with [id].
    #[inline]
    pub fn ty(&self, id: TypeId) -> &'a TypeKind {
        if id.is_local() {
            self.overlay().get(id)
        } else {
            self.world.generation.interner.get(id)
        }
    }

    /// Interns a list.
    pub fn intern_list<T: ListItem>(&self, items: &[T]) -> ListId<T> {
        if items.iter().any(MentionsLocal::mentions_local) {
            self.overlay().intern_list(items)
        } else {
            self.world.generation.interner.intern_list(items)
        }
    }

    /// The items of a list.
    #[inline]
    pub fn list<T: ListItem>(&self, id: ListId<T>) -> &'a [T] {
        if id.is_local() {
            self.overlay().list(id)
        } else {
            self.world.generation.interner.list(id)
        }
    }

    /// Interns a substitution (sorted into canonical order first).
    pub fn intern_subst(&self, pairs: &mut [SubstPair]) -> SubstId {
        if pairs.iter().any(MentionsLocal::mentions_local) {
            self.overlay().intern_subst(pairs)
        } else {
            self.world.generation.interner.intern_subst(pairs)
        }
    }

    pub fn subst(&self, id: SubstId) -> &'a [SubstPair] {
        if id.is_local() {
            self.overlay().subst(id)
        } else {
            self.world.generation.interner.subst(id)
        }
    }

    pub fn intern_member(&self, member: Member) -> MemberId {
        if member.mentions_local() {
            self.overlay().intern_member(member)
        } else {
            self.world.generation.interner.intern_member(member)
        }
    }

    pub fn member(&self, id: MemberId) -> &'a Member {
        if id.is_local() {
            self.overlay().member(id)
        } else {
            self.world.generation.interner.member(id)
        }
    }

    pub fn intern_alias(&self, alias: AliasRef) -> AliasId {
        if alias.mentions_local() {
            self.overlay().intern_alias(alias)
        } else {
            self.world.generation.interner.intern_alias(alias)
        }
    }

    pub fn alias(&self, id: AliasId) -> &'a AliasRef {
        if id.is_local() {
            self.overlay().alias(id)
        } else {
            self.world.generation.interner.alias(id)
        }
    }
}
