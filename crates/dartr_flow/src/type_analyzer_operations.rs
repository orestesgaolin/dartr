// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer_operations.dart

//! Callback API used by the shared type analyzer to query and manipulate the
//! client's representation of variables and types.
//!
//! # Mixins
//!
//! Dart has an interface `TypeAnalyzerOperations` and a mixin
//! `TypeAnalyzerOperationsMixin` that implements most of the interface in
//! terms of `...Internal` methods (for example `futureType` and
//! `futureTypeSchema` in terms of `futureTypeInternal`). Here both are one
//! trait, [`TypeAnalyzerOperations`]: the abstract members are required
//! methods, the mixin members are provided methods. A client can still
//! override a provided method, as a Dart class can override a mixin member.
//!
//! Two mixin members implement methods of the supertrait
//! [`FlowAnalysisTypeOperations`](crate::flow_analysis_operations::FlowAnalysisTypeOperations) (`isSubtypeOf`, `makeNullable`). A Rust
//! subtrait can't provide them, so the client implements them (one line
//! each, see their docs).
//!
//! `TypeConstraintGenerator` + `TypeConstraintGeneratorMixin` are merged the
//! same way into [`TypeConstraintGenerator`].
//!
//! Provided methods whose Dart bodies are more than a wrapper are
//! `todo!("<Dart name>")`; unit A13 ports them.

use indexmap::IndexMap;
use std::fmt::Debug;
use std::hash::Hash;

use crate::flow_analysis_operations::FlowAnalysisOperations;
use crate::shared_type::{
    SchemaView, SharedTypeKind, SharedTypeParameterView, SharedTypeSchemaView, SharedTypeView,
    TypeOf, TypeParameterOf, TypeView, Variance,
};
use crate::type_constraint::{MergedTypeConstraint, TypeConstraintGenerationDataForTesting};

/// Shorthand: the map from type parameters to their merged constraints
/// (Dart `Map<SharedTypeParameter, MergedTypeConstraint<...>>`).
pub type ConstraintsMap<O> = IndexMap<TypeParameterOf<O>, MergedTypeConstraint<TypeOf<O>>>;

/// Shorthand: the testing data of the operations `O`
/// (Dart `TypeConstraintGenerationDataForTesting<Variable, AstNode>`).
pub type DataForTestingOf<O> = TypeConstraintGenerationDataForTesting<
    TypeParameterOf<O>,
    TypeOf<O>,
    <O as TypeAnalyzerOperations>::AstNode,
>;

/// The key and value types of a map type (Dart record
/// `({SharedTypeView keyType, SharedTypeView valueType})`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyValueTypes<V> {
    /// The key type.
    pub key_type: V,
    /// The value type.
    pub value_type: V,
}

/// Callback API used by the shared type analyzer to query and manipulate the
/// client's representation of variables and types.
///
/// Dart: `TypeAnalyzerOperations<Variable, TypeDeclarationType,
/// TypeDeclaration, AstNode> implements FlowAnalysisOperations<Variable>`,
/// together with `TypeAnalyzerOperationsMixin` (provided methods).
///
/// Not object safe (generic associated type
/// [`ConstraintGenerator`](Self::ConstraintGenerator)); the shared code uses
/// it as a generic bound.
pub trait TypeAnalyzerOperations: FlowAnalysisOperations {
    /// A more specific type structure for types derived from a class, mixin,
    /// enum or extension type declaration (Dart type parameter
    /// `TypeDeclarationType`). For the analyzer this can be `TypeId` again.
    type TypeDeclarationType: Copy + Eq + Debug;

    /// The client's representation of a class, mixin, enum or extension type
    /// declaration (Dart type parameter `TypeDeclaration`). For the analyzer
    /// this is the id of an `InterfaceElement`.
    type TypeDeclaration: Copy + Eq + Debug;

    /// The client's AST node, used for testing data only (Dart type parameter
    /// `AstNode`).
    type AstNode: Copy + Eq + Hash + Debug;

    /// The constraint generator created by
    /// [`create_type_constraint_generator`](Self::create_type_constraint_generator).
    /// It borrows the operations (Dart: field `typeAnalyzerOperations`).
    type ConstraintGenerator<'a>: TypeConstraintGenerator<Operations = Self>
    where
        Self: 'a;

    /// Returns the type `double`.
    fn double_type(&self) -> TypeView<Self>;

    /// Returns the type `dynamic`.
    fn dynamic_type(&self) -> TypeView<Self>;

    /// Returns the type used by the client in the case of errors.
    fn error_type(&self) -> TypeView<Self>;

    /// Returns the type `int`.
    fn int_type(&self) -> TypeView<Self>;

    /// Returns the type `Never`.
    fn never_type(&self) -> TypeView<Self>;

    /// Returns the type `Null`.
    fn null_type(&self) -> TypeView<Self>;

    /// Returns the type `Object?`.
    fn object_question_type(&self) -> TypeView<Self>;

    /// Returns the type `Object`.
    fn object_type(&self) -> TypeView<Self>;

    /// Returns the unknown type schema (`_`) used in type inference.
    fn unknown_type(&self) -> SchemaView<Self>;

