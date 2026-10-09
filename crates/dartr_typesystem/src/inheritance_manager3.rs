// Dart source: pkg/analyzer/lib/src/dart/element/inheritance_manager3.dart

//! `InheritanceManager3`: the interfaces of classes, mixins, enums and
//! extension types ([`Interface`]): member signatures from superclasses,
//! mixins and interfaces, combined signatures (`topMerge`), concrete
//! implementations, `noSuchMethod` forwarders, and [`Conflict`]s.
//!
//! # Rust model (design §2.3)
//!
//! - The Dart manager keeps identity maps keyed by element
//!   (`_interfaces`, `_combinedSignatures`, `_processingClasses`). Rust
//!   keeps the interface of an element in
//!   `InterfaceElementData.inheritance` (an [`ElementCache`] holding an
//!   [`InheritanceCache`]), so [`InheritanceManager3`] is a stateless view
//!   over the elements. Interfaces are computed under `ctx.global()`, and the
//!   members they synthesize (`_topMerge`, `_inheritCovariance`) go to
//!   `fresh_store(ctx.global())` (the cycle being linked, else the synthetic
//!   store of the generation).
//! - `Reference.getOrCreate{Method,Getter,Setter}` of the target class
//!   becomes [`InheritanceCache::synthesized`] plus a lookup of the declared
//!   members with the same name, so a second merge of the same name returns
//!   the same element, as in Dart.
//! - Members (`InternalExecutableElement`) are [`ElemRef`]s; see
//!   [`crate::member`].
//! - Dart maps keep insertion order: every map here is an `IndexMap`.
//!
//! [`ElementCache`]: dartr_element::ElementCache

// Keep the nesting of the Dart code.
#![allow(clippy::collapsible_if)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

use dartr_element::{
    BoolSlot, ClassElement, Ctx, EId, ElemRef, ElementData, ElementFlags, ElementId,
    ExecutableElement, ExecutableElementData, ExecutableFragmentData, ExtensionTypeElement, FId,
    FieldElement, FieldFragment, FormalParameterElement, FormalParameterFragment, FragmentData,
    FragmentFlags, GetterElement, GetterFragment, InterfaceElement, LibraryElement, LookupMap,
    MethodElement, MethodFragment, MixinElement, Nullability, OnceSlot, ParameterKind,
    PropertyAccessorElement, PropertyAccessorElementData, PropertyAccessorFragmentData,
    PropertyInducingElement, PropertyInducingElementData, PropertyInducingFragmentData,
    Requirement, SetterElement, SetterFragment, Tag, TypeId, TypeKind, TypeParameterElement,
    TypeParameterFragment, VarSlot, VariableElementData, VariableFragmentData,
};
use indexmap::{IndexMap, IndexSet};

use crate::member;
use crate::type_algebra::MapSubstitution;
use crate::type_ext::{TypeExt, fresh_store, is_named};
use crate::type_system::TypeSystem;

// ------------------------------------------------------------------ Name

/// `Name`: a public name, or a private name qualified by its library.
/// Names of setters end with `=`.
///
/// Dart keeps the library URI; Rust keeps the library element (one element
/// per URI in a world). [`Name::to_display`] is Dart `toString`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Name {
    /// If the name is private, the defining library; otherwise `None`.
    pub library_uri: Option<EId<LibraryElement>>,
    /// The name text (`foo`, `foo=`, `_bar`).
    pub name: dartr_element::Name,
}

impl Name {
    /// `Name(libraryUri, name)`.
    pub fn new(ctx: &Ctx<'_>, library_uri: Option<EId<LibraryElement>>, name: &str) -> Name {
        if name.starts_with('_') {
            Name {
                library_uri,
                name: ctx.name(name),
            }
        } else {
            Name {
                library_uri: None,
                name: ctx.name(name),
            }
        }
    }

    /// `Name.forLibrary(library, name)`.
    pub fn for_library(ctx: &Ctx<'_>, library: Option<EId<LibraryElement>>, name: &str) -> Name {
        Name::new(ctx, library, name)
    }

    /// `Name.forElement(element)`.
    pub fn for_element(ctx: &Ctx<'_>, element: ElemRef) -> Option<Name> {
        let name = member::lookup_name(ctx, element)?;
        if name.starts_with('_') {
            Some(Name::new(ctx, member::library(ctx, element), &name))
        } else {
            Some(Name::new(ctx, None, &name))
        }
    }

    /// The name text (Dart `name`).
    pub fn text<'a>(&self, ctx: &Ctx<'a>) -> &'a str {
        ctx.name_str(self.name)
    }

    /// `isPublic`.
    pub fn is_public(&self, ctx: &Ctx<'_>) -> bool {
        !self.text(ctx).starts_with('_')
    }

    /// `forGetter`.
    pub fn for_getter(&self, ctx: &Ctx<'_>) -> Name {
        if self.is_setter(ctx) {
            let text = self.text(ctx);
            Name::new(ctx, self.library_uri, &text[..text.len() - 1])
        } else {
            *self
        }
    }

    /// `forSetter`.
    pub fn for_setter(&self, ctx: &Ctx<'_>) -> Name {
        if self.is_setter(ctx) {
            *self
        } else {
            Name::new(ctx, self.library_uri, &format!("{}=", self.text(ctx)))
        }
    }

    /// `isSetter`.
    pub fn is_setter(&self, ctx: &Ctx<'_>) -> bool {
        let text = self.text(ctx);
        text.ends_with('=') && !matches!(text, "[]=" | "==" | "<=" | ">=")
    }

    /// `isAccessibleFor(libraryUri)`.
    pub fn is_accessible_for(&self, ctx: &Ctx<'_>, library: EId<LibraryElement>) -> bool {
        self.is_public(ctx) || self.library_uri == Some(library)
    }

    /// `toString()`: `<libraryUri>::<name>` for private names.
    pub fn to_display(&self, ctx: &Ctx<'_>) -> String {
        match self.library_uri {
            Some(library) => format!("{}::{}", ctx.library_uri(library), self.text(ctx)),
            None => self.text(ctx).to_string(),
        }
    }
}

// ------------------------------------------------------------------ Conflict

/// `Conflict` and its subclasses: a failure to find a valid signature from
/// superinterfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// `CandidatesConflict`: no most specific signature among [candidates]
    /// (at least two).
    Candidates {
        name: Name,
        candidates: Vec<ElemRef>,
    },
    /// `GetterMethodConflict`: a getter and a method from direct
    /// superinterfaces.
    GetterMethod {
        name: Name,
        getter: ElemRef,
        method: ElemRef,
    },
    /// `HasNonExtensionAndExtensionMemberConflict`.
    HasNonExtensionAndExtensionMember {
        name: Name,
        non_extension: Vec<ElemRef>,
        extension: Vec<ElemRef>,
    },
    /// `NotUniqueExtensionMemberConflict`.
    NotUniqueExtensionMember {
        name: Name,
        candidates: Vec<ElemRef>,
    },
    /// `ExtensionTypeConflictingInheritedMethodAndSetterConflict`.
    ExtensionTypeConflictingInheritedMethodAndSetter {
        name: Name,
        method: ElemRef,
        setter: ElemRef,
    },
    /// `ExtensionTypeConflictingStaticAndInstanceConflict`.
    ExtensionTypeConflictingStaticAndInstance {
        name: Name,
        declared: ElemRef,
        inherited: ElemRef,
    },
}

impl Conflict {
    /// `name`.
    pub fn name(&self) -> Name {
        match self {
            Conflict::Candidates { name, .. }
            | Conflict::GetterMethod { name, .. }
            | Conflict::HasNonExtensionAndExtensionMember { name, .. }
            | Conflict::NotUniqueExtensionMember { name, .. }
            | Conflict::ExtensionTypeConflictingInheritedMethodAndSetter { name, .. }
            | Conflict::ExtensionTypeConflictingStaticAndInstance { name, .. } => *name,
        }
    }

