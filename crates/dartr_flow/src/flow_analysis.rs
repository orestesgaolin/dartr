// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart

//! The public interface of flow analysis: the abstract class `FlowAnalysis`,
//! `FlowAnalysisNullShortingInterface`, `PropertyTarget`, the non-promotion
//! reasons and `PatternVariableInfo`.
//!
//! The implementation (`_FlowAnalysisImpl`, `FlowModel`, `PromotionModel`,
//! `SsaNode`, `Reachability`, `ExpressionInfo` and its subclasses, the
//! `_FlowContext` classes) belongs to units A9-A11. `FlowAnalysisDebug` is
//! not ported.
//!
//! Method names are the Dart names in snake case: `ifStatement_thenBegin`
//! becomes `if_statement_then_begin`. Named parameters become ordinary
//! parameters in Dart order; Dart default values are given in the docs.

use std::fmt::Debug;
use std::hash::Hash;

use crate::flow_analysis_operations::{FlowAnalysisOperations, PropertyNonPromotabilityReason};
use crate::shared_type::{NameOf, SharedTypeView};

/// A promotion key: the integer that `PromotionKeyStore` assigns to a
/// promotable thing (variable, property, `this`, `super`, or a temporary
/// pattern value). Dart `int`.
pub type PromotionKey = u32;

/// Shorthand for the [`NonPromotionReason`] of a flow analysis `F`.
pub type NonPromotionReasonOf<F> = NonPromotionReason<
    <F as FlowAnalysisNullShortingInterface>::Variable,
    <F as FlowAnalysis>::Node,
    <<F as FlowAnalysis>::Operations as FlowAnalysisOperations>::PropertyMember,
    NameOf<<F as FlowAnalysis>::Operations>,
>;

/// The result of `whyNotPromoted`: a function yielding a map whose keys are
/// types that the user might have been expecting the target to be promoted
/// to, and whose values are reasons why the corresponding promotion did not
/// occur. The map is a list of pairs in Dart (insertion) order.
///
/// Dart: `Map<SharedTypeView, NonPromotionReason> Function()`. The function
/// owns what it needs (a snapshot of the non-promotion history), so it does
/// not borrow the flow analysis.
pub type WhyNotPromoted<T, R> = Box<dyn FnOnce() -> Vec<(SharedTypeView<T>, R)>>;

/// The part of the flow analysis interface used by null shorting
/// (`NullShortingMixin`).
///
/// Object safe once the associated types are fixed.
pub trait FlowAnalysisNullShortingInterface {
    /// The client's expression node (Dart type parameter `Expression`).
    type Expression: Copy + Eq + Hash + Debug;

    /// The client's variable (Dart type parameter `Variable`).
    type Variable: Copy + Eq + Hash + Debug;

    /// The client's type structure (wrapped in [`SharedTypeView`]).
    type Type: Copy + Eq + Hash + Debug;

    /// Information gathered by flow analysis about an expression (Dart class
    /// `ExpressionInfo`). Opaque to the client; the implementation (A10)
    /// chooses the representation (for example an `Rc`).
    type ExpressionInfo: Clone + Debug;

    /// Call this method after visiting an expression using `?.`.
    fn null_aware_access_end(&mut self);

    /// Call this method after visiting a null-aware operator such as `?.` or
    /// `?[`.
    ///
    /// Not needed for a cascade; [`FlowAnalysis::cascade_expression_after_target`]
    /// does it. `target_info` is the expression info for the expression just
    /// before the null-aware operator, or `None` if the null-aware access
    /// starts a cascade section. `target_type` is its type. `guard_variable`
    /// is the variable used to desugar the null-aware access, if any.
    ///
    /// [`null_aware_access_end`](Self::null_aware_access_end) should be
    /// called after the conclusion of any null-shorting that is caused by the
    /// `?.`.
    ///
    /// Returns the expression info for the target of the null-aware access,
    /// when it is not null.
    fn null_aware_access_right_begin(
        &mut self,
        target_info: Option<Self::ExpressionInfo>,
        target_type: SharedTypeView<Self::Type>,
        guard_variable: Option<Self::Variable>,
    ) -> Option<Self::ExpressionInfo>;
}

/// Implementation of flow analysis to be shared between the analyzer and the
/// front end (the public interface).
///
/// The client creates one instance for every method, field, or top level
/// variable to be analyzed (Dart factory `FlowAnalysis(operations,
/// assignedVariables, typeAnalyzerOptions: ...)`, which creates a
/// `_FlowAnalysisImpl`; unit A10 provides the constructor), and calls the
/// methods below during a single-pass depth-first pre-order traversal of the
/// AST. The methods are named after a kind of AST node, followed by an
/// underscore, followed by when to call them.
///
/// AST node arguments let flow analysis recognize parent/child
/// relationships: in `if (x is T)` the same node is passed to
/// [`is_expression_end`](Self::is_expression_end) and then to
/// [`if_statement_then_begin`](Self::if_statement_then_begin), so the
/// promotion applies.
///
/// Dart: `FlowAnalysis<Node, Statement extends Node, Expression extends Node,
/// Variable>`. Object safe once the associated types are fixed.
pub trait FlowAnalysis: FlowAnalysisNullShortingInterface {
    /// The client's AST node (Dart type parameter `Node`).
    type Node: Copy + Eq + Hash + Debug;

