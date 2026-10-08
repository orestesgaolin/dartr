//! Implements the operations traits for a toy type model and checks the
//! provided trait methods (the `TypeAnalyzerOperationsMixin` and
//! `NullShortingMixin` bodies, `MergedTypeConstraint`, `MatchContext`) and
//! the `Variance` lattice against the tables in the Dart doc comments.

use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

use dartr_flow::flow_analysis::FlowAnalysisNullShortingInterface;
use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
    TypeClassification,
};
use dartr_flow::null_shorting::TypeAnalysisNullShortingInterface;
use dartr_flow::shared_type::{
    SharedNamedFunctionParameter, SharedNamedType, SharedTypeKind, SharedTypeOperations,
    SharedTypeParameterView, SharedTypeSchemaView, SharedTypeView, Variance,
};
use dartr_flow::type_analysis_result::{
    ExpressionTypeAnalysisResult, MatchContext, UnnecessaryWildcardKind,
};
use dartr_flow::type_analyzer::JoinedPatternVariableInconsistency;
use dartr_flow::type_analyzer_operations::{
    ConstraintsMap, DataForTestingOf, KeyValueTypes, TypeAnalyzerOperations,
    TypeConstraintGenerator, TypeConstraintGeneratorState, TypeDeclarationKind,
    TypeDeclarationMatchResult,
};
use dartr_flow::type_constraint::{
    GeneratedTypeConstraint, MergedTypeConstraint, TypeConstraintOrigin,
};

// ------------------------------------------------------------------ toy model

/// A non-nullable base type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Base {
    Int,
    Num,
    Object,
}

impl Base {
    fn is_subtype_of(self, other: Base) -> bool {
        matches!(
            (self, other),
            (Base::Int, Base::Int | Base::Num | Base::Object)
                | (Base::Num, Base::Num | Base::Object)
                | (Base::Object, Base::Object)
        )
    }

    fn name(self) -> &'static str {
        match self {
            Base::Int => "int",
            Base::Num => "num",
            Base::Object => "Object",
        }
    }
}

/// The toy types: `Never`, `Null`, `dynamic`, `_` and `B` / `B?` for a base
/// type `B`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ty {
    Never,
    Null,
    Dynamic,
    Unknown,
    NonNull(Base),
    Nullable(Base),
}

use Ty::*;

const INT: Ty = NonNull(Base::Int);
const NUM: Ty = NonNull(Base::Num);
const OBJECT: Ty = NonNull(Base::Object);
const INT_Q: Ty = Nullable(Base::Int);
const NUM_Q: Ty = Nullable(Base::Num);

fn view(t: Ty) -> SharedTypeView<Ty> {
    SharedTypeView::new(t)
}

fn schema(t: Ty) -> SharedTypeSchemaView<Ty> {
    SharedTypeSchemaView::new(t)
}

/// The toy operations. Variables are `u32`, the only type parameter is
/// `'T'`.
struct ToyOps;

impl ToyOps {
    /// Subtyping with `_` as top on the right and bottom on the left (the
    /// schema rule).
    fn subtype(left: Ty, right: Ty) -> bool {
        match (left, right) {
            (Unknown, _) | (_, Unknown) => true,
            (Never, _) => true,
            (_, Dynamic) => true,
            (Dynamic, NonNull(Base::Object)) => false,
            (Dynamic, Nullable(Base::Object)) => true,
            (Dynamic, _) => false,
            (Null, Null | Nullable(_)) => true,
            (Null, _) => false,
            (NonNull(a), NonNull(b) | Nullable(b)) => a.is_subtype_of(b),
            (Nullable(a), Nullable(b)) => a.is_subtype_of(b),
            (Nullable(_), _) => false,
            (NonNull(_), _) => false,
        }
    }

    fn nullable(t: Ty) -> Ty {
        match t {
            Never | Null => Null,
            NonNull(b) | Nullable(b) => Nullable(b),
            Dynamic | Unknown => t,
        }
    }
}

impl SharedTypeOperations for ToyOps {
    type Type = Ty;
    type TypeParameter = char;
    type Name = &'static str;

    fn compare_names(&self, name1: &'static str, name2: &'static str) -> std::cmp::Ordering {
        name1.cmp(name2)
    }

    fn shared_type_kind(&self, ty: Ty) -> SharedTypeKind {
        match ty {
            Dynamic => SharedTypeKind::Dynamic,
            Null => SharedTypeKind::Null,
            Unknown => SharedTypeKind::Unknown,
            _ => SharedTypeKind::Other,
        }
    }

    fn is_question_type(&self, ty: Ty) -> bool {
        matches!(ty, Nullable(_))
    }

    fn as_question_type(&self, ty: Ty, is_question_type: bool) -> Ty {
        match (ty, is_question_type) {
            (NonNull(b), true) => Nullable(b),
            (Nullable(b), false) => NonNull(b),
            _ => ty,
        }
    }