    /// The Dart class name of the conflict.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Conflict::Candidates { .. } => "CandidatesConflict",
            Conflict::GetterMethod { .. } => "GetterMethodConflict",
            Conflict::HasNonExtensionAndExtensionMember { .. } => {
                "HasNonExtensionAndExtensionMemberConflict"
            }
            Conflict::NotUniqueExtensionMember { .. } => "NotUniqueExtensionMemberConflict",
            Conflict::ExtensionTypeConflictingInheritedMethodAndSetter { .. } => {
                "ExtensionTypeConflictingInheritedMethodAndSetterConflict"
            }
            Conflict::ExtensionTypeConflictingStaticAndInstance { .. } => {
                "ExtensionTypeConflictingStaticAndInstanceConflict"
            }
        }
    }
}

// ------------------------------------------------------------------ Interface

/// A map from names to members (Dart `Map<Name, InternalExecutableElement>`).
pub type NameMap = IndexMap<Name, ElemRef>;

/// A map from names to lists of members.
pub type NameListMap = IndexMap<Name, Vec<ElemRef>>;

/// `Interface`: the instance interface of an interface element.
#[derive(Debug, Default)]
pub struct Interface {
    /// The map of names to their signature in the interface.
    pub map: NameMap,
    /// The map of declared names to their signatures.
    pub declared: NameMap,
    /// The map of names to their concrete implementations.
    pub implemented: NameMap,
    /// The set of names that are `noSuchMethod` forwarders in
    /// [Interface::implemented].
    pub no_such_method_forwarders: IndexSet<Name>,
    /// The map of names to their signatures from the mixins, superclasses,
    /// or interfaces.
    pub overridden: NameListMap,
    /// The map of names to the signatures from superinterfaces that a member
    /// declaration in this extension type redeclares.
    pub redeclared: NameListMap,
    /// Each item maps names to their concrete implementations: the nominal
    /// superclass, then the superclass plus the first mixin, etc.
    pub super_implemented: Vec<NameMap>,
    /// The conflicts between superinterfaces (not with the declared members
    /// of the class).
    pub conflicts: Vec<Conflict>,
    /// Signatures from superinterfaces that were combined (for manifests).
    pub combined_signatures: NameListMap,
    /// `inheritedMap`: the most specific signatures from the mixins,
    /// superclasses, or interfaces; computed by
    /// [`InheritanceManager3::get_inherited_map`].
    pub inherited_map: OnceLock<NameMap>,
}

impl Interface {
    /// `Interface._empty`.
    fn empty() -> Interface {
        Interface {
            super_implemented: vec![NameMap::new()],
            ..Interface::default()
        }
    }

    /// `isSuperImplemented(name)`.
    pub fn is_super_implemented(&self, name: &Name) -> bool {
        self.super_implemented
            .last()
            .is_some_and(|m| m.contains_key(name))
    }
}

/// The kind of a synthesized member (the reference kinds of
/// `getOrCreateMethod` / `getOrCreateGetter` / `getOrCreateSetter`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum SynthesizedKind {
    Method,
    Getter,
    Setter,
}

/// The per-element entry of the Dart manager maps, stored in
/// `InterfaceElementData.inheritance`.
#[derive(Default)]
pub struct InheritanceCache {
    /// `_interfaces[element]`.
    interface: OnceLock<Arc<Interface>>,
    /// The members that `_topMerge` / `_inheritCovariance` created under the
    /// reference of this element (`reference.getOrCreate*(name)`).
    synthesized: Mutex<LookupMap<(SynthesizedKind, dartr_element::Name), ElementId>>,
}

thread_local! {
    /// `_processingClasses`: elements whose interface is being computed on
    /// this thread.
    static PROCESSING: RefCell<Vec<EId<InterfaceElement>>> = const { RefCell::new(Vec::new()) };
}

// ------------------------------------------------------------------ manager

/// Options of [`InheritanceManager3::get_member_with`] (the named
/// parameters of Dart `getMember`).
#[derive(Clone, Copy, Debug)]
pub struct GetMemberOptions {
    pub concrete: bool,
    pub for_mixin_index: i32,
    pub for_super: bool,
}

impl Default for GetMemberOptions {
    fn default() -> Self {
        GetMemberOptions {
            concrete: false,
            for_mixin_index: -1,
            for_super: false,
        }
    }
}

/// `InheritanceManager3`: manages knowledge about interface types and their
/// members. Stateless: the caches are on the elements (see the module
/// documentation). [ctx] is the context of the caller; interfaces are
/// computed under `ctx.global()`, and the members that [`Self::get_member3`]
/// substitutes are interned under [ctx].
#[derive(Clone, Copy)]
pub struct InheritanceManager3<'a> {
    pub ctx: Ctx<'a>,
}

impl<'a> InheritanceManager3<'a> {
    pub fn new(ctx: Ctx<'a>) -> Self {
        InheritanceManager3 { ctx }
    }

    /// `combineSignatureTypes`: combines the types of [candidates] into a
    /// single most specific type. Returns `None` (and adds a conflict to
    /// [conflicts]) when there is none.
    pub fn combine_signature_types(
        &self,
        candidates: &[ElemRef],
        name: Name,
        conflicts: Option<&mut Vec<Conflict>>,
    ) -> Option<TypeId> {
        let ctx = &self.ctx;
        let type_system = TypeSystem::new(*ctx);
        if candidates.len() == 1 {
            return Some(member::type_(ctx, candidates[0]));
        }

        let valid_overrides = get_valid_overrides(&type_system, candidates);

        if valid_overrides.is_empty() {
            if let Some(conflicts) = conflicts {
                conflicts.push(Conflict::Candidates {
                    name,
                    candidates: candidates.to_vec(),
                });
            }
            return None;
        }

        // Often there is one most specific signature.
        let first_type = member::type_(ctx, valid_overrides[0]);
        if valid_overrides.len() == 1 {
            return Some(first_type);
        }

        // Maybe more than valid, but the same type.
        // Dart: e.type == firstType
        if valid_overrides
            .iter()
            .all(|&e| ctx.dart_eq(member::type_(ctx, e), first_type))
        {
            return Some(first_type);
        }

        Some(top_merge_signature_types(&type_system, &valid_overrides))
    }

    /// `getInherited(element, name)`: the most specific signature of the
    /// member [name] that [element] inherits (`getInheritedMap(element)[name]`).
    pub fn get_inherited(&self, element: EId<InterfaceElement>, name: Name) -> Option<ElemRef> {
        self.get_inherited_map(element).get(&name).copied()
    }

    /// `getInheritedConcreteMap(element)`: the concrete members that
    /// [element] inherits from the superclasses and mixins.
    pub fn get_inherited_concrete_map(&self, element: EId<InterfaceElement>) -> NameMap {
        if element.raw().is::<ExtensionTypeElement>() {
            return NameMap::new();
        }

        let interface = self.get_interface(element);
        match interface.super_implemented.last() {
            None => {
                debug_assert_eq!(self.ctx.element_name(element.raw()), Some("Object"));
                NameMap::new()
            }
            Some(last) => last.clone(),
        }
    }

