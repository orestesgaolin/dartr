// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! This file implements a "mini type system" that's similar to full Dart
//! types, but light weight enough to be suitable for unit testing of code in
//! the `_fe_analyzer_shared` package.
//!
//! # Rust design
//!
//! - A [`Type`] is a `Copy` id into a **thread-local** interner (cargo runs
//!   each test on its own thread; the Dart `TypeRegistry` is a global
//!   static). Interning gives `==` the meaning of the Dart `operator ==` of
//!   the mini types: when a new type is Dart-`==` to an interned one (for
//!   example a generic function type that is equal up to renaming of its type
//!   formals), the existing id is returned. The structure is in
//!   [`TypeData`], reachable with [`Type::data`].
//! - The Dart classes `PrimaryType`, `FunctionType`, `RecordType`,
//!   `TypeParameterType` and `UnknownType` are the data structs
//!   [`PrimaryType`], [`FunctionType`], [`RecordType`],
//!   [`TypeParameterType`] and [`UnknownType`]. Their `new` functions mirror
//!   the Dart constructors (optional named parameters become `with_...`
//!   methods), and `into_type` interns them. The subclasses of `PrimaryType`
//!   (`DynamicType`, `InvalidType`, `NeverType`, `NullType`, `VoidType`,
//!   `FutureOrType`) are a [`PrimaryType`] whose `name_info` is a
//!   [`SpecialTypeName`]; [`DynamicType::instance`] and the like construct
//!   them, and `Type::is_dynamic_type` and the like replace the Dart `is`
//!   tests.
//! - A [`TypeParameter`] is a `Copy` id into the same thread-local storage;
//!   its mutable `explicitBound` has the setter
//!   [`TypeParameter::set_explicit_bound`].
//! - Record field names, named parameter names and type names are [`Name`]s
//!   (interned strings).
//! - Dart `ParseError` is [`ParseError`]: [`Type::try_parse`] returns it,
//!   [`Type::new`] (the Dart factory `Type(typeStr)`) panics with it. Dart
//!   `StateError`s (registry misuse) are panics.
//!
//! Known difference: interning happens when a type is constructed. If the
//! bound of a type formal is changed with
//! [`TypeParameter::set_explicit_bound`] after a generic function type that
//! uses it was interned, two ids can exist for types that Dart would now
//! consider `==`.

mod name;
mod parser;
mod registry;
mod state;
mod type_system;
mod types;

pub use name::Name;
pub use parser::ParseError;
pub use registry::{
    InterfaceTypeName, RegistryGuard, SpecialTypeName, TypeNameInfo, TypeParameter, TypeRegistry,
};
pub use type_system::{SuperInterfaceTemplate, TypeSystem};
pub use types::{
    DynamicType, FreshTypeParameterGenerator, FunctionType, FutureOrType, InvalidType,
    NamedFunctionParameter, NamedType, NeverType, NullType, PrimaryType, RecordType, Substitution,
    Type, TypeData, TypeParameterType, UnknownType, VoidType,
};

/// Surrounds [s] with parentheses if [condition] is `true`, otherwise returns
/// [s] unchanged.
fn parenthesize_if(condition: bool, s: String) -> String {
    if condition { format!("({s})") } else { s }
}
