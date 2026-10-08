// Dart source: pkg/analyzer/lib/src/dart/element/type_constraint_gatherer.dart

//! [`TypeConstraintGatherer`]: creates sets of `GeneratedTypeConstraint`s for
//! type parameters, based on an attempt to make one type schema a subtype of
//! another.
//!
//! The matching algorithm is the shared `TypeConstraintGeneratorMixin`
//! (`dartr_type_analyzer::type_constraint_generator_mixin!`). This file has
//! the analyzer-specific members: the constraint list, its state, the
//! elimination of fresh type parameters, `getTypeArgumentsAsInstanceOf` and
//! the fresh type parameters of generic function types.
//!
//! The Dart typedefs (`GeneratedTypeConstraint`, `MergedTypeConstraint`,
//! `TypeConstraintOrigin`, ...) are the generic `dartr_flow::type_constraint`
//! types with `P = EId<TypeParameterElement>` and `T = TypeId`; see the
//! aliases below.

use dartr_ast::NodeId;
use dartr_element::{EId, InterfaceElement, Nullability, TypeId, TypeKind, TypeParameterElement};
use dartr_flow::shared_type::{SharedTypeParameterView, SharedTypeSchemaView};
use dartr_flow::type_analyzer_operations::{
    ConstraintsMap, TypeConstraintGenerator, TypeConstraintGeneratorState,
};
use dartr_flow::type_constraint;
use indexmap::IndexSet;

use crate::type_algebra::MapSubstitution;
use crate::type_ext::TypeExt;
use crate::type_system_operations::{
    TypeConstraintGenerationDataForTestingImpl, TypeSystemOperations,
};

/// `GeneratedTypeConstraint` of the analyzer.
pub type GeneratedTypeConstraint =
    type_constraint::GeneratedTypeConstraint<EId<TypeParameterElement>, TypeId>;

/// `MergedTypeConstraint` of the analyzer.
pub type MergedTypeConstraint = type_constraint::MergedTypeConstraint<TypeId>;

/// `TypeConstraintOrigin` of the analyzer (the subclasses are its variants).
pub type TypeConstraintOrigin = type_constraint::TypeConstraintOrigin<TypeId>;

/// `TypeConstraintGenerationDataForTesting` of the analyzer.
pub type TypeConstraintGenerationDataForTesting = TypeConstraintGenerationDataForTestingImpl;

/// `TypeConstraintGatherer`.
pub struct TypeConstraintGatherer<'b, 'a> {
    /// Dart: `Set.identity()` (iterated in insertion order).
    type_parameters_to_constrain: IndexSet<EId<TypeParameterElement>>,
    constraints: Vec<GeneratedTypeConstraint>,
    type_system_operations: &'b TypeSystemOperations<'a>,
    pub data_for_testing: Option<&'b mut TypeConstraintGenerationDataForTesting>,
    inference_using_bounds_is_enabled: bool,
}

impl<'b, 'a> TypeConstraintGatherer<'b, 'a> {
    /// `TypeConstraintGatherer(typeParameters:, typeSystemOperations:,
    /// inferenceUsingBoundsIsEnabled:, dataForTesting:)`.
    pub fn new(
        type_parameters: &[EId<TypeParameterElement>],
        type_system_operations: &'b TypeSystemOperations<'a>,
        inference_using_bounds_is_enabled: bool,
        data_for_testing: Option<&'b mut TypeConstraintGenerationDataForTesting>,
    ) -> TypeConstraintGatherer<'b, 'a> {
        TypeConstraintGatherer {
            type_parameters_to_constrain: type_parameters.iter().copied().collect(),
            constraints: Vec::new(),
            type_system_operations,
            data_for_testing,
            inference_using_bounds_is_enabled,
        }
    }

    /// `isConstraintSetEmpty`.
    pub fn is_constraint_set_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    fn record_for_testing(
        &mut self,
        constraint: GeneratedTypeConstraint,
        ast_node_for_testing: Option<NodeId>,
    ) {
        if let (Some(data), Some(node)) =
            (self.data_for_testing.as_deref_mut(), ast_node_for_testing)
        {
            data.generated_type_constraints
                .entry(node)
                .or_default()
                .push(constraint);
        }
    }
}

impl<'b, 'a> TypeConstraintGenerator for TypeConstraintGatherer<'b, 'a> {
    type Operations = TypeSystemOperations<'a>;

    fn inference_using_bounds_is_enabled(&self) -> bool {
        self.inference_using_bounds_is_enabled
    }

    fn current_state(&self) -> TypeConstraintGeneratorState {
        TypeConstraintGeneratorState {
            count: self.constraints.len(),
        }
    }

    fn enable_discrepant_obliviousness_of_nullability_suffix_of_future_or(&self) -> bool {
        false
    }

