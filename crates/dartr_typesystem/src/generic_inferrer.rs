// Dart source: pkg/analyzer/lib/src/dart/element/generic_inferrer.dart

//! [`GenericInferrer`]: tracks upper and lower type bounds for a set of type
//! parameters.
//!
//! When the methods of this type encounter one of the type parameters it is
//! inferring, it records the constraint and optimistically assumes the
//! constraint will be satisfied.
//!
//! For example if we are inferring type parameter A, and we ask if
//! `A <: num`, this records that A must be a subtype of `num`. It also
//! handles cases when A appears as part of the structure of another type, for
//! example `Iterable<A> <: Iterable<num>` would infer the same constraint
//! (due to covariant generic types) as would `() -> A <: () -> num`. In
//! contrast `(A) -> void <: (num) -> void`.
//!
//! Once the lower/upper bounds are determined, `chooseFinalTypes` should be
//! called to finish the inference.
//!
//! Differences to the Dart class:
//! - Dart compares `TypeConstraintOrigin`s by identity (`_formatError`
//!   groups constraints by origin object). Here each stored constraint
//!   carries an origin id: one id per `_tryMatchSubtypeOf` call and one per
//!   `MergedTypeConstraint.fromExtends`, as Dart creates one origin object
//!   for each.
//! - The Dart `SyntacticEntity errorEntity` is an [`InferenceErrorEntity`]:
//!   the range and the facts about the AST node that
//!   `_reportInferenceFailure` reads. The resolver fills it.
//! - `inferenceLogWriter` is not ported (it is `null` unless a debug flag is
//!   set).

use dartr_ast::NodeId;
use dartr_diagnostics::{DiagnosticReporter, LocatableDiagnostic, diag};
use dartr_element::{
    DisplayOptions, EId, FnParam, FormalParameterElement, TypeId, TypeKind, TypeParameterElement,
};
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer_operations::{TypeAnalyzerOperations, TypeConstraintGenerator};
use indexmap::{IndexMap, IndexSet};

use crate::type_algebra::MapSubstitution;
use crate::type_constraint_gatherer::{
    MergedTypeConstraint, TypeConstraintGatherer, TypeConstraintGenerationDataForTesting,
    TypeConstraintOrigin,
};
use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;
use crate::type_system_operations::TypeSystemOperations;

/// The facts about the error entity (Dart `SyntacticEntity errorEntity`)
/// that the inferrer needs: the range for `couldNotInfer`, and what
/// `_reportInferenceFailure` reads from the AST node.
#[derive(Clone, Debug)]
pub struct InferenceErrorEntity {
    pub offset: usize,
    pub length: usize,
    /// `errorEntity is AstNode && errorEntity.parent is InvocationExpression
    /// && errorEntity.parent?.parent is AsExpression`.
    pub is_invocation_in_as_expression: bool,
    pub kind: InferenceErrorEntityKind,
}

impl InferenceErrorEntity {
    /// An entity that is none of the kinds that `_reportInferenceFailure`
    /// checks.
    pub fn other(offset: usize, length: usize) -> InferenceErrorEntity {
        InferenceErrorEntity {
            offset,
            length,
            is_invocation_in_as_expression: false,
            kind: InferenceErrorEntityKind::Other,
        }
    }
}

/// The kind of the error entity, as `_reportInferenceFailure` checks it.
#[derive(Clone, Debug)]
pub enum InferenceErrorEntityKind {
    /// `ConstructorName`.
    ConstructorName {
        /// `(errorEntity.type.type as InterfaceType).element.metadata
        /// .hasOptionalTypeArgs`.
        type_element_has_optional_type_args: bool,
        /// `errorEntity.name == null ? errorEntity.type.qualifiedName :
        /// '${errorEntity.type}.${errorEntity.name}'`.
        constructor_name: String,
    },
    /// `Annotation`.
    Annotation {
        /// `errorEntity.name.element?.metadata.hasOptionalTypeArgs`
        /// (`None` when the element is `null`).
        element_has_optional_type_args: Option<bool>,
        /// `errorEntity.constructorName == null ? errorEntity.name.name :
        /// '${errorEntity.name.name}.${errorEntity.constructorName}'`.
        constructor_name: String,
    },
    /// `SimpleIdentifier`.
    SimpleIdentifier {
        /// `errorEntity.name`.
        name: String,
        /// `errorEntity.element` (`None` when it is `null`).
        element: Option<SimpleIdentifierElementFacts>,
    },
    /// Any other `Expression`.
    Expression {
        /// `errorEntity.staticType`.
        static_type: Option<TypeId>,
    },
    /// Any other entity.
    Other,
}

