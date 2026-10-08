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

pub mod dump;
pub mod reference;