    /// The client's statement node (Dart type parameter
    /// `Statement extends Node`).
    type Statement: Copy + Eq + Hash + Debug;

    /// The operations used by flow analysis.
    type Operations: FlowAnalysisOperations<Variable = Self::Variable, Type = Self::Type>;

    /// Return `true` if the current state is reachable.
    fn is_reachable(&self) -> bool;

    /// The operations object passed to the constructor.
    fn operations(&self) -> &Self::Operations;

    /// Retrieves the type that `this` is promoted to, if it is currently
    /// promoted.
    fn promoted_type_of_this(&self) -> Option<SharedTypeView<Self::Type>>;

    /// Call this method before visiting an anonymous block body.
    fn anonymous_block_body_begin(&mut self);

    /// Call this method after visiting an anonymous block body.
    fn anonymous_block_body_end(&mut self);

    /// Call this method after visiting an "as" expression.
    fn as_expression_end(
        &mut self,
        sub_expression_info: Option<Self::ExpressionInfo>,
        sub_expression_type: SharedTypeView<Self::Type>,
        cast_type: SharedTypeView<Self::Type>,
    );

    /// Call this method after visiting the condition part of an assert
    /// statement (or assert initializer).
    fn assert_after_condition(&mut self, condition_info: Option<Self::ExpressionInfo>);

    /// Call this method before visiting the condition part of an assert
    /// statement (or assert initializer).
    fn assert_begin(&mut self);

    /// Call this method after visiting an assert statement (or assert
    /// initializer).
    fn assert_end(&mut self);

    /// Call this method after visiting a reference to a variable inside a
    /// pattern assignment.
    fn assigned_variable_pattern(
        &mut self,
        node: Self::Node,
        variable: Self::Variable,
        written_type: SharedTypeView<Self::Type>,
    );

    /// Call this method when the temporary variable holding the result of a
    /// pattern match is assigned to a user-accessible variable.
    fn assign_matched_pattern_variable(
        &mut self,
        variable: Self::Variable,
        promotion_key: PromotionKey,
    );

    /// Call this method when visiting a boolean literal expression.
    fn boolean_literal(&mut self, value: bool) -> Self::ExpressionInfo;

    /// Call this method just after visiting the target of a cascade
    /// expression. Returns the effective type of the target expression
    /// during execution of the cascade sections (its non-nullable equivalent
    /// if `is_null_aware`).
    fn cascade_expression_after_target(
        &mut self,
        target_info: Option<Self::ExpressionInfo>,
        target_type: SharedTypeView<Self::Type>,
        is_null_aware: bool,
        guard_variable: Option<Self::Variable>,
    ) -> SharedTypeView<Self::Type>;

    /// Call this method just after visiting a cascade expression.
    fn cascade_expression_end(&mut self) -> Self::ExpressionInfo;

    /// Call this method just before visiting a conditional expression
    /// ("?:").
    fn conditional_condition_begin(&mut self);

    /// Call this method upon reaching the ":" part of a conditional
    /// expression ("?:").
    fn conditional_else_begin(
        &mut self,
        then_expression_info: Option<Self::ExpressionInfo>,
        then_type: SharedTypeView<Self::Type>,
    );

    /// Call this method when finishing the visit of a conditional expression
    /// ("?:").
    fn conditional_end(
        &mut self,
        conditional_expression_type: SharedTypeView<Self::Type>,
        else_expression_info: Option<Self::ExpressionInfo>,
        else_type: SharedTypeView<Self::Type>,
    ) -> Self::ExpressionInfo;

    /// Call this method upon reaching the "?" part of a conditional
    /// expression ("?:").
    fn conditional_then_begin(
        &mut self,
        condition_info: Option<Self::ExpressionInfo>,
        conditional_expression: Self::Node,
    );

    /// Call this method after processing a constant pattern.
    fn constant_pattern_end(
        &mut self,
        expression_info: Option<Self::ExpressionInfo>,
        ty: SharedTypeView<Self::Type>,
        patterns_enabled: bool,
        matched_value_type: SharedTypeView<Self::Type>,
    );

    /// Copies promotion data associated with one promotion key to another.
    fn copy_promotion_data(&mut self, source_key: PromotionKey, destination_key: PromotionKey);

    /// Registers a declaration of the `variable` in the current state.
    fn declare(
        &mut self,
        variable: Self::Variable,
        static_type: SharedTypeView<Self::Type>,
        initialized: bool,
    );