    fn type_analyzer_operations(&self) -> &TypeSystemOperations<'a> {
        self.type_system_operations
    }

    fn type_parameters_to_constrain(&self) -> Vec<EId<TypeParameterElement>> {
        self.type_parameters_to_constrain.iter().copied().collect()
    }

    fn add_lower_constraint_for_parameter(
        &mut self,
        element: EId<TypeParameterElement>,
        lower: TypeId,
        ast_node_for_testing: Option<NodeId>,
    ) {
        let generated_type_constraint = GeneratedTypeConstraint::lower(
            SharedTypeParameterView::new(element),
            SharedTypeSchemaView::new(lower),
        );
        self.constraints.push(generated_type_constraint);
        self.record_for_testing(generated_type_constraint, ast_node_for_testing);
    }

    fn add_upper_constraint_for_parameter(
        &mut self,
        element: EId<TypeParameterElement>,
        upper: TypeId,
        ast_node_for_testing: Option<NodeId>,
    ) {
        let generated_type_constraint = GeneratedTypeConstraint::upper(
            SharedTypeParameterView::new(element),
            SharedTypeSchemaView::new(upper),
        );
        self.constraints.push(generated_type_constraint);
        self.record_for_testing(generated_type_constraint, ast_node_for_testing);
    }

    fn compute_constraints(&mut self) -> ConstraintsMap<TypeSystemOperations<'a>> {
        let mut result: ConstraintsMap<TypeSystemOperations<'a>> = indexmap::IndexMap::new();
        for &parameter in &self.type_parameters_to_constrain {
            result.insert(
                parameter,
                MergedTypeConstraint {
                    lower: SharedTypeSchemaView::new(TypeId::UNKNOWN),
                    upper: SharedTypeSchemaView::new(TypeId::UNKNOWN),
                    origin: TypeConstraintOrigin::Unknown,
                },
            );
        }

        for constraint in &self.constraints {
            let parameter = constraint
                .type_parameter
                .unwrap_type_parameter_view_as_type_parameter_structure();
            let merged_constraint = result
                .get_mut(&parameter)
                .expect("a constraint for a type parameter to constrain");
            merged_constraint.merge_in(constraint, self.type_system_operations);
        }

        result
    }

    fn eliminate_type_parameters_in_generated_constraints(
        &mut self,
        eliminator: &[EId<TypeParameterElement>],
        elimination_start_state: TypeConstraintGeneratorState,
        ast_node_for_testing: Option<NodeId>,
    ) {
        let constraints = self.constraints.split_off(elimination_start_state.count);
        let operations = self.type_system_operations;
        for constraint in constraints {
            let parameter = constraint
                .type_parameter
                .unwrap_type_parameter_view_as_type_parameter_structure();
            if constraint.is_upper {
                self.add_upper_constraint_for_parameter(
                    parameter,
                    dartr_flow::type_analyzer_operations::TypeAnalyzerOperations::least_closure_of_type_internal(
                        operations,
                        constraint.constraint.unwrap_type_schema_view(),
                        eliminator,
                    ),
                    ast_node_for_testing,
                );
            } else {
                self.add_lower_constraint_for_parameter(
                    parameter,
                    dartr_flow::type_analyzer_operations::TypeAnalyzerOperations::greatest_closure_of_type_internal(
                        operations,
                        constraint.constraint.unwrap_type_schema_view(),
                        eliminator,
                    ),
                    ast_node_for_testing,
                );
            }
        }
    }

    fn get_type_arguments_as_instance_of(
        &self,
        ty: TypeId,
        type_declaration: EId<InterfaceElement>,
    ) -> Option<Vec<TypeId>> {
        let ctx = self.type_system_operations.type_system.ctx;
        let element = ctx.interface_element(ty).expect("interface type");
        for &interface in ctx.element_all_supertypes(element) {
            if ctx.interface_element(interface) == Some(type_declaration) {
                let substitution = MapSubstitution::from_interface_type(&ctx, ty);
                let mapped = substitution.substitute_type(&ctx, interface);
                return Some(ctx.type_arguments(mapped).to_vec());
            }
        }
        None
    }

    fn instantiate_function_types_and_provide_fresh_type_parameters(
        &mut self,
        p: TypeId,
        q: TypeId,
        left_schema: bool,
    ) -> (TypeId, TypeId, Vec<EId<TypeParameterElement>>) {
        let ctx = self.type_system_operations.type_system.ctx;
        let function_type_parameters = |t: TypeId| match *ctx.ty(t) {
            TypeKind::Function(f) => ctx.list(f.type_params),
            _ => panic!("not a function type: {t:?}"),
        };
        let p_type_parameters = function_type_parameters(p);
        let q_type_parameters = function_type_parameters(q);
        // And `Z0...Zn` are fresh variables with bounds `B20, ..., B2n`.
        //   Where `B2i` is `B0i[Z0/T0, ..., Zn/Tn]` if `P` is a type schema.
        //   Or `B2i` is `B1i[Z0/S0, ..., Zn/Sn]` if `Q` is a type schema.
        // In other words, we choose the bounds for the fresh variables from
        // whichever of the two generic function types is a type schema and
        // does not contain any variables from `L`.
        let mut new_type_parameters: Vec<EId<TypeParameterElement>> = Vec::new();
        for i in 0..p_type_parameters.len() {
            let bound = if left_schema {
                ctx.type_parameter_bound(p_type_parameters[i])
            } else {
                ctx.type_parameter_bound(q_type_parameters[i])
            };
            let z = ctx.new_type_parameter(Some(ctx.name(&format!("Z{i}"))), None, bound);
            new_type_parameters.push(z);
        }

        // And `F0[Z0/T0, ..., Zn/Tn]` is a subtype match for
        // `F1[Z0/S0, ..., Zn/Sn]` with respect to `L` under constraints `C0`.
        let type_arguments: Vec<TypeId> = new_type_parameters
            .iter()
            .map(|&e| ctx.type_parameter_type(e, Nullability::None))
            .collect();
        let p_instantiated = ctx.instantiate_function_type(p, &type_arguments);
        let q_instantiated = ctx.instantiate_function_type(q, &type_arguments);

        (p_instantiated, q_instantiated, new_type_parameters)
    }

    fn restore_state(&mut self, state: TypeConstraintGeneratorState) {
        self.constraints.truncate(state.count);
    }

    dartr_type_analyzer::type_constraint_generator_mixin!();
}