    /// Computes the constraint solution for a type parameter based on a given
    /// set of constraints.
    ///
    /// If `grounded` is `true`, then the returned type is guaranteed to be a
    /// known type (i.e. it will not contain any instances of `?`) if it is
    /// constrained at all. The returned type for unconstrained variables is
    /// `?`.
    ///
    /// If `is_contravariant` is `true`, then we are solving for a
    /// contravariant type parameter which means we choose the upper bound
    /// rather than the lower bound for normally covariant type parameters.
    ///
    /// See <https://github.com/dart-lang/language/blob/main/resources/type-system/inference.md#constraint-solution-for-a-type-variable>.
    fn choose_type_from_constraint(
        &self,
        constraint: &MergedTypeConstraint<Self::Type>,
        grounded: bool,
        is_contravariant: bool,
    ) -> Self::Type {
        let is_unknown = |schema: SchemaView<Self>| {
            self.shared_type_kind(schema.unwrap_type_schema_view()) == SharedTypeKind::Unknown
        };
        let lower = constraint.lower;
        let upper = constraint.upper;
        if !is_contravariant {
            // Prefer the known bound, if any.
            if self.is_known_type(lower) {
                return lower.unwrap_type_schema_view();
            }
            if self.is_known_type(upper) {
                return upper.unwrap_type_schema_view();
            }

            // Otherwise take whatever bound has partial information,
            // e.g. `Iterable<?>`
            if !is_unknown(lower) {
                if grounded {
                    self.least_closure_of_schema(lower).unwrap_type_view()
                } else {
                    lower.unwrap_type_schema_view()
                }
            } else if !is_unknown(upper) {
                if grounded {
                    self.greatest_closure_of_schema(upper, None)
                        .unwrap_type_view()
                } else {
                    upper.unwrap_type_schema_view()
                }
            } else {
                lower.unwrap_type_schema_view()
            }
        } else {
            // Prefer the known bound, if any.
            if self.is_known_type(upper) {
                return upper.unwrap_type_schema_view();
            }
            if self.is_known_type(lower) {
                return lower.unwrap_type_schema_view();
            }

            // Otherwise take whatever bound has partial information,
            // e.g. `Iterable<?>`
            if !is_unknown(upper) {
                if grounded {
                    self.greatest_closure_of_schema(upper, None)
                        .unwrap_type_view()
                } else {
                    upper.unwrap_type_schema_view()
                }
            } else if !is_unknown(lower) {
                if grounded {
                    self.least_closure_of_schema(lower).unwrap_type_view()
                } else {
                    lower.unwrap_type_schema_view()
                }
            } else {
                upper.unwrap_type_schema_view()
            }
        }
    }

    /// Computes (or recomputes) a set of inferred types based on the
    /// constraints that have been recorded so far.
    ///
    /// `constraints` is mutated (constraints from bounds are merged in).
    fn choose_types(
        &self,
        type_parameters_to_infer: &[Self::TypeParameter],
        constraints: &mut ConstraintsMap<Self>,
        previously_inferred_types: Option<&[Self::Type]>,
        preliminary: bool,
        inference_using_bounds_is_enabled: bool,
        data_for_testing: Option<&mut DataForTestingOf<Self>>,
        tree_node_for_testing: Option<Self::AstNode>,
    ) -> Vec<Self::Type> {
        let _ = (
            type_parameters_to_infer,
            constraints,
            previously_inferred_types,
            preliminary,
            inference_using_bounds_is_enabled,
            data_for_testing,
            tree_node_for_testing,
        );
        todo!("chooseTypes")
    }

