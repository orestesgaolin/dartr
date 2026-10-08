//! dartr_constant: constant values of the analyzer (unit A14), a port of
//! `pkg/analyzer/lib/src/dart/constant/value.dart` and its helpers.
//!
//! - [`value`]: [`DartObjectImpl`], [`InstanceState`] and the state structs,
//!   [`EvaluationException`], [`InvalidConstant`], [`Constant`],
//!   [`DartObjectSet`] / [`DartObjectMap`].
//! - [`type_system`]: [`ConstTypeSystem`], the hook to the type system
//!   (implemented by `dartr_typesystem`).
//! - [`dart_num`]: Dart VM `int` / `double` semantics (`double.toString()`,
//!   `%`, `~/`, shifts, `int.parse`).
//! - [`has_type_parameter_reference`], [`has_invalid_type`]: type walks.
//! - [`from_environment_evaluator`], [`declared_variables`]: `-D`
//!   variables.
//!
//! The evaluator (`evaluation.dart`, `utilities.dart`, `compute.dart`,
//! `potentially_constant.dart`) works on the AST and is unit D1–D2.

pub mod dart_num;
pub mod declared_variables;
pub mod from_environment_evaluator;
pub mod has_invalid_type;
pub mod has_type_parameter_reference;
pub mod type_system;
pub mod value;

pub use declared_variables::DeclaredVariables;
pub use from_environment_evaluator::FromEnvironmentEvaluator;
pub use has_invalid_type::has_invalid_type;
pub use has_type_parameter_reference::has_type_parameter_reference;
pub use type_system::ConstTypeSystem;
pub use value::{
    BoolState, Constant, ConstructorInvocationImpl, DartObjectImpl, DartObjectMap, DartObjectSet,
    DoubleState, EvalResult, EvaluationException, FieldMap, FunctionState, GenericState,
    InstanceState, IntState, InvalidConstant, ListState, MapState, NullState, RecordState,
    SetState, StringState, SymbolState, TypeState, is_dart_core_type, positional_field_index,
};