    fn get_display_string(&self, ty: Ty) -> String {
        match ty {
            Never => "Never".into(),
            Null => "Null".into(),
            Dynamic => "dynamic".into(),
            Unknown => "_".into(),
            NonNull(b) => b.name().into(),
            Nullable(b) => format!("{}?", b.name()),
        }
    }

    fn is_structurally_equal_to(&self, ty: Ty, other: Ty) -> bool {
        ty == other
    }

    fn positional_parameter_types_shared(&self, _: Ty) -> Vec<Ty> {
        unreachable!("toy model has no function types")
    }

    fn required_positional_parameter_count(&self, _: Ty) -> usize {
        unreachable!("toy model has no function types")
    }

    fn return_type_shared(&self, _: Ty) -> Ty {
        unreachable!("toy model has no function types")
    }

    fn sorted_named_parameters_shared(
        &self,
        _: Ty,
    ) -> Vec<SharedNamedFunctionParameter<&'static str, Ty>> {
        unreachable!("toy model has no function types")
    }

    fn type_parameters_shared(&self, _: Ty) -> Vec<char> {
        unreachable!("toy model has no function types")
    }

    fn positional_types_shared(&self, _: Ty) -> Vec<Ty> {
        unreachable!("toy model has no record types")
    }

    fn sorted_named_types_shared(&self, _: Ty) -> Vec<SharedNamedType<&'static str, Ty>> {
        unreachable!("toy model has no record types")
    }

    fn bound_shared(&self, _: char) -> Option<Ty> {
        None
    }

    fn display_name(&self, type_parameter: char) -> String {
        type_parameter.to_string()
    }

    fn variance(&self, _: char) -> Variance {
        Variance::Covariant
    }

    fn is_legacy_covariant(&self, _: char) -> bool {
        true
    }

    fn invocation_structural_context_schema_return_type(&self, _: Ty) -> Ty {
        unreachable!("toy model has no structural context schemas")
    }

    fn lookup_structural_context_schema_lookup_name(&self, _: Ty) -> &'static str {
        unreachable!("toy model has no structural context schemas")
    }

    fn lookup_structural_context_schema_lookup_type(&self, _: Ty) -> Ty {
        unreachable!("toy model has no structural context schemas")
    }
}

impl FlowAnalysisTypeOperations for ToyOps {
    fn bool_type(&self) -> SharedTypeView<Ty> {
        unreachable!("toy model has no bool")
    }

    fn classify_type(&self, ty: SharedTypeView<Ty>) -> TypeClassification {
        match ty.unwrap_type_view() {
            Null => TypeClassification::NullOrEquivalent,
            Nullable(_) | Dynamic | Unknown => TypeClassification::PotentiallyNullable,
            Never | NonNull(_) => TypeClassification::NonNullable,
        }
    }

    fn extension_type_erasure(&self, ty: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        ty
    }

    fn factor(&self, from: SharedTypeView<Ty>, _: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        from
    }

    fn is_bottom_type(&self, ty: SharedTypeView<Ty>) -> bool {
        ty.unwrap_type_view() == Never
    }

    fn is_subtype_of(&self, left_type: SharedTypeView<Ty>, right_type: SharedTypeView<Ty>) -> bool {
        // As `TypeAnalyzerOperationsMixin.isSubtypeOf`.
        self.is_subtype_of_internal(left_type.unwrap_type_view(), right_type.unwrap_type_view())
    }

    fn is_type_parameter_type(&self, _: SharedTypeView<Ty>) -> bool {
        false
    }

    fn is_invalid_type(&self, _: SharedTypeView<Ty>) -> bool {
        false
    }

    fn make_nullable(&self, ty: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        // As `TypeAnalyzerOperationsMixin.makeNullable`.
        SharedTypeView::new(self.make_nullable_internal(ty.unwrap_type_view()))
    }

    fn promote_to_non_null(&self, ty: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        view(match ty.unwrap_type_view() {
            Null => Never,
            Nullable(b) => NonNull(b),
            t => t,
        })
    }

    fn try_promote_to_type(
        &self,
        to: SharedTypeView<Ty>,
        from: SharedTypeView<Ty>,
    ) -> Option<SharedTypeView<Ty>> {
        ToyOps::subtype(to.unwrap_type_view(), from.unwrap_type_view()).then_some(to)
    }
}

impl FlowAnalysisOperations for ToyOps {
    type Variable = u32;
    type PropertyMember = &'static str;

    fn is_final(&self, _: u32) -> bool {
        false
    }

    fn is_property_promotable(&self, _: &&'static str) -> bool {
        false
    }

    fn variable_type(&self, _: u32) -> SharedTypeView<Ty> {
        view(INT_Q)
    }

