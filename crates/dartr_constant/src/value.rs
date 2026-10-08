// Dart source: pkg/analyzer/lib/src/dart/constant/value.dart
// (DartObjectImpl, InstanceState and its subclasses, EvaluationException,
// InvalidConstant, ConstructorInvocationImpl) and the public interface
// pkg/analyzer/lib/dart/constant/value.dart (DartObject).

//! Constant values.
//!
//! # Mapping from Dart
//!
//! - `DartObjectImpl` is a value type ([`DartObjectImpl`], cheap to clone:
//!   the large states are behind `Arc`). The Dart `_typeSystem` field is not
//!   stored; every operation that needs the type system takes a
//!   `&dyn ConstTypeSystem` (the Dart code passes `typeSystem` too).
//! - `InstanceState` and its subclasses are the enum [`InstanceState`]. A
//!   Dart virtual method is one `match`; the arm `_` is the base class
//!   implementation.
//! - Dart `==` / `hashCode` of objects and states need the type system
//!   (`runtimeTypesEqual`, `TypeImpl.==`): [`DartObjectImpl::dart_eq`],
//!   [`InstanceState::state_eq`]. The Dart `Set<DartObjectImpl>` and
//!   `Map<DartObjectImpl, DartObjectImpl>` are [`DartObjectSet`] and
//!   [`DartObjectMap`] (insertion order, lookup through the type system).
//! - Dart `toString()` needs display strings of types:
//!   [`DartObjectImpl::display`], [`InstanceState::display`].
//! - A thrown `EvaluationException` is `Err(EvaluationException)`.
//! - Dart `String` values are Rust strings. Lengths and orderings use UTF-16
//!   code units; a lone surrogate is not representable (the parser gives
//!   U+FFFD for it, see `dartr_parser::quote`).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{
    ClassElement, Ctx, EId, ElemRef, ElementId, ExecutableElement, FeatureSet, LookupMap,
    TypeAliasElement, TypeId, TypeKind, VariableElement,
};
use indexmap::IndexMap;

use crate::dart_num;
use crate::has_invalid_type::has_invalid_type;
use crate::has_type_parameter_reference::has_type_parameter_reference;
use crate::type_system::ConstTypeSystem;

/// The result of an operation that can throw an [`EvaluationException`].
pub type EvalResult<T> = Result<T, EvaluationException>;

/// A map from names to values with Dart `Map` (insertion) order: the fields
/// of a `GenericState`, the named fields of a record, named arguments.
pub type FieldMap = IndexMap<Arc<str>, DartObjectImpl>;

fn exception(locatable_diagnostic: LocatableDiagnostic) -> EvaluationException {
    EvaluationException::new(locatable_diagnostic)
}

// ---------------------------------------------------------------------------
// Constant, EvaluationException, InvalidConstant, ConstructorInvocationImpl
// ---------------------------------------------------------------------------

/// Dart `sealed class Constant`: a valid or invalid constant used by the
/// constant evaluator.
///
/// [`DartObjectImpl`] represents a valid result. Note that the
/// [`DartObjectImpl`] could have an unknown state and still be a valid
/// constant. [`InvalidConstant`] represents an invalid result with error
/// information.
#[derive(Clone, Debug)]
pub enum Constant {
    Value(DartObjectImpl),
    Invalid(Box<InvalidConstant>),
}

/// Dart `EvaluationException`: exception that would be thrown during the
/// evaluation of Dart code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationException {
    /// The [`LocatableDiagnostic`] associated with the exception.
    pub locatable_diagnostic: LocatableDiagnostic,
    /// Whether the evaluation exception is a runtime exception.
    pub is_runtime_exception: bool,
}

impl EvaluationException {
    pub fn new(locatable_diagnostic: LocatableDiagnostic) -> EvaluationException {
        EvaluationException {
            locatable_diagnostic,
            is_runtime_exception: false,
        }
    }

    pub fn runtime(locatable_diagnostic: LocatableDiagnostic) -> EvaluationException {
        EvaluationException {
            locatable_diagnostic,
            is_runtime_exception: true,
        }
    }
}

/// Dart `InvalidConstant`: an invalid constant that contains diagnostic
/// information.
///
/// The constructors that take AST nodes (`forEntity`, `copyWithEntity`,
/// `genericError`) are in the evaluator (unit D1), which owns the AST; they
/// reduce to [`InvalidConstant::new`] with the offset and length of the
/// entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidConstant {
    /// The offset of the entity that the evaluation error is reported at.
    pub offset: i64,
    /// The length of the entity that the evaluation error is reported at.
    pub length: i64,
    /// The [`LocatableDiagnostic`] that is being reported.
    pub locatable_diagnostic: LocatableDiagnostic,
    /// Whether to omit reporting this error.
    pub avoid_reporting: bool,
    /// Whether this error was an exception thrown during constant evaluation.
    pub is_runtime_exception: bool,
    /// Whether the constant evaluation encounters an unresolved expression.
    pub is_unresolved: bool,
}

impl InvalidConstant {
    /// Dart `InvalidConstant._` / `InvalidConstant.forEntity` (with the
    /// entity's `offset` and `length`).
    pub fn new(offset: i64, length: i64, locatable_diagnostic: LocatableDiagnostic) -> Self {
        InvalidConstant {
            offset,
            length,
            locatable_diagnostic,
            avoid_reporting: false,
            is_runtime_exception: false,
            is_unresolved: false,
        }
    }

    /// Dart `InvalidConstant.forElement`: an error at the name of [element]
    /// (`element.name!.length`, `element.firstFragment.nameOffset ?? -1`).
    pub fn for_element(
        ctx: &Ctx<'_>,
        element: ElementId,
        locatable_diagnostic: LocatableDiagnostic,
    ) -> Self {
        let data = ctx.element_data(element).expect("element with data");
        let name = data.name.expect("Dart `element.name!`");
        let length = dart_num::string_length(ctx.name_str(name));
        let offset = ctx
            .fragment_data(data.first_fragment)
            .and_then(|f| f.name_offset)
            .map_or(-1, i64::from);
        InvalidConstant::new(offset, length, locatable_diagnostic)
    }
}

/// Dart `ConstructorInvocationImpl`.
#[derive(Clone, Debug)]
pub struct ConstructorInvocationImpl {
    /// Dart `ConstructorElement` (a base element or a substituted member).
    pub constructor: ElemRef,
    pub positional_arguments: Vec<DartObjectImpl>,
    pub named_arguments: FieldMap,
}

// ---------------------------------------------------------------------------
// DartObjectImpl
// ---------------------------------------------------------------------------

/// Dart `DartObjectImpl`: a representation of an instance of a Dart class.
#[derive(Clone, Debug)]
pub struct DartObjectImpl {
    /// Dart `type` (the extension type erasure of the given type).
    pub ty: TypeId,
    /// Dart `typeNotExtensionTypeErased`.
    pub type_not_extension_type_erased: TypeId,
    /// The state of the object.
    pub state: InstanceState,
    /// Dart `variable`.
    pub variable: Option<EId<VariableElement>>,
}

/// Whether [ty] is an interface type of the `dart:core` class [name]
/// (Dart `isDartCoreBool`, `isDartCoreInt`, ...; the nullability does not
/// matter).
pub fn is_dart_core_type(ctx: &Ctx<'_>, ty: TypeId, name: &str) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => is_dart_core_element_named(ctx, element.raw(), name),
        _ => false,
    }
}

/// Whether [element] has the [name] and is declared in `dart:core`.
fn is_dart_core_element_named(ctx: &Ctx<'_>, element: ElementId, name: &str) -> bool {
    let Some(data) = ctx.element_data(element) else {
        return false;
    };
    if data.name.map(|n| ctx.name_str(n)) != Some(name) {
        return false;
    }
    let Some(library) = data.library else {
        return false;
    };
    let unit = ctx.get(library).first_fragment();
    &*ctx.fragment(unit).source.uri == "dart:core"
}

impl DartObjectImpl {
    /// Dart `DartObjectImpl(typeSystem, type, state, variable: variable)`.
    pub fn new(ts: &dyn ConstTypeSystem, ty: TypeId, state: InstanceState) -> DartObjectImpl {
        DartObjectImpl {
            ty: ts.extension_type_erasure(ty),
            type_not_extension_type_erased: ty,
            state,
            variable: None,
        }
    }

    /// Dart `DartObjectImpl.forVariable`: a duplicate of [other], tied to
    /// [variable].
    pub fn for_variable(other: DartObjectImpl, variable: EId<VariableElement>) -> DartObjectImpl {
        DartObjectImpl {
            variable: Some(variable),
            ..other
        }
    }

    /// Dart `DartObjectImpl.validWithUnknownValue`: an object that represents
    /// an unknown value.
    pub fn valid_with_unknown_value(ts: &dyn ConstTypeSystem, ty: TypeId) -> DartObjectImpl {
        let ctx = ts.ctx();
        let dynamic = ctx.tp.dynamic_type();
        let state = if is_dart_core_type(&ctx, ty, "bool") {
            InstanceState::Bool(BoolState::UNKNOWN_VALUE)
        } else if is_dart_core_type(&ctx, ty, "double") {
            InstanceState::Double(DoubleState::UNKNOWN_VALUE)
        } else if is_dart_core_type(&ctx, ty, "int") {
            InstanceState::Int(IntState::UNKNOWN_VALUE)
        } else if is_dart_core_type(&ctx, ty, "List") {
            InstanceState::List(Arc::new(ListState::unknown(ts, dynamic)))
        } else if is_dart_core_type(&ctx, ty, "Map") {
            InstanceState::Map(Arc::new(MapState::unknown(ts, dynamic, dynamic)))
        } else if is_dart_core_type(&ctx, ty, "Set") {
            InstanceState::Set(Arc::new(SetState::unknown(ts, dynamic)))
        } else if is_dart_core_type(&ctx, ty, "String") {
            InstanceState::String(StringState::UNKNOWN_VALUE)
        } else {
            InstanceState::Generic(Arc::new(GenericState::new(FieldMap::new(), None, true)))
        };
        DartObjectImpl::new(ts, ty, state)
    }

    /// Dart `DartObjectImpl.withExtensionType`.
    pub fn with_extension_type(
        ty: TypeId,
        type_not_extension_type_erased: TypeId,
        state: InstanceState,
        variable: Option<EId<VariableElement>>,
    ) -> DartObjectImpl {
        DartObjectImpl {
            ty,
            type_not_extension_type_erased,
            state,
            variable,
        }
    }