    /// Creates a constraint generator for `type_parameters_to_infer`.
    ///
    /// Dart also passes `typeAnalyzerOperations: this`; here the generator
    /// borrows `self`.
    fn create_type_constraint_generator<'a>(
        &'a self,
        type_constraint_generation_data_for_testing: Option<&'a mut DataForTestingOf<Self>>,
        type_parameters_to_infer: &[SharedTypeParameterView<Self::TypeParameter>],
        inference_using_bounds_is_enabled: bool,
    ) -> Self::ConstraintGenerator<'a>;

    /// Computes `flatten` of `ty`.
    fn flatten(&self, ty: TypeView<Self>) -> TypeView<Self>;

    /// Returns the type `FutureOr` with omitted nullability and type argument
    /// `argument_type`.
    fn future_or_type(&self, argument_type: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(self.future_or_type_internal(argument_type.unwrap_type_view()))
    }

    /// Returns `FutureOr<type_structure>` with omitted nullability. Provides
    /// [`future_or_type`](Self::future_or_type) and
    /// [`future_or_type_schema`](Self::future_or_type_schema).
    fn future_or_type_internal(&self, type_structure: Self::Type) -> Self::Type;

    /// Returns the type schema `FutureOr` with omitted nullability and type
    /// argument `argument_type_schema`.
    fn future_or_type_schema(&self, argument_type_schema: SchemaView<Self>) -> SchemaView<Self> {
        SharedTypeSchemaView::new(
            self.future_or_type_internal(argument_type_schema.unwrap_type_schema_view()),
        )
    }

    /// Returns the type `Future` with omitted nullability and type argument
    /// `argument_type`.
    fn future_type(&self, argument_type: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(self.future_type_internal(argument_type.unwrap_type_view()))
    }

    /// Returns `Future<type_structure>` with omitted nullability. Provides
    /// [`future_type`](Self::future_type) and
    /// [`future_type_schema`](Self::future_type_schema).
    fn future_type_internal(&self, type_structure: Self::Type) -> Self::Type;

    /// Returns the type schema `Future` with omitted nullability and type
    /// argument `argument_type_schema`.
    fn future_type_schema(&self, argument_type_schema: SchemaView<Self>) -> SchemaView<Self> {
        SharedTypeSchemaView::new(
            self.future_type_internal(argument_type_schema.unwrap_type_schema_view()),
        )
    }

    /// If `ty` was introduced by a class, mixin, enum, or extension type,
    /// returns a [`TypeDeclarationKind`] indicating what kind of thing it was
    /// introduced by. Otherwise, returns `None`.
    fn get_type_declaration_kind(&self, ty: TypeView<Self>) -> Option<TypeDeclarationKind> {
        self.get_type_declaration_kind_internal(ty.unwrap_type_view())
    }

    /// `getTypeDeclarationKindInternal`: the structure form of
    /// [`get_type_declaration_kind`](Self::get_type_declaration_kind).
    fn get_type_declaration_kind_internal(&self, ty: Self::Type) -> Option<TypeDeclarationKind>;

    /// Returns variance for of the type parameter at index `parameter_index`
    /// in `type_declaration`.
    fn get_type_parameter_variance(
        &self,
        type_declaration: Self::TypeDeclaration,
        parameter_index: usize,
    ) -> Variance;

    /// If at top level `type_schema` describes a type that was introduced by
    /// a class, mixin, enum, or extension type, returns a
    /// [`TypeDeclarationKind`] indicating what kind of thing it was
    /// introduced by. Otherwise, returns `None`.
    fn get_type_schema_declaration_kind(
        &self,
        type_schema: SchemaView<Self>,
    ) -> Option<TypeDeclarationKind> {
        self.get_type_declaration_kind_internal(type_schema.unwrap_type_schema_view())
    }

    /// Computes the greatest lower bound of `type1` and `type2`.
    fn glb(&self, type1: TypeView<Self>, type2: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(self.glb_internal(type1.unwrap_type_view(), type2.unwrap_type_view()))
    }

    /// Greatest lower bound on type structures. Provides [`glb`](Self::glb)
    /// and [`type_schema_glb`](Self::type_schema_glb).
    fn glb_internal(&self, type1: Self::Type, type2: Self::Type) -> Self::Type;

    /// Computes the greatest closure of a type schema.
    ///
    /// `top_type` accounts for a known discrepancy between the CFE and the
    /// analyzer (<https://github.com/dart-lang/language/issues/4466>); Dart
    /// named parameter `{SharedTypeView? topType}`.
    fn greatest_closure_of_schema(
        &self,
        schema: SchemaView<Self>,
        top_type: Option<TypeView<Self>>,
    ) -> TypeView<Self>;

    /// Computes the greatest closure of a type with respect to
    /// `type_parameters_to_eliminate`.
    fn greatest_closure_of_type_internal(
        &self,
        ty: Self::Type,
        type_parameters_to_eliminate: &[Self::TypeParameter],
    ) -> Self::Type;

    /// Chooses types from all available sources at the final stage of
    /// inference.
    ///
    /// Dart also passes `constraint`, which is always
    /// `constraints[typeParameterToInfer]` (the same object, mutated through
    /// both references). Here the constraint is looked up in `constraints`.
    /// Dart defaults: `isContravariant = false`, `isLegacyCovariant = true`.
    fn infer_type_parameter_from_all(
        &self,
        type_from_previous_inference: Option<Self::Type>,
        extends_constraint: Option<Self::Type>,
        is_contravariant: bool,
        is_legacy_covariant: bool,
        constraints: &mut ConstraintsMap<Self>,
        type_parameter_to_infer: Self::TypeParameter,
        type_parameters_to_infer: &[Self::TypeParameter],
        data_for_testing: Option<&mut DataForTestingOf<Self>>,
        inference_using_bounds_is_enabled: bool,
    ) -> Self::Type {
        let _ = (
            type_from_previous_inference,
            extends_constraint,
            is_contravariant,
            is_legacy_covariant,
            constraints,
            type_parameter_to_infer,
            type_parameters_to_infer,
            data_for_testing,
            inference_using_bounds_is_enabled,
        );
        todo!("inferTypeParameterFromAll")
    }

    /// Chooses types from the current inference context in preliminary
    /// stages.
    ///
    /// See [`infer_type_parameter_from_all`](Self::infer_type_parameter_from_all)
    /// for the `constraint` parameter. Dart default:
    /// `isLegacyCovariant = true`.
    fn infer_type_parameter_from_context(
        &self,
        type_from_previous_inference: Option<Self::Type>,
        extends_constraint: Option<Self::Type>,
        is_contravariant: bool,
        is_legacy_covariant: bool,
        constraints: &mut ConstraintsMap<Self>,
        type_parameters_to_infer: &[Self::TypeParameter],
        type_parameter_to_infer: Self::TypeParameter,
        data_for_testing: Option<&mut DataForTestingOf<Self>>,
        inference_using_bounds_is_enabled: bool,
    ) -> Self::Type {
        let _ = (
            type_from_previous_inference,
            extends_constraint,
            is_contravariant,
            is_legacy_covariant,
            constraints,
            type_parameters_to_infer,
            type_parameter_to_infer,
            data_for_testing,
            inference_using_bounds_is_enabled,
        );
        todo!("inferTypeParameterFromContext")
    }

    /// Queries whether `ty` is an "always-exhaustive" type (as defined in the
    /// patterns spec).
    fn is_always_exhaustive_type(&self, ty: TypeView<Self>) -> bool;

    /// Returns `true` if `from_type` is assignable to `to_type`.
    fn is_assignable_to(&self, from_type: TypeView<Self>, to_type: TypeView<Self>) -> bool;

    /// True if `type_parameter` doesn't have an explicit bound.
    fn is_bound_omitted(&self, type_parameter: Self::TypeParameter) -> bool;

    /// Returns `true` if `ty` is `Function` from `dart:core` (not `Function?`
    /// or `Function*`).
    fn is_dart_core_function_internal(&self, ty: Self::Type) -> bool;

    /// Returns `true` if `ty` is `Record` from `dart:core` (not `Record?` or
    /// `Record*`).
    fn is_dart_core_record_internal(&self, ty: Self::Type) -> bool;

    /// Returns `true` if `ty` is `E<T1, ..., Tn>`, `E<T1, ..., Tn>?`, or
    /// `E<T1, ..., Tn>*` for some extension type declaration E.
    fn is_extension_type_internal(&self, ty: Self::Type) -> bool;

    /// Returns `true` if `ty` is `A<T1, ..., Tn>`, `A<T1, ..., Tn>?`, or
    /// `A<T1, ..., Tn>*` for some class, mixin, or enum A. Returns `false`
    /// for extension types, type aliases, `Null`, `Never`, and `FutureOr<X>`.
    fn is_interface_type_internal(&self, ty: Self::Type) -> bool;

    /// Determines whether a type schema contains no unknown type `_`
    /// (`int`, `List<String>` are known; `_`, `List<_>` are not).
    fn is_known_type(&self, type_schema: SchemaView<Self>) -> bool;

    /// Returns `true` if `Null` is not a subtype of all types matching `ty`.
    fn is_non_nullable_internal(&self, ty: Self::Type) -> bool;

    /// Returns `true` if `Null` is a subtype of all types matching `ty`.
    fn is_nullable_internal(&self, ty: Self::Type) -> bool;

    /// Returns `true` if `ty` is `Object` from `dart:core` (not `Object?` or
    /// `Object*`).
    fn is_object(&self, ty: TypeView<Self>) -> bool;

    /// Subtype test on type structures. Provides
    /// [`type_is_subtype_of_type_schema`](Self::type_is_subtype_of_type_schema),
    /// [`type_schema_is_subtype_of_type`](Self::type_schema_is_subtype_of_type),
    /// [`type_schema_is_subtype_of_type_schema`](Self::type_schema_is_subtype_of_type_schema)
    /// and (by the client, see its doc)
    /// [`is_subtype_of`](crate::flow_analysis_operations::FlowAnalysisTypeOperations::is_subtype_of).
    fn is_subtype_of_internal(&self, left: Self::Type, right: Self::Type) -> bool;

    /// Returns `true` if the type `ty` satisfies the type schema
    /// `type_schema`.
    fn is_type_schema_satisfied(&self, type_schema: SchemaView<Self>, ty: TypeView<Self>) -> bool;

    /// Returns whether `node` is final.
    fn is_variable_final(&self, node: Self::Variable) -> bool;

    /// Returns the type schema `Iterable`, with type argument.
    fn iterable_type_schema(&self, element_type_schema: SchemaView<Self>) -> SchemaView<Self>;

    /// Computes the least closure of a type schema.
    fn least_closure_of_schema(&self, schema: SchemaView<Self>) -> TypeView<Self>;

    /// Computes the least closure of a type with respect to
    /// `type_parameters_to_eliminate`.
    fn least_closure_of_type_internal(
        &self,
        ty: Self::Type,
        type_parameters_to_eliminate: &[Self::TypeParameter],
    ) -> Self::Type;

    /// Returns the type `List`, with type argument `element_type`.
    fn list_type(&self, element_type: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(self.list_type_internal(element_type.unwrap_type_view()))
    }

    /// `List<element_type>`. Provides [`list_type`](Self::list_type) and
    /// [`list_type_schema`](Self::list_type_schema).
    fn list_type_internal(&self, element_type: Self::Type) -> Self::Type;

    /// Returns the type schema `List`, with type argument
    /// `element_type_schema`.
    fn list_type_schema(&self, element_type_schema: SchemaView<Self>) -> SchemaView<Self> {
        SharedTypeSchemaView::new(
            self.list_type_internal(element_type_schema.unwrap_type_schema_view()),
        )
    }

    /// Computes the least upper bound of `type1` and `type2`.
    fn lub(&self, type1: TypeView<Self>, type2: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(self.lub_internal(type1.unwrap_type_view(), type2.unwrap_type_view()))
    }

    /// Least upper bound on type structures. Provides [`lub`](Self::lub) and
    /// [`type_schema_lub`](Self::type_schema_lub).
    fn lub_internal(&self, type1: Self::Type, type2: Self::Type) -> Self::Type;

    /// Nullable form of a type structure. Provides
    /// [`make_type_schema_nullable`](Self::make_type_schema_nullable) and
    /// (by the client, see its doc)
    /// [`make_nullable`](crate::flow_analysis_operations::FlowAnalysisTypeOperations::make_nullable).
    fn make_nullable_internal(&self, ty: Self::Type) -> Self::Type;

    /// Computes the nullable form of `type_schema`.
    fn make_type_schema_nullable(&self, type_schema: SchemaView<Self>) -> SchemaView<Self> {
        SharedTypeSchemaView::new(
            self.make_nullable_internal(type_schema.unwrap_type_schema_view()),
        )
    }

    /// Returns the type `Map`, with type arguments.
    fn map_type(&self, key_type: TypeView<Self>, value_type: TypeView<Self>) -> TypeView<Self> {
        SharedTypeView::new(
            self.map_type_internal(key_type.unwrap_type_view(), value_type.unwrap_type_view()),
        )
    }

    /// `Map<key_type, value_type>`. Provides [`map_type`](Self::map_type) and
    /// [`map_type_schema`](Self::map_type_schema).
    fn map_type_internal(&self, key_type: Self::Type, value_type: Self::Type) -> Self::Type;

    /// Returns the type schema `Map`, with type arguments `key_type_schema`
    /// and `value_type_schema`.
    fn map_type_schema(
        &self,
        key_type_schema: SchemaView<Self>,
        value_type_schema: SchemaView<Self>,
    ) -> SchemaView<Self> {
        SharedTypeSchemaView::new(self.map_type_internal(
            key_type_schema.unwrap_type_schema_view(),
            value_type_schema.unwrap_type_schema_view(),
        ))
    }

    /// If `ty` takes the form `FutureOr<T>`, `FutureOr<T>?`, or `FutureOr<T>*`
    /// for some `T`, returns the type `T`. Otherwise returns `None`.
    fn match_future_or(&self, ty: TypeView<Self>) -> Option<TypeView<Self>> {
        self.match_future_or_internal(ty.unwrap_type_view())
            .map(SharedTypeView::new)
    }

    /// Structure form of [`match_future_or`](Self::match_future_or) and
    /// [`match_type_schema_future_or`](Self::match_type_schema_future_or).
    fn match_future_or_internal(&self, ty: Self::Type) -> Option<Self::Type>;

    /// If `ty` is a parameter type that is of a kind used in type inference,
    /// returns the corresponding parameter.
    fn match_inferable_parameter_internal(&self, ty: Self::Type) -> Option<Self::TypeParameter>;

    /// If `ty` is a subtype of the type `Iterable<T>?` for some `T`, returns
    /// the type `T`. Otherwise returns `None`.
    fn match_iterable_type(&self, ty: TypeView<Self>) -> Option<TypeView<Self>> {
        self.match_iterable_type_internal(ty.unwrap_type_view())
            .map(SharedTypeView::new)
    }

    /// Structure form of [`match_iterable_type`](Self::match_iterable_type)
    /// and [`match_iterable_type_schema`](Self::match_iterable_type_schema).
    fn match_iterable_type_internal(&self, ty: Self::Type) -> Option<Self::Type>;

    /// If `type_schema` is the type schema `Iterable<T>?` (or a subtype
    /// thereof), for some `T`, returns the type `T`. Otherwise returns
    /// `None`.
    fn match_iterable_type_schema(
        &self,
        type_schema: SchemaView<Self>,
    ) -> Option<SchemaView<Self>> {
        self.match_iterable_type_internal(type_schema.unwrap_type_schema_view())
            .map(SharedTypeSchemaView::new)
    }

    /// If `ty` is a subtype of the type `List<T>?` for some `T`, returns the
    /// type `T`. Otherwise returns `None`.
    fn match_list_type(&self, ty: TypeView<Self>) -> Option<TypeView<Self>>;

    /// If `ty` is a subtype of the type `Map<K, V>?` for some `K` and `V`,
    /// returns these `K` and `V`. Otherwise returns `None`.
    fn match_map_type(&self, ty: TypeView<Self>) -> Option<KeyValueTypes<TypeView<Self>>>;

    /// If `ty` is a subtype of the type `Stream<T>?` for some `T`, returns
    /// the type `T`. Otherwise returns `None`.
    fn match_stream_type(&self, ty: TypeView<Self>) -> Option<TypeView<Self>>;

    /// If `ty` was introduced by a class, mixin, enum, or extension type,
    /// returns a [`TypeDeclarationMatchResult`] describing the constituents
    /// of the matched type. Otherwise returns `None`.
    fn match_type_declaration_type_internal(
        &self,
        ty: Self::Type,
    ) -> Option<
        TypeDeclarationMatchResult<Self::TypeDeclarationType, Self::TypeDeclaration, Self::Type>,
    >;

    /// If `ty` is a parameter type with empty nullability suffix, returns its
    /// bound, whether it is its type parameter bound or its promoted bound.
    /// Otherwise, returns `None`.
    fn match_type_parameter_bound_internal(&self, ty: Self::Type) -> Option<Self::Type>;

    /// If `type_schema` takes the form `FutureOr<T>`, `FutureOr<T>?`, or
    /// `FutureOr<T>*` for some `T`, returns the type schema `T`. Otherwise
    /// returns `None`.
    fn match_type_schema_future_or(
        &self,
        type_schema: SchemaView<Self>,
    ) -> Option<SchemaView<Self>> {
        self.match_future_or_internal(type_schema.unwrap_type_schema_view())
            .map(SharedTypeSchemaView::new)
    }

    /// Generates the constraints implied by `lower <: bound of
    /// type_parameter_to_infer` and merges them into
    /// `inference_phase_constraints`. Returns the constraint for
    /// `type_parameter_to_infer`.
    fn merge_in_constraints_from_bound(
        &self,
        type_parameter_to_infer: Self::TypeParameter,
        type_parameters_to_infer: &[SharedTypeParameterView<Self::TypeParameter>],
        lower: Self::Type,
        inference_phase_constraints: &mut ConstraintsMap<Self>,
        data_for_testing: Option<&mut DataForTestingOf<Self>>,
        inference_using_bounds_is_enabled: bool,
    ) -> MergedTypeConstraint<Self::Type> {
        let _ = (
            type_parameter_to_infer,
            type_parameters_to_infer,
            lower,
            inference_phase_constraints,
            data_for_testing,
            inference_using_bounds_is_enabled,
        );
        todo!("mergeInConstraintsFromBound")
    }

    /// Computes `NORM` of `ty`.
    fn normalize(&self, ty: TypeView<Self>) -> TypeView<Self>;

    /// Builds the client specific record type.
    fn record_type(
        &self,
        positional: &[TypeView<Self>],
        named: &[(Self::Name, TypeView<Self>)],
    ) -> TypeView<Self> {
        let positional: Vec<Self::Type> = positional.iter().map(|t| t.unwrap_type_view()).collect();
        let named: Vec<(Self::Name, Self::Type)> = named
            .iter()
            .map(|(n, t)| (*n, t.unwrap_type_view()))
            .collect();
        SharedTypeView::new(self.record_type_internal(&positional, &named))
    }

    /// Record type on type structures. Provides
    /// [`record_type`](Self::record_type) and
    /// [`record_type_schema`](Self::record_type_schema).
    fn record_type_internal(
        &self,
        positional: &[Self::Type],
        named: &[(Self::Name, Self::Type)],
    ) -> Self::Type;

    /// Builds the client specific record type schema.
    fn record_type_schema(
        &self,
        positional: &[SchemaView<Self>],
        named: &[(Self::Name, SchemaView<Self>)],
    ) -> SchemaView<Self> {
        let positional: Vec<Self::Type> = positional
            .iter()
            .map(|t| t.unwrap_type_schema_view())
            .collect();
        let named: Vec<(Self::Name, Self::Type)> = named
            .iter()
            .map(|(n, t)| (*n, t.unwrap_type_schema_view()))
            .collect();
        SharedTypeSchemaView::new(self.record_type_internal(&positional, &named))
    }

    /// Returns the type schema `Stream`, with type argument
    /// `element_type_schema`.
    fn stream_type_schema(&self, element_type_schema: SchemaView<Self>) -> SchemaView<Self>;

    /// Substitutes `types` for `type_parameters` in `type_to_substitute`.
    fn substitute_type_from_iterables(
        &self,
        type_to_substitute: Self::Type,
        type_parameters: &[Self::TypeParameter],
        types: &[Self::Type],
    ) -> Self::Type;

    /// Returns `true` if `left_type` is a subtype of the greatest closure of
    /// `right_schema`.
    fn type_is_subtype_of_type_schema(
        &self,
        left_type: TypeView<Self>,
        right_schema: SchemaView<Self>,
    ) -> bool {
        self.is_subtype_of_internal(
            left_type.unwrap_type_view(),
            right_schema.unwrap_type_schema_view(),
        )
    }

    /// Computes the greatest lower bound of `type_schema1` and
    /// `type_schema2`.
    fn type_schema_glb(
        &self,
        type_schema1: SchemaView<Self>,
        type_schema2: SchemaView<Self>,
    ) -> SchemaView<Self> {
        SharedTypeSchemaView::new(self.glb_internal(
            type_schema1.unwrap_type_schema_view(),
            type_schema2.unwrap_type_schema_view(),
        ))
    }

    /// Returns `true` if the least closure of `left_schema` is a subtype of
    /// `right_type`.
    fn type_schema_is_subtype_of_type(
        &self,
        left_schema: SchemaView<Self>,
        right_type: TypeView<Self>,
    ) -> bool {
        self.is_subtype_of_internal(
            left_schema.unwrap_type_schema_view(),
            right_type.unwrap_type_view(),
        )
    }

    /// Returns `true` if least closure of `left_schema` is a subtype of the
    /// greatest closure of `right_schema`.
    fn type_schema_is_subtype_of_type_schema(
        &self,
        left_schema: SchemaView<Self>,
        right_schema: SchemaView<Self>,
    ) -> bool {
        self.is_subtype_of_internal(
            left_schema.unwrap_type_schema_view(),
            right_schema.unwrap_type_schema_view(),
        )
    }

    /// Computes the least upper bound of `type_schema1` and `type_schema2`.
    fn type_schema_lub(
        &self,
        type_schema1: SchemaView<Self>,
        type_schema2: SchemaView<Self>,
    ) -> SchemaView<Self> {
        SharedTypeSchemaView::new(self.lub_internal(
            type_schema1.unwrap_type_schema_view(),
            type_schema2.unwrap_type_schema_view(),
        ))
    }

    /// Converts a type into a corresponding type schema.
    fn type_to_schema(&self, ty: TypeView<Self>) -> SchemaView<Self> {
        SharedTypeSchemaView::new(ty.unwrap_type_view())
    }

    /// Looks up the type of the interface member `lookup_name` in `ty`: the
    /// static type of `e.lookupName` where `e` has static type `ty`. Returns
    /// `None` if there's no such member.
    ///
    /// For a method, its function type is returned; for a getter, its return
    /// type. Type substitution is applied (`A<String>`, `foo` gives
    /// `String Function(List<String>)` for `X foo(List<X> list)` in
    /// `class A<X>`). Nullable types have the members of `Object`, function
    /// types have `call`, `dynamic` has every member with type `dynamic`.
    fn lookup_member_type_internal(
        &self,
        ty: Self::Type,
        lookup_name: Self::Name,
    ) -> Option<Self::Type>;
}

