// Dart source: pkg/_fe_analyzer_shared/lib/src/types/shared_type.dart

//! `Variance`, the type views and the `Shared*` type interfaces.
//!
//! # Type views
//!
//! Dart wraps the client's type objects (`SharedType`) in zero-cost extension
//! types: [`SharedTypeView`] (a type), [`SharedTypeSchemaView`] (a type
//! schema, can contain `_`) and [`SharedTypeParameterView`]. The views keep
//! types and schemas apart at compile time. Here they are generic
//! `#[repr(transparent)]` newtypes over the client's type id
//! ([`SharedTypeOperations::Type`]). `new` wraps, `unwrap_type_view` /
//! `unwrap_type_schema_view` unwrap (the Dart names).
//!
//! # `Shared*` interfaces
//!
//! In Dart the client's type objects implement `SharedType`,
//! `SharedFunctionType`, `SharedRecordType`, ... and the shared code calls
//! their getters and tests them with `is`. In dartr a type is an id (for the
//! analyzer: `TypeId`), and the data is in an interner. So the getters become
//! methods of [`SharedTypeOperations`] that take the type, and the `is` tests
//! become [`SharedTypeOperations::shared_type_kind`].

use std::fmt::Debug;
use std::hash::Hash;

/// The variance of a type parameter `X` in a type `T`.
///
/// The discriminants are the Dart `index` values; [`Variance::meet`] depends
/// on them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Variance {
    /// Used when `X` does not occur free in `T`.
    Unrelated = 0,

    /// Used when `X` occurs free in `T`, and `U <: V` implies
    /// `[U/X]T <: [V/X]T`.
    Covariant = 1,

    /// Used when `X` occurs free in `T`, and `U <: V` implies
    /// `[V/X]T <: [U/X]T`.
    Contravariant = 2,

    /// Used when there exists a pair `U` and `V` such that `U <: V`, but
    /// `[U/X]T` and `[V/X]T` are incomparable.
    Invariant = 3,
}

impl Variance {
    /// `Variance.values`, in Dart declaration order.
    pub const VALUES: [Variance; 4] = [
        Variance::Unrelated,
        Variance::Covariant,
        Variance::Contravariant,
        Variance::Invariant,
    ];