    /// Call this method after visiting a variable pattern in a non-assignment
    /// context (or a wildcard pattern). Returns the promotion key used by
    /// flow analysis to track the temporary variable holding the matched
    /// value.
    ///
    /// Dart defaults: `isFinal = false`, `isLate = false`.
    fn declared_variable_pattern(
        &mut self,
        matched_type: SharedTypeView<Self::Type>,
        static_type: SharedTypeView<Self::Type>,
        is_final: bool,
        is_late: bool,
        is_implicitly_typed: bool,
    ) -> PromotionKey;

    /// Call this method before visiting the body of a "do-while" statement.
    fn do_statement_body_begin(&mut self, do_statement: Self::Statement);

    /// Call this method after visiting the body of a "do-while" statement,
    /// and before visiting its condition.
    fn do_statement_condition_begin(&mut self);

    /// Call this method after visiting the condition of a "do-while"
    /// statement.
    fn do_statement_end(&mut self, condition_info: Option<Self::ExpressionInfo>);

    /// Call this method just after visiting the operands of a binary `==` or
    /// `!=` expression, or an invocation of `identical`.
    ///
    /// Dart default: `notEqual = false`.
    fn equality_operation_end(
        &mut self,
        left_operand_info: Option<Self::ExpressionInfo>,
        left_operand_type: SharedTypeView<Self::Type>,
        right_operand_info: Option<Self::ExpressionInfo>,
        right_operand_type: SharedTypeView<Self::Type>,
        not_equal: bool,
    ) -> Option<Self::ExpressionInfo>;

    /// Call this method after processing a relational pattern that uses an
    /// equality operator (either `==` or `!=`).
    ///
    /// Dart default: `notEqual = false`.
    fn equality_relational_pattern_end(
        &mut self,
        operand_info: Option<Self::ExpressionInfo>,
        operand_type: SharedTypeView<Self::Type>,
        not_equal: bool,
        matched_value_type: SharedTypeView<Self::Type>,
    );

    /// Performs assertion checks at the conclusion of flow analysis.
    fn finish(&mut self);

    /// Call this method just before visiting the body of a conventional "for"
    /// statement or collection element.
    fn for_body_begin(
        &mut self,
        node: Option<Self::Statement>,
        condition_info: Option<Self::ExpressionInfo>,
    );

    /// Call this method just before visiting the condition of a conventional
    /// "for" statement or collection element.
    fn for_condition_begin(&mut self, node: Self::Node);

    /// Call this method just after visiting the updaters of a conventional
    /// "for" statement or collection element.
    fn for_end(&mut self);

    /// Call this method just before visiting the updaters of a conventional
    /// "for" statement or collection element.
    fn for_updater_begin(&mut self);

    /// Call this method just before visiting the body of a "for-in" statement
    /// or collection element.
    fn for_each_body_begin(&mut self, node: Self::Node);

    /// Call this method just after visiting the body of a "for-in" statement
    /// or collection element.
    fn for_each_end(&mut self);

    /// Call this method just before visiting the body of a function
    /// expression or local function.
    fn function_expression_begin(&mut self, node: Self::Node);

    /// Call this method just after visiting the body of a function expression
    /// or local function.
    fn function_expression_end(&mut self);

    /// Gets the matched value type that should be used to type check the
    /// pattern currently being analyzed.
    fn get_matched_value_type(&self) -> SharedTypeView<Self::Type>;

    /// Call this method when visiting a break statement.
    fn handle_break(&mut self, target: Option<Self::Statement>);

    /// Call this method when visiting a continue statement.
    fn handle_continue(&mut self, target: Option<Self::Statement>);

    /// Register the fact that the current state definitely exits, e.g.
    /// returns from the body, throws an exception, etc.
    fn handle_exit(&mut self);

    /// Call this method when visiting a return statement.
    fn handle_return(&mut self);

    /// Call this method after visiting the scrutinee expression of an if-case
    /// statement.
    fn if_case_statement_after_expression(
        &mut self,
        scrutinee_info: Option<Self::ExpressionInfo>,
        scrutinee_type: SharedTypeView<Self::Type>,
    );

    /// Call this method before visiting an if-case statement.
    fn if_case_statement_begin(&mut self);

    /// Call this method after visiting pattern and guard parts of an if-case
    /// statement.
    fn if_case_statement_then_begin(&mut self, guard_info: Option<Self::ExpressionInfo>);

    /// Call this method after visiting the RHS of an if-null expression
    /// ("??") or if-null assignment ("??=").
    fn if_null_expression_end(&mut self);

    /// Call this method after visiting the LHS of an if-null expression
    /// ("??") or if-null assignment ("??=").
    fn if_null_expression_right_begin(
        &mut self,
        left_hand_side_info: Option<Self::ExpressionInfo>,
        left_hand_side_type: SharedTypeView<Self::Type>,
    );