    fn why_property_is_not_promotable(
        &self,
        _: &&'static str,
    ) -> Option<PropertyNonPromotabilityReason> {
        Some(PropertyNonPromotabilityReason::IsNotPrivate)
    }
}

impl TypeAnalyzerOperations for ToyOps {
    type TypeDeclarationType = Ty;
    type TypeDeclaration = Base;
    type AstNode = u32;
    type ConstraintGenerator<'a> = ToyGenerator<'a>;

    fn double_type(&self) -> SharedTypeView<Ty> {
        unreachable!("toy model has no double")
    }

    fn dynamic_type(&self) -> SharedTypeView<Ty> {
        view(Dynamic)
    }

    fn error_type(&self) -> SharedTypeView<Ty> {
        view(Dynamic)
    }

    fn int_type(&self) -> SharedTypeView<Ty> {
        view(INT)
    }

    fn never_type(&self) -> SharedTypeView<Ty> {
        view(Never)
    }

    fn null_type(&self) -> SharedTypeView<Ty> {
        view(Null)
    }

    fn object_question_type(&self) -> SharedTypeView<Ty> {
        view(Nullable(Base::Object))
    }

    fn object_type(&self) -> SharedTypeView<Ty> {
        view(OBJECT)
    }

    fn unknown_type(&self) -> SharedTypeSchemaView<Ty> {
        schema(Unknown)
    }

