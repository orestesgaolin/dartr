// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart

//! The bodies of the concrete members of the `TypeAnalyzer` mixin.
//!
//! The mixin's interface (abstract members, data types, errors) is
//! [`dartr_flow::type_analyzer::TypeAnalyzer`]. Each `analyzeX` method of the
//! mixin is a free function here, named after the Dart method; the private
//! helpers (`_analyzeIfCommon`, `_checkGuardType`, ...) are private
//! functions. A client applies the mixin with [`type_analyzer_mixin!`]
//! (see the crate docs).
//!
//! Stack effects are documented in the Dart source and in
//! [`dartr_flow::type_analyzer`]; they are not repeated here.
//!
//! # Node kinds
//!
//! In Dart, `Statement`, `Expression` and `Pattern` are subtypes of `Node`,
//! so a statement can be passed where a node is expected. In Rust they are
//! different associated types, and [`SharedTypeAnalyzer`] converts them
//! (the client provides `Into<Node>` conversions).

use dartr_flow::flow_analysis::{FlowAnalysis, PromotionKey};
use dartr_flow::flow_analysis_operations::{FlowAnalysisOperations, FlowAnalysisTypeOperations};
use dartr_flow::null_shorting::{
    ExpressionInfoOf, ExpressionResultOf, TypeAnalysisNullShortingInterface,
};
use dartr_flow::shared_type::{
    NameOf, SchemaView, SharedTypeKind, SharedTypeOperations, TypeOf, TypeView,
};
use dartr_flow::type_analysis_result::{
    AssignedVariablePatternResult, AwaitExpressionResult, ConstantPatternResult,
    DeclaredVariablePatternResult, IfCaseStatementResult, IntTypeAnalysisResult, ListPatternResult,
    LogicalOrPatternResult, MapPatternResult, MatchContext, NullCheckOrAssertPatternResult,
    ObjectPatternResult, PatternAssignmentAnalysisResult, PatternForInResult, PatternResult,
    PatternVariableDeclarationAnalysisResult, RecordPatternResult, RelationalPatternResult,
    SharedMap, SwitchExpressionResult, SwitchStatementTypeAnalysisResult, UnnecessaryWildcardKind,
    WildcardPatternResult, YieldStatementResult,
};
use dartr_flow::body_inference_context::SharedBodyInferenceContext;
use dartr_flow::type_analyzer::{
    JoinedPatternVariableInconsistency, JoinedPatternVariableLocation, MatchContextOf,
    PropertyMemberOf, RecordPatternFieldOf, RelationalOperatorKind, TypeAnalyzer,
    TypeAnalyzerErrors, TypeAnalyzerErrorsBase,
};
use dartr_flow::type_analyzer_operations::{KeyValueTypes, TypeAnalyzerOperations};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The operations of a type analyzer `A`.
type Ops<A> = <A as TypeAnalysisNullShortingInterface>::Operations;

/// The type structure of a type analyzer `A`.
type Ty<A> = TypeOf<Ops<A>>;

/// A [`TypeAnalyzer`] whose statements, expressions and patterns convert to
/// its nodes (Dart: `Statement extends Node`, `Expression extends Node`,
/// `Pattern extends Node`).
///
/// Implemented for every [`TypeAnalyzer`] whose `Statement`, `Expression`
/// and `Pattern` types implement `Into<Node>`, and whose name type
/// implements `Default` (the default is the empty name, Dart `''`).
pub trait SharedTypeAnalyzer: TypeAnalyzer {
    /// The empty name (Dart `''`), used for error recovery.
    fn empty_name() -> NameOf<Ops<Self>>;
    /// Converts an expression to a node.
    fn expression_node(expression: Self::Expression) -> Self::Node;
    /// Converts a statement to a node.
    fn statement_node(statement: Self::Statement) -> Self::Node;
    /// Converts a pattern to a node.
    fn pattern_node(pattern: Self::Pattern) -> Self::Node;
}

impl<A: TypeAnalyzer + ?Sized> SharedTypeAnalyzer for A
where
    A::Expression: Into<A::Node>,
    A::Statement: Into<A::Node>,
    A::Pattern: Into<A::Node>,
    NameOf<Ops<A>>: Default,
{
    fn empty_name() -> NameOf<Ops<Self>> {
        Default::default()
    }
    fn expression_node(expression: Self::Expression) -> Self::Node {
        expression.into()
    }
    fn statement_node(statement: Self::Statement) -> Self::Node {
        statement.into()
    }
    fn pattern_node(pattern: Self::Pattern) -> Self::Node {
        pattern.into()
    }
}

// ------------------------------------------------------------ map helpers

/// Dart `map[key]` on a [`SharedMap`].
fn map_get<K: PartialEq, V: Clone>(map: &SharedMap<K, V>, key: &K) -> Option<V> {
    map.borrow()
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// Dart `map[key] = value` on a [`SharedMap`] (keeps the position of an
/// existing key, as a Dart `LinkedHashMap` does).
fn map_set<K: PartialEq, V>(map: &SharedMap<K, V>, key: K, value: V) {
    let mut map = map.borrow_mut();
    if let Some(entry) = map.iter_mut().find(|(k, _)| *k == key) {
        entry.1 = value;
    } else {
        map.push((key, value));
    }
}

/// Dart `map.containsKey(key)` on a [`SharedMap`].
fn map_contains_key<K: PartialEq, V>(map: &SharedMap<K, V>, key: &K) -> bool {
    map.borrow().iter().any(|(k, _)| k == key)
}

/// Dart `{}` as a [`SharedMap`].
fn new_shared_map<K, V>() -> SharedMap<K, V> {
    Rc::new(RefCell::new(Vec::new()))
}

/// Dart `list[key]` lookup on a list of pairs in map order.
fn pairs_get<K: PartialEq, V: Clone>(pairs: &[(K, V)], key: &K) -> Option<V> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Creates the [`MatchContext`] of a top-level pattern (Dart `new
/// MatchContext(...)` with `unnecessaryWildcardKind: null`).
fn new_match_context<A: SharedTypeAnalyzer + ?Sized>(
    is_final: bool,
    irrefutable_context: Option<A::Node>,
    switch_scrutinee: Option<A::Expression>,
    assigned_variables: Option<SharedMap<A::Variable, A::Pattern>>,
    component_variables: SharedMap<NameOf<Ops<A>>, Vec<A::Variable>>,
    pattern_variable_promotion_keys: SharedMap<NameOf<Ops<A>>, PromotionKey>,
) -> MatchContextOf<A> {
    MatchContext {
        irrefutable_context,
        is_final,
        switch_scrutinee,
        assigned_variables,
        component_variables,
        pattern_variable_promotion_keys,
        unnecessary_wildcard_kind: None,
    }
}

/// `type is SharedXType`.
fn kind_of<A: SharedTypeAnalyzer + ?Sized>(a: &A, ty: TypeView<Ops<A>>) -> SharedTypeKind {
    a.operations().shared_type_kind(ty.unwrap_type_view())
}

/// `schema is SharedXTypeSchemaView`.
fn schema_kind_of<A: SharedTypeAnalyzer + ?Sized>(
    a: &A,
    schema: SchemaView<Ops<A>>,
) -> SharedTypeKind {
    a.operations()
        .shared_type_kind(schema.unwrap_type_schema_view())
}

// --------------------------------------------------------- analyze methods

/// Analyzes a non-wildcard variable pattern appearing in an assignment
/// context. `node` is the pattern itself, and `variable` is the variable
/// being referenced.
///
/// For wildcard patterns in an assignment context,
/// `analyzeDeclaredVariablePattern` should be used instead.
pub fn analyze_assigned_variable_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    variable: A::Variable,
) -> AssignedVariablePatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let mut duplicate_assignment_pattern_variable_error = None;
    if let Some(assigned_variables) = &context.assigned_variables {
        let original = map_get(assigned_variables, &variable);
        match original {
            None => map_set(assigned_variables, variable, node),
            Some(original) => {
                duplicate_assignment_pattern_variable_error = Some(
                    a.errors()
                        .duplicate_assignment_pattern_variable(variable, original, node),
                );
            }
        }
    }

    let variable_declared_type = a.operations().variable_type(variable);
    let irrefutable_context = context.irrefutable_context;
    debug_assert!(
        irrefutable_context.is_some(),
        "Assigned variables must only appear in irrefutable pattern contexts"
    );
    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = irrefutable_context {
        let kind = kind_of(a, matched_value_type);
        if kind != SharedTypeKind::Dynamic
            && kind != SharedTypeKind::Invalid
            && !a
                .operations()
                .is_subtype_of(matched_value_type, variable_declared_type)
        {
            pattern_type_mismatch_in_irrefutable_context_error =
                Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                    node,
                    irrefutable_context,
                    matched_value_type,
                    variable_declared_type,
                ));
        }
    }
    a.flow()
        .promote_for_pattern(matched_value_type, variable_declared_type, true, false);
    a.flow()
        .assigned_variable_pattern(A::pattern_node(node), variable, matched_value_type);
    AssignedVariablePatternResult {
        duplicate_assignment_pattern_variable_error,
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Analyzes an expression of the form `await operand`.
///
/// Returns an [`AwaitExpressionResult`] containing the static type of the
/// await expression and its operand.
pub fn analyze_await_expression<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Expression,
    operand: A::Expression,
    schema: SchemaView<Ops<A>>,
) -> AwaitExpressionResult<Ty<A>, ExpressionInfoOf<A>> {
    // Stack: ()

    // (Note: comments pulled from
    // https://github.com/dart-lang/language/blob/main/resources/type-system/inference.md)

    // Expression inference of an await expression await e_1, in context K,
    // produces an elaborated expression m with static type T, where m and T
    // are determined as follows:
    let k = schema;

    // Define K_1 as follows:
    // - If K is FutureOr<S> or FutureOr<S>? for some type schema S, then let
    //   K_1 be K.
    // - Otherwise, if K is dynamic, then let K_1 be FutureOr<_>.
    // - Otherwise, let K_1 be FutureOr<K>.
    debug_assert!(
        schema_kind_of(a, schema) != SharedTypeKind::Dynamic,
        "Caller should convert dynamic context to _"
    );
    let k1 = if a.operations().match_type_schema_future_or(k).is_some() {
        k
    } else {
        a.operations().future_or_type_schema(k)
    };

    // Let m_1 be the result of performing expression inference on e_1, in
    // context K_1.
    let m1 = a.analyze_expression(operand, k1, false, false, false);
    // Stack: (operand)

    a.flow().suspension(A::expression_node(node));

    // Let T_1 be the static type of m_1.
    let t1 = m1.type_;

    // If T_1 is incompatible with await (as defined in the extension types
    // specification), then there is a compile-time error.
    // (Currently this error is detected by the analyzer and front_end
    // clients, not by shared code. TODO(paulberry): share this logic.)

    // Let T_2 be flatten(T_1).
    let t2 = a.operations().flatten(t1);

    // Let m_2 be @AWAIT_WITH_TYPE_CHECK(m_1), with static type Future<T_2>.

    // Let T be T_2, and let m be `await m_2`.
    AwaitExpressionResult {
        type_: t2,
        flow_analysis_info: None,
        operand_type: t1,
    }
}

/// Analyzes a cast pattern. `inner_pattern` is the sub-pattern and
/// `required_type` is the type to cast to.
pub fn analyze_cast_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    pattern: A::Pattern,
    inner_pattern: A::Pattern,
    required_type: TypeView<Ops<A>>,
) -> PatternResult<Ty<A>> {
    let matched_value_type = a.flow().get_matched_value_type();
    a.flow()
        .promote_for_pattern(matched_value_type, required_type, false, false);
    if a.operations()
        .is_subtype_of(matched_value_type, required_type)
        && kind_of(a, required_type) != SharedTypeKind::Invalid
    {
        a.errors()
            .matched_type_is_subtype_of_required(pattern, matched_value_type, required_type);
    }
    // Note: although technically the inner pattern match of a cast-pattern
    // operates on the same value as the cast pattern does, we analyze it as
    // though it's a different value; this ensures that (a) the matched value
    // type when matching the inner pattern is precisely the cast type, and
    // (b) promotions triggered by the inner pattern have no effect outside
    // the cast.
    a.flow().push_subpattern(required_type);
    a.dispatch_pattern(
        &context.with_unnecessary_wildcard_kind(None),
        A::pattern_node(inner_pattern),
    );
    // Stack: (Pattern)
    a.flow().pop_subpattern();
    PatternResult { matched_value_type }
}

