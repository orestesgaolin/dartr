// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart
// (class `_FlowContext` and its subclasses)

//! The `_FlowContext` classes: objects representing constructs for which
//! flow analysis information needs to be tracked.
//!
//! Dart contexts are objects that are referenced from the context stack, from
//! `_statementToContext`, from `_anonymousBlockContext` and from
//! `_SwitchAlternativesContext`, and are mutated through any of these. Here
//! contexts live in an arena owned by the flow analysis and are referenced by
//! [`ContextId`]; the class hierarchy is the enum [`FlowContext`]. A Dart
//! cast (`_stack.last as _IfContext`) is a `match` that panics on mismatch.

use std::fmt;

use super::model::{ExpressionInfo, FlowModel, FlowTypes, NameF, Reachability, VariableF};
use crate::flow_analysis::PatternVariableInfo;

/// The index of a context in the context arena.
pub type ContextId = usize;

/// Fields of Dart `_BranchTargetContext`: a language construct that can be
/// targeted by `break` or `continue` statements, such as a loop or switch
/// statement.
pub struct BranchTargetData<F: FlowTypes> {
    /// Accumulated flow model for all `break` statements seen so far, or
    /// `None` if no `break` statements have been seen yet.
    pub break_model: Option<FlowModel<F>>,

    /// Accumulated flow model for all `continue` statements seen so far, or
    /// `None` if no `continue` statements have been seen yet.
    pub continue_model: Option<FlowModel<F>>,

    /// The reachability checkpoint associated with this loop or switch
    /// statement. When analyzing deeply nested `break` and `continue`
    /// statements, their flow models need to be unsplit to this point before
    /// joining them to the control flow paths for the loop or switch.
    pub checkpoint: Reachability,
}

impl<F: FlowTypes> BranchTargetData<F> {
    pub fn new(checkpoint: Reachability) -> Self {
        BranchTargetData {
            break_model: None,
            continue_model: None,
            checkpoint,
        }
    }
}

/// Dart `_FlowContext` and its subclasses.
pub enum FlowContext<F: FlowTypes> {
    /// `_AnonymousBlockContext`: a block-bodied anonymous method.
    AnonymousBlock {
        /// Accumulated flow model for all `return` statements seen so far, or
        /// `None` if no `return` statements have been seen yet.
        return_model: Option<FlowModel<F>>,
        /// The reachability checkpoint associated with this block-bodied
        /// anonymous method. When analyzing deeply nested `return`
        /// statements, their flow models need to be unsplit to this point
        /// before joining them to `return_model`.
        checkpoint: Reachability,
        /// The context for the immediately enclosing block-bodied anonymous
        /// method, or `None` if there is none.
        previous_anonymous_block_context: Option<ContextId>,
    },

    /// `_AssertContext extends _SimpleContext`: an assert statement or
    /// assert initializer.
    Assert {
        /// The stored state.
        previous: FlowModel<F>,
        /// Flow model if the condition being asserted is true.
        condition_true: Option<FlowModel<F>>,
    },

    /// `_BranchContext`: a language construct that branches on a boolean
    /// condition, such as a logical binary operator.
    Branch {
        /// Flow model if the branch is taken.
        branch_model: FlowModel<F>,
    },

    /// `_ConditionalContext extends _BranchContext`: a conditional
    /// expression.
    Conditional {
        /// Flow model if the branch is taken.
        branch_model: FlowModel<F>,
        /// Expression info for the "then" expression, or `None` if the
        /// "then" expression hasn't been analyzed yet.
        then_info: Option<ExpressionInfo<F>>,
        /// Flow model leaving the "then" expression, or `None` if the "then"
        /// expression hasn't been analyzed yet.
        then_model: Option<FlowModel<F>>,
    },

    /// `_IfContext extends _BranchContext`: an `if` statement.
    If {
        /// Flow model if the branch is taken.
        branch_model: FlowModel<F>,
        /// Flow model associated with the state of program execution after
        /// the `if` statement executes, in the circumstance where the "then"
        /// branch is taken.
        after_then: Option<FlowModel<F>>,
    },

    /// `_BranchTargetContext`: a labeled statement or `do` statement.
    BranchTarget(BranchTargetData<F>),

