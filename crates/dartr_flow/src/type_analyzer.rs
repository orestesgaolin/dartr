// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart

//! Type analysis logic to be shared between the analyzer and front end: the
//! `TypeAnalyzer` mixin as a trait, `TypeAnalyzerErrors`,
//! `TypeAnalyzerOptions` and the data types the client passes in.
//!
//! The mixin's abstract members are the required methods of
//! [`TypeAnalyzer`]; the client (the analyzer's `ResolverVisitor`)
//! implements them. The mixin's concrete members (`analyzeX`) are provided
//! methods; most are `todo!("<Dart name>")` and are ported by units A12
//! (expressions, switch/if-case, statements) and A13 (patterns). The
//! mixin's state (`_dotShorthands`) is reached through
//! [`TypeAnalyzer::dot_shorthands`].
//!
//! The stack effects documented in Dart ("pushes (Expression, Pattern)")
//! are not repeated here; see the Dart source.

use std::fmt::Debug;
use std::hash::Hash;

use crate::body_inference_context::SharedBodyInferenceContext;
use crate::flow_analysis::FlowAnalysis;
use crate::flow_analysis_operations::FlowAnalysisOperations;
use crate::null_shorting::{
    ExpressionInfoOf, ExpressionResultOf, TypeAnalysisNullShortingInterface,
};
use crate::shared_type::{NameOf, SchemaView, SharedTypeView, TypeOf, TypeView};
use crate::type_analysis_result::{
    AssignedVariablePatternResult, AwaitExpressionResult, ConstantPatternResult,
    DeclaredVariablePatternResult, IfCaseStatementResult, IntTypeAnalysisResult, ListPatternResult,
    LogicalOrPatternResult, MapPatternResult, MatchContext, NullCheckOrAssertPatternResult,
    ObjectPatternResult, PatternAssignmentAnalysisResult, PatternForInResult, PatternResult,
    PatternVariableDeclarationAnalysisResult, RecordPatternResult, RelationalPatternResult,
    SwitchExpressionResult, SwitchStatementTypeAnalysisResult, UnnecessaryWildcardKind,
    WildcardPatternResult, YieldStatementResult,
};
use crate::type_analyzer_operations::{KeyValueTypes, TypeAnalyzerOperations};

/// Information supplied by the client to `analyzeSwitchExpression` or
/// `analyzeSwitchStatement` about a single case head or `default` clause.
#[derive(Clone, Debug)]
pub struct CaseHeadOrDefaultInfo<Node, Expression, Variable, Name> {
    /// For a `case` clause, the case pattern. For a `default` clause,
    /// `None`.
    pub pattern: Option<Node>,

    /// The pattern variables declared in `pattern` (Dart map order). Some of
    /// them are joins of individual pattern variable declarations, and might
    /// become not consistent during type analysis.
    pub variables: Vec<(Name, Variable)>,

    /// For a `case` clause that has a guard clause, the expression following
    /// `when`. Otherwise `None`.
    pub guard: Option<Expression>,
}

/// The kind of inconsistency identified for a variable.
///
/// Ordered by declaration as in Dart; the severity is
/// [`severity`](Self::severity).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum JoinedPatternVariableInconsistency {
    /// No inconsistency.
    None,
    /// Only one branch of a logical-or pattern has the variable.
    LogicalOr,
    /// Not every case of a shared case scope has the variable.
    SharedCaseAbsent,
    /// The shared case scope has a label or `default` case.
    SharedCaseHasLabel,
    /// The finality or type of the variable components is not the same.
    /// This is reported for both logical-or and shared cases.
    DifferentFinalityOrType,
}

impl JoinedPatternVariableInconsistency {
    /// The Dart `_severity`: higher is more serious.
    pub fn severity(self) -> u8 {
        match self {
            Self::None => 0,
            Self::LogicalOr => 4,
            Self::SharedCaseAbsent => 3,
            Self::SharedCaseHasLabel => 2,
            Self::DifferentFinalityOrType => 1,
        }
    }

    /// Returns the most serious inconsistency for `self` or `other`.
    pub fn max_with(
        self,
        other: JoinedPatternVariableInconsistency,
    ) -> JoinedPatternVariableInconsistency {
        if self.severity() > other.severity() {
            self
        } else {
            other
        }
    }

    /// Returns the most serious inconsistency for `self` or `others`.
    pub fn max_with_all(
        self,
        others: impl IntoIterator<Item = JoinedPatternVariableInconsistency>,
    ) -> JoinedPatternVariableInconsistency {
        others
            .into_iter()
            .fold(self, JoinedPatternVariableInconsistency::max_with)
    }
}

/// The location where the join of a pattern variable happens.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum JoinedPatternVariableLocation {
    /// A single pattern, from `logical-or` patterns.
    SinglePattern,
    /// A shared `case` scope, when multiple `case`s share the same body.
    SharedCaseScope,
}

/// A `key: value` entry of a map pattern.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapPatternEntry<Expression, Pattern> {
    /// The key expression.
    pub key: Expression,
    /// The value pattern.
    pub value: Pattern,
}

/// Information supplied by the client about a single field in a record or
/// object pattern.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecordPatternField<Node, Pattern, Name> {
    /// The client specific node from which this object was created. It can
    /// be used for error reporting.
    pub node: Node,
    /// If not `None` then the field is named, otherwise it is positional.
    pub name: Option<Name>,
    /// The field's pattern.
    pub pattern: Pattern,
}

