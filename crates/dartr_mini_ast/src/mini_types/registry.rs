// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! `TypeNameInfo` and its subclasses (`InterfaceTypeName`, `SpecialTypeName`,
//! `TypeParameter`), and `TypeRegistry`.

use std::fmt;

use dartr_flow::shared_type::Variance;
use indexmap::IndexMap;

use super::name::Name;
use super::state::{STATIC_INTERFACE_TYPE_NAMES, TypeParameterData, with_state};
use super::types::Type;

/// A type name that represents an ordinary interface type.
///
/// Rust: an id into the thread-local storage. Like the Dart objects, two
/// registrations of the same name give different (not `==`) names.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterfaceTypeName(pub(crate) u32);

impl InterfaceTypeName {
    /// `InterfaceTypeName._(name)`: creates a new, unregistered name.
    fn new_unregistered(name: &str) -> InterfaceTypeName {
        let name = Name::new(name);
        with_state(|s| {
            s.interface_type_names.push(name);
            InterfaceTypeName((s.interface_type_names.len() - 1) as u32)
        })
    }

    /// The name.
    pub fn name(self) -> Name {
        with_state(|s| s.interface_type_names[self.0 as usize])
    }
}

impl fmt::Debug for InterfaceTypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InterfaceTypeName({}: {})", self.0, self.name())
    }
}

/// A type name that represents one of Dart's built-in "special" types, such
/// as:
/// - `dynamic`
/// - `error` (to represent an invalid type)
/// - `FutureOr`
/// - `Never`
/// - `Null`
/// - `void`
///
/// Rust: the Dart objects are static singletons, so this is an enum.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SpecialTypeName {
    /// `TypeRegistry.dynamic_` (runtime type `DynamicType`).
    Dynamic,
    /// `TypeRegistry.error_` (runtime type `InvalidType`).
    Error,
    /// `TypeRegistry.futureOr` (runtime type `FutureOrType`).
    FutureOr,
    /// `TypeRegistry.never` (runtime type `NeverType`).
    Never,
    /// `TypeRegistry.null_` (runtime type `NullType`).
    Null,
    /// `TypeRegistry.void_` (runtime type `VoidType`).
    Void,
}

impl SpecialTypeName {
    /// The name.
    pub fn name(self) -> Name {
        Name::new(match self {
            SpecialTypeName::Dynamic => "dynamic",
            SpecialTypeName::Error => "error",
            SpecialTypeName::FutureOr => "FutureOr",
            SpecialTypeName::Never => "Never",
            SpecialTypeName::Null => "Null",
            SpecialTypeName::Void => "void",
        })
    }
}

/// Information about a single type name recognized by the [`Type`] parser.
///
/// Rust: the Dart field `_expectedRuntimeType` is implied by the variant
/// (and, for [`SpecialTypeName`], by the special name).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeNameInfo {
    /// [`InterfaceTypeName`] (runtime type `PrimaryType`).
    Interface(InterfaceTypeName),
    /// [`SpecialTypeName`].
    Special(SpecialTypeName),
    /// [`TypeParameter`] (runtime type `TypeParameterType`).
    TypeParameter(TypeParameter),
}

impl TypeNameInfo {
    /// The name.
    pub fn name(self) -> Name {
        match self {
            TypeNameInfo::Interface(n) => n.name(),
            TypeNameInfo::Special(n) => n.name(),
            TypeNameInfo::TypeParameter(n) => n.name(),
        }
    }
}

impl From<InterfaceTypeName> for TypeNameInfo {
    fn from(n: InterfaceTypeName) -> TypeNameInfo {
        TypeNameInfo::Interface(n)
    }
}

impl From<SpecialTypeName> for TypeNameInfo {
    fn from(n: SpecialTypeName) -> TypeNameInfo {
        TypeNameInfo::Special(n)
    }
}

impl From<TypeParameter> for TypeNameInfo {
    fn from(n: TypeParameter) -> TypeNameInfo {
        TypeNameInfo::TypeParameter(n)
    }
}

/// A type name that represents a type variable.
///
/// Rust: an id into the thread-local storage. Equality is identity, like the
/// Dart object (which does not override `operator ==`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeParameter(pub(crate) u32);

impl TypeParameter {
    /// `TypeParameter._(name)`: creates a new, unregistered type parameter.
    pub(crate) fn new_unregistered(name: &str) -> TypeParameter {
        let name = Name::new(name);
        with_state(|s| {
            s.type_parameters.push(TypeParameterData {
                name,
                explicit_bound: None,
            });
            TypeParameter((s.type_parameters.len() - 1) as u32)
        })
    }