/// Analyzes a constant pattern. `node` is the pattern itself, and
/// `expression` is the constant expression. Depending on the client's
/// representation, `node` and `expression` might or might not be identical.
///
/// Returns a [`ConstantPatternResult`] with the static type of `expression`
/// and information about reported errors.
pub fn analyze_constant_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Node,
    expression: A::Expression,
) -> ConstantPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    // Stack: ()
    let mut refutable_pattern_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context {
        refutable_pattern_in_irrefutable_context_error = Some(
            a.errors()
                .refutable_pattern_in_irrefutable_context(node, irrefutable_context),
        );
    }
    let schema = a.operations().type_to_schema(matched_value_type);
    let expression_analysis_result = a.analyze_expression(expression, schema, false, true, false);
    let expression_type = expression_analysis_result.type_;
    let patterns_enabled = a.type_analyzer_options().patterns_enabled;
    a.flow().constant_pattern_end(
        expression_analysis_result.flow_analysis_info,
        expression_type,
        patterns_enabled,
        matched_value_type,
    );
    // Stack: (Expression)
    let mut case_expression_type_mismatch_error = None;
    if !a.type_analyzer_options().patterns_enabled
        && let Some(switch_scrutinee) = context.switch_scrutinee
    {
        let matches = a
            .operations()
            .is_subtype_of(expression_type, matched_value_type);
        if !matches {
            case_expression_type_mismatch_error = Some(a.errors().case_expression_type_mismatch(
                switch_scrutinee,
                expression,
                matched_value_type,
                expression_type,
            ));
        }
    }
    ConstantPatternResult {
        expression_type,
        refutable_pattern_in_irrefutable_context_error,
        case_expression_type_mismatch_error,
        matched_value_type,
    }
}

/// Analyzes a variable pattern in a non-assignment context. `node` is the
/// pattern itself, `variable` is the variable, `declared_type` is the
/// explicitly declared type (if present). `variable_name` is the name of the
/// variable; this is used to match up corresponding variables in the
/// different branches of logical-or patterns, as well as different switch
/// cases that share a body.
///
/// Returns a [`DeclaredVariablePatternResult`] with the static type of the
/// variable (possibly inferred) and information about reported errors.
pub fn analyze_declared_variable_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    variable: A::Variable,
    variable_name: NameOf<Ops<A>>,
    declared_type: Option<TypeView<Ops<A>>>,
) -> DeclaredVariablePatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let static_type = match declared_type {
        Some(declared_type) => declared_type,
        None => a.variable_type_from_initializer_type(matched_value_type),
    };
    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context {
        let kind = kind_of(a, matched_value_type);
        if kind != SharedTypeKind::Dynamic
            && kind != SharedTypeKind::Invalid
            && !a.operations().is_subtype_of(matched_value_type, static_type)
        {
            pattern_type_mismatch_in_irrefutable_context_error =
                Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                    node,
                    irrefutable_context,
                    matched_value_type,
                    static_type,
                ));
        }
    }
    a.flow()
        .promote_for_pattern(matched_value_type, static_type, true, false);
    // The promotion may have made the matched type even more specific than
    // either `matchedType` or `staticType`, so fetch it again and use that
    // in the call to `declaredVariablePattern` below.
    let promoted_value_type = a.flow().get_matched_value_type();
    let is_implicitly_typed = declared_type.is_none();
    // TODO(paulberry): are we handling _isFinal correctly?
    let is_final = context.is_final || a.operations().is_variable_final(variable);
    let promotion_key = a.flow().declared_variable_pattern(
        promoted_value_type,
        static_type,
        is_final,
        false,
        is_implicitly_typed,
    );
    map_set(
        &context.pattern_variable_promotion_keys,
        variable_name,
        promotion_key,
    );
    a.set_variable_type(variable, static_type);
    {
        let mut component_variables = context.component_variables.borrow_mut();
        if let Some(entry) = component_variables
            .iter_mut()
            .find(|(name, _)| *name == variable_name)
        {
            entry.1.push(variable);
        } else {
            component_variables.push((variable_name, vec![variable]));
        }
    }
    a.flow()
        .assign_matched_pattern_variable(variable, promotion_key);
    DeclaredVariablePatternResult {
        static_type,
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Analyzes a dot shorthand. Saves the `context` for when we resolve the
/// dot shorthand head.
pub fn analyze_dot_shorthand<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Expression,
    context: SchemaView<Ops<A>>,
) -> TypeView<Ops<A>> {
    a.push_dot_shorthand_context(A::expression_node(node), context);
    let analysis_result = a.dispatch_expression(node, context, true, false);
    a.pop_dot_shorthand_context();
    analysis_result.type_
}

/// Analyzes an expression. `node` is the expression to analyze, and
/// `schema` is the type schema which should be used for type inference.
///
/// If `continue_null_shorting` is `false`, then any null shorting that
/// starts inside `node` will be terminated, and the returned type will be
/// nullable, to reflect the fact that null-aware expressions might evaluate
/// to `null`. If it is `true`, then null shorting that starts inside `node`
/// will be allowed to continue into the containing expression.
///
/// If `is_void_allowed` is `false`, and the static type of the expression is
/// void, an error will be reported.
///
/// If `needs_coercion` is `true`, the expression is coerced on assignment.
/// This is used by the CFE which performs expression coercion in the process
/// of type inference of the nodes where an assignment is executed.
pub fn analyze_expression<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Expression,
    schema: SchemaView<Ops<A>>,
    continue_null_shorting: bool,
    is_void_allowed: bool,
    needs_coercion: bool,
) -> ExpressionResultOf<A> {
    let mut null_shorting_target_depth = None;
    if !continue_null_shorting {
        null_shorting_target_depth = Some(a.null_shorting_depth());
    }
    // Stack: ()
    let mut schema = schema;
    if schema_kind_of(a, schema) == SharedTypeKind::Dynamic {
        schema = a.operations().unknown_type();
    }
    let mut result = a.dispatch_expression(node, schema, is_void_allowed, needs_coercion);
    // Stack: (Expression)
    if a.operations().is_bottom_type(result.type_) {
        a.flow().handle_exit();
    }
    if let Some(null_shorting_target_depth) = null_shorting_target_depth
        && a.null_shorting_depth() > null_shorting_target_depth
    {
        result = a.finish_null_shorting(null_shorting_target_depth, result, node);
    }
    result
}

/// Analyzes a collection element of the form `if (expression case pattern)
/// ifTrue` or `if (expression case pattern) ifTrue else ifFalse`.
///
/// `variables` maps the variable names to the variables the client wishes
/// to use to represent them (joins of logical-or branches).
///
/// Returns a [`IfCaseStatementResult`] with the static type of `expression`
/// and information about reported errors.
pub fn analyze_if_case_element<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    expression: A::Expression,
    pattern: A::Pattern,
    variables: &[(NameOf<Ops<A>>, A::Variable)],
    guard: Option<A::Expression>,
    if_true: A::Node,
    if_false: Option<A::Node>,
    context: A::CollectionElementContext,
) -> IfCaseStatementResult<Ty<A>, A::Error> {
    // Stack: ()
    a.flow().if_case_statement_begin();
    let unknown = a.operations().unknown_type();
    let expression_analysis_result = a.analyze_expression(expression, unknown, false, true, false);
    let expression_type = expression_analysis_result.type_;
    a.flow().if_case_statement_after_expression(
        expression_analysis_result.flow_analysis_info,
        expression_type,
    );
    // Stack: (Expression)
    let component_variables = new_shared_map();
    let pattern_variable_promotion_keys = new_shared_map();
    // TODO(paulberry): rework handling of isFinal
    a.dispatch_pattern(
        &new_match_context::<A>(
            false,
            None,
            None,
            None,
            Rc::clone(&component_variables),
            Rc::clone(&pattern_variable_promotion_keys),
        ),
        A::pattern_node(pattern),
    );
    // Stack: (Expression, Pattern)
    let component_variables = component_variables.borrow().clone();
    let pattern_variable_promotion_keys = pattern_variable_promotion_keys.borrow().clone();
    finish_joined_pattern_variables(
        a,
        variables,
        &component_variables,
        &pattern_variable_promotion_keys,
        JoinedPatternVariableLocation::SinglePattern,
    );
    let (non_boolean_guard_error, guard_type, guard_info) = analyze_guard(a, node, guard);
    // Stack: (Expression, Pattern, Guard)
    a.flow().if_case_statement_then_begin(guard_info);
    analyze_if_element_common(a, node, if_true, if_false, context);
    IfCaseStatementResult {
        matched_expression_type: expression_type,
        non_boolean_guard_error,
        guard_type,
    }
}

/// The guard part shared by [`analyze_if_case_element`] and
/// [`analyze_if_case_statement`] (inlined twice in Dart).
fn analyze_guard<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    guard: Option<A::Expression>,
) -> (
    Option<A::Error>,
    Option<TypeView<Ops<A>>>,
    Option<ExpressionInfoOf<A>>,
) {
    let non_boolean_guard_error;
    let guard_type;
    let guard_info;
    if let Some(guard) = guard {
        let bool_schema = a.operations().type_to_schema(a.operations().bool_type());
        let guard_analysis_result = a.analyze_expression(guard, bool_schema, false, true, false);
        guard_type = Some(guard_analysis_result.type_);
        non_boolean_guard_error = check_guard_type(a, guard, guard_analysis_result.type_);
        guard_info = guard_analysis_result.flow_analysis_info;
    } else {
        non_boolean_guard_error = None;
        guard_type = None;
        a.handle_no_guard(node, 0);
        guard_info = Some(a.flow().boolean_literal(true));
    }
    (non_boolean_guard_error, guard_type, guard_info)
}

/// Analyzes a statement of the form `if (expression case pattern) ifTrue` or
/// `if (expression case pattern) ifTrue else ifFalse`.
///
/// Returns a [`IfCaseStatementResult`] with the static type of `expression`
/// and information about reported errors.
pub fn analyze_if_case_statement<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Statement,
    expression: A::Expression,
    pattern: A::Pattern,
    guard: Option<A::Expression>,
    if_true: A::Statement,
    if_false: Option<A::Statement>,
    variables: &[(NameOf<Ops<A>>, A::Variable)],
) -> IfCaseStatementResult<Ty<A>, A::Error> {
    // Stack: ()
    a.flow().if_case_statement_begin();
    let unknown = a.operations().unknown_type();
    let expression_analysis_result = a.analyze_expression(expression, unknown, false, true, false);
    let expression_type = expression_analysis_result.type_;
    a.flow().if_case_statement_after_expression(
        expression_analysis_result.flow_analysis_info,
        expression_type,
    );
    // Stack: (Expression)
    let component_variables = new_shared_map();
    let pattern_variable_promotion_keys = new_shared_map();
    // TODO(paulberry): rework handling of isFinal
    a.dispatch_pattern(
        &new_match_context::<A>(
            false,
            None,
            None,
            None,
            Rc::clone(&component_variables),
            Rc::clone(&pattern_variable_promotion_keys),
        ),
        A::pattern_node(pattern),
    );

    let component_variables = component_variables.borrow().clone();
    let pattern_variable_promotion_keys = pattern_variable_promotion_keys.borrow().clone();
    finish_joined_pattern_variables(
        a,
        variables,
        &component_variables,
        &pattern_variable_promotion_keys,
        JoinedPatternVariableLocation::SinglePattern,
    );

    a.handle_if_case_statement_after_pattern(node);
    // Stack: (Expression, Pattern)
    let (non_boolean_guard_error, guard_type, guard_info) =
        analyze_guard(a, A::statement_node(node), guard);
    // Stack: (Expression, Pattern, Guard)
    a.flow().if_case_statement_then_begin(guard_info);
    analyze_if_common(a, node, if_true, if_false);
    IfCaseStatementResult {
        matched_expression_type: expression_type,
        non_boolean_guard_error,
        guard_type,
    }
}

