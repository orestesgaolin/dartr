//! dartr_type_analyzer: the implementation of the shared type analyzer
//! (`pkg/_fe_analyzer_shared/lib/src/type_inference`) and of the
//! exhaustiveness checker (`pkg/_fe_analyzer_shared/lib/src/exhaustiveness`).
//!
//! The interfaces (traits, results, enums) are in `dartr_flow`. This crate
//! depends only on `dartr_flow`.
//!
//! # Mixins
//!
//! The Dart mixins `TypeAnalyzer`, `TypeAnalyzerOperationsMixin` and
//! `TypeConstraintGeneratorMixin` are traits in `dartr_flow`, whose
//! non-trivial provided methods are `todo!()`. This crate ports their bodies
//! as generic free functions named after the Dart methods (for example
//! [`type_analyzer::analyze_switch_statement`]). Inside these functions,
//! calls to other mixin members go through the trait (`a.analyze_expression(..)`),
//! so a client override is respected, as with Dart virtual dispatch.
//!
//! A client applies a mixin by invoking the matching macro inside its
//! `impl` block. The macro overrides each `todo!()` provided method with a
//! call to the free function:
//!
//! ```ignore
//! impl TypeAnalyzer for MyAnalyzer {
//!     // required methods ...
//!     dartr_type_analyzer::type_analyzer_mixin!();
//! }
//! ```
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

#[doc(hidden)]
pub use dartr_flow as __dartr_flow;

pub mod shared_inference_log;
pub mod type_analyzer;
pub mod type_analyzer_operations;
pub mod variable_bindings;

/// Re-exports used by the mixin macros (not public API).
#[doc(hidden)]
pub mod __private {
    pub use dartr_flow::{
        null_shorting, shared_type, type_analysis_result, type_analyzer, type_analyzer_operations,
    };
}