    /// Call this method before visiting the condition part of an if
    /// statement.
    fn if_statement_condition_begin(&mut self);

    /// Call this method after visiting the "then" part of an if statement,
    /// and before visiting the "else" part.
    fn if_statement_else_begin(&mut self);

    /// Call this method after visiting an if statement.
    fn if_statement_end(&mut self, has_else: bool);

    /// Call this method after visiting the condition part of an if
    /// statement.
    fn if_statement_then_begin(
        &mut self,
        condition_info: Option<Self::ExpressionInfo>,
        if_node: Self::Node,
    );

    /// Call this method after visiting the initializer of a variable
    /// declaration, or a variable pattern that is being matched (and hence
    /// being initialized with an implicit value).
    ///
    /// Dart default: `inheritPromotableProperties = false`.
    fn initialize(
        &mut self,
        variable: Self::Variable,
        matched_type: SharedTypeView<Self::Type>,
        initializer_expression_info: Option<Self::ExpressionInfo>,
        is_final: bool,
        is_late: bool,
        is_implicitly_typed: bool,
        inherit_promotable_properties: bool,
    );

    /// Whether the `variable` is definitely assigned in the current state.
    fn is_assigned(&self, variable: Self::Variable) -> bool;

    /// Call this method after visiting the LHS of an "is" expression.
    fn is_expression_end(
        &mut self,
        sub_expression_info: Option<Self::ExpressionInfo>,
        is_not: bool,
        sub_expression_type: SharedTypeView<Self::Type>,
        checked_type: SharedTypeView<Self::Type>,
    ) -> Option<Self::ExpressionInfo>;

    /// Whether the `variable` is definitely unassigned in the current state.
    fn is_unassigned(&self, variable: Self::Variable) -> bool;

    /// Call this method before visiting a labeled statement.
    fn labeled_statement_begin(&mut self, node: Self::Statement);

    /// Call this method after visiting a labeled statement.
    fn labeled_statement_end(&mut self);

    /// Call this method just before visiting the initializer of a late
    /// variable.
    fn late_initializer_begin(&mut self, node: Self::Node);

    /// Call this method just after visiting the initializer of a late
    /// variable.
    fn late_initializer_end(&mut self);

    /// Call this method before visiting the LHS of a logical binary operation
    /// ("||" or "&&").
    fn logical_binary_op_begin(&mut self);

    /// Call this method after visiting the RHS of a logical binary operation
    /// ("||" or "&&").
    fn logical_binary_op_end(
        &mut self,
        right_operand_info: Option<Self::ExpressionInfo>,
        is_and: bool,
    ) -> Self::ExpressionInfo;

    /// Call this method after visiting the LHS of a logical binary operation
    /// ("||" or "&&").
    fn logical_binary_op_right_begin(
        &mut self,
        left_operand_info: Option<Self::ExpressionInfo>,
        whole_expression: Self::Node,
        is_and: bool,
    );

    /// Call this method after visiting a logical not ("!") expression.
    fn logical_not_end(
        &mut self,
        operand_info: Option<Self::ExpressionInfo>,
    ) -> Option<Self::ExpressionInfo>;

    /// Call this method after visiting the left hand side of a logical-or
    /// (`||`) pattern.
    fn logical_or_pattern_after_lhs(&mut self);

    /// Call this method before visiting a logical-or (`||`) pattern.
    fn logical_or_pattern_begin(&mut self);

    /// Call this method after visiting a logical-or (`||`) pattern.
    fn logical_or_pattern_end(&mut self);

    /// Call this method after processing a relational pattern that uses a
    /// non-equality operator (any operator other than `==` or `!=`).
    fn non_equality_relational_pattern_end(&mut self);

    /// Call this method just after visiting a non-null assertion (`x!`)
    /// expression.
    fn non_null_assert_end(&mut self, operand_info: Option<Self::ExpressionInfo>);

    /// Call this method after visiting the value of a null-aware map entry.
    fn null_aware_map_entry_end(&mut self, is_key_null_aware: bool);

    /// Call this method after visiting the key of a null-aware map entry.
    fn null_aware_map_entry_value_begin(
        &mut self,
        key_info: Option<Self::ExpressionInfo>,
        key_type: SharedTypeView<Self::Type>,
        is_key_null_aware: bool,
    );

    /// Call this method before visiting the subpattern of a null-check or a
    /// null-assert pattern.
    ///
    /// Not documented in Dart; the implementation returns `true` if the null
    /// check could not narrow the matched value (`_nullCheckPattern` returned
    /// no "not null" flow model).
    fn null_check_or_assert_pattern_begin(
        &mut self,
        is_assert: bool,
        matched_value_type: SharedTypeView<Self::Type>,
    ) -> bool;

    /// Call this method after visiting the subpattern of a null-check or a
    /// null-assert pattern.
    fn null_check_or_assert_pattern_end(&mut self);