    fn create_type_constraint_generator<'a>(
        &'a self,
        _: Option<&'a mut DataForTestingOf<Self>>,
        type_parameters_to_infer: &[SharedTypeParameterView<char>],
        inference_using_bounds_is_enabled: bool,
    ) -> ToyGenerator<'a> {
        ToyGenerator {
            operations: self,
            type_parameters: type_parameters_to_infer
                .iter()
                .map(|p| p.unwrap_type_parameter_view_as_type_parameter_structure())
                .collect(),
            constraints: Vec::new(),
            inference_using_bounds_is_enabled,
        }
    }

    fn flatten(&self, ty: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        ty
    }

    fn future_or_type_internal(&self, _: Ty) -> Ty {
        unreachable!("toy model has no FutureOr")
    }

    fn future_type_internal(&self, _: Ty) -> Ty {
        unreachable!("toy model has no Future")
    }

    fn get_type_declaration_kind_internal(&self, ty: Ty) -> Option<TypeDeclarationKind> {
        matches!(ty, NonNull(_) | Nullable(_)).then_some(TypeDeclarationKind::InterfaceDeclaration)
    }

    fn get_type_parameter_variance(&self, _: Base, _: usize) -> Variance {
        unreachable!("toy model has no generic classes")
    }

    fn glb_internal(&self, type1: Ty, type2: Ty) -> Ty {
        if ToyOps::subtype(type1, type2) {
            if type1 == Unknown { type2 } else { type1 }
        } else if ToyOps::subtype(type2, type1) {
            if type2 == Unknown { type1 } else { type2 }
        } else {
            Never
        }
    }

    fn greatest_closure_of_schema(
        &self,
        schema: SharedTypeSchemaView<Ty>,
        top_type: Option<SharedTypeView<Ty>>,
    ) -> SharedTypeView<Ty> {
        match schema.unwrap_type_schema_view() {
            Unknown => top_type.unwrap_or(view(Nullable(Base::Object))),
            t => view(t),
        }
    }

    fn greatest_closure_of_type_internal(&self, ty: Ty, _: &[char]) -> Ty {
        ty
    }

    fn is_always_exhaustive_type(&self, _: SharedTypeView<Ty>) -> bool {
        false
    }

    fn is_assignable_to(&self, from_type: SharedTypeView<Ty>, to_type: SharedTypeView<Ty>) -> bool {
        self.is_subtype_of(from_type, to_type)
    }

    fn is_bound_omitted(&self, _: char) -> bool {
        true
    }

    fn is_dart_core_function_internal(&self, _: Ty) -> bool {
        false
    }

    fn is_dart_core_record_internal(&self, _: Ty) -> bool {
        false
    }

    fn is_extension_type_internal(&self, _: Ty) -> bool {
        false
    }

    fn is_interface_type_internal(&self, ty: Ty) -> bool {
        matches!(ty, NonNull(_) | Nullable(_))
    }

    fn is_known_type(&self, type_schema: SharedTypeSchemaView<Ty>) -> bool {
        type_schema.unwrap_type_schema_view() != Unknown
    }

    fn is_non_nullable_internal(&self, ty: Ty) -> bool {
        matches!(ty, Never | NonNull(_))
    }

    fn is_nullable_internal(&self, ty: Ty) -> bool {
        matches!(ty, Null | Nullable(_) | Dynamic)
    }

    fn is_object(&self, ty: SharedTypeView<Ty>) -> bool {
        ty.unwrap_type_view() == OBJECT
    }

    fn is_subtype_of_internal(&self, left: Ty, right: Ty) -> bool {
        ToyOps::subtype(left, right)
    }

    fn is_type_schema_satisfied(
        &self,
        type_schema: SharedTypeSchemaView<Ty>,
        ty: SharedTypeView<Ty>,
    ) -> bool {
        self.type_is_subtype_of_type_schema(ty, type_schema)
    }

    fn is_variable_final(&self, node: u32) -> bool {
        self.is_final(node)
    }

    fn iterable_type_schema(&self, _: SharedTypeSchemaView<Ty>) -> SharedTypeSchemaView<Ty> {
        unreachable!("toy model has no Iterable")
    }

    fn least_closure_of_schema(&self, schema: SharedTypeSchemaView<Ty>) -> SharedTypeView<Ty> {
        match schema.unwrap_type_schema_view() {
            Unknown => view(Never),
            t => view(t),
        }
    }

    fn least_closure_of_type_internal(&self, ty: Ty, _: &[char]) -> Ty {
        ty
    }

    fn list_type_internal(&self, _: Ty) -> Ty {
        unreachable!("toy model has no List")
    }

    fn lub_internal(&self, type1: Ty, type2: Ty) -> Ty {
        if type1 == Unknown {
            return type2;
        }
        if type2 == Unknown {
            return type1;
        }
        if ToyOps::subtype(type1, type2) {
            type2
        } else if ToyOps::subtype(type2, type1) {
            type1
        } else if ToyOps::subtype(type1, ToyOps::nullable(type2)) {
            ToyOps::nullable(type2)
        } else if ToyOps::subtype(type2, ToyOps::nullable(type1)) {
            ToyOps::nullable(type1)
        } else {
            Nullable(Base::Object)
        }
    }

    fn make_nullable_internal(&self, ty: Ty) -> Ty {
        ToyOps::nullable(ty)
    }

    fn map_type_internal(&self, _: Ty, _: Ty) -> Ty {
        unreachable!("toy model has no Map")
    }

    fn match_future_or_internal(&self, _: Ty) -> Option<Ty> {
        None
    }

    fn match_inferable_parameter_internal(&self, _: Ty) -> Option<char> {
        None
    }

    fn match_iterable_type_internal(&self, _: Ty) -> Option<Ty> {
        None
    }

    fn match_list_type(&self, _: SharedTypeView<Ty>) -> Option<SharedTypeView<Ty>> {
        None
    }

    fn match_map_type(&self, _: SharedTypeView<Ty>) -> Option<KeyValueTypes<SharedTypeView<Ty>>> {
        None
    }

    fn match_stream_type(&self, _: SharedTypeView<Ty>) -> Option<SharedTypeView<Ty>> {
        None
    }

    fn match_type_declaration_type_internal(
        &self,
        ty: Ty,
    ) -> Option<TypeDeclarationMatchResult<Ty, Base, Ty>> {
        match ty {
            NonNull(b) | Nullable(b) => Some(TypeDeclarationMatchResult {
                type_declaration_kind: TypeDeclarationKind::InterfaceDeclaration,
                type_declaration_type: ty,
                type_declaration: b,
                type_arguments: Vec::new(),
            }),
            _ => None,
        }
    }

    fn match_type_parameter_bound_internal(&self, _: Ty) -> Option<Ty> {
        None
    }

    fn normalize(&self, ty: SharedTypeView<Ty>) -> SharedTypeView<Ty> {
        ty
    }

    fn record_type_internal(&self, _: &[Ty], _: &[(&'static str, Ty)]) -> Ty {
        unreachable!("toy model has no record types")
    }

    fn stream_type_schema(&self, _: SharedTypeSchemaView<Ty>) -> SharedTypeSchemaView<Ty> {
        unreachable!("toy model has no Stream")
    }

    fn substitute_type_from_iterables(&self, ty: Ty, _: &[char], _: &[Ty]) -> Ty {
        ty
    }

    fn lookup_member_type_internal(&self, _: Ty, _: &'static str) -> Option<Ty> {
        None
    }
}

/// A toy constraint generator: records the generated constraints in a list
/// (the state is the list length, as in the analyzer).
struct ToyGenerator<'a> {
    operations: &'a ToyOps,
    type_parameters: Vec<char>,
    constraints: Vec<GeneratedTypeConstraint<char, Ty>>,
    inference_using_bounds_is_enabled: bool,
}

