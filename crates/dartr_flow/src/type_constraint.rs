// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_constraint.dart

//! Type constraints of type inference (data types used in the signatures of
//! [`TypeAnalyzerOperations`] and [`TypeConstraintGenerator`]).
//!
//! The methods here are the short Dart method bodies; the constraint
//! generation algorithm itself belongs to unit A6 (analyzer side) and A13
//! (shared side).
//!
//! [`TypeConstraintGenerator`]: crate::type_analyzer_operations::TypeConstraintGenerator

use indexmap::IndexMap;
use std::hash::Hash;

use crate::shared_type::{
    SharedTypeOperations, SharedTypeParameterView, SharedTypeSchemaView, SharedTypeView,
};
use crate::type_analyzer_operations::TypeAnalyzerOperations;

/// Tracks a single constraint on a single type parameter.
///
/// We require that `typeParameter <: constraint` if `is_upper` is true, and
/// `constraint <: typeParameter` otherwise.
///
/// `P` is the client's type parameter, `T` the client's type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeneratedTypeConstraint<P, T> {
    /// The type parameter that is constrained by `constraint`.
    pub type_parameter: SharedTypeParameterView<P>,

    /// The type schema constraining the type parameter.
    pub constraint: SharedTypeSchemaView<T>,

    /// True if `typeParameter <: constraint`, and false otherwise.
    pub is_upper: bool,
}

impl<P, T> GeneratedTypeConstraint<P, T> {
    /// `GeneratedTypeConstraint.lower`: `constraint <: typeParameter`.
    pub fn lower(
        type_parameter: SharedTypeParameterView<P>,
        constraint: SharedTypeSchemaView<T>,
    ) -> Self {
        GeneratedTypeConstraint {
            type_parameter,
            constraint,
            is_upper: false,
        }
    }

    /// `GeneratedTypeConstraint.upper`: `typeParameter <: constraint`.
    pub fn upper(
        type_parameter: SharedTypeParameterView<P>,
        constraint: SharedTypeSchemaView<T>,
    ) -> Self {
        GeneratedTypeConstraint {
            type_parameter,
            constraint,
            is_upper: true,
        }
    }
}

/// A constraint on a type parameter that we're inferring:
/// `lower <: T <: upper`.
///
/// Dart passes these objects by reference and mutates them in place (also
/// through the `constraints` map); Rust code keeps them in the map and
/// mutates them there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MergedTypeConstraint<T> {
    /// The lower bound of the type being constrained. This bound must be a
    /// subtype of the type being constrained. In other words,
    /// `lowerBound <: T`.
    pub lower: SharedTypeSchemaView<T>,

    /// The upper bound of the type being constrained. The type being
    /// constrained must be a subtype of this bound. In other words,
    /// `T <: upperBound`.
    pub upper: SharedTypeSchemaView<T>,

    /// Where this constraint comes from, used for error messages.
    pub origin: TypeConstraintOrigin<T>,
}

impl<T: Copy> MergedTypeConstraint<T> {
    /// `MergedTypeConstraint.fromExtends`.
    pub fn from_extends<O>(
        type_parameter_name: String,
        bound_type: SharedTypeView<T>,
        extends_type: SharedTypeView<T>,
        type_analyzer_operations: &O,
    ) -> Self
    where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        MergedTypeConstraint {
            origin: TypeConstraintOrigin::FromExtendsClause {
                type_parameter_name,
                bound_type,
                extends_type,
            },
            upper: type_analyzer_operations.type_to_schema(extends_type),
            lower: type_analyzer_operations.unknown_type(),
        }
    }

    /// `MergedTypeConstraint.isEmpty`: both bounds are `_`.
    pub fn is_empty<O>(&self, type_analyzer_operations: &O) -> bool
    where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        use crate::shared_type::SharedTypeKind::Unknown;
        type_analyzer_operations.shared_type_kind(self.lower.unwrap_type_schema_view()) == Unknown
            && type_analyzer_operations.shared_type_kind(self.upper.unwrap_type_schema_view())
                == Unknown
    }

    /// `MergedTypeConstraint.isSatisfiedBy`.
    pub fn is_satisfied_by<O>(&self, ty: SharedTypeView<T>, type_analyzer_operations: &O) -> bool
    where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        type_analyzer_operations.type_is_subtype_of_type_schema(ty, self.upper)
            && type_analyzer_operations.type_schema_is_subtype_of_type(self.lower, ty)
    }

    /// `MergedTypeConstraint.mergeIn`.
    pub fn merge_in<P, O>(
        &mut self,
        generated_type_constraint: &GeneratedTypeConstraint<P, T>,
        type_analyzer_operations: &O,
    ) where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        if generated_type_constraint.is_upper {
            self.merge_in_type_schema_upper(
                generated_type_constraint.constraint,
                type_analyzer_operations,
            );
        } else {
            self.merge_in_type_schema_lower(
                generated_type_constraint.constraint,
                type_analyzer_operations,
            );
        }
    }

    /// `MergedTypeConstraint.mergeInTypeSchemaLower`: `lower = lower ⊔ constraint`.
    pub fn merge_in_type_schema_lower<O>(
        &mut self,
        constraint: SharedTypeSchemaView<T>,
        type_analyzer_operations: &O,
    ) where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        self.lower = type_analyzer_operations.type_schema_lub(self.lower, constraint);
    }

    /// `MergedTypeConstraint.mergeInTypeSchemaUpper`: `upper = upper ⊓ constraint`.
    pub fn merge_in_type_schema_upper<O>(
        &mut self,
        constraint: SharedTypeSchemaView<T>,
        type_analyzer_operations: &O,
    ) where
        O: TypeAnalyzerOperations + SharedTypeOperations<Type = T> + ?Sized,
    {
        self.upper = type_analyzer_operations.type_schema_glb(self.upper, constraint);
    }
}

