//! dartr_link: the element linker, a port of
//! `pkg/analyzer/lib/src/summary2` (design `docs/design/semantics.md` §2.2,
//! units B1–B5).
//!
//! A library cycle is linked by [`link::link_cycle`]: the library builders
//! (`library_builder.dart`) build the fragments and elements of every
//! library (`element_builder.dart`), then the export scopes
//! (`export.dart`); later phases resolve types and infer.
//!
//! # Modules
//!
//! - [`reference`]: `summary2/reference.dart` (symbolic references).
//! - [`dump`]: the `elements` dump (`tools/oracle/bin/elements.dart`).

#![allow(
    // Ported functions keep the parameters and the index loops of the Dart
    // code.
    clippy::too_many_arguments,
    clippy::needless_range_loop
)]

pub mod ast_util;
pub mod detach_nodes;
pub mod dump;
pub mod element_builder;
pub mod export;
pub mod informative_data;
pub mod instance_member_inferrer;
pub mod input;
pub mod library_builder;
pub mod link;
pub mod outline;
pub mod reference;
pub mod scope;
pub mod top_level_inference;
pub mod type_bounds;
pub mod types;
pub mod types_builder;