    /// Call this method when encountering an expression that is a `null`
    /// literal.
    fn null_literal(&mut self, ty: SharedTypeView<Self::Type>) -> Self::ExpressionInfo;

    /// Call this method just after visiting a parenthesized expression.
    fn parenthesized_expression(
        &mut self,
        expression_info: Option<Self::ExpressionInfo>,
    ) -> Option<Self::ExpressionInfo>;

    /// Call this method just after visiting the right hand side of a pattern
    /// assignment expression, and before visiting the pattern.
    fn pattern_assignment_after_rhs(
        &mut self,
        rhs_info: Option<Self::ExpressionInfo>,
        rhs_type: SharedTypeView<Self::Type>,
    );

    /// Call this method after visiting a pattern assignment expression.
    fn pattern_assignment_end(&mut self);

    /// Call this method just after visiting the expression (which usually
    /// implements `Iterable`, but can also be `dynamic`), and before visiting
    /// the pattern or body.
    fn pattern_for_in_after_expression(&mut self, element_type: SharedTypeView<Self::Type>);

    /// Call this method after visiting the body.
    fn pattern_for_in_end(&mut self);

    /// Call this method just after visiting the initializer of a pattern
    /// variable declaration, and before visiting the pattern.
    fn pattern_variable_declaration_after_initializer(
        &mut self,
        initializer_info: Option<Self::ExpressionInfo>,
        initializer_type: SharedTypeView<Self::Type>,
    );

    /// Call this method after visiting the pattern of a pattern variable
    /// declaration.
    fn pattern_variable_declaration_end(&mut self);

    /// Call this method after visiting the subpattern of an object pattern,
    /// to restore the state that was saved by
    /// [`push_property_subpattern`](Self::push_property_subpattern).
    fn pop_property_subpattern(&mut self);

    /// Call this method after visiting a pattern's subpattern, to restore the
    /// state that was saved by [`push_subpattern`](Self::push_subpattern).
    fn pop_subpattern(&mut self);

    /// Call this method when writing to the `variable` with type
    /// `written_type` in a postfix increment or decrement operation.
    fn post_inc_dec(
        &mut self,
        node: Self::Node,
        variable: Self::Variable,
        written_type: SharedTypeView<Self::Type>,
    );

    /// The type that a property named `property_name` is promoted to, if the
    /// property is currently promoted.
    fn promoted_property_type(
        &mut self,
        target: PropertyTarget<Self::ExpressionInfo>,
        property_name: NameOf<Self::Operations>,
        property_member: Option<<Self::Operations as FlowAnalysisOperations>::PropertyMember>,
        unpromoted_type: SharedTypeView<Self::Type>,
    ) -> Option<SharedTypeView<Self::Type>>;

    /// Retrieves the type that `variable` is promoted to, if it is currently
    /// promoted.
    fn promoted_type(&self, variable: Self::Variable) -> Option<SharedTypeView<Self::Type>>;

    /// Call this method when visiting a pattern whose semantics constrain the
    /// type of the matched value.
    ///
    /// Returns `true` if `matched_type` is a subtype of `known_type` (and
    /// thus the user might need to be warned of an unnecessary cast or
    /// unnecessary wildcard pattern).
    ///
    /// Dart defaults: `matchFailsIfWrongType = true`,
    /// `matchMayFailEvenIfCorrectType = false`.
    fn promote_for_pattern(
        &mut self,
        matched_type: SharedTypeView<Self::Type>,
        known_type: SharedTypeView<Self::Type>,
        match_fails_if_wrong_type: bool,
        match_may_fail_even_if_correct_type: bool,
    ) -> bool;

    /// Call this method just after visiting a property get expression.
    ///
    /// `property_member` is whatever data structure the client uses to keep
    /// track of the field or property being accessed. Returns the promoted
    /// type (if the property is currently promoted) and the expression info
    /// for the property get.
    fn property_get(
        &mut self,
        target: PropertyTarget<Self::ExpressionInfo>,
        property_name: NameOf<Self::Operations>,
        property_member: Option<<Self::Operations as FlowAnalysisOperations>::PropertyMember>,
        unpromoted_type: SharedTypeView<Self::Type>,
    ) -> (
        Option<SharedTypeView<Self::Type>>,
        Option<Self::ExpressionInfo>,
    );

    /// The promotion chain associated with the property named
    /// `property_name` (promoted-to types only). **For testing only!**
    fn property_promotion_chain_for_testing(
        &mut self,
        target: PropertyTarget<Self::ExpressionInfo>,
        property_name: NameOf<Self::Operations>,
        property_member: Option<<Self::Operations as FlowAnalysisOperations>::PropertyMember>,
    ) -> Vec<SharedTypeView<Self::Type>>;