/// The origin of a type constraint, for the purposes of producing a human
/// readable error message during type inference as well as determining
/// whether the constraint was used to fix the type parameter or not.
///
/// Dart: abstract class `TypeConstraintOrigin` and its subclasses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeConstraintOrigin<T> {
    /// `TypeConstraintFromArgument`.
    FromArgument {
        /// The static type of the argument.
        argument_type: SharedTypeView<T>,
        /// The declared type of the parameter.
        parameter_type: SharedTypeView<T>,
        /// The name of the parameter.
        parameter_name: String,
    },
    /// `TypeConstraintFromExtendsClause`.
    FromExtendsClause {
        /// Name of the type parameter with the extends clause.
        type_parameter_name: String,
        /// The declared bound of the type parameter, for example
        /// `Iterable<T>` for `<T, E extends Iterable<T>>`.
        bound_type: SharedTypeView<T>,
        /// `bound_type` in which type parameters are substituted with
        /// inferred type arguments, for example `Iterable<int>`.
        extends_type: SharedTypeView<T>,
    },
    /// `TypeConstraintFromFunctionContext`.
    FromFunctionContext {
        /// The context type.
        context_type: T,
        /// The function type.
        function_type: T,
    },
    /// `TypeConstraintFromReturnType`.
    FromReturnType {
        /// The context type.
        context_type: T,
        /// The declared return type.
        declared_type: T,
    },
    /// `UnknownTypeConstraintOrigin`.
    Unknown,
}

impl<T: Copy> TypeConstraintOrigin<T> {
    /// `TypeConstraintOrigin.formatError`: the lines of the error message
    /// that explain this origin.
    pub fn format_error<O>(&self, type_analyzer_operations: &O) -> Vec<String>
    where
        O: SharedTypeOperations<Type = T> + ?Sized,
    {
        let display = |ty: T| type_analyzer_operations.get_display_string(ty);
        match self {
            TypeConstraintOrigin::FromArgument {
                argument_type,
                parameter_type,
                parameter_name,
            } => vec![
                format!("Parameter '{parameter_name}'"),
                format!(
                    "declared as     '{}'",
                    display(parameter_type.unwrap_type_view())
                ),
                format!(
                    "but argument is '{}'.",
                    display(argument_type.unwrap_type_view())
                ),
            ],
            TypeConstraintOrigin::FromExtendsClause {
                type_parameter_name,
                bound_type,
                extends_type,
            } => vec![
                format!("Type parameter '{type_parameter_name}'"),
                format!(
                    "is declared to extend '{}' producing '{}'.",
                    display(bound_type.unwrap_type_view()),
                    display(extends_type.unwrap_type_view())
                ),
            ],
            TypeConstraintOrigin::FromFunctionContext {
                context_type,
                function_type,
            } => vec![
                "Function type".to_string(),
                format!("declared as '{}'", display(*function_type)),
                format!("used where  '{}' is required.", display(*context_type)),
            ],
            TypeConstraintOrigin::FromReturnType {
                context_type,
                declared_type,
            } => vec![
                "Return type".to_string(),
                format!("declared as '{}'", display(*declared_type)),
                format!("used where  '{}' is required.", display(*context_type)),
            ],
            TypeConstraintOrigin::Unknown => Vec::new(),
        }
    }
}

/// Data structure maintaining intermediate type inference results, such as
/// type constraints, for testing purposes. Under normal execution, no
/// instance of this type should be created.
///
/// `P` is the client's type parameter, `T` the client's type, `A` the
/// client's AST node.
#[derive(Clone, Debug)]
pub struct TypeConstraintGenerationDataForTesting<P, T, A> {
    /// Map from nodes requiring type inference to the generated type
    /// constraints for the node.
    pub generated_type_constraints: IndexMap<A, Vec<GeneratedTypeConstraint<P, T>>>,
}

impl<P, T, A> Default for TypeConstraintGenerationDataForTesting<P, T, A> {
    fn default() -> Self {
        TypeConstraintGenerationDataForTesting {
            generated_type_constraints: IndexMap::new(),
        }
    }
}

impl<P, T, A: Eq + Hash> TypeConstraintGenerationDataForTesting<P, T, A> {
    /// Merges `other` into the receiver, combining the constraints.
    pub fn merge_in(&mut self, other: TypeConstraintGenerationDataForTesting<P, T, A>) {
        for (node, constraints) in other.generated_type_constraints {
            self.generated_type_constraints
                .entry(node)
                .or_default()
                .extend(constraints);
        }
    }
}