    /// The name.
    pub fn name(self) -> Name {
        with_state(|s| s.type_parameters[self.0 as usize].name)
    }

    /// The type variable's bound. If `None`, the bound is `Object?`.
    ///
    /// This is mutable because it needs to be possible to set it after
    /// construction, in order to create "F-bounded" type parameters (type
    /// parameters whose bound refers to the type parameter itself).
    pub fn explicit_bound(self) -> Option<Type> {
        with_state(|s| s.type_parameters[self.0 as usize].explicit_bound)
    }

    /// Sets [explicit_bound](Self::explicit_bound).
    pub fn set_explicit_bound(self, bound: Option<Type>) {
        with_state(|s| s.type_parameters[self.0 as usize].explicit_bound = bound);
    }

    /// `bound`: the explicit bound, or `Object?`.
    ///
    /// Panics if there is no explicit bound and the [`TypeRegistry`] is not
    /// initialized (like Dart, which parses `Object?`).
    pub fn bound(self) -> Type {
        self.explicit_bound()
            .unwrap_or_else(|| Type::new("Object?"))
    }

    /// `boundShared`.
    pub fn bound_shared(self) -> Option<Type> {
        Some(self.bound())
    }

    /// `displayName`.
    pub fn display_name(self) -> String {
        self.name().as_str().to_owned()
    }

    /// `isLegacyCovariant`.
    // TODO(paulberry): Implement isLegacyCovariant.
    pub fn is_legacy_covariant(self) -> bool {
        true
    }

    /// `variance`.
    // TODO(paulberry): Implement variance.
    pub fn variance(self) -> Variance {
        Variance::Covariant
    }
}

impl fmt::Display for TypeParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl fmt::Debug for TypeParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeParameter({}: {})", self.0, self.name())
    }
}

/// Container for static methods that can be used to customize the "mini
/// types" representation used in `_fe_analyzer_shared` unit tests.
///
/// Thanks to Dart's scoping rules, it's possible for a single identifier to
/// represent an interface type in some contexts, a special type like `Null`
/// in other contexts, and a type parameter name in other contexts. But
/// allowing a single name to have multiple meanings isn't useful in
/// `_fe_analyzer_shared` unit tests, and opens up greater risk of confusion.
/// Therefore, the "mini types" representation does not permit it; every test
/// must register each type name it intends to use, specifying its meaning,
/// before using that name in a call to the [`Type`] constructor. This
/// registration can happen either within the test itself or in a callback
/// passed to `setUp`.
///
/// Rust: the state is thread-local; use [`TypeRegistry::init_for_test`] at
/// the start of a test.
pub enum TypeRegistry {}

impl TypeRegistry {
    /// The [`TypeNameInfo`] object representing the special type `dynamic`.
    pub fn dynamic_() -> SpecialTypeName {
        SpecialTypeName::Dynamic
    }

    /// The [`TypeNameInfo`] object representing the special type `error`.
    pub fn error_() -> SpecialTypeName {
        SpecialTypeName::Error
    }

    /// The [`TypeNameInfo`] object representing the interface type `Future`.
    pub fn future() -> InterfaceTypeName {
        InterfaceTypeName(0)
    }

    /// The [`TypeNameInfo`] object representing the special type `FutureOr`.
    pub fn future_or() -> SpecialTypeName {
        SpecialTypeName::FutureOr
    }

    /// The [`TypeNameInfo`] object representing the interface type
    /// `Iterable`.
    pub fn iterable() -> InterfaceTypeName {
        InterfaceTypeName(1)
    }

    /// The [`TypeNameInfo`] object representing the interface type `List`.
    pub fn list() -> InterfaceTypeName {
        InterfaceTypeName(2)
    }

    /// The [`TypeNameInfo`] object representing the interface type `Map`.
    pub fn map() -> InterfaceTypeName {
        InterfaceTypeName(3)
    }

    /// The [`TypeNameInfo`] object representing the special type `Never`.
    pub fn never() -> SpecialTypeName {
        SpecialTypeName::Never
    }

    /// The [`TypeNameInfo`] object representing the special type `Null`.
    pub fn null_() -> SpecialTypeName {
        SpecialTypeName::Null
    }

    /// The [`TypeNameInfo`] object representing the interface type `Stream`.
    pub fn stream() -> InterfaceTypeName {
        InterfaceTypeName(4)
    }

    /// The [`TypeNameInfo`] object representing the special type `void`.
    pub fn void_() -> SpecialTypeName {
        SpecialTypeName::Void
    }