/// Abstract interface of a type constraint generator.
///
/// Dart: abstract class `TypeConstraintGenerator<Variable,
/// TypeDeclarationType, TypeDeclaration, AstNode>` together with
/// `TypeConstraintGeneratorMixin`. The analyzer's implementation is
/// `TypeConstraintGatherer` (unit A6); the provided methods are the shared
/// algorithm (unit A13).
///
/// The `perform...` methods match `p` against `q`. If `p` is a subtype of
/// `q` under some constraints, the constraints are recorded and `true` is
/// returned. Otherwise the constraint state is unchanged (or rolled back
/// with [`restore_state`](Self::restore_state)) and `false` is returned.
/// Only `p` or `q` may be a schema: if `left_schema` is `true`, `p` may
/// contain `_`; if it is `false`, `q` may contain `_`.
///
/// Object safe once `Operations` is fixed.
pub trait TypeConstraintGenerator {
    /// The type operations used by the generator.
    type Operations: TypeAnalyzerOperations + ?Sized;

    /// True if the language feature inference-using-bounds is enabled (Dart
    /// field `inferenceUsingBoundsIsEnabled`).
    fn inference_using_bounds_is_enabled(&self) -> bool;

    /// The current state of the constraint generator, a checkpoint that
    /// [`restore_state`](Self::restore_state) can roll back to.
    fn current_state(&self) -> TypeConstraintGeneratorState;