    /// `getInheritedMap(element)`: names to the most specific signatures of
    /// members inherited from the superinterfaces (superclasses, mixins and
    /// interfaces). Names without a most specific signature are missing.
    pub fn get_inherited_map(&self, element: EId<InterfaceElement>) -> &'a NameMap {
        let interface: &'a Interface = self.get_interface_ref(element);
        if let Some(map) = interface.inherited_map.get() {
            return map;
        }
        let mut inherited_map = NameMap::new();
        let mut computer = Computer::new(owner_ctx(&self.ctx, element), element);
        computer.find_most_specific_from_named_candidates(
            &mut inherited_map,
            if element.raw().is::<ExtensionTypeElement>() {
                &interface.redeclared
            } else {
                &interface.overridden
            },
        );
        interface.inherited_map.get_or_init(|| inherited_map)
    }

    /// `getInterface(element)`: the interface of [element]. It might include
    /// private members, not necessary accessible in all libraries.
    pub fn get_interface(&self, element: EId<InterfaceElement>) -> Arc<Interface> {
        self.ctx.req.record(Requirement::InterfaceAll {
            element: element.raw(),
        });
        get_interface_impl(&self.ctx, element)
    }

    /// [`Self::get_interface`] as a reference that lives as long as the
    /// element store (the cached interface is never replaced). An interface
    /// that is being computed (a cycle) is empty and is leaked.
    pub fn get_interface_ref(&self, element: EId<InterfaceElement>) -> &'a Interface {
        self.ctx.req.record(Requirement::InterfaceAll {
            element: element.raw(),
        });
        let cache = inheritance_cache(&self.ctx, element);
        if let Some(interface) = cache.interface.get() {
            return interface;
        }
        let interface = get_interface_impl(&self.ctx, element);
        match cache.interface.get() {
            Some(cached) if Arc::ptr_eq(cached, &interface) => cached,
            // Only the empty interface of a cycle is not cached.
            _ => Box::leak(Box::new(Interface::empty())),
        }
    }

    /// `getMember(element, name)` with the default options.
    pub fn get_member(&self, element: EId<InterfaceElement>, name: Name) -> Option<ElemRef> {
        self.get_member_with(element, name, GetMemberOptions::default())
    }

    /// `getMember(element, name, concrete:, forMixinIndex:, forSuper:)`.
    ///
    /// If [GetMemberOptions::concrete], the concrete implementation from the
    /// element or its superclass. If [GetMemberOptions::for_super], only
    /// concrete members from the superclass ([concrete] is implied). If
    /// [GetMemberOptions::for_mixin_index] is not negative, only the nominal
    /// superclass and that many mixins after it.
    pub fn get_member_with(
        &self,
        element: EId<InterfaceElement>,
        name: Name,
        options: GetMemberOptions,
    ) -> Option<ElemRef> {
        let interface = get_interface_impl(&self.ctx, element);

        let result = if options.for_super {
            if element.raw().is::<ExtensionTypeElement>() {
                return None;
            }
            let super_implemented = &interface.super_implemented;
            if options.for_mixin_index >= 0 {
                super_implemented[options.for_mixin_index as usize]
                    .get(&name)
                    .copied()
            } else if let Some(last) = super_implemented.last() {
                last.get(&name).copied()
            } else {
                debug_assert_eq!(self.ctx.element_name(element.raw()), Some("Object"));
                return None;
            }
        } else if options.concrete {
            interface.implemented.get(&name).copied()
        } else {
            interface.map.get(&name).copied()
        };

        self.ctx.req.record(Requirement::InterfaceGetMember {
            element: element.raw(),
            name: name.name,
            method_element: result.map(|e| member::base_element(&self.ctx, e)),
            concrete: options.concrete,
            for_super: options.for_super,
            for_mixin_index: options.for_mixin_index,
        });
        result
    }

    /// `getMember3(type, name)`: [`Self::get_member_with`] of the element of
    /// [t], substituted with the type arguments of [t].
    pub fn get_member3(&self, t: TypeId, name: Name, options: GetMemberOptions) -> Option<ElemRef> {
        let ctx = &self.ctx;
        let element = ctx.interface_element(t).expect("an interface type");
        let raw_element = self.get_member_with(element, name, options)?;
        let substitution = MapSubstitution::from_interface_type(ctx, t);
        Some(member::substitute(ctx, raw_element, &substitution))
    }

    /// `getOverridden(element, name)`: the members of mixins, superclasses
    /// and interfaces that a member [name] declared in [element] would
    /// override; `None` if none.
    pub fn get_overridden(
        &self,
        element: EId<InterfaceElement>,
        name: Name,
    ) -> Option<Vec<ElemRef>> {
        let interface = self.get_interface(element);
        interface.overridden.get(&name).cloned()
    }

    // `removeOfLibraries(uriSet)` is not ported: the caches live on the
    // elements, which are replaced as a whole when a library cycle is
    // relinked (design §4.2).
}

/// The [`InheritanceCache`] of [element].
fn inheritance_cache<'a>(ctx: &Ctx<'a>, element: EId<InterfaceElement>) -> &'a InheritanceCache {
    ctx.interface(element)
        .inheritance
        .get_or_init(InheritanceCache::default)
}

/// `_getInterface(element)`: the interface without dependency tracking.
fn get_interface_impl(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> Arc<Interface> {
    let cache = inheritance_cache(ctx, element);
    if let Some(interface) = cache.interface.get() {
        return interface.clone();
    }
    // Dart: `_interfaces[element] = Interface._empty;` and
    // `_processingClasses.add(element)`; a cycle sees the empty interface.
    if PROCESSING.with(|p| p.borrow().contains(&element)) {
        return Arc::new(Interface::empty());
    }
    PROCESSING.with(|p| p.borrow_mut().push(element));
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            PROCESSING.with(|p| p.borrow_mut().pop());
        }
    }
    let result = {
        let _pop = Pop;
        let mut computer = Computer::new(owner_ctx(ctx, element), element);
        if element.raw().is::<ExtensionTypeElement>() {
            computer.get_interface_extension_type()
        } else if element.raw().is::<MixinElement>() {
            computer.get_interface_mixin()
        } else {
            computer.get_interface_class()
        }
    };
    cache.interface.get_or_init(|| Arc::new(result)).clone()
}

/// The context to compute the interface of [element] in: `ctx.global()`,
/// without the cycle being linked when [element] is not in it. The
/// interface is cached on [element] and seen by every context that sees
/// [element], so the members that it synthesizes must not go to (and its
/// types must not mention) a cycle that is being linked and that [element]
/// does not belong to.
fn owner_ctx<'a>(ctx: &Ctx<'a>, element: EId<InterfaceElement>) -> Ctx<'a> {
    crate::type_ext::cache_ctx(ctx, |store| element.raw().store() == store)
}

// ------------------------------------------------------------------ computer

/// The computation of the interface of one element ([target]), with the
/// Dart manager state that belongs to it (`_combinedSignatures[target]`).
struct Computer<'a> {
    ctx: Ctx<'a>,
    type_system: TypeSystem<'a>,
    target: EId<InterfaceElement>,
    /// `_combinedSignatures[target]`.
    combined_signatures: NameListMap,
}

impl<'a> Computer<'a> {
    fn new(ctx: Ctx<'a>, target: EId<InterfaceElement>) -> Self {
        Computer {
            ctx,
            type_system: TypeSystem::new(ctx),
            target,
            combined_signatures: NameListMap::new(),
        }
    }

    fn get_interface(&self, element: EId<InterfaceElement>) -> Arc<Interface> {
        self.ctx.req.record(Requirement::InterfaceAll {
            element: element.raw(),
        });
        get_interface_impl(&self.ctx, element)
    }

    /// `_addCandidates`.
    fn add_candidates(
        &self,
        named_candidates: &mut NameListMap,
        substitution: &MapSubstitution,
        interface: &Interface,
    ) {
        // Optimization: Simple/common case of no substitution (handled by
        // `substitute`, which returns the element itself).
        for (name, &value) in &interface.map {
            let candidate = member::substitute(&self.ctx, value, substitution);
            named_candidates.entry(*name).or_default().push(candidate);
        }
    }

    /// `_addImplemented`.
    fn add_implemented(&self, implemented: &mut NameMap, element: EId<InterfaceElement>) {
        let ctx = &self.ctx;
        let library = ctx.element_data(element.raw()).and_then(|d| d.library);
        let mut add_member = |m: ElementId| {
            let member = ElemRef::Base(m);
            if !member::is_abstract(ctx, member) && !member::is_static(ctx, member) {
                if let Some(lookup_name) = member::lookup_name(ctx, member) {
                    let name = Name::new(ctx, library, &lookup_name);
                    implemented.insert(name, member);
                }
            }
        };
        let data = ctx.interface(element);
        data.methods.iter().for_each(|m| add_member(m.raw()));
        data.getters.iter().for_each(|m| add_member(m.raw()));
        data.setters.iter().for_each(|m| add_member(m.raw()));
    }

    /// `_addMixinMembers`.
    fn add_mixin_members(
        &self,
        implemented: &mut NameMap,
        substitution: &MapSubstitution,
        mixin: &Interface,
    ) {
        let ctx = &self.ctx;
        for (name, &executable) in &mixin.implemented {
            if member::is_abstract(ctx, executable) {
                continue;
            }
            if member::is_object_member(ctx, executable) {
                continue;
            }
            let executable = member::substitute(ctx, executable, substitution);
            implemented.insert(*name, executable);
        }
    }