    /// The keyword of this variance (`''`, `out`, `in`, `inout`).
    pub fn keyword(self) -> &'static str {
        match self {
            Variance::Unrelated => "",
            Variance::Covariant => "out",
            Variance::Contravariant => "in",
            Variance::Invariant => "inout",
        }
    }

    /// Return the variance with the given `encoding` (`Variance.fromEncoding`).
    ///
    /// Panics if `encoding` is not in `0..4` (Dart throws a `RangeError`).
    pub fn from_encoding(encoding: usize) -> Variance {
        Variance::VALUES[encoding]
    }

    /// Return the variance associated with the string representation of
    /// variance (`Variance.fromKeywordString`).
    ///
    /// Returns `None` where Dart throws an `ArgumentError`. Note: Dart maps
    /// `"unrelated"` to [`Variance::Unrelated`] (its `assert` on the keyword
    /// fails only in checked mode); this port does the same.
    pub fn from_keyword_string(keyword_string: &str) -> Option<Variance> {
        match keyword_string {
            "in" => Some(Variance::Contravariant),
            "inout" => Some(Variance::Invariant),
            "out" => Some(Variance::Covariant),
            "unrelated" => Some(Variance::Unrelated),
            _ => None,
        }
    }

    /// The Dart `index` of this value.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Return `true` if this represents the case when `X` occurs free in `T`,
    /// and `U <: V` implies `[V/X]T <: [U/X]T`.
    pub fn is_contravariant(self) -> bool {
        self == Variance::Contravariant
    }

    /// Return `true` if this represents the case when `X` occurs free in `T`,
    /// and `U <: V` implies `[U/X]T <: [V/X]T`.
    pub fn is_covariant(self) -> bool {
        self == Variance::Covariant
    }

    /// Return `true` if this represents the case when there exists a pair `U`
    /// and `V` such that `U <: V`, but `[U/X]T` and `[V/X]T` are
    /// incomparable.
    pub fn is_invariant(self) -> bool {
        self == Variance::Invariant
    }

    /// Return `true` if this represents the case when `X` does not occur free
    /// in `T`.
    pub fn is_unrelated(self) -> bool {
        self == Variance::Unrelated
    }

    /// Combines variances of `X` in `T` and `Y` in `S` into variance of `X` in
    /// `[Y/T]S`.
    ///
    /// Examples: variance of `X` in `Function(X)` is contravariant, variance
    /// of `Y` in `List<Y>` is covariant, so variance of `X` in
    /// `List<Function(X)>` is contravariant. Variance of `X` in `Function(X)`
    /// and of `Y` in `Function(Y)` are contravariant, so variance of `X` in
    /// `Function(Function(X))` is covariant. Unrelated wins over everything,
    /// then invariant.
    pub fn combine(self, other: Variance) -> Variance {
        if self.is_unrelated() || other.is_unrelated() {
            return Variance::Unrelated;
        }
        if self.is_invariant() || other.is_invariant() {
            return Variance::Invariant;
        }
        if self == other {
            Variance::Covariant
        } else {
            Variance::Contravariant
        }
    }

    /// Returns true if this variance is greater than (above) or equal to the
    /// `other` variance in the partial order induced by the variance lattice.
    ///
    /// ```text
    ///       unrelated
    /// covariant   contravariant
    ///       invariant
    /// ```
    pub fn greater_than_or_equal(self, other: Variance) -> bool {
        match self {
            Variance::Unrelated => true,
            Variance::Covariant => other.is_covariant() || other.is_invariant(),
            Variance::Contravariant => other.is_contravariant() || other.is_invariant(),
            Variance::Invariant => other.is_invariant(),
        }
    }

    /// Variance values form a lattice where unrelated is the top, invariant is
    /// the bottom, and covariant and contravariant are incomparable. `meet`
    /// calculates the meet of two elements of such lattice. It can be used,
    /// for example, to calculate the variance of a typedef type parameter if
    /// it's encountered on the RHS of the typedef multiple times.
    ///
    /// ```text
    ///       unrelated
    /// covariant   contravariant
    ///       invariant
    /// ```
    pub fn meet(self, other: Variance) -> Variance {
        Variance::from_encoding(self.index() | other.index())
    }
}

/// `SharedTypeView`: a type (never contains the unknown type schema `_`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct SharedTypeView<T>(T);

impl<T> SharedTypeView<T> {
    /// Wraps a type structure (`new SharedTypeView(type)`, or
    /// `type.wrapSharedTypeView()`).
    pub const fn new(type_structure: T) -> Self {
        SharedTypeView(type_structure)
    }

    /// `unwrapTypeView()`.
    pub fn unwrap_type_view(self) -> T {
        self.0
    }

    /// The wrapped type structure, by reference.
    pub fn type_structure(&self) -> &T {
        &self.0
    }
}

/// `SharedTypeSchemaView`: a type schema (may contain `_`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct SharedTypeSchemaView<T>(T);

impl<T> SharedTypeSchemaView<T> {
    /// Wraps a type structure (`new SharedTypeSchemaView(type)`, or
    /// `type.wrapSharedTypeSchemaView()`).
    pub const fn new(type_structure: T) -> Self {
        SharedTypeSchemaView(type_structure)
    }

    /// `unwrapTypeSchemaView()`.
    pub fn unwrap_type_schema_view(self) -> T {
        self.0
    }

    /// The wrapped type structure, by reference.
    pub fn type_structure(&self) -> &T {
        &self.0
    }
}

/// `SharedTypeParameterView`: a type parameter.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct SharedTypeParameterView<P>(P);

impl<P> SharedTypeParameterView<P> {
    /// Wraps a type parameter structure.
    pub const fn new(type_parameter: P) -> Self {
        SharedTypeParameterView(type_parameter)
    }

    /// `unwrapTypeParameterViewAsTypeParameterStructure()`.
    pub fn unwrap_type_parameter_view_as_type_parameter_structure(self) -> P {
        self.0
    }
}