/// Analyzes a collection element of the form `if (condition) ifTrue` or
/// `if (condition) ifTrue else ifFalse`.
pub fn analyze_if_element<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    condition: A::Expression,
    if_true: A::Node,
    if_false: Option<A::Node>,
    context: A::CollectionElementContext,
) {
    // Stack: ()
    a.flow().if_statement_condition_begin();
    let bool_schema = a.operations().type_to_schema(a.operations().bool_type());
    let condition_analysis_result = a.analyze_expression(condition, bool_schema, false, true, false);
    a.handle_if_element_condition_end(node);
    // Stack: (Expression condition)
    a.flow()
        .if_statement_then_begin(condition_analysis_result.flow_analysis_info, node);
    analyze_if_element_common(a, node, if_true, if_false, context);
}

/// Analyzes a statement of the form `if (condition) ifTrue` or
/// `if (condition) ifTrue else ifFalse`.
pub fn analyze_if_statement<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Statement,
    condition: A::Expression,
    if_true: A::Statement,
    if_false: Option<A::Statement>,
) {
    // Stack: ()
    a.flow().if_statement_condition_begin();
    let bool_schema = a.operations().type_to_schema(a.operations().bool_type());
    let condition_analysis_result = a.analyze_expression(condition, bool_schema, false, true, false);
    a.handle_if_statement_condition_end(node);
    // Stack: (Expression condition)
    a.flow().if_statement_then_begin(
        condition_analysis_result.flow_analysis_info,
        A::statement_node(node),
    );
    analyze_if_common(a, node, if_true, if_false);
}

/// Analyzes an integer literal, given the type schema `schema`.
pub fn analyze_int_literal<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    schema: SchemaView<Ops<A>>,
) -> IntTypeAnalysisResult<Ty<A>, ExpressionInfoOf<A>> {
    let ops = a.operations();
    let convert_to_double = !ops.is_type_schema_satisfied(schema, ops.int_type())
        && ops.is_type_schema_satisfied(schema, ops.double_type());
    let type_ = if convert_to_double {
        ops.double_type()
    } else {
        ops.int_type()
    };
    IntTypeAnalysisResult {
        type_,
        flow_analysis_info: None,
        converted_to_double: convert_to_double,
    }
}

/// Analyzes a list pattern. `node` is the pattern itself, `element_type` is
/// the list element type (if explicitly supplied), and `elements` is the
/// list of subpatterns.
///
/// Returns a [`ListPatternResult`] with the required type and information
/// about reported errors.
pub fn analyze_list_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    element_type: Option<TypeView<Ops<A>>>,
    elements: &[A::Node],
) -> ListPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let value_type = if let Some(element_type) = element_type {
        element_type
    } else {
        let list_element_type = a.operations().match_list_type(matched_value_type);
        if let Some(list_element_type) = list_element_type {
            list_element_type
        } else {
            match kind_of(a, matched_value_type) {
                SharedTypeKind::Dynamic => a.operations().dynamic_type(),
                SharedTypeKind::Invalid => a.operations().error_type(),
                _ => a.operations().object_question_type(),
            }
        }
    };
    let required_type = a.operations().list_type(value_type);
    let match_may_fail_even_if_correct_type =
        !(elements.len() == 1 && a.is_rest_pattern_element(elements[0]));
    a.flow().promote_for_pattern(
        matched_value_type,
        required_type,
        true,
        match_may_fail_even_if_correct_type,
    );
    // Stack: ()
    let mut previous_rest_pattern: Option<A::Node> = None;
    let mut duplicate_rest_pattern_errors: Option<BTreeMap<usize, A::Error>> = None;
    for (i, &element) in elements.iter().enumerate() {
        if a.is_rest_pattern_element(element) {
            if let Some(previous_rest_pattern) = previous_rest_pattern {
                let error = a
                    .errors()
                    .duplicate_rest_pattern(node, previous_rest_pattern, element);
                duplicate_rest_pattern_errors
                    .get_or_insert_with(BTreeMap::new)
                    .insert(i, error);
            }
            previous_rest_pattern = Some(element);
            let sub_pattern = a.get_rest_pattern_element_pattern(element);
            if let Some(sub_pattern) = sub_pattern {
                let sub_pattern_matched_type = required_type;
                a.flow().push_subpattern(sub_pattern_matched_type);
                a.dispatch_pattern(
                    &context.with_unnecessary_wildcard_kind(None),
                    A::pattern_node(sub_pattern),
                );
                a.flow().pop_subpattern();
            }
            a.handle_list_pattern_rest_element(node, element);
        } else {
            a.flow().push_subpattern(value_type);
            a.dispatch_pattern(&context.with_unnecessary_wildcard_kind(None), element);
            a.flow().pop_subpattern();
        }
    }
    // Stack: (n * Pattern) where n = elements.length
    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context
        && !a
            .operations()
            .is_assignable_to(matched_value_type, required_type)
    {
        pattern_type_mismatch_in_irrefutable_context_error =
            Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                node,
                irrefutable_context,
                matched_value_type,
                required_type,
            ));
    }
    ListPatternResult {
        required_type,
        duplicate_rest_pattern_errors,
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Computes the type schema for a list pattern. `element_type` is the list
/// element type (if explicitly supplied), and `elements` is the list of
/// subpatterns.
pub fn analyze_list_pattern_schema<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    element_type: Option<TypeView<Ops<A>>>,
    elements: &[A::Node],
) -> SchemaView<Ops<A>> {
    if let Some(element_type) = element_type {
        let ops = a.operations();
        return ops.list_type_schema(ops.type_to_schema(element_type));
    }

    if elements.is_empty() {
        let ops = a.operations();
        return ops.list_type_schema(ops.unknown_type());
    }

    let mut current_glb: Option<SchemaView<Ops<A>>> = None;
    for &element in elements {
        let mut type_to_add = None;
        if a.is_rest_pattern_element(element) {
            let sub_pattern = a.get_rest_pattern_element_pattern(element);
            if let Some(sub_pattern) = sub_pattern {
                let sub_pattern_type = a.dispatch_pattern_schema(A::pattern_node(sub_pattern));
                type_to_add = a.operations().match_iterable_type_schema(sub_pattern_type);
            }
        } else {
            type_to_add = Some(a.dispatch_pattern_schema(element));
        }
        if let Some(type_to_add) = type_to_add {
            current_glb = Some(match current_glb {
                None => type_to_add,
                Some(current_glb) => a.operations().type_schema_glb(current_glb, type_to_add),
            });
        }
    }
    let ops = a.operations();
    let current_glb = current_glb.unwrap_or_else(|| ops.unknown_type());
    ops.list_type_schema(current_glb)
}

/// Analyzes a logical-and pattern. `node` is the pattern itself, and `lhs`
/// and `rhs` are the left and right sides of the `&&` operator.
pub fn analyze_logical_and_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    lhs: A::Node,
    rhs: A::Node,
) -> PatternResult<Ty<A>> {
    let _ = node;
    let matched_value_type = a.flow().get_matched_value_type();
    // Stack: ()
    a.dispatch_pattern(
        &context.with_unnecessary_wildcard_kind(Some(
            UnnecessaryWildcardKind::LogicalAndPatternOperand,
        )),
        lhs,
    );
    // Stack: (Pattern left)
    a.dispatch_pattern(
        &context.with_unnecessary_wildcard_kind(Some(
            UnnecessaryWildcardKind::LogicalAndPatternOperand,
        )),
        rhs,
    );
    // Stack: (Pattern left, Pattern right)
    PatternResult { matched_value_type }
}

/// Analyzes a logical-or pattern. `node` is the pattern itself, and `lhs`
/// and `rhs` are the left and right sides of the `||` operator.
///
/// Returns a [`LogicalOrPatternResult`] with information about reported
/// errors.
pub fn analyze_logical_or_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    lhs: A::Node,
    rhs: A::Node,
) -> LogicalOrPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let mut context = context.clone();
    let mut refutable_pattern_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context {
        refutable_pattern_in_irrefutable_context_error = Some(
            a.errors()
                .refutable_pattern_in_irrefutable_context(A::pattern_node(node), irrefutable_context),
        );
        // Avoid cascading errors
        context = context.make_refutable();
    }
    // Stack: ()
    a.flow().logical_or_pattern_begin();
    let left_promotion_keys: SharedMap<NameOf<Ops<A>>, PromotionKey> = new_shared_map();
    a.dispatch_pattern(
        &context
            .with_promotion_keys(Rc::clone(&left_promotion_keys))
            .with_unnecessary_wildcard_kind(None),
        lhs,
    );
    // Stack: (Pattern left)
    // We'll use the promotion keys allocated during processing of the LHS as
    // the merged keys.
    let left_entries = left_promotion_keys.borrow().clone();
    for (variable_name, promotion_key) in left_entries {
        debug_assert!(!map_contains_key(
            &context.pattern_variable_promotion_keys,
            &variable_name
        ));
        map_set(
            &context.pattern_variable_promotion_keys,
            variable_name,
            promotion_key,
        );
    }
    a.flow().logical_or_pattern_after_lhs();
    a.handle_logical_or_pattern_after_lhs(node);
    let right_promotion_keys: SharedMap<NameOf<Ops<A>>, PromotionKey> = new_shared_map();
    a.dispatch_pattern(
        &context
            .with_promotion_keys(Rc::clone(&right_promotion_keys))
            .with_unnecessary_wildcard_kind(None),
        rhs,
    );
    // Stack: (Pattern left, Pattern right)
    let right_entries = right_promotion_keys.borrow().clone();
    for (variable_name, right_promotion_key) in right_entries {
        let merged_promotion_key = map_get(&left_promotion_keys, &variable_name);
        match merged_promotion_key {
            None => {
                // No matching variable on the LHS. This is an error condition
                // (which has already been reported by VariableBinder). For
                // error recovery, we still need to add the variable to
                // context.patternVariablePromotionKeys so that later analysis
                // still accounts for the presence of this variable. So we just
                // use the promotion key from the RHS as the merged key.
                let merged_promotion_key = right_promotion_key;
                debug_assert!(!map_contains_key(
                    &context.pattern_variable_promotion_keys,
                    &variable_name
                ));
                map_set(
                    &context.pattern_variable_promotion_keys,
                    variable_name,
                    merged_promotion_key,
                );
            }
            Some(merged_promotion_key) => {
                // Copy the promotion data over to the merged key.
                a.flow()
                    .copy_promotion_data(right_promotion_key, merged_promotion_key);
            }
        }
    }
    // Since the promotion data is now all stored in the merged keys in both
    // flow control branches, the normal join process will combine promotions
    // accordingly.
    a.flow().logical_or_pattern_end();
    LogicalOrPatternResult {
        refutable_pattern_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Computes the type schema for a logical-or pattern. `lhs` and `rhs` are
/// the left and right sides of the `|` or `&` operator.
pub fn analyze_logical_or_pattern_schema<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    lhs: A::Node,
    rhs: A::Node,
) -> SchemaView<Ops<A>> {
    let _ = (lhs, rhs);
    // Logical-or patterns are only allowed in refutable contexts, and
    // refutable contexts don't propagate a type schema into the scrutinee.
    // So this code path is only reachable if the user's code contains
    // errors.
    a.errors().assert_in_error_recovery();
    a.operations().unknown_type()
}

