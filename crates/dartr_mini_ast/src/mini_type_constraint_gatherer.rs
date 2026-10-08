// Dart source: pkg/_fe_analyzer_shared/test/mini_type_constraint_gatherer.dart

//! [`TypeConstraintGatherer`]: the constraint generator of the mini-AST
//! test harness. It records each generated constraint as a string
//! (`"T <: int"`, `"int <: T"`) in [`constraints`](TypeConstraintGatherer::constraints).
//!
//! The Dart class mixes in `TypeConstraintGeneratorMixin`; here the shared
//! algorithm is applied with
//! [`dartr_type_analyzer::type_constraint_generator_mixin!`].

use indexmap::IndexSet;

use dartr_flow::type_analyzer_operations::{
    ConstraintsMap, TypeConstraintGenerator, TypeConstraintGeneratorState,
};

use super::mini_types::{Name, Type, TypeParameter, TypeRegistry};
use super::node::Node;
use super::operations::MiniAstOperations;

/// The constraint generator of the mini-AST test harness (Dart class
/// `TypeConstraintGatherer`).
pub struct TypeConstraintGatherer {
    /// `typeParametersToConstrain` (a Dart `Set`, iterated in insertion
    /// order).
    pub type_parameters_to_constrain: IndexSet<TypeParameter>,

    /// `enableDiscrepantObliviousnessOfNullabilitySuffixOfFutureOr`.
    pub enable_discrepant_obliviousness_of_nullability_suffix_of_future_or: bool,

    /// `typeAnalyzerOperations`: a fresh [`MiniAstOperations`], as in Dart.
    pub type_analyzer_operations: MiniAstOperations,

    /// The generated constraints, as strings.
    pub constraints: Vec<String>,

    /// `inferenceUsingBoundsIsEnabled` (always `false`).
    inference_using_bounds_is_enabled: bool,
}

impl TypeConstraintGatherer {
    /// `TypeConstraintGatherer(typeVariablesBeingConstrained)`: registers a
    /// type parameter for each name in `type_variables_being_constrained`.
    pub fn new(type_variables_being_constrained: &[&str]) -> Self {
        Self::with_options(type_variables_being_constrained, false)
    }

    /// `TypeConstraintGatherer(typeVariablesBeingConstrained,
    /// enableDiscrepantObliviousnessOfNullabilitySuffixOfFutureOr: ...)`.
    pub fn with_options(
        type_variables_being_constrained: &[&str],
        enable_discrepant_obliviousness_of_nullability_suffix_of_future_or: bool,
    ) -> Self {
        let mut type_parameters_to_constrain = IndexSet::new();
        // Dart iterates a `Set<String>` literal, so duplicates are dropped.
        let mut seen = IndexSet::new();
        for type_variable_name in type_variables_being_constrained {
            if seen.insert(*type_variable_name) {
                type_parameters_to_constrain
                    .insert(TypeRegistry::add_type_parameter(type_variable_name));
            }
        }
        TypeConstraintGatherer {
            type_parameters_to_constrain,
            enable_discrepant_obliviousness_of_nullability_suffix_of_future_or,
            type_analyzer_operations: MiniAstOperations::new(),
            constraints: Vec::new(),
            inference_using_bounds_is_enabled: false,
        }
    }
}

impl TypeConstraintGenerator for TypeConstraintGatherer {
    type Operations = MiniAstOperations;

    fn inference_using_bounds_is_enabled(&self) -> bool {
        self.inference_using_bounds_is_enabled
    }

    fn current_state(&self) -> TypeConstraintGeneratorState {
        TypeConstraintGeneratorState {
            count: self.constraints.len(),
        }
    }

    fn enable_discrepant_obliviousness_of_nullability_suffix_of_future_or(&self) -> bool {
        self.enable_discrepant_obliviousness_of_nullability_suffix_of_future_or
    }

    fn type_analyzer_operations(&self) -> &MiniAstOperations {
        &self.type_analyzer_operations
    }

    fn type_parameters_to_constrain(&self) -> Vec<TypeParameter> {
        self.type_parameters_to_constrain.iter().copied().collect()
    }

    fn add_lower_constraint_for_parameter(
        &mut self,
        type_parameter: TypeParameter,
        lower: Type,
        ast_node_for_testing: Option<Node>,
    ) {
        let _ = ast_node_for_testing;
        self.constraints
            .push(format!("{lower} <: {type_parameter}"));
    }

    fn add_upper_constraint_for_parameter(
        &mut self,
        type_parameter: TypeParameter,
        upper: Type,
        ast_node_for_testing: Option<Node>,
    ) {
        let _ = ast_node_for_testing;
        self.constraints
            .push(format!("{type_parameter} <: {upper}"));
    }

    fn compute_constraints(&mut self) -> ConstraintsMap<MiniAstOperations> {
        // TODO(cstefantsova): implement computeConstraints
        unimplemented!("computeConstraints")
    }

    fn eliminate_type_parameters_in_generated_constraints(
        &mut self,
        type_parameters_to_eliminate: &[TypeParameter],
        elimination_start_state: TypeConstraintGeneratorState,
        ast_node_for_testing: Option<Node>,
    ) {
        let _ = (
            type_parameters_to_eliminate,
            elimination_start_state,
            ast_node_for_testing,
        );
        // TODO(paulberry): implement eliminateTypeParametersInGeneratedConstraints
    }

    fn get_type_arguments_as_instance_of(
        &self,
        ty: Type,
        type_declaration: Name,
    ) -> Option<Vec<Type>> {
        // We just have a few cases hardcoded here to make the tests work.
        // TODO(paulberry): if this gets too unwieldy, replace it with a more
        // general implementation.
        let primary = ty.as_primary_type();
        let name = primary.as_ref().map(|p| p.name());
        match (name, type_declaration) {
            (Some("List"), "Iterable") => {
                // List<T> inherits from Iterable<T>
                Some(primary.unwrap().args)
            }
            (Some("MyListOfInt"), "List") => {
                // MyListOfInt inherits from List<int>
                Some(vec![Type::parse("int")])
            }
            (Some("Future"), "int")
            | (Some("int"), "String")
            | (Some("List"), "Future")
            | (Some("String"), "int")
            | (Some("Future"), "String") => {
                // Unrelated types
                None
            }
            _ => unimplemented!("getTypeArgumentsAsInstanceOf({ty}, {type_declaration})"),
        }
    }

    fn instantiate_function_types_and_provide_fresh_type_parameters(
        &mut self,
        p: Type,
        q: Type,
        left_schema: bool,
    ) -> (Type, Type, Vec<TypeParameter>) {
        let _ = (p, q, left_schema);
        // TODO(paulberry): implement instantiateFunctionTypesAndProvideEliminator
        unimplemented!("instantiateFunctionTypesAndProvideFreshTypeParameters")
    }

    fn restore_state(&mut self, state: TypeConstraintGeneratorState) {
        self.constraints.truncate(state.count);
    }

    dartr_type_analyzer::type_constraint_generator_mixin!();
}