    /// True if FutureOr types are required to have `isQuestionType == false`
    /// when they are matched (analyzer/CFE discrepancy,
    /// <https://github.com/dart-lang/sdk/issues/55344>).
    fn enable_discrepant_obliviousness_of_nullability_suffix_of_future_or(&self) -> bool;

    /// Abstract type operations to be used in the matching methods.
    fn type_analyzer_operations(&self) -> &Self::Operations;

    /// Type parameters being constrained.
    fn type_parameters_to_constrain(&self) -> Vec<TypeParameterOf<Self::Operations>>;

    /// Add constraint: `lower <: type_parameter <: TOP`.
    fn add_lower_constraint_for_parameter(
        &mut self,
        type_parameter: TypeParameterOf<Self::Operations>,
        lower: TypeOf<Self::Operations>,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    );

    /// Add constraint: `BOTTOM <: type_parameter <: upper`.
    fn add_upper_constraint_for_parameter(
        &mut self,
        type_parameter: TypeParameterOf<Self::Operations>,
        upper: TypeOf<Self::Operations>,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    );

    /// Returns the set of type constraints that was gathered.
    fn compute_constraints(&mut self) -> ConstraintsMap<Self::Operations>;

    /// Iterates over all of the type constraints generated since
    /// `elimination_start_state` and eliminates the type variables in them
    /// using `type_parameters_to_eliminate`.
    fn eliminate_type_parameters_in_generated_constraints(
        &mut self,
        type_parameters_to_eliminate: &[TypeParameterOf<Self::Operations>],
        elimination_start_state: TypeConstraintGeneratorState,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    );