/// Analyzes a map pattern. `node` is the pattern itself, `type_arguments`
/// contain explicit type arguments (if specified), and `elements` is the
/// list of subpatterns.
///
/// Returns a [`MapPatternResult`] with the required type and information
/// about reported errors.
pub fn analyze_map_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    type_arguments: Option<KeyValueTypes<TypeView<Ops<A>>>>,
    elements: &[A::Node],
) -> MapPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let key_type;
    let value_type;
    let key_schema;
    if let Some(type_arguments) = type_arguments {
        key_type = type_arguments.key_type;
        value_type = type_arguments.value_type;
        key_schema = a.operations().type_to_schema(key_type);
    } else {
        let type_arguments = a.operations().match_map_type(matched_value_type);
        if let Some(type_arguments) = type_arguments {
            key_type = type_arguments.key_type;
            value_type = type_arguments.value_type;
            key_schema = a.operations().type_to_schema(key_type);
        } else {
            let ops = a.operations();
            match ops.shared_type_kind(matched_value_type.unwrap_type_view()) {
                SharedTypeKind::Dynamic => {
                    key_type = ops.dynamic_type();
                    value_type = ops.dynamic_type();
                    key_schema = ops.unknown_type();
                }
                SharedTypeKind::Invalid => {
                    key_type = ops.error_type();
                    value_type = ops.error_type();
                    key_schema = ops.unknown_type();
                }
                _ => {
                    key_type = ops.object_question_type();
                    value_type = ops.object_question_type();
                    key_schema = ops.unknown_type();
                }
            }
        }
    }
    let required_type = a.operations().map_type(key_type, value_type);
    let mut match_may_fail_even_if_correct_type = true;
    if a.type_analyzer_options().sound_flow_analysis_enabled && elements.is_empty() {
        // With sound null safety, an empty map pattern can only fail to match
        // if the types don't match.
        match_may_fail_even_if_correct_type = false;
    }
    a.flow().promote_for_pattern(
        matched_value_type,
        required_type,
        true,
        match_may_fail_even_if_correct_type,
    );
    // Stack: ()

    let mut rest_pattern_errors: Option<BTreeMap<usize, A::Error>> = None;
    for (i, &element) in elements.iter().enumerate() {
        if a.is_rest_pattern_element(element) {
            let error = a.errors().rest_pattern_in_map(node, element);
            rest_pattern_errors
                .get_or_insert_with(BTreeMap::new)
                .insert(i, error);
        }
    }

    for &element in elements {
        let entry = a.get_map_pattern_entry(element);
        if let Some(entry) = entry {
            let key_type = a
                .analyze_expression(entry.key, key_schema, false, true, false)
                .type_;
            a.flow().push_subpattern(value_type);
            a.dispatch_pattern(
                &context.with_unnecessary_wildcard_kind(None),
                A::pattern_node(entry.value),
            );
            a.handle_map_pattern_entry(node, element, key_type);
            a.flow().pop_subpattern();
        } else {
            debug_assert!(a.is_rest_pattern_element(element));
            let sub_pattern = a.get_rest_pattern_element_pattern(element);
            if let Some(sub_pattern) = sub_pattern {
                let dynamic_type = a.operations().dynamic_type();
                a.flow().push_subpattern(dynamic_type);
                a.dispatch_pattern(
                    &context.with_unnecessary_wildcard_kind(None),
                    A::pattern_node(sub_pattern),
                );
                a.flow().pop_subpattern();
            }
            a.handle_map_pattern_rest_element(node, element);
        }
    }
    // Stack: (n * MapPatternElement) where n = elements.length
    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context
        && !a
            .operations()
            .is_assignable_to(matched_value_type, required_type)
    {
        pattern_type_mismatch_in_irrefutable_context_error =
            Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                node,
                irrefutable_context,
                matched_value_type,
                required_type,
            ));
    }
    let mut empty_map_pattern_error = None;
    if elements.is_empty() {
        empty_map_pattern_error = Some(a.errors().empty_map_pattern(node));
    }
    MapPatternResult {
        required_type,
        pattern_type_mismatch_in_irrefutable_context_error,
        empty_map_pattern_error,
        rest_pattern_errors,
        matched_value_type,
    }
}

/// Computes the type schema for a map pattern. `type_arguments` contain
/// explicit type arguments (if specified), and `elements` is the list of
/// subpatterns.
pub fn analyze_map_pattern_schema<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    type_arguments: Option<KeyValueTypes<TypeView<Ops<A>>>>,
    elements: &[A::Node],
) -> SchemaView<Ops<A>> {
    if let Some(type_arguments) = type_arguments {
        let ops = a.operations();
        return ops.type_to_schema(ops.map_type(type_arguments.key_type, type_arguments.value_type));
    }

    let mut value_type: Option<SchemaView<Ops<A>>> = None;
    for &element in elements {
        let entry = a.get_map_pattern_entry(element);
        if let Some(entry) = entry {
            let entry_value_type = a.dispatch_pattern_schema(A::pattern_node(entry.value));
            value_type = Some(match value_type {
                None => entry_value_type,
                Some(value_type) => a.operations().type_schema_glb(value_type, entry_value_type),
            });
        }
    }
    let ops = a.operations();
    ops.map_type_schema(
        ops.unknown_type(),
        value_type.unwrap_or_else(|| ops.unknown_type()),
    )
}

/// Analyzes a null-check or null-assert pattern. `node` is the pattern
/// itself, `inner_pattern` is the sub-pattern, and `is_assert` indicates
/// whether this is a null-check or a null-assert pattern.
///
/// Returns a [`NullCheckOrAssertPatternResult`] with information about
/// reported errors.
pub fn analyze_null_check_or_assert_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    inner_pattern: A::Pattern,
    is_assert: bool,
) -> NullCheckOrAssertPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    // Stack: ()
    let mut refutable_pattern_in_irrefutable_context_error = None;
    let mut matched_type_is_strictly_non_nullable_error = None;
    let irrefutable_context = context.irrefutable_context;
    let matched_type_is_strictly_non_nullable = a
        .flow()
        .null_check_or_assert_pattern_begin(is_assert, matched_value_type);
    let mut context = context.clone();
    if let Some(irrefutable_context) = irrefutable_context
        && !is_assert
    {
        refutable_pattern_in_irrefutable_context_error = Some(
            a.errors()
                .refutable_pattern_in_irrefutable_context(A::pattern_node(node), irrefutable_context),
        );
        // Avoid cascading errors
        context = context.make_refutable();
    } else if matched_type_is_strictly_non_nullable {
        matched_type_is_strictly_non_nullable_error = a
            .errors()
            .matched_type_is_strictly_non_nullable(node, matched_value_type);
    }
    a.dispatch_pattern(
        &context.with_unnecessary_wildcard_kind(None),
        A::pattern_node(inner_pattern),
    );
    // Stack: (Pattern)
    a.flow().null_check_or_assert_pattern_end();

    NullCheckOrAssertPatternResult {
        refutable_pattern_in_irrefutable_context_error,
        matched_type_is_strictly_non_nullable_error,
        matched_value_type,
    }
}

/// Computes the type schema for a null-check or null-assert pattern.
/// `inner_pattern` is the sub-pattern and `is_assert` indicates whether this
/// is a null-check or a null-assert pattern.
pub fn analyze_null_check_or_assert_pattern_schema<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    inner_pattern: A::Pattern,
    is_assert: bool,
) -> SchemaView<Ops<A>> {
    if is_assert {
        let inner = a.dispatch_pattern_schema(A::pattern_node(inner_pattern));
        a.operations().make_type_schema_nullable(inner)
    } else {
        // Null-check patterns are only allowed in refutable contexts, and
        // refutable contexts don't propagate a type schema into the
        // scrutinee. So this code path is only reachable if the user's code
        // contains errors.
        a.errors().assert_in_error_recovery();
        a.operations().unknown_type()
    }
}

/// Analyzes an object pattern. `node` is the pattern itself, and `fields` is
/// the list of subpatterns.
///
/// Returns a [`ObjectPatternResult`] with the required type and information
/// about reported errors.
pub fn analyze_object_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    fields: &[RecordPatternFieldOf<A>],
) -> ObjectPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let duplicate_record_pattern_field_errors =
        report_duplicate_record_pattern_fields(a, node, fields);

    let required_type = a.downward_infer_object_pattern_required_type(matched_value_type, node);
    a.flow()
        .promote_for_pattern(matched_value_type, required_type, true, false);

    // If the required type is `dynamic` or `Never`, then every getter is
    // treated as having the same type.
    let mut override_property_get_type: Option<(Option<PropertyMemberOf<A>>, TypeView<Ops<A>>)> =
        None;
    let required_kind = kind_of(a, required_type);
    if required_kind == SharedTypeKind::Dynamic
        || required_kind == SharedTypeKind::Invalid
        || a.operations().is_bottom_type(required_type)
    {
        override_property_get_type = Some((None, required_type));
    }

    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context
        && !a
            .operations()
            .is_assignable_to(matched_value_type, required_type)
    {
        pattern_type_mismatch_in_irrefutable_context_error =
            Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                node,
                irrefutable_context,
                matched_value_type,
                required_type,
            ));
    }

    // Stack: ()
    for field in fields {
        let (property_member, unpromoted_property_type) = match &override_property_get_type {
            Some(override_property_get_type) => override_property_get_type.clone(),
            None => a.resolve_object_pattern_property_get(node, required_type, field),
        };
        // Note: an object pattern field must always have a property name, but
        // in error recovery circumstances, one may be absent; when this
        // happens, use the empty string as a the property name to prevent a
        // crash.
        let property_name = field
            .name
            .unwrap_or_else(A::empty_name);
        let promoted_property_type = a
            .flow()
            .push_property_subpattern(property_name, property_member, unpromoted_property_type)
            .unwrap_or(unpromoted_property_type);
        if a.operations().is_bottom_type(promoted_property_type) {
            a.flow().handle_exit();
        }
        a.dispatch_pattern(
            &context.with_unnecessary_wildcard_kind(None),
            A::pattern_node(field.pattern),
        );
        a.flow().pop_property_subpattern();
    }
    // Stack: (n * Pattern) where n = fields.length

    ObjectPatternResult {
        required_type,
        duplicate_record_pattern_field_errors,
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Analyzes a patternAssignment expression of the form `pattern = rhs`.
///
/// `node` should be the AST node for the entire expression, `pattern` for
/// the pattern, and `rhs` for the right hand side.
pub fn analyze_pattern_assignment<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Expression,
    pattern: A::Pattern,
    rhs: A::Expression,
) -> PatternAssignmentAnalysisResult<Ty<A>, ExpressionInfoOf<A>> {
    // Stack: ()
    let pattern_schema = a.dispatch_pattern_schema(A::pattern_node(pattern));
    let rhs_analysis_result = a.analyze_expression(
        rhs,
        pattern_schema,
        false,
        true,
        // The expression is assigned to the pattern, and so the coercion
        // needs to be performed.
        true,
    );
    let rhs_type = rhs_analysis_result.type_;
    // Stack: (Expression)
    a.flow()
        .pattern_assignment_after_rhs(rhs_analysis_result.flow_analysis_info, rhs_type);
    let component_variables: SharedMap<NameOf<Ops<A>>, Vec<A::Variable>> = new_shared_map();
    let pattern_variable_promotion_keys = new_shared_map();
    a.dispatch_pattern(
        &new_match_context::<A>(
            false,
            Some(A::expression_node(node)),
            None,
            Some(new_shared_map()),
            Rc::clone(&component_variables),
            pattern_variable_promotion_keys,
        ),
        A::pattern_node(pattern),
    );
    if !component_variables.borrow().is_empty() {
        // Declared pattern variables should never appear in a pattern
        // assignment so this should never happen.
        a.errors().assert_in_error_recovery();
    }
    a.flow().pattern_assignment_end();
    // Stack: (Expression, Pattern)
    PatternAssignmentAnalysisResult {
        pattern_schema,
        type_: rhs_type,
        flow_analysis_info: None,
    }
}

