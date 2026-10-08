// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis_operations.dart

//! Callback API used by flow analysis to query the client's variables and
//! types.

use std::fmt::Debug;
use std::hash::Hash;

use crate::shared_type::{SharedTypeOperations, SharedTypeView};

/// Callback API used by flow analysis to query and manipulate the client's
/// representation of variables and types.
///
/// Dart: `FlowAnalysisOperations<Variable extends Object> implements
/// FlowAnalysisTypeOperations`.
///
/// Object safe once the associated types are fixed.
pub trait FlowAnalysisOperations: FlowAnalysisTypeOperations {
    /// The client's representation of a variable (Dart type parameter
    /// `Variable`). For the analyzer this is the id of a local variable or
    /// formal parameter element.
    type Variable: Copy + Eq + Hash + Debug;

    /// The client's representation of a property (Dart `Object property` /
    /// `Object? propertyMember`): the data structure passed as
    /// `propertyMember` to [`FlowAnalysis::promoted_property_type`],
    /// [`FlowAnalysis::property_get`] and
    /// [`FlowAnalysis::push_property_subpattern`]. For the analyzer this is
    /// an element reference.
    ///
    /// [`FlowAnalysis::promoted_property_type`]: crate::flow_analysis::FlowAnalysis::promoted_property_type
    /// [`FlowAnalysis::property_get`]: crate::flow_analysis::FlowAnalysis::property_get
    /// [`FlowAnalysis::push_property_subpattern`]: crate::flow_analysis::FlowAnalysis::push_property_subpattern
    type PropertyMember: Clone + Debug;

    /// Whether the given `variable` was declared with the `final` modifier.
    fn is_final(&self, variable: Self::Variable) -> bool;

    /// Determines whether the given property can be promoted.
    ///
    /// This method will not be called if field promotion is disabled for the
    /// current library.
    fn is_property_promotable(&self, property: &Self::PropertyMember) -> bool;

    /// Whether `name` is a private name (Dart `name.startsWith('_')`; used by
    /// `whyNotPromoted` for property names).
    ///
    /// Not in Dart: added because [`SharedTypeOperations::Name`] is opaque.
    ///
    /// [`SharedTypeOperations::Name`]: crate::shared_type::SharedTypeOperations::Name
    fn is_private_name(&self, name: Self::Name) -> bool;

    /// Returns the static type of the given `variable`.
    fn variable_type(&self, variable: Self::Variable) -> SharedTypeView<Self::Type>;

    /// Returns additional information about why a given property couldn't be
    /// promoted.
    ///
    /// Only called if a closure returned by `FlowAnalysis.whyNotPromoted` is
    /// invoked, and the expression being queried is a reference to a private
    /// property that wasn't promoted.
    ///
    /// The client should return `None` if `property` was not promotable due
    /// to a conflict with a field, getter, or noSuchMethod forwarder
    /// elsewhere in the library; flow analysis then yields a
    /// `PropertyNotPromotedForNonInherentReason`. If field promotion is
    /// disabled and the property *would* have been promotable with it
    /// enabled, the client should return `None`; otherwise it should behave
    /// as if field promotion were enabled.
    fn why_property_is_not_promotable(
        &self,
        property: &Self::PropertyMember,
    ) -> Option<PropertyNonPromotabilityReason>;
}

/// Callback API used by flow analysis to query and manipulate the client's
/// representation of types.
///
/// Object safe once the associated types are fixed.
pub trait FlowAnalysisTypeOperations: SharedTypeOperations {
    /// Returns the client's representation of the type `bool`.
    fn bool_type(&self) -> SharedTypeView<Self::Type>;

    /// Classifies the given type into one of the three categories defined by
    /// the [`TypeClassification`] enum.
    fn classify_type(&self, ty: SharedTypeView<Self::Type>) -> TypeClassification;

    /// If `ty` is an extension type, returns the ultimate representation
    /// type. Otherwise returns `ty` as is.
    fn extension_type_erasure(&self, ty: SharedTypeView<Self::Type>) -> SharedTypeView<Self::Type>;