    /// Call this method just before analyzing a subpattern of an object
    /// pattern. If the property's type is currently promoted, the promoted
    /// type is returned.
    fn push_property_subpattern(
        &mut self,
        property_name: NameOf<Self::Operations>,
        property_member: Option<<Self::Operations as FlowAnalysisOperations>::PropertyMember>,
        unpromoted_type: SharedTypeView<Self::Type>,
    ) -> Option<SharedTypeView<Self::Type>>;

    /// Call this method just before analyzing a subpattern of a pattern.
    fn push_subpattern(&mut self, matched_type: SharedTypeView<Self::Type>);

    /// Call this method after visiting an `await` expression or `yield`
    /// statement.
    fn suspension(&mut self, node: Self::Node);

    /// Call this method just after visiting a `case` or `default` body.
    /// Returns whether the end of the case body is "locally reachable" (i.e.
    /// reachable from its start).
    fn switch_statement_after_case(&mut self) -> bool;

    /// Call this method just before visiting a `case` or `default` clause.
    fn switch_statement_begin_alternative(&mut self);

    /// Call this method just before visiting a sequence of one or more `case`
    /// or `default` clauses that share a body.
    fn switch_statement_begin_alternatives(&mut self);

    /// Call this method just after visiting the body of a switch statement.
    /// Returns whether flow analysis was able to prove the switch statement
    /// to be exhaustive (e.g. due to a `default` clause, or a pattern that is
    /// guaranteed to match the scrutinee type).
    fn switch_statement_end(&mut self, is_exhaustive: bool) -> bool;

    /// Call this method just after visiting a `case` or `default` clause.
    /// `variables` are the pattern variables of the clause, in Dart map
    /// order.
    fn switch_statement_end_alternative(
        &mut self,
        guard_info: Option<Self::ExpressionInfo>,
        variables: &[(NameOf<Self::Operations>, Self::Variable)],
    );

    /// Call this method just after visiting a sequence of one or more `case`
    /// or `default` clauses that share a body.
    fn switch_statement_end_alternatives(
        &mut self,
        node: Option<Self::Statement>,
        has_labels: bool,
    ) -> PatternVariableInfo<NameOf<Self::Operations>, Self::Variable>;

    /// Call this method just after visiting the expression part of a switch
    /// statement or expression.
    fn switch_statement_expression_end(
        &mut self,
        switch_statement: Option<Self::Statement>,
        scrutinee_info: Option<Self::ExpressionInfo>,
        scrutinee_type: SharedTypeView<Self::Type>,
    );

    /// Call this method just before changing the binding of `this`.
    fn this_binding_begin(&mut self, target_info: Option<Self::ExpressionInfo>);

    /// Call this method just after the end of a `this` binding.
    fn this_binding_end(&mut self);

    /// Call this method just after visiting the expression `this` (or the
    /// pseudo-expression `super`).
    fn this_or_super(
        &mut self,
        static_type: SharedTypeView<Self::Type>,
        is_super: bool,
    ) -> Self::ExpressionInfo;

    /// Call this method just before visiting the body of a "try/catch"
    /// statement.
    fn try_catch_statement_body_begin(&mut self);

    /// Call this method just after visiting the body of a "try/catch"
    /// statement.
    fn try_catch_statement_body_end(&mut self, body: Self::Node);

    /// Call this method just before visiting a catch clause of a "try/catch"
    /// statement.
    fn try_catch_statement_catch_begin(
        &mut self,
        exception_variable: Option<Self::Variable>,
        stack_trace_variable: Option<Self::Variable>,
    );

    /// Call this method just after visiting a catch clause of a "try/catch"
    /// statement.
    fn try_catch_statement_catch_end(&mut self);

    /// Call this method just after visiting a "try/catch" statement.
    fn try_catch_statement_end(&mut self);

    /// Call this method just before visiting the body of a "try/finally"
    /// statement.
    fn try_finally_statement_body_begin(&mut self);

    /// Call this method just after visiting a "try/finally" statement.
    fn try_finally_statement_end(&mut self);

    /// Call this method just before visiting the finally block of a
    /// "try/finally" statement.
    fn try_finally_statement_finally_begin(&mut self, body: Self::Node);

    /// The promotion chain associated with `variable` (promoted-to types
    /// only). **For testing only!**
    fn variable_promotion_chain_for_testing(
        &self,
        variable: Self::Variable,
    ) -> Vec<SharedTypeView<Self::Type>>;

    /// Call this method when encountering an expression that reads the value
    /// of a variable. Returns the promoted type (if any) and the expression
    /// info.
    fn variable_read(
        &mut self,
        variable: Self::Variable,
    ) -> (Option<SharedTypeView<Self::Type>>, Self::ExpressionInfo);

    /// Call this method after visiting the condition part of a "while"
    /// statement.
    fn while_statement_body_begin(
        &mut self,
        while_statement: Self::Statement,
        condition_info: Option<Self::ExpressionInfo>,
    );

    /// Call this method before visiting the condition part of a "while"
    /// statement.
    fn while_statement_condition_begin(&mut self, node: Self::Node);