/// Analyzes a `pattern-for-in` statement or element.
///
/// Statement: `for (<keyword> <pattern> in <expression>) <statement>`
///
/// Element: `for (<keyword> <pattern> in <expression>) <body>`
///
/// Returns a [`PatternForInResult`] containing information on reported
/// errors.
///
/// Note, however, that the caller is responsible for reporting an error if
/// the static type of `expression` is potentially nullable.
pub fn analyze_pattern_for_in<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    has_await: bool,
    pattern: A::Pattern,
    expression: A::Expression,
    dispatch_body: &mut dyn FnMut(&mut A),
) -> PatternForInResult<Ty<A>, A::Error> {
    // Stack: ()
    let pattern_type_schema = a.dispatch_pattern_schema(A::pattern_node(pattern));
    let expression_type_schema = if has_await {
        a.operations().stream_type_schema(pattern_type_schema)
    } else {
        a.operations().iterable_type_schema(pattern_type_schema)
    };
    let expression_type = a
        .analyze_expression(expression, expression_type_schema, false, true, false)
        .type_;
    // Stack: (Expression)

    let mut pattern_for_in_expression_is_not_iterable_error = None;
    let element_type = if has_await {
        a.operations().match_stream_type(expression_type)
    } else {
        a.operations().match_iterable_type(expression_type)
    };
    let element_type = match element_type {
        Some(element_type) => element_type,
        None => match kind_of(a, expression_type) {
            SharedTypeKind::Dynamic => a.operations().dynamic_type(),
            SharedTypeKind::Invalid => a.operations().error_type(),
            _ => {
                pattern_for_in_expression_is_not_iterable_error = Some(
                    a.errors()
                        .pattern_for_in_expression_is_not_iterable(node, expression, expression_type),
                );
                a.operations().error_type()
            }
        },
    };
    a.flow().pattern_for_in_after_expression(element_type);

    let component_variables = new_shared_map();
    let pattern_variable_promotion_keys = new_shared_map();
    a.dispatch_pattern(
        &new_match_context::<A>(
            false,
            Some(node),
            None,
            None,
            component_variables,
            pattern_variable_promotion_keys,
        ),
        A::pattern_node(pattern),
    );
    // Stack: (Expression, Pattern)

    a.flow().for_each_body_begin(node);
    dispatch_body(a);
    a.flow().for_each_end();
    a.flow().pattern_for_in_end();

    PatternForInResult {
        element_type,
        expression_type,
        pattern_for_in_expression_is_not_iterable_error,
    }
}

/// Analyzes a patternVariableDeclaration node of the form `var pattern =
/// initializer` or `final pattern = initializer`.
///
/// Returns a [`PatternVariableDeclarationAnalysisResult`] holding the static
/// type of the initializer and the type schema of the `pattern`.
pub fn analyze_pattern_variable_declaration<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    pattern: A::Pattern,
    initializer: A::Expression,
    is_final: bool,
) -> PatternVariableDeclarationAnalysisResult<Ty<A>> {
    // Stack: ()
    let pattern_schema = a.dispatch_pattern_schema(A::pattern_node(pattern));
    let initializer_analysis_result = a.analyze_expression(
        initializer,
        pattern_schema,
        false,
        true,
        // The initializer expression is assigned to the pattern, and so the
        // coercion needs to be performed.
        true,
    );
    let initializer_type = initializer_analysis_result.type_;
    // Stack: (Expression)
    a.flow().pattern_variable_declaration_after_initializer(
        initializer_analysis_result.flow_analysis_info,
        initializer_type,
    );
    let component_variables = new_shared_map();
    let pattern_variable_promotion_keys = new_shared_map();
    a.dispatch_pattern(
        &new_match_context::<A>(
            is_final,
            Some(node),
            None,
            None,
            Rc::clone(&component_variables),
            Rc::clone(&pattern_variable_promotion_keys),
        ),
        A::pattern_node(pattern),
    );
    let component_variables = component_variables.borrow().clone();
    let pattern_variable_promotion_keys = pattern_variable_promotion_keys.borrow().clone();
    finish_joined_pattern_variables(
        a,
        &[],
        &component_variables,
        &pattern_variable_promotion_keys,
        JoinedPatternVariableLocation::SinglePattern,
    );
    a.flow().pattern_variable_declaration_end();
    // Stack: (Expression, Pattern)
    PatternVariableDeclarationAnalysisResult {
        pattern_schema,
        initializer_type,
    }
}

/// Analyzes a record pattern. `node` is the pattern itself, and `fields` is
/// the list of subpatterns.
///
/// Returns a [`RecordPatternResult`] with the required type and information
/// about reported errors.
pub fn analyze_record_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    fields: &[RecordPatternFieldOf<A>],
) -> RecordPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let mut demonstrated_positional_types: Vec<TypeView<Ops<A>>> = Vec::new();
    let mut demonstrated_named_types: Vec<(NameOf<Ops<A>>, TypeView<Ops<A>>)> = Vec::new();

    let duplicate_record_pattern_field_errors =
        report_duplicate_record_pattern_fields(a, node, fields);

    let is_duplicate = |i: usize| -> bool {
        duplicate_record_pattern_field_errors
            .as_ref()
            .is_some_and(|errors| errors.contains_key(&i))
    };

    let mut dispatch_field = |a: &mut A, i: usize, matched_type: TypeView<Ops<A>>| {
        let field = &fields[i];
        a.flow().push_subpattern(matched_type);
        a.dispatch_pattern(
            &context.with_unnecessary_wildcard_kind(None),
            A::pattern_node(field.pattern),
        );
        let demonstrated_type = a.flow().get_matched_value_type();
        match field.name {
            None => demonstrated_positional_types.push(demonstrated_type),
            Some(name) => {
                if !is_duplicate(i) {
                    demonstrated_named_types.push((name, demonstrated_type));
                }
            }
        }
        a.flow().pop_subpattern();
    };

    // Build the required type.
    let mut required_type_positional_count = 0;
    let mut required_type_named_types: Vec<(NameOf<Ops<A>>, TypeView<Ops<A>>)> = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        match field.name {
            None => required_type_positional_count += 1,
            Some(name) => {
                if !is_duplicate(i) {
                    required_type_named_types.push((name, a.operations().object_question_type()));
                }
            }
        }
    }
    let required_type = {
        let ops = a.operations();
        let positional = vec![ops.object_question_type(); required_type_positional_count];
        ops.record_type(&positional, &required_type_named_types)
    };
    a.flow()
        .promote_for_pattern(matched_value_type, required_type, true, false);

    // Stack: ()
    match kind_of(a, matched_value_type) {
        SharedTypeKind::Record => {
            let field_types = match_record_type_shape(a, fields, matched_value_type);
            if let Some(field_types) = field_types {
                debug_assert!(field_types.len() == fields.len());
                for (i, field_type) in field_types.into_iter().enumerate() {
                    dispatch_field(a, i, field_type);
                }
            } else {
                let object_question_type = a.operations().object_question_type();
                for i in 0..fields.len() {
                    dispatch_field(a, i, object_question_type);
                }
            }
        }
        SharedTypeKind::Dynamic => {
            let dynamic_type = a.operations().dynamic_type();
            for i in 0..fields.len() {
                dispatch_field(a, i, dynamic_type);
            }
        }
        SharedTypeKind::Invalid => {
            let error_type = a.operations().error_type();
            for i in 0..fields.len() {
                dispatch_field(a, i, error_type);
            }
        }
        _ => {
            let object_question_type = a.operations().object_question_type();
            for i in 0..fields.len() {
                dispatch_field(a, i, object_question_type);
            }
        }
    }
    // Stack: (n * Pattern) where n = fields.length

    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context
        && !a
            .operations()
            .is_assignable_to(matched_value_type, required_type)
    {
        pattern_type_mismatch_in_irrefutable_context_error =
            Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                node,
                irrefutable_context,
                matched_value_type,
                required_type,
            ));
    }

    let demonstrated_type = a
        .operations()
        .record_type(&demonstrated_positional_types, &demonstrated_named_types);
    a.flow()
        .promote_for_pattern(matched_value_type, demonstrated_type, false, false);
    RecordPatternResult {
        required_type,
        duplicate_record_pattern_field_errors,
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Computes the type schema for a record pattern.
pub fn analyze_record_pattern_schema<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    fields: &[RecordPatternFieldOf<A>],
) -> SchemaView<Ops<A>> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for field in fields {
        let field_type = a.dispatch_pattern_schema(A::pattern_node(field.pattern));
        match field.name {
            Some(name) => named.push((name, field_type)),
            None => positional.push(field_type),
        }
    }
    a.operations().record_type_schema(&positional, &named)
}

/// Analyzes a relational pattern. `node` is the pattern itself, and
/// `operand` is a constant expression that will be passed to the relational
/// operator.
///
/// This method will invoke `resolveRelationalPatternOperator` to obtain
/// information about the operator.
///
/// Returns a [`RelationalPatternResult`] with the type of the `operand` and
/// information about reported errors.
pub fn analyze_relational_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    operand: A::Expression,
) -> RelationalPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    // Stack: ()
    let mut refutable_pattern_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context {
        refutable_pattern_in_irrefutable_context_error = Some(
            a.errors()
                .refutable_pattern_in_irrefutable_context(A::pattern_node(node), irrefutable_context),
        );
    }
    let operator = a.resolve_relational_pattern_operator(node, matched_value_type);
    let mut parameter_type = operator.map(|operator| operator.parameter_type);
    let is_equality = matches!(
        operator.map(|operator| operator.kind),
        Some(RelationalOperatorKind::Equals | RelationalOperatorKind::NotEquals)
    );
    if is_equality && let Some(p) = parameter_type {
        parameter_type = Some(a.operations().make_nullable(p));
    }

    let operand_schema = if a.is_dot_shorthand(operand) {
        a.operations().type_to_schema(matched_value_type)
    } else if let Some(parameter_type) = parameter_type {
        a.operations().type_to_schema(parameter_type)
    } else {
        a.operations().unknown_type()
    };
    let operand_analysis_result = a.analyze_expression(
        operand,
        operand_schema,
        false,
        true,
        // The constant expressions in relational patterns are considered to
        // be passed into the corresponding operator, and so the coercion
        // needs to be performed.
        true,
    );
    let operand_type = operand_analysis_result.type_;
    if is_equality {
        a.flow().equality_relational_pattern_end(
            operand_analysis_result.flow_analysis_info,
            operand_type,
            operator.map(|operator| operator.kind) == Some(RelationalOperatorKind::NotEquals),
            matched_value_type,
        );
    } else {
        a.flow().non_equality_relational_pattern_end();
    }
    // Stack: (Expression)
    let mut argument_type_not_assignable_error = None;
    let mut operator_return_type_not_assignable_to_bool_error = None;
    if let Some(operator) = operator {
        if let Some(parameter_type) = parameter_type
            && !a.operations().is_assignable_to(operand_type, parameter_type)
        {
            argument_type_not_assignable_error =
                Some(a.errors().relational_pattern_operand_type_not_assignable(
                    node,
                    operand_type,
                    operator.parameter_type,
                ));
        }
        let bool_type = a.operations().bool_type();
        if !a
            .operations()
            .is_assignable_to(operator.return_type, bool_type)
        {
            operator_return_type_not_assignable_to_bool_error = Some(
                a.errors()
                    .relational_pattern_operator_return_type_not_assignable_to_bool(
                        node,
                        operator.return_type,
                    ),
            );
        }
    }
    RelationalPatternResult {
        operand_type,
        refutable_pattern_in_irrefutable_context_error,
        argument_type_not_assignable_error,
        operator_return_type_not_assignable_to_bool_error,
        matched_value_type,
    }
}