    /// `_checkForGetterMethodConflict`.
    fn check_for_getter_method_conflict(
        &self,
        name: Name,
        candidates: &[ElemRef],
    ) -> Option<Conflict> {
        debug_assert!(candidates.len() > 1);
        let ctx = &self.ctx;
        let mut getter = None;
        let mut method = None;
        for &candidate in candidates {
            if member::is_getter(ctx, candidate) {
                getter.get_or_insert(candidate);
            } else if member::is_method(ctx, candidate) {
                method.get_or_insert(candidate);
            }
        }
        match (getter, method) {
            (Some(getter), Some(method)) => Some(Conflict::GetterMethod {
                name,
                getter,
                method,
            }),
            _ => None,
        }
    }

    /// `_combineSignatures`.
    fn combine_signatures(
        &mut self,
        candidates: &[ElemRef],
        name: Name,
        conflicts: Option<&mut Vec<Conflict>>,
    ) -> Option<ElemRef> {
        // If just one candidate, it is always valid.
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }
        self.combine_signatures_impl(candidates, name, conflicts)
    }

    /// `_combineSignaturesImpl`.
    fn combine_signatures_impl(
        &mut self,
        candidates: &[ElemRef],
        name: Name,
        conflicts: Option<&mut Vec<Conflict>>,
    ) -> Option<ElemRef> {
        let ctx = self.ctx;
        let valid_overrides = get_valid_overrides(&self.type_system, candidates);

        if valid_overrides.is_empty() {
            if let Some(conflicts) = conflicts {
                conflicts.push(Conflict::Candidates {
                    name,
                    candidates: candidates.to_vec(),
                });
            }
            return None;
        }

        // https://github.com/flutter/flutter/issues/178925#issuecomment-3573399510
        let target = Some(self.target.raw());
        let mut expanded_candidates = candidates.to_vec();
        if candidates
            .iter()
            .any(|&c| member::enclosing_element(&ctx, c) == target)
        {
            expanded_candidates = Vec::new();
            for &candidate in candidates {
                if member::enclosing_element(&ctx, candidate) == target {
                    if let Some(previous) = self.combined_signatures.get(&name) {
                        expanded_candidates.extend_from_slice(previous);
                        continue;
                    }
                }
                expanded_candidates.push(candidate);
            }
        }

        self.combined_signatures.insert(name, expanded_candidates);

        Some(self.top_merge(&valid_overrides))
    }

    /// `_findMostSpecificFromNamedCandidates`: for each name of
    /// [named_candidates] (candidates from direct superinterfaces), puts the
    /// most specific signature into [map] unless it has one (from the class
    /// itself). Returns the conflicts.
    fn find_most_specific_from_named_candidates(
        &mut self,
        map: &mut NameMap,
        named_candidates: &NameListMap,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        for (&name, candidates) in named_candidates {
            // There is no way to resolve the getter / method conflict.
            if candidates.len() > 1 {
                if let Some(conflict) = self.check_for_getter_method_conflict(name, candidates) {
                    conflicts.push(conflict);
                    continue;
                }
            }

            if map.contains_key(&name) {
                continue;
            }

            let combined_signature =
                self.combine_signatures(candidates, name, Some(&mut conflicts));

            if let Some(combined_signature) = combined_signature {
                map.insert(name, combined_signature);
                continue;
            }
        }

        conflicts
    }

    /// `_getInterfaceClass`.
    fn get_interface_class(&mut self) -> Interface {
        let ctx = self.ctx;
        let element = self.target;
        let mut named_candidates = NameListMap::new();
        let mut super_implemented: Vec<NameMap> = Vec::new();
        let mut implemented = NameMap::new();

        let super_type = ctx.element_supertype(element);

        let mut super_type_interface: Option<Arc<Interface>> = None;
        if let Some(super_type) = super_type {
            let substitution = MapSubstitution::from_interface_type(&ctx, super_type);
            let interface = self.get_interface(ctx.interface_element(super_type).unwrap());
            self.add_candidates(&mut named_candidates, &substitution, &interface);

            // Optimization: Simple/common case of no substitution (handled
            // by `substitute`).
            for (name, &executable) in &interface.implemented {
                implemented.insert(*name, member::substitute(&ctx, executable, &substitution));
            }

            super_implemented.push(implemented.clone());
            super_type_interface = Some(interface);
        }

        // Made nullable for the common case that there are no mixins or no
        // conflicts.
        let mut mixins_conflicts: Option<Vec<Vec<Conflict>>> = None;
        for &mixin in ctx.element_mixins(element) {
            let mixin_element = ctx.interface_element(mixin).unwrap();
            let substitution = MapSubstitution::from_interface_type(&ctx, mixin);
            let mixin_interface = self.get_interface(mixin_element);
            // `class X extends S with M1, M2 {}` is semantically a sequence of:
            //     class S&M1 extends S implements M1 {
            //       // declared M1 members
            //     }
            //     class S&M2 extends S&M1 implements M2 {
            //       // declared M2 members
            //     }
            //     class X extends S&M2 {
            //       // declared X members
            //     }
            // So, each mixin always replaces members in the interface.
            // And there are individual override conflicts for each mixin.
            let mut candidates_from_super_and_mixin = NameListMap::new();
            let mut mixin_conflicts = Vec::new();
            for (&name, &value) in &mixin_interface.map {
                let candidate = member::substitute(&ctx, value, &substitution);

                let Some(current_list) = named_candidates.get(&name) else {
                    named_candidates.insert(name, vec![candidate]);
                    continue;
                };

                debug_assert_eq!(current_list.len(), 1, "currentList.single");
                let current = current_list[0];
                if member::enclosing_element(&ctx, candidate) == Some(mixin_element.raw()) {
                    named_candidates.insert(name, vec![candidate]);
                    // Dart: current.kind != candidate.kind
                    if member::base_element(&ctx, current).tag()
                        != member::base_element(&ctx, candidate).tag()
                    {
                        let current_is_getter = member::is_getter(&ctx, current);
                        mixin_conflicts.push(Conflict::GetterMethod {
                            name,
                            getter: if current_is_getter {
                                current
                            } else {
                                candidate
                            },
                            method: if current_is_getter {
                                candidate
                            } else {
                                current
                            },
                        });
                    }
                } else {
                    candidates_from_super_and_mixin.insert(name, vec![current, candidate]);
                }
            }

            // Merge members from the superclass and the mixin interface.
            {
                let mut map = NameMap::new();
                self.find_most_specific_from_named_candidates(
                    &mut map,
                    &candidates_from_super_and_mixin,
                );
                for (name, value) in map {
                    named_candidates.insert(name, vec![value]);
                }
            }

            mixins_conflicts
                .get_or_insert_with(Vec::new)
                .push(mixin_conflicts);

            self.add_mixin_members(&mut implemented, &substitution, &mixin_interface);

            super_implemented.push(implemented.clone());
        }

        for &interface in ctx.element_interfaces(element) {
            let substitution = MapSubstitution::from_interface_type(&ctx, interface);
            let interface = self.get_interface(ctx.interface_element(interface).unwrap());
            self.add_candidates(&mut named_candidates, &substitution, &interface);
        }

        self.add_implemented(&mut implemented, element);

        // If a class declaration has a member declaration, the signature of
        // that member declaration becomes the signature in the interface.
        let declared = get_type_members(&ctx, element);

        // If a class declaration does not have a member declaration with a
        // particular name, but some super-interfaces do have a member with
        // that name, it's a compile-time error if there is no signature
        // among the super-interfaces that is a valid override of all the
        // other super-interface signatures with the same name. That "most
        // specific" signature becomes the signature of the class's
        // interface.
        let mut interface = declared.clone();
        let mut conflicts =
            self.find_most_specific_from_named_candidates(&mut interface, &named_candidates);

        let mut no_such_method_forwarders: Option<IndexSet<Name>> = None;
        let is_abstract_class = element.raw().is::<ClassElement>()
            && ctx
                .element_data(element.raw())
                .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT));
        if is_abstract_class {
            if let Some(super_type_interface) = &super_type_interface {
                no_such_method_forwarders =
                    Some(super_type_interface.no_such_method_forwarders.clone());
            }
        } else {
            let no_such_method = implemented.get(&self.no_such_method_name()).copied();
            if let Some(no_such_method) = no_such_method
                && !member::is_object_member(&ctx, no_such_method)
            {
                let super_forwarders = super_type_interface
                    .as_ref()
                    .map(|i| &i.no_such_method_forwarders);
                for (&name, &value) in &interface {
                    if !implemented.contains_key(&name)
                        || super_forwarders.is_some_and(|f| f.contains(&name))
                    {
                        implemented.insert(name, value);
                        no_such_method_forwarders
                            .get_or_insert_with(IndexSet::new)
                            .insert(name);
                    }
                }
            }
        }

        // TODO(scheglov): Instead of merging conflicts we could report them on
        // the corresponding mixins applied in the class.
        if let Some(mixins_conflicts) = mixins_conflicts {
            for mixin_conflicts in mixins_conflicts {
                conflicts.extend(mixin_conflicts);
            }
        }

        let names: Vec<Name> = implemented.keys().copied().collect();
        for name in names {
            let value = implemented[&name];
            let result = self.inherit_covariance(&named_candidates, name, value);
            // Dart: !identical(entry.value, result)
            if result != value {
                implemented.insert(name, result);
            }
        }

        Interface {
            map: interface,
            declared,
            implemented,
            no_such_method_forwarders: no_such_method_forwarders.unwrap_or_default(),
            overridden: named_candidates,
            redeclared: NameListMap::new(),
            super_implemented,
            conflicts,
            combined_signatures: std::mem::take(&mut self.combined_signatures),
            inherited_map: OnceLock::new(),
        }
    }

    /// `_noSuchMethodName`.
    fn no_such_method_name(&self) -> Name {
        Name::new(&self.ctx, None, "noSuchMethod")
    }

    /// `_getInterfaceExtensionType`. See
    /// https://github.com/dart-lang/language/blob/main/accepted/future-releases/extension-types/feature-specification.md#static-analysis-of-an-extension-type-member-invocation
    ///
    /// We handle "has an extension type member" and "has a non-extension
    /// type member" portions, considering redeclaration and conflicts.
    fn get_interface_extension_type(&mut self) -> Interface {
        let ctx = self.ctx;
        let element = self.target;

        // Add instance members implemented by the element itself.
        let mut declared = NameMap::new();
        self.add_implemented(&mut declared, element);

        // Prepare precluded names.
        let mut precluded_names = IndexSet::new();
        let mut precluded_methods = IndexSet::new();
        let mut precluded_setters = IndexSet::new();
        for (&name, &value) in &declared {
            precluded_names.insert(name);
            if member::is_method(&ctx, value) {
                precluded_setters.insert(name.for_setter(&ctx));
            } else if member::is_setter(&ctx, value) {
                precluded_methods.insert(name.for_getter(&ctx));
            }
        }
        let precluded = Precluded {
            names: &precluded_names,
            methods: &precluded_methods,
            setters: &precluded_setters,
        };

        // These declared members take precedence over "inherited" ones.
        let mut implemented = declared.clone();

        // Prepare candidates for inheritance.
        let mut extension_candidates: IndexMap<Name, ExtensionTypeCandidates> = IndexMap::new();
        let mut not_extension_candidates: IndexMap<Name, ExtensionTypeCandidates> = IndexMap::new();
        for &interface in ctx.element_interfaces(element) {
            let substitution = MapSubstitution::from_interface_type(&ctx, interface);
            let interface_obj = self.get_interface(ctx.interface_element(interface).unwrap());
            for (&name, &value) in &interface_obj.map {
                let executable = member::substitute(&ctx, value, &substitution);
                let target = if member::is_extension_type_member(&ctx, executable) {
                    &mut extension_candidates
                } else {
                    &mut not_extension_candidates
                };
                target
                    .entry(name)
                    .or_insert_with(|| ExtensionTypeCandidates::new(name))
                    .add(&ctx, executable);
            }
        }

        let mut redeclared = NameListMap::new();
        let mut conflicts = Vec::new();

        // Add extension type members.
        for (&name, candidates) in &extension_candidates {
            redeclared.entry(name).or_default().extend(candidates.all());

            let not_precluded = candidates.not_precluded(&precluded);

            // Stop if all precluded.
            if not_precluded.is_empty() {
                continue;
            }

            // If not precluded, can have either non-extension, or extension.
            if let Some(non_extension_signatures) = not_extension_candidates.get(&name) {
                let not_extension_not_precluded =
                    non_extension_signatures.not_precluded(&precluded);
                if !not_extension_not_precluded.is_empty() {
                    conflicts.push(Conflict::HasNonExtensionAndExtensionMember {
                        name,
                        non_extension: not_extension_not_precluded,
                        extension: not_precluded,
                    });
                }
                continue;
            }

            // The inherited member must be unique.
            let mut unique_element: Option<ElemRef> = None;
            for &candidate in &not_precluded {
                match unique_element {
                    None => unique_element = Some(candidate),
                    Some(unique)
                        if member::base_element(&ctx, unique)
                            != member::base_element(&ctx, candidate) =>
                    {
                        unique_element = None;
                        break;
                    }
                    Some(_) => {}
                }
            }

            let Some(unique_element) = unique_element else {
                conflicts.push(Conflict::NotUniqueExtensionMember {
                    name,
                    candidates: not_precluded,
                });
                continue;
            };

            implemented.insert(name, unique_element);
        }

        // Add non-extension type members.
        for (&name, candidates) in &not_extension_candidates {
            redeclared.entry(name).or_default().extend(candidates.all());

            let not_precluded = candidates.not_precluded(&precluded);

            // Stop if all precluded.
            if not_precluded.is_empty() {
                continue;
            }

            // Skip, if also has extension candidates.
            // The conflict is already reported.
            if extension_candidates.contains_key(&name) {
                continue;
            }

            let combined_signature = self.combine_signatures(&not_precluded, name, None);

            let Some(combined_signature) = combined_signature else {
                conflicts.push(Conflict::Candidates {
                    name,
                    candidates: not_precluded,
                });
                continue;
            };

            implemented.insert(name, combined_signature);
        }

        // Check for conflicting static and instance members.
        let data = ctx.interface(element);
        let members: Vec<ElemRef> = data
            .methods
            .iter()
            .map(|m| ElemRef::Base(m.raw()))
            .chain(data.getters.iter().map(|m| ElemRef::Base(m.raw())))
            .chain(data.setters.iter().map(|m| ElemRef::Base(m.raw())))
            .collect();
        for member in members {
            if !member::is_static(&ctx, member) {
                continue;
            }

            let Some(getter_name) = Name::for_element(&ctx, member).map(|n| n.for_getter(&ctx))
            else {
                continue;
            };

            if let Some(&inherited) = redeclared.get(&getter_name).and_then(|l| l.first()) {
                conflicts.push(Conflict::ExtensionTypeConflictingStaticAndInstance {
                    name: getter_name,
                    declared: member,
                    inherited,
                });
                continue;
            }

            let setter_name = getter_name.for_setter(&ctx);
            if let Some(&inherited) = redeclared.get(&setter_name).and_then(|l| l.first()) {
                conflicts.push(Conflict::ExtensionTypeConflictingStaticAndInstance {
                    name: setter_name,
                    declared: member,
                    inherited,
                });
            }
        }

        // Inherited method and setter with the same name.
        for (&method_name, candidates) in &redeclared {
            if method_name.is_setter(&ctx) {
                continue;
            }

            // We are looking for a conflict between an inherited method and an
            // inherited setter. If either is precluded by a declaration in the
            // extension type, there is no conflict.
            let setter_name = method_name.for_setter(&ctx);
            if precluded_names.contains(&method_name)
                || precluded_methods.contains(&method_name)
                || precluded_names.contains(&setter_name)
                || precluded_setters.contains(&setter_name)
            {
                continue;
            }

            // Choose any method.
            let Some(&method) = candidates.iter().find(|&&e| member::is_method(&ctx, e)) else {
                continue;
            };

            // Choose any corresponding setter.
            let Some(&setter) = redeclared
                .get(&setter_name)
                .and_then(|l| l.iter().find(|&&e| member::is_setter(&ctx, e)))
            else {
                continue;
            };

            conflicts.push(Conflict::ExtensionTypeConflictingInheritedMethodAndSetter {
                name: method_name,
                method,
                setter,
            });
        }

        // Dart: `elements.toSet()`. Members are compared by identity in Dart
        // and by (base, substitution) in Rust; see `member`.
        let mut unique_redeclared = NameListMap::new();
        for (name, elements) in redeclared {
            if elements.len() == 1 {
                unique_redeclared.insert(name, elements);
            } else {
                let set: IndexSet<ElemRef> = elements.into_iter().collect();
                unique_redeclared.insert(name, set.into_iter().collect());
            }
        }

        Interface {
            map: implemented.clone(),
            declared,
            implemented,
            no_such_method_forwarders: IndexSet::new(),
            overridden: NameListMap::new(),
            redeclared: unique_redeclared,
            super_implemented: Vec::new(),
            conflicts,
            combined_signatures: std::mem::take(&mut self.combined_signatures),
            inherited_map: OnceLock::new(),
        }
    }

    /// `_getInterfaceMixin`.
    fn get_interface_mixin(&mut self) -> Interface {
        let ctx = self.ctx;
        let element = self.target;

        let mut super_candidates = NameListMap::new();
        for &constraint in ctx.element_superclass_constraints(element) {
            let substitution = MapSubstitution::from_interface_type(&ctx, constraint);
            let interface_obj = self.get_interface(ctx.interface_element(constraint).unwrap());
            self.add_candidates(&mut super_candidates, &substitution, &interface_obj);
        }

        // `mixin M on S1, S2 {}` can call using `super` any instance member
        // from its superclass constraints, whether it is abstract or concrete.
        let mut super_interface = NameMap::new();
        let super_conflicts =
            self.find_most_specific_from_named_candidates(&mut super_interface, &super_candidates);

        let mut interface_candidates = super_candidates.clone();
        for &interface in ctx.element_interfaces(element) {
            let substitution = MapSubstitution::from_interface_type(&ctx, interface);
            let interface_obj = self.get_interface(ctx.interface_element(interface).unwrap());
            self.add_candidates(&mut interface_candidates, &substitution, &interface_obj);
        }

        let declared = get_type_members(&ctx, element);

        let mut interface = declared.clone();
        let interface_conflicts =
            self.find_most_specific_from_named_candidates(&mut interface, &interface_candidates);

        let mut implemented = NameMap::new();
        self.add_implemented(&mut implemented, element);

        let mut conflicts = super_conflicts;
        conflicts.extend(interface_conflicts);

        Interface {
            map: interface,
            declared,
            implemented,
            no_such_method_forwarders: IndexSet::new(),
            overridden: interface_candidates,
            redeclared: NameListMap::new(),
            super_implemented: vec![super_interface],
            conflicts,
            combined_signatures: std::mem::take(&mut self.combined_signatures),
            inherited_map: OnceLock::new(),
        }
    }

    /// `_inheritCovariance`: if a candidate from [named_candidates] has
    /// covariant parameters, returns a copy of [executable] with the
    /// corresponding parameters marked covariant. If there are no covariant
    /// parameters, or the parameters are already covariant, returns
    /// [executable] itself.
    fn inherit_covariance(
        &self,
        named_candidates: &NameListMap,
        name: Name,
        executable: ElemRef,
    ) -> ElemRef {
        let ctx = self.ctx;
        let class = self.target;
        if member::enclosing_element(&ctx, executable) == Some(class.raw()) {
            return executable;
        }

        let parameters = member::formal_parameters(&ctx, executable);
        if parameters.is_empty() {
            return executable;
        }

        let Some(candidates) = named_candidates.get(&name) else {
            return executable;
        };

        // Find parameters that are covariant (by declaration) in any overridden.
        let mut covariant_parameters: Option<IndexSet<ParameterDesc>> = None;
        for &candidate in candidates {
            let parameters = member::formal_parameters(&ctx, candidate);
            for (i, &parameter) in parameters.iter().enumerate() {
                if member::is_covariant(&ctx, parameter) {
                    covariant_parameters
                        .get_or_insert_with(IndexSet::new)
                        .insert(ParameterDesc::new(&ctx, i, parameter));
                }
            }
        }

        let Some(covariant_parameters) = covariant_parameters else {
            return executable;
        };

        // Update covariance of the parameters of the chosen executable.
        let mut transformed_parameters: Option<Vec<ParameterSpec>> = None;
        for (index, &parameter) in parameters.iter().enumerate() {
            let should_be_covariant =
                covariant_parameters.contains(&ParameterDesc::new(&ctx, index, parameter));
            if member::is_covariant(&ctx, parameter) != should_be_covariant {
                let transformed = transformed_parameters.get_or_insert_with(|| {
                    parameters
                        .iter()
                        .map(|&p| ParameterSpec::of_base_element(&ctx, p))
                        .collect()
                });
                transformed[index] = ParameterSpec {
                    covariant: should_be_covariant,
                    ..ParameterSpec::of_base_element(&ctx, parameter)
                };
            }
        }

        let Some(transformed_parameters) = transformed_parameters else {
            return executable;
        };

        if member::is_method(&ctx, executable) {
            let fragment_name = member::name(&ctx, executable).unwrap_or("").to_string();
            let type_parameters = member::type_parameters(&ctx, executable);
            let return_type = member::return_type(&ctx, executable);
            return self.get_or_create(SynthesizedKind::Method, &fragment_name, |s| {
                s.synthesize_method(
                    &fragment_name,
                    &type_parameters,
                    &transformed_parameters,
                    return_type,
                )
            });
        }

        // Dart: `executable is SetterElementImpl` (not a substituted setter).
        if let ElemRef::Base(base) = executable
            && base.tag() == Tag::Setter
        {
            let fragment_name = member::name(&ctx, executable).unwrap_or("").to_string();
            let return_type = member::return_type(&ctx, executable);
            return self.get_or_create(SynthesizedKind::Setter, &fragment_name, |s| {
                let setter = s.synthesize_accessor(
                    SynthesizedKind::Setter,
                    &fragment_name,
                    &transformed_parameters,
                    return_type,
                    true,
                );
                // Dart creates a field under the class reference but does not
                // link it to the setter.
                s.synthesize_field(&fragment_name, None);
                setter
            });
        }

        executable
    }

    /// `_topMerge`: merges [valid_overrides] (one or more) into a single
    /// signature. This signature always exists.
    fn top_merge(&self, valid_overrides: &[ElemRef]) -> ElemRef {
        let ctx = self.ctx;
        let first_element = valid_overrides[0];

        if valid_overrides.len() == 1 {
            return first_element;
        }

        // Dart: e.type != firstType
        let first_type = member::type_(&ctx, first_element);
        let all_first_type = valid_overrides
            .iter()
            .all(|&e| ctx.dart_eq(member::type_(&ctx, e), first_type));
        if all_first_type {
            return first_element;
        }

        let result_type = top_merge_signature_types(&self.type_system, valid_overrides);

        for &executable in valid_overrides {
            // Dart: executable.type == resultType
            if ctx.dart_eq(member::type_(&ctx, executable), result_type) {
                return executable;
            }
        }

        let TypeKind::Function(result) = *ctx.ty(result_type) else {
            unreachable!("topMerge of function types");
        };
        let result_type_parameters = ctx.list(result.type_params).to_vec();
        let result_parameters: Vec<ParameterSpec> = ctx
            .list(result.params)
            .iter()
            .map(|p| ParameterSpec {
                name: p.name,
                ty: p.ty,
                kind: p.kind,
                covariant: p.covariant,
                base: p
                    .element
                    .map(|e| member::base_element(&ctx, e))
                    .and_then(|e| e.cast::<FormalParameterElement>()),
            })
            .collect();

        let fragment_name = member::name(&ctx, first_element).unwrap_or("").to_string();
        if member::is_method(&ctx, first_element) {
            self.get_or_create(SynthesizedKind::Method, &fragment_name, |s| {
                s.synthesize_method(
                    &fragment_name,
                    &result_type_parameters,
                    &result_parameters,
                    result.ret,
                )
            })
        } else {
            let is_getter = member::is_getter(&ctx, first_element);
            let kind = if is_getter {
                SynthesizedKind::Getter
            } else {
                SynthesizedKind::Setter
            };
            self.get_or_create(kind, &fragment_name, |s| {
                let accessor = s.synthesize_accessor(
                    kind,
                    &fragment_name,
                    &result_parameters,
                    result.ret,
                    !is_getter,
                );
                let field_type = if is_getter {
                    member::return_type(&ctx, ElemRef::Base(accessor))
                } else {
                    let executable = accessor.cast::<ExecutableElement>().unwrap();
                    dartr_element::type_inference::ensure_accessor_return_type(&ctx, executable);
                    let parameter = ctx.executable(executable).formal_params[0];
                    ctx.get(parameter).type_.get().unwrap_or(TypeId::INVALID)
                };
                let field = s.synthesize_field(&fragment_name, Some(field_type));
                ctx.property_accessor(accessor.cast::<PropertyAccessorElement>().unwrap())
                    .variable
                    .set(Some(field.upcast::<PropertyInducingElement>()));
                accessor
            })
        }
    }

    // -------------------------------------------------------------- synthesis

    /// `targetClass.reference.getOrCreate{Method,Getter,Setter}(name)`: the
    /// declared member of the target with this name, or the member that an
    /// earlier merge synthesized, or a new one made by [create].
    fn get_or_create(
        &self,
        kind: SynthesizedKind,
        name: &str,
        create: impl FnOnce(&Synthesizer<'a>) -> ElementId,
    ) -> ElemRef {
        let ctx = self.ctx;
        let target = self.target;
        let data = ctx.interface(target);
        let ids: Vec<ElementId> = match kind {
            SynthesizedKind::Method => data.methods.iter().map(|e| e.raw()).collect(),
            SynthesizedKind::Getter => data.getters.iter().map(|e| e.raw()).collect(),
            SynthesizedKind::Setter => data.setters.iter().map(|e| e.raw()).collect(),
        };
        let existing = ids.into_iter().find(|&e| ctx.element_name(e) == Some(name));
        if let Some(existing) = existing {
            return ElemRef::Base(existing);
        }
        let cache = inheritance_cache(&ctx, target);
        let mut synthesized = cache.synthesized.lock().unwrap();
        let key = (kind, ctx.name(name));
        if let Some(&existing) = synthesized.get(&key) {
            return ElemRef::Base(existing);
        }
        let created = create(&Synthesizer { ctx, target });
        synthesized.insert(key, created);
        ElemRef::Base(created)
    }
}