    /// Dart `constructorInvocation`.
    pub fn constructor_invocation(&self) -> Option<&ConstructorInvocationImpl> {
        match &self.state {
            InstanceState::Generic(s) => s.invocation.as_deref(),
            _ => None,
        }
    }

    /// Dart `fields`.
    pub fn fields(&self) -> Option<&FieldMap> {
        self.state.fields()
    }

    /// Dart `hasKnownValue`.
    pub fn has_known_value(&self) -> bool {
        !self.state.is_unknown()
    }

    /// Dart `isBool`.
    pub fn is_bool(&self) -> bool {
        self.state.is_bool()
    }

    /// Dart `isBoolNumStringOrNull`.
    pub fn is_bool_num_string_or_null(&self) -> bool {
        self.state.is_bool_num_string_or_null()
    }

    /// Dart `isInt`.
    pub fn is_int(&self) -> bool {
        self.state.is_int()
    }

    /// Dart `isInvalid`.
    pub fn is_invalid(&self, ctx: &Ctx<'_>) -> bool {
        self.state.is_invalid() || has_invalid_type(ctx, self.ty)
    }

    /// Dart `isNull`.
    pub fn is_null(&self) -> bool {
        matches!(self.state, InstanceState::Null(_))
    }

    /// Dart `isUnknown`.
    pub fn is_unknown(&self) -> bool {
        self.state.is_unknown()
    }

    /// Dart `operator ==`.
    pub fn dart_eq(&self, other: &DartObjectImpl, ts: &dyn ConstTypeSystem) -> bool {
        ts.runtime_types_equal(self.ty, other.ty) && self.state.state_eq(&other.state, ts)
    }

    /// A hash that is consistent with [`DartObjectImpl::dart_eq`] (Dart
    /// `hashCode` is `Object.hash(type, state)`; the type is left out here,
    /// because runtime type equality is not identity). Never printed.
    pub fn dart_hash(&self) -> u64 {
        self.state.state_hash()
    }

    fn with_type(ts: &dyn ConstTypeSystem, ty: TypeId, state: InstanceState) -> DartObjectImpl {
        DartObjectImpl::new(ts, ty, state)
    }

    fn num_result(ts: &dyn ConstTypeSystem, result: InstanceState, op: &str) -> DartObjectImpl {
        let tp = ts.ctx().tp;
        match result {
            InstanceState::Int(_) => Self::with_type(ts, tp.int_type(), result),
            InstanceState::Double(_) => Self::with_type(ts, tp.double_type(), result),
            InstanceState::String(_) if op == "add" => {
                Self::with_type(ts, tp.string_type(), result)
            }
            // We should never get here.
            _ => panic!("{op} returned a {}", result.runtime_type_name()),
        }
    }

    fn bool_result(ts: &dyn ConstTypeSystem, state: BoolState) -> DartObjectImpl {
        Self::with_type(ts, ts.ctx().tp.bool_type(), InstanceState::Bool(state))
    }

    fn int_result(ts: &dyn ConstTypeSystem, state: IntState) -> DartObjectImpl {
        Self::with_type(ts, ts.ctx().tp.int_type(), InstanceState::Int(state))
    }

    /// Dart `add`: the `+` operator.
    pub fn add(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        let result = self.state.add(&right.state)?;
        Ok(Self::num_result(ts, result, "add"))
    }

    /// Dart `bitNot`: the `~` operator.
    pub fn bit_not(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        Ok(Self::int_result(ts, self.state.bit_not()?))
    }

    /// Dart `castToType`.
    pub fn cast_to_type(
        &self,
        ts: &dyn ConstTypeSystem,
        cast_type: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Self::assert_type(cast_type)?;
        let InstanceState::Type(type_state) = &cast_type.state else {
            unreachable!()
        };
        // If we don't know the type, we cannot prove that the cast will fail.
        let Some(result_type) = type_state.ty else {
            return Ok(self.clone());
        };
        let ctx = ts.ctx();
        // If any type is unresolved, we cannot prove that the cast will fail.
        if self.is_invalid(&ctx) || cast_type.is_invalid(&ctx) {
            return Ok(self.clone());
        }
        // We don't know the actual value of a type parameter.
        // So, the object type might be a subtype of the result type.
        if has_type_parameter_reference(&ctx, result_type) {
            return Ok(self.clone());
        }
        if !ts.is_subtype_of(self.ty, result_type) {
            return Err(exception(diag::const_eval_throws_exception()));
        }
        Ok(self.clone())
    }

    /// Dart `concatenate`: string juxtaposition / interpolation.
    pub fn concatenate(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        let state = self.state.concatenate(&right.state)?;
        Ok(Self::with_type(
            ts,
            ts.ctx().tp.string_type(),
            InstanceState::String(state),
        ))
    }

    /// Dart `convertToBool`.
    pub fn convert_to_bool(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        let bool_type = ts.ctx().tp.bool_type();
        // Dart: identical
        if self.ty == bool_type {
            return Ok(self.clone());
        }
        Ok(Self::bool_result(ts, self.state.convert_to_bool()?))
    }

    /// Dart `divide`: the `/` operator.
    pub fn divide(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        let result = self.state.divide(&right.state)?;
        Ok(Self::num_result(ts, result, "divide"))
    }

    /// Dart `eagerAnd`: the `&` operator.
    pub fn eager_and(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        if self.is_bool() && right.is_bool() {
            return Ok(Self::bool_result(ts, self.state.logical_and(&right.state)?));
        } else if self.is_int() && right.is_int() {
            return Ok(Self::int_result(ts, self.state.bit_and(&right.state)?));
        }
        Err(exception(diag::const_eval_type_bool_int()))
    }

    /// Dart `eagerOr`: the `|` operator.
    pub fn eager_or(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        if self.is_bool() && right.is_bool() {
            return Ok(Self::bool_result(ts, self.state.logical_or(&right.state)?));
        } else if self.is_int() && right.is_int() {
            return Ok(Self::int_result(ts, self.state.bit_or(&right.state)?));
        }
        Err(exception(diag::const_eval_type_bool_int()))
    }

    /// Dart `eagerXor`: the `^` operator.
    pub fn eager_xor(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        if self.is_bool() && right.is_bool() {
            return Ok(Self::bool_result(ts, self.state.logical_xor(&right.state)?));
        } else if self.is_int() && right.is_int() {
            return Ok(Self::int_result(ts, self.state.bit_xor(&right.state)?));
        }
        Err(exception(diag::const_eval_type_bool_int()))
    }

    /// Dart `equalEqual`: the `==` operator.
    pub fn equal_equal(
        &self,
        ts: &dyn ConstTypeSystem,
        feature_set: &FeatureSet,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        if self.is_null() || right.is_null() {
            let state = if self.is_null() && right.is_null() {
                BoolState::TRUE_STATE
            } else {
                BoolState::FALSE_STATE
            };
            return Ok(Self::bool_result(ts, state));
        }
        let patterns = feature_set.is_enabled("patterns");
        if patterns {
            if matches!(self.state, InstanceState::Double(_))
                || self.has_primitive_equality(ts, feature_set)
            {
                return Ok(Self::bool_result(
                    ts,
                    self.state.equal_equal(ts, &right.state),
                ));
            }
        } else if self.is_bool_num_string_or_null() {
            return Ok(Self::bool_result(
                ts,
                self.state.equal_equal(ts, &right.state),
            ));
        }
        Err(exception(if patterns {
            diag::const_eval_primitive_equality()
        } else {
            diag::const_eval_type_bool_num_string()
        }))
    }

    /// Dart `getField`.
    pub fn get_field(&self, name: &str) -> Option<&DartObjectImpl> {
        match &self.state {
            InstanceState::Generic(s) => s.fields.get(name),
            InstanceState::Record(s) => s.get_field(name),
            _ => None,
        }
    }

