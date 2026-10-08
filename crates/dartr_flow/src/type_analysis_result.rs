// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/type_analysis_result.dart

//! Result types of the `analyze...` methods of
//! [`TypeAnalyzer`](crate::type_analyzer::TypeAnalyzer), and `MatchContext`.
//!
//! Generic parameters: `T` is the client's type structure (wrapped in
//! [`SharedTypeView`] / [`SharedTypeSchemaView`]), `I` the flow analysis
//! `ExpressionInfo`, `E` the client's error type (Dart type parameter
//! `Error`).
//!
//! Dart subclassing (`ListPatternResult extends PatternResult`) becomes a
//! copy of the base fields plus a `From` conversion to the base struct.
//! Dart `Map<int, Error>` (keyed by case or field index) becomes a
//! `BTreeMap<usize, E>`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::flow_analysis::PromotionKey;
use crate::shared_type::{SharedTypeSchemaView, SharedTypeView};

/// The result of analyzing an expression.
#[derive(Clone, Debug)]
pub struct ExpressionTypeAnalysisResult<T, I> {
    /// The static type of the expression (Dart `type`).
    pub type_: SharedTypeView<T>,

    /// The flow analysis info associated with the expression, if any.
    pub flow_analysis_info: Option<I>,
}

impl<T, I> ExpressionTypeAnalysisResult<T, I> {
    /// `ExpressionTypeAnalysisResult(type: type)` (no flow analysis info).
    pub fn new(type_: SharedTypeView<T>) -> Self {
        ExpressionTypeAnalysisResult {
            type_,
            flow_analysis_info: None,
        }
    }
}

/// The result of analyzing an `await` expression.
#[derive(Clone, Debug)]
pub struct AwaitExpressionResult<T, I> {
    /// The static type of the expression.
    pub type_: SharedTypeView<T>,
    /// The flow analysis info associated with the expression, if any.
    pub flow_analysis_info: Option<I>,
    /// The static type of the operand.
    pub operand_type: SharedTypeView<T>,
}

impl<T, I> From<AwaitExpressionResult<T, I>> for ExpressionTypeAnalysisResult<T, I> {
    fn from(r: AwaitExpressionResult<T, I>) -> Self {
        ExpressionTypeAnalysisResult {
            type_: r.type_,
            flow_analysis_info: r.flow_analysis_info,
        }
    }
}

/// The result of analyzing an integer literal.
#[derive(Clone, Debug)]
pub struct IntTypeAnalysisResult<T, I> {
    /// The static type of the expression.
    pub type_: SharedTypeView<T>,
    /// The flow analysis info associated with the expression, if any.
    pub flow_analysis_info: Option<I>,
    /// Whether the integer literal was converted to a double.
    pub converted_to_double: bool,
}

impl<T, I> From<IntTypeAnalysisResult<T, I>> for ExpressionTypeAnalysisResult<T, I> {
    fn from(r: IntTypeAnalysisResult<T, I>) -> Self {
        ExpressionTypeAnalysisResult {
            type_: r.type_,
            flow_analysis_info: r.flow_analysis_info,
        }
    }
}

/// The result of analyzing a pattern assignment expression.
#[derive(Clone, Debug)]
pub struct PatternAssignmentAnalysisResult<T, I> {
    /// The type schema of the pattern on the left hand size of the
    /// assignment.
    pub pattern_schema: SharedTypeSchemaView<T>,
    /// The static type of the expression.
    pub type_: SharedTypeView<T>,
    /// The flow analysis info associated with the expression, if any.
    pub flow_analysis_info: Option<I>,
}

impl<T, I> From<PatternAssignmentAnalysisResult<T, I>> for ExpressionTypeAnalysisResult<T, I> {
    fn from(r: PatternAssignmentAnalysisResult<T, I>) -> Self {
        ExpressionTypeAnalysisResult {
            type_: r.type_,
            flow_analysis_info: r.flow_analysis_info,
        }
    }
}

/// The result of analyzing a switch expression.
#[derive(Clone, Debug)]
pub struct SwitchExpressionResult<T, I, E> {
    /// The static type of the expression.
    pub type_: SharedTypeView<T>,
    /// The flow analysis info associated with the expression, if any.
    pub flow_analysis_info: Option<I>,
    /// Errors for non-bool guards, keyed by case index. `None` if there are
    /// no such errors.
    pub non_boolean_guard_errors: Option<BTreeMap<usize, E>>,
    /// The types of the guard expressions, keyed by case index. `None` if
    /// there are no guards.
    pub guard_types: Option<BTreeMap<usize, SharedTypeView<T>>>,
}