impl TypeConstraintGenerator for ToyGenerator<'_> {
    type Operations = ToyOps;

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

    fn type_analyzer_operations(&self) -> &ToyOps {
        self.operations
    }

    fn type_parameters_to_constrain(&self) -> Vec<char> {
        self.type_parameters.clone()
    }

    fn add_lower_constraint_for_parameter(
        &mut self,
        type_parameter: char,
        lower: Ty,
        _: Option<u32>,
    ) {
        self.constraints.push(GeneratedTypeConstraint::lower(
            SharedTypeParameterView::new(type_parameter),
            schema(lower),
        ));
    }

    fn add_upper_constraint_for_parameter(
        &mut self,
        type_parameter: char,
        upper: Ty,
        _: Option<u32>,
    ) {
        self.constraints.push(GeneratedTypeConstraint::upper(
            SharedTypeParameterView::new(type_parameter),
            schema(upper),
        ));
    }

    fn compute_constraints(&mut self) -> ConstraintsMap<ToyOps> {
        let mut result: ConstraintsMap<ToyOps> = IndexMap::new();
        for p in &self.type_parameters {
            result.insert(
                *p,
                MergedTypeConstraint {
                    lower: schema(Unknown),
                    upper: schema(Unknown),
                    origin: TypeConstraintOrigin::Unknown,
                },
            );
        }
        for c in &self.constraints {
            let p = c
                .type_parameter
                .unwrap_type_parameter_view_as_type_parameter_structure();
            result.get_mut(&p).unwrap().merge_in(c, self.operations);
        }
        result
    }

    fn eliminate_type_parameters_in_generated_constraints(
        &mut self,
        _: &[char],
        _: TypeConstraintGeneratorState,
        _: Option<u32>,
    ) {
    }

    fn get_type_arguments_as_instance_of(&self, _: Ty, _: Base) -> Option<Vec<Ty>> {
        None
    }

    fn instantiate_function_types_and_provide_fresh_type_parameters(
        &mut self,
        _: Ty,
        _: Ty,
        _: bool,
    ) -> (Ty, Ty, Vec<char>) {
        unreachable!("toy model has no function types")
    }

    /// Toy rule, to observe which `left_schema` flag the mixin passes: with
    /// `left_schema` the schema `p` becomes a lower constraint of `'T'`,
    /// otherwise the schema `q` becomes an upper constraint of `'T'`.
    fn perform_subtype_constraint_generation_internal(
        &mut self,
        p: Ty,
        q: Ty,
        left_schema: bool,
        ast_node_for_testing: Option<u32>,
    ) -> bool {
        if left_schema {
            self.add_lower_constraint_for_parameter('T', p, ast_node_for_testing);
        } else {
            self.add_upper_constraint_for_parameter('T', q, ast_node_for_testing);
        }
        true
    }

    fn restore_state(&mut self, state: TypeConstraintGeneratorState) {
        self.constraints.truncate(state.count);
    }
}

// --------------------------------------------------------------- Variance

/// The lattice from the Dart doc comments:
///
/// ```text
///       unrelated
/// covariant   contravariant
///       invariant
/// ```
fn lattice_ge(a: Variance, b: Variance) -> bool {
    use Variance::*;
    a == b || a == Unrelated || b == Invariant
}

#[test]
fn variance_greater_than_or_equal_matches_the_lattice_diagram() {
    for a in Variance::VALUES {
        for b in Variance::VALUES {
            assert_eq!(
                a.greater_than_or_equal(b),
                lattice_ge(a, b),
                "{a:?} >= {b:?}"
            );
        }
    }
    assert!(!Variance::Covariant.greater_than_or_equal(Variance::Contravariant));
    assert!(!Variance::Contravariant.greater_than_or_equal(Variance::Covariant));
}

#[test]
fn variance_meet_is_the_greatest_lower_bound() {
    use Variance::*;
    for a in Variance::VALUES {
        for b in Variance::VALUES {
            let m = a.meet(b);
            assert_eq!(m, b.meet(a), "meet is commutative");
            assert!(
                a.greater_than_or_equal(m) && b.greater_than_or_equal(m),
                "{m:?} is below {a:?} and {b:?}"
            );
            for c in Variance::VALUES {
                if a.greater_than_or_equal(c) && b.greater_than_or_equal(c) {
                    assert!(
                        m.greater_than_or_equal(c),
                        "{c:?} <= meet({a:?}, {b:?}) = {m:?}"
                    );
                }
            }
        }
    }
    assert_eq!(Covariant.meet(Contravariant), Invariant);
    assert_eq!(Unrelated.meet(Covariant), Covariant);
    assert_eq!(Unrelated.meet(Unrelated), Unrelated);
}

#[test]
fn variance_combine_matches_the_dart_examples() {
    use Variance::*;
    // `X` in `Function(X)` is contravariant, `Y` in `List<Y>` covariant:
    // `X` in `List<Function(X)>` is contravariant.
    assert_eq!(Contravariant.combine(Covariant), Contravariant);
    // `X` in `List<X>`, `Y` in `Function(Y)`: `Function(List<X>)`.
    assert_eq!(Covariant.combine(Contravariant), Contravariant);
    // `Function(Function(X))` is covariant.
    assert_eq!(Contravariant.combine(Contravariant), Covariant);
    // `typedef F<Z> = Function()`: `X` in `List<F<X>>` is unrelated.
    assert_eq!(Unrelated.combine(Covariant), Unrelated);
    // `typedef G<Z> = Z Function(Z)`: `X` in `G<List<X>>` is invariant.
    assert_eq!(Covariant.combine(Invariant), Invariant);
    // Unrelated wins over invariant.
    assert_eq!(Invariant.combine(Unrelated), Unrelated);
    for a in Variance::VALUES {
        for b in Variance::VALUES {
            assert_eq!(a.combine(b), b.combine(a), "combine is commutative");
        }
    }
}