/// Kinds of relational pattern operators that shared analysis needs to
/// distinguish.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RelationalOperatorKind {
    /// The operator `==`
    Equals,
    /// The operator `!=`
    NotEquals,
    /// Any relational pattern operator other than `==` or `!=`
    Other,
}

/// Information about a relational operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RelationalOperatorResolution<T> {
    /// The kind of the operator.
    pub kind: RelationalOperatorKind,
    /// The type of the operator's parameter.
    pub parameter_type: SharedTypeView<T>,
    /// The operator's return type.
    pub return_type: SharedTypeView<T>,
}

/// Information supplied by the client to `analyzeSwitchExpression` about an
/// individual `case` or `default` clause.
#[derive(Clone, Debug)]
pub struct SwitchExpressionMemberInfo<Node, Expression, Variable, Name> {
    /// The case head associated with this clause.
    pub head: CaseHeadOrDefaultInfo<Node, Expression, Variable, Name>,
    /// The body of the `case` or `default` clause.
    pub expression: Expression,
}

/// Information supplied by the client to `analyzeSwitchStatement` about an
/// individual `case` or `default` clause.
#[derive(Clone, Debug)]
pub struct SwitchStatementMemberInfo<Node, Statement, Expression, Variable, Name> {
    /// The list of case heads for this case (the front end merges cases that
    /// share a body).
    pub heads: Vec<CaseHeadOrDefaultInfo<Node, Expression, Variable, Name>>,
    /// Is `true` if the group of `case` and `default` clauses has a label.
    pub has_labels: bool,
    /// The statements following this `case` or `default` clause. If empty,
    /// and this is not the last clause, this clause shares a body with the
    /// following clause.
    pub body: Vec<Statement>,
    /// The merged set of pattern variables from `heads` (Dart map order).
    pub variables: Vec<(Name, Variable)>,
}

/// Options affecting the behavior of [`TypeAnalyzer`] (and flow analysis).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeAnalyzerOptions {
    /// The "patterns" feature is enabled.
    pub patterns_enabled: bool,
    /// The "inference-update-3" feature is enabled.
    pub inference_update3_enabled: bool,
    /// Whether initializers of implicitly typed variables should be
    /// accounted for by SSA analysis (old language versions didn't, see
    /// <https://github.com/dart-lang/language/issues/1785>).
    pub respect_implicitly_typed_var_initializers: bool,
    /// The "field promotion" feature is enabled.
    pub field_promotion_enabled: bool,
    /// The "inference-update-4" feature is enabled.
    pub inference_update4_enabled: bool,
    /// Promotion of `this` is enabled.
    pub this_promotion_enabled: bool,
    /// Sound flow analysis is enabled.
    pub sound_flow_analysis_enabled: bool,
}

/// Base of the error reporting callbacks that might be reported either in
/// the "pre-visit" or the "visit" phase of type analysis.
///
/// Object safe.
pub trait TypeAnalyzerErrorsBase {
    /// Called when the [`TypeAnalyzer`] encounters a condition which should
    /// be impossible if the user's code is free from static errors, but which
    /// might arise as a result of error recovery. The client should check
    /// (preferably with an assertion) that at least one error is reported,
    /// possibly after this call.
    fn assert_in_error_recovery(&mut self);
}

/// Interface used by the shared [`TypeAnalyzer`] logic to report error
/// conditions up to the client during the "visit" phase of type analysis.
///
/// Dart: `TypeAnalyzerErrors<Node, Statement, Expression, Variable, Pattern,
/// Error>`. Object safe once the associated types are fixed.
pub trait TypeAnalyzerErrors: TypeAnalyzerErrorsBase {
    /// The client's AST node.
    type Node: Copy + Eq + Hash + Debug;
    /// The client's statement node.
    type Statement: Copy + Eq + Hash + Debug;
    /// The client's expression node.
    type Expression: Copy + Eq + Hash + Debug;
    /// The client's variable.
    type Variable: Copy + Eq + Hash + Debug;
    /// The client's pattern node.
    type Pattern: Copy + Eq + Hash + Debug;
    /// The error object returned to the caller (Dart type parameter `Error`).
    type Error;
    /// The client's type structure.
    type Type: Copy + Eq + Hash + Debug;
    /// The client's name (Dart `String`).
    type Name: Copy + Eq + Hash + Debug;

    /// Called if pattern support is disabled and a case constant's static
    /// type doesn't properly match the scrutinee's static type.
    fn case_expression_type_mismatch(
        &mut self,
        scrutinee: Self::Expression,
        case_expression: Self::Expression,
        scrutinee_type: SharedTypeView<Self::Type>,
        case_expression_type: SharedTypeView<Self::Type>,
    ) -> Self::Error;

    /// Called for variable that is assigned more than once.
    fn duplicate_assignment_pattern_variable(
        &mut self,
        variable: Self::Variable,
        original: Self::Pattern,
        duplicate: Self::Pattern,
    ) -> Self::Error;

    /// Called for a pair of named fields have the same name.
    fn duplicate_record_pattern_field(
        &mut self,
        object_or_record_pattern: Self::Pattern,
        name: Self::Name,
        original: RecordPatternField<Self::Node, Self::Pattern, Self::Name>,
        duplicate: RecordPatternField<Self::Node, Self::Pattern, Self::Name>,
    ) -> Self::Error;

    /// Called for a duplicate rest pattern found in a list or map pattern.
    fn duplicate_rest_pattern(
        &mut self,
        map_or_list_pattern: Self::Pattern,
        original: Self::Node,
        duplicate: Self::Node,
    ) -> Self::Error;