/// The precluded names of an extension type.
struct Precluded<'p> {
    names: &'p IndexSet<Name>,
    methods: &'p IndexSet<Name>,
    setters: &'p IndexSet<Name>,
}

/// `_ExtensionTypeCandidates`.
struct ExtensionTypeCandidates {
    name: Name,
    methods: Vec<ElemRef>,
    getters: Vec<ElemRef>,
    setters: Vec<ElemRef>,
}

impl ExtensionTypeCandidates {
    fn new(name: Name) -> Self {
        ExtensionTypeCandidates {
            name,
            methods: Vec::new(),
            getters: Vec::new(),
            setters: Vec::new(),
        }
    }

    /// `all`.
    fn all(&self) -> Vec<ElemRef> {
        let mut all = self.methods.clone();
        all.extend_from_slice(&self.getters);
        all.extend_from_slice(&self.setters);
        all
    }

    /// `add(element)`.
    fn add(&mut self, ctx: &Ctx<'_>, element: ElemRef) {
        if member::is_method(ctx, element) {
            self.methods.push(element);
        } else if member::is_getter(ctx, element) {
            self.getters.push(element);
        } else if member::is_setter(ctx, element) {
            self.setters.push(element);
        }
    }

    /// `notPrecluded(...)`.
    fn not_precluded(&self, precluded: &Precluded<'_>) -> Vec<ElemRef> {
        if precluded.names.contains(&self.name) {
            return Vec::new();
        }
        let mut result = Vec::new();
        if !precluded.methods.contains(&self.name) {
            result.extend_from_slice(&self.methods);
        }
        result.extend_from_slice(&self.getters);
        if !precluded.setters.contains(&self.name) {
            result.extend_from_slice(&self.setters);
        }
        result
    }
}