#[test]
fn variance_keywords_and_encodings_round_trip() {
    for v in [
        Variance::Covariant,
        Variance::Contravariant,
        Variance::Invariant,
    ] {
        assert_eq!(Variance::from_keyword_string(v.keyword()), Some(v));
    }
    assert_eq!(
        Variance::from_keyword_string("unrelated"),
        Some(Variance::Unrelated)
    );
    assert_eq!(Variance::from_keyword_string(""), None);
    assert_eq!(Variance::from_keyword_string("covariant"), None);
    for v in Variance::VALUES {
        assert_eq!(Variance::from_encoding(v.index()), v);
    }
}

// ------------------------------------------------- TypeAnalyzerOperations

#[test]
fn mixin_wrappers_keep_types_and_schemas_apart_but_share_the_structure() {
    let ops = ToyOps;
    assert_eq!(ops.make_nullable(view(INT)), view(INT_Q));
    assert_eq!(ops.make_type_schema_nullable(schema(NUM)), schema(NUM_Q));
    assert_eq!(
        ops.make_type_schema_nullable(schema(Unknown)),
        schema(Unknown)
    );
    assert_eq!(ops.lub(view(INT), view(Null)), view(INT_Q));
    assert_eq!(
        ops.type_schema_lub(schema(Unknown), schema(INT)),
        schema(INT)
    );
    assert_eq!(ops.glb(view(INT_Q), view(NUM_Q)), view(INT_Q));
    assert_eq!(ops.glb(view(INT_Q), view(NUM)), view(Never));
    assert_eq!(
        ops.type_schema_glb(schema(NUM), schema(Unknown)),
        schema(NUM)
    );
    assert_eq!(ops.type_to_schema(view(NUM_Q)), schema(NUM_Q));
    assert_eq!(
        ops.get_type_declaration_kind(view(INT_Q)),
        Some(TypeDeclarationKind::InterfaceDeclaration)
    );
    assert_eq!(ops.get_type_schema_declaration_kind(schema(Unknown)), None);
    assert_eq!(ops.match_future_or(view(INT)), None);
}

#[test]
fn schema_subtype_queries_treat_unknown_as_top_on_the_right_and_bottom_on_the_left() {
    let ops = ToyOps;
    assert!(ops.type_is_subtype_of_type_schema(view(INT), schema(NUM)));
    assert!(!ops.type_is_subtype_of_type_schema(view(NUM), schema(INT)));
    assert!(ops.type_is_subtype_of_type_schema(view(OBJECT), schema(Unknown)));
    assert!(ops.type_schema_is_subtype_of_type(schema(Unknown), view(INT)));
    assert!(!ops.type_schema_is_subtype_of_type(schema(INT_Q), view(INT)));
    assert!(ops.type_schema_is_subtype_of_type_schema(schema(Null), schema(INT_Q)));
    assert!(ops.is_subtype_of(view(Never), view(INT)));
}

fn constraint(lower: Ty, upper: Ty) -> MergedTypeConstraint<Ty> {
    MergedTypeConstraint {
        lower: schema(lower),
        upper: schema(upper),
        origin: TypeConstraintOrigin::Unknown,
    }
}

#[test]
fn choose_type_from_constraint_prefers_the_known_bound_on_the_variance_side() {
    let ops = ToyOps;
    // Covariant: the lower bound wins.
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(INT, NUM), false, false),
        INT
    );
    // Contravariant: the upper bound wins.
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(INT, NUM), false, true),
        NUM
    );
    // Only one bound known.
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(Unknown, NUM), false, false),
        NUM
    );
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(INT, Unknown), false, true),
        INT
    );
    // Unconstrained: `_`, also when grounded (Dart returns the unknown
    // bound itself).
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(Unknown, Unknown), true, false),
        Unknown
    );
    assert_eq!(
        ops.choose_type_from_constraint(&constraint(Unknown, Unknown), true, true),
        Unknown
    );
}