    /// Returns the type arguments of the supertype of `ty` that is an
    /// instantiation of `type_declaration`, or `None`.
    fn get_type_arguments_as_instance_of(
        &self,
        ty: <Self::Operations as TypeAnalyzerOperations>::TypeDeclarationType,
        type_declaration: <Self::Operations as TypeAnalyzerOperations>::TypeDeclaration,
    ) -> Option<Vec<TypeOf<Self::Operations>>>;

    /// Creates fresh type parameters, instantiates the non-generic parts of
    /// the function types `p` and `q` with them, and returns the instantiated
    /// function types and the fresh type parameters (to eliminate later).
    ///
    /// Dart returns the record `(SharedType, SharedType, {List<...>
    /// typeParametersToEliminate})`.
    fn instantiate_function_types_and_provide_fresh_type_parameters(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
    ) -> (
        TypeOf<Self::Operations>,
        TypeOf<Self::Operations>,
        Vec<TypeParameterOf<Self::Operations>>,
    );

    /// Matches `p` against `q` when both are function types.
    fn perform_subtype_constraint_generation_for_function_types(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForFunctionTypes")
    }

    /// Matches `p` against `q` when `p` is `FutureOr<p0>`.
    fn perform_subtype_constraint_generation_for_left_future_or(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForLeftFutureOr")
    }