/// Analyzes an expression of the form `switch (expression) { cases }`.
///
/// Returns a [`SwitchExpressionResult`] with the static type of the switch
/// expression and information about reported errors.
#[allow(clippy::if_same_then_else)] // Keeps the structure of the Dart code.
pub fn analyze_switch_expression<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Expression,
    scrutinee: A::Expression,
    num_cases: usize,
    schema: SchemaView<Ops<A>>,
) -> SwitchExpressionResult<Ty<A>, ExpressionInfoOf<A>, A::Error> {
    // Stack: ()

    // The static type of a switch expression `E` of the form `switch (e0) {
    // p1 => e1, p2 => e2, ... pn => en }` with context type `K` is computed
    // as follows:
    //
    // - The scrutinee (`e0`) is first analyzed with context type `_`.
    let unknown = a.operations().unknown_type();
    let scrutinee_analysis_result = a.analyze_expression(scrutinee, unknown, false, true, false);
    let expression_type = scrutinee_analysis_result.type_;
    // Stack: (Expression)
    a.handle_switch_scrutinee(expression_type);
    a.flow().switch_statement_expression_end(
        None,
        scrutinee_analysis_result.flow_analysis_info,
        expression_type,
    );

    // - If the switch expression has no cases, its static type is `Never`.
    let mut non_boolean_guard_errors: Option<BTreeMap<usize, A::Error>> = None;
    let mut guard_types: Option<BTreeMap<usize, TypeView<Ops<A>>>> = None;
    let static_type;
    let node_as_node = A::expression_node(node);
    if num_cases == 0 {
        static_type = a.operations().never_type();
    } else {
        // - Otherwise, for each case `pi => ei`, let `Ti` be the type of `ei`
        //   inferred with context type `K`.
        // - Let `T` be the least upper bound of the static types of all the
        //   case expressions.
        // - Let `S` be the greatest closure of `K`.
        let mut t: Option<TypeView<Ops<A>>> = None;
        // Note that the `topType` named parameter below is a work-around for
        // the discrepancy between the Analyzer and the CFE and should be
        // removed when the discrepancy is resolved. For details, see
        // https://github.com/dart-lang/language/issues/4466.
        let s = {
            let ops = a.operations();
            ops.greatest_closure_of_schema(schema, Some(ops.dynamic_type()))
        };
        let mut all_cases_satisfy_context = true;
        for i in 0..num_cases {
            // Stack: (Expression, i * ExpressionCase)
            let member_info = a.get_switch_expression_member_info(node, i);
            a.flow().switch_statement_begin_alternatives();
            a.flow().switch_statement_begin_alternative();
            a.handle_switch_before_alternative(node_as_node, i, 0);
            let pattern = member_info.head.pattern;
            let guard_info;
            if let Some(pattern) = pattern {
                let component_variables = new_shared_map();
                let pattern_variable_promotion_keys = new_shared_map();
                a.dispatch_pattern(
                    &new_match_context::<A>(
                        false,
                        None,
                        Some(scrutinee),
                        None,
                        Rc::clone(&component_variables),
                        Rc::clone(&pattern_variable_promotion_keys),
                    ),
                    pattern,
                );
                let component_variables = component_variables.borrow().clone();
                let pattern_variable_promotion_keys =
                    pattern_variable_promotion_keys.borrow().clone();
                finish_joined_pattern_variables(
                    a,
                    &member_info.head.variables,
                    &component_variables,
                    &pattern_variable_promotion_keys,
                    JoinedPatternVariableLocation::SinglePattern,
                );
                // Stack: (Expression, i * ExpressionCase, Pattern)
                let guard = member_info.head.guard;
                if let Some(guard) = guard {
                    let bool_schema = a.operations().type_to_schema(a.operations().bool_type());
                    let guard_analysis_result =
                        a.analyze_expression(guard, bool_schema, false, true, false);
                    let guard_type = guard_analysis_result.type_;
                    let non_boolean_guard_error = check_guard_type(a, guard, guard_type);
                    guard_types
                        .get_or_insert_with(BTreeMap::new)
                        .insert(i, guard_type);
                    if let Some(non_boolean_guard_error) = non_boolean_guard_error {
                        non_boolean_guard_errors
                            .get_or_insert_with(BTreeMap::new)
                            .insert(i, non_boolean_guard_error);
                    }
                    guard_info = guard_analysis_result.flow_analysis_info;
                    // Stack: (Expression, i * ExpressionCase, Pattern, Expression)
                } else {
                    a.handle_no_guard(node_as_node, i);
                    guard_info = Some(a.flow().boolean_literal(true));
                    // Stack: (Expression, i * ExpressionCase, Pattern, Expression)
                }
                a.handle_case_head(node_as_node, i, 0);
            } else {
                a.handle_default(node_as_node, i, 0);
                guard_info = Some(a.flow().boolean_literal(true));
            }
            a.flow().switch_statement_end_alternative(guard_info, &[]);
            a.flow().switch_statement_end_alternatives(None, false);
            // Stack: (Expression, i * ExpressionCase, CaseHead)
            let ti = a
                .analyze_expression(member_info.expression, schema, false, true, false)
                .type_;
            if all_cases_satisfy_context && !a.operations().is_subtype_of(ti, s) {
                all_cases_satisfy_context = false;
            }
            a.flow().switch_statement_after_case();
            // Stack: (Expression, i * ExpressionCase, CaseHead, Expression)
            t = Some(match t {
                None => ti,
                Some(t) => a.operations().lub(t, ti),
            });
            a.finish_expression_case(node, i);
            // Stack: (Expression, (i + 1) * ExpressionCase)
        }
        let t = t.expect("at least one case");
        // If `inferenceUpdate3` is not enabled, then the type of `E` is `T`.
        if !a.type_analyzer_options().inference_update3_enabled {
            static_type = t;
        } else
        // - If `T <: S`, then the type of `E` is `T`.
        if a.operations().is_subtype_of(t, s) {
            static_type = t;
        } else
        // - Otherwise, if `Ti <: S` for all `i`, then the type of `E` is `S`.
        if all_cases_satisfy_context {
            static_type = s;
        } else
        // - Otherwise, the type of `E` is `T`.
        {
            static_type = t;
        }
    }
    // Stack: (Expression, numCases * ExpressionCase)
    a.flow().switch_statement_end(true);
    SwitchExpressionResult {
        type_: static_type,
        flow_analysis_info: None,
        non_boolean_guard_errors,
        guard_types,
    }
}

/// Analyzes a statement of the form `switch (expression) { cases }`.
pub fn analyze_switch_statement<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Statement,
    scrutinee: A::Expression,
    num_cases: usize,
) -> SwitchStatementTypeAnalysisResult<Ty<A>, A::Error> {
    // Stack: ()
    let unknown = a.operations().unknown_type();
    let scrutinee_analysis_result = a.analyze_expression(scrutinee, unknown, false, true, false);
    let scrutinee_type = scrutinee_analysis_result.type_;
    // Stack: (Expression)
    a.handle_switch_scrutinee(scrutinee_type);
    a.flow().switch_statement_expression_end(
        Some(node),
        scrutinee_analysis_result.flow_analysis_info,
        scrutinee_type,
    );
    let node_as_node = A::statement_node(node);
    let mut has_default = false;
    let mut last_case_terminates = true;
    let mut switch_case_completes_normally_errors: Option<BTreeMap<usize, A::Error>> = None;
    let mut non_boolean_guard_errors: Option<BTreeMap<usize, BTreeMap<usize, A::Error>>> = None;
    let mut guard_types: Option<BTreeMap<usize, BTreeMap<usize, TypeView<Ops<A>>>>> = None;
    for case_index in 0..num_cases {
        // Stack: (Expression, numExecutionPaths * StatementCase)
        a.flow().switch_statement_begin_alternatives();
        // Stack: (Expression, numExecutionPaths * StatementCase,
        //         numHeads * CaseHead)
        let member_info = a.get_switch_statement_member_info(node, case_index);
        let heads = &member_info.heads;
        for (head_index, head) in heads.iter().enumerate() {
            let pattern = head.pattern;
            a.flow().switch_statement_begin_alternative();
            a.handle_switch_before_alternative(node_as_node, case_index, head_index);
            let guard_info;
            if let Some(pattern) = pattern {
                let component_variables = new_shared_map();
                let pattern_variable_promotion_keys = new_shared_map();
                a.dispatch_pattern(
                    &new_match_context::<A>(
                        false,
                        None,
                        Some(scrutinee),
                        None,
                        Rc::clone(&component_variables),
                        Rc::clone(&pattern_variable_promotion_keys),
                    ),
                    pattern,
                );
                let component_variables = component_variables.borrow().clone();
                let pattern_variable_promotion_keys =
                    pattern_variable_promotion_keys.borrow().clone();
                finish_joined_pattern_variables(
                    a,
                    &head.variables,
                    &component_variables,
                    &pattern_variable_promotion_keys,
                    JoinedPatternVariableLocation::SinglePattern,
                );
                // Stack: (Expression, numExecutionPaths * StatementCase,
                //         numHeads * CaseHead, Pattern),
                let guard = head.guard;
                if let Some(guard) = guard {
                    let bool_schema = a.operations().type_to_schema(a.operations().bool_type());
                    let guard_analysis_result =
                        a.analyze_expression(guard, bool_schema, false, true, false);
                    let guard_type = guard_analysis_result.type_;
                    let non_boolean_guard_error = check_guard_type(a, guard, guard_type);
                    guard_types
                        .get_or_insert_with(BTreeMap::new)
                        .entry(case_index)
                        .or_default()
                        .insert(head_index, guard_type);
                    if let Some(non_boolean_guard_error) = non_boolean_guard_error {
                        non_boolean_guard_errors
                            .get_or_insert_with(BTreeMap::new)
                            .entry(case_index)
                            .or_default()
                            .insert(head_index, non_boolean_guard_error);
                    }
                    guard_info = guard_analysis_result.flow_analysis_info;
                    // Stack: (Expression, numExecutionPaths * StatementCase,
                    //         numHeads * CaseHead, Pattern, Expression),
                } else {
                    a.handle_no_guard(node_as_node, case_index);
                    guard_info = Some(a.flow().boolean_literal(true));
                }
                a.handle_case_head(node_as_node, case_index, head_index);
            } else {
                has_default = true;
                a.handle_default(node_as_node, case_index, head_index);
                guard_info = Some(a.flow().boolean_literal(true));
            }
            // Stack: (Expression, numExecutionPaths * StatementCase,
            //         numHeads * CaseHead),
            a.flow()
                .switch_statement_end_alternative(guard_info, &head.variables);
        }
        // Stack: (Expression, numExecutionPaths * StatementCase,
        //         numHeads * CaseHead)
        let pattern_variable_info = a
            .flow()
            .switch_statement_end_alternatives(Some(node), member_info.has_labels);
        let variables = &member_info.variables;
        if member_info.has_labels || heads.len() > 1 {
            finish_joined_pattern_variables(
                a,
                variables,
                &pattern_variable_info.component_variables,
                &pattern_variable_info.pattern_variable_promotion_keys,
                JoinedPatternVariableLocation::SharedCaseScope,
            );
        }
        let variable_values: Vec<A::Variable> = variables.iter().map(|(_, v)| *v).collect();
        a.handle_case_after_case_heads(node, case_index, &variable_values);
        // Stack: (Expression, numExecutionPaths * StatementCase, CaseHeads)
        // If there are joined variables, declare them.
        for &statement in &member_info.body {
            a.dispatch_statement(statement);
        }
        // Stack: (Expression, numExecutionPaths * StatementCase, CaseHeads,
        //         n * Statement), where n = body.length
        last_case_terminates = !a.flow().switch_statement_after_case();
        if case_index + 1 < num_cases
            && !a.type_analyzer_options().patterns_enabled
            && !last_case_terminates
        {
            let error = a.errors().switch_case_completes_normally(node, case_index);
            switch_case_completes_normally_errors
                .get_or_insert_with(BTreeMap::new)
                .insert(case_index, error);
        }
        a.handle_merged_statement_case(node, case_index, last_case_terminates);
        // Stack: (Expression, (numExecutionPaths + 1) * StatementCase)
    }
    // Stack: (Expression, numExecutionPaths * StatementCase)
    let is_exhaustive;
    let requires_exhaustiveness_validation;
    if has_default {
        is_exhaustive = true;
        requires_exhaustiveness_validation = false;
    } else if a.type_analyzer_options().patterns_enabled {
        is_exhaustive = a.operations().is_always_exhaustive_type(scrutinee_type);
        requires_exhaustiveness_validation = is_exhaustive;
    } else {
        is_exhaustive = a.is_legacy_switch_exhaustive(node_as_node, scrutinee_type);
        requires_exhaustiveness_validation = false;
    }
    a.flow().switch_statement_end(is_exhaustive);
    SwitchStatementTypeAnalysisResult {
        has_default,
        is_exhaustive,
        last_case_terminates,
        requires_exhaustiveness_validation,
        scrutinee_type,
        switch_case_completes_normally_errors,
        non_boolean_guard_errors,
        guard_types,
    }
}