    /// Dart `greaterThan`: the `>` operator.
    pub fn greater_than(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::bool_result(
            ts,
            self.state.greater_than(&right.state)?,
        ))
    }

    /// Dart `greaterThanOrEqual`: the `>=` operator.
    pub fn greater_than_or_equal(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::bool_result(
            ts,
            self.state.greater_than_or_equal(&right.state)?,
        ))
    }

    /// Dart `hasPrimitiveEquality`: whether this value, inside a library
    /// with the [feature_set], has primitive equality, so can be used at
    /// compile-time.
    pub fn has_primitive_equality(
        &self,
        ts: &dyn ConstTypeSystem,
        feature_set: &FeatureSet,
    ) -> bool {
        self.state.has_primitive_equality(ts, feature_set, self.ty)
    }

    /// Dart `hasType`: the `is` test.
    pub fn has_type(
        &self,
        ts: &dyn ConstTypeSystem,
        tested_type: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Self::assert_type(tested_type)?;
        let InstanceState::Type(type_state) = &tested_type.state else {
            unreachable!()
        };
        let state = match type_state.ty {
            None => BoolState::TRUE_STATE,
            Some(type_type) => BoolState::from(ts.is_subtype_of(self.ty, type_type)),
        };
        Ok(Self::bool_result(ts, state))
    }

    /// Dart `integerDivide`: the `~/` operator.
    pub fn integer_divide(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::int_result(
            ts,
            self.state.integer_divide(&right.state)?,
        ))
    }

    /// Dart `isIdentical2`: the `identical` function.
    pub fn is_identical2(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> Self {
        let ctx = ts.ctx();
        // Workaround for Flutter `const kIsWeb = identical(0, 0.0)`.
        if is_dart_core_type(&ctx, self.ty, "int") && is_dart_core_type(&ctx, right.ty, "double")
            || is_dart_core_type(&ctx, self.ty, "double")
                && is_dart_core_type(&ctx, right.ty, "int")
        {
            return Self::bool_result(ts, BoolState::UNKNOWN_VALUE);
        }
        if !ts.runtime_types_equal(self.ty, right.ty) {
            return Self::bool_result(ts, BoolState::new(Some(false)));
        }
        Self::bool_result(ts, self.state.is_identical(ts, &right.state))
    }

    /// Dart `lazyAnd`: the `&&` operator.
    pub fn lazy_and(
        &self,
        ts: &dyn ConstTypeSystem,
        right_operand_computer: impl FnOnce() -> EvalResult<DartObjectImpl>,
    ) -> EvalResult<Self> {
        let state = self
            .state
            .lazy_and(|| right_operand_computer().map(|o| Some(o.state)))?;
        Ok(Self::bool_result(ts, state))
    }

    /// Dart `lazyOr`: the `||` operator.
    pub fn lazy_or(
        &self,
        ts: &dyn ConstTypeSystem,
        right_operand_computer: impl FnOnce() -> EvalResult<DartObjectImpl>,
    ) -> EvalResult<Self> {
        let state = self
            .state
            .lazy_or(|| right_operand_computer().map(|o| Some(o.state)))?;
        Ok(Self::bool_result(ts, state))
    }

    /// Dart `lessThan`: the `<` operator.
    pub fn less_than(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        Ok(Self::bool_result(ts, self.state.less_than(&right.state)?))
    }

    /// Dart `lessThanOrEqual`: the `<=` operator.
    pub fn less_than_or_equal(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::bool_result(
            ts,
            self.state.less_than_or_equal(&right.state)?,
        ))
    }

    /// Dart `logicalNot`: the `!` operator.
    pub fn logical_not(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        Ok(Self::bool_result(ts, self.state.logical_not()?))
    }

    /// Dart `logicalShiftRight`: the `>>>` operator.
    pub fn logical_shift_right(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::int_result(
            ts,
            self.state.logical_shift_right(&right.state)?,
        ))
    }

    /// Dart `minus`: the binary `-` operator.
    pub fn minus(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        let result = self.state.minus(&right.state)?;
        Ok(Self::num_result(ts, result, "minus"))
    }

    /// Dart `negated`: the unary `-` operator.
    pub fn negated(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        let result = self.state.negated()?;
        Ok(Self::num_result(ts, result, "negated"))
    }

    /// Dart `notEqual`: the `!=` operator.
    pub fn not_equal(
        &self,
        ts: &dyn ConstTypeSystem,
        feature_set: &FeatureSet,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        self.equal_equal(ts, feature_set, right)?.logical_not(ts)
    }

    /// Dart `performToString`: string interpolation of this object.
    pub fn perform_to_string(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        let string_type = ts.ctx().tp.string_type();
        // Dart: identical
        if self.ty == string_type {
            return Ok(self.clone());
        }
        Ok(Self::with_type(
            ts,
            string_type,
            InstanceState::String(self.state.convert_to_string(ts)?),
        ))
    }

    /// Dart `remainder`: the `%` operator.
    pub fn remainder(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        let result = self.state.remainder(&right.state)?;
        Ok(Self::num_result(ts, result, "remainder"))
    }

    /// Dart `shiftLeft`: the `<<` operator.
    pub fn shift_left(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        Ok(Self::int_result(ts, self.state.shift_left(&right.state)?))
    }

    /// Dart `shiftRight`: the `>>` operator.
    pub fn shift_right(
        &self,
        ts: &dyn ConstTypeSystem,
        right: &DartObjectImpl,
    ) -> EvalResult<Self> {
        Ok(Self::int_result(ts, self.state.shift_right(&right.state)?))
    }

    /// Dart `stringLength`: the `length` getter of a `String`.
    pub fn string_length(&self, ts: &dyn ConstTypeSystem) -> EvalResult<Self> {
        Ok(Self::int_result(ts, self.state.string_length()?))
    }

    /// Dart `times`: the `*` operator.
    pub fn times(&self, ts: &dyn ConstTypeSystem, right: &DartObjectImpl) -> EvalResult<Self> {
        let result = self.state.times(&right.state)?;
        Ok(Self::num_result(ts, result, "times"))
    }

    /// Dart `toBoolValue`.
    pub fn to_bool_value(&self) -> Option<bool> {
        match &self.state {
            InstanceState::Bool(s) => s.value,
            _ => None,
        }
    }

    /// Dart `toDoubleValue`.
    pub fn to_double_value(&self) -> Option<f64> {
        match &self.state {
            InstanceState::Double(s) => s.value,
            _ => None,
        }
    }

    /// Dart `toFunctionValue`.
    pub fn to_function_value(&self) -> Option<EId<ExecutableElement>> {
        match &self.state {
            InstanceState::Function(s) => Some(s.element),
            _ => None,
        }
    }

    /// Dart `toIntValue`.
    pub fn to_int_value(&self) -> Option<i64> {
        match &self.state {
            InstanceState::Int(s) => s.value,
            _ => None,
        }
    }

    /// Dart `toListValue`.
    pub fn to_list_value(&self) -> Option<&[DartObjectImpl]> {
        match &self.state {
            InstanceState::List(s) => Some(&s.elements),
            _ => None,
        }
    }

    /// Dart `toMapValue`.
    pub fn to_map_value(&self) -> Option<&DartObjectMap> {
        match &self.state {
            InstanceState::Map(s) => Some(&s.entries),
            _ => None,
        }
    }

    /// Dart `toRecordValue`: the positional and named fields.
    pub fn to_record_value(&self) -> Option<(&[DartObjectImpl], &FieldMap)> {
        match &self.state {
            InstanceState::Record(s) => Some((&s.positional_fields, &s.named_fields)),
            _ => None,
        }
    }

    /// Dart `toSetValue`.
    pub fn to_set_value(&self) -> Option<&DartObjectSet> {
        match &self.state {
            InstanceState::Set(s) => Some(&s.elements),
            _ => None,
        }
    }

    /// Dart `toString()`: `"<type display string> (<state>)"`, used in
    /// diagnostics and in the `const` field of the `elements` dump.
    pub fn display(&self, ts: &dyn ConstTypeSystem) -> String {
        format!(
            "{} ({})",
            ts.display_string(self.ty),
            self.state.display(ts)
        )
    }

    /// Dart `toStringValue`.
    pub fn to_string_value(&self) -> Option<&str> {
        match &self.state {
            InstanceState::String(s) => s.value.as_deref(),
            _ => None,
        }
    }

    /// Dart `toSymbolValue`.
    pub fn to_symbol_value(&self) -> Option<&str> {
        match &self.state {
            InstanceState::Symbol(s) => s.value.as_deref(),
            _ => None,
        }
    }

    /// Dart `toTypeValue`.
    pub fn to_type_value(&self) -> Option<TypeId> {
        match &self.state {
            InstanceState::Type(s) => s.ty,
            _ => None,
        }
    }

    /// Dart `toTypeValueNotExtensionTypeErased`.
    pub fn to_type_value_not_extension_type_erased(&self) -> Option<TypeId> {
        match &self.state {
            InstanceState::Type(s) => s.type_not_extension_type_erased,
            _ => None,
        }
    }

    /// Dart `typeInstantiate`: the result of type-instantiating this
    /// function object as [ty] with the [type_arguments].
    pub fn type_instantiate(
        &self,
        ts: &dyn ConstTypeSystem,
        ty: TypeId,
        type_arguments: Vec<TypeId>,
    ) -> Self {
        let InstanceState::Function(function_state) = &self.state else {
            panic!("Dart `state as FunctionState` failed");
        };
        Self::with_type(
            ts,
            ty,
            InstanceState::Function(Arc::new(FunctionState::new(
                function_state.element,
                Some(type_arguments),
                function_state.via_type_alias,
            ))),
        )
    }

    /// Dart `updateEnumConstant`: set the `index` and `_name` fields for
    /// this enum constant.
    pub fn update_enum_constant(&mut self, ts: &dyn ConstTypeSystem, index: i64, name: &str) {
        let tp = ts.ctx().tp;
        let index = DartObjectImpl::new(
            ts,
            tp.int_type(),
            InstanceState::Int(IntState::new(Some(index))),
        );
        let name = DartObjectImpl::new(
            ts,
            tp.string_type(),
            InstanceState::String(StringState::new(name)),
        );
        let InstanceState::Generic(state) = &mut self.state else {
            panic!("Dart `this.state as GenericState` failed");
        };
        let fields = &mut Arc::make_mut(state).fields;
        fields.insert(Arc::from("index"), index);
        fields.insert(Arc::from("_name"), name);
    }

    /// Dart `_assertType`: throw if the [object]'s state does not represent
    /// a Type value.
    fn assert_type(object: &DartObjectImpl) -> EvalResult<()> {
        if !matches!(object.state, InstanceState::Type(_)) {
            return Err(exception(diag::const_eval_type_type()));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// States
// ---------------------------------------------------------------------------

/// Dart `BoolState`: the state of an object representing a boolean value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoolState {
    /// The value of this instance (`None`: unknown).
    pub value: Option<bool>,
}

impl BoolState {
    /// An instance representing the boolean value 'false'.
    pub const FALSE_STATE: BoolState = BoolState { value: Some(false) };
    /// An instance representing the boolean value 'true'.
    pub const TRUE_STATE: BoolState = BoolState { value: Some(true) };
    /// A state that can be used to represent a boolean whose value is not
    /// known.
    pub const UNKNOWN_VALUE: BoolState = BoolState { value: None };

    pub fn new(value: Option<bool>) -> BoolState {
        BoolState { value }
    }

    /// Dart `BoolState.from`.
    pub fn from(value: bool) -> BoolState {
        if value {
            Self::TRUE_STATE
        } else {
            Self::FALSE_STATE
        }
    }
}

/// Dart `DoubleState`.
#[derive(Clone, Copy, Debug)]
pub struct DoubleState {
    /// The value of this instance (`None`: unknown).
    pub value: Option<f64>,
}

impl DoubleState {
    /// A state that can be used to represent a double whose value is not
    /// known.
    pub const UNKNOWN_VALUE: DoubleState = DoubleState { value: None };

    pub fn new(value: Option<f64>) -> DoubleState {
        DoubleState { value }
    }

    fn known(value: f64) -> InstanceState {
        InstanceState::Double(DoubleState::new(Some(value)))
    }
}

/// Dart `IntState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntState {
    /// The value of this instance (`None`: unknown).
    pub value: Option<i64>,
}

impl IntState {
    /// A state that can be used to represent an int whose value is not known.
    pub const UNKNOWN_VALUE: IntState = IntState { value: None };

    pub fn new(value: Option<i64>) -> IntState {
        IntState { value }
    }

    fn known(value: i64) -> IntState {
        IntState::new(Some(value))
    }
}

/// Dart `StringState`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringState {
    /// The value of this instance (`None`: unknown).
    pub value: Option<Arc<str>>,
}

impl StringState {
    /// A state that can be used to represent a string whose value is not
    /// known.
    pub const UNKNOWN_VALUE: StringState = StringState { value: None };

    pub fn new(value: impl Into<Arc<str>>) -> StringState {
        StringState {
            value: Some(value.into()),
        }
    }
}

/// Dart `SymbolState`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SymbolState {
    /// The value of this instance (`None`: unknown).
    pub value: Option<Arc<str>>,
}

impl SymbolState {
    pub fn new(value: Option<Arc<str>>) -> SymbolState {
        SymbolState { value }
    }
}

