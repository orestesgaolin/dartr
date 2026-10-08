// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer_operations.dart

//! The bodies of the `TypeAnalyzerOperationsMixin` inference methods and of
//! the shared subtype constraint generation algorithm of
//! `TypeConstraintGenerator`.
//!
//! The traits are
//! [`TypeAnalyzerOperations`] and [`TypeConstraintGenerator`] in
//! `dartr_flow`. Their provided methods that are `todo!()` there are ported
//! here as generic free functions with the snake_case Dart names. A client
//! applies them with the macros [`type_analyzer_operations_mixin!`] and
//! [`type_constraint_generator_mixin!`] inside its `impl` block.
//!
//! Inside the functions, calls to other members go through the trait, so a
//! client override is respected (Dart virtual dispatch).
//!
//! [`type_analyzer_operations_mixin!`]: crate::type_analyzer_operations_mixin
//! [`type_constraint_generator_mixin!`]: crate::type_constraint_generator_mixin

use std::cmp::Ordering;

use dartr_flow::flow_analysis_operations::FlowAnalysisTypeOperations;
use dartr_flow::shared_type::{
    SharedTypeKind, SharedTypeOperations, SharedTypeParameterView, SharedTypeSchemaView,
    SharedTypeView, TypeOf, Variance,
};
use dartr_flow::type_analyzer_operations::{
    ConstraintsMap, DataForTestingOf, TypeAnalyzerOperations, TypeConstraintGenerator,
};
use dartr_flow::type_constraint::{MergedTypeConstraint, TypeConstraintOrigin};

/// Shorthand: the AST node of the operations of the generator `G`.
type AstNodeOf<G> = <<G as TypeConstraintGenerator>::Operations as TypeAnalyzerOperations>::AstNode;

/// Shorthand: the type of the operations of the generator `G`.
type GType<G> = TypeOf<<G as TypeConstraintGenerator>::Operations>;

/// Whether `ty` is the unknown type schema `_` (Dart `is
/// SharedUnknownTypeSchemaView` / `is SharedUnknownType`).
fn is_unknown<O: TypeAnalyzerOperations>(o: &O, ty: TypeOf<O>) -> bool {
    o.shared_type_kind(ty) == SharedTypeKind::Unknown
}

// ===================================================== TypeAnalyzerOperationsMixin

/// `TypeAnalyzerOperationsMixin.chooseTypes`: computes (or recomputes) a set
/// of inferred types based on the constraints that have been recorded so
/// far.
pub fn choose_types<O: TypeAnalyzerOperations>(
    o: &O,
    type_parameters_to_infer: &[O::TypeParameter],
    constraints: &mut ConstraintsMap<O>,
    previously_inferred_types: Option<&[O::Type]>,
    preliminary: bool,
    inference_using_bounds_is_enabled: bool,
    mut data_for_testing: Option<&mut DataForTestingOf<O>>,
    tree_node_for_testing: Option<O::AstNode>,
) -> Vec<O::Type> {
    let _ = tree_node_for_testing;
    let mut inferred_types: Vec<O::Type> = match previously_inferred_types {
        Some(types) => types.to_vec(),
        None => vec![o.unknown_type().unwrap_type_schema_view(); type_parameters_to_infer.len()],
    };

    for i in 0..type_parameters_to_infer.len() {
        let type_param = type_parameters_to_infer[i];

        let type_param_bound = o.bound_shared(type_param);
        let mut extends_constraint = None;
        if let Some(type_param_bound) = type_param_bound
            && !o.is_bound_omitted(type_param)
        {
            extends_constraint = Some(o.substitute_type_from_iterables(
                type_param_bound,
                type_parameters_to_infer,
                &inferred_types,
            ));
        }

        assert!(
            constraints.contains_key(&type_param),
            "chooseTypes: no constraint for {type_param:?}"
        );
        if preliminary {
            inferred_types[i] = o.infer_type_parameter_from_context(
                previously_inferred_types.map(|types| types[i]),
                extends_constraint,
                o.variance(type_param) == Variance::Contravariant,
                o.is_legacy_covariant(type_param),
                constraints,
                type_parameters_to_infer,
                type_param,
                data_for_testing.as_deref_mut(),
                inference_using_bounds_is_enabled,
            );
        } else {
            inferred_types[i] = o.infer_type_parameter_from_all(
                previously_inferred_types.map(|types| types[i]),
                extends_constraint,
                o.variance(type_param) == Variance::Contravariant,
                o.is_legacy_covariant(type_param),
                constraints,
                type_param,
                type_parameters_to_infer,
                data_for_testing.as_deref_mut(),
                inference_using_bounds_is_enabled,
            );
        }
    }

    inferred_types
}

/// Merges the constraints implied by the bound of `type_parameter_to_infer`
/// into `constraints[type_parameter_to_infer]` (the shared part of
/// `inferTypeParameterFromAll` and `inferTypeParameterFromContext`).
fn merge_in_bound_constraints_if_enabled<O: TypeAnalyzerOperations>(
    o: &O,
    constraints: &mut ConstraintsMap<O>,
    type_parameter_to_infer: O::TypeParameter,
    type_parameters_to_infer: &[O::TypeParameter],
    data_for_testing: Option<&mut DataForTestingOf<O>>,
    inference_using_bounds_is_enabled: bool,
) {
    let lower = constraints[&type_parameter_to_infer].lower;
    if inference_using_bounds_is_enabled
        && !is_unknown(o, lower.unwrap_type_schema_view())
        && !o.is_bound_omitted(type_parameter_to_infer)
    {
        let type_parameters_to_infer: Vec<SharedTypeParameterView<O::TypeParameter>> =
            type_parameters_to_infer
                .iter()
                .map(|p| SharedTypeParameterView::new(*p))
                .collect();
        let constraint_from_bound = o.merge_in_constraints_from_bound(
            type_parameter_to_infer,
            &type_parameters_to_infer,
            lower.unwrap_type_schema_view(),
            constraints,
            data_for_testing,
            inference_using_bounds_is_enabled,
        );

        // Dart mutates `constraint`, which is the same object as
        // `constraints[typeParameterToInfer]`.
        let constraint = constraints
            .get_mut(&type_parameter_to_infer)
            .expect("constraint for the type parameter to infer");
        constraint.merge_in_type_schema_upper(constraint_from_bound.upper, o);
        constraint.merge_in_type_schema_lower(constraint_from_bound.lower, o);
    }
}