/// `SharedNamedFunctionParameter`: a named parameter of a function type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SharedNamedFunctionParameter<N, T> {
    /// Whether this named parameter is required.
    pub is_required: bool,
    /// The name of the parameter.
    pub name_shared: N,
    /// The type of the parameter.
    pub type_shared: T,
}

/// `SharedNamedType`: a name/type pair (a named record field).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SharedNamedType<N, T> {
    /// The name of the field.
    pub name_shared: N,
    /// The type of the field.
    pub type_shared: T,
}

/// Which `Shared*` interface a type structure implements.
///
/// Replaces the Dart `is SharedDynamicType`, `is SharedRecordType`, ...
/// tests. Types that implement only `SharedType` (interface types, `Never`,
/// type parameter types, `FutureOr`, ...) are [`SharedTypeKind::Other`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SharedTypeKind {
    /// `SharedDynamicType`: the type `dynamic`.
    Dynamic,
    /// `SharedFunctionType`.
    Function,
    /// `SharedInvalidType`: a type resulting from a compile-time error.
    Invalid,
    /// `SharedNullType`: the type `Null`.
    Null,
    /// `SharedRecordType`.
    Record,
    /// `SharedUnknownType`: the unknown type schema `_` (only in schemas).
    Unknown,
    /// `SharedVoidType`: the type `void`.
    Void,
    /// `SharedInvocationStructuralContextSchema`: `(...) -> returnType`.
    InvocationStructuralContextSchema,
    /// `SharedLookupStructuralContextSchema`: `{lookupName: lookupType}`.
    LookupStructuralContextSchema,
    /// Any other type.
    Other,
}

/// The `Shared*` interfaces of `shared_type.dart` as queries on the client's
/// type ids.
///
/// This is the root of the operations traits:
/// [`FlowAnalysisTypeOperations`](crate::flow_analysis_operations::FlowAnalysisTypeOperations)
/// extends it.
///
/// The getters of `SharedFunctionType` may only be called for a type whose
/// [`shared_type_kind`](Self::shared_type_kind) is
/// [`SharedTypeKind::Function`]; the same holds for the record, type
/// parameter and structural context schema getters. Implementations may
/// panic otherwise (Dart would fail the cast).
///
/// Object safe (all methods take `&self` and no generic parameters), but
/// the shared algorithms use it as a generic bound.
pub trait SharedTypeOperations {
    /// The client's type structure (`SharedType`). For the analyzer this is
    /// `TypeId`. Types and type schemas use the same structure; the
    /// distinction is made with [`SharedTypeView`] and
    /// [`SharedTypeSchemaView`].
    type Type: Copy + Eq + Hash + Debug;

    /// The client's type parameter (`SharedTypeParameter`). For the analyzer
    /// this is the id of a `TypeParameterElement`.
    type TypeParameter: Copy + Eq + Hash + Debug;

    /// The client's name of a member, record field or named parameter (Dart
    /// `String`). For the analyzer this is the interned `Name`.
    type Name: Copy + Eq + Hash + Debug;

    /// Compares two names as Dart `String.compareTo` does (lexicographic
    /// order of the UTF-16 code units).
    ///
    /// Dart compares the `String` names directly; here a name is an id, so
    /// the client compares the strings behind the ids. The shared constraint
    /// generator uses it to walk the sorted named parameters of two function
    /// types in tandem; it must agree with the order of
    /// [`sorted_named_parameters_shared`](Self::sorted_named_parameters_shared).
    fn compare_names(&self, name1: Self::Name, name2: Self::Name) -> std::cmp::Ordering;

    /// Which `Shared*` interface `ty` implements.
    fn shared_type_kind(&self, ty: Self::Type) -> SharedTypeKind;

    // ---------------------------------------------------------------- SharedType

    /// `SharedType.isQuestionType`: whether this type ends in a `?` suffix.
    ///
    /// Note that some types are nullable even though they do not end in a `?`
    /// suffix (for example, `Null`, `dynamic`, and `FutureOr<int?>`). These
    /// types all respond to this query with `false`.
    fn is_question_type(&self, ty: Self::Type) -> bool;

