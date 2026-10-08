//! dartr_flow: the interfaces of the shared flow analysis and type analyzer
//! (`pkg/_fe_analyzer_shared/lib/src/{flow_analysis,type_inference,types}`).
//!
//! This crate is dependency-free and generic. It contains only traits,
//! enums and plain data types, plus the Dart method bodies that are
//! one-line wrappers. The implementation of flow analysis and of the shared
//! type analyzer is written against these traits by units A9-A13; the
//! analyzer side (`dartr_resolver`) implements the operations traits with
//! its `TypeId` and element ids.
//!
//! # Modules (Dart file → module)
//!
//! | Dart file (`_fe_analyzer_shared/lib/src/...`) | module | main items |
//! |---|---|---|
//! | `types/shared_type.dart` | [`shared_type`] | [`Variance`](shared_type::Variance), the views [`SharedTypeView`](shared_type::SharedTypeView) / [`SharedTypeSchemaView`](shared_type::SharedTypeSchemaView), the `Shared*` interfaces as [`SharedTypeOperations`](shared_type::SharedTypeOperations) |
//! | `type_inference/nullability_suffix.dart` | [`nullability_suffix`] | [`NullabilitySuffix`](nullability_suffix::NullabilitySuffix) |
//! | `flow_analysis/flow_analysis_operations.dart` | [`flow_analysis_operations`] | [`FlowAnalysisTypeOperations`](flow_analysis_operations::FlowAnalysisTypeOperations), [`FlowAnalysisOperations`](flow_analysis_operations::FlowAnalysisOperations) |
//! | `type_inference/type_analyzer_operations.dart` | [`type_analyzer_operations`] | [`TypeAnalyzerOperations`](type_analyzer_operations::TypeAnalyzerOperations) (+ mixin), [`TypeConstraintGenerator`](type_analyzer_operations::TypeConstraintGenerator) (+ mixin) |
//! | `type_inference/type_constraint.dart` | [`type_constraint`] | [`MergedTypeConstraint`](type_constraint::MergedTypeConstraint), [`TypeConstraintOrigin`](type_constraint::TypeConstraintOrigin) |
//! | `type_inference/type_analysis_result.dart` | [`type_analysis_result`] | the `...Result` structs, [`MatchContext`](type_analysis_result::MatchContext) |
//! | `flow_analysis/flow_analysis.dart` (public part) | [`flow_analysis`] | [`FlowAnalysis`](flow_analysis::FlowAnalysis), [`PropertyTarget`](flow_analysis::PropertyTarget), [`NonPromotionReason`](flow_analysis::NonPromotionReason) |
//! | `type_inference/type_analyzer.dart` | [`type_analyzer`] | [`TypeAnalyzer`](type_analyzer::TypeAnalyzer), [`TypeAnalyzerErrors`](type_analyzer::TypeAnalyzerErrors), [`TypeAnalyzerOptions`](type_analyzer::TypeAnalyzerOptions) |
//! | `type_inference/null_shorting.dart` | [`null_shorting`] | [`TypeAnalysisNullShortingInterface`](null_shorting::TypeAnalysisNullShortingInterface) (+ `NullShortingMixin`) |
//! | `type_inference/assigned_variables.dart` (public API) | [`assigned_variables`] | [`AssignedVariables`](assigned_variables::AssignedVariables) |
//! | `type_inference/variable_bindings.dart` (client side) | [`variable_bindings`] | [`VariableBinder`](variable_bindings::VariableBinder), [`VariableBinderErrors`](variable_bindings::VariableBinderErrors) |
//! | `type_inference/body_inference_context.dart` | [`body_inference_context`] | [`SharedBodyInferenceContext`](body_inference_context::SharedBodyInferenceContext) |
//!
//! # Translation rules
//!
//! - Dart type parameters (`Node`, `Statement`, `Expression`, `Variable`,
//!   `Pattern`, `Error`, `TypeDeclarationType`, `TypeDeclaration`,
//!   `AstNode`, `Guard`) become associated types. Ids (types, type
//!   parameters, names, variables, nodes) are `Copy + Eq + Hash + Debug`
//!   and passed by value.
//! - `SharedType` / `SharedTypeParameter` become the associated types
//!   [`SharedTypeOperations::Type`](shared_type::SharedTypeOperations::Type)
//!   and `::TypeParameter`; the views `SharedTypeView`,
//!   `SharedTypeSchemaView`, `SharedTypeParameterView` are generic newtypes.
//!   Getters of the `Shared*` interfaces are methods of
//!   [`SharedTypeOperations`](shared_type::SharedTypeOperations), `is
//!   SharedXType` is [`shared_type_kind`](shared_type::SharedTypeOperations::shared_type_kind).
//! - Dart `String` names of members, record fields, named parameters and
//!   pattern variables are the associated type
//!   [`SharedTypeOperations::Name`](shared_type::SharedTypeOperations::Name).
//!   Display strings and error message parts stay `String`.
//! - Dart `Object` / `Object?` parameters become associated types
//!   (`PropertyMember`, `CollectionElementContext`, `Key`).
//! - A Dart mixin becomes provided methods of a trait; its abstract members
//!   are required methods; its state is reached through a required accessor
//!   (`guards`, `dot_shorthands`). Provided methods that are more than a
//!   wrapper are `todo!("<Dart method name>")`.
//! - Nullable returns are `Option`; named parameters are ordinary parameters
//!   in Dart order, with Dart default values given in the docs; Dart records
//!   are tuples or small structs; `Map<String, X>` that is iterated is a
//!   `Vec<(Name, X)>` in Dart insertion order; callbacks are
//!   `&mut dyn FnMut(&mut Self)`.
//! - Queries take `&self`; flow analysis events and visitor callbacks take
//!   `&mut self`.
//! - Object safety is stated on each trait. The operations traits are object
//!   safe once their associated types are fixed, except
//!   [`TypeAnalyzerOperations`](type_analyzer_operations::TypeAnalyzerOperations)
//!   (generic associated type). The shared algorithms use all traits as
//!   generic bounds, not as `dyn`.
//!
//! # Left for units A9-A13
//!
//! - A9: `FlowModel`, `PromotionModel`, `SsaNode`, `Reachability`,
//!   `FlowLink` / `flow_link.dart`, `PromotionInfo`, `NonPromotionHistory`.
//! - A10: `_FlowAnalysisImpl` (statements and expressions), the
//!   `_FlowContext` classes, the concrete `ExpressionInfo` (the associated
//!   type [`FlowAnalysisNullShortingInterface::ExpressionInfo`](flow_analysis::FlowAnalysisNullShortingInterface::ExpressionInfo)),
//!   `_Reference`, `TrivialVariableReference`, the constructor (Dart
//!   factory `FlowAnalysis(...)`).
//! - A11: patterns in flow analysis, `PromotionKeyStore`, the
//!   implementation of [`AssignedVariables`](assigned_variables::AssignedVariables)
//!   (+ `AssignedVariablesForTesting`).
//! - A12: the `analyze...` expression, statement, switch and if-case methods
//!   of [`TypeAnalyzer`](type_analyzer::TypeAnalyzer).
//! - A13: the pattern `analyze...` methods, the `TypeAnalyzerOperationsMixin`
//!   inference methods (`chooseTypes`, `inferTypeParameterFromAll`,
//!   `inferTypeParameterFromContext`, `mergeInConstraintsFromBound`), the
//!   shared `TypeConstraintGenerator` algorithm (`performSubtypeConstraint...`),
//!   the `VariableBinder` state and concrete methods.
//!
//! Not ported: `FlowAnalysisDebug`, `FlowAnalysis.ssaNodeForTesting` (its
//! return type is an implementation type), `_dumpState`,
//! `shared_inference_log.dart`.

#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod assigned_variables;
pub mod body_inference_context;
pub mod flow_analysis;
pub mod flow_analysis_impl;
pub mod flow_analysis_operations;
pub mod flow_link;
pub mod null_shorting;
pub mod nullability_suffix;
pub mod promotion_key_store;
pub mod shared_type;
pub mod type_analysis_result;
pub mod type_analyzer;
pub mod type_analyzer_operations;
pub mod type_constraint;
pub mod variable_bindings;