/// What `_reportInferenceFailure` reads from the element of a
/// `SimpleIdentifier` error entity.
#[derive(Clone, Copy, Debug)]
pub struct SimpleIdentifierElementFacts {
    /// The element is a `VariableElement`, and the interface element of its
    /// type or the element of the alias of its type has
    /// `metadata.hasOptionalTypeArgs`.
    pub variable_type_has_optional_type_args: bool,
    /// `element.metadata.hasOptionalTypeArgs`.
    pub has_optional_type_args: bool,
}

/// The feature flags of generic inference (Dart named parameters of the
/// `GenericInferrer` constructor and the `TypeSystemImpl` entry points).
#[derive(Clone, Copy, Debug, Default)]
pub struct InferenceFlags {
    /// The "generic metadata" feature: type arguments may be generic
    /// function types.
    pub generic_metadata_is_enabled: bool,
    /// The "inference using bounds" feature.
    pub inference_using_bounds_is_enabled: bool,
    /// The `strict-inference` analysis option.
    pub strict_inference: bool,
}

/// A constraint of `_constraints` with the identity of its origin object.
#[derive(Clone, Debug)]
struct StoredConstraint {
    constraint: MergedTypeConstraint,
    origin_id: u32,
}

/// `GenericInferrer`.
///
/// As in Dart, one value infers a single call and is dropped afterwards.
pub struct GenericInferrer<'a, 'r, 'l> {
    type_system: TypeSystem<'a>,
    type_parameters: Vec<EId<TypeParameterElement>>,
    constraints: IndexMap<EId<TypeParameterElement>, Vec<StoredConstraint>>,

    /// The list of type parameters being inferred.
    type_formals: Vec<EId<TypeParameterElement>>,

    /// The reporter to which inference diagnostics should be reported, or
    /// `None` if diagnostics shouldn't be reported.
    diagnostic_reporter: Option<&'r mut DiagnosticReporter<'l>>,

    /// The entity to which errors should be attached. May be `None` if errors
    /// are not being reported (that is, if `diagnostic_reporter` is also
    /// `None`).
    pub error_entity: Option<InferenceErrorEntity>,

    generic_metadata_is_enabled: bool,

    /// Whether the "inference using bounds" feature is enabled.
    pub inference_using_bounds_is_enabled: bool,

    strict_inference: bool,

    /// The fixed inferred types (`_typesInferredSoFar`). See the Dart
    /// documentation: downwards inference fixes a type parameter so that
    /// upwards inference can't refine it (runtime checked covariant
    /// generics).
    types_inferred_so_far: Vec<TypeId>,

    type_system_operations: TypeSystemOperations<'a>,

    pub data_for_testing: Option<&'r mut TypeConstraintGenerationDataForTesting>,

    next_origin_id: u32,
}