    /// Matches `p` against `q` when `p` is `p0?`.
    fn perform_subtype_constraint_generation_for_left_nullable_type(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForLeftNullableType")
    }

    /// Matches `p` against `q` when both are record types.
    fn perform_subtype_constraint_generation_for_record_types(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForRecordTypes")
    }

    /// Matches `p` against `q` when `q` is `FutureOr<q0>`.
    fn perform_subtype_constraint_generation_for_right_future_or(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForRightFutureOr")
    }

    /// Matches `p` against `q` when `q` is `q0?`.
    fn perform_subtype_constraint_generation_for_right_nullable_type(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForRightNullableType")
    }

    /// Matches `p` against `q` when both are type declaration types. Returns
    /// `None` (state unchanged) if either is not a type declaration type.
    fn perform_subtype_constraint_generation_for_type_declaration_types(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> Option<bool> {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationForTypeDeclarationTypes")
    }

    /// Matches `p` against `q` (the main entry point of subtype constraint
    /// generation).
    fn perform_subtype_constraint_generation_internal(
        &mut self,
        p: TypeOf<Self::Operations>,
        q: TypeOf<Self::Operations>,
        left_schema: bool,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        let _ = (p, q, left_schema, ast_node_for_testing);
        todo!("performSubtypeConstraintGenerationInternal")
    }

    /// Matches the schema `p` against the type `q`
    /// (`TypeConstraintGeneratorMixin`).
    fn perform_subtype_constraint_generation_left_schema(
        &mut self,
        p: SchemaView<Self::Operations>,
        q: TypeView<Self::Operations>,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        self.perform_subtype_constraint_generation_internal(
            p.unwrap_type_schema_view(),
            q.unwrap_type_view(),
            true,
            ast_node_for_testing,
        )
    }

    /// Matches the type `p` against the schema `q`
    /// (`TypeConstraintGeneratorMixin`).
    fn perform_subtype_constraint_generation_right_schema(
        &mut self,
        p: TypeView<Self::Operations>,
        q: SchemaView<Self::Operations>,
        ast_node_for_testing: Option<<Self::Operations as TypeAnalyzerOperations>::AstNode>,
    ) -> bool {
        self.perform_subtype_constraint_generation_internal(
            p.unwrap_type_view(),
            q.unwrap_type_schema_view(),
            false,
            ast_node_for_testing,
        )
    }

    /// Rolls the generator back to `state` (discards the constraints
    /// generated after it).
    fn restore_state(&mut self, state: TypeConstraintGeneratorState);
}

/// Describes all possibility for a type to be derived from a declaration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeDeclarationKind {
    /// The type is derived from a declaration inducing interface (a class,
    /// mixin, or enum declaration).
    InterfaceDeclaration,

    /// The type is derived from an extension type declaration.
    ExtensionTypeDeclaration,
}

/// Describes constituents of a type derived from a declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TypeDeclarationMatchResult<TypeDeclarationType, TypeDeclaration, Type> {
    /// The kind of type declaration the matched type is of.
    pub type_declaration_kind: TypeDeclarationKind,

    /// A more specific client type describing the matched type.
    pub type_declaration_type: TypeDeclarationType,

    /// The type declaration (class, enum, mixin, or extension type) that the
    /// matched type is derived from.
    pub type_declaration: TypeDeclaration,

    /// Type arguments instantiating `type_declaration` to the matched type.
    /// Empty if `type_declaration` is not generic.
    pub type_arguments: Vec<Type>,
}

/// Representation of the state of [`TypeConstraintGenerator`]: the count of
/// the constraints generated so far. Restoring to a state means discarding
/// the constraints generated after it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TypeConstraintGeneratorState {
    /// The number of constraints generated so far.
    pub count: usize,
}