/// `TypeAnalyzerOperationsMixin.inferTypeParameterFromAll`: chooses types
/// from all available sources at the final stage of inference.
///
/// The Dart parameter `constraint` is `constraints[type_parameter_to_infer]`.
pub fn infer_type_parameter_from_all<O: TypeAnalyzerOperations>(
    o: &O,
    type_from_previous_inference: Option<O::Type>,
    extends_constraint: Option<O::Type>,
    is_contravariant: bool,
    is_legacy_covariant: bool,
    constraints: &mut ConstraintsMap<O>,
    type_parameter_to_infer: O::TypeParameter,
    type_parameters_to_infer: &[O::TypeParameter],
    data_for_testing: Option<&mut DataForTestingOf<O>>,
    inference_using_bounds_is_enabled: bool,
) -> O::Type {
    // See if we already fixed this type in a previous inference step.
    // If so, then we aren't allowed to change it unless [isLegacyCovariant] is
    // false.
    if let Some(type_from_previous_inference) = type_from_previous_inference
        && is_legacy_covariant
        && o.is_known_type(SharedTypeSchemaView::new(type_from_previous_inference))
    {
        return type_from_previous_inference;
    }

    merge_in_bound_constraints_if_enabled(
        o,
        constraints,
        type_parameter_to_infer,
        type_parameters_to_infer,
        data_for_testing,
        inference_using_bounds_is_enabled,
    );

    let mut constraint = constraints[&type_parameter_to_infer].clone();
    if let Some(extends_constraint) = extends_constraint {
        // Dart: `constraint = constraint.clone()` before the merge.
        constraint.merge_in_type_schema_upper(SharedTypeSchemaView::new(extends_constraint), o);
    }

    o.choose_type_from_constraint(&constraint, true, is_contravariant)
}

/// `TypeAnalyzerOperationsMixin.inferTypeParameterFromContext`: chooses
/// types from the current inference context in preliminary stages.
///
/// The Dart parameter `constraint` is `constraints[type_parameter_to_infer]`.
pub fn infer_type_parameter_from_context<O: TypeAnalyzerOperations>(
    o: &O,
    type_from_previous_inference: Option<O::Type>,
    extends_constraint: Option<O::Type>,
    is_contravariant: bool,
    is_legacy_covariant: bool,
    constraints: &mut ConstraintsMap<O>,
    type_parameters_to_infer: &[O::TypeParameter],
    type_parameter_to_infer: O::TypeParameter,
    data_for_testing: Option<&mut DataForTestingOf<O>>,
    inference_using_bounds_is_enabled: bool,
) -> O::Type {
    // See if we already fixed this type in a previous inference step.
    // If so, then we aren't allowed to change it unless [isLegacyCovariant] is
    // false.
    if let Some(type_from_previous_inference) = type_from_previous_inference
        && is_legacy_covariant
        && o.is_known_type(SharedTypeSchemaView::new(type_from_previous_inference))
    {
        return type_from_previous_inference;
    }

    let t = o.choose_type_from_constraint(
        &constraints[&type_parameter_to_infer],
        false,
        is_contravariant,
    );
    if !o.is_known_type(SharedTypeSchemaView::new(t)) {
        return t;
    }

    // If we're about to make our final choice, apply the extends clause.
    // This gives us a chance to refine the choice, in case it would violate
    // the `extends` clause. For example:
    //
    //     Object obj = math.min/*<infer Object, error>*/(1, 2);
    //
    // If we consider the `T extends num` we conclude `<num>`, which works.

    merge_in_bound_constraints_if_enabled(
        o,
        constraints,
        type_parameter_to_infer,
        type_parameters_to_infer,
        data_for_testing,
        inference_using_bounds_is_enabled,
    );

    if let Some(extends_constraint) = extends_constraint {
        // Dart: `constraint = constraint.clone()` before the merge.
        let mut constraint = constraints[&type_parameter_to_infer].clone();
        constraint.merge_in_type_schema_upper(SharedTypeSchemaView::new(extends_constraint), o);
        return o.choose_type_from_constraint(&constraint, false, false);
    }

    t
}

/// `TypeAnalyzerOperationsMixin.mergeInConstraintsFromBound`: generates the
/// constraints implied by `lower <: bound of type_parameter_to_infer`, merges
/// them into `inference_phase_constraints`, and returns the constraint for
/// `type_parameter_to_infer`.
pub fn merge_in_constraints_from_bound<O: TypeAnalyzerOperations>(
    o: &O,
    type_parameter_to_infer: O::TypeParameter,
    type_parameters_to_infer: &[SharedTypeParameterView<O::TypeParameter>],
    lower: O::Type,
    inference_phase_constraints: &mut ConstraintsMap<O>,
    data_for_testing: Option<&mut DataForTestingOf<O>>,
    inference_using_bounds_is_enabled: bool,
) -> MergedTypeConstraint<O::Type> {
    // The type parameter's bound may refer to itself (or other type
    // parameters), so we might have to create an additional constraint.
    // Consider this example from
    // https://github.com/dart-lang/language/issues/3009:
    //
    //     class A<X extends A<X>> {}
    //     class B extends A<B> {}
    //     class C extends B {}
    //     void f<X extends A<X>>(X x) {}
    //     void main() {
    //       f(C()); // should infer f<B>(C()).
    //     }
    //
    // In order for `f(C())` to be inferred as `f<B>(C())`, we need to
    // generate the constraint `X <: B`. To do this, we first take the lower
    // constraint we've accumulated so far (which, in this example, is `C`,
    // due to the presence of the actual argument `C()`), and use subtype
    // constraint generation to match it against the explicit bound (which
    // is `A<X>`; hence we perform `C <# A<X>`). If this produces any
    // constraints (i.e. `X <: B` in this example), then they are added to
    // the set of constraints just before choosing the final type.

    let type_parameter_to_infer_bound = o
        .bound_shared(type_parameter_to_infer)
        .expect("mergeInConstraintsFromBound: the type parameter has a bound");

    // TODO(cstefantsova): Pass [dataForTesting] when
    // [InferenceDataForTesting] is merged with [TypeInferenceResultForTesting].
    let _ = data_for_testing;
    let mut constraints_per_type_variable = {
        let mut type_constraint_gatherer = o.create_type_constraint_generator(
            None,
            type_parameters_to_infer,
            inference_using_bounds_is_enabled,
        );
        type_constraint_gatherer.perform_subtype_constraint_generation_internal(
            lower,
            type_parameter_to_infer_bound,
            true,
            None,
        );
        type_constraint_gatherer.compute_constraints()
    };
    for (type_parameter, constraint) in constraints_per_type_variable.iter_mut() {
        constraint.origin = TypeConstraintOrigin::FromExtendsClause {
            type_parameter_name: o.display_name(type_parameter_to_infer),
            bound_type: SharedTypeView::new(type_parameter_to_infer_bound),
            extends_type: SharedTypeView::new(type_parameter_to_infer_bound),
        };
        if !constraint.is_empty(o) {
            match inference_phase_constraints.get_mut(type_parameter) {
                None => {
                    inference_phase_constraints.insert(*type_parameter, constraint.clone());
                }
                Some(constraint_for_parameter) => {
                    constraint_for_parameter.merge_in_type_schema_upper(constraint.upper, o);
                    constraint_for_parameter.merge_in_type_schema_lower(constraint.lower, o);
                }
            }
        }
    }
    constraints_per_type_variable
        .get(&type_parameter_to_infer)
        .expect("mergeInConstraintsFromBound: constraint for the type parameter to infer")
        .clone()
}