    /// Call this method after visiting a "while" statement.
    fn while_statement_end(&mut self);

    /// Call this method when an error occurs that may be due to a lack of
    /// type promotion, to retrieve information about why an expression was
    /// not promoted.
    fn why_not_promoted(
        &mut self,
        target_info: Option<Self::ExpressionInfo>,
    ) -> WhyNotPromoted<Self::Type, NonPromotionReasonOf<Self>>;

    /// Call this method when an error occurs that may be due to a lack of
    /// type promotion, to retrieve information about why an implicit
    /// reference to `this` was not promoted.
    fn why_not_promoted_implicit_this(
        &mut self,
        static_type: SharedTypeView<Self::Type>,
    ) -> WhyNotPromoted<Self::Type, NonPromotionReasonOf<Self>>;

    /// Registers a write of the given `variable` in the current state.
    /// Returns the expression info for the full assignment expression.
    fn write(
        &mut self,
        node: Self::Node,
        variable: Self::Variable,
        written_type: SharedTypeView<Self::Type>,
        written_expression_info: Option<Self::ExpressionInfo>,
    ) -> Option<Self::ExpressionInfo>;
}

/// Target for a property access that might undergo promotion.
///
/// Dart: sealed class `PropertyTarget` with `ExpressionPropertyTarget`,
/// `CascadePropertyTarget`, `SuperPropertyTarget` and `ThisPropertyTarget`.
/// `I` is the flow analysis `ExpressionInfo`.
#[derive(Clone, Debug)]
pub enum PropertyTarget<I> {
    /// `ExpressionPropertyTarget`: an expression appearing explicitly in the
    /// source code; holds the expression info for the expression whose
    /// property is being accessed.
    Expression(Option<I>),
    /// `CascadePropertyTarget`: an implicit reference to the target of the
    /// innermost enclosing cascade expression.
    Cascade,
    /// `SuperPropertyTarget`: an implicit or explicit reference to `super`.
    Super,
    /// `ThisPropertyTarget`: an implicit reference to `this`.
    This,
}

/// Links to documentation of non-promotion reasons.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NonPromotionDocumentationLink {
    /// A private final field could not be promoted because another class in
    /// the library has a concrete getter with the same name.
    ConflictingGetter,
    /// A private final field could not be promoted because another class in
    /// the library has a non-promotable field with the same name.
    ConflictingNonPromotableField,
    /// A private final field could not be promoted because a concrete class
    /// forwards a getter with the same name to `noSuchMethod`.
    ConflictingNoSuchMethodForwarder,
    /// A private field could not be promoted because it's external.
    ExternalField,
    /// A private field could not be promoted because the library's language
    /// version is prior to field promotion support.
    FieldPromotionUnavailable,
    /// A property get could not be promoted because it doesn't refer to a
    /// field.
    NonField,
    /// A private field could not be promoted because it's not final.
    NonFinalField,
    /// Deprecated (Dart 3.1 and earlier): promotion of property gets is not
    /// supported.
    Property,
    /// A field could not be promoted because it's not private.
    PublicField,
    /// `this` could not be promoted.
    This,
    /// A local variable could not be promoted because it was written between
    /// the type test and the usage.
    Write,
    /// A local variable was demoted due to an `await` or `yield`.
    Suspension,
}

impl NonPromotionDocumentationLink {
    /// The link URL, as a text string (also Dart `toString`).
    pub fn url(self) -> &'static str {
        match self {
            Self::ConflictingGetter => "http://dart.dev/go/non-promo-conflicting-getter",
            Self::ConflictingNonPromotableField => {
                "http://dart.dev/go/non-promo-conflicting-non-promotable-field"
            }
            Self::ConflictingNoSuchMethodForwarder => {
                "http://dart.dev/go/non-promo-conflicting-noSuchMethod-forwarder"
            }
            Self::ExternalField => "http://dart.dev/go/non-promo-external-field",
            Self::FieldPromotionUnavailable => {
                "http://dart.dev/go/non-promo-field-promotion-unavailable"
            }
            Self::NonField => "http://dart.dev/go/non-promo-non-field",
            Self::NonFinalField => "http://dart.dev/go/non-promo-non-final-field",
            Self::Property => "http://dart.dev/go/non-promo-property",
            Self::PublicField => "http://dart.dev/go/non-promo-public-field",
            Self::This => "http://dart.dev/go/non-promo-this",
            Self::Write => "http://dart.dev/go/non-promo-write",
            Self::Suspension => "http://dart.dev/go/non-promo-suspension",
        }
    }
}