    /// Returns the "remainder" of `from` when `what` has been removed from
    /// consideration by an instance check.
    fn factor(
        &self,
        from: SharedTypeView<Self::Type>,
        what: SharedTypeView<Self::Type>,
    ) -> SharedTypeView<Self::Type>;

    /// Determines whether the given `ty` is a bottom type.
    ///
    /// A type is a bottom type if it:
    /// (a) is the `Never` type itself.
    /// (b) is a type variable that extends `Never`, OR
    /// (c) is a type variable that has been promoted to `Never`
    fn is_bottom_type(&self, ty: SharedTypeView<Self::Type>) -> bool;

    /// Return `true` if the `left_type` is a subtype of the `right_type`.
    ///
    /// A client that implements
    /// [`TypeAnalyzerOperations`](crate::type_analyzer_operations::TypeAnalyzerOperations)
    /// implements this as `TypeAnalyzerOperationsMixin.isSubtypeOf` does:
    /// `self.is_subtype_of_internal(left.unwrap_type_view(),
    /// right.unwrap_type_view())`. (A Rust subtrait can't provide a method of
    /// its supertrait.)
    fn is_subtype_of(
        &self,
        left_type: SharedTypeView<Self::Type>,
        right_type: SharedTypeView<Self::Type>,
    ) -> bool;

    /// Returns `true` if `ty` is a reference to a type parameter.
    fn is_type_parameter_type(&self, ty: SharedTypeView<Self::Type>) -> bool;

    /// Returns `true` if `ty` represents the invalid type, i.e. the type of
    /// an invalid expression.
    fn is_invalid_type(&self, ty: SharedTypeView<Self::Type>) -> bool;

    /// Computes the nullable form of `ty`, in other words the least upper
    /// bound of `ty` and `Null`.
    ///
    /// A client that implements
    /// [`TypeAnalyzerOperations`](crate::type_analyzer_operations::TypeAnalyzerOperations)
    /// implements this as `TypeAnalyzerOperationsMixin.makeNullable` does:
    /// `SharedTypeView::new(self.make_nullable_internal(ty.unwrap_type_view()))`.
    fn make_nullable(&self, ty: SharedTypeView<Self::Type>) -> SharedTypeView<Self::Type>;

    /// Returns the non-null promoted version of `ty`.
    ///
    /// Note that some types don't have a non-nullable version (e.g.
    /// `FutureOr<int?>`), so `ty` may be returned even if it is nullable.
    fn promote_to_non_null(&self, ty: SharedTypeView<Self::Type>) -> SharedTypeView<Self::Type>;

    /// Tries to promote to the first type from the second type, and returns
    /// the promoted type if it succeeds, otherwise `None`.
    fn try_promote_to_type(
        &self,
        to: SharedTypeView<Self::Type>,
        from: SharedTypeView<Self::Type>,
    ) -> Option<SharedTypeView<Self::Type>>;
}

/// Possible reasons why a property may not be promotable.
///
/// This enum captures the possible non-promotability reasons that are
/// inherent to the property declaration itself. A property may also be
/// non-promotable because field promotion is disabled, or due to a conflict
/// with another declaration; the code that handles those two reasons doesn't
/// use this enum.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PropertyNonPromotabilityReason {
    /// The property is not promotable because it's not a field (it's either a
    /// getter or a tear-off of a method).
    IsNotField,

    /// The property is not promotable because its name is public.
    IsNotPrivate,

    /// The property is not promotable because it's an external field.
    IsExternal,

    /// The property is not promotable because it's a non-final field.
    IsNotFinal,
}

/// The different classifications of types that can be returned by
/// [`FlowAnalysisTypeOperations::classify_type`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeClassification {
    /// The type is `Null` or an equivalent type (e.g. `Never?`)
    NullOrEquivalent,

    /// The type is a potentially nullable type, but not equivalent to `Null`
    /// (e.g. `int?`, or a type variable whose bound is potentially nullable)
    PotentiallyNullable,

    /// The type is a non-nullable type.
    NonNullable,
}