// ========================================================= TypeConstraintGenerator

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForFunctionTypes`:
/// matches `p` against `q` when both are function types.
///
/// If `p` and `q` are both function types, and `p` is a subtype of `q` under
/// some constraints, the constraints making the relation possible are
/// recorded, and `true` is returned. Otherwise, the constraint state is
/// unchanged (or rolled back using `restoreState`), and `false` is returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_function_types<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    if ops.shared_type_kind(p) == SharedTypeKind::Function
        && ops.shared_type_kind(q) == SharedTypeKind::Function
    {
        if ops.type_parameters_shared(p).is_empty() && ops.type_parameters_shared(q).is_empty() {
            return handle_non_generic_function_types(g, p, q, left_schema, ast_node_for_testing);
        } else {
            return handle_generic_function_types(g, p, q, left_schema, ast_node_for_testing);
        }
    }

    false
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForLeftFutureOr`:
/// matches `p` against `q`.
///
/// If `p` is of the form `FutureOr<p0>` for some `p0`, and `p` is a subtype
/// of `q` under some constraints, the constraints making the relation
/// possible are recorded, and `true` is returned. Otherwise, the constraint
/// state is unchanged (or rolled back using `restoreState`), and `false` is
/// returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_left_future_or<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    // If `P` is `FutureOr<P0>` the match holds under constraint set `C1 + C2`:
    let ops = g.type_analyzer_operations();
    if let Some(p0) = ops.match_future_or_internal(p)
        && !ops.is_question_type(p)
    {
        let state = g.current_state();

        // If `Future<P0>` is a subtype match for `Q` under constraint set `C1`.
        // And if `P0` is a subtype match for `Q` under constraint set `C2`.
        let future_p0 = g.type_analyzer_operations().future_type_internal(p0);
        if g.perform_subtype_constraint_generation_internal(
            future_p0,
            q,
            left_schema,
            ast_node_for_testing,
        ) && g.perform_subtype_constraint_generation_internal(
            p0,
            q,
            left_schema,
            ast_node_for_testing,
        ) {
            return true;
        }

        g.restore_state(state);
    }

    false
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForLeftNullableType`:
/// matches `p` against `q` as a subtype against supertype.
///
/// - If `p` is `p0?` for some `p0` and `p` is a subtype of `q` under some
///   constraints, the constraints making the relation possible are recorded,
///   and `true` is returned.
/// - Otherwise, the constraint state is unchanged (or rolled back using
///   `restoreState`), and `false` is returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_left_nullable_type<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    // If `P` is `P0?` the match holds under constraint set `C1 + C2`:
    let ops = g.type_analyzer_operations();
    if ops.is_question_type(p) {
        let p0 = ops.as_question_type(p, false);
        let null_type = ops.null_type().unwrap_type_view();
        let state = g.current_state();

        // If `P0` is a subtype match for `Q` under constraint set `C1`.
        // And if `Null` is a subtype match for `Q` under constraint set `C2`.
        if g.perform_subtype_constraint_generation_internal(
            p0,
            q,
            left_schema,
            ast_node_for_testing,
        ) && g.perform_subtype_constraint_generation_internal(
            null_type,
            q,
            left_schema,
            ast_node_for_testing,
        ) {
            return true;
        }

        g.restore_state(state);
    }

    false
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForRecordTypes`:
/// matches `p` against `q`, where `p` and `q` are both record types.
///
/// If `p` is a subtype of `q` under some constraints, the constraints making
/// the relation possible are recorded, and `true` is returned. Otherwise,
/// the constraint state is unchanged (or rolled back), and `false` is
/// returned.
pub fn perform_subtype_constraint_generation_for_record_types<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    if ops.shared_type_kind(p) != SharedTypeKind::Record
        || ops.shared_type_kind(q) != SharedTypeKind::Record
    {
        return false;
    }

    // A record type `(M0,..., Mk, {M{k+1} d{k+1}, ..., Mm dm])` is a subtype
    // match for a record type `(N0,..., Nk, {N{k+1} d{k+1}, ..., Nm dm])`
    // with respect to `L` under constraints `C0 + ... + Cm`
    // If for `i` in `0...m`, `Mi` is a subtype match for `Ni` with respect
    // to `L` under constraints `Ci`.
    let p_positional = ops.positional_types_shared(p);
    let q_positional = ops.positional_types_shared(q);
    let p_named = ops.sorted_named_types_shared(p);
    let q_named = ops.sorted_named_types_shared(q);
    if p_positional.len() != q_positional.len() || p_named.len() != q_named.len() {
        return false;
    }

    let state = g.current_state();

    for i in 0..p_positional.len() {
        if !g.perform_subtype_constraint_generation_internal(
            p_positional[i],
            q_positional[i],
            left_schema,
            ast_node_for_testing,
        ) {
            g.restore_state(state);
            return false;
        }
    }

    // Since record types don't allow optional positional or named
    // parameters, and the named parameters are sorted, it's sufficient to
    // check that the named parameters at the same index have the same name
    // and matching types.
    for i in 0..p_named.len() {
        if p_named[i].name_shared != q_named[i].name_shared
            || !g.perform_subtype_constraint_generation_internal(
                p_named[i].type_shared,
                q_named[i].type_shared,
                left_schema,
                ast_node_for_testing,
            )
        {
            g.restore_state(state);
            return false;
        }
    }

    true
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForRightFutureOr`:
/// matches `p` against `q`.
///
/// If `q` is of the form `FutureOr<q0>` for some `q0`, and `p` is a subtype
/// of `q` under some constraints, the constraints making the relation
/// possible are recorded, and `true` is returned. Otherwise, the constraint
/// state is unchanged (or rolled back using `restoreState`), and `false` is
/// returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_right_future_or<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    // If `Q` is `FutureOr<Q0>` the match holds under constraint set `C`:
    let oblivious = g.enable_discrepant_obliviousness_of_nullability_suffix_of_future_or();
    let ops = g.type_analyzer_operations();
    if let Some(q0) = ops.match_future_or_internal(q)
        && (oblivious || !ops.is_question_type(q))
    {
        let p_future_or = ops.match_future_or_internal(p);
        let p_is_question_type = ops.is_question_type(p);
        let future_q0 = ops.future_type_internal(q0);
        let state = g.current_state();

        // If `P` is `FutureOr<P0>` and `P0` is a subtype match for `Q0` under
        // constraint set `C`.
        if let Some(p0) = p_future_or
            && (oblivious || !p_is_question_type)
            && g.perform_subtype_constraint_generation_internal(
                p0,
                q0,
                left_schema,
                ast_node_for_testing,
            )
        {
            return true;
        }

        // Or if `P` is a subtype match for `Future<Q0>` under non-empty
        // constraint set `C`.
        let is_match_with_future = g.perform_subtype_constraint_generation_internal(
            p,
            future_q0,
            left_schema,
            ast_node_for_testing,
        );
        let match_with_future_adds_constraints = g.current_state() != state;
        if is_match_with_future && match_with_future_adds_constraints {
            return true;
        }

        // Or if `P` is a subtype match for `Q0` under constraint set `C`.
        if g.perform_subtype_constraint_generation_internal(
            p,
            q0,
            left_schema,
            ast_node_for_testing,
        ) {
            return true;
        }

        // Or if `P` is a subtype match for `Future<Q0>` under empty
        // constraint set `C`.
        if is_match_with_future && !match_with_future_adds_constraints {
            return true;
        }
    }

    false
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForRightNullableType`:
/// matches `p` against `q` as a subtype against supertype.
///
/// - If `q` is `q0?` for some `q0` and `p` is a subtype of `q` under some
///   constraints, the constraints making the relation possible are recorded,
///   and `true` is returned.
/// - Otherwise, the constraint state is unchanged (or rolled back using
///   `restoreState`), and `false` is returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_right_nullable_type<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    // If `Q` is `Q0?` the match holds under constraint set `C`:
    let ops = g.type_analyzer_operations();
    if ops.is_question_type(q) {
        let q0 = ops.as_question_type(q, false);
        let p_is_question_type = ops.is_question_type(p);
        let p0 = ops.as_question_type(p, false);
        let p_kind = ops.shared_type_kind(p);
        let object_type = ops.object_type().unwrap_type_view();
        let null_type = ops.null_type().unwrap_type_view();
        let state = g.current_state();

        // If `P` is `P0?` and `P0` is a subtype match for `Q0` under
        // constraint set `C`.
        if p_is_question_type
            && g.perform_subtype_constraint_generation_internal(
                p0,
                q0,
                left_schema,
                ast_node_for_testing,
            )
        {
            return true;
        }

        // Or if `P` is `dynamic` or `void` and `Object` is a subtype match
        // for `Q0` under constraint set `C`.
        if (p_kind == SharedTypeKind::Dynamic || p_kind == SharedTypeKind::Void)
            && g.perform_subtype_constraint_generation_internal(
                object_type,
                q0,
                left_schema,
                ast_node_for_testing,
            )
        {
            return true;
        }

        // Or if `P` is a subtype match for `Q0` under non-empty
        // constraint set `C`.
        let p_matches_q0 = g.perform_subtype_constraint_generation_internal(
            p,
            q0,
            left_schema,
            ast_node_for_testing,
        );
        if p_matches_q0 && state != g.current_state() {
            return true;
        }

        // Or if `P` is a subtype match for `Null` under constraint set `C`.
        if g.perform_subtype_constraint_generation_internal(
            p,
            null_type,
            left_schema,
            ast_node_for_testing,
        ) {
            return true;
        }

        // Or if `P` is a subtype match for `Q0` under empty
        // constraint set `C`.
        if p_matches_q0 {
            return true;
        }
    }

    false
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationForTypeDeclarationTypes`:
/// matches `p` against `q` as a subtype against supertype.
///
/// If `p` and `q` are both type declaration types, then:
///
/// - If `p` is a subtype of `q` under some constraints, the constraints
///   making the relation possible are recorded, and `true` is returned.
/// - Otherwise, the constraint state is unchanged (or rolled back using
///   `restoreState`), and `false` is returned.
///
/// Otherwise (either `p` or `q` is not a type declaration type), the
/// constraint state is unchanged, and `None` is returned.
///
/// An invariant of the type inference is that only `p` or `q` may be a
/// schema (in other words, may contain the unknown type `_`); the other must
/// be simply a type. If `left_schema` is `true`, `p` may contain `_`; if it
/// is `false`, `q` may contain `_`.
pub fn perform_subtype_constraint_generation_for_type_declaration_types<
    G: TypeConstraintGenerator,
>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> Option<bool> {
    let ops = g.type_analyzer_operations();
    match (
        ops.match_type_declaration_type_internal(p),
        ops.match_type_declaration_type_internal(q),
    ) {
        // If `P` is `C<M0, ..., Mk> and `Q` is `C<N0, ..., Nk>`, then the match
        // holds under constraints `C0 + ... + Ck`:
        //   If `Mi` is a subtype match for `Ni` with respect to L under
        //   constraints `Ci`.
        (Some(p_matched), Some(q_matched))
            if p_matched.type_declaration_kind == q_matched.type_declaration_kind
                && p_matched.type_declaration == q_matched.type_declaration =>
        {
            Some(interface_type_arguments(
                g,
                p_matched.type_declaration,
                &p_matched.type_arguments,
                &q_matched.type_arguments,
                left_schema,
                ast_node_for_testing,
            ))
        }

        (Some(_), Some(_)) => Some(interface_types(g, p, q, left_schema, ast_node_for_testing)),

        (p_matched, q_matched) => {
            debug_assert!(p_matched.is_none() || q_matched.is_none());
            None
        }
    }
}

