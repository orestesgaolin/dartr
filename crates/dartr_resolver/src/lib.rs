//! dartr_resolver: body resolution of the analyzer, a port of
//! `pkg/analyzer/lib/src/dart/resolver/*`, `generated/resolver.dart`,
//! `generated/element_resolver.dart`, `generated/static_type_analyzer.dart`,
//! `generated/element_walker.dart`, `dart/element/scope.dart` (analysis
//! scopes) and `dart/analysis/library_analyzer.dart` (design
//! `docs/design/semantics.md` wave C, §3).
//!
//! See `README.md` of this crate for how to port one resolver file against
//! the core ([`resolver::ResolverVisitor`]).
//!
//! # Passes over one unit
//!
//! 1. [`element_binding_visitor`]: binds the linked fragments to the
//!    declarations (`declared_fragment`) and creates the fragments and
//!    elements of local declarations in the unit's [`LocalArena`].
//! 2. [`resolution_visitor`]: scopes, type annotations
//!    ([`named_type_resolver`]), AST rewrites ([`ast_rewrite`]), the scope
//!    lookup result of each identifier.
//! 3. [`resolver::ResolverVisitor`]: types, inference, flow analysis.
//!
//! [`library_analyzer`] runs the passes for every unit of a library (units
//! in parallel) and then the library-wide steps.
//!
//! # Results
//!
//! - [`dartr_element::ResolutionTables`]: what the Dart resolver writes into
//!   AST nodes (static types, elements, ...).
//! - [`tables::ResolverTables`]: other per-node data of the resolver passes
//!   (for example `SimpleIdentifier.scopeLookupResult`).
//! - The diagnostics of the unit.
//!
//! [`LocalArena`]: dartr_element::LocalArena

#![allow(
    // Ported functions keep the parameters and the structure of the Dart
    // code.
    clippy::too_many_arguments,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::type_complexity
)]

pub mod generated {
    pub mod dispatch;
}

pub mod ast_ext;
pub mod library_analyzer;
pub mod options;
pub mod resolver;
pub mod tables;

// C1: scopes, element binding, resolution visitor.
pub mod ast_rewrite;
pub mod element_binding_visitor;
pub mod element_ext;
pub mod element_walker;
pub mod ast_resolver;
pub mod named_type_resolver;
pub mod record_type_annotation_resolver;
pub mod resolution_visitor;
pub mod scope;
pub mod scope_context;

// C2: the resolver core.
pub mod body_inference_context;
pub mod element_resolver;
pub mod error;
pub mod error_detection_helpers;
pub mod error_verifier;
pub mod ffi_verifier;
pub mod flow_analysis_visitor;
pub mod lexical_lookup;
pub mod shared_type_analyzer;
pub mod simple_identifier_resolver;
pub mod static_type_analyzer;
pub mod this_lookup;
pub mod type_analyzer_options;
pub mod variable_declaration_resolver;

// Stubs: one module per Dart resolver file, filled by units C3–C9.
pub mod annotation_resolver;
pub mod applicable_extensions;
pub mod assignment_expression_resolver;
pub mod binary_expression_resolver;
pub mod comment_reference_resolver;
pub mod constructor_reference_resolver;
pub mod dot_shorthand_resolver;
pub mod exit_detector;
pub mod extension_member_resolver;
pub mod for_resolver;
pub mod function_expression_invocation_resolver;
pub mod function_expression_resolver;
pub mod function_reference_resolver;
pub mod instance_creation_expression_resolver;
pub mod invocation_inference_helper;
pub mod invocation_inferrer;
pub mod list_pattern_resolver;
pub mod method_invocation_resolver;
pub mod pattern_resolver;
pub mod postfix_expression_resolver;
pub mod prefix_expression_resolver;
pub mod prefixed_identifier_resolver;
pub mod property_element_resolver;
pub mod record_literal_resolver;
pub mod resolution_result;
pub mod type_property_resolver;
pub mod typed_literal_resolver;
pub mod yield_statement_resolver;

pub use library_analyzer::{LibraryAnalysisInput, ResolvedLibrary, ResolvedUnit, UnitInput, analyze_library};
pub use resolver::ResolverVisitor;
pub use tables::ResolverTables;
