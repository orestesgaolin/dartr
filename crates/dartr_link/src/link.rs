// Dart source: pkg/analyzer/lib/src/summary2/link.dart

//! [`link_cycle`]: links one library cycle (Dart `Linker.link`) into a new
//! [`ElementStore`].
//!
//! Phases (Dart `Linker._buildOutlines`), as far as ported:
//! 1. `LibraryBuilder.build` for every library (library elements and
//!    defining units).
//! 2. `_computeLibraryScopes`: `buildElements` of every library (directives,
//!    parts, fragments, elements, informative data), then
//!    `_buildExportScopes`.
//! 3. `_buildClassSyntheticConstructors`, `_buildEnumSyntheticConstructors`,
//!    `_replaceConstFieldsIfNoConstConstructor`,
//!    `_resolveConstructorFieldFormals`, `_collectMixinSuperInvokedNames`
//!    (the structural parts of the later phases).
//!
//! Not ported yet: type resolution (B2), type aliases, simple bounds,
//! defaults, variance, interface cycles (B3), mixin application
//! constructors, enum children, field promotability, `hasNonFinalField`,
//! extension types (B4), top-level inference and the expression phases.

use std::sync::Arc;

use dartr_ast::{Ast, NodeId};
use dartr_ast_builder::ParsedUnit;
use dartr_element::*;
use dartr_syntax::Tokens;
use indexmap::IndexMap;

use crate::export::{ExportEntry, compute_export_scopes};
use crate::input::LinkLibraryInput;
use crate::library_builder::LibraryBuilder;

/// A linked library, as other cycles see it.
#[derive(Debug)]
pub struct LinkedLibrary {
    pub uri: Arc<str>,
    pub element: EId<LibraryElement>,
    /// Dart `LibraryElementImpl.exportEntries`.
    pub export_entries: Vec<ExportEntry>,
    pub reference: crate::reference::LibraryReference,
}

/// The libraries of the cycles linked before (Dart
/// `LinkedElementFactory.libraryOfUri2` and `exportEntries`).
pub trait LinkedLibraries: Sync {
    fn library(&self, uri: &str) -> Option<Arc<LinkedLibrary>>;
    /// The `ConstExprs` of the linked cycle with the store [store].
    fn const_exprs(&self, store: StoreId) -> Option<Arc<ConstExprs>>;
}

/// The resolver that linking calls (Dart: the linker creates an
/// `AstResolver`, summary2/ast_resolver.dart, unit C10).
///
/// `dartr_resolver` (body resolution) is a separate crate that does not
/// depend on `dartr_link`, and `dartr_link` does not depend on it: the
/// driver, which depends on both, passes an implementation into
/// [`link_cycle`] (`dartr_driver::link_resolver`, over
/// `dartr_resolver::ast_resolver`). [`NoLinkResolver`] resolves nothing
/// (the variables whose type comes from an initializer stay without a
/// type).
pub trait LinkResolver: Sync {
    /// A session for one cycle: the state that the resolutions of the
    /// cycle share (library scopes, unit copies). Used on one thread.
    fn session(&self) -> Box<dyn LinkResolverSession + '_>;
}

/// The resolutions of one linked cycle (see [`LinkResolver`]).
pub trait LinkResolverSession {
    /// Dart `AstResolver(...).resolveExpression(...)` followed by
    /// `node.typeOrThrow`: the static type of the expression of
    /// [request]. The type has no local elements. `None` when it could not
    /// be resolved.
    fn resolve_expression(&self, ctx: &Ctx<'_>, request: &ExpressionRequest<'_>) -> Option<TypeId>;
}

/// An expression to resolve while linking (Dart `AstResolver(linker,
/// libraryFragment, scope, analysisOptions, enclosingClassElement:)` and
/// the arguments of `resolveExpression`).
pub struct ExpressionRequest<'r> {
    pub library: EId<LibraryElement>,
    /// The library fragment of the unit (Dart `libraryFragment`).
    pub fragment: FId<LibraryFragment>,
    pub source: ExpressionSource<'r>,
    /// The instance element that encloses the declaration (the scopes of
    /// Dart `initializerScope`).
    pub enclosing_instance: Option<EId<InstanceElement>>,
    /// Dart `enclosingClassElement`.
    pub enclosing_class: Option<EId<InterfaceElement>>,
    /// Dart `contextType` (`UnknownInferredType` by default).
    pub context_type: TypeId,
    /// Dart `inScopePrimaryConstructorParameters`.
    pub in_scope_primary_constructor_parameters: Option<&'r [EId<FormalParameterElement>]>,
}

