// Dart source: pkg/analyzer/lib/src/dart/element/class_hierarchy.dart

//! `ClassHierarchy`: `InterfaceElementImpl.allSupertypes` and the
//! hierarchy errors (`IncompatibleInterfacesClassHierarchyError`).
//!
//! Unit A3 needs `allSupertypes` for subtyping, so the file is ported here;
//! unit A7 (inheritance) uses it from this crate.
//!
//! The Dart `ClassHierarchy` keeps a map per analysis session. Rust caches
//! the interfaces in `InterfaceElementData.all_supertypes` (a `OnceSlot`,
//! filled under `ctx.global()`, design §2.3). The errors are not cached:
//! [`errors`] computes them again (they are only read by verifiers).

use std::cell::RefCell;

use dartr_element::{ClassElement, Ctx, EId, InterfaceElement, Nullability, TypeId};
use indexmap::IndexMap;

use crate::type_algebra::MapSubstitution;
use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;

/// `ClassHierarchyError` (`IncompatibleInterfacesClassHierarchyError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncompatibleInterfacesError {
    pub first: TypeId,
    pub second: TypeId,
}

thread_local! {
    /// Elements whose hierarchy is being computed on this thread. Dart puts
    /// an empty `_Hierarchy` in the map before computing, so a cycle sees
    /// no interfaces; this stack gives the same result.
    static IN_PROGRESS: RefCell<Vec<EId<InterfaceElement>>> = const { RefCell::new(Vec::new()) };
}

/// `ClassHierarchy.implementedInterfaces(element)` =
/// `InterfaceElementImpl.allSupertypes`.
pub fn implemented_interfaces<'a>(ctx: &Ctx<'a>, element: EId<InterfaceElement>) -> &'a [TypeId] {
    let data = ctx.interface(element);
    if let Some(list) = data.all_supertypes.try_get() {
        return ctx.list(*list);
    }
    if IN_PROGRESS.with(|s| s.borrow().contains(&element)) {
        return &[];
    }
    let global = crate::type_ext::cache_ctx(ctx, |store| element.raw().store() == store);
    IN_PROGRESS.with(|s| s.borrow_mut().push(element));
    let (interfaces, _) = compute_hierarchy(&global, element);
    IN_PROGRESS.with(|s| s.borrow_mut().pop());
    let list = global.intern_list(&interfaces);
    let list = *data.all_supertypes.get_or_init(|| list);
    ctx.list(list)
}

/// `ClassHierarchy.errors(element)`.
pub fn errors(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> Vec<IncompatibleInterfacesError> {
    compute_hierarchy(&ctx.global(), element).1
}

/// `ClassHierarchy._getHierarchy(element)`.
fn compute_hierarchy(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
) -> (Vec<TypeId>, Vec<IncompatibleInterfacesError>) {
    let type_system = TypeSystem::new(*ctx);
    let mut merger = InterfacesMerger::new(type_system);

    let append = |merger: &mut InterfacesMerger<'_>, t: Option<TypeId>| {
        let Some(t) = t else { return };
        merger.add(t);
        let substitution = MapSubstitution::from_interface_type(ctx, t);
        let element = ctx.interface_element(t).unwrap();
        for &raw in implemented_interfaces(ctx, element) {
            merger.add(substitution.substitute_type(ctx, raw));
        }
    };

    append(&mut merger, ctx.element_supertype(element));
    for &t in ctx.element_superclass_constraints(element) {
        append(&mut merger, Some(t));
    }
    for &t in ctx.element_interfaces(element) {
        append(&mut merger, Some(t));
    }
    for &t in ctx.element_mixins(element) {
        append(&mut merger, Some(t));
    }

    let mut errors = Vec::new();
    let mut interfaces = Vec::new();
    for collector in merger.map.values() {
        if let Some(error) = collector.error {
            errors.push(error);
        }
        interfaces.push(collector.ty());
    }
    (interfaces, errors)
}

/// `InterfacesMerger`.
pub struct InterfacesMerger<'a> {
    type_system: TypeSystem<'a>,
    map: IndexMap<EId<InterfaceElement>, ClassInterfaceType>,
}

impl<'a> InterfacesMerger<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        InterfacesMerger {
            type_system,
            map: IndexMap::new(),
        }
    }

    /// `typeList`.
    pub fn type_list(&self) -> Vec<TypeId> {
        self.map.values().map(|c| c.ty()).collect()
    }

    /// `add(type)`.
    pub fn add(&mut self, t: TypeId) {
        let ctx = self.type_system.ctx;
        let element = ctx.interface_element(t).expect("interface type");
        let is_dart_core_object = element.raw().is::<ClassElement>()
            && ctx.is_element(element.raw(), "dart.core", "Object");
        let ts = self.type_system;
        self.map
            .entry(element)
            .or_insert_with(|| ClassInterfaceType::new(is_dart_core_object))
            .update(&ts, t);
    }

    /// `addWithSupertypes(type)`.
    pub fn add_with_supertypes(&mut self, t: Option<TypeId>) {
        if let Some(t) = t {
            for s in self.type_system.ctx.all_supertypes(t) {
                self.add(s);
            }
            self.add(t);
        }
    }
}

/// `_ClassInterfaceType`.
struct ClassInterfaceType {
    is_dart_core_object: bool,
    error: Option<IncompatibleInterfacesError>,
    single_type: Option<TypeId>,
    current_result: Option<TypeId>,
}

impl ClassInterfaceType {
    fn new(is_dart_core_object: bool) -> Self {
        ClassInterfaceType {
            is_dart_core_object,
            error: None,
            single_type: None,
            current_result: None,
        }
    }

    fn ty(&self) -> TypeId {
        self.current_result.or(self.single_type).unwrap()
    }

    fn update(&mut self, ts: &TypeSystem<'_>, t: TypeId) {
        if self.error.is_some() {
            return;
        }
        if self.current_result.is_none() {
            match self.single_type {
                None => {
                    self.single_type = Some(t);
                    return;
                }
                // Dart: type == _singleType
                Some(single) if ts.dart_eq(t, single) => return,
                Some(single) => {
                    self.current_result = Some(ts.normalize_interface_type(single));
                }
            }
        }
        let norm_type = ts.normalize_interface_type(t);
        match self.merge(ts, self.current_result.unwrap(), norm_type) {
            Some(merged) => self.current_result = Some(merged),
            None => {
                self.error = Some(IncompatibleInterfacesError {
                    first: self.current_result.unwrap(),
                    second: t,
                });
            }
        }
    }

    /// `_merge(T1, T2)`; `None` where Dart throws.
    fn merge(&self, ts: &TypeSystem<'_>, t1: TypeId, t2: TypeId) -> Option<TypeId> {
        let ctx = ts.ctx;
        // Normally `Object?` cannot be a superinterface.
        // However, it can happen for extension types.
        if self.is_dart_core_object {
            let n1 = ctx.nullability_suffix(t1);
            let n2 = ctx.nullability_suffix(t2);
            if n1 == Nullability::Question && n2 == Nullability::None {
                return Some(t2);
            }
            if n1 == Nullability::None && n2 == Nullability::Question {
                return Some(t1);
            }
        }
        ts.try_top_merge(t1, t2)
    }
}