#[test]
fn merged_type_constraint_merges_lower_with_lub_and_upper_with_glb() {
    let ops = ToyOps;
    let p = SharedTypeParameterView::new('T');
    let mut c = constraint(Unknown, Unknown);
    assert!(c.is_empty(&ops));
    c.merge_in(&GeneratedTypeConstraint::lower(p, schema(INT)), &ops);
    c.merge_in(&GeneratedTypeConstraint::lower(p, schema(Null)), &ops);
    assert_eq!(c.lower, schema(INT_Q));
    c.merge_in(
        &GeneratedTypeConstraint::upper(p, schema(Nullable(Base::Object))),
        &ops,
    );
    c.merge_in(&GeneratedTypeConstraint::upper(p, schema(NUM_Q)), &ops);
    assert_eq!(c.upper, schema(NUM_Q));
    assert!(!c.is_empty(&ops));
    assert!(c.is_satisfied_by(view(INT_Q), &ops));
    assert!(c.is_satisfied_by(view(NUM_Q), &ops));
    assert!(
        !c.is_satisfied_by(view(INT), &ops),
        "int does not include the lower bound Null"
    );
    assert!(!c.is_satisfied_by(view(Nullable(Base::Object)), &ops));
}

#[test]
fn merged_type_constraint_from_extends_has_the_extends_type_as_upper_bound() {
    let ops = ToyOps;
    let c = MergedTypeConstraint::from_extends("T".to_string(), view(NUM), view(NUM_Q), &ops);
    assert_eq!(c.lower, schema(Unknown));
    assert_eq!(c.upper, schema(NUM_Q));
    assert_eq!(
        c.origin.format_error(&ops),
        vec![
            "Type parameter 'T'".to_string(),
            "is declared to extend 'num' producing 'num?'.".to_string()
        ]
    );
    let from_argument = TypeConstraintOrigin::FromArgument {
        argument_type: view(INT_Q),
        parameter_type: view(NUM),
        parameter_name: "x".into(),
    };
    assert_eq!(
        from_argument.format_error(&ops),
        vec![
            "Parameter 'x'",
            "declared as     'num'",
            "but argument is 'int?'."
        ]
    );
}

#[test]
fn constraint_generator_mixin_passes_the_schema_side_and_state_rolls_back() {
    let ops = ToyOps;
    let mut generator =
        ops.create_type_constraint_generator(None, &[SharedTypeParameterView::new('T')], false);
    let start = generator.current_state();
    assert!(generator.perform_subtype_constraint_generation_left_schema(
        schema(INT),
        view(Unknown),
        Some(1)
    ));
    assert!(
        generator.perform_subtype_constraint_generation_right_schema(
            view(Unknown),
            schema(NUM_Q),
            Some(2)
        )
    );
    let constraints = generator.compute_constraints();
    assert_eq!(constraints[&'T'], constraint(INT, NUM_Q));
    generator.restore_state(start);
    assert_eq!(
        generator.compute_constraints()[&'T'],
        constraint(Unknown, Unknown)
    );
}

// ------------------------------------------------------------ null shorting

#[derive(Debug, PartialEq, Eq)]
enum FlowEvent {
    RightBegin(Option<&'static str>, Ty, Option<u32>),
    End,
}

#[derive(Default)]
struct ToyFlow {
    events: Vec<FlowEvent>,
}

impl FlowAnalysisNullShortingInterface for ToyFlow {
    type Expression = u32;
    type Variable = u32;
    type Type = Ty;
    type ExpressionInfo = &'static str;

    fn null_aware_access_end(&mut self) {
        self.events.push(FlowEvent::End);
    }

    fn null_aware_access_right_begin(
        &mut self,
        target_info: Option<&'static str>,
        target_type: SharedTypeView<Ty>,
        guard_variable: Option<u32>,
    ) -> Option<&'static str> {
        self.events.push(FlowEvent::RightBegin(
            target_info,
            target_type.unwrap_type_view(),
            guard_variable,
        ));
        target_info.map(|_| "non-null target")
    }
}

#[derive(Default)]
struct ToyAnalyzer {
    flow: ToyFlow,
    guards: Vec<&'static str>,
    steps: Vec<&'static str>,
    finished: Vec<Ty>,
}

impl TypeAnalysisNullShortingInterface for ToyAnalyzer {
    type Expression = u32;
    type Variable = u32;
    type Operations = ToyOps;
    type Flow = ToyFlow;
    type Guard = &'static str;

    fn flow(&mut self) -> &mut ToyFlow {
        &mut self.flow
    }

    fn operations(&self) -> &ToyOps {
        &ToyOps
    }

    fn guards(&self) -> &[&'static str] {
        &self.guards
    }

    fn guards_mut(&mut self) -> &mut Vec<&'static str> {
        &mut self.guards
    }

    fn handle_null_shorting_step(
        &mut self,
        _: ExpressionTypeAnalysisResult<Ty, &'static str>,
        guard: &'static str,
        inferred_type: SharedTypeView<Ty>,
    ) -> ExpressionTypeAnalysisResult<Ty, &'static str> {
        self.steps.push(guard);
        ExpressionTypeAnalysisResult::new(inferred_type)
    }

    fn handle_null_shorting_finished(&mut self, inferred_type: SharedTypeView<Ty>) {
        self.finished.push(inferred_type.unwrap_type_view());
    }
}