/// `TypeConstraintGenerator.performSubtypeConstraintGenerationInternal`:
/// the implementation backing `performSubtypeConstraintGenerationLeftSchema`
/// and `performSubtypeConstraintGenerationRightSchema`.
///
/// If `p` is a subtype of `q` under some constraints, the constraints making
/// the relation possible are recorded, and `true` is returned. Otherwise,
/// the constraint state is unchanged (or rolled back using `restoreState`),
/// and `false` is returned.
pub fn perform_subtype_constraint_generation_internal<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    let p_kind = ops.shared_type_kind(p);
    let q_kind = ops.shared_type_kind(q);

    // If `P` is `_` then the match holds with no constraints.
    if p_kind == SharedTypeKind::Unknown {
        return true;
    }

    // If `Q` is `_` then the match holds with no constraints.
    if q_kind == SharedTypeKind::Unknown {
        return true;
    }

    // If `P` is a structural context schema, the match holds with no
    // constraints.
    if is_structural_context_schema(p_kind) {
        return true;
    }

    // If `P` is a type variable `X` in `L`, then the match holds:
    //   Under constraint `_ <: X <: Q`.
    if let Some(p_parameter) = ops.match_inferable_parameter_internal(p)
        && !ops.is_question_type(p)
        && g.type_parameters_to_constrain().contains(&p_parameter)
    {
        g.add_upper_constraint_for_parameter(p_parameter, q, ast_node_for_testing);
        return true;
    }

    // If `Q` is a type variable `X` in `L`, then the match holds:
    //   Under constraint `P <: X <: _`.
    let ops = g.type_analyzer_operations();
    if let Some(q_parameter) = ops.match_inferable_parameter_internal(q) {
        let type_parameters_to_constrain = g.type_parameters_to_constrain();
        if !ops.is_question_type(q)
            && type_parameters_to_constrain.contains(&q_parameter)
            && (!g.inference_using_bounds_is_enabled()
                || match ops.bound_shared(q_parameter) {
                    None => true,
                    Some(bound) => ops.is_subtype_of_internal(
                        p,
                        ops.greatest_closure_of_type_internal(bound, &type_parameters_to_constrain),
                    ),
                })
        {
            g.add_lower_constraint_for_parameter(q_parameter, p, ast_node_for_testing);
            return true;
        }
    }

    // If `P` and `Q` are identical types, then the subtype match holds
    // under no constraints.
    if p == q {
        return true;
    }

    let ops = g.type_analyzer_operations();
    if is_structural_context_schema(q_kind) {
        // At this point, `P` can't be a structural context schema, and `Q` is a
        // structural context schema.
        debug_assert!(!is_structural_context_schema(p_kind));

        match q_kind {
            SharedTypeKind::InvocationStructuralContextSchema => {
                let return_type = ops.invocation_structural_context_schema_return_type(q);
                if p_kind != SharedTypeKind::Function {
                    // If `P` is not a function type, the match doesn't hold.
                    return false;
                } else {
                    // Otherwise, the match holds under constraints `C` if `P1` is a
                    // subtype match for `Q1` with respect to `L` under constraint set
                    // `C1`, where `P1` is the return type of the function type `P`,
                    // and `C` is constructed from `C1` by taking all of the
                    // constraints from `C1` and marking them secondary.
                    let p_return_type = ops.return_type_shared(p);
                    let result = g.perform_subtype_constraint_generation_internal(
                        p_return_type,
                        return_type,
                        left_schema,
                        ast_node_for_testing,
                    );
                    return result;
                }
            }
            _ => {
                let lookup_name = ops.lookup_structural_context_schema_lookup_name(q);
                let lookup_type = ops.lookup_structural_context_schema_lookup_type(q);
                match ops.lookup_member_type_internal(p, lookup_name) {
                    // If the interface of `P` doesn't contain a member with name `A`,
                    // the match doesn't hold.
                    None => return false,
                    // Otherwise, the match holds under constraints `C` if `P1` is a
                    // subtype match for `Q1` with respect to `L` under constraint set
                    // `C1`, where `P1` is the type of the member with the name `A`
                    // looked up in the interface of `P`, and `C` is constructed from
                    // `C1` by taking all of the constraints from `C1` and marking them
                    // secondary.
                    Some(member_type) => {
                        return g.perform_subtype_constraint_generation_internal(
                            member_type,
                            lookup_type,
                            left_schema,
                            ast_node_for_testing,
                        );
                    }
                }
            }
        }
    }

    // Note that it's not necessary to rewind [_constraints] to its prior state
    // in case [performSubtypeConstraintGenerationForFutureOr] returns false, as
    // [performSubtypeConstraintGenerationForFutureOr] handles the rewinding of
    // the state itself.
    if g.perform_subtype_constraint_generation_for_right_future_or(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    if g.perform_subtype_constraint_generation_for_right_nullable_type(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    // If `P` is `FutureOr<P0>` the match holds under constraint set `C1 + C2`:
    if g.perform_subtype_constraint_generation_for_left_future_or(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    // If `P` is `P0?` the match holds under constraint set `C1 + C2`:
    if g.perform_subtype_constraint_generation_for_left_nullable_type(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    // If `Q` is `dynamic`, `Object?`, or `void` then the match holds under
    // no constraints.
    let ops = g.type_analyzer_operations();
    if q_kind == SharedTypeKind::Dynamic
        || q_kind == SharedTypeKind::Void
        || q == ops.object_question_type().unwrap_type_view()
    {
        return true;
    }

    // If `P` is `Never` then the match holds under no constraints.
    if ops.is_bottom_type(SharedTypeView::new(p)) {
        return true;
    }

    // If `Q` is `Object`, then the match holds under no constraints:
    //  Only if `P` is non-nullable.
    if q == ops.object_type().unwrap_type_view() {
        return ops.is_non_nullable_internal(p);
    }

    // If `P` is `Null`, then the match holds under no constraints:
    //  Only if `Q` is nullable.
    if p_kind == SharedTypeKind::Null {
        return ops.is_nullable_internal(q);
    }

    // If `P` is a type variable `X` with bound `B` (or a promoted type
    // variable `X & B`), the match holds with constraint set `C`:
    //   If `B` is a subtype match for `Q` with constraint set `C`.
    // Note: we have already eliminated the case that `X` is a variable in `L`.
    if let Some(bound) = ops.match_type_parameter_bound_internal(p)
        && g.perform_subtype_constraint_generation_internal(
            bound,
            q,
            left_schema,
            ast_node_for_testing,
        )
    {
        return true;
    }

    let result = g.perform_subtype_constraint_generation_for_type_declaration_types(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    );
    if let Some(result) = result {
        return result;
    }

    // If `Q` is `Function` then the match holds under no constraints:
    //   If `P` is a function type.
    let ops = g.type_analyzer_operations();
    if ops.is_dart_core_function_internal(q) && p_kind == SharedTypeKind::Function {
        return true;
    }

    if g.perform_subtype_constraint_generation_for_function_types(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    // A type `P` is a subtype match for `Record` with respect to `L` under no
    // constraints:
    //   If `P` is a record type or `Record`.
    let ops = g.type_analyzer_operations();
    if ops.is_dart_core_record_internal(q) && p_kind == SharedTypeKind::Record {
        return true;
    }

    if g.perform_subtype_constraint_generation_for_record_types(
        p,
        q,
        left_schema,
        ast_node_for_testing,
    ) {
        return true;
    }

    false
}

/// Dart `is SharedStructuralContextSchema`.
fn is_structural_context_schema(kind: SharedTypeKind) -> bool {
    matches!(
        kind,
        SharedTypeKind::InvocationStructuralContextSchema
            | SharedTypeKind::LookupStructuralContextSchema
    )
}

/// `TypeConstraintGenerator._handleGenericFunctionTypes`: matches generic
/// function type `p` against generic function type `q`.
///
/// See the documentation on
/// [`perform_subtype_constraint_generation_for_function_types`] for details.
fn handle_generic_function_types<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    let p_type_parameters = ops.type_parameters_shared(p);
    let q_type_parameters = ops.type_parameters_shared(q);
    debug_assert!(!p_type_parameters.is_empty() || !q_type_parameters.is_empty());
    // A generic function type <T0 extends B00, ..., Tn extends B0n>F0 is a
    // subtype match for a generic function type <S0 extends B10, ..., Sn
    // extends B1n>F1 with respect to L under constraint set C2
    //
    // If B0i is a subtype match for B1i with constraint set Ci0.  And B1i
    // is a subtype match for B0i with constraint set Ci1.  And Ci2 is Ci0
    // + Ci1.
    //
    // And Z0...Zn are fresh variables with bounds B20, ..., B2n, Where B2i
    // is B0i[Z0/T0, ..., Zn/Tn] if P is a type schema.  Or B2i is
    // B1i[Z0/S0, ..., Zn/Sn] if Q is a type schema.  In other words, we
    // choose the bounds for the fresh variables from whichever of the two
    // generic function types is a type schema and does not contain any
    // variables from L.
    //
    // And F0[Z0/T0, ..., Zn/Tn] is a subtype match for F1[Z0/S0, ...,
    // Zn/Sn] with respect to L under constraints C0.  And C1 is C02 + ...
    // + Cn2 + C0.  And C2 is C1 with each constraint replaced with its
    // closure with respect to [Z0, ..., Zn].
    if p_type_parameters.len() == q_type_parameters.len() {
        let state = g.current_state();

        let mut is_match = true;
        let mut i = 0;
        while is_match && i < p_type_parameters.len() {
            let ops = g.type_analyzer_operations();
            let object_question_type = ops.object_question_type().unwrap_type_view();
            let p_bound = ops
                .bound_shared(p_type_parameters[i])
                .unwrap_or(object_question_type);
            let q_bound = ops
                .bound_shared(q_type_parameters[i])
                .unwrap_or(object_question_type);
            is_match = is_match
                && g.perform_subtype_constraint_generation_internal(
                    p_bound,
                    q_bound,
                    left_schema,
                    ast_node_for_testing,
                )
                && g.perform_subtype_constraint_generation_internal(
                    q_bound,
                    p_bound,
                    !left_schema,
                    ast_node_for_testing,
                );
            i += 1;
        }
        if is_match {
            let (instantiated_p, instantiated_q, type_parameters_to_eliminate) =
                g.instantiate_function_types_and_provide_fresh_type_parameters(p, q, left_schema);

            if g.perform_subtype_constraint_generation_internal(
                instantiated_p,
                instantiated_q,
                left_schema,
                ast_node_for_testing,
            ) {
                g.eliminate_type_parameters_in_generated_constraints(
                    &type_parameters_to_eliminate,
                    state,
                    ast_node_for_testing,
                );
                return true;
            }
        }
        g.restore_state(state);
    }

    false
}

/// `TypeConstraintGenerator._handleNonGenericFunctionTypes`: matches
/// non-generic function type `p` against non-generic function type `q`.
///
/// See the documentation on
/// [`perform_subtype_constraint_generation_for_function_types`] for details.
fn handle_non_generic_function_types<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    debug_assert!(
        ops.type_parameters_shared(p).is_empty() && ops.type_parameters_shared(q).is_empty()
    );
    let p_named = ops.sorted_named_parameters_shared(p);
    let q_named = ops.sorted_named_parameters_shared(q);
    let p_positional = ops.positional_parameter_types_shared(p);
    let q_positional = ops.positional_parameter_types_shared(q);
    let p_required_count = ops.required_positional_parameter_count(p);
    let q_required_count = ops.required_positional_parameter_count(q);
    let p_return_type = ops.return_type_shared(p);
    let q_return_type = ops.return_type_shared(q);
    // A function type (M0,..., Mn, [M{n+1}, ..., Mm]) -> R0 is a subtype
    // match for a function type (N0,..., Nk, [N{k+1}, ..., Nr]) -> R1 with
    // respect to L under constraints C0 + ... + Cr + C
    //
    // If R0 is a subtype match for a type R1 with respect to L under
    // constraints C.  If n <= k and r <= m.  And for i in 0...r, Ni is a
    // subtype match for Mi with respect to L under constraints Ci.
    if p_named.is_empty()
        && q_named.is_empty()
        && p_required_count <= q_required_count
        && p_positional.len() >= q_positional.len()
    {
        let state = g.current_state();

        if !g.perform_subtype_constraint_generation_internal(
            p_return_type,
            q_return_type,
            left_schema,
            ast_node_for_testing,
        ) {
            return false;
        }
        for i in 0..q_positional.len() {
            if !g.perform_subtype_constraint_generation_internal(
                q_positional[i],
                p_positional[i],
                !left_schema,
                ast_node_for_testing,
            ) {
                g.restore_state(state);
                return false;
            }
        }
        return true;
    } else if p_positional.len() == p_required_count
        && q_positional.len() == q_required_count
        && p_required_count == q_required_count
        && !p_named.is_empty()
        && q_named.len() <= p_named.len()
    {
        // Function types with named parameters are treated analogously to the
        // positional parameter case above.

        let state = g.current_state();

        if !g.perform_subtype_constraint_generation_internal(
            p_return_type,
            q_return_type,
            left_schema,
            ast_node_for_testing,
        ) {
            return false;
        }
        for i in 0..p_positional.len() {
            if !g.perform_subtype_constraint_generation_internal(
                q_positional[i],
                p_positional[i],
                !left_schema,
                ast_node_for_testing,
            ) {
                g.restore_state(state);
                return false;
            }
        }
        // Consume parameter names from p and q in order. Since the named
        // parameters in p and q are already sorted by name, we can do this by
        // iterating through both lists in tandem.
        let mut i = 0;
        let mut j = 0;
        loop {
            // Determine whether the next parameter should be consumed from p,
            // q, or both (because the next set of names matches). If the next
            // parameter should be consumed from p, comparisonResult will be set
            // to a value < 0. If the next parameter should be consumed from q,
            // comparisonResult will be set to a value > 0. If the next
            // parameter should be consumed from both, comparisonResult will be
            // set to 0.
            let comparison_result = if i >= p_named.len() {
                if j >= q_named.len() {
                    // No parameters left.
                    return true;
                } else {
                    // No more parameters in p, so the next parameter must come from
                    // q.
                    Ordering::Greater
                }
            } else if j >= q_named.len() {
                // No more parameters in q, so the next parameter must come from
                // p.
                Ordering::Less
            } else {
                g.type_analyzer_operations()
                    .compare_names(p_named[i].name_shared, q_named[j].name_shared)
            };
            match comparison_result {
                Ordering::Greater => {
                    // Extra parameter in q that q that doesn't exist in p. No match.
                    g.restore_state(state);
                    return false;
                }
                Ordering::Less => {
                    // Extra parameter in p that doesn't exist in q. Ok if not
                    // required.
                    if p_named[i].is_required {
                        g.restore_state(state);
                        return false;
                    } else {
                        i += 1;
                    }
                }
                Ordering::Equal => {
                    // The next parameter in p and q matches, so match their types.
                    if !g.perform_subtype_constraint_generation_internal(
                        q_named[j].type_shared,
                        p_named[i].type_shared,
                        !left_schema,
                        ast_node_for_testing,
                    ) {
                        g.restore_state(state);
                        return false;
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
    }

    false
}

/// `TypeConstraintGenerator._interfaceTypeArguments`: match arguments
/// `p_type_arguments` of P against arguments `q_type_arguments` of Q, taking
/// into account the variance of type variables in `declaration`. If returns
/// `false`, the constraints are unchanged.
fn interface_type_arguments<G: TypeConstraintGenerator>(
    g: &mut G,
    declaration: <G::Operations as TypeAnalyzerOperations>::TypeDeclaration,
    p_type_arguments: &[GType<G>],
    q_type_arguments: &[GType<G>],
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    debug_assert_eq!(p_type_arguments.len(), q_type_arguments.len());

    let state = g.current_state();

    for i in 0..p_type_arguments.len() {
        let variance = g
            .type_analyzer_operations()
            .get_type_parameter_variance(declaration, i);
        #[allow(non_snake_case)]
        let M = p_type_arguments[i];
        #[allow(non_snake_case)]
        let N = q_type_arguments[i];
        if (variance == Variance::Covariant || variance == Variance::Invariant)
            && !g.perform_subtype_constraint_generation_internal(
                M,
                N,
                left_schema,
                ast_node_for_testing,
            )
        {
            g.restore_state(state);
            return false;
        }
        if (variance == Variance::Contravariant || variance == Variance::Invariant)
            && !g.perform_subtype_constraint_generation_internal(
                N,
                M,
                !left_schema,
                ast_node_for_testing,
            )
        {
            g.restore_state(state);
            return false;
        }
    }

    true
}

/// `TypeConstraintGenerator._interfaceTypes`: matches `p` against `q`,
/// assuming both `p` and `q` are both type declaration types that refer to
/// different type declarations.
///
/// If `p` is a subtype of `q` under some constraints, the constraints making
/// the relation possible are recorded, and `true` is returned. Otherwise,
/// the constraint state is unchanged (or rolled back using `restoreState`),
/// and `false` is returned.
fn interface_types<G: TypeConstraintGenerator>(
    g: &mut G,
    p: GType<G>,
    q: GType<G>,
    left_schema: bool,
    ast_node_for_testing: Option<AstNodeOf<G>>,
) -> bool {
    let ops = g.type_analyzer_operations();
    if ops.is_question_type(p) {
        return false;
    }

    if ops.is_question_type(q) {
        return false;
    }

    // If `P` is `C0<M0, ..., Mk>` and `Q` is `C1<N0, ..., Nj>` then the match
    // holds with respect to `L` under constraints `C`:
    //   If `C1<B0, ..., Bj>` is a superinterface of `C0<M0, ..., Mk>` and
    //   `C1<B0, ..., Bj>` is a subtype match for `C1<N0, ..., Nj>` with
    //   respect to `L` under constraints `C`.

    if let (Some(p_matched), Some(q_matched)) = (
        ops.match_type_declaration_type_internal(p),
        ops.match_type_declaration_type_internal(q),
    ) && let Some(type_arguments) = g.get_type_arguments_as_instance_of(
        p_matched.type_declaration_type,
        q_matched.type_declaration,
    ) {
        return interface_type_arguments(
            g,
            q_matched.type_declaration,
            &type_arguments,
            &q_matched.type_arguments,
            left_schema,
            ast_node_for_testing,
        );
    }

    false
}

// ========================================================================= macros

/// Applies `TypeAnalyzerOperationsMixin`: overrides the `todo!()` provided
/// methods of
/// [`TypeAnalyzerOperations`](dartr_flow::type_analyzer_operations::TypeAnalyzerOperations)
/// (`choose_types`, `infer_type_parameter_from_all`,
/// `infer_type_parameter_from_context`, `merge_in_constraints_from_bound`)
/// with calls to the functions of this module.
///
/// Invoke it inside `impl TypeAnalyzerOperations for X { ... }`.
#[macro_export]
macro_rules! type_analyzer_operations_mixin {
    () => {
        fn choose_types(
            &self,
            type_parameters_to_infer: &[Self::TypeParameter],
            constraints: &mut $crate::__dartr_flow::type_analyzer_operations::ConstraintsMap<Self>,
            previously_inferred_types: Option<&[Self::Type]>,
            preliminary: bool,
            inference_using_bounds_is_enabled: bool,
            data_for_testing: Option<
                &mut $crate::__dartr_flow::type_analyzer_operations::DataForTestingOf<Self>,
            >,
            tree_node_for_testing: Option<Self::AstNode>,
        ) -> Vec<Self::Type> {
            $crate::type_analyzer_operations::choose_types(
                self,
                type_parameters_to_infer,
                constraints,
                previously_inferred_types,
                preliminary,
                inference_using_bounds_is_enabled,
                data_for_testing,
                tree_node_for_testing,
            )
        }

        fn infer_type_parameter_from_all(
            &self,
            type_from_previous_inference: Option<Self::Type>,
            extends_constraint: Option<Self::Type>,
            is_contravariant: bool,
            is_legacy_covariant: bool,
            constraints: &mut $crate::__dartr_flow::type_analyzer_operations::ConstraintsMap<Self>,
            type_parameter_to_infer: Self::TypeParameter,
            type_parameters_to_infer: &[Self::TypeParameter],
            data_for_testing: Option<
                &mut $crate::__dartr_flow::type_analyzer_operations::DataForTestingOf<Self>,
            >,
            inference_using_bounds_is_enabled: bool,
        ) -> Self::Type {
            $crate::type_analyzer_operations::infer_type_parameter_from_all(
                self,
                type_from_previous_inference,
                extends_constraint,
                is_contravariant,
                is_legacy_covariant,
                constraints,
                type_parameter_to_infer,
                type_parameters_to_infer,
                data_for_testing,
                inference_using_bounds_is_enabled,
            )
        }

        fn infer_type_parameter_from_context(
            &self,
            type_from_previous_inference: Option<Self::Type>,
            extends_constraint: Option<Self::Type>,
            is_contravariant: bool,
            is_legacy_covariant: bool,
            constraints: &mut $crate::__dartr_flow::type_analyzer_operations::ConstraintsMap<Self>,
            type_parameters_to_infer: &[Self::TypeParameter],
            type_parameter_to_infer: Self::TypeParameter,
            data_for_testing: Option<
                &mut $crate::__dartr_flow::type_analyzer_operations::DataForTestingOf<Self>,
            >,
            inference_using_bounds_is_enabled: bool,
        ) -> Self::Type {
            $crate::type_analyzer_operations::infer_type_parameter_from_context(
                self,
                type_from_previous_inference,
                extends_constraint,
                is_contravariant,
                is_legacy_covariant,
                constraints,
                type_parameters_to_infer,
                type_parameter_to_infer,
                data_for_testing,
                inference_using_bounds_is_enabled,
            )
        }

        fn merge_in_constraints_from_bound(
            &self,
            type_parameter_to_infer: Self::TypeParameter,
            type_parameters_to_infer: &[$crate::__dartr_flow::shared_type::SharedTypeParameterView<
                Self::TypeParameter,
            >],
            lower: Self::Type,
            inference_phase_constraints: &mut $crate::__dartr_flow::type_analyzer_operations::ConstraintsMap<Self>,
            data_for_testing: Option<
                &mut $crate::__dartr_flow::type_analyzer_operations::DataForTestingOf<Self>,
            >,
            inference_using_bounds_is_enabled: bool,
        ) -> $crate::__dartr_flow::type_constraint::MergedTypeConstraint<Self::Type> {
            $crate::type_analyzer_operations::merge_in_constraints_from_bound(
                self,
                type_parameter_to_infer,
                type_parameters_to_infer,
                lower,
                inference_phase_constraints,
                data_for_testing,
                inference_using_bounds_is_enabled,
            )
        }
    };
}

/// Applies the shared constraint generation algorithm of
/// `TypeConstraintGenerator`: overrides the `todo!()` provided methods of
/// [`TypeConstraintGenerator`](dartr_flow::type_analyzer_operations::TypeConstraintGenerator)
/// (the eight `perform_subtype_constraint_generation_*` methods) with calls
/// to the functions of this module.
///
/// Invoke it inside `impl TypeConstraintGenerator for X { ... }`.
#[macro_export]
macro_rules! type_constraint_generator_mixin {
    () => {
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_function_types -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_left_future_or -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_left_nullable_type -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_record_types -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_right_future_or -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_right_nullable_type -> bool);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_for_type_declaration_types -> Option<bool>);
        $crate::type_constraint_generator_mixin!(@method perform_subtype_constraint_generation_internal -> bool);
    };
    (@method $name:ident -> $ret:ty) => {
        fn $name(
            &mut self,
            p: $crate::__dartr_flow::shared_type::TypeOf<Self::Operations>,
            q: $crate::__dartr_flow::shared_type::TypeOf<Self::Operations>,
            left_schema: bool,
            ast_node_for_testing: Option<
                <Self::Operations as $crate::__dartr_flow::type_analyzer_operations::TypeAnalyzerOperations>::AstNode,
            >,
        ) -> $ret {
            $crate::type_analyzer_operations::$name(self, p, q, left_schema, ast_node_for_testing)
        }
    };
}