/// `_ParameterDesc`: a parameter by its index (positional) or its name
/// (named).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ParameterDesc {
    Index(usize),
    Name(Option<dartr_element::Name>),
}

impl ParameterDesc {
    fn new(ctx: &Ctx<'_>, index: usize, element: ElemRef) -> Self {
        let base = member::base_element(ctx, element);
        let data = ctx.get(EId::<FormalParameterElement>::from_raw(base));
        if is_named(data.kind) {
            ParameterDesc::Name(data.name)
        } else {
            ParameterDesc::Index(index)
        }
    }
}

/// A formal parameter to create (`FormalParameterElementImpl.synthetic` /
/// `copyWith`).
#[derive(Clone, Copy, Debug)]
struct ParameterSpec {
    name: Option<dartr_element::Name>,
    ty: TypeId,
    kind: ParameterKind,
    covariant: bool,
    /// `baseFormalParameter`.
    base: Option<EId<FormalParameterElement>>,
}

impl ParameterSpec {
    /// `parameter.baseElement` (its declared type, kind and covariance).
    fn of_base_element(ctx: &Ctx<'_>, parameter: ElemRef) -> Self {
        let base = EId::<FormalParameterElement>::from_raw(member::base_element(ctx, parameter));
        dartr_element::type_inference::ensure_formal_parameter_type(ctx, base);
        let data = ctx.get(base);
        ParameterSpec {
            name: data.name,
            ty: data.type_.get().unwrap_or(TypeId::INVALID),
            kind: data.kind,
            covariant: data
                .flags
                .has(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT),
            base: Some(base),
        }
    }
}