    /// Called if a map pattern does not have elements.
    fn empty_map_pattern(&mut self, pattern: Self::Pattern) -> Self::Error;

    /// Called when both branches have variables with the same name, but
    /// these variables either don't have the same finality, or their `NORM`
    /// types are not structurally equal.
    fn inconsistent_joined_pattern_variable(
        &mut self,
        variable: Self::Variable,
        component: Self::Variable,
    );

    /// Called when a null-assert or null-check pattern is used with the
    /// matched type that is strictly non-nullable, so the null check is not
    /// necessary.
    fn matched_type_is_strictly_non_nullable(
        &mut self,
        pattern: Self::Pattern,
        matched_type: SharedTypeView<Self::Type>,
    ) -> Option<Self::Error>;

    /// Called when the matched type of a cast pattern is a subtype of the
    /// required type, so the cast is not necessary.
    fn matched_type_is_subtype_of_required(
        &mut self,
        pattern: Self::Pattern,
        matched_type: SharedTypeView<Self::Type>,
        required_type: SharedTypeView<Self::Type>,
    );

    /// Called if the static type of a condition is not assignable to
    /// `bool`.
    fn non_boolean_condition(&mut self, node: Self::Expression) -> Self::Error;

    /// Called if in a pattern `for-in` statement or element, the
    /// `expression` that should be an `Iterable` (or dynamic) is actually
    /// not.
    fn pattern_for_in_expression_is_not_iterable(
        &mut self,
        node: Self::Node,
        expression: Self::Expression,
        expression_type: SharedTypeView<Self::Type>,
    ) -> Self::Error;

    /// Called if, for a pattern in an irrefutable context, the matched type
    /// of the pattern is not assignable to the required type. `context` is
    /// the containing AST node that established the irrefutable context.
    fn pattern_type_mismatch_in_irrefutable_context(
        &mut self,
        pattern: Self::Pattern,
        context: Self::Node,
        matched_type: SharedTypeView<Self::Type>,
        required_type: SharedTypeView<Self::Type>,
    ) -> Self::Error;

    /// Called if a refutable pattern is illegally used in an irrefutable
    /// context.
    fn refutable_pattern_in_irrefutable_context(
        &mut self,
        pattern: Self::Node,
        context: Self::Node,
    ) -> Self::Error;

    /// Called if the operand of the `pattern` has the type `operand_type`,
    /// which is not assignable to `parameter_type` of the invoked relational
    /// operator.
    fn relational_pattern_operand_type_not_assignable(
        &mut self,
        pattern: Self::Pattern,
        operand_type: SharedTypeView<Self::Type>,
        parameter_type: SharedTypeView<Self::Type>,
    ) -> Self::Error;

    /// Called if the `return_type` of the invoked relational operator is not
    /// assignable to `bool`.
    fn relational_pattern_operator_return_type_not_assignable_to_bool(
        &mut self,
        pattern: Self::Pattern,
        return_type: SharedTypeView<Self::Type>,
    ) -> Self::Error;

    /// Called if a rest pattern found inside a map pattern. `node` is the
    /// map pattern, `element` is the rest pattern.
    fn rest_pattern_in_map(&mut self, node: Self::Pattern, element: Self::Node) -> Self::Error;

    /// Called if one of the case bodies of a switch statement completes
    /// normally (other than the last case body), and the "patterns" feature
    /// is not enabled.
    fn switch_case_completes_normally(
        &mut self,
        node: Self::Statement,
        case_index: usize,
    ) -> Self::Error;

    /// Called when a wildcard pattern appears in the context where it is not
    /// necessary, e.g. `0 && var _` vs. `[var _]`, and does not add anything
    /// to type promotion.
    fn unnecessary_wildcard_pattern(
        &mut self,
        pattern: Self::Pattern,
        kind: UnnecessaryWildcardKind,
    );
}

/// Shorthand: the [`MatchContext`] of a type analyzer `A`.
pub type MatchContextOf<A> = MatchContext<
    <A as TypeAnalyzer>::Node,
    <A as TypeAnalysisNullShortingInterface>::Expression,
    <A as TypeAnalyzer>::Pattern,
    <A as TypeAnalysisNullShortingInterface>::Variable,
    NameOf<<A as TypeAnalysisNullShortingInterface>::Operations>,
>;

/// Shorthand: the [`RecordPatternField`] of a type analyzer `A`.
pub type RecordPatternFieldOf<A> = RecordPatternField<
    <A as TypeAnalyzer>::Node,
    <A as TypeAnalyzer>::Pattern,
    NameOf<<A as TypeAnalysisNullShortingInterface>::Operations>,
>;

/// Shorthand: the property member of a type analyzer `A`.
pub type PropertyMemberOf<A> =
    <<A as TypeAnalysisNullShortingInterface>::Operations as FlowAnalysisOperations>::PropertyMember;