#[test]
fn null_shorting_terminates_down_to_the_target_depth_innermost_first() {
    let mut a = ToyAnalyzer::default();
    // `a?.b?.c`: two null-aware operators.
    let target = a.start_null_shorting("a?.", Some("a"), view(NUM_Q), Some(7));
    assert_eq!(target, Some("non-null target"));
    a.start_null_shorting("b?.", None, view(INT_Q), None);
    a.start_null_shorting("c?.", None, view(INT_Q), None);
    assert_eq!(a.null_shorting_depth(), 3);

    // Terminate the two inner ones.
    let inner = ExpressionTypeAnalysisResult::new(view(INT));
    let result = a.finish_null_shorting(1, inner, 0);
    assert_eq!(
        result.type_,
        view(INT_Q),
        "the result type is made nullable"
    );
    assert_eq!(a.null_shorting_depth(), 1);
    assert_eq!(a.steps, vec!["c?.", "b?."]);
    assert_eq!(
        a.finished,
        vec![INT_Q],
        "finished hook runs once per finish_null_shorting"
    );
    assert_eq!(
        a.flow.events,
        vec![
            FlowEvent::RightBegin(Some("a"), NUM_Q, Some(7)),
            FlowEvent::RightBegin(None, INT_Q, None),
            FlowEvent::RightBegin(None, INT_Q, None),
            FlowEvent::End,
            FlowEvent::End,
        ]
    );

    // Terminate the last one.
    a.finish_null_shorting(0, ExpressionTypeAnalysisResult::new(view(Null)), 0);
    assert_eq!(a.null_shorting_depth(), 0);
    assert_eq!(a.steps, vec!["c?.", "b?.", "a?."]);
    assert_eq!(a.finished, vec![INT_Q, Null]);
}

// ------------------------------------------------------ type analyzer data

#[test]
fn joined_pattern_variable_inconsistency_keeps_the_most_serious() {
    use JoinedPatternVariableInconsistency::*;
    // Dart severities: logicalOr 4, sharedCaseAbsent 3, sharedCaseHasLabel 2,
    // differentFinalityOrType 1, none 0.
    assert_eq!(
        None.max_with_all([DifferentFinalityOrType, SharedCaseHasLabel]),
        SharedCaseHasLabel
    );
    assert_eq!(
        DifferentFinalityOrType.max_with_all([LogicalOr, SharedCaseAbsent]),
        LogicalOr
    );
    assert_eq!(
        SharedCaseAbsent.max_with(SharedCaseHasLabel),
        SharedCaseAbsent
    );
    assert_eq!(None.max_with_all([]), None);
}

#[test]
fn match_context_copies_share_the_variable_maps() {
    let component_variables = Rc::new(RefCell::new(Vec::new()));
    let keys = Rc::new(RefCell::new(vec![("x", 1u32)]));
    let context: MatchContext<u32, u32, u32, u32, &'static str> = MatchContext {
        irrefutable_context: Some(10),
        is_final: true,
        switch_scrutinee: Some(20),
        assigned_variables: None,
        component_variables: Rc::clone(&component_variables),
        pattern_variable_promotion_keys: Rc::clone(&keys),
        unnecessary_wildcard_kind: Some(UnnecessaryWildcardKind::LogicalAndPatternOperand),
    };

    let refutable = context.make_refutable();
    assert_eq!(refutable.irrefutable_context, None);
    assert_eq!(refutable.switch_scrutinee, Some(20));
    assert_eq!(
        refutable.unnecessary_wildcard_kind, None,
        "Dart's makeRefutable drops it"
    );
    refutable
        .component_variables
        .borrow_mut()
        .push(("y", vec![5]));
    assert_eq!(
        context.component_variables.borrow().len(),
        1,
        "the copy writes into the shared map"
    );

    let new_keys = Rc::new(RefCell::new(Vec::new()));
    let with_keys = context.with_promotion_keys(Rc::clone(&new_keys));
    assert_eq!(with_keys.switch_scrutinee, None);
    assert_eq!(
        with_keys.unnecessary_wildcard_kind,
        Some(UnnecessaryWildcardKind::LogicalAndPatternOperand)
    );
    with_keys
        .pattern_variable_promotion_keys
        .borrow_mut()
        .push(("z", 3));
    assert_eq!(
        *keys.borrow(),
        vec![("x", 1)],
        "the old keys map is not touched"
    );
    assert_eq!(*new_keys.borrow(), vec![("z", 3)]);

    let without_wildcard = context.with_unnecessary_wildcard_kind(None);
    assert_eq!(without_wildcard.irrefutable_context, Some(10));
    assert_eq!(without_wildcard.switch_scrutinee, None);
}