/// Where the expression of an [`ExpressionRequest`] is.
pub enum ExpressionSource<'r> {
    /// An expression of a unit.
    Unit {
        parsed: &'r Arc<ParsedUnit>,
        /// Dart `node.declaredFragment` of the declarations of the unit.
        declared_fragments: &'r IndexMap<NodeId, FragmentId>,
        /// The node that has the expression: a `VariableDeclaration` (its
        /// initializer) or a formal parameter (its default value).
        owner: NodeId,
    },
    /// A synthetic expression in the [`ConstExprs`] of the cycle (Dart: the
    /// initializer of the synthetic `VariableDeclaration` of an enum
    /// constant).
    Synthetic {
        ast: &'r Ast,
        expression: NodeId,
        /// The features of the unit of the declaration.
        features: dartr_parser::experimental_features::ExperimentalFeatures,
    },
}

/// A [`LinkResolver`] that resolves nothing.
pub struct NoLinkResolver;

impl LinkResolver for NoLinkResolver {
    fn session(&self) -> Box<dyn LinkResolverSession + '_> {
        Box::new(NoLinkResolver)
    }
}

impl LinkResolverSession for NoLinkResolver {
    fn resolve_expression(&self, _ctx: &Ctx<'_>, _request: &ExpressionRequest<'_>) -> Option<TypeId> {
        None
    }
}

/// The expressions of a cycle that linking copies from the units
/// (design §2.2): detached subtrees in one [`Ast`].
#[derive(Debug)]
pub struct ConstExprs {
    pub ast: Ast,
}

impl ConstExprs {
    pub fn new() -> ConstExprs {
        ConstExprs {
            ast: Ast::new(Tokens::new(Arc::from(""))),
        }
    }

    /// Copies [node] of [src] and returns the copy.
    pub fn copy(&mut self, src: &Ast, node: impl Into<NodeId>) -> ConstExprId {
        ConstExprId(dartr_ast::copy_subtree(src, node, &mut self.ast))
    }
}

impl Default for ConstExprs {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of linking one cycle (design §2.2 `LinkedCycle`).
#[derive(Debug)]
pub struct LinkedCycle {
    pub store: Arc<ElementStore>,
    pub const_exprs: Arc<ConstExprs>,
    pub libraries: Vec<Arc<LinkedLibrary>>,
}

/// A unit of a library under linking (Dart `LinkingUnit`).
#[derive(Debug)]
pub struct LinkingUnit {
    pub parsed: Arc<ParsedUnit>,
    pub fragment: FId<LibraryFragment>,
    /// Dart `node.declaredFragment` of the declarations in this unit.
    pub declared_fragments: IndexMap<NodeId, FragmentId>,
}

/// The state of the linker that the library builders share (Dart
/// `Linker`, without the builders).
pub struct LinkerCore<'a> {
    pub generation: &'a Generation,
    pub world: &'a WorldSnapshot,
    pub deps: &'a dyn LinkedLibraries,
    pub store: ElementStore,
    pub const_exprs: ConstExprs,
    /// Dart `Linker._fragmentNodes`: (library, unit, node) of a fragment.
    pub fragment_nodes: IndexMap<FragmentId, (usize, usize, NodeId)>,
    /// Dart `Linker.declaringFormalParameters`: the field and the formal
    /// parameter fragment of each declaring parameter of a primary
    /// constructor.
    pub declaring_formal_parameters: Vec<(FId<FieldFragment>, FId<FormalParameterFragment>)>,
}

impl LinkerCore<'_> {
    pub fn name(&self, text: &str) -> Name {
        self.generation.names.intern(text)
    }

    pub fn name_opt(&self, text: Option<&str>) -> Option<Name> {
        text.map(|t| self.name(t))
    }

    pub fn name_str(&self, name: Name) -> &str {
        self.generation.names.get(name)
    }
}

/// Dart `Linker`.
pub struct Linker<'a> {
    pub core: LinkerCore<'a>,
    pub builders: Vec<LibraryBuilder>,
    pub builder_by_uri: IndexMap<Arc<str>, usize>,
}

impl Linker<'_> {
    /// The library element of [uri] in this cycle or in a linked cycle
    /// (Dart `elementFactory.libraryOfUri2`).
    pub fn library_of_uri(&self, uri: &str) -> Option<EId<LibraryElement>> {
        if let Some(&b) = self.builder_by_uri.get(uri) {
            return Some(self.builders[b].element);
        }
        self.core.deps.library(uri).map(|l| l.element)
    }
}

