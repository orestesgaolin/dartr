//! The corrections of the language server (Dart
//! `pkg/analysis_server/lib/src/services/correction` and the change builder
//! of `pkg/analyzer_plugin`): fixes for diagnostics, organize imports and
//! sort members.
//!
//! - [`change`], [`change_builder`], [`imports`]: the change model and the
//!   change builder (source edits, linked edit groups, selection, imports
//!   with the import insertion logic of Dart).
//! - [`fix_kind`], [`generated`]: the fix kinds and the registry of the
//!   producers of each diagnostic code (generated from the Dart sources by
//!   `tools/codegen/gen_fixes.py`).
//! - [`producer`], [`producers`]: the correction producers.
//! - [`fix_processor`]: the fixes of a diagnostic, "fix all in file" and
//!   the ignore fixes ([`ignore`], [`yaml_edit`]).
//! - [`organize_imports`], [`sort_members`]: the source actions.

pub mod change;
pub mod change_builder;
pub mod code_style;
pub mod dart_edit;
pub mod fix_kind;
pub mod fix_processor;
pub mod generated;
pub mod ignore;
pub mod imports;
pub mod insert;
pub mod organize_imports;
pub mod producer;
pub mod producers;
pub mod sort_members;
pub mod utils;
pub mod yaml_edit;