    /// `SharedType.asQuestionType`: returns a modified version of `ty`, with
    /// the nullability suffix changed to `is_question_type`.
    ///
    /// For types that don't accept a nullability suffix (`dynamic`,
    /// InvalidType, `Null`, `_`, and `void`), the type is returned unchanged.
    fn as_question_type(&self, ty: Self::Type, is_question_type: bool) -> Self::Type;

    /// `SharedType.getDisplayString`: the presentation of `ty` as it should
    /// appear when presented to users in contexts such as error messages.
    fn get_display_string(&self, ty: Self::Type) -> String;

    /// `SharedType.isStructurallyEqualTo`.
    fn is_structurally_equal_to(&self, ty: Self::Type, other: Self::Type) -> bool;

    // -------------------------------------------------------- SharedFunctionType

    /// `SharedFunctionType.positionalParameterTypesShared`: all the positional
    /// parameter types, starting with the required ones, and followed by the
    /// optional ones.
    fn positional_parameter_types_shared(&self, function_type: Self::Type) -> Vec<Self::Type>;

    /// `SharedFunctionType.requiredPositionalParameterCount`.
    fn required_positional_parameter_count(&self, function_type: Self::Type) -> usize;

    /// `SharedFunctionType.returnTypeShared`.
    fn return_type_shared(&self, function_type: Self::Type) -> Self::Type;

    /// `SharedFunctionType.sortedNamedParametersShared`: all the named
    /// parameters, sorted by name.
    fn sorted_named_parameters_shared(
        &self,
        function_type: Self::Type,
    ) -> Vec<SharedNamedFunctionParameter<Self::Name, Self::Type>>;

    /// `SharedFunctionType.typeParametersShared`.
    fn type_parameters_shared(&self, function_type: Self::Type) -> Vec<Self::TypeParameter>;

    // ---------------------------------------------------------- SharedRecordType

    /// `SharedRecordType.positionalTypesShared`.
    fn positional_types_shared(&self, record_type: Self::Type) -> Vec<Self::Type>;

    /// `SharedRecordType.sortedNamedTypesShared`: all the named fields,
    /// sorted by name.
    fn sorted_named_types_shared(
        &self,
        record_type: Self::Type,
    ) -> Vec<SharedNamedType<Self::Name, Self::Type>>;

    // ------------------------------------------------------- SharedTypeParameter

    /// `SharedTypeParameter.boundShared`.
    fn bound_shared(&self, type_parameter: Self::TypeParameter) -> Option<Self::Type>;

    /// `SharedTypeParameter.displayName`: the name, for display to the user.
    fn display_name(&self, type_parameter: Self::TypeParameter) -> String;

    /// `SharedTypeParameter.variance`.
    fn variance(&self, type_parameter: Self::TypeParameter) -> Variance;

    /// `SharedTypeParameter.isLegacyCovariant`: true if the type parameter
    /// doesn't use declared variance.
    fn is_legacy_covariant(&self, type_parameter: Self::TypeParameter) -> bool;

    // ---------------------------------------------- structural context schemas

    /// `SharedInvocationStructuralContextSchema.returnType`.
    fn invocation_structural_context_schema_return_type(&self, schema: Self::Type) -> Self::Type;

    /// `SharedLookupStructuralContextSchema.lookupName`.
    fn lookup_structural_context_schema_lookup_name(&self, schema: Self::Type) -> Self::Name;

    /// `SharedLookupStructuralContextSchema.lookupType`.
    fn lookup_structural_context_schema_lookup_type(&self, schema: Self::Type) -> Self::Type;
}

/// Shorthand: the [`SharedTypeView`] of the type of the operations `O`.
pub type TypeView<O> = SharedTypeView<<O as SharedTypeOperations>::Type>;

/// Shorthand: the [`SharedTypeSchemaView`] of the type of the operations `O`.
pub type SchemaView<O> = SharedTypeSchemaView<<O as SharedTypeOperations>::Type>;

/// Shorthand: the type structure of the operations `O`.
pub type TypeOf<O> = <O as SharedTypeOperations>::Type;

/// Shorthand: the type parameter of the operations `O`.
pub type TypeParameterOf<O> = <O as SharedTypeOperations>::TypeParameter;

/// Shorthand: the name of the operations `O`.
pub type NameOf<O> = <O as SharedTypeOperations>::Name;