/// A reason why something was not promoted.
///
/// Dart: abstract class `NonPromotionReason` with subclasses
/// `DemoteViaExplicitWrite`, `DemoteViaSuspension`,
/// `PropertyNotPromotedForInherentReason`,
/// `PropertyNotPromotedForNonInherentReason` (both extend
/// `PropertyNotPromoted`) and `ThisNotPromoted`. `NonPromotionReasonVisitor`
/// and `accept` become a `match`.
///
/// `V` is the variable, `N` the AST node, `M` the property member and
/// `Name` the property name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NonPromotionReason<V, N, M, Name> {
    /// A variable was not promoted due to an explicit write to the variable.
    DemoteViaExplicitWrite {
        /// The local variable that was not promoted.
        variable: V,
        /// The node that wrote to the variable (passed to
        /// [`FlowAnalysis::write`]).
        node: N,
    },
    /// A promotion was lost because the current function was suspended (due
    /// to an `await` or `yield`) while inside a local function.
    DemoteViaSuspension {
        /// The local variable that was not promoted.
        variable: V,
        /// The node representing the suspension (passed to
        /// [`FlowAnalysis::suspension`]).
        node: N,
    },
    /// An expression was not promoted because it's a property get, and the
    /// target of the property get is inherently non-promotable.
    PropertyNotPromotedForInherentReason {
        /// The name of the property.
        property_name: Name,
        /// The field or property being accessed (the `propertyMember` passed
        /// to [`FlowAnalysis::property_get`]).
        property_member: Option<M>,
        /// The reason why the property isn't promotable.
        why_not_promotable: PropertyNonPromotabilityReason,
        /// Whether field promotion is enabled for the current library.
        field_promotion_enabled: bool,
    },
    /// An expression was not promoted because it's a property get, but the
    /// target is not inherently non-promotable: a name conflict with
    /// something else in the library, or field promotion is disabled. The
    /// client decides which reason to report.
    PropertyNotPromotedForNonInherentReason {
        /// The name of the property.
        property_name: Name,
        /// The field or property being accessed.
        property_member: Option<M>,
        /// Whether field promotion is enabled for the current library.
        field_promotion_enabled: bool,
    },
    /// An expression was not promoted because it's a reference to `this`.
    ThisNotPromoted,
}

impl<V, N, M, Name> NonPromotionReason<V, N, M, Name> {
    /// Link to documentation describing this non-promotion reason. `None`
    /// means the client needs to supply the link.
    pub fn documentation_link(&self) -> Option<NonPromotionDocumentationLink> {
        match self {
            NonPromotionReason::DemoteViaExplicitWrite { .. } => {
                Some(NonPromotionDocumentationLink::Write)
            }
            NonPromotionReason::DemoteViaSuspension { .. } => {
                Some(NonPromotionDocumentationLink::Suspension)
            }
            NonPromotionReason::PropertyNotPromotedForInherentReason {
                why_not_promotable, ..
            } => Some(match why_not_promotable {
                PropertyNonPromotabilityReason::IsNotField => {
                    NonPromotionDocumentationLink::NonField
                }
                PropertyNonPromotabilityReason::IsNotPrivate => {
                    NonPromotionDocumentationLink::PublicField
                }
                PropertyNonPromotabilityReason::IsExternal => {
                    NonPromotionDocumentationLink::ExternalField
                }
                PropertyNonPromotabilityReason::IsNotFinal => {
                    NonPromotionDocumentationLink::NonFinalField
                }
            }),
            NonPromotionReason::PropertyNotPromotedForNonInherentReason { .. } => None,
            NonPromotionReason::ThisNotPromoted => Some(NonPromotionDocumentationLink::This),
        }
    }

    /// Short text description of this non-promotion reason; intended for ID
    /// testing.
    pub fn short_name(&self) -> &'static str {
        match self {
            NonPromotionReason::DemoteViaExplicitWrite { .. } => "explicitWrite",
            NonPromotionReason::DemoteViaSuspension { .. } => "demoteViaSuspension",
            NonPromotionReason::PropertyNotPromotedForInherentReason { .. } => {
                "propertyNotPromotedForInherentReason"
            }
            NonPromotionReason::PropertyNotPromotedForNonInherentReason { .. } => {
                "PropertyNotPromotedForNonInherentReason"
            }
            NonPromotionReason::ThisNotPromoted => "thisNotPromoted",
        }
    }
}

/// The relationship among variables defined by patterns in the various
/// alternatives of a set of switch cases that share a body.
///
/// The Dart maps (`Map<String, ...>`) are lists of pairs in insertion order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PatternVariableInfo<Name, V> {
    /// Map from variable name to a list of the variables with this name
    /// defined in each case.
    pub component_variables: Vec<(Name, Vec<V>)>,

    /// Map from variable name to the promotion key used by flow analysis to
    /// track the merged variable.
    pub pattern_variable_promotion_keys: Vec<(Name, PromotionKey)>,
}

impl<Name, V> Default for PatternVariableInfo<Name, V> {
    fn default() -> Self {
        PatternVariableInfo {
            component_variables: Vec::new(),
            pattern_variable_promotion_keys: Vec::new(),
        }
    }
}