/// Creates the members of `_topMerge` / `_inheritCovariance` in
/// `fresh_store(ctx)`, enclosed by [target].
///
/// Children (type parameters, formal parameters) have no enclosing
/// element: the Rust store cannot name the id of the parent before it is
/// added. Display strings and types do not read it.
struct Synthesizer<'a> {
    ctx: Ctx<'a>,
    target: EId<InterfaceElement>,
}

impl Synthesizer<'_> {
    fn target_fragment(&self) -> dartr_element::FragmentId {
        self.ctx
            .element_data(self.target.raw())
            .unwrap()
            .first_fragment
    }

    fn element_data(
        &self,
        name: Option<dartr_element::Name>,
        fragment: dartr_element::FragmentId,
    ) -> ElementData {
        let mut data = ElementData::new(name, fragment);
        data.library = self
            .ctx
            .element_data(self.target.raw())
            .and_then(|d| d.library);
        data.enclosing = Some(self.target.raw());
        data
    }

    fn fragment_data(&self, name: Option<dartr_element::Name>) -> FragmentData {
        let mut f = FragmentData::new(name, None);
        f.enclosing_fragment = Some(self.target_fragment());
        f
    }

    /// `_FreshExecutableTypeParameters(source)` with `initializeElements`:
    /// synthetic type parameters with the names of [source], and the
    /// substitution from [source] to them.
    fn fresh_type_parameters(
        &self,
        source: &[EId<TypeParameterElement>],
    ) -> (
        Vec<FId<TypeParameterFragment>>,
        Vec<EId<TypeParameterElement>>,
        MapSubstitution,
    ) {
        let ctx = &self.ctx;
        let store = fresh_store(ctx);
        let mut fragments = Vec::new();
        let mut elements = Vec::new();
        for &p in source {
            let name = ctx.get(p).name.or(Some(ctx.name("")));
            let fragment = store.add_fragment::<TypeParameterFragment>(TypeParameterFragment {
                fragment: FragmentData::new(name, None),
            });
            let mut element = TypeParameterElement::new(ElementData::new(name, fragment.raw()));
            // Dart (`initializeElements`): `if (!isLegacyCovariant)
            // target.variance = source.variance`; `None` is legacy covariant.
            element.variance = ctx.get(p).variance;
            let element: EId<TypeParameterElement> = store.add(element);
            store.fragment(fragment).element.set_once(element.raw());
            fragments.push(fragment);
            elements.push(element);
        }
        let types: Vec<TypeId> = elements
            .iter()
            .map(|&e| ctx.type_parameter_type(e, Nullability::None))
            .collect();
        let substitution = MapSubstitution::from_pairs(source, &types);
        for (i, &source_parameter) in source.iter().enumerate() {
            let target = ctx.get(elements[i]);
            let source_data = ctx.get(source_parameter);
            if let Some(default_type) = source_data.default_type.get() {
                target
                    .default_type
                    .set(Some(substitution.substitute_type(ctx, default_type)));
            }
            if let Some(bound) = source_data.bound.get() {
                target
                    .bound
                    .set(Some(substitution.substitute_type(ctx, bound)));
            }
        }
        (fragments, elements, substitution)
    }

    /// `freshFormalParameterElements(formalParameters)`:
    /// `copyWith(type: substitute(type))` of each parameter.
    fn fresh_formal_parameters(
        &self,
        parameters: &[ParameterSpec],
        substitution: &MapSubstitution,
    ) -> (
        Vec<FId<FormalParameterFragment>>,
        Vec<EId<FormalParameterElement>>,
    ) {
        let ctx = &self.ctx;
        let store = fresh_store(ctx);
        let mut fragments = Vec::new();
        let mut elements = Vec::new();
        for p in parameters {
            let fd = FragmentData::new(p.name, None);
            let fragment = store.add_fragment::<FormalParameterFragment>(FormalParameterFragment {
                variable: VariableFragmentData::new(fd),
                parameter_kind: p.kind,
                private_name: None,
            });
            let mut data = ElementData::new(p.name, fragment.raw());
            data.library = ctx.element_data(self.target.raw()).and_then(|d| d.library);
            data.flags.set(
                ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT,
                p.covariant,
            );
            let element: EId<FormalParameterElement> = store.add(FormalParameterElement {
                variable: VariableElementData::new(data),
                kind: p.kind,
                type_: VarSlot::with(substitution.substitute_type(ctx, p.ty)),
                base_formal_parameter: p.base,
                field: VarSlot::new(),
            });
            store.fragment(fragment).element.set_once(element.raw());
            fragments.push(fragment);
            elements.push(element);
        }
        (fragments, elements)
    }

    /// A synthetic `MethodElementImpl` with `isOriginInterface`.
    fn synthesize_method(
        &self,
        name: &str,
        type_parameters: &[EId<TypeParameterElement>],
        parameters: &[ParameterSpec],
        return_type: TypeId,
    ) -> ElementId {
        let ctx = &self.ctx;
        let store = fresh_store(ctx);
        let name = Some(ctx.name(name));
        let (tp_fragments, tp_elements, substitution) = self.fresh_type_parameters(type_parameters);
        let (p_fragments, p_elements) = self.fresh_formal_parameters(parameters, &substitution);

        let fd = self.fragment_data(name);
        fd.flags
            .set(FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_INTERFACE, true);
        let mut executable_fragment = ExecutableFragmentData::new(fd);
        executable_fragment.type_params = tp_fragments;
        executable_fragment.formal_params = p_fragments;
        let fragment = store.add_fragment::<MethodFragment>(MethodFragment {
            executable: executable_fragment,
        });

        let mut executable = ExecutableElementData::new(self.element_data(name, fragment.raw()));
        executable.type_params = tp_elements;
        executable.formal_params = p_elements;
        executable
            .return_type
            .set(Some(substitution.substitute_type(ctx, return_type)));
        let element: EId<MethodElement> = store.add(MethodElement {
            executable,
            is_operator_equal_with_parameter_type_from_object: BoolSlot::new(false),
            type_inference_error: OnceSlot::new(),
        });
        store.fragment(fragment).element.set_once(element.raw());
        element.raw()
    }

    /// A synthetic `GetterElementImpl` / `SetterElementImpl`.
    fn synthesize_accessor(
        &self,
        kind: SynthesizedKind,
        name: &str,
        parameters: &[ParameterSpec],
        return_type: TypeId,
        is_origin_interface: bool,
    ) -> ElementId {
        let ctx = &self.ctx;
        let store = fresh_store(ctx);
        let name = Some(ctx.name(name));
        let substitution = MapSubstitution::empty();
        let (p_fragments, p_elements) = self.fresh_formal_parameters(parameters, &substitution);

        let fd = self.fragment_data(name);
        fd.flags.set(
            FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_INTERFACE,
            is_origin_interface,
        );
        let mut executable_fragment = ExecutableFragmentData::new(fd);
        executable_fragment.formal_params = p_fragments;
        let accessor = PropertyAccessorFragmentData {
            executable: executable_fragment,
            inducing_variable: None,
        };
        let fragment = match kind {
            SynthesizedKind::Getter => store
                .add_fragment::<GetterFragment>(GetterFragment { accessor })
                .raw(),
            SynthesizedKind::Setter => store
                .add_fragment::<SetterFragment>(SetterFragment { accessor })
                .raw(),
            SynthesizedKind::Method => unreachable!(),
        };

        let mut executable = ExecutableElementData::new(self.element_data(name, fragment));
        executable.formal_params = p_elements;
        executable.return_type.set(Some(return_type));
        let accessor = PropertyAccessorElementData {
            executable,
            variable: VarSlot::new(),
        };
        let element = match kind {
            SynthesizedKind::Getter => store.add::<GetterElement>(GetterElement { accessor }).raw(),
            SynthesizedKind::Setter => store.add::<SetterElement>(SetterElement { accessor }).raw(),
            SynthesizedKind::Method => unreachable!(),
        };
        ctx.fragment_data(fragment)
            .unwrap()
            .element
            .set_once(element);
        element
    }

    /// A synthetic `FieldElementImpl` with `isOriginGetterSetter`.
    fn synthesize_field(&self, name: &str, ty: Option<TypeId>) -> EId<FieldElement> {
        let ctx = &self.ctx;
        let store = fresh_store(ctx);
        let name = Some(ctx.name(name));
        let fd = self.fragment_data(name);
        fd.flags.set(
            FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER,
            true,
        );
        let fragment = store.add_fragment::<FieldFragment>(FieldFragment {
            property: PropertyInducingFragmentData {
                variable: VariableFragmentData::new(fd),
                induced_getter: None,
                induced_setter: None,
            },
            inherits_covariant: BoolSlot::new(false),
        });
        let element: EId<FieldElement> = store.add(FieldElement {
            property: PropertyInducingElementData::new(self.element_data(name, fragment.raw())),
        });
        if let Some(ty) = ty {
            ctx.get(element).type_.set(Some(ty));
        }
        store.fragment(fragment).element.set_once(element.raw());
        element
    }
}

