// Dart source: pkg/analyzer/lib/src/dart/element/type_system.dart
// (the `TypeSystemImpl` members that value.dart calls) and
// pkg/analyzer/lib/src/dart/element/type.dart (`TypeImpl.==`,
// `extensionTypeErasure`, `getDisplayString`, `InterfaceType.lookUp*`).

//! [`ConstTypeSystem`]: the hook through which constant values call the type
//! system. `dartr_typesystem` implements it; tests use a small stand-in.

use dartr_element::{Ctx, EId, ElemRef, LibraryElement, TypeId};

/// The parts of `TypeSystemImpl` (and of `TypeImpl`) that value.dart uses.
///
/// Every value operation that the Dart code gives a `TypeSystemImpl` takes a
/// `&dyn ConstTypeSystem`. The [`Ctx`] gives the type provider and element
/// data.
pub trait ConstTypeSystem {
    /// The lookup context (`typeSystem.typeProvider` is `ctx().tp`).
    fn ctx(&self) -> Ctx<'_>;

    /// Dart `TypeSystemImpl.isSubtypeOf(leftType, rightType)`.
    fn is_subtype_of(&self, left: TypeId, right: TypeId) -> bool;

    /// Dart `TypeSystemImpl.runtimeTypesEqual(T1, T2)`.
    fn runtime_types_equal(&self, t1: TypeId, t2: TypeId) -> bool;

    /// Dart `TypeSystemImpl.normalize(T)`.
    fn normalize(&self, t: TypeId) -> TypeId;

    /// Dart `TypeImpl.==` (structural equality that ignores the alias).
    /// `TypeId ==` is Dart `identical`, see the `dartr_element` README.
    fn types_equal(&self, t1: TypeId, t2: TypeId) -> bool;

    /// Dart `TypeImpl.extensionTypeErasure`.
    fn extension_type_erasure(&self, t: TypeId) -> TypeId;

    /// Dart `TypeImpl.getDisplayString()` (default arguments).
    fn display_string(&self, t: TypeId) -> String;

    /// Dart `InterfaceType.lookUpMethod(name, library, concrete: true)` on
    /// the interface type [ty].
    fn look_up_concrete_method(
        &self,
        ty: TypeId,
        name: &str,
        library: EId<LibraryElement>,
    ) -> Option<ElemRef>;

    /// Dart `InterfaceType.lookUpGetter(name, library, concrete: true)` on
    /// the interface type [ty].
    fn look_up_concrete_getter(
        &self,
        ty: TypeId,
        name: &str,
        library: EId<LibraryElement>,
    ) -> Option<ElemRef>;
}

/// The type system of `dartr_typesystem` (Dart `TypeSystemImpl`).
impl ConstTypeSystem for dartr_typesystem::TypeSystem<'_> {
    fn ctx(&self) -> Ctx<'_> {
        self.ctx
    }

    fn is_subtype_of(&self, left: TypeId, right: TypeId) -> bool {
        dartr_typesystem::TypeSystem::is_subtype_of(self, left, right)
    }

    fn runtime_types_equal(&self, t1: TypeId, t2: TypeId) -> bool {
        dartr_typesystem::TypeSystem::runtime_types_equal(self, t1, t2)
    }

    fn normalize(&self, t: TypeId) -> TypeId {
        dartr_typesystem::TypeSystem::normalize(self, t)
    }

    fn types_equal(&self, t1: TypeId, t2: TypeId) -> bool {
        self.dart_eq(t1, t2)
    }

    fn extension_type_erasure(&self, t: TypeId) -> TypeId {
        dartr_typesystem::TypeSystem::extension_type_erasure(self, t)
    }

    fn display_string(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(
            &self.ctx,
            t,
            dartr_element::DisplayOptions::default(),
        )
    }

    fn look_up_concrete_method(
        &self,
        ty: TypeId,
        name: &str,
        library: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        dartr_typesystem::lookup::type_look_up_method(
            &self.ctx,
            ty,
            name,
            library,
            dartr_typesystem::lookup::LookUpOptions {
                concrete: true,
                inherited: false,
                recovery_static: false,
            },
        )
    }

    fn look_up_concrete_getter(
        &self,
        ty: TypeId,
        name: &str,
        library: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        dartr_typesystem::lookup::type_look_up_getter(
            &self.ctx,
            ty,
            name,
            library,
            dartr_typesystem::lookup::LookUpOptions {
                concrete: true,
                inherited: false,
                recovery_static: false,
            },
        )
    }
}