impl<'a, 'r, 'l> GenericInferrer<'a, 'r, 'l> {
    /// `GenericInferrer(typeSystem, typeFormals, diagnosticReporter:,
    /// errorEntity:, genericMetadataIsEnabled:, inferenceUsingBoundsIsEnabled:,
    /// strictInference:, typeSystemOperations:, dataForTesting:)`.
    pub fn new(
        type_system: TypeSystem<'a>,
        type_formals: &[EId<TypeParameterElement>],
        diagnostic_reporter: Option<&'r mut DiagnosticReporter<'l>>,
        error_entity: Option<InferenceErrorEntity>,
        flags: InferenceFlags,
        type_system_operations: TypeSystemOperations<'a>,
        data_for_testing: Option<&'r mut TypeConstraintGenerationDataForTesting>,
    ) -> GenericInferrer<'a, 'r, 'l> {
        assert!(diagnostic_reporter.is_none() || error_entity.is_some());
        GenericInferrer {
            type_system,
            type_parameters: type_formals.to_vec(),
            types_inferred_so_far: vec![TypeId::UNKNOWN; type_formals.len()],
            constraints: type_formals.iter().map(|&f| (f, Vec::new())).collect(),
            diagnostic_reporter,
            error_entity,
            type_formals: type_formals.to_vec(),
            generic_metadata_is_enabled: flags.generic_metadata_is_enabled,
            inference_using_bounds_is_enabled: flags.inference_using_bounds_is_enabled,
            strict_inference: flags.strict_inference,
            type_system_operations,
            data_for_testing,
            next_origin_id: 0,
        }
    }

    /// Performs upwards inference, producing a final set of inferred types
    /// that does not contain references to the "unknown type".
    pub fn choose_final_types(&mut self) -> Vec<TypeId> {
        self.try_choose_final_types(false)
            .expect("chooseFinalTypes: tryChooseFinalTypes(failAtError: false) returned null")
    }

    /// Performs partial (either downwards or horizontal) inference, producing
    /// a set of inferred types that may contain references to the "unknown
    /// type".
    pub fn choose_preliminary_types(&mut self) -> Vec<TypeId> {
        let mut inference_phase_constraints = self.inference_phase_constraints();
        let types = self.type_system_operations.choose_types(
            &self.type_formals,
            &mut inference_phase_constraints,
            Some(&self.types_inferred_so_far),
            true,
            self.inference_using_bounds_is_enabled,
            None,
            None,
        );

        // Mark type parameters with fully known inferred types as "fixed" in
        // the overall solution.
        for (type_parameter_index, &inferred_type) in types.iter().enumerate() {
            let type_parameter = self.type_formals[type_parameter_index];
            if self
                .type_system
                .ctx
                .type_parameter_is_legacy_covariant(type_parameter)
                && self
                    .type_system_operations
                    .is_known_type(SharedTypeSchemaView::new(inferred_type))
            {
                self.types_inferred_so_far[type_parameter_index] = inferred_type;
            }
        }

        types
    }

    /// Applies an argument constraint, which asserts that the
    /// [argument_type] static type is a subtype of the [parameter_type].
    pub fn constrain_argument(
        &mut self,
        argument_type: TypeId,
        parameter_type: TypeId,
        parameter_name: &str,
        node_for_testing: Option<NodeId>,
    ) {
        let origin = TypeConstraintOrigin::FromArgument {
            argument_type: SharedTypeView::new(argument_type),
            parameter_type: SharedTypeView::new(parameter_type),
            parameter_name: parameter_name.to_string(),
        };
        self.try_match_subtype_of(
            argument_type,
            parameter_type,
            origin,
            false,
            node_for_testing,
        );
    }

    /// `constrainArguments`: applies all the argument constraints implied by
    /// the formal parameter elements [parameters] and [argument_types].
    pub fn constrain_arguments(
        &mut self,
        parameters: &[EId<FormalParameterElement>],
        argument_types: &[TypeId],
        node_for_testing: Option<NodeId>,
    ) {
        let ctx = self.type_system.ctx;
        for (i, &argument_type) in argument_types.iter().enumerate() {
            // Try to pass each argument to each parameter, recording any type
            // parameter bounds that were implied by this assignment.
            let parameter = ctx.get(parameters[i]);
            let parameter_type = parameter.type_.get().unwrap_or(TypeId::INVALID);
            let name = parameter.name.map(|n| ctx.name_str(n)).unwrap_or("");
            self.constrain_argument(argument_type, parameter_type, name, node_for_testing);
        }
    }

    /// `constrainArguments2`: applies all the argument constraints implied by
    /// the function type parameters [parameters] and [argument_types].
    pub fn constrain_arguments2(
        &mut self,
        parameters: &[FnParam],
        argument_types: &[TypeId],
        node_for_testing: Option<NodeId>,
    ) {
        let ctx = self.type_system.ctx;
        for (i, &argument_type) in argument_types.iter().enumerate() {
            // Try to pass each argument to each parameter, recording any type
            // parameter bounds that were implied by this assignment.
            let parameter = parameters[i];
            let name = parameter.name.map(|n| ctx.name_str(n)).unwrap_or("");
            self.constrain_argument(argument_type, parameter.ty, name, node_for_testing);
        }
    }

    /// Constrains a universal function type [fn_type] used in a context
    /// [context_type].
    pub fn constrain_generic_function_in_context(
        &mut self,
        fn_type: TypeId,
        context_type: TypeId,
        node_for_testing: Option<NodeId>,
    ) {
        let ctx = self.type_system.ctx;
        let origin = TypeConstraintOrigin::FromFunctionContext {
            context_type,
            function_type: fn_type,
        };

        // Since we're trying to infer the instantiation, we want to ignore
        // type formals as we check the parameters and return type.
        let TypeKind::Function(f) = *ctx.ty(fn_type) else {
            panic!("not a function type: {fn_type:?}");
        };
        let infer_fn_type = ctx.function_type(&[], ctx.list(f.params), f.ret, f.nullability, None);
        self.try_match_subtype_of(infer_fn_type, context_type, origin, true, node_for_testing);
    }

    /// Applies a return type constraint, which asserts that the
    /// [declared_type] is a subtype of the [context_type].
    pub fn constrain_return_type(
        &mut self,
        declared_type: TypeId,
        context_type: TypeId,
        node_for_testing: Option<NodeId>,
    ) {
        let origin = TypeConstraintOrigin::FromReturnType {
            declared_type,
            context_type,
        };
        self.try_match_subtype_of(declared_type, context_type, origin, true, node_for_testing);
    }

    /// Same as [choose_final_types](Self::choose_final_types), but if
    /// [fail_at_error] is `true` (the Dart default) and inference fails,
    /// returns `None` rather than trying to perform error recovery.
    pub fn try_choose_final_types(&mut self, fail_at_error: bool) -> Option<Vec<TypeId>> {
        let ctx = self.type_system.ctx;
        let ops = self.type_system_operations;
        let mut inference_phase_constraints = self.inference_phase_constraints();
        let inferred_types = ops.choose_types(
            &self.type_formals,
            &mut inference_phase_constraints,
            Some(&self.types_inferred_so_far),
            false,
            self.inference_using_bounds_is_enabled,
            None,
            None,
        );
        // Check the inferred types against all of the constraints.
        let mut known_types: IndexMap<EId<TypeParameterElement>, TypeId> = IndexMap::new();
        let mut has_error_reported = false;
        for i in 0..self.type_formals.len() {
            let parameter = self.type_formals[i];

            let inferred = inferred_types[i];
            let mut success = self.constraints[&parameter].iter().all(|c| {
                c.constraint
                    .is_satisfied_by(SharedTypeView::new(inferred), &ops)
            });

            // If everything else succeeded, check the `extends` constraint.
            if success {
                let name = ctx.element_name(parameter.raw());
                let parameter_bound_raw = ctx.type_parameter_bound(parameter);
                if let (Some(name), Some(parameter_bound_raw)) = (name, parameter_bound_raw) {
                    let parameter_bound =
                        MapSubstitution::from_pairs(&self.type_formals, &inferred_types)
                            .substitute_type(&ctx, parameter_bound_raw);
                    let extends_constraint = MergedTypeConstraint::from_extends(
                        name.to_string(),
                        SharedTypeView::new(parameter_bound_raw),
                        SharedTypeView::new(parameter_bound),
                        &ops,
                    );
                    success =
                        extends_constraint.is_satisfied_by(SharedTypeView::new(inferred), &ops);
                    let origin_id = self.new_origin_id();
                    self.constraints[&parameter].push(StoredConstraint {
                        constraint: extends_constraint,
                        origin_id,
                    });
                }
            }

            if !success {
                if fail_at_error {
                    return None;
                }
                has_error_reported = true;

                let name = ctx.element_name(parameter.raw())?;

                let detail_text =
                    self.format_error(parameter, inferred, &self.constraints[&parameter]);
                self.report(diag::could_not_infer(name, &detail_text));

                // Heuristic: even if we failed, keep the erroneous type.
                // It should satisfy at least some of the constraints (e.g. the
                // return context). If we fall back to instantiateToBounds,
                // we'll typically get more errors (e.g. because `dynamic` is
                // the most common bound).
            }

            if let TypeKind::Function(f) = *ctx.ty(inferred)
                && !f.type_params.is_empty()
                && !self.generic_metadata_is_enabled
                && self.diagnostic_reporter.is_some()
            {
                if fail_at_error {
                    return None;
                }
                has_error_reported = true;

                let name = ctx.element_name(parameter.raw())?;

                let type_parameters_str = ctx
                    .list(f.type_params)
                    .iter()
                    .map(|e| self.element_str(*e))
                    .collect::<Vec<_>>()
                    .join(", ");
                let detail_text = format!(
                    " Inferred candidate type {} has type parameters [{type_parameters_str}], but a function with type parameters cannot be used as a type argument.",
                    self.type_str(inferred)
                );
                self.report(diag::could_not_infer(name, &detail_text));
            }

            if ops.is_known_type(SharedTypeSchemaView::new(inferred)) {
                known_types.insert(parameter, inferred);
            } else if self.strict_inference {
                // [typeParam] could not be inferred. A result will still be
                // returned by [infer], with [typeParam] filled in as its
                // bounds. This is considered a failure of inference, under the
                // "strict-inference" mode.
                self.report_inference_failure();
            }
        }

        // Use instantiate to bounds to finish things off.
        let (mut result, has_error_0) = self
            .type_system
            .instantiate_type_formals_to_bounds(&self.type_formals, Some(known_types));
        // Dart `hasError` is a list, but only `hasError[0]` is ever set.
        let mut has_error = vec![false; self.type_formals.len()];
        if has_error_0 {
            has_error[0] = true;
        }

        // Report any errors from instantiateToBounds.
        for (i, &error) in has_error.iter().enumerate() {
            if error {
                if fail_at_error {
                    return None;
                }
                has_error_reported = true;
                let type_param = self.type_formals[i];

                let name = ctx.element_name(type_param.raw())?;

                let type_param_bound =
                    MapSubstitution::from_pairs(&self.type_formals, &inferred_types)
                        .substitute_type(
                            &ctx,
                            ctx.type_parameter_bound(type_param)
                                .unwrap_or_else(|| ctx.tp.object_type()),
                        );
                // TODO(jmesserly): improve this error message.
                let detail_text = format!(
                    "\nRecursive bound cannot be instantiated: '{}'.\nConsider passing explicit type argument(s) to the generic.\n\n'",
                    self.type_str(type_param_bound)
                );
                self.report(diag::could_not_infer(name, &detail_text));
            }
        }

        if !has_error_reported {
            self.check_arguments_not_matching_bounds(&result);
        }

        self.demote_types(&mut result);
        Some(result)
    }

    /// The squashed constraints of each type parameter (Dart
    /// `inferencePhaseConstraints`).
    fn inference_phase_constraints(
        &self,
    ) -> IndexMap<EId<TypeParameterElement>, MergedTypeConstraint> {
        self.constraints
            .iter()
            .map(|(&type_parameter, constraints)| {
                (type_parameter, self.squash_constraints(constraints))
            })
            .collect()
    }

    fn new_origin_id(&mut self) -> u32 {
        let id = self.next_origin_id;
        self.next_origin_id += 1;
        id
    }

    fn report(&mut self, diagnostic: LocatableDiagnostic) {
        if let Some(reporter) = self.diagnostic_reporter.as_deref_mut() {
            let entity = self
                .error_entity
                .as_ref()
                .expect("errorEntity is set when diagnosticReporter is set");
            reporter.report(diagnostic.at_offset(entity.offset, entity.length));
        }
    }

    /// Checks that inferred [type_arguments] satisfy the bounds of the type
    /// parameters.
    fn check_arguments_not_matching_bounds(&mut self, type_arguments: &[TypeId]) {
        let ctx = self.type_system.ctx;
        for i in 0..self.type_formals.len() {
            let parameter = self.type_formals[i];
            let argument = type_arguments[i];

            let Some(raw_bound) = ctx.type_parameter_bound(parameter) else {
                continue;
            };

            let Some(name) = ctx.element_name(parameter.raw()) else {
                continue;
            };

            let substitution = MapSubstitution::from_pairs(&self.type_formals, type_arguments);
            let bound = substitution.substitute_type(&ctx, raw_bound);
            if !self.type_system.is_subtype_of(argument, bound) {
                let arguments = type_arguments
                    .iter()
                    .map(|&t| self.type_str(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                let detail_text = format!(
                    "\n'{}' doesn't conform to the bound '{}', instantiated from '{}' using type arguments [{arguments}].",
                    self.type_str(argument),
                    self.type_str(bound),
                    self.type_str(raw_bound),
                );
                self.report(diag::could_not_infer(name, &detail_text));
            }
        }
    }

    fn demote_types(&self, types: &mut [TypeId]) {
        for t in types.iter_mut() {
            *t = self.type_system.demote_type(*t);
        }
    }

    fn element_str(&self, element: EId<TypeParameterElement>) -> String {
        dartr_element::element_display_string_with(
            &self.type_system.ctx,
            element.raw(),
            DisplayOptions::default(),
        )
    }

    fn format_error(
        &self,
        type_param: EId<TypeParameterElement>,
        inferred: TypeId,
        constraints: &[StoredConstraint],
    ) -> String {
        let ops = &self.type_system_operations;
        let inferred_str = self.type_str(inferred);
        let type_param_name = self
            .type_system
            .ctx
            .element_name(type_param.raw())
            .unwrap_or("null");
        let intro =
            format!("Tried to infer '{inferred_str}' for '{type_param_name}' which doesn't work:");

        let mut constraints_by_origin: IndexMap<u32, Vec<&StoredConstraint>> = IndexMap::new();
        for c in constraints {
            constraints_by_origin
                .entry(c.origin_id)
                .or_default()
                .push(c);
        }

        // Only report unique constraint origins.
        let is_satisfied = |expected: bool| -> Vec<&StoredConstraint> {
            constraints_by_origin
                .values()
                .filter(|l| {
                    l.iter().all(|c| {
                        c.constraint
                            .is_satisfied_by(SharedTypeView::new(inferred), ops)
                    }) == expected
                })
                .flat_map(|l| l.iter().copied())
                .collect()
        };

        let unsatisfied = Self::format_constraints(&is_satisfied(false), ops);
        let mut satisfied = Self::format_constraints(&is_satisfied(true), ops);

        assert!(!unsatisfied.is_empty());
        if !satisfied.is_empty() {
            satisfied = format!("\nThe type '{inferred_str}' was inferred from:\n{satisfied}");
        }

        format!(
            "\n\n{intro}\n{unsatisfied}{satisfied}\n\nConsider passing explicit type argument(s) to the generic.\n\n"
        )
    }

    /// Reports an inference failure on the error entity according to its
    /// type.
    fn report_inference_failure(&mut self) {
        if self.diagnostic_reporter.is_none() {
            return;
        }
        let Some(error_entity) = self.error_entity.clone() else {
            return;
        };
        if error_entity.is_invocation_in_as_expression {
            // Casts via `as` do not play a part in downward inference. We
            // allow an exception when inference has "failed" but the return
            // value is immediately cast with `as`.
            return;
        }
        match &error_entity.kind {
            InferenceErrorEntityKind::ConstructorName {
                type_element_has_optional_type_args,
                constructor_name,
            } => {
                if !type_element_has_optional_type_args {
                    self.report(diag::inference_failure_on_instance_creation(
                        constructor_name,
                    ));
                }
            }
            InferenceErrorEntityKind::Annotation {
                element_has_optional_type_args,
                constructor_name,
            } => {
                if self.generic_metadata_is_enabled {
                    // Only report an error if generic metadata is valid
                    // syntax.
                    if *element_has_optional_type_args == Some(false) {
                        self.report(diag::inference_failure_on_instance_creation(
                            constructor_name,
                        ));
                    }
                }
            }
            InferenceErrorEntityKind::SimpleIdentifier { name, element } => {
                if let Some(element) = element {
                    // For variable elements, we check their type and possible
                    // alias type.
                    if element.variable_type_has_optional_type_args {
                        return;
                    }
                    if !element.has_optional_type_args {
                        self.report(diag::inference_failure_on_function_invocation(name));
                    }
                }
            }
            InferenceErrorEntityKind::Expression { static_type } => {
                if let Some(ty) = *static_type {
                    let type_display_string = self.type_str(ty);
                    self.report(diag::inference_failure_on_generic_invocation(
                        &type_display_string,
                    ));
                }
            }
            InferenceErrorEntityKind::Other => {}
        }
    }

    fn squash_constraints(&self, constraints: &[StoredConstraint]) -> MergedTypeConstraint {
        let mut lower = TypeId::UNKNOWN;
        let mut upper = TypeId::UNKNOWN;
        let origin = TypeConstraintOrigin::Unknown;

        for constraint in constraints {
            // Given constraints:
            //
            //     L1 <: T <: U1
            //     L2 <: T <: U2
            //
            // These can be combined to produce:
            //
            //     LUB(L1, L2) <: T <: GLB(U1, U2).
            //
            // This can then be done for all constraints in sequence.
            //
            // This resulting constraint may be unsatisfiable; in that case
            // inference will fail.
            upper = self
                .type_system
                .greatest_lower_bound(upper, constraint.constraint.upper.unwrap_type_schema_view());
            lower = self
                .type_system
                .least_upper_bound(lower, constraint.constraint.lower.unwrap_type_schema_view());
        }
        MergedTypeConstraint {
            lower: SharedTypeSchemaView::new(lower),
            upper: SharedTypeSchemaView::new(upper),
            origin,
        }
    }

    /// Tries to make [t1] a subtype of [t2] and accumulate constraints as
    /// needed.
    ///
    /// The return value indicates whether the match was successful. If it
    /// was unsuccessful, any constraints that were accumulated during the
    /// match attempt have been rewound.
    fn try_match_subtype_of(
        &mut self,
        t1: TypeId,
        t2: TypeId,
        origin: TypeConstraintOrigin,
        covariant: bool,
        node_for_testing: Option<NodeId>,
    ) -> bool {
        let ops = &self.type_system_operations;
        let mut gatherer = TypeConstraintGatherer::new(
            &self.type_parameters,
            ops,
            self.inference_using_bounds_is_enabled,
            self.data_for_testing.as_deref_mut(),
        );
        let success = gatherer.perform_subtype_constraint_generation_internal(
            t1,
            t2,
            !covariant,
            node_for_testing,
        );
        if success {
            let constraints = gatherer.compute_constraints();
            drop(gatherer);
            // Dart creates one origin object per call and assigns it to every
            // constraint it keeps.
            let origin_id = self.new_origin_id();
            for (type_parameter_index, &type_parameter) in self.type_parameters.iter().enumerate() {
                if let Some(constraint) = constraints.get(&type_parameter)
                    && !constraint.is_empty(&self.type_system_operations)
                    // Dart: `== UnknownInferredType.instance` (identity).
                    && self.types_inferred_so_far[type_parameter_index] == TypeId::UNKNOWN
                {
                    let mut constraint = constraint.clone();
                    constraint.origin = origin.clone();
                    self.constraints[&type_parameter].push(StoredConstraint {
                        constraint,
                        origin_id,
                    });
                }
            }
        }

        success
    }

    fn type_str(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(&self.type_system.ctx, t, DisplayOptions::default())
    }

    fn format_constraints(
        constraints: &[&StoredConstraint],
        type_system_operations: &TypeSystemOperations<'_>,
    ) -> String {
        // Dart: `Set<TypeConstraintOrigin>.from(...)` (identity).
        let mut origins: IndexMap<u32, &TypeConstraintOrigin> = IndexMap::new();
        for c in constraints {
            origins.entry(c.origin_id).or_insert(&c.constraint.origin);
        }
        let line_parts: Vec<Vec<String>> = origins
            .values()
            .map(|o| o.format_error(type_system_operations))
            .collect();

        // Dart `String.length`: UTF-16 code units.
        let dart_length = |s: &str| s.encode_utf16().count();
        let prefix_max = line_parts
            .iter()
            .map(|p| dart_length(&p[0]))
            .fold(0, usize::max);

        // Use a set to prevent identical message lines.
        // (It's not uncommon for the same constraint to show up in a few
        // places.)
        let message_lines: IndexSet<String> = line_parts
            .iter()
            .map(|parts| {
                let prefix = &parts[0];
                let middle = &parts[1];
                let prefix_pad = " ".repeat(prefix_max - dart_length(prefix));
                let middle_pad = " ".repeat(prefix_max);
                let mut end = String::new();
                if parts.len() > 2 {
                    end = format!("\n  {middle_pad} {}", parts[2]);
                }
                format!("  {prefix}{prefix_pad} {middle}{end}")
            })
            .collect();

        message_lines.into_iter().collect::<Vec<_>>().join("\n")
    }
}