/// Dart `TypeState`.
#[derive(Clone, Copy, Debug)]
pub struct TypeState {
    /// Dart `type`: the type being modeled (extension type erased).
    pub ty: Option<TypeId>,
    /// From the point of view of constant evaluation, the value is [ty].
    /// But some clients want to know the explicit provided type.
    pub type_not_extension_type_erased: Option<TypeId>,
}

impl TypeState {
    /// Dart `TypeState(type)`.
    pub fn new(ts: &dyn ConstTypeSystem, ty: Option<TypeId>) -> TypeState {
        TypeState {
            ty: ty.map(|t| ts.extension_type_erasure(t)),
            type_not_extension_type_erased: ty,
        }
    }
}

/// Dart `FunctionState`.
#[derive(Clone, Debug)]
pub struct FunctionState {
    /// The element representing the function being modeled.
    pub element: EId<ExecutableElement>,
    pub type_arguments: Option<Vec<TypeId>>,
    /// Dart `_viaTypeAlias`: the type alias which was referenced when tearing
    /// off a constructor, if this function is a constructor tear-off,
    /// referenced via a type alias, and the type alias is not a proper rename
    /// for the class, and the constructor tear-off is generic, so the
    /// tear-off cannot be considered equivalent to tearing off the associated
    /// constructor function of the aliased class. Otherwise `None`.
    pub via_type_alias: Option<EId<TypeAliasElement>>,
}

impl FunctionState {
    pub fn new(
        element: EId<ExecutableElement>,
        type_arguments: Option<Vec<TypeId>>,
        via_type_alias: Option<EId<TypeAliasElement>>,
    ) -> FunctionState {
        FunctionState {
            element,
            type_arguments,
            via_type_alias,
        }
    }
}

/// Dart `GenericState`: the state of an object representing a Dart object
/// for which there is no more specific state.
#[derive(Clone, Debug)]
pub struct GenericState {
    /// The values of the fields of this instance (Dart `_fieldMap`).
    pub fields: FieldMap,
    /// Information about the constructor invoked to generate this instance.
    pub invocation: Option<Arc<ConstructorInvocationImpl>>,
    /// Dart `isUnknown`.
    pub is_unknown: bool,
}

impl GenericState {
    /// Pseudo-field that we use to represent fields in the superclass.
    pub const SUPERCLASS_FIELD: &'static str = "(super)";

    pub fn new(
        fields: FieldMap,
        invocation: Option<Arc<ConstructorInvocationImpl>>,
        is_unknown: bool,
    ) -> GenericState {
        GenericState {
            fields,
            invocation,
            is_unknown,
        }
    }
}

/// Dart `ListState`.
#[derive(Clone, Debug)]
pub struct ListState {
    /// The element type (extension type erased).
    pub element_type: TypeId,
    pub elements: Vec<DartObjectImpl>,
    pub is_unknown: bool,
}

impl ListState {
    pub fn new(
        ts: &dyn ConstTypeSystem,
        element_type: TypeId,
        elements: Vec<DartObjectImpl>,
        is_unknown: bool,
    ) -> ListState {
        ListState {
            element_type: ts.extension_type_erasure(element_type),
            elements,
            is_unknown,
        }
    }

    /// Dart `ListState.unknown`: a list whose value is not known.
    pub fn unknown(ts: &dyn ConstTypeSystem, element_type: TypeId) -> ListState {
        ListState::new(ts, element_type, Vec::new(), true)
    }
}

/// Dart `MapState`.
#[derive(Clone, Debug)]
pub struct MapState {
    /// Dart `_keyType` (extension type erased).
    pub key_type: TypeId,
    /// Dart `_valueType` (extension type erased).
    pub value_type: TypeId,
    /// The entries in the map.
    pub entries: DartObjectMap,
    /// Whether the map contains an entry that has an unknown value.
    pub is_unknown: bool,
}

impl MapState {
    pub fn new(
        ts: &dyn ConstTypeSystem,
        key_type: TypeId,
        value_type: TypeId,
        entries: DartObjectMap,
        is_unknown: bool,
    ) -> MapState {
        MapState {
            key_type: ts.extension_type_erasure(key_type),
            value_type: ts.extension_type_erasure(value_type),
            entries,
            is_unknown,
        }
    }

    /// Dart `MapState.unknown`: a map whose value is not known.
    pub fn unknown(ts: &dyn ConstTypeSystem, key_type: TypeId, value_type: TypeId) -> MapState {
        MapState::new(ts, key_type, value_type, DartObjectMap::new(), true)
    }
}

/// Dart `NullState`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullState {
    /// Dart `isInvalid`.
    pub is_invalid: bool,
}

impl NullState {
    /// An instance representing the value 'null'.
    pub const NULL_STATE: NullState = NullState { is_invalid: false };
}

/// Dart `RecordState`.
#[derive(Clone, Debug)]
pub struct RecordState {
    /// The values of the positional fields.
    pub positional_fields: Vec<DartObjectImpl>,
    /// The values of the named fields.
    pub named_fields: FieldMap,
}

impl RecordState {
    pub fn new(positional_fields: Vec<DartObjectImpl>, named_fields: FieldMap) -> RecordState {
        RecordState {
            positional_fields,
            named_fields,
        }
    }

    /// Dart `getField`: the value of the field with the given [name].
    pub fn get_field(&self, name: &str) -> Option<&DartObjectImpl> {
        match positional_field_index(name) {
            Some(index) if index < self.positional_fields.len() as i64 => {
                Some(&self.positional_fields[index as usize])
            }
            _ => self.named_fields.get(name),
        }
    }
}

/// Dart `RecordTypeExtension.positionalFieldIndex`: parse `$1`, `$2`, ...
/// (`^\$[1-9]\d*$`; numerals that do not fit in an `int` give `None`).
pub fn positional_field_index(name: &str) -> Option<i64> {
    let digits = name.strip_prefix('$')?;
    let mut chars = digits.chars();
    if !matches!(chars.next(), Some('1'..='9')) || !chars.all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok().map(|position| position - 1)
}

/// Dart `SetState`.
#[derive(Clone, Debug)]
pub struct SetState {
    /// Dart `_elementType` (extension type erased).
    pub element_type: TypeId,
    /// The elements of the set.
    pub elements: DartObjectSet,
    /// Whether the set contains an entry that has an unknown value.
    pub is_unknown: bool,
}

impl SetState {
    pub fn new(
        ts: &dyn ConstTypeSystem,
        element_type: TypeId,
        elements: DartObjectSet,
        is_unknown: bool,
    ) -> SetState {
        SetState {
            element_type: ts.extension_type_erasure(element_type),
            elements,
            is_unknown,
        }
    }

    /// Dart `SetState.unknown`: a set whose value is not known.
    pub fn unknown(ts: &dyn ConstTypeSystem, element_type: TypeId) -> SetState {
        SetState::new(ts, element_type, DartObjectSet::new(), true)
    }
}

/// Dart `InstanceState` and its subclasses. `NumState` is the pair
/// [`InstanceState::Int`] / [`InstanceState::Double`].
#[derive(Clone, Debug)]
pub enum InstanceState {
    Bool(BoolState),
    Double(DoubleState),
    Function(Arc<FunctionState>),
    Generic(Arc<GenericState>),
    Int(IntState),
    List(Arc<ListState>),
    Map(Arc<MapState>),
    Null(NullState),
    Record(Arc<RecordState>),
    Set(Arc<SetState>),
    String(StringState),
    Symbol(SymbolState),
    Type(TypeState),
}

// The `assert*` helpers of `InstanceState`.

/// Dart `assertBool`.
fn assert_bool(state: Option<&InstanceState>) -> EvalResult<()> {
    if !matches!(state, Some(InstanceState::Bool(_))) {
        return Err(exception(diag::const_eval_type_bool()));
    }
    Ok(())
}

/// Dart `assertIntOrNull`.
fn assert_int_or_null(state: &InstanceState) -> EvalResult<()> {
    if !matches!(state, InstanceState::Int(_) | InstanceState::Null(_)) {
        return Err(exception(diag::const_eval_type_int()));
    }
    Ok(())
}

/// Dart `assertNumOrNull`.
fn assert_num_or_null(state: &InstanceState) -> EvalResult<()> {
    if !(state.is_num() || matches!(state, InstanceState::Null(_))) {
        return Err(exception(diag::const_eval_type_num()));
    }
    Ok(())
}

/// Dart `assertNumStringOrNull`.
fn assert_num_string_or_null(state: &InstanceState) -> EvalResult<()> {
    if !(state.is_num() || matches!(state, InstanceState::String(_) | InstanceState::Null(_))) {
        return Err(exception(diag::const_eval_type_num_string()));
    }
    Ok(())
}

/// Dart `assertString`.
fn assert_string(state: &InstanceState) -> EvalResult<()> {
    if !matches!(state, InstanceState::String(_)) {
        return Err(exception(diag::const_eval_type_string()));
    }
    Ok(())
}

fn throws_exception<T>() -> EvalResult<T> {
    Err(exception(diag::const_eval_throws_exception()))
}

/// The value of an int or double operand: `Ok(None)` when it is unknown.
enum Num {
    Int(Option<i64>),
    Double(Option<f64>),
}

fn num_operand(state: &InstanceState) -> Option<Num> {
    match state {
        InstanceState::Int(s) => Some(Num::Int(s.value)),
        InstanceState::Double(s) => Some(Num::Double(s.value)),
        _ => None,
    }
}