impl<T, I, E> From<SwitchExpressionResult<T, I, E>> for ExpressionTypeAnalysisResult<T, I> {
    fn from(r: SwitchExpressionResult<T, I, E>) -> Self {
        ExpressionTypeAnalysisResult {
            type_: r.type_,
            flow_analysis_info: r.flow_analysis_info,
        }
    }
}

/// The result of analyzing a pattern (base of the pattern results).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PatternResult<T> {
    /// The matched value type that was used to type check the pattern.
    pub matched_value_type: SharedTypeView<T>,
}

/// Declares a pattern result struct with `matched_value_type` and a `From`
/// conversion to [`PatternResult`].
macro_rules! pattern_result {
    (
        $(#[$meta:meta])*
        pub struct $name:ident<T, E> {
            $( $(#[$fmeta:meta])* pub $field:ident : $ty:ty, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        pub struct $name<T, E> {
            $( $(#[$fmeta])* pub $field: $ty, )*
            /// The matched value type that was used to type check the pattern.
            pub matched_value_type: SharedTypeView<T>,
        }

        impl<T, E> From<$name<T, E>> for PatternResult<T> {
            fn from(r: $name<T, E>) -> Self {
                PatternResult { matched_value_type: r.matched_value_type }
            }
        }
    };
}

pattern_result! {
    /// The result of analyzing an assigned variable pattern.
    pub struct AssignedVariablePatternResult<T, E> {
        /// Error for when a variable was assigned multiple times within a
        /// pattern.
        pub duplicate_assignment_pattern_variable_error: Option<E>,
        /// Error for when the matched value type is not assignable to the
        /// variable type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a constant pattern.
    pub struct ConstantPatternResult<T, E> {
        /// The static type of the constant expression.
        pub expression_type: SharedTypeView<T>,
        /// Error for when the pattern occurred in an irrefutable context.
        pub refutable_pattern_in_irrefutable_context_error: Option<E>,
        /// Error for when the pattern, used as a case constant expression,
        /// does not have a valid type wrt. the switch expression type.
        pub case_expression_type_mismatch_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a declared variable pattern.
    pub struct DeclaredVariablePatternResult<T, E> {
        /// The static type of the variable.
        pub static_type: SharedTypeView<T>,
        /// Error for when the matched value type is not assignable to the
        /// static type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a list pattern.
    pub struct ListPatternResult<T, E> {
        /// The required type of the list pattern.
        pub required_type: SharedTypeView<T>,
        /// Errors for multiple rest patterns, keyed by the index of the
        /// pattern within the list pattern. `None` if there are none.
        pub duplicate_rest_pattern_errors: Option<BTreeMap<usize, E>>,
        /// Error for when the matched value type is not assignable to the
        /// required type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a logical-or pattern.
    pub struct LogicalOrPatternResult<T, E> {
        /// Error for when the pattern occurred in an irrefutable context.
        pub refutable_pattern_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a map pattern.
    pub struct MapPatternResult<T, E> {
        /// The required type of the map pattern.
        pub required_type: SharedTypeView<T>,
        /// Error for when the matched value type is not assignable to the
        /// required type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
        /// Error for when the map pattern is empty.
        pub empty_map_pattern_error: Option<E>,
        /// Errors for rest patterns in the map pattern, keyed by index.
        pub rest_pattern_errors: Option<BTreeMap<usize, E>>,
    }
}

pattern_result! {
    /// The result of analyzing a null-check or null-assert pattern.
    pub struct NullCheckOrAssertPatternResult<T, E> {
        /// Error for when the pattern occurred in an irrefutable context.
        pub refutable_pattern_in_irrefutable_context_error: Option<E>,
        /// Error for when the matched type is known to be non-null.
        pub matched_type_is_strictly_non_nullable_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing an object pattern.
    pub struct ObjectPatternResult<T, E> {
        /// The required type of the object pattern.
        pub required_type: SharedTypeView<T>,
        /// Errors for duplicate property names, keyed by the index of the
        /// duplicate field. `None` if there are none.
        pub duplicate_record_pattern_field_errors: Option<BTreeMap<usize, E>>,
        /// Error for when the matched value type is not assignable to the
        /// required type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a record pattern.
    pub struct RecordPatternResult<T, E> {
        /// The required type of the record pattern.
        pub required_type: SharedTypeView<T>,
        /// Errors for duplicate field names, keyed by the index of the
        /// duplicate field. `None` if there are none.
        pub duplicate_record_pattern_field_errors: Option<BTreeMap<usize, E>>,
        /// Error for when the matched value type is not assignable to the
        /// required type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a relational pattern.
    pub struct RelationalPatternResult<T, E> {
        /// The static type of the operand.
        pub operand_type: SharedTypeView<T>,
        /// Error for when the pattern occurred in an irrefutable context.
        pub refutable_pattern_in_irrefutable_context_error: Option<E>,
        /// Error for when the operand type is not assignable to the
        /// parameter type of the relational operator.
        pub argument_type_not_assignable_error: Option<E>,
        /// Error for when the relational operator does not return a bool.
        pub operator_return_type_not_assignable_to_bool_error: Option<E>,
    }
}

pattern_result! {
    /// The result of analyzing a wildcard pattern.
    pub struct WildcardPatternResult<T, E> {
        /// Error for when the matched value type is not assignable to the
        /// wildcard type in an irrefutable context.
        pub pattern_type_mismatch_in_irrefutable_context_error: Option<E>,
    }
}

/// The result of analyzing an if-case statement or element.
#[derive(Clone, Debug)]
pub struct IfCaseStatementResult<T, E> {
    /// The static type of the matched expression.
    pub matched_expression_type: SharedTypeView<T>,
    /// Error for when the guard has a non-bool type.
    pub non_boolean_guard_error: Option<E>,
    /// The type of the guard expression, if present.
    pub guard_type: Option<SharedTypeView<T>>,
}

/// The result of analyzing a pattern for-in statement or element.
#[derive(Clone, Debug)]
pub struct PatternForInResult<T, E> {
    /// The static type of the elements of the for in expression.
    pub element_type: SharedTypeView<T>,
    /// The static type of the collection of elements of the for in
    /// expression.
    pub expression_type: SharedTypeView<T>,
    /// Error for when the expression is not an iterable.
    pub pattern_for_in_expression_is_not_iterable_error: Option<E>,
}

/// The result of analyzing a pattern variable declaration.
#[derive(Clone, Copy, Debug)]
pub struct PatternVariableDeclarationAnalysisResult<T> {
    /// The type schema of the pattern on the left hand size of the
    /// declaration.
    pub pattern_schema: SharedTypeSchemaView<T>,
    /// The type of the initializer expression.
    pub initializer_type: SharedTypeView<T>,
}

/// The result of analyzing a statement (no data).
#[derive(Clone, Copy, Debug, Default)]
pub struct StatementTypeAnalysisResult;

/// The result of analyzing a switch statement.
#[derive(Clone, Debug)]
pub struct SwitchStatementTypeAnalysisResult<T, E> {
    /// Whether the switch statement had a `default` clause.
    pub has_default: bool,
    /// Whether the switch statement was exhaustive.
    pub is_exhaustive: bool,
    /// Whether the last case body in the switch statement terminated.
    pub last_case_terminates: bool,
    /// If `true`, patterns support is enabled, there is no default clause,
    /// and the static type of the scrutinee expression is an "always
    /// exhaustive" type, so flow analysis assumed (without checking) that
    /// the switch statement is exhaustive; exhaustiveness checking must
    /// validate it later.
    pub requires_exhaustiveness_validation: bool,
    /// The static type of the scrutinee expression.
    pub scrutinee_type: SharedTypeView<T>,
    /// Errors for the cases that don't complete normally, keyed by case
    /// index. `None` if there are none.
    pub switch_case_completes_normally_errors: Option<BTreeMap<usize, E>>,
    /// Errors for non-bool guards, keyed by case and head index. `None` if
    /// there are none.
    pub non_boolean_guard_errors: Option<BTreeMap<usize, BTreeMap<usize, E>>>,
    /// The types of the guard expressions, keyed by case and head index.
    /// `None` if there are no guards.
    pub guard_types: Option<BTreeMap<usize, BTreeMap<usize, SharedTypeView<T>>>>,
}

/// The result of analyzing a `yield` statement.
#[derive(Clone, Copy, Debug)]
pub struct YieldStatementResult<T> {
    /// The static type of the operand.
    pub operand_type: SharedTypeView<T>,
}

/// Why a wildcard pattern is unnecessary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnnecessaryWildcardKind {
    /// The wildcard pattern is the left or the right side of a logical-and
    /// pattern. Because we found that is always matches, it has no effect,
    /// and can be removed.
    LogicalAndPatternOperand,
}

/// A Dart map shared by reference between copies of a [`MatchContext`]
/// (the copies made by `makeRefutable` etc. see the same map, and analysis
/// adds to it). Entries are in Dart (insertion) order.
pub type SharedMap<K, V> = Rc<RefCell<Vec<(K, V)>>>;

/// Context for pattern matching (passed down to subpatterns).
///
/// `Node`, `Expression`, `Pattern` and `Variable` are the client's types;
/// `Name` is the client's name (Dart `String` keys).
#[derive(Debug)]
pub struct MatchContext<Node, Expression, Pattern, Variable, Name> {
    /// If not `None`, the match is being done in an irrefutable context, and
    /// this is the surrounding AST node that establishes the irrefutable
    /// context.
    pub irrefutable_context: Option<Node>,

    /// Indicates whether variables declared in the pattern should be
    /// `final`.
    pub is_final: bool,

    /// The switch scrutinee, or `None` if this pattern does not occur in a
    /// switch statement or switch expression, or this pattern is not the
    /// top-level pattern.
    pub switch_scrutinee: Option<Expression>,

    /// If the match is being done in a pattern assignment, the set of
    /// variables assigned so far.
    pub assigned_variables: Option<SharedMap<Variable, Pattern>>,

    /// For each variable name in the pattern, a list of the variables which
    /// might capture that variable's value, depending upon which alternative
    /// is taken in a logical-or pattern.
    pub component_variables: SharedMap<Name, Vec<Variable>>,

    /// For each variable name in the pattern, the promotion key holding the
    /// value captured by that variable.
    pub pattern_variable_promotion_keys: SharedMap<Name, PromotionKey>,

    /// If not `None`, the warning that should be issued if the pattern is
    /// `_`.
    pub unnecessary_wildcard_kind: Option<UnnecessaryWildcardKind>,
}

impl<Node: Clone, Expression: Clone, Pattern, Variable, Name> Clone
    for MatchContext<Node, Expression, Pattern, Variable, Name>
{
    /// Copies the context; the maps stay shared (as Dart references).
    fn clone(&self) -> Self {
        MatchContext {
            irrefutable_context: self.irrefutable_context.clone(),
            is_final: self.is_final,
            switch_scrutinee: self.switch_scrutinee.clone(),
            assigned_variables: self.assigned_variables.clone(),
            component_variables: Rc::clone(&self.component_variables),
            pattern_variable_promotion_keys: Rc::clone(&self.pattern_variable_promotion_keys),
            unnecessary_wildcard_kind: self.unnecessary_wildcard_kind,
        }
    }
}

impl<Node: Clone, Expression: Clone, Pattern, Variable, Name>
    MatchContext<Node, Expression, Pattern, Variable, Name>
{
    /// Returns a modified version of `self`, with `irrefutable_context` set
    /// to `None`. This is used to suppress cascading errors after reporting
    /// `refutablePatternInIrrefutableContext`.
    ///
    /// Note: like Dart, the copy also drops `unnecessary_wildcard_kind`.
    pub fn make_refutable(&self) -> Self {
        if self.irrefutable_context.is_none() {
            return self.clone();
        }
        MatchContext {
            irrefutable_context: None,
            is_final: self.is_final,
            switch_scrutinee: self.switch_scrutinee.clone(),
            assigned_variables: self.assigned_variables.clone(),
            component_variables: Rc::clone(&self.component_variables),
            pattern_variable_promotion_keys: Rc::clone(&self.pattern_variable_promotion_keys),
            unnecessary_wildcard_kind: None,
        }
    }

    /// Returns a modified version of `self`, with a new value of
    /// `pattern_variable_promotion_keys` (and `switch_scrutinee` set to
    /// `None`).
    pub fn with_promotion_keys(
        &self,
        pattern_variable_promotion_keys: SharedMap<Name, PromotionKey>,
    ) -> Self {
        MatchContext {
            irrefutable_context: self.irrefutable_context.clone(),
            is_final: self.is_final,
            switch_scrutinee: None,
            assigned_variables: self.assigned_variables.clone(),
            component_variables: Rc::clone(&self.component_variables),
            pattern_variable_promotion_keys,
            unnecessary_wildcard_kind: self.unnecessary_wildcard_kind,
        }
    }

    /// Returns a modified version of `self`, with
    /// `unnecessary_wildcard_kind` replaced and `switch_scrutinee` set to
    /// `None` (because this context is not for a top-level pattern anymore).
    pub fn with_unnecessary_wildcard_kind(
        &self,
        unnecessary_wildcard_kind: Option<UnnecessaryWildcardKind>,
    ) -> Self {
        MatchContext {
            irrefutable_context: self.irrefutable_context.clone(),
            is_final: self.is_final,
            switch_scrutinee: None,
            assigned_variables: self.assigned_variables.clone(),
            component_variables: Rc::clone(&self.component_variables),
            pattern_variable_promotion_keys: Rc::clone(&self.pattern_variable_promotion_keys),
            unnecessary_wildcard_kind,
        }
    }
}