/// Dart `link`: links the libraries of one cycle. The libraries of the
/// cycles that this cycle depends on are in [world] and [deps].
pub fn link_cycle(
    world: &WorldSnapshot,
    deps: &dyn LinkedLibraries,
    resolver: &dyn LinkResolver,
    inputs: &[LinkLibraryInput],
) -> LinkedCycle {
    let generation = &*world.generation;
    let mut linker = Linker {
        core: LinkerCore {
            generation,
            world,
            deps,
            store: generation.new_cycle_store(),
            const_exprs: ConstExprs::new(),
            fragment_nodes: IndexMap::new(),
            declaring_formal_parameters: Vec::new(),
        },
        builders: Vec::new(),
        builder_by_uri: IndexMap::new(),
    };

    // LibraryBuilder.build
    for input in inputs {
        let builder = LibraryBuilder::build(&mut linker.core, input.clone());
        linker
            .builder_by_uri
            .insert(builder.uri.clone(), linker.builders.len());
        linker.builders.push(builder);
    }

    // _computeLibraryScopes
    for index in 0..linker.builders.len() {
        LibraryBuilder::build_elements(&mut linker, index);
    }
    set_library_and_enclosing(&mut linker.core.store);
    build_export_scopes(&mut linker);

    // _createTypeSystem, _resolveTypes
    let type_provider = crate::types_builder::create_type_provider(&linker);
    crate::types_builder::resolve_types(&mut linker, &type_provider);
    // _MixinsInference._resetHierarchies: hierarchies computed during mixin
    // inference may have seen mixins that were not inferred yet.
    clear_interface_caches(&mut linker.core.store, true);

    // _computeHasNonFinalField ... buildExtensionTypes
    crate::outline::build_outlines(&mut linker, &type_provider, resolver);
    for index in 0..linker.builders.len() {
        LibraryBuilder::collect_mixin_super_invoked_names(&mut linker, index);
    }
    set_library_and_enclosing(&mut linker.core.store);
    // Dart: the `InheritanceManager3` of the linker is dropped with it.
    clear_interface_caches(&mut linker.core.store, false);
    // _detachNodes
    crate::detach_nodes::detach_nodes(&mut linker.core);

    let Linker { core, builders, .. } = linker;
    let LinkerCore {
        store, const_exprs, ..
    } = core;
    LinkedCycle {
        store: Arc::new(store),
        const_exprs: Arc::new(const_exprs),
        libraries: builders
            .into_iter()
            .map(|b| {
                Arc::new(LinkedLibrary {
                    uri: b.uri.clone(),
                    element: b.element,
                    export_entries: b.export_scope.to_export_entries(),
                    reference: b.references.reference,
                })
            })
            .collect(),
    }
}

/// Dart `Linker._buildExportScopes` + `storeExportScope`.
fn build_export_scopes(linker: &mut Linker<'_>) {
    let count = linker.builders.len();
    let mut scopes = Vec::with_capacity(count);
    for index in 0..count {
        scopes.push(linker.builders[index].initial_export_scope());
    }
    let mut exports: Vec<Vec<crate::export::Export>> = vec![Vec::new(); count];
    for index in 0..count {
        LibraryBuilder::add_exporters(linker, index, &mut scopes, &mut exports);
    }
    compute_export_scopes(&mut scopes, &exports);
    for (index, scope) in scopes.into_iter().enumerate() {
        linker.builders[index].export_scope = scope;
        LibraryBuilder::store_export_scope(linker, index);
    }
}

/// Sets the cached `library` and `enclosing` of every element of [store]
/// (Dart computes `enclosingElement` from the enclosing fragment, and
/// `library` by walking up).
pub fn set_library_and_enclosing(store: &mut ElementStore) {
    let mut updates: Vec<(ElementId, Option<ElementId>)> = Vec::new();
    let mut ids: Vec<ElementId> = Vec::new();
    macro_rules! collect {
        ($($field:ident: $t:ty),*) => {$(
            for (index, _) in store.elements.$field.iter() {
                ids.push(ElementId::new(store.id, <$t as StoredElement>::TAG, index));
            }
        )*};
    }
    collect!(
        classes: ClassElement, enums: EnumElement, mixins: MixinElement,
        extensions: ExtensionElement, extension_types: ExtensionTypeElement,
        fields: FieldElement, getters: GetterElement, setters: SetterElement,
        methods: MethodElement, constructors: ConstructorElement,
        functions: TopLevelFunctionElement, variables: TopLevelVariableElement,
        type_aliases: TypeAliasElement, type_params: TypeParameterElement,
        prefixes: PrefixElement, generic_function_types: GenericFunctionTypeElement
    );
    for (index, p) in store.elements.params.iter() {
        ids.push(ElementId::new(store.id, p.first_fragment.tag(), index));
    }
    for &id in &ids {
        let first = store.element_data(id).unwrap().first_fragment;
        let enclosing = match id.tag() {
            Tag::Prefix => None,
            _ => store
                .fragment_data(first)
                .and_then(|f| f.enclosing_fragment)
                .and_then(|e| store.fragment_data(e))
                .and_then(|e| e.element.try_get().copied()),
        };
        updates.push((id, enclosing));
    }
    for (id, enclosing) in &updates {
        element_data_mut(store, *id).enclosing = *enclosing;
    }
    // Libraries: walk up.
    for (id, _) in &updates {
        let mut current = *id;
        let library = loop {
            if current.tag() == Tag::Library {
                break Some(EId::<LibraryElement>::from_raw(current));
            }
            let Some(data) = store.element_data(current) else {
                break None;
            };
            match data.enclosing {
                Some(e) if e.store() == store.id => current = e,
                // Only library elements are outside of the store chain.
                _ => break None,
            }
        };
        element_data_mut(store, *id).library = library;
    }
    let library_indexes: Vec<u32> = store.elements.libraries.iter().map(|(i, _)| i).collect();
    for index in library_indexes {
        let id = EId::<LibraryElement>::from_raw(ElementId::new(store.id, Tag::Library, index));
        store.get_mut(id).element.library = Some(id);
    }
}