/// Type analysis logic to be shared between the analyzer and front end (the
/// `TypeAnalyzer` mixin).
///
/// The client's main type inference visitor implements this trait and calls
/// the `analyze_x` methods; they call back the required methods to report
/// results, query client-specific information and dispatch the analysis of
/// child nodes.
///
/// Dart: `mixin TypeAnalyzer<Node, Statement, Expression, Variable, Pattern,
/// Error, TypeDeclarationType, TypeDeclaration> implements
/// TypeAnalysisNullShortingInterface<Expression, Variable>`. `Expression`,
/// `Variable`, `operations` and `flow` come from the supertrait; the flow
/// analysis must use the same operations type.
///
/// Not object safe (generic supertrait bounds, `Self` in callbacks); used as
/// a generic bound.
pub trait TypeAnalyzer:
    TypeAnalysisNullShortingInterface<
        Operations: TypeAnalyzerOperations<Variable = Self::Variable, AstNode = Self::Node>,
        Flow: FlowAnalysis<
            Node = Self::Node,
            Statement = Self::Statement,
            Operations = Self::Operations,
        >,
    >
{
    /// The client's AST node (Dart type parameter `Node`).
    type Node: Copy + Eq + Hash + Debug;

    /// The client's statement node (Dart type parameter `Statement`).
    type Statement: Copy + Eq + Hash + Debug;

    /// The client's pattern node (Dart type parameter `Pattern`).
    type Pattern: Copy + Eq + Hash + Debug;

    /// The error object returned by [`TypeAnalyzerErrors`] (Dart type
    /// parameter `Error`).
    type Error;

    /// The error reporter.
    type Errors: TypeAnalyzerErrors<
            Node = Self::Node,
            Statement = Self::Statement,
            Expression = Self::Expression,
            Variable = Self::Variable,
            Pattern = Self::Pattern,
            Error = Self::Error,
            Type = TypeOf<Self::Operations>,
            Name = NameOf<Self::Operations>,
        >;

    /// The client's body inference context.
    type BodyContext: SharedBodyInferenceContext<TypeOf<Self::Operations>>;

    /// The context passed through `analyzeIfElement` etc. to
    /// [`dispatch_collection_element`](Self::dispatch_collection_element)
    /// (Dart `Object? context`). The client may use an `Option`.
    type CollectionElementContext: Clone;

    // ------------------------------------------------------ abstract members

    /// Inference context information for the current function body, if the
    /// current node is inside a function body.
    fn body_context(&self) -> Option<&Self::BodyContext>;

    /// Returns the interface for reporting errors to the client.
    fn errors(&mut self) -> &mut Self::Errors;

    /// Options affecting the behavior of [`TypeAnalyzer`].
    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions;

    /// The stack of dot shorthand contexts (the mixin's state, Dart field
    /// `_dotShorthands`).
    fn dot_shorthands(&mut self) -> &mut Vec<(Self::Node, SchemaView<Self::Operations>)>;

    /// Calls the appropriate `analyze` method according to the form of
    /// collection `element` (for an `if` element,
    /// [`analyze_if_element`](Self::analyze_if_element)).
    fn dispatch_collection_element(
        &mut self,
        element: Self::Node,
        context: Self::CollectionElementContext,
    );

    /// Dispatches an expression to the appropriate `analyze` method.
    ///
    /// Dart defaults: `isVoidAllowed = false`, `needsCoercion = false`.
    fn dispatch_expression(
        &mut self,
        node: Self::Expression,
        schema: SchemaView<Self::Operations>,
        is_void_allowed: bool,
        needs_coercion: bool,
    ) -> ExpressionResultOf<Self>;

    /// Dispatches a pattern to the appropriate `analyze` method.
    fn dispatch_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        pattern: Self::Node,
    ) -> PatternResult<TypeOf<Self::Operations>>;

    /// Dispatches a pattern to the appropriate `analyze...Schema` method.
    fn dispatch_pattern_schema(&mut self, pattern: Self::Node) -> SchemaView<Self::Operations>;

    /// Dispatches a statement to the appropriate `analyze` method.
    fn dispatch_statement(&mut self, statement: Self::Statement);

    /// Infers the type for the `pattern`, should be a subtype of
    /// `matched_type`.
    fn downward_infer_object_pattern_required_type(
        &mut self,
        matched_type: TypeView<Self::Operations>,
        pattern: Self::Pattern,
    ) -> TypeView<Self::Operations>;

    /// Called after visiting an expression case. `node` is the enclosing
    /// switch expression, `case_index` the index of the case.
    fn finish_expression_case(&mut self, node: Self::Expression, case_index: usize);

    /// Called when the type and finality of a joined pattern variable are
    /// known (no Dart doc).
    fn finish_joined_pattern_variable(
        &mut self,
        variable: Self::Variable,
        location: JoinedPatternVariableLocation,
        inconsistency: JoinedPatternVariableInconsistency,
        is_final: bool,
        type_: TypeView<Self::Operations>,
    );

    /// If `element` is a map pattern entry, returns it.
    fn get_map_pattern_entry(
        &mut self,
        element: Self::Node,
    ) -> Option<MapPatternEntry<Self::Expression, Self::Pattern>>;

    /// If `node` is a rest pattern element, returns its optional pattern.
    fn get_rest_pattern_element_pattern(&mut self, node: Self::Node) -> Option<Self::Pattern>;

    /// Returns a [`SwitchExpressionMemberInfo`] describing the `index`th
    /// `case` or `default` clause in the switch expression `node`.
    fn get_switch_expression_member_info(
        &mut self,
        node: Self::Expression,
        index: usize,
    ) -> SwitchExpressionMemberInfo<
        Self::Node,
        Self::Expression,
        Self::Variable,
        NameOf<Self::Operations>,
    >;

    /// Returns a [`SwitchStatementMemberInfo`] describing the `case_index`th
    /// `case` or `default` clause in the switch statement `node`.
    fn get_switch_statement_member_info(
        &mut self,
        node: Self::Statement,
        case_index: usize,
    ) -> SwitchStatementMemberInfo<
        Self::Node,
        Self::Statement,
        Self::Expression,
        Self::Variable,
        NameOf<Self::Operations>,
    >;

    /// Called after visiting the pattern in `if-case` statement.
    fn handle_if_case_statement_after_pattern(&mut self, node: Self::Statement) {
        let _ = node;
    }

    /// Called after visiting the expression of an `if` element.
    fn handle_if_element_condition_end(&mut self, node: Self::Node) {
        let _ = node;
    }

    /// Called after visiting the `else` element of an `if` element.
    fn handle_if_element_else_end(&mut self, node: Self::Node, if_false: Self::Node) {
        let _ = (node, if_false);
    }

    /// Called after visiting the `then` element of an `if` element.
    fn handle_if_element_then_end(&mut self, node: Self::Node, if_true: Self::Node) {
        let _ = (node, if_true);
    }

    /// Called after visiting the expression of an `if` statement.
    fn handle_if_statement_condition_end(&mut self, node: Self::Statement) {
        let _ = node;
    }

    /// Called after visiting the `else` statement of an `if` statement.
    fn handle_if_statement_else_end(&mut self, node: Self::Statement, if_false: Self::Statement) {
        let _ = (node, if_false);
    }

    /// Called after visiting the `then` statement of an `if` statement.
    fn handle_if_statement_then_end(&mut self, node: Self::Statement, if_true: Self::Statement) {
        let _ = (node, if_true);
    }

    /// Called after visiting the left hand side of a logical-or (`||`)
    /// pattern.
    fn handle_logical_or_pattern_after_lhs(&mut self, node: Self::Pattern) {
        let _ = node;
    }

    /// Called after visiting a merged set of `case` / `default` clauses.
    /// `node` is the enclosing switch statement, `case_index` the index of
    /// the merged group.
    fn handle_case_after_case_heads(
        &mut self,
        node: Self::Statement,
        case_index: usize,
        variables: &[Self::Variable],
    );

    /// Called after visiting a single `case` clause (pattern and optional
    /// guard). `node` is the enclosing switch statement or switch
    /// expression.
    fn handle_case_head(&mut self, node: Self::Node, case_index: usize, sub_index: usize);

    /// Called after visiting a `default` clause.
    fn handle_default(&mut self, node: Self::Node, case_index: usize, sub_index: usize);

    /// Called after visiting a rest element in a list pattern.
    fn handle_list_pattern_rest_element(
        &mut self,
        container: Self::Pattern,
        rest_element: Self::Node,
    );

    /// Called after visiting an entry element in a map pattern.
    fn handle_map_pattern_entry(
        &mut self,
        container: Self::Pattern,
        entry_element: Self::Node,
        key_type: TypeView<Self::Operations>,
    );

    /// Called after visiting a rest element in a map pattern.
    fn handle_map_pattern_rest_element(
        &mut self,
        container: Self::Pattern,
        rest_element: Self::Node,
    );

    /// Called after visiting a merged statement case. If `is_terminating`
    /// is `true`, flow analysis has determined that the case ends in a
    /// construct that doesn't complete normally (`break`, `return`,
    /// `continue`, `throw`, infinite loop).
    fn handle_merged_statement_case(
        &mut self,
        node: Self::Statement,
        case_index: usize,
        is_terminating: bool,
    );

    /// Called when visiting a syntactic construct where there is an implicit
    /// no-op collection element (e.g. the missing `else` of an `if`
    /// element).
    fn handle_no_collection_element(&mut self, node: Self::Node);

    /// Called when visiting a `case` that lacks a guard clause (equivalent to
    /// `when true`). `node` is the enclosing switch statement, switch
    /// expression, or `if`.
    fn handle_no_guard(&mut self, node: Self::Node, case_index: usize);

    /// Called when visiting a syntactic construct where there is an implicit
    /// no-op statement (e.g. the missing `else` of an `if` statement).
    fn handle_no_statement(&mut self, node: Self::Statement);

    /// Called before visiting a single `case` or `default` clause.
    fn handle_switch_before_alternative(
        &mut self,
        node: Self::Node,
        case_index: usize,
        sub_index: usize,
    );

    /// Called after visiting the scrutinee part of a switch statement or
    /// switch expression (hook for exhaustiveness analysis). `type_` is the
    /// static type of the scrutinee.
    fn handle_switch_scrutinee(&mut self, type_: TypeView<Self::Operations>);

    /// Whether `node` is a dot shorthand.
    fn is_dot_shorthand(&mut self, node: Self::Expression) -> bool;

    /// Queries whether the switch statement or expression `node` was
    /// exhaustive. Only called if it lacks a `default` clause and patterns
    /// support is disabled.
    fn is_legacy_switch_exhaustive(
        &mut self,
        node: Self::Node,
        expression_type: TypeView<Self::Operations>,
    ) -> bool;

    /// Returns whether `node` is a rest element in a list or map pattern.
    fn is_rest_pattern_element(&mut self, node: Self::Node) -> bool;

    /// Returns whether `pattern` is a variable pattern.
    fn is_variable_pattern(&mut self, pattern: Self::Node) -> bool;

    /// Returns the property member (Dart `Object?`, passed to flow analysis)
    /// and the type of the property in `receiver_type` that corresponds to
    /// the name of the `field`. If the property cannot be resolved, the
    /// client should report an error, and return `dynamic` for recovery.
    fn resolve_object_pattern_property_get(
        &mut self,
        object_pattern: Self::Pattern,
        receiver_type: TypeView<Self::Operations>,
        field: &RecordPatternFieldOf<Self>,
    ) -> (Option<PropertyMemberOf<Self>>, TypeView<Self::Operations>);

    /// Resolves the relational operator of the relational pattern `node`
    /// against `matched_value_type`. `None` if no operator is found (invalid
    /// code, or `matched_value_type` is `dynamic`).
    fn resolve_relational_pattern_operator(
        &mut self,
        node: Self::Pattern,
        matched_value_type: TypeView<Self::Operations>,
    ) -> Option<RelationalOperatorResolution<TypeOf<Self::Operations>>>;

    /// Records that type inference has assigned `type_` to `variable`.
    /// Called once per variable, whether the type is explicit or inferred.
    fn set_variable_type(&mut self, variable: Self::Variable, type_: TypeView<Self::Operations>);

    /// Computes the type that should be inferred for an implicitly typed
    /// variable whose initializer has the type `type_`.
    fn variable_type_from_initializer_type(
        &mut self,
        type_: TypeView<Self::Operations>,
    ) -> TypeView<Self::Operations>;

    // ----------------------------------------------------- concrete members

    /// Whether the dot shorthand context stack is empty.
    fn is_dot_shorthand_context_empty(&mut self) -> bool {
        self.dot_shorthands().is_empty()
    }

    /// Returns the most recently cached dot shorthand context type.
    fn get_dot_shorthand_context(&mut self) -> SchemaView<Self::Operations> {
        self.dot_shorthands()
            .last()
            .expect("dot shorthand context")
            .1
    }

    /// Removes the most recently cached dot shorthand context type.
    fn pop_dot_shorthand_context(&mut self) {
        self.dot_shorthands().pop();
    }

    /// Caches the context type `context` of the dot shorthand `node`, unless
    /// it is already on top of the stack.
    fn push_dot_shorthand_context(
        &mut self,
        node: Self::Node,
        context: SchemaView<Self::Operations>,
    ) {
        let dot_shorthands = self.dot_shorthands();
        if dot_shorthands.last().is_none_or(|last| last.0 != node) {
            dot_shorthands.push((node, context));
        }
    }

    /// Analyzes an assigned variable pattern `node` (a variable on the left
    /// side of a pattern assignment).
    fn analyze_assigned_variable_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        variable: Self::Variable,
    ) -> AssignedVariablePatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, variable);
        todo!("analyzeAssignedVariablePattern")
    }

    /// Computes the type schema for an assigned variable pattern: the
    /// promoted (or declared) type of `variable`.
    fn analyze_assigned_variable_pattern_schema(
        &mut self,
        variable: Self::Variable,
    ) -> SchemaView<Self::Operations> {
        let ty = match self.flow().promoted_type(variable) {
            Some(promoted) => promoted,
            None => self.operations().variable_type(variable),
        };
        self.operations().type_to_schema(ty)
    }

    /// Analyzes an `await` expression.
    fn analyze_await_expression(
        &mut self,
        node: Self::Expression,
        operand: Self::Expression,
        schema: SchemaView<Self::Operations>,
    ) -> AwaitExpressionResult<TypeOf<Self::Operations>, ExpressionInfoOf<Self>> {
        let _ = (node, operand, schema);
        todo!("analyzeAwaitExpression")
    }

    /// Analyzes a cast pattern `pattern as required_type`.
    fn analyze_cast_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        pattern: Self::Pattern,
        inner_pattern: Self::Pattern,
        required_type: TypeView<Self::Operations>,
    ) -> PatternResult<TypeOf<Self::Operations>> {
        let _ = (context, pattern, inner_pattern, required_type);
        todo!("analyzeCastPattern")
    }

    /// Computes the type schema for a cast pattern (`_`).
    fn analyze_cast_pattern_schema(&mut self) -> SchemaView<Self::Operations> {
        self.operations().unknown_type()
    }

    /// Analyzes a constant pattern.
    fn analyze_constant_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Node,
        expression: Self::Expression,
    ) -> ConstantPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, expression);
        todo!("analyzeConstantPattern")
    }

    /// Computes the type schema for a constant pattern. Only reachable in
    /// code with errors (constant patterns are refutable).
    fn analyze_constant_pattern_schema(&mut self) -> SchemaView<Self::Operations> {
        self.errors().assert_in_error_recovery();
        self.operations().unknown_type()
    }

    /// Analyzes a variable pattern in a non-assignment context.
    fn analyze_declared_variable_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        variable: Self::Variable,
        variable_name: NameOf<Self::Operations>,
        declared_type: Option<TypeView<Self::Operations>>,
    ) -> DeclaredVariablePatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, variable, variable_name, declared_type);
        todo!("analyzeDeclaredVariablePattern")
    }

    /// Computes the type schema for a variable pattern in a non-assignment
    /// context: the declared type, or `_`.
    fn analyze_declared_variable_pattern_schema(
        &mut self,
        declared_type: Option<TypeView<Self::Operations>>,
    ) -> SchemaView<Self::Operations> {
        match declared_type {
            None => self.operations().unknown_type(),
            Some(declared_type) => self.operations().type_to_schema(declared_type),
        }
    }

    /// Analyzes a dot shorthand `node` with context `context`.
    fn analyze_dot_shorthand(
        &mut self,
        node: Self::Expression,
        context: SchemaView<Self::Operations>,
    ) -> TypeView<Self::Operations> {
        let _ = (node, context);
        todo!("analyzeDotShorthand")
    }

    /// Analyzes an expression (dispatches it, applies the context, finishes
    /// null shorting).
    ///
    /// Dart defaults: `continueNullShorting = false`, `isVoidAllowed =
    /// false`, `needsCoercion = false`.
    fn analyze_expression(
        &mut self,
        node: Self::Expression,
        schema: SchemaView<Self::Operations>,
        continue_null_shorting: bool,
        is_void_allowed: bool,
        needs_coercion: bool,
    ) -> ExpressionResultOf<Self> {
        let _ = (
            node,
            schema,
            continue_null_shorting,
            is_void_allowed,
            needs_coercion,
        );
        todo!("analyzeExpression")
    }

    /// Analyzes an if-case element.
    fn analyze_if_case_element(
        &mut self,
        node: Self::Node,
        expression: Self::Expression,
        pattern: Self::Pattern,
        variables: &[(NameOf<Self::Operations>, Self::Variable)],
        guard: Option<Self::Expression>,
        if_true: Self::Node,
        if_false: Option<Self::Node>,
        context: Self::CollectionElementContext,
    ) -> IfCaseStatementResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (
            node, expression, pattern, variables, guard, if_true, if_false, context,
        );
        todo!("analyzeIfCaseElement")
    }

    /// Analyzes an if-case statement.
    fn analyze_if_case_statement(
        &mut self,
        node: Self::Statement,
        expression: Self::Expression,
        pattern: Self::Pattern,
        guard: Option<Self::Expression>,
        if_true: Self::Statement,
        if_false: Option<Self::Statement>,
        variables: &[(NameOf<Self::Operations>, Self::Variable)],
    ) -> IfCaseStatementResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (
            node, expression, pattern, guard, if_true, if_false, variables,
        );
        todo!("analyzeIfCaseStatement")
    }

    /// Analyzes an if element.
    fn analyze_if_element(
        &mut self,
        node: Self::Node,
        condition: Self::Expression,
        if_true: Self::Node,
        if_false: Option<Self::Node>,
        context: Self::CollectionElementContext,
    ) {
        let _ = (node, condition, if_true, if_false, context);
        todo!("analyzeIfElement")
    }

    /// Analyzes an if statement.
    fn analyze_if_statement(
        &mut self,
        node: Self::Statement,
        condition: Self::Expression,
        if_true: Self::Statement,
        if_false: Option<Self::Statement>,
    ) {
        let _ = (node, condition, if_true, if_false);
        todo!("analyzeIfStatement")
    }

    /// Analyzes an integer literal (int-to-double conversion).
    fn analyze_int_literal(
        &mut self,
        schema: SchemaView<Self::Operations>,
    ) -> IntTypeAnalysisResult<TypeOf<Self::Operations>, ExpressionInfoOf<Self>> {
        let _ = schema;
        todo!("analyzeIntLiteral")
    }

    /// Analyzes a list pattern.
    fn analyze_list_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        element_type: Option<TypeView<Self::Operations>>,
        elements: &[Self::Node],
    ) -> ListPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, element_type, elements);
        todo!("analyzeListPattern")
    }

    /// Computes the type schema for a list pattern.
    fn analyze_list_pattern_schema(
        &mut self,
        element_type: Option<TypeView<Self::Operations>>,
        elements: &[Self::Node],
    ) -> SchemaView<Self::Operations> {
        let _ = (element_type, elements);
        todo!("analyzeListPatternSchema")
    }

    /// Analyzes a logical-and pattern.
    fn analyze_logical_and_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        lhs: Self::Node,
        rhs: Self::Node,
    ) -> PatternResult<TypeOf<Self::Operations>> {
        let _ = (context, node, lhs, rhs);
        todo!("analyzeLogicalAndPattern")
    }

    /// Computes the type schema for a logical-and pattern: the greatest
    /// lower bound of the operand schemas.
    fn analyze_logical_and_pattern_schema(
        &mut self,
        lhs: Self::Node,
        rhs: Self::Node,
    ) -> SchemaView<Self::Operations> {
        let lhs_schema = self.dispatch_pattern_schema(lhs);
        let rhs_schema = self.dispatch_pattern_schema(rhs);
        self.operations().type_schema_glb(lhs_schema, rhs_schema)
    }

    /// Analyzes a logical-or pattern.
    fn analyze_logical_or_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        lhs: Self::Node,
        rhs: Self::Node,
    ) -> LogicalOrPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, lhs, rhs);
        todo!("analyzeLogicalOrPattern")
    }

    /// Computes the type schema for a logical-or pattern.
    fn analyze_logical_or_pattern_schema(
        &mut self,
        lhs: Self::Node,
        rhs: Self::Node,
    ) -> SchemaView<Self::Operations> {
        let _ = (lhs, rhs);
        todo!("analyzeLogicalOrPatternSchema")
    }

    /// Analyzes a map pattern.
    fn analyze_map_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        type_arguments: Option<KeyValueTypes<TypeView<Self::Operations>>>,
        elements: &[Self::Node],
    ) -> MapPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, type_arguments, elements);
        todo!("analyzeMapPattern")
    }

    /// Computes the type schema for a map pattern.
    fn analyze_map_pattern_schema(
        &mut self,
        type_arguments: Option<KeyValueTypes<TypeView<Self::Operations>>>,
        elements: &[Self::Node],
    ) -> SchemaView<Self::Operations> {
        let _ = (type_arguments, elements);
        todo!("analyzeMapPatternSchema")
    }

    /// Analyzes a null-check (`?`) or null-assert (`!`) pattern.
    fn analyze_null_check_or_assert_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        inner_pattern: Self::Pattern,
        is_assert: bool,
    ) -> NullCheckOrAssertPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, inner_pattern, is_assert);
        todo!("analyzeNullCheckOrAssertPattern")
    }

    /// Computes the type schema for a null-check or null-assert pattern.
    fn analyze_null_check_or_assert_pattern_schema(
        &mut self,
        inner_pattern: Self::Pattern,
        is_assert: bool,
    ) -> SchemaView<Self::Operations> {
        let _ = (inner_pattern, is_assert);
        todo!("analyzeNullCheckOrAssertPatternSchema")
    }

    /// Analyzes an object pattern.
    fn analyze_object_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        fields: &[RecordPatternFieldOf<Self>],
    ) -> ObjectPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, fields);
        todo!("analyzeObjectPattern")
    }

    /// Computes the type schema for an object pattern: `type_` as a schema.
    fn analyze_object_pattern_schema(
        &mut self,
        type_: TypeView<Self::Operations>,
    ) -> SchemaView<Self::Operations> {
        self.operations().type_to_schema(type_)
    }

    /// Analyzes a pattern assignment expression `pattern = rhs`.
    fn analyze_pattern_assignment(
        &mut self,
        node: Self::Expression,
        pattern: Self::Pattern,
        rhs: Self::Expression,
    ) -> PatternAssignmentAnalysisResult<TypeOf<Self::Operations>, ExpressionInfoOf<Self>> {
        let _ = (node, pattern, rhs);
        todo!("analyzePatternAssignment")
    }

    /// Analyzes a pattern for-in statement or element. `dispatch_body`
    /// analyzes the body (Dart `void Function() dispatchBody`).
    fn analyze_pattern_for_in(
        &mut self,
        node: Self::Node,
        has_await: bool,
        pattern: Self::Pattern,
        expression: Self::Expression,
        dispatch_body: &mut dyn FnMut(&mut Self),
    ) -> PatternForInResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (node, has_await, pattern, expression, dispatch_body);
        todo!("analyzePatternForIn")
    }

    /// Analyzes a pattern variable declaration.
    fn analyze_pattern_variable_declaration(
        &mut self,
        node: Self::Node,
        pattern: Self::Pattern,
        initializer: Self::Expression,
        is_final: bool,
    ) -> PatternVariableDeclarationAnalysisResult<TypeOf<Self::Operations>> {
        let _ = (node, pattern, initializer, is_final);
        todo!("analyzePatternVariableDeclaration")
    }

    /// Analyzes a record pattern.
    fn analyze_record_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        fields: &[RecordPatternFieldOf<Self>],
    ) -> RecordPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, fields);
        todo!("analyzeRecordPattern")
    }

    /// Computes the type schema for a record pattern.
    fn analyze_record_pattern_schema(
        &mut self,
        fields: &[RecordPatternFieldOf<Self>],
    ) -> SchemaView<Self::Operations> {
        let _ = fields;
        todo!("analyzeRecordPatternSchema")
    }

    /// Analyzes a relational pattern.
    fn analyze_relational_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        operand: Self::Expression,
    ) -> RelationalPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, operand);
        todo!("analyzeRelationalPattern")
    }

    /// Computes the type schema for a relational pattern. Only reachable in
    /// code with errors (relational patterns are refutable).
    fn analyze_relational_pattern_schema(&mut self) -> SchemaView<Self::Operations> {
        self.errors().assert_in_error_recovery();
        self.operations().unknown_type()
    }

    /// Analyzes a switch expression with `num_cases` cases.
    fn analyze_switch_expression(
        &mut self,
        node: Self::Expression,
        scrutinee: Self::Expression,
        num_cases: usize,
        schema: SchemaView<Self::Operations>,
    ) -> SwitchExpressionResult<TypeOf<Self::Operations>, ExpressionInfoOf<Self>, Self::Error> {
        let _ = (node, scrutinee, num_cases, schema);
        todo!("analyzeSwitchExpression")
    }

    /// Analyzes a switch statement with `num_cases` (merged) cases.
    fn analyze_switch_statement(
        &mut self,
        node: Self::Statement,
        scrutinee: Self::Expression,
        num_cases: usize,
    ) -> SwitchStatementTypeAnalysisResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (node, scrutinee, num_cases);
        todo!("analyzeSwitchStatement")
    }

    /// Analyzes a variable declaration without an initializer. Returns the
    /// variable's type.
    fn analyze_uninitialized_variable_declaration(
        &mut self,
        node: Self::Node,
        variable: Self::Variable,
        declared_type: Option<TypeView<Self::Operations>>,
        is_final: bool,
    ) -> TypeView<Self::Operations> {
        let _ = (node, variable, declared_type, is_final);
        todo!("analyzeUninitializedVariableDeclaration")
    }

    /// Analyzes a wildcard pattern.
    fn analyze_wildcard_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: Self::Pattern,
        declared_type: Option<TypeView<Self::Operations>>,
    ) -> WildcardPatternResult<TypeOf<Self::Operations>, Self::Error> {
        let _ = (context, node, declared_type);
        todo!("analyzeWildcardPattern")
    }

    /// Computes the type schema for a wildcard pattern: the declared type,
    /// or `_`.
    fn analyze_wildcard_pattern_schema(
        &mut self,
        declared_type: Option<TypeView<Self::Operations>>,
    ) -> SchemaView<Self::Operations> {
        match declared_type {
            None => self.operations().unknown_type(),
            Some(declared_type) => self.operations().type_to_schema(declared_type),
        }
    }

    /// Analyzes a `yield` or `yield*` statement.
    fn analyze_yield_statement(
        &mut self,
        node: Self::Statement,
        operand: Self::Expression,
        is_yield_star: bool,
    ) -> YieldStatementResult<TypeOf<Self::Operations>> {
        let _ = (node, operand, is_yield_star);
        todo!("analyzeYieldStatement")
    }
}
