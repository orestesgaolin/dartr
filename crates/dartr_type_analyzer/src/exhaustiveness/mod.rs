// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/ (library directory)

//! The exhaustiveness checker for switch statements and expressions
//! (`pkg/_fe_analyzer_shared/lib/src/exhaustiveness`).
//!
//! # Modules (Dart file → module)
//!
//! | Dart file | module |
//! |---|---|
//! | `static_type.dart` | [`static_type`] |
//! | `space.dart` | [`space`] |
//! | `key.dart` | [`key`] |
//! | `path.dart` | [`path`] |
//! | `witness.dart` | [`witness`] |
//! | `exhaustive.dart` | [`exhaustive`] |
//! | `shared.dart` | [`shared`] |
//! | `types.dart` + `types/*.dart` (parts) | [`types`] (`types/*.rs`) |
//! | `dart_template_buffer.dart` | [`dart_template_buffer`] |
//! | `profile.dart` | [`profile`] |
//! | `test_helper.dart` | [`test_helper`] |
//!
//! # Design
//!
//! - A Dart `StaticType` object is a `Copy` handle [`StaticType`] into an arena
//!   owned by the [`ExhaustivenessCache`]. The handle carries the node id
//!   (Dart object identity) and the id of the first node that is Dart-`==`
//!   to it, so `==`, sets and maps behave as in Dart. The virtual members of
//!   `StaticType` are the methods of the trait [`StaticTypeArena`], which
//!   the cache implements; `&dyn StaticTypeArena` is passed where Dart calls
//!   a method on a static type. [`ObjectPropertyLookup::static_types`] gives
//!   the arena to the checker.
//! - A Dart `Space` is a shared handle [`Space`] with identity equality (Dart
//!   `Space` has no `==`). Printing (`toString`) of spaces, witnesses and
//!   results is `to_text(types)`.
//! - The client interfaces `TypeOperations<Type>`, `EnumOperations`,
//!   `SealedClassOperations` are traits with associated types; `SpaceCreator`
//!   is a trait with provided methods.
//! - [`DartTemplateBuffer`] receives the client values (types, enum
//!   elements, constants) as `&dyn Any`.
//!
//! # Usage
//!
//! ```ignore
//! let cache = ExhaustivenessCache::new(type_ops, enum_ops, sealed_ops);
//! let scrutinee = cache.get_static_type(&scrutinee_type);
//! // Build one Space per case, for example with a `SpaceCreator`
//! // (`creator.create_root_space(scrutinee, &pattern)`).
//! let mut unreachable = vec![];
//! let non_exhaustiveness = compute_exhaustiveness(
//!     &cache, scrutinee, &case_is_guarded, &case_spaces, Some(&mut unreachable));
//! if let Some(n) = &non_exhaustiveness {
//!     let mut buffer = SimpleDartBuffer::new();
//!     n.witnesses[0].to_dart(&cache, &mut buffer, false);
//! }
//! ```

pub mod dart_template_buffer;
pub mod exhaustive;
pub mod key;
pub mod path;
pub mod profile;
pub mod shared;
pub mod space;
pub mod static_type;
pub mod test_helper;
pub mod types;
pub mod witness;

pub use dart_template_buffer::{DartTemplateBuffer, SimpleDartBuffer};
pub use exhaustive::{
    CaseUnreachability, NonExhaustiveness, check_exhaustiveness, checking_order,
    compute_exhaustiveness, expand_sealed_subtypes, is_exhaustive,
};
pub use key::{ExtensionKey, Identity, IdentityValue, Key, MapKey};
pub use path::Path;
pub use shared::{ExhaustivenessCache, FieldLookup, SpaceCreator, TypeOperations};
pub use space::{SingleSpace, Space};
pub use static_type::{ObjectPropertyLookup, StaticType, StaticTypeArena, StaticTypeId};
pub use types::{
    EnumOperations, IdentityRestriction, ListTypeRestriction, MapTypeRestriction, Restriction,
    SealedClassOperations, TypeBasedKind, TypeBasedStaticType,
};
pub use witness::{Predicate, PropertyWitness, Witness};