/// Analyzes a variable declaration of the form `type variable;` or `var
/// variable;`.
///
/// Returns the inferred type of the variable.
pub fn analyze_uninitialized_variable_declaration<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    variable: A::Variable,
    declared_type: Option<TypeView<Ops<A>>>,
    is_final: bool,
) -> TypeView<Ops<A>> {
    let _ = (node, is_final);
    let inferred_type = declared_type.unwrap_or_else(|| a.operations().dynamic_type());
    a.set_variable_type(variable, inferred_type);
    a.flow().declare(variable, inferred_type, false);
    inferred_type
}

/// Analyzes a wildcard pattern. `node` is the pattern.
///
/// Returns a [`WildcardPatternResult`] with information about reported
/// errors.
pub fn analyze_wildcard_pattern<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    context: &MatchContextOf<A>,
    node: A::Pattern,
    declared_type: Option<TypeView<Ops<A>>>,
) -> WildcardPatternResult<Ty<A>, A::Error> {
    let matched_value_type = a.flow().get_matched_value_type();
    let mut pattern_type_mismatch_in_irrefutable_context_error = None;
    if let Some(irrefutable_context) = context.irrefutable_context
        && let Some(declared_type) = declared_type
        && !a
            .operations()
            .is_assignable_to(matched_value_type, declared_type)
    {
        pattern_type_mismatch_in_irrefutable_context_error =
            Some(a.errors().pattern_type_mismatch_in_irrefutable_context(
                node,
                irrefutable_context,
                matched_value_type,
                declared_type,
            ));
    }

    let is_always_matching = match declared_type {
        Some(declared_type) => a
            .flow()
            .promote_for_pattern(matched_value_type, declared_type, true, false),
        None => true,
    };

    let unnecessary_wildcard_kind = context.unnecessary_wildcard_kind;
    if is_always_matching && let Some(unnecessary_wildcard_kind) = unnecessary_wildcard_kind {
        a.errors()
            .unnecessary_wildcard_pattern(node, unnecessary_wildcard_kind);
    }
    WildcardPatternResult {
        pattern_type_mismatch_in_irrefutable_context_error,
        matched_value_type,
    }
}

/// Analyzes a statement of the form `yield operand;` or `yield* operand;`.
///
/// Returns an [`YieldStatementResult`] containing the static type of the
/// operand.
pub fn analyze_yield_statement<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Statement,
    operand: A::Expression,
    is_yield_star: bool,
) -> YieldStatementResult<Ty<A>> {
    // Stack: ()

    let (shared_yield_context, is_async) = {
        let body_context = a.body_context().expect("bodyContext");
        (body_context.shared_yield_context(), body_context.is_async())
    };
    let operand_context = if schema_kind_of(a, shared_yield_context) == SharedTypeKind::Unknown {
        shared_yield_context
    } else if is_yield_star {
        if is_async {
            a.operations().stream_type_schema(shared_yield_context)
        } else {
            a.operations().iterable_type_schema(shared_yield_context)
        }
    } else {
        shared_yield_context
    };

    let operand_result = a.analyze_expression(operand, operand_context, false, true, false);
    // Stack: (operand)

    a.flow().suspension(A::statement_node(node));
    YieldStatementResult {
        operand_type: operand_result.type_,
    }
}

// --------------------------------------------------------- private helpers

/// Common functionality shared by `analyzeIfStatement` and
/// `analyzeIfCaseStatement` (Dart `_analyzeIfCommon`).
fn analyze_if_common<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Statement,
    if_true: A::Statement,
    if_false: Option<A::Statement>,
) {
    // Stack: ()
    a.dispatch_statement(if_true);
    a.handle_if_statement_then_end(node, if_true);
    // Stack: (Statement ifTrue)
    match if_false {
        None => {
            a.handle_no_statement(node);
            a.flow().if_statement_end(false);
        }
        Some(if_false) => {
            a.flow().if_statement_else_begin();
            a.dispatch_statement(if_false);
            a.flow().if_statement_end(true);
            a.handle_if_statement_else_end(node, if_false);
        }
    }
    // Stack: (Statement ifTrue, Statement ifFalse)
}

/// Common functionality shared by `analyzeIfElement` and
/// `analyzeIfCaseElement` (Dart `_analyzeIfElementCommon`).
fn analyze_if_element_common<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    node: A::Node,
    if_true: A::Node,
    if_false: Option<A::Node>,
    context: A::CollectionElementContext,
) {
    // Stack: ()
    a.dispatch_collection_element(if_true, context.clone());
    a.handle_if_element_then_end(node, if_true);
    // Stack: (CollectionElement ifTrue)
    match if_false {
        None => {
            a.handle_no_collection_element(node);
            a.flow().if_statement_end(false);
        }
        Some(if_false) => {
            a.flow().if_statement_else_begin();
            a.dispatch_collection_element(if_false, context);
            a.flow().if_statement_end(true);
            a.handle_if_element_else_end(node, if_false);
        }
    }
    // Stack: (CollectionElement ifTrue, CollectionElement ifFalse)
}

/// Dart `_checkGuardType`.
fn check_guard_type<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    expression: A::Expression,
    ty: TypeView<Ops<A>>,
) -> Option<A::Error> {
    // TODO(paulberry): harmonize this with analyzer's
    // checkForNonBoolExpression
    // TODO(paulberry): spec says the type must be `bool` or `dynamic`. This
    // logic permits `T extends bool`, `T promoted to bool`, or `Never`. What
    // do we want?
    let bool_type = a.operations().bool_type();
    if !a.operations().is_assignable_to(ty, bool_type) {
        return Some(a.errors().non_boolean_condition(expression));
    }
    None
}

/// Dart `_finishJoinedPatternVariables`. The maps are lists of pairs in Dart
/// map order.
fn finish_joined_pattern_variables<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    variables: &[(NameOf<Ops<A>>, A::Variable)],
    component_variables: &[(NameOf<Ops<A>>, Vec<A::Variable>)],
    pattern_variable_promotion_keys: &[(NameOf<Ops<A>>, PromotionKey)],
    location: JoinedPatternVariableLocation,
) {
    debug_assert!({
        // Every entry in `variables` should match a variable we know about.
        variables.iter().all(|(variable_name, _)| {
            pattern_variable_promotion_keys
                .iter()
                .any(|(k, _)| k == variable_name)
        })
    });
    for &(variable_name, promotion_key) in pattern_variable_promotion_keys {
        let variable = pairs_get(variables, &variable_name);
        let components = pairs_get(component_variables, &variable_name).unwrap_or_default();
        let mut is_first = true;
        let mut type_if_consistent: Option<TypeView<Ops<A>>> = None;
        let mut is_final_if_consistent: Option<bool> = None;
        let mut is_identical_to_component = false;
        for &component in &components {
            // Dart: identical(variable, component)
            if variable == Some(component) {
                is_identical_to_component = true;
            }
            let component_type = a.operations().variable_type(component);
            let is_component_final = a.operations().is_variable_final(component);
            if is_first {
                type_if_consistent = Some(component_type);
                is_final_if_consistent = Some(is_component_final);
                is_first = false;
            } else {
                let mut inconsistency_found = false;
                if let Some(t) = type_if_consistent
                    && !structurally_equal_after_norm_types(a, t, component_type)
                {
                    type_if_consistent = None;
                    inconsistency_found = true;
                }
                if let Some(f) = is_final_if_consistent
                    && f != is_component_final
                {
                    is_final_if_consistent = None;
                    inconsistency_found = true;
                }
                if inconsistency_found
                    && location == JoinedPatternVariableLocation::SinglePattern
                    && let Some(variable) = variable
                {
                    a.errors()
                        .inconsistent_joined_pattern_variable(variable, component);
                }
            }
        }
        if let Some(variable) = variable
            && !is_identical_to_component
        {
            let inconsistency = if type_if_consistent.is_some() && is_final_if_consistent.is_some()
            {
                JoinedPatternVariableInconsistency::None
            } else {
                JoinedPatternVariableInconsistency::DifferentFinalityOrType
            };
            let type_ = type_if_consistent.unwrap_or_else(|| a.operations().error_type());
            a.finish_joined_pattern_variable(
                variable,
                location,
                inconsistency,
                is_final_if_consistent.unwrap_or(false),
                type_,
            );
            a.flow()
                .assign_matched_pattern_variable(variable, promotion_key);
        }
    }
}

/// If the shape described by `fields` is the same as the shape of the
/// `matched_type`, returns matched types for each field in `fields`.
/// Otherwise returns `None` (Dart `_matchRecordTypeShape`).
fn match_record_type_shape<A: SharedTypeAnalyzer + ?Sized>(
    a: &A,
    fields: &[RecordPatternFieldOf<A>],
    matched_type: TypeView<Ops<A>>,
) -> Option<Vec<TypeView<Ops<A>>>> {
    let ops = a.operations();
    let record_type = matched_type.unwrap_type_view();
    let matched_type_named: Vec<(NameOf<Ops<A>>, TypeView<Ops<A>>)> = ops
        .sorted_named_types_shared(record_type)
        .into_iter()
        .map(|named| (named.name_shared, TypeView::<Ops<A>>::new(named.type_shared)))
        .collect();

    let mut result = Vec::new();
    let mut named_count = 0;
    let mut positional_iterator = ops
        .positional_types_shared(record_type)
        .into_iter()
        .map(TypeView::<Ops<A>>::new);
    for field in fields {
        let field_type = match field.name {
            Some(name) => {
                let field_type = pairs_get(&matched_type_named, &name)?;
                named_count += 1;
                field_type
            }
            None => positional_iterator.next()?,
        };
        result.push(field_type);
    }
    if positional_iterator.next().is_some() {
        return None;
    }
    if named_count != matched_type_named.len() {
        return None;
    }

    debug_assert!(result.len() == fields.len());
    Some(result)
}

/// Reports errors for duplicate named record fields (Dart
/// `_reportDuplicateRecordPatternFields`).
fn report_duplicate_record_pattern_fields<A: SharedTypeAnalyzer + ?Sized>(
    a: &mut A,
    pattern: A::Pattern,
    fields: &[RecordPatternFieldOf<A>],
) -> Option<BTreeMap<usize, A::Error>> {
    let mut error_results: Option<BTreeMap<usize, A::Error>> = None;
    let mut name_to_field: Vec<(NameOf<Ops<A>>, RecordPatternFieldOf<A>)> = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        if let Some(name) = field.name {
            let original = pairs_get(&name_to_field, &name);
            if let Some(original) = original {
                let error = a
                    .errors()
                    .duplicate_record_pattern_field(pattern, name, original, *field);
                error_results
                    .get_or_insert_with(BTreeMap::new)
                    .insert(i, error);
            } else {
                name_to_field.push((name, *field));
            }
        }
    }
    error_results
}

/// Dart `_structurallyEqualAfterNormTypes`.
fn structurally_equal_after_norm_types<A: SharedTypeAnalyzer + ?Sized>(
    a: &A,
    type1: TypeView<Ops<A>>,
    type2: TypeView<Ops<A>>,
) -> bool {
    let ops = a.operations();
    let norm1 = ops.normalize(type1);
    let norm2 = ops.normalize(type2);
    ops.is_structurally_equal_to(norm1.unwrap_type_view(), norm2.unwrap_type_view())
}