    /// Registers [name] as the name of an ordinary interface type.
    pub fn add_interface_type_name(name: &str) -> InterfaceTypeName {
        let interface_type_name = InterfaceTypeName::new_unregistered(name);
        Self::add(TypeNameInfo::Interface(interface_type_name));
        interface_type_name
    }

    /// Registers [name] as the name of a type parameter.
    pub fn add_type_parameter(name: &str) -> TypeParameter {
        let type_parameter = TypeParameter::new_unregistered(name);
        Self::add(TypeNameInfo::TypeParameter(type_parameter));
        type_parameter
    }

    /// Initializes the "mini type" infrastructure.
    ///
    /// This method must be called before any unit test that makes use of mini
    /// types. Panics if already initialized (Dart: `StateError`). The Dart
    /// assert that the caller is a `setUp` callback is not ported.
    pub fn init() {
        let already_initialized = with_state(|s| {
            if s.type_name_info_map.is_some() {
                return true;
            }
            s.type_name_info_map = Some(IndexMap::new());
            false
        });
        if already_initialized {
            panic!("init() already called. Did you forget to call uninit() from `tearDown`?");
        }
        debug_assert_eq!(Self::future().name(), STATIC_INTERFACE_TYPE_NAMES[0]);
        // Set up some common built-in type names.
        Self::add_interface_type_name("bool");
        Self::add_interface_type_name("double");
        Self::add(Self::dynamic_().into());
        Self::add(Self::error_().into());
        Self::add(Self::future().into());
        Self::add(Self::future_or().into());
        Self::add_interface_type_name("int");
        Self::add(Self::iterable().into());
        Self::add(Self::list().into());
        Self::add(Self::map().into());
        Self::add(Self::never().into());
        Self::add(Self::null_().into());
        Self::add_interface_type_name("num");
        Self::add_interface_type_name("Object");
        Self::add(Self::stream().into());
        Self::add_interface_type_name("String");
        Self::add_interface_type_name("StackTrace");
        Self::add(Self::void_().into());
    }

    /// Calls [init](Self::init) and returns a guard that calls
    /// [uninit](Self::uninit) when dropped (also when the test panics).
    #[must_use = "the registry is un-initialized when the guard is dropped"]
    pub fn init_for_test() -> RegistryGuard {
        Self::init();
        RegistryGuard(())
    }

    /// Whether [init](Self::init) was called and [uninit](Self::uninit) was
    /// not called since.
    pub fn is_initialized() -> bool {
        with_state(|s| s.type_name_info_map.is_some())
    }

    /// Retrieves the [`TypeNameInfo`] corresponding to [name].
    ///
    /// Panics if the registry is not initialized or the name is unknown
    /// (Dart: `StateError`).
    pub fn lookup(name: &str) -> TypeNameInfo {
        match Self::try_lookup(name) {
            Some(info) => info,
            None => panic!("Unknown type name {name} (use `TypeRegistry.add...` first)"),
        }
    }

    /// Like [lookup](Self::lookup), but returns `None` for an unknown name.
    /// Panics if the registry is not initialized.
    pub fn try_lookup(name: &str) -> Option<TypeNameInfo> {
        let name = Name::new(name);
        with_state(|s| {
            s.type_name_info_map
                .as_ref()
                .map(|map| map.get(&name).copied())
        })
        .unwrap_or_else(|| Self::not_initialized())
    }

    /// Un-does the operation of [init](Self::init), rendering the "mini type"
    /// infrastructure unusable.
    pub fn uninit() {
        // Note: don't complain if `_typeNameInfoMap` is `null`, because we
        // don't want to produce confusing failure messages if a test runs
        // into trouble while trying to initialize itself.
        with_state(|s| s.type_name_info_map = None);
    }

    /// Registers [info] as information about a type name.
    fn add(info: TypeNameInfo) {
        let name = info.name();
        let result = with_state(|s| {
            let map = s.type_name_info_map.as_mut()?;
            Some(if map.contains_key(&name) {
                false
            } else {
                map.insert(name, info);
                true
            })
        });
        match result {
            None => Self::not_initialized(),
            Some(false) => panic!("Type name {name} already registered"),
            Some(true) => {}
        }
    }

    fn not_initialized() -> ! {
        panic!(
            "TypeRegistry not initialized (call `TypeRegistry.init` from a test `setUp` callback)"
        )
    }
}

/// Guard returned by [`TypeRegistry::init_for_test`]; calls
/// [`TypeRegistry::uninit`] on drop.
pub struct RegistryGuard(());

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        TypeRegistry::uninit();
    }
}