/// `_getTypeMembers(element)`: the instance members declared in [element].
fn get_type_members(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> NameMap {
    let mut declared = NameMap::new();
    let library = ctx.element_data(element.raw()).and_then(|d| d.library);

    let mut add_member = |m: ElementId| {
        let member = ElemRef::Base(m);
        if !member::is_static(ctx, member) {
            if let Some(lookup_name) = member::lookup_name(ctx, member) {
                let name = Name::new(ctx, library, &lookup_name);
                declared.insert(name, member);
            }
        }
    };

    let data = ctx.interface(element);
    data.methods.iter().for_each(|m| add_member(m.raw()));
    data.getters.iter().for_each(|m| add_member(m.raw()));
    data.setters.iter().for_each(|m| add_member(m.raw()));

    declared
}

/// `_getValidOverrides`: the executables that are valid overrides of all
/// [candidates].
fn get_valid_overrides(type_system: &TypeSystem<'_>, candidates: &[ElemRef]) -> Vec<ElemRef> {
    let ctx = &type_system.ctx;
    let types: Vec<TypeId> = candidates.iter().map(|&c| member::type_(ctx, c)).collect();
    let mut valid_overrides = Vec::new();
    'outer: for (i, &valid_override) in candidates.iter().enumerate() {
        let valid_override_type = types[i];
        for &candidate_type in &types {
            if !type_system.is_subtype_of(valid_override_type, candidate_type) {
                continue 'outer;
            }
        }
        valid_overrides.push(valid_override);
    }
    valid_overrides
}

/// `_topMergeSignatureTypes`.
fn top_merge_signature_types(type_system: &TypeSystem<'_>, valid_overrides: &[ElemRef]) -> TypeId {
    let ctx = &type_system.ctx;
    valid_overrides
        .iter()
        .map(|&e| type_system.normalize_function_type(member::type_(ctx, e)))
        .reduce(|previous, next| type_system.top_merge(previous, next))
        .unwrap()
}