    /// `_SimpleStatementContext extends _BranchTargetContext`: e.g. a "for
    /// each" statement.
    SimpleStatement {
        /// The branch target fields.
        target: BranchTargetData<F>,
        /// The stored state. For a "for each" statement, this is the state
        /// after evaluation of the iterable.
        previous: FlowModel<F>,
    },

    /// `_SwitchStatementContext extends _SimpleStatementContext`: a switch
    /// statement.
    SwitchStatement {
        /// The branch target fields.
        target: BranchTargetData<F>,
        /// The state after evaluation of the switch expression.
        previous: FlowModel<F>,
        /// Reference for the value being matched.
        matched_value_info: ExpressionInfo<F>,
        /// Flow state for the code path where no switch cases have matched
        /// yet. If we think of a switch statement as syntactic sugar for a
        /// chain of if-else statements, this is the flow state on entry to
        /// the next `if`.
        unmatched: FlowModel<F>,
    },

    /// `_WhileContext extends _BranchTargetContext`: a `while` loop (or a
    /// C-style `for` loop, which is functionally similar).
    While {
        /// The branch target fields.
        target: BranchTargetData<F>,
        /// Flow model if the condition evaluates to `false`.
        condition_false: FlowModel<F>,
    },

    /// `_FunctionExpressionContext extends _SimpleContext`: a function
    /// expression.
    FunctionExpression {
        /// The state at the point the function expression was created.
        previous: FlowModel<F>,
        /// The context for the immediately enclosing block-bodied anonymous
        /// method, or `None` if there is none.
        previous_anonymous_block_context: Option<ContextId>,
    },

    /// `_IfNullExpressionContext`: an "if-null" (`??`) expression.
    IfNullExpression {
        /// The state if the operation short-cuts (i.e. if the expression
        /// before the `??` was non-`null`).
        shortcut_state: FlowModel<F>,
    },

    /// `_NullAwareAccessContext extends _SimpleContext`: a null aware access
    /// (`?.`).
    NullAwareAccess {
        /// The stored state.
        previous: FlowModel<F>,
    },

    /// `_NullAwareMapEntryContext`: a null-aware map entry with a null-aware
    /// key.
    NullAwareMapEntry {
        /// The state if the operation short-cuts (i.e. if the key expression
        /// was `null`).
        shortcut_state: FlowModel<F>,
    },

    /// `_PatternContext`: a pattern.
    Pattern {
        /// Expression info (a reference) for the value being matched.
        matched_value_info: ExpressionInfo<F>,
    },

    /// `_OrPatternContext extends _PatternContext`: a logical-or pattern.
    OrPattern {
        /// Expression info (a reference) for the value being matched.
        matched_value_info: ExpressionInfo<F>,
        /// The value of `_unmatched` prior to entering the logical-or
        /// pattern.
        previous_unmatched: FlowModel<F>,
        /// If the left hand side of the logical-or pattern has already been
        /// traversed, the value of `_current` after traversing it.
        lhs_matched: Option<FlowModel<F>>,
    },

    /// `_PropertyPatternContext extends _PatternContext`: a subpattern of an
    /// object pattern.
    PropertyPattern {
        /// Expression info (a reference) for the value being matched.
        matched_value_info: ExpressionInfo<F>,
        /// The value of `_scrutineeReference` that was in effect prior to
        /// visiting the subpattern.
        previous_scrutinee: Option<ExpressionInfo<F>>,
    },

    /// `_TopPatternContext extends _PatternContext`: the top level of a
    /// pattern syntax tree.
    TopPattern {
        /// Expression info (a reference) for the value being matched.
        matched_value_info: ExpressionInfo<F>,
        /// The value of `_unmatched` prior to the pattern.
        previous_unmatched: Option<FlowModel<F>>,
    },

    /// `_ScrutineeContext`: a construct that can contain one or more
    /// patterns, and thus has a scrutinee.
    Scrutinee {
        /// The value of `_scrutineeReference` prior to the construct.
        previous_scrutinee_reference: Option<ExpressionInfo<F>>,
    },