fn hash_one<T: Hash>(value: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn combine_ordered(seed: u64, value: u64) -> u64 {
    seed.wrapping_mul(31).wrapping_add(value)
}

impl InstanceState {
    /// The Dart class name of the state, for messages.
    pub fn runtime_type_name(&self) -> &'static str {
        match self {
            InstanceState::Bool(_) => "BoolState",
            InstanceState::Double(_) => "DoubleState",
            InstanceState::Function(_) => "FunctionState",
            InstanceState::Generic(_) => "GenericState",
            InstanceState::Int(_) => "IntState",
            InstanceState::List(_) => "ListState",
            InstanceState::Map(_) => "MapState",
            InstanceState::Null(_) => "NullState",
            InstanceState::Record(_) => "RecordState",
            InstanceState::Set(_) => "SetState",
            InstanceState::String(_) => "StringState",
            InstanceState::Symbol(_) => "SymbolState",
            InstanceState::Type(_) => "TypeState",
        }
    }

    fn is_num(&self) -> bool {
        matches!(self, InstanceState::Int(_) | InstanceState::Double(_))
    }

    /// Dart `fields`: the fields of a generic Dart object, else `None`.
    pub fn fields(&self) -> Option<&FieldMap> {
        match self {
            InstanceState::Generic(s) => Some(&s.fields),
            _ => None,
        }
    }

    /// Dart `isBool`.
    pub fn is_bool(&self) -> bool {
        matches!(self, InstanceState::Bool(_))
    }

    /// Dart `isBoolNumStringOrNull`.
    pub fn is_bool_num_string_or_null(&self) -> bool {
        matches!(
            self,
            InstanceState::Bool(_)
                | InstanceState::Int(_)
                | InstanceState::Double(_)
                | InstanceState::String(_)
                | InstanceState::Null(_)
        )
    }

    /// Dart `isInt`.
    pub fn is_int(&self) -> bool {
        matches!(self, InstanceState::Int(_))
    }

    /// Dart `isInvalid`.
    pub fn is_invalid(&self) -> bool {
        match self {
            InstanceState::Null(s) => s.is_invalid,
            _ => false,
        }
    }

    /// Dart `isNull`.
    pub fn is_null(&self) -> bool {
        matches!(self, InstanceState::Null(_))
    }

    /// Dart `isUnknown`.
    pub fn is_unknown(&self) -> bool {
        match self {
            InstanceState::Bool(s) => s.value.is_none(),
            InstanceState::Double(s) => s.value.is_none(),
            InstanceState::Int(s) => s.value.is_none(),
            InstanceState::String(s) => s.value.is_none(),
            InstanceState::Generic(s) => s.is_unknown,
            InstanceState::List(s) => s.is_unknown,
            InstanceState::Map(s) => s.is_unknown,
            InstanceState::Set(s) => s.is_unknown,
            InstanceState::Function(_)
            | InstanceState::Null(_)
            | InstanceState::Record(_)
            | InstanceState::Symbol(_)
            | InstanceState::Type(_) => false,
        }
    }

    /// Dart `typeName`: the name of the type of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            InstanceState::Bool(_) => "bool",
            InstanceState::Double(_) => "double",
            InstanceState::Function(_) => "Function",
            InstanceState::Generic(_) => "user defined type",
            InstanceState::Int(_) => "int",
            InstanceState::List(_) => "List",
            InstanceState::Map(_) => "Map",
            InstanceState::Null(_) => "Null",
            InstanceState::Record(_) => "Record",
            InstanceState::Set(_) => "Set",
            InstanceState::String(_) => "String",
            InstanceState::Symbol(_) => "Symbol",
            InstanceState::Type(_) => "Type",
        }
    }

    /// Dart `add`: the `+` operator.
    pub fn add(&self, right: &InstanceState) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    if let InstanceState::Double(_) = right {
                        return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
                    }
                    return Ok(InstanceState::Int(IntState::UNKNOWN_VALUE));
                };
                match num_operand(right) {
                    Some(Num::Int(None)) => Ok(InstanceState::Int(IntState::UNKNOWN_VALUE)),
                    Some(Num::Int(Some(r))) => {
                        Ok(InstanceState::Int(IntState::known(value.wrapping_add(r))))
                    }
                    Some(Num::Double(None)) => {
                        Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE))
                    }
                    Some(Num::Double(Some(r))) => {
                        Ok(DoubleState::known(dart_num::int_to_double(value) + r))
                    }
                    None => throws_exception(),
                }
            }
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                self.double_binary(s, right, |a, b| a + b)
            }
            _ => {
                if let (InstanceState::String(_), InstanceState::String(_)) = (self, right) {
                    return Ok(InstanceState::String(self.concatenate(right)?));
                }
                assert_num_string_or_null(self)?;
                assert_num_string_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// The shape of `DoubleState.add/divide/minus/remainder/times`: an
    /// unknown operand gives an unknown double.
    fn double_binary(
        &self,
        s: &DoubleState,
        right: &InstanceState,
        op: impl FnOnce(f64, f64) -> f64,
    ) -> EvalResult<InstanceState> {
        let Some(value) = s.value else {
            return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
        };
        match num_operand(right) {
            Some(Num::Int(Some(r))) => {
                Ok(DoubleState::known(op(value, dart_num::int_to_double(r))))
            }
            Some(Num::Double(Some(r))) => Ok(DoubleState::known(op(value, r))),
            Some(Num::Int(None) | Num::Double(None)) => {
                Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE))
            }
            None => throws_exception(),
        }
    }

    /// The shape of `IntState.minus/times`: int op int is an int, int op
    /// double is a double.
    fn int_binary(
        &self,
        s: &IntState,
        right: &InstanceState,
        int_op: impl FnOnce(i64, i64) -> i64,
        double_op: impl FnOnce(f64, f64) -> f64,
    ) -> EvalResult<InstanceState> {
        assert_num_or_null(right)?;
        let Some(value) = s.value else {
            if let InstanceState::Double(_) = right {
                return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
            }
            return Ok(InstanceState::Int(IntState::UNKNOWN_VALUE));
        };
        match num_operand(right) {
            Some(Num::Int(None)) => Ok(InstanceState::Int(IntState::UNKNOWN_VALUE)),
            Some(Num::Int(Some(r))) => Ok(InstanceState::Int(IntState::known(int_op(value, r)))),
            Some(Num::Double(None)) => Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE)),
            Some(Num::Double(Some(r))) => Ok(DoubleState::known(double_op(
                dart_num::int_to_double(value),
                r,
            ))),
            None => throws_exception(),
        }
    }

    /// The shape of `IntState.bitAnd/bitOr/bitXor`.
    fn int_bitwise(
        &self,
        right: &InstanceState,
        op: impl FnOnce(i64, i64) -> i64,
    ) -> EvalResult<IntState> {
        match self {
            InstanceState::Int(s) => {
                assert_int_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(IntState::UNKNOWN_VALUE);
                };
                match right {
                    InstanceState::Int(r) => match r.value {
                        None => Ok(IntState::UNKNOWN_VALUE),
                        Some(r) => Ok(IntState::known(op(value, r))),
                    },
                    _ => throws_exception(),
                }
            }
            _ => {
                assert_int_or_null(self)?;
                assert_int_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `bitAnd`: the `&` operator on ints.
    pub fn bit_and(&self, right: &InstanceState) -> EvalResult<IntState> {
        self.int_bitwise(right, |a, b| a & b)
    }

    /// Dart `bitNot`: the `~` operator.
    pub fn bit_not(&self) -> EvalResult<IntState> {
        match self {
            InstanceState::Int(s) => Ok(match s.value {
                None => IntState::UNKNOWN_VALUE,
                Some(v) => IntState::known(!v),
            }),
            _ => {
                assert_int_or_null(self)?;
                throws_exception()
            }
        }
    }

    /// Dart `bitOr`: the `|` operator on ints.
    pub fn bit_or(&self, right: &InstanceState) -> EvalResult<IntState> {
        self.int_bitwise(right, |a, b| a | b)
    }

    /// Dart `bitXor`: the `^` operator on ints.
    pub fn bit_xor(&self, right: &InstanceState) -> EvalResult<IntState> {
        self.int_bitwise(right, |a, b| a ^ b)
    }

    /// Dart `concatenate`.
    pub fn concatenate(&self, right: &InstanceState) -> EvalResult<StringState> {
        if let InstanceState::String(s) = self {
            let Some(value) = &s.value else {
                return Ok(StringState::UNKNOWN_VALUE);
            };
            if let InstanceState::String(r) = right {
                let Some(right_value) = &r.value else {
                    return Ok(StringState::UNKNOWN_VALUE);
                };
                return Ok(StringState::new(format!("{value}{right_value}")));
            }
        }
        // `super.concatenate`.
        assert_string(right)?;
        throws_exception()
    }

    /// Dart `convertToBool`.
    pub fn convert_to_bool(&self) -> EvalResult<BoolState> {
        match self {
            InstanceState::Bool(s) => Ok(*s),
            InstanceState::Null(_) => throws_exception(),
            _ => Ok(BoolState::FALSE_STATE),
        }
    }

    /// Dart `convertToString`.
    pub fn convert_to_string(&self, ts: &dyn ConstTypeSystem) -> EvalResult<StringState> {
        Ok(match self {
            InstanceState::Bool(s) => match s.value {
                None => StringState::UNKNOWN_VALUE,
                Some(v) => StringState::new(if v { "true" } else { "false" }),
            },
            InstanceState::Double(s) => match s.value {
                None => StringState::UNKNOWN_VALUE,
                Some(v) => StringState::new(dart_num::double_to_string(v)),
            },
            InstanceState::Int(s) => match s.value {
                None => StringState::UNKNOWN_VALUE,
                Some(v) => StringState::new(v.to_string()),
            },
            InstanceState::Function(s) => {
                let ctx = ts.ctx();
                StringState {
                    value: ctx
                        .element_data(s.element.raw())
                        .and_then(|d| d.name)
                        .map(|n| Arc::from(ctx.name_str(n))),
                }
            }
            InstanceState::Generic(_)
            | InstanceState::List(_)
            | InstanceState::Map(_)
            // The behavior of `toString` is undefined.
            | InstanceState::Record(_)
            | InstanceState::Set(_) => StringState::UNKNOWN_VALUE,
            InstanceState::Null(_) => StringState::new("null"),
            InstanceState::String(s) => s.clone(),
            InstanceState::Symbol(s) => StringState {
                value: s.value.clone(),
            },
            InstanceState::Type(s) => match s.ty {
                Some(ty) => StringState::new(ts.display_string(ty)),
                None => StringState::UNKNOWN_VALUE,
            },
        })
    }

    /// Dart `divide`: the `/` operator.
    pub fn divide(&self, right: &InstanceState) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
                };
                let left = dart_num::int_to_double(value);
                match num_operand(right) {
                    Some(Num::Int(Some(r))) => {
                        Ok(DoubleState::known(left / dart_num::int_to_double(r)))
                    }
                    Some(Num::Double(Some(r))) => Ok(DoubleState::known(left / r)),
                    Some(Num::Int(None) | Num::Double(None)) => {
                        Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE))
                    }
                    None => throws_exception(),
                }
            }
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                self.double_binary(s, right, |a, b| a / b)
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `equalEqual`: every subclass forwards to `isIdentical`.
    pub fn equal_equal(&self, ts: &dyn ConstTypeSystem, right: &InstanceState) -> BoolState {
        self.is_identical(ts, right)
    }

    /// The shape of the comparison operators of `IntState` / `DoubleState`.
    fn compare(
        &self,
        right: &InstanceState,
        int_op: impl FnOnce(std::cmp::Ordering) -> bool,
        double_op: impl FnOnce(f64, f64) -> bool,
    ) -> EvalResult<BoolState> {
        match self {
            InstanceState::Int(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(BoolState::UNKNOWN_VALUE);
                };
                match num_operand(right) {
                    Some(Num::Int(Some(r))) => {
                        Ok(BoolState::from(int_op(dart_num::int_compare(value, r))))
                    }
                    Some(Num::Double(Some(r))) => Ok(BoolState::from(double_op(
                        dart_num::int_to_double(value),
                        r,
                    ))),
                    Some(Num::Int(None) | Num::Double(None)) => Ok(BoolState::UNKNOWN_VALUE),
                    None => throws_exception(),
                }
            }
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(BoolState::UNKNOWN_VALUE);
                };
                match num_operand(right) {
                    Some(Num::Int(Some(r))) => Ok(BoolState::from(double_op(
                        value,
                        dart_num::int_to_double(r),
                    ))),
                    Some(Num::Double(Some(r))) => Ok(BoolState::from(double_op(value, r))),
                    Some(Num::Int(None) | Num::Double(None)) => Ok(BoolState::UNKNOWN_VALUE),
                    None => throws_exception(),
                }
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `greaterThan`: the `>` operator.
    pub fn greater_than(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.compare(right, |o| o.is_gt(), |a, b| a > b)
    }

    /// Dart `greaterThanOrEqual`: the `>=` operator.
    pub fn greater_than_or_equal(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.compare(right, |o| o.is_ge(), |a, b| a >= b)
    }

    /// Dart `lessThan`: the `<` operator.
    pub fn less_than(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.compare(right, |o| o.is_lt(), |a, b| a < b)
    }

    /// Dart `lessThanOrEqual`: the `<=` operator.
    pub fn less_than_or_equal(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.compare(right, |o| o.is_le(), |a, b| a <= b)
    }

    /// Dart `hasPrimitiveEquality`: whether this value, inside a library
    /// with the [feature_set], has primitive equality. [object_type] is the
    /// type of the enclosing `DartObjectImpl` (Dart `GenericState._object`).
    pub fn has_primitive_equality(
        &self,
        ts: &dyn ConstTypeSystem,
        feature_set: &FeatureSet,
        object_type: TypeId,
    ) -> bool {
        match self {
            InstanceState::Bool(_)
            | InstanceState::Function(_)
            | InstanceState::Int(_)
            | InstanceState::List(_)
            | InstanceState::Map(_)
            | InstanceState::Null(_)
            | InstanceState::Set(_)
            | InstanceState::String(_)
            | InstanceState::Symbol(_)
            | InstanceState::Type(_) => true,
            // `NumState` does not override the base implementation.
            InstanceState::Double(_) => false,
            InstanceState::Record(s) => s
                .positional_fields
                .iter()
                .chain(s.named_fields.values())
                .all(|e| e.has_primitive_equality(ts, feature_set)),
            InstanceState::Generic(_) => {
                generic_has_primitive_equality(ts, feature_set, object_type)
            }
        }
    }

    /// Dart `integerDivide`: the `~/` operator.
    pub fn integer_divide(&self, right: &InstanceState) -> EvalResult<IntState> {
        match self {
            InstanceState::Int(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(IntState::UNKNOWN_VALUE);
                };
                match num_operand(right) {
                    Some(Num::Int(None)) => return Ok(IntState::UNKNOWN_VALUE),
                    Some(Num::Int(Some(r))) => {
                        return match dart_num::int_truncating_div(value, r) {
                            Some(q) => Ok(IntState::known(q)),
                            None => {
                                Err(EvaluationException::runtime(diag::const_eval_throws_idbze()))
                            }
                        };
                    }
                    Some(Num::Double(None)) => return Ok(IntState::UNKNOWN_VALUE),
                    Some(Num::Double(Some(r))) => {
                        let result = dart_num::int_to_double(value) / r;
                        if let Some(q) = dart_num::double_to_int(result) {
                            return Ok(IntState::known(q));
                        }
                    }
                    None => {}
                }
                throws_exception()
            }
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(IntState::UNKNOWN_VALUE);
                };
                let result = match num_operand(right) {
                    Some(Num::Int(None) | Num::Double(None)) => return Ok(IntState::UNKNOWN_VALUE),
                    Some(Num::Int(Some(r))) => Some(value / dart_num::int_to_double(r)),
                    Some(Num::Double(Some(r))) => Some(value / r),
                    None => None,
                };
                if let Some(q) = result.and_then(dart_num::double_to_int) {
                    return Ok(IntState::known(q));
                }
                throws_exception()
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `isIdentical`: the `identical` function on states.
    pub fn is_identical(&self, ts: &dyn ConstTypeSystem, right: &InstanceState) -> BoolState {
        match self {
            InstanceState::Bool(s) => {
                let Some(value) = s.value else {
                    return BoolState::UNKNOWN_VALUE;
                };
                if let InstanceState::Bool(r) = right {
                    return match r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(r) => BoolState::from(value == r),
                    };
                }
                BoolState::FALSE_STATE
            }
            InstanceState::Double(s) => {
                let Some(value) = s.value else {
                    return BoolState::UNKNOWN_VALUE;
                };
                if value.is_nan() {
                    // `double.nan` equality will always be `false`.
                    return BoolState::FALSE_STATE;
                }
                match right {
                    InstanceState::Double(r) => match r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        // `double.nan` equality will always be `false`.
                        Some(r) if r.is_nan() => BoolState::FALSE_STATE,
                        Some(r) => BoolState::from(dart_num::double_identical(value, r)),
                    },
                    InstanceState::Int(r) => match r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(r) => BoolState::from(dart_num::double_identical(
                            value,
                            dart_num::int_to_double(r),
                        )),
                    },
                    _ => BoolState::FALSE_STATE,
                }
            }
            InstanceState::Int(s) => {
                let Some(value) = s.value else {
                    return BoolState::UNKNOWN_VALUE;
                };
                match right {
                    InstanceState::Int(r) => match r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(r) => BoolState::from(value == r),
                    },
                    InstanceState::Double(r) => match r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        // Dart `==` on doubles.
                        Some(r) => BoolState::from(r == dart_num::int_to_double(value)),
                    },
                    _ => BoolState::FALSE_STATE,
                }
            }
            InstanceState::Function(s) => {
                let InstanceState::Function(r) = right else {
                    return BoolState::FALSE_STATE;
                };
                // `ExecutableElementImpl.baseElement` is the element itself.
                if s.element != r.element {
                    return BoolState::FALSE_STATE;
                }
                if s.via_type_alias != r.via_type_alias {
                    return BoolState::FALSE_STATE;
                }
                let (Some(type_arguments), Some(other_type_arguments)) =
                    (&s.type_arguments, &r.type_arguments)
                else {
                    return BoolState::from(
                        s.type_arguments.is_none() && r.type_arguments.is_none(),
                    );
                };
                if type_arguments.len() != other_type_arguments.len() {
                    return BoolState::FALSE_STATE;
                }
                for (&a, &b) in type_arguments.iter().zip(other_type_arguments) {
                    if !ts.runtime_types_equal(a, b) {
                        return BoolState::FALSE_STATE;
                    }
                }
                BoolState::TRUE_STATE
            }
            InstanceState::Generic(_) => BoolState::from(self.state_eq(right, ts)),
            InstanceState::List(s) => {
                if self.is_unknown() || right.is_unknown() {
                    return BoolState::UNKNOWN_VALUE;
                }
                let InstanceState::List(r) = right else {
                    return BoolState::FALSE_STATE;
                };
                BoolState::from(
                    ts.types_equal(ts.normalize(s.element_type), ts.normalize(r.element_type))
                        && self.state_eq(right, ts),
                )
            }
            InstanceState::Map(s) => {
                if self.is_unknown() || right.is_unknown() {
                    return BoolState::UNKNOWN_VALUE;
                }
                let InstanceState::Map(r) = right else {
                    return BoolState::FALSE_STATE;
                };
                BoolState::from(
                    ts.types_equal(ts.normalize(s.key_type), ts.normalize(r.key_type))
                        && ts.types_equal(ts.normalize(s.value_type), ts.normalize(r.value_type))
                        && self.state_eq(right, ts),
                )
            }
            InstanceState::Null(_) => BoolState::from(right.is_null()),
            InstanceState::Record(_) => {
                if !self.state_eq(right, ts) {
                    return BoolState::FALSE_STATE;
                }
                BoolState::UNKNOWN_VALUE
            }
            InstanceState::Set(s) => {
                if self.is_unknown() || right.is_unknown() {
                    return BoolState::UNKNOWN_VALUE;
                }
                let InstanceState::Set(r) = right else {
                    return BoolState::FALSE_STATE;
                };
                BoolState::from(
                    ts.types_equal(ts.normalize(s.element_type), ts.normalize(r.element_type))
                        && self.state_eq(right, ts),
                )
            }
            InstanceState::String(s) => {
                let Some(value) = &s.value else {
                    return BoolState::UNKNOWN_VALUE;
                };
                if let InstanceState::String(r) = right {
                    return match &r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(r) => BoolState::from(value == r),
                    };
                }
                BoolState::FALSE_STATE
            }
            InstanceState::Symbol(s) => {
                let Some(value) = &s.value else {
                    return BoolState::UNKNOWN_VALUE;
                };
                if let InstanceState::Symbol(r) = right {
                    return match &r.value {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(r) => BoolState::from(value == r),
                    };
                }
                BoolState::FALSE_STATE
            }
            InstanceState::Type(s) => {
                let Some(ty) = s.ty else {
                    return BoolState::UNKNOWN_VALUE;
                };
                if let InstanceState::Type(r) = right {
                    return match r.ty {
                        None => BoolState::UNKNOWN_VALUE,
                        Some(right_type) => BoolState::from(ts.runtime_types_equal(ty, right_type)),
                    };
                }
                BoolState::FALSE_STATE
            }
        }
    }

    /// Dart `lazyAnd`: the `&&` operator.
    pub fn lazy_and(
        &self,
        right_operand_computer: impl FnOnce() -> EvalResult<Option<InstanceState>>,
    ) -> EvalResult<BoolState> {
        match self {
            InstanceState::Bool(s) => {
                if s.value == Some(false) {
                    return Ok(BoolState::FALSE_STATE);
                }
                let right = right_operand_computer()?;
                assert_bool(right.as_ref())?;
                if s.value.is_none() {
                    return Ok(BoolState::UNKNOWN_VALUE);
                }
                right.expect("asserted").convert_to_bool()
            }
            _ => {
                // The base implementation starts with `assertBool(this)`.
                assert_bool(Some(self))?;
                unreachable!()
            }
        }
    }

    /// Dart `lazyOr`: the `||` operator.
    pub fn lazy_or(
        &self,
        right_operand_computer: impl FnOnce() -> EvalResult<Option<InstanceState>>,
    ) -> EvalResult<BoolState> {
        match self {
            InstanceState::Bool(s) => {
                if s.value == Some(true) {
                    return Ok(BoolState::TRUE_STATE);
                }
                let right = right_operand_computer()?;
                assert_bool(right.as_ref())?;
                if s.value.is_none() {
                    return Ok(BoolState::UNKNOWN_VALUE);
                }
                right.expect("asserted").convert_to_bool()
            }
            _ => {
                assert_bool(Some(self))?;
                unreachable!()
            }
        }
    }

    /// The shape of `logicalAnd/logicalOr/logicalXor` (base class only).
    fn logical(
        &self,
        right: &InstanceState,
        op: impl FnOnce(bool, bool) -> bool,
    ) -> EvalResult<BoolState> {
        assert_bool(Some(self))?;
        assert_bool(Some(right))?;
        let left_value = self.convert_to_bool()?.value;
        let right_value = right.convert_to_bool()?.value;
        match (left_value, right_value) {
            (Some(l), Some(r)) => Ok(BoolState::from(op(l, r))),
            _ => Ok(BoolState::UNKNOWN_VALUE),
        }
    }

    /// Dart `logicalAnd`: the `&` operator on bools.
    pub fn logical_and(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.logical(right, |a, b| a & b)
    }

    /// Dart `logicalOr`: the `|` operator on bools.
    pub fn logical_or(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.logical(right, |a, b| a | b)
    }

    /// Dart `logicalXor`: the `^` operator on bools.
    pub fn logical_xor(&self, right: &InstanceState) -> EvalResult<BoolState> {
        self.logical(right, |a, b| a ^ b)
    }

    /// Dart `logicalNot`: the `!` operator.
    pub fn logical_not(&self) -> EvalResult<BoolState> {
        match self {
            InstanceState::Bool(s) => Ok(match s.value {
                None => BoolState::UNKNOWN_VALUE,
                Some(v) => BoolState::from(!v),
            }),
            InstanceState::Null(_) => throws_exception(),
            _ => {
                assert_bool(Some(self))?;
                Ok(BoolState::TRUE_STATE)
            }
        }
    }

    /// Dart `logicalShiftRight`: the `>>>` operator.
    pub fn logical_shift_right(&self, right: &InstanceState) -> EvalResult<IntState> {
        match self {
            InstanceState::Int(s) => {
                assert_int_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(IntState::UNKNOWN_VALUE);
                };
                if let InstanceState::Int(r) = right {
                    match r.value {
                        None => return Ok(IntState::UNKNOWN_VALUE),
                        Some(r) if r >= 64 => return Ok(IntState::known(0)),
                        Some(r) if r >= 0 => {
                            // Dart: `(value >> rightValue) & ((1 << (64 - rightValue)) - 1)`.
                            let mask = dart_num::int_shl(1, 64 - r).wrapping_sub(1);
                            return Ok(IntState::known(dart_num::int_shr(value, r) & mask));
                        }
                        Some(_) => {}
                    }
                }
                throws_exception()
            }
            _ => {
                assert_int_or_null(self)?;
                assert_int_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `minus`: the binary `-` operator.
    pub fn minus(&self, right: &InstanceState) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => self.int_binary(s, right, i64::wrapping_sub, |a, b| a - b),
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                self.double_binary(s, right, |a, b| a - b)
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `negated`: the unary `-` operator.
    pub fn negated(&self) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => Ok(InstanceState::Int(match s.value {
                None => IntState::UNKNOWN_VALUE,
                Some(v) => IntState::known(v.wrapping_neg()),
            })),
            InstanceState::Double(s) => Ok(InstanceState::Double(match s.value {
                None => DoubleState::UNKNOWN_VALUE,
                Some(v) => DoubleState::new(Some(-v)),
            })),
            _ => {
                assert_num_or_null(self)?;
                throws_exception()
            }
        }
    }

    /// Dart `remainder`: the `%` operator.
    pub fn remainder(&self, right: &InstanceState) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => {
                assert_num_or_null(right)?;
                let Some(value) = s.value else {
                    if let InstanceState::Double(_) = right {
                        return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
                    }
                    return Ok(InstanceState::Int(IntState::UNKNOWN_VALUE));
                };
                match num_operand(right) {
                    Some(Num::Int(None)) => return Ok(InstanceState::Int(IntState::UNKNOWN_VALUE)),
                    Some(Num::Int(Some(r))) => {
                        if let Some(m) = dart_num::int_modulo(value, r) {
                            return Ok(InstanceState::Int(IntState::known(m)));
                        }
                    }
                    Some(Num::Double(None)) => {
                        return Ok(InstanceState::Double(DoubleState::UNKNOWN_VALUE));
                    }
                    Some(Num::Double(Some(r))) => {
                        return Ok(DoubleState::known(dart_num::double_modulo(
                            dart_num::int_to_double(value),
                            r,
                        )));
                    }
                    None => {}
                }
                throws_exception()
            }
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                self.double_binary(s, right, dart_num::double_modulo)
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// The shape of `IntState.shiftLeft/shiftRight`.
    fn int_shift(
        &self,
        right: &InstanceState,
        op: impl FnOnce(i64, i64) -> i64,
    ) -> EvalResult<IntState> {
        match self {
            InstanceState::Int(s) => {
                assert_int_or_null(right)?;
                let Some(value) = s.value else {
                    return Ok(IntState::UNKNOWN_VALUE);
                };
                if let InstanceState::Int(r) = right {
                    match r.value {
                        None => return Ok(IntState::UNKNOWN_VALUE),
                        Some(r) if dart_num::int_bit_length(r) > 31 => {
                            return Ok(IntState::UNKNOWN_VALUE);
                        }
                        Some(r) if r >= 0 => return Ok(IntState::known(op(value, r))),
                        Some(_) => {}
                    }
                }
                throws_exception()
            }
            _ => {
                assert_int_or_null(self)?;
                assert_int_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `shiftLeft`: the `<<` operator.
    pub fn shift_left(&self, right: &InstanceState) -> EvalResult<IntState> {
        self.int_shift(right, dart_num::int_shl)
    }

    /// Dart `shiftRight`: the `>>` operator.
    pub fn shift_right(&self, right: &InstanceState) -> EvalResult<IntState> {
        self.int_shift(right, dart_num::int_shr)
    }

    /// Dart `stringLength`: the `length` getter of a `String`.
    pub fn string_length(&self) -> EvalResult<IntState> {
        match self {
            InstanceState::String(s) => Ok(match &s.value {
                None => IntState::UNKNOWN_VALUE,
                Some(v) => IntState::known(dart_num::string_length(v)),
            }),
            _ => {
                assert_string(self)?;
                throws_exception()
            }
        }
    }

    /// Dart `times`: the `*` operator.
    pub fn times(&self, right: &InstanceState) -> EvalResult<InstanceState> {
        match self {
            InstanceState::Int(s) => self.int_binary(s, right, i64::wrapping_mul, |a, b| a * b),
            InstanceState::Double(s) => {
                assert_num_or_null(right)?;
                self.double_binary(s, right, |a, b| a * b)
            }
            _ => {
                assert_num_or_null(self)?;
                assert_num_or_null(right)?;
                throws_exception()
            }
        }
    }

    /// Dart `operator ==` of the state classes.
    pub fn state_eq(&self, other: &InstanceState, ts: &dyn ConstTypeSystem) -> bool {
        match (self, other) {
            // Dart: identical(value, other.value)
            (InstanceState::Bool(a), InstanceState::Bool(b)) => a.value == b.value,
            // Dart: identical(value, other.value): the same bits.
            (InstanceState::Double(a), InstanceState::Double(b)) => match (a.value, b.value) {
                (None, None) => true,
                (Some(a), Some(b)) => dart_num::double_identical(a, b),
                _ => false,
            },
            (InstanceState::Int(a), InstanceState::Int(b)) => a.value == b.value,
            (InstanceState::String(a), InstanceState::String(b)) => a.value == b.value,
            (InstanceState::Symbol(a), InstanceState::Symbol(b)) => a.value == b.value,
            (InstanceState::Null(_), InstanceState::Null(_)) => true,
            // Dart: type == other.type
            (InstanceState::Type(a), InstanceState::Type(b)) => match (a.ty, b.ty) {
                (None, None) => true,
                (Some(a), Some(b)) => ts.types_equal(a, b),
                _ => false,
            },
            (InstanceState::Function(a), InstanceState::Function(b)) => {
                if a.element != b.element {
                    return false;
                }
                let (Some(type_arguments), Some(other_type_arguments)) =
                    (&a.type_arguments, &b.type_arguments)
                else {
                    return a.type_arguments.is_none() && b.type_arguments.is_none();
                };
                if type_arguments.len() != other_type_arguments.len() {
                    return false;
                }
                if a.via_type_alias != b.via_type_alias {
                    return false;
                }
                type_arguments
                    .iter()
                    .zip(other_type_arguments)
                    .all(|(&x, &y)| ts.types_equal(x, y))
            }
            (InstanceState::Generic(a), InstanceState::Generic(b)) => {
                // Every field of `a` equals the field of `b`, and `b` has no
                // other fields (a missing field is Dart `null`).
                a.fields.len() == b.fields.len()
                    && a.fields.iter().all(|(name, value)| {
                        b.fields
                            .get(name)
                            .is_some_and(|other| value.dart_eq(other, ts))
                    })
            }
            (InstanceState::List(a), InstanceState::List(b)) => {
                a.elements.len() == b.elements.len()
                    && a.elements
                        .iter()
                        .zip(&b.elements)
                        .all(|(x, y)| x.dart_eq(y, ts))
            }
            (InstanceState::Set(a), InstanceState::Set(b)) => {
                a.elements.len() == b.elements.len()
                    && a.elements
                        .iter()
                        .zip(b.elements.iter())
                        .all(|(x, y)| x.dart_eq(y, ts))
            }
            (InstanceState::Map(a), InstanceState::Map(b)) => {
                a.entries.len() == b.entries.len()
                    && a.entries.iter().all(|(key, value)| {
                        b.entries
                            .get(ts, key)
                            .is_some_and(|other| value.dart_eq(other, ts))
                    })
            }
            (InstanceState::Record(a), InstanceState::Record(b)) => {
                a.positional_fields.len() == b.positional_fields.len()
                    && a.named_fields.len() == b.named_fields.len()
                    && a.positional_fields
                        .iter()
                        .zip(&b.positional_fields)
                        .all(|(x, y)| x.dart_eq(y, ts))
                    && a.named_fields.iter().all(|(name, value)| {
                        b.named_fields
                            .get(name)
                            .is_some_and(|other| value.dart_eq(other, ts))
                    })
            }
            _ => false,
        }
    }

    /// A hash that is consistent with [`InstanceState::state_eq`] for every
    /// type system (types are not hashed). Never printed.
    pub fn state_hash(&self) -> u64 {
        match self {
            InstanceState::Bool(s) => hash_one((1u8, s.value)),
            InstanceState::Double(s) => hash_one((2u8, s.value.map(f64::to_bits))),
            InstanceState::Int(s) => hash_one((3u8, s.value)),
            InstanceState::String(s) => hash_one((4u8, &s.value)),
            InstanceState::Symbol(s) => hash_one((5u8, &s.value)),
            InstanceState::Null(_) => hash_one(6u8),
            InstanceState::Type(_) => hash_one(7u8),
            InstanceState::Function(s) => hash_one((8u8, s.element)),
            InstanceState::Generic(s) => s.fields.iter().fold(hash_one(9u8), |h, (name, value)| {
                // Order independent, like the equality.
                h.wrapping_add(hash_one((name, value.dart_hash())))
            }),
            InstanceState::List(s) => s
                .elements
                .iter()
                .fold(hash_one(10u8), |h, e| combine_ordered(h, e.dart_hash())),
            InstanceState::Set(s) => s
                .elements
                .iter()
                .fold(hash_one(11u8), |h, e| combine_ordered(h, e.dart_hash())),
            InstanceState::Map(s) => s.entries.iter().fold(hash_one(12u8), |h, (k, v)| {
                h.wrapping_add(hash_one((k.dart_hash(), v.dart_hash())))
            }),
            InstanceState::Record(s) => {
                let h = s
                    .positional_fields
                    .iter()
                    .fold(hash_one(13u8), |h, e| combine_ordered(h, e.dart_hash()));
                s.named_fields.iter().fold(h, |h, (name, value)| {
                    h.wrapping_add(hash_one((name, value.dart_hash())))
                })
            }
        }
    }

    /// Dart `toString()` of the state classes.
    pub fn display(&self, ts: &dyn ConstTypeSystem) -> String {
        const UNKNOWN: &str = "-unknown-";
        match self {
            InstanceState::Bool(s) => match s.value {
                None => UNKNOWN.to_string(),
                Some(v) => v.to_string(),
            },
            InstanceState::Double(s) => match s.value {
                None => UNKNOWN.to_string(),
                Some(v) => dart_num::double_to_string(v),
            },
            InstanceState::Int(s) => match s.value {
                None => UNKNOWN.to_string(),
                Some(v) => v.to_string(),
            },
            InstanceState::String(s) => match &s.value {
                None => UNKNOWN.to_string(),
                Some(v) => format!("'{v}'"),
            },
            InstanceState::Symbol(s) => match &s.value {
                None => UNKNOWN.to_string(),
                Some(v) => format!("#{v}"),
            },
            InstanceState::Null(_) => "null".to_string(),
            InstanceState::Type(s) => match s.ty {
                None => UNKNOWN.to_string(),
                Some(ty) => ts.display_string(ty),
            },
            InstanceState::Function(s) => {
                let ctx = ts.ctx();
                match ctx.element_data(s.element.raw()).and_then(|d| d.name) {
                    Some(name) => ctx.name_str(name).to_string(),
                    None => "<unnamed>".to_string(),
                }
            }
            InstanceState::Generic(s) => {
                let mut field_names: Vec<&Arc<str>> = s.fields.keys().collect();
                field_names.sort_by(|a, b| dart_num::string_compare(a, b));
                field_names
                    .iter()
                    .map(|name| format!("{name} = {}", s.fields[*name].display(ts)))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
            InstanceState::List(s) => {
                let parts: Vec<String> = s.elements.iter().map(|e| e.display(ts)).collect();
                format!("[{}]", parts.join(", "))
            }
            InstanceState::Set(s) => {
                let parts: Vec<String> = s.elements.iter().map(|e| e.display(ts)).collect();
                format!("{{{}}}", parts.join(", "))
            }
            InstanceState::Map(s) => {
                let parts: Vec<String> = s
                    .entries
                    .iter()
                    .map(|(k, v)| format!("{} = {}", k.display(ts), v.display(ts)))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            InstanceState::Record(s) => {
                let mut buffer = String::from("(");
                let mut first = true;
                for value in &s.positional_fields {
                    if first {
                        first = false;
                    } else {
                        buffer.push_str(", ");
                    }
                    buffer.push_str(&value.display(ts));
                }
                let mut entries: Vec<(&Arc<str>, &DartObjectImpl)> =
                    s.named_fields.iter().collect();
                if !entries.is_empty() {
                    entries.sort_by(|a, b| dart_num::string_compare(a.0, b.0));
                    if !first {
                        buffer.push_str(", ");
                        first = true;
                    }
                    buffer.push('{');
                    for (name, value) in entries {
                        if first {
                            first = false;
                        } else {
                            buffer.push_str(", ");
                        }
                        buffer.push_str(name);
                        buffer.push_str(": ");
                        buffer.push_str(&value.display(ts));
                    }
                    buffer.push('}');
                }
                buffer.push(')');
                buffer
            }
        }
    }
}

/// Dart `GenericState.hasPrimitiveEquality`.
fn generic_has_primitive_equality(
    ts: &dyn ConstTypeSystem,
    feature_set: &FeatureSet,
    ty: TypeId,
) -> bool {
    let ctx = ts.ctx();
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    let is_from_dart_core_object = |member: Option<ElemRef>| -> bool {
        let Some(member) = member else {
            return false;
        };
        let base = match member {
            ElemRef::Base(e) => e,
            ElemRef::Member(m) => ctx.member(m).base,
        };
        let Some(enclosing) = ctx.element_data(base).and_then(|d| d.enclosing) else {
            return false;
        };
        enclosing.is::<ClassElement>() && is_dart_core_element_named(&ctx, enclosing, "Object")
    };
    let Some(library) = ctx.element_data(element.raw()).and_then(|d| d.library) else {
        return false;
    };
    let eq_eq = ts.look_up_concrete_method(ty, "==", library);
    if !is_from_dart_core_object(eq_eq) {
        return false;
    }
    if feature_set.is_enabled("patterns") {
        let hash = ts.look_up_concrete_getter(ty, "hashCode", library);
        if !is_from_dart_core_object(hash) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Set and Map of DartObjectImpl
// ---------------------------------------------------------------------------

/// Dart `Set<DartObjectImpl>` (a `LinkedHashSet`): insertion order, and
/// membership by [`DartObjectImpl::dart_eq`].
#[derive(Clone, Debug, Default)]
pub struct DartObjectSet {
    elements: Vec<DartObjectImpl>,
    index: LookupMap<u64, Vec<u32>>,
}

impl DartObjectSet {
    pub fn new() -> DartObjectSet {
        DartObjectSet::default()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// The elements in insertion order.
    pub fn iter(&self) -> std::slice::Iter<'_, DartObjectImpl> {
        self.elements.iter()
    }

    /// The element that equals [value].
    pub fn lookup(
        &self,
        ts: &dyn ConstTypeSystem,
        value: &DartObjectImpl,
    ) -> Option<&DartObjectImpl> {
        let bucket = self.index.get(&value.dart_hash())?;
        bucket
            .iter()
            .map(|&i| &self.elements[i as usize])
            .find(|e| e.dart_eq(value, ts))
    }

    /// Dart `contains`.
    pub fn contains(&self, ts: &dyn ConstTypeSystem, value: &DartObjectImpl) -> bool {
        self.lookup(ts, value).is_some()
    }

    /// Dart `add`: `true` when [value] was not in the set (an equal element
    /// keeps its place and identity).
    pub fn add(&mut self, ts: &dyn ConstTypeSystem, value: DartObjectImpl) -> bool {
        if self.contains(ts, &value) {
            return false;
        }
        let hash = value.dart_hash();
        let i = self.elements.len() as u32;
        self.elements.push(value);
        self.index.get_or_insert_with(hash, Vec::new).push(i);
        true
    }
}

impl<'a> IntoIterator for &'a DartObjectSet {
    type Item = &'a DartObjectImpl;
    type IntoIter = std::slice::Iter<'a, DartObjectImpl>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

/// Dart `Map<DartObjectImpl, DartObjectImpl>` (a `LinkedHashMap`):
/// insertion order, and keys by [`DartObjectImpl::dart_eq`].
#[derive(Clone, Debug, Default)]
pub struct DartObjectMap {
    entries: Vec<(DartObjectImpl, DartObjectImpl)>,
    index: LookupMap<u64, Vec<u32>>,
}

impl DartObjectMap {
    pub fn new() -> DartObjectMap {
        DartObjectMap::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&DartObjectImpl, &DartObjectImpl)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// The keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &DartObjectImpl> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// The values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &DartObjectImpl> {
        self.entries.iter().map(|(_, v)| v)
    }

    fn position(&self, ts: &dyn ConstTypeSystem, key: &DartObjectImpl) -> Option<usize> {
        let bucket = self.index.get(&key.dart_hash())?;
        bucket
            .iter()
            .map(|&i| i as usize)
            .find(|&i| self.entries[i].0.dart_eq(key, ts))
    }

    /// Dart `map[key]`.
    pub fn get(&self, ts: &dyn ConstTypeSystem, key: &DartObjectImpl) -> Option<&DartObjectImpl> {
        self.position(ts, key).map(|i| &self.entries[i].1)
    }

    /// Dart `containsKey`.
    pub fn contains_key(&self, ts: &dyn ConstTypeSystem, key: &DartObjectImpl) -> bool {
        self.position(ts, key).is_some()
    }

    /// Dart `map[key] = value`: an existing equal key keeps its place and
    /// identity, its value is replaced.
    pub fn insert(&mut self, ts: &dyn ConstTypeSystem, key: DartObjectImpl, value: DartObjectImpl) {
        if let Some(i) = self.position(ts, &key) {
            self.entries[i].1 = value;
            return;
        }
        let hash = key.dart_hash();
        let i = self.entries.len() as u32;
        self.entries.push((key, value));
        self.index.get_or_insert_with(hash, Vec::new).push(i);
    }
}