/// `ElementImpl` data of any element, mutable.
pub fn element_data_mut(store: &mut ElementStore, id: ElementId) -> &mut ElementData {
    let i = id.index();
    let e = &mut store.elements;
    fn co<T: std::ops::DerefMut>(x: &mut T) -> &mut ElementData
    where
        T::Target: CoerceElementData,
    {
        (**x).coerce()
    }
    match id.tag() {
        Tag::Class => co(e.classes.get_mut(i)),
        Tag::Enum => co(e.enums.get_mut(i)),
        Tag::Mixin => co(e.mixins.get_mut(i)),
        Tag::Extension => co(e.extensions.get_mut(i)),
        Tag::ExtensionType => co(e.extension_types.get_mut(i)),
        Tag::Field => co(e.fields.get_mut(i)),
        Tag::Getter => co(e.getters.get_mut(i)),
        Tag::Setter => co(e.setters.get_mut(i)),
        Tag::Method => co(e.methods.get_mut(i)),
        Tag::Constructor => co(e.constructors.get_mut(i)),
        Tag::TopLevelFunction => co(e.functions.get_mut(i)),
        Tag::TopLevelVariable => co(e.variables.get_mut(i)),
        Tag::TypeAlias => co(e.type_aliases.get_mut(i)),
        Tag::TypeParameter => co(e.type_params.get_mut(i)),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            co(e.params.get_mut(i))
        }
        Tag::Prefix => co(e.prefixes.get_mut(i)),
        Tag::Library => co(e.libraries.get_mut(i)),
        Tag::GenericFunctionType => co(e.generic_function_types.get_mut(i)),
        Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable => {
            co(e.locals.get_mut(i))
        }
        Tag::LocalFunction => co(e.local_functions.get_mut(i)),
        Tag::Label => co(e.labels.get_mut(i)),
        Tag::MultiplyDefined => co(e.multiply_defined.get_mut(i)),
        Tag::Dynamic | Tag::Never => unreachable!("{id:?}"),
    }
}

/// Steps down the `DerefMut` chain of element structs.
pub trait CoerceElementData {
    fn coerce(&mut self) -> &mut ElementData;
}

impl CoerceElementData for ElementData {
    fn coerce(&mut self) -> &mut ElementData {
        self
    }
}

macro_rules! coerce_via {
    ($($t:ty),*) => {$(
        impl CoerceElementData for $t {
            fn coerce(&mut self) -> &mut ElementData {
                (**self).coerce()
            }
        }
    )*};
}
coerce_via!(
    InstanceElementData,
    InterfaceElementData,
    ExecutableElementData,
    PropertyAccessorElementData,
    VariableElementData,
    PropertyInducingElementData
);

/// Removes the caches that the inheritance manager (`Interface`s) and, with
/// [hierarchies], the class hierarchy (`allSupertypes`) keep on the
/// interface elements of [store].
pub fn clear_interface_caches(store: &mut ElementStore, hierarchies: bool) {
    fn clear(i: &mut InterfaceElementData, hierarchies: bool) {
        i.inheritance = dartr_element::slot::ElementCache::new();
        if hierarchies {
            i.all_supertypes = OnceSlot::new();
        }
    }
    let e = &mut store.elements;
    let indexes: Vec<u32> = e.classes.iter().map(|(i, _)| i).collect();
    for i in indexes {
        clear(e.classes.get_mut(i), hierarchies);
    }
    let indexes: Vec<u32> = e.enums.iter().map(|(i, _)| i).collect();
    for i in indexes {
        clear(e.enums.get_mut(i), hierarchies);
    }
    let indexes: Vec<u32> = e.mixins.iter().map(|(i, _)| i).collect();
    for i in indexes {
        clear(e.mixins.get_mut(i), hierarchies);
    }
    let indexes: Vec<u32> = e.extension_types.iter().map(|(i, _)| i).collect();
    for i in indexes {
        clear(e.extension_types.get_mut(i), hierarchies);
    }
}