    /// `_SwitchAlternativesContext`.
    SwitchAlternatives {
        /// The enclosing switch statement context.
        switch_statement_context: ContextId,
        /// Data structure accumulating information about the relationship
        /// among variables defined by patterns in the various alternatives.
        pattern_variable_info: PatternVariableInfo<NameF<F>, VariableF<F>>,
        /// The join of the flow models of the alternatives seen so far.
        combined_model: Option<FlowModel<F>>,
    },

    /// `_TryContext extends _SimpleContext`: a try statement.
    Try {
        /// The state from the beginning of the `try` block.
        previous: FlowModel<F>,
        /// If the statement is a "try/catch" statement, the flow model
        /// representing program state at the top of any `catch` block.
        before_catch: Option<FlowModel<F>>,
        /// If the statement is a "try/catch" statement, the accumulated flow
        /// model representing program state after the `try` block or one of
        /// the `catch` blocks has finished executing.
        after_body_and_catches: Option<FlowModel<F>>,
    },

    /// `_TryFinallyContext`.
    TryFinally {
        /// The flow model representing program state at the top of the
        /// `try` block.
        before_try: FlowModel<F>,
        /// The flow model representing program state at the bottom of the
        /// `try` block.
        after_try: Option<FlowModel<F>>,
        /// The flow model representing program state at the top of the
        /// `finally` block.
        before_finally: Option<FlowModel<F>>,
    },
}

impl<F: FlowTypes> FlowContext<F> {
    /// The Dart class name (`_debugType`).
    pub fn debug_type(&self) -> &'static str {
        match self {
            FlowContext::AnonymousBlock { .. } => "_AnonymousBlockContext",
            FlowContext::Assert { .. } => "_AssertContext",
            FlowContext::Branch { .. } => "_BranchContext",
            FlowContext::Conditional { .. } => "_ConditionalContext",
            FlowContext::If { .. } => "_IfContext",
            FlowContext::BranchTarget(_) => "_BranchTargetContext",
            FlowContext::SimpleStatement { .. } => "_SimpleStatementContext",
            FlowContext::SwitchStatement { .. } => "_SwitchStatementContext",
            FlowContext::While { .. } => "_WhileContext",
            FlowContext::FunctionExpression { .. } => "_FunctionExpressionContext",
            FlowContext::IfNullExpression { .. } => "_IfNullExpressionContext",
            FlowContext::NullAwareAccess { .. } => "_NullAwareAccessContext",
            FlowContext::NullAwareMapEntry { .. } => "_NullAwareMapEntryContext",
            FlowContext::Pattern { .. } => "_PatternContext",
            FlowContext::OrPattern { .. } => "_OrPatternContext",
            FlowContext::PropertyPattern { .. } => "_PropertyPatternContext",
            FlowContext::TopPattern { .. } => "_TopPatternContext",
            FlowContext::Scrutinee { .. } => "_ScrutineeContext",
            FlowContext::SwitchAlternatives { .. } => "_SwitchAlternativesContext",
            FlowContext::Try { .. } => "_TryContext",
            FlowContext::TryFinally { .. } => "_TryFinallyContext",
        }
    }

    /// Dart `is _BranchTargetContext`: the branch target fields.
    pub fn branch_target_mut(&mut self) -> Option<&mut BranchTargetData<F>> {
        match self {
            FlowContext::BranchTarget(t)
            | FlowContext::SimpleStatement { target: t, .. }
            | FlowContext::SwitchStatement { target: t, .. }
            | FlowContext::While { target: t, .. } => Some(t),
            _ => None,
        }
    }

    /// Dart `is _PatternContext`: the `_matchedValueInfo` field.
    pub fn matched_value_info(&self) -> Option<&ExpressionInfo<F>> {
        match self {
            FlowContext::Pattern { matched_value_info }
            | FlowContext::OrPattern {
                matched_value_info, ..
            }
            | FlowContext::PropertyPattern {
                matched_value_info, ..
            }
            | FlowContext::TopPattern {
                matched_value_info, ..
            } => Some(matched_value_info),
            _ => None,
        }
    }

    /// Dart `is _PatternContext`.
    pub fn is_pattern_context(&self) -> bool {
        self.matched_value_info().is_some()
    }
}

impl<F: FlowTypes> fmt::Debug for FlowContext<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(..)", self.debug_type())
    }
}