/// Applies the `TypeAnalyzer` mixin: overrides each `todo!()` provided
/// method of [`TypeAnalyzer`] with a call to the function of this module
/// with the same name. Invoke it inside `impl TypeAnalyzer for X { ... }`.
#[macro_export]
macro_rules! type_analyzer_mixin {
    () => {
        fn analyze_assigned_variable_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            variable: Self::Variable,
        ) -> $crate::__private::type_analysis_result::AssignedVariablePatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_assigned_variable_pattern(self, context, node, variable)
        }

        fn analyze_await_expression(
            &mut self,
            node: Self::Expression,
            operand: Self::Expression,
            schema: $crate::__private::shared_type::SchemaView<Self::Operations>,
        ) -> $crate::__private::type_analysis_result::AwaitExpressionResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            $crate::__private::null_shorting::ExpressionInfoOf<Self>,
        > {
            $crate::type_analyzer::analyze_await_expression(self, node, operand, schema)
        }

        fn analyze_cast_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            pattern: Self::Pattern,
            inner_pattern: Self::Pattern,
            required_type: $crate::__private::shared_type::TypeView<Self::Operations>,
        ) -> $crate::__private::type_analysis_result::PatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
        > {
            $crate::type_analyzer::analyze_cast_pattern(
                self,
                context,
                pattern,
                inner_pattern,
                required_type,
            )
        }

        fn analyze_constant_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Node,
            expression: Self::Expression,
        ) -> $crate::__private::type_analysis_result::ConstantPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_constant_pattern(self, context, node, expression)
        }

        fn analyze_declared_variable_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            variable: Self::Variable,
            variable_name: $crate::__private::shared_type::NameOf<Self::Operations>,
            declared_type: Option<$crate::__private::shared_type::TypeView<Self::Operations>>,
        ) -> $crate::__private::type_analysis_result::DeclaredVariablePatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_declared_variable_pattern(
                self,
                context,
                node,
                variable,
                variable_name,
                declared_type,
            )
        }

        fn analyze_dot_shorthand(
            &mut self,
            node: Self::Expression,
            context: $crate::__private::shared_type::SchemaView<Self::Operations>,
        ) -> $crate::__private::shared_type::TypeView<Self::Operations> {
            $crate::type_analyzer::analyze_dot_shorthand(self, node, context)
        }

        fn analyze_expression(
            &mut self,
            node: Self::Expression,
            schema: $crate::__private::shared_type::SchemaView<Self::Operations>,
            continue_null_shorting: bool,
            is_void_allowed: bool,
            needs_coercion: bool,
        ) -> $crate::__private::null_shorting::ExpressionResultOf<Self> {
            $crate::type_analyzer::analyze_expression(
                self,
                node,
                schema,
                continue_null_shorting,
                is_void_allowed,
                needs_coercion,
            )
        }

        fn analyze_if_case_element(
            &mut self,
            node: Self::Node,
            expression: Self::Expression,
            pattern: Self::Pattern,
            variables: &[(
                $crate::__private::shared_type::NameOf<Self::Operations>,
                Self::Variable,
            )],
            guard: Option<Self::Expression>,
            if_true: Self::Node,
            if_false: Option<Self::Node>,
            context: Self::CollectionElementContext,
        ) -> $crate::__private::type_analysis_result::IfCaseStatementResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_if_case_element(
                self, node, expression, pattern, variables, guard, if_true, if_false, context,
            )
        }

        fn analyze_if_case_statement(
            &mut self,
            node: Self::Statement,
            expression: Self::Expression,
            pattern: Self::Pattern,
            guard: Option<Self::Expression>,
            if_true: Self::Statement,
            if_false: Option<Self::Statement>,
            variables: &[(
                $crate::__private::shared_type::NameOf<Self::Operations>,
                Self::Variable,
            )],
        ) -> $crate::__private::type_analysis_result::IfCaseStatementResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_if_case_statement(
                self, node, expression, pattern, guard, if_true, if_false, variables,
            )
        }

        fn analyze_if_element(
            &mut self,
            node: Self::Node,
            condition: Self::Expression,
            if_true: Self::Node,
            if_false: Option<Self::Node>,
            context: Self::CollectionElementContext,
        ) {
            $crate::type_analyzer::analyze_if_element(
                self, node, condition, if_true, if_false, context,
            )
        }

        fn analyze_if_statement(
            &mut self,
            node: Self::Statement,
            condition: Self::Expression,
            if_true: Self::Statement,
            if_false: Option<Self::Statement>,
        ) {
            $crate::type_analyzer::analyze_if_statement(self, node, condition, if_true, if_false)
        }

        fn analyze_int_literal(
            &mut self,
            schema: $crate::__private::shared_type::SchemaView<Self::Operations>,
        ) -> $crate::__private::type_analysis_result::IntTypeAnalysisResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            $crate::__private::null_shorting::ExpressionInfoOf<Self>,
        > {
            $crate::type_analyzer::analyze_int_literal(self, schema)
        }

        fn analyze_list_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            element_type: Option<$crate::__private::shared_type::TypeView<Self::Operations>>,
            elements: &[Self::Node],
        ) -> $crate::__private::type_analysis_result::ListPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_list_pattern(self, context, node, element_type, elements)
        }

        fn analyze_list_pattern_schema(
            &mut self,
            element_type: Option<$crate::__private::shared_type::TypeView<Self::Operations>>,
            elements: &[Self::Node],
        ) -> $crate::__private::shared_type::SchemaView<Self::Operations> {
            $crate::type_analyzer::analyze_list_pattern_schema(self, element_type, elements)
        }

        fn analyze_logical_and_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            lhs: Self::Node,
            rhs: Self::Node,
        ) -> $crate::__private::type_analysis_result::PatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
        > {
            $crate::type_analyzer::analyze_logical_and_pattern(self, context, node, lhs, rhs)
        }

        fn analyze_logical_or_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            lhs: Self::Node,
            rhs: Self::Node,
        ) -> $crate::__private::type_analysis_result::LogicalOrPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_logical_or_pattern(self, context, node, lhs, rhs)
        }

        fn analyze_logical_or_pattern_schema(
            &mut self,
            lhs: Self::Node,
            rhs: Self::Node,
        ) -> $crate::__private::shared_type::SchemaView<Self::Operations> {
            $crate::type_analyzer::analyze_logical_or_pattern_schema(self, lhs, rhs)
        }

        fn analyze_map_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            type_arguments: Option<
                $crate::__private::type_analyzer_operations::KeyValueTypes<
                    $crate::__private::shared_type::TypeView<Self::Operations>,
                >,
            >,
            elements: &[Self::Node],
        ) -> $crate::__private::type_analysis_result::MapPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_map_pattern(self, context, node, type_arguments, elements)
        }

        fn analyze_map_pattern_schema(
            &mut self,
            type_arguments: Option<
                $crate::__private::type_analyzer_operations::KeyValueTypes<
                    $crate::__private::shared_type::TypeView<Self::Operations>,
                >,
            >,
            elements: &[Self::Node],
        ) -> $crate::__private::shared_type::SchemaView<Self::Operations> {
            $crate::type_analyzer::analyze_map_pattern_schema(self, type_arguments, elements)
        }

        fn analyze_null_check_or_assert_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            inner_pattern: Self::Pattern,
            is_assert: bool,
        ) -> $crate::__private::type_analysis_result::NullCheckOrAssertPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_null_check_or_assert_pattern(
                self,
                context,
                node,
                inner_pattern,
                is_assert,
            )
        }

        fn analyze_null_check_or_assert_pattern_schema(
            &mut self,
            inner_pattern: Self::Pattern,
            is_assert: bool,
        ) -> $crate::__private::shared_type::SchemaView<Self::Operations> {
            $crate::type_analyzer::analyze_null_check_or_assert_pattern_schema(
                self,
                inner_pattern,
                is_assert,
            )
        }

        fn analyze_object_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            fields: &[$crate::__private::type_analyzer::RecordPatternFieldOf<Self>],
        ) -> $crate::__private::type_analysis_result::ObjectPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_object_pattern(self, context, node, fields)
        }

        fn analyze_pattern_assignment(
            &mut self,
            node: Self::Expression,
            pattern: Self::Pattern,
            rhs: Self::Expression,
        ) -> $crate::__private::type_analysis_result::PatternAssignmentAnalysisResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            $crate::__private::null_shorting::ExpressionInfoOf<Self>,
        > {
            $crate::type_analyzer::analyze_pattern_assignment(self, node, pattern, rhs)
        }

        fn analyze_pattern_for_in(
            &mut self,
            node: Self::Node,
            has_await: bool,
            pattern: Self::Pattern,
            expression: Self::Expression,
            dispatch_body: &mut dyn FnMut(&mut Self),
        ) -> $crate::__private::type_analysis_result::PatternForInResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_pattern_for_in(
                self,
                node,
                has_await,
                pattern,
                expression,
                dispatch_body,
            )
        }

        fn analyze_pattern_variable_declaration(
            &mut self,
            node: Self::Node,
            pattern: Self::Pattern,
            initializer: Self::Expression,
            is_final: bool,
        ) -> $crate::__private::type_analysis_result::PatternVariableDeclarationAnalysisResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
        > {
            $crate::type_analyzer::analyze_pattern_variable_declaration(
                self,
                node,
                pattern,
                initializer,
                is_final,
            )
        }

        fn analyze_record_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            fields: &[$crate::__private::type_analyzer::RecordPatternFieldOf<Self>],
        ) -> $crate::__private::type_analysis_result::RecordPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_record_pattern(self, context, node, fields)
        }

        fn analyze_record_pattern_schema(
            &mut self,
            fields: &[$crate::__private::type_analyzer::RecordPatternFieldOf<Self>],
        ) -> $crate::__private::shared_type::SchemaView<Self::Operations> {
            $crate::type_analyzer::analyze_record_pattern_schema(self, fields)
        }

        fn analyze_relational_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            operand: Self::Expression,
        ) -> $crate::__private::type_analysis_result::RelationalPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_relational_pattern(self, context, node, operand)
        }

        fn analyze_switch_expression(
            &mut self,
            node: Self::Expression,
            scrutinee: Self::Expression,
            num_cases: usize,
            schema: $crate::__private::shared_type::SchemaView<Self::Operations>,
        ) -> $crate::__private::type_analysis_result::SwitchExpressionResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            $crate::__private::null_shorting::ExpressionInfoOf<Self>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_switch_expression(self, node, scrutinee, num_cases, schema)
        }

        fn analyze_switch_statement(
            &mut self,
            node: Self::Statement,
            scrutinee: Self::Expression,
            num_cases: usize,
        ) -> $crate::__private::type_analysis_result::SwitchStatementTypeAnalysisResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_switch_statement(self, node, scrutinee, num_cases)
        }

        fn analyze_uninitialized_variable_declaration(
            &mut self,
            node: Self::Node,
            variable: Self::Variable,
            declared_type: Option<$crate::__private::shared_type::TypeView<Self::Operations>>,
            is_final: bool,
        ) -> $crate::__private::shared_type::TypeView<Self::Operations> {
            $crate::type_analyzer::analyze_uninitialized_variable_declaration(
                self,
                node,
                variable,
                declared_type,
                is_final,
            )
        }

        fn analyze_wildcard_pattern(
            &mut self,
            context: &$crate::__private::type_analyzer::MatchContextOf<Self>,
            node: Self::Pattern,
            declared_type: Option<$crate::__private::shared_type::TypeView<Self::Operations>>,
        ) -> $crate::__private::type_analysis_result::WildcardPatternResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
            Self::Error,
        > {
            $crate::type_analyzer::analyze_wildcard_pattern(self, context, node, declared_type)
        }

        fn analyze_yield_statement(
            &mut self,
            node: Self::Statement,
            operand: Self::Expression,
            is_yield_star: bool,
        ) -> $crate::__private::type_analysis_result::YieldStatementResult<
            $crate::__private::shared_type::TypeOf<Self::Operations>,
        > {
            $crate::type_analyzer::analyze_yield_statement(self, node, operand, is_yield_star)
        }
    };
}
