//! dartr_element: the element and type model of the analyzer
//! (`pkg/analyzer/lib/src/dart/element/{element,type,type_provider,member}.dart`),
//! the lookup context, and the resolution side tables.
//!
//! See `README.md` of this crate for the API contract of the semantic units
//! (how to build elements, read them through [`Ctx`], intern types, write
//! resolution results, and the determinism rules).
//!
//! # Modules
//!
//! - [`ids`]: [`ElementId`], [`FragmentId`], [`StoreId`], typed [`EId<T>`] /
//!   [`FId<T>`], the category markers and [`SubtypeOf`].
//! - [`element`], [`fragment`], [`data`]: the data structs of every
//!   `*ElementImpl` / `*FragmentImpl` class, with all Dart fields.
//! - [`flags`]: [`ElementFlags`] / [`FragmentFlags`] (generated).
//! - [`kind`]: Dart [`ElementKind`].
//! - [`name`]: [`Name`] interning.
//! - [`store`]: [`ElementStore`] / [`FragmentStore`].
//! - [`types`], [`interner`]: [`TypeKind`], [`TypeId`], [`ListId`], the global
//!   [`Interner`] and the local [`TypeOverlay`].
//! - [`ctx`]: [`Ctx`], [`WorldSnapshot`], [`Generation`], [`LocalArena`],
//!   [`RequirementSink`].
//! - [`type_provider`]: the [`TypeProvider`].
//! - [`resolution`]: the [`ResolutionTables`].
//! - [`diagnostics`]: type and element arguments for
//!   `dartr_diagnostics::DiagnosticReporter`.
//!
//! # Regenerating
//!
//! The schema (`schema/element.json`) is extracted from the pinned analyzer;
//! `src/generated/flags.rs` is generated from it. The structs are written by
//! hand (each field needs a decision about its Rust type and mutability);
//! `tests/schema_coverage.rs` checks that every Dart field is ported or
//! listed as dropped, so a schema update shows what to change.
//!
//! ```text
//! (cd tools/oracle && dart run bin/element_schema.dart > ../../crates/dartr_element/schema/element.json)
//! python3 tools/codegen/gen_element.py
//! ```

pub mod ctx;
pub mod data;
pub mod diagnostics;
pub mod element;
pub mod fragment;
pub mod ids;
pub mod interner;
pub mod kind;
pub mod lookup;
pub mod name;
mod pool;
pub mod resolution;
pub mod slot;
pub mod store;
pub mod type_provider;
pub mod types;

/// Generated code.
pub mod flags {
    include!("generated/flags.rs");
}

pub use ctx::{
    Ctx, FeatureSet, Generation, LocalArena, NoopSink, Requirement, RequirementSink, WorldSnapshot,
};
pub use data::*;
pub use element::*;
pub use flags::{ElementFlags, FragmentFlags};
pub use fragment::*;
pub use ids::*;
pub use interner::{Interner, ListItem, TypeOverlay};
pub use kind::ElementKind;
pub use lookup::{LookupMap, LookupSet};
pub use name::{Name, NamePool};
pub use resolution::{NodeFlags, PatternInfo, ResolutionTables};
pub use slot::{Arena, BoolSlot, ElementFlagCell, FragmentFlagCell, OnceSlot, VarSlot};
pub use store::{
    AnyElement, ElementArenas, ElementStore, FragmentStore, StoredElement, StoredFragment,
};
pub use type_provider::TypeProvider;
pub use types::*;
