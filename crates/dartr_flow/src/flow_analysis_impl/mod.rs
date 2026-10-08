// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart
// (class `_FlowAnalysisImpl`, `_PropertyTargetHelper`, `PropertyTarget._getSsaNode`,
// the `_EqualityCheckResult` classes)

//! The implementation of flow analysis (units A9-A11).
//!
//! - [`model`]: the data model (`FlowModel`, `PromotionModel`, `SsaNode`,
//!   `Reachability`, `ExpressionInfo` and its subclasses, `FlowModelHelper`).
//! - [`contexts`]: the `_FlowContext` classes.
//! - this module: [`FlowAnalysisImpl`] (Dart `_FlowAnalysisImpl`), which
//!   implements the [`FlowAnalysis`] trait.
//!
//! `FlowAnalysisDebug` (a wrapper printing every call) and `_dumpState` are
//! not ported.

pub mod contexts;
pub mod model;

use std::cell::RefCell;

use indexmap::{IndexMap, IndexSet};

use crate::assigned_variables::{AssignedVariables, AssignedVariablesImpl};
use crate::flow_analysis::{
    FlowAnalysis, FlowAnalysisNullShortingInterface, NonPromotionReason, NonPromotionReasonOf,
    PatternVariableInfo, PromotionKey, PropertyTarget, WhyNotPromoted,
};
use crate::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
    TypeClassification,
};
use crate::flow_link::FlowLinkReader;
use crate::promotion_key_store::PromotionKeyStore;
use crate::shared_type::{SharedTypeKind, SharedTypeOperations};
use crate::type_analyzer::TypeAnalyzerOptions;

use contexts::{BranchTargetData, ContextId, FlowContext};
use model::{
    ExpressionInfo, FlowModel, FlowModelHelper, FlowTypes, NameF, NonPromotionReasonF,
    PromotionModel, PropertyMemberF, Reachability, ReferenceKind, SsaNode, TypeList, TypeOfF,
    TypeViewF, VariableF, empty_type_list, opt_ptr_eq, promotion_info_get,
};

/// Safely downcasts `expression_info` to a `_Reference`.
///
/// If `expression_info` is a reference, it is returned. Otherwise, `None`
/// is returned.
fn get_expression_reference<F: FlowTypes>(
    expression_info: Option<&ExpressionInfo<F>>,
) -> Option<&ExpressionInfo<F>> {
    expression_info.filter(|info| info.as_reference().is_some())
}

/// Result of performing equality check (Dart sealed class
/// `_EqualityCheckResult`).
enum EqualityCheckResult<F: FlowTypes> {
    /// `_EqualityCheckIsNullCheck`: exactly one of the two operands is a
    /// `null` literal (and therefore the equality test is testing whether
    /// the other operand is `null`).
    IsNullCheck {
        /// If the operand that is being null-tested is something that can
        /// undergo type promotion, the reference. Otherwise, `None`.
        reference: Option<ExpressionInfo<F>>,
        /// If `true` the operand that's being null-tested corresponds to
        /// `_equalityCheck`'s `rightOperandInfo` argument; if `false`, it
        /// corresponds to its `leftOperandInfo` argument.
        is_reference_on_right: bool,
    },
    /// `_GuaranteedEqual`: the two operands are guaranteed to be equal (both
    /// have type `Null`).
    GuaranteedEqual,
    /// `_GuaranteedNotEqual`: the two operands are guaranteed to be not
    /// equal (one has type `Null` and the other a non-nullable type, with
    /// sound flow analysis).
    GuaranteedNotEqual,
    /// `_NoEqualityInformation`: no conclusion can be drawn.
    NoEqualityInformation,
}

/// Implementation of flow analysis to be shared between the analyzer and the
/// front end (Dart `_FlowAnalysisImpl`).
///
/// Create one instance for every method, field, or top level variable to be
/// analyzed with [`FlowAnalysisImpl::new`] (Dart factory `FlowAnalysis(...)`)
/// and drive it through the [`FlowAnalysis`] trait.
pub struct FlowAnalysisImpl<F: FlowTypes> {
    /// Language features affecting the behavior of flow analysis.
    type_analyzer_options: TypeAnalyzerOptions,

    /// The operations, used to access types, check subtyping, and query
    /// variable types.
    operations: F::Ops,

    /// Arena of all contexts created so far (see [`contexts`]).
    contexts: Vec<FlowContext<F>>,

    /// Stack of contexts representing the statements and expressions that
    /// are currently being visited.
    stack: Vec<ContextId>,

    /// The mapping from statements that can act as targets for `break` and
    /// `continue` statements (i.e. loops and switch statements) to their
    /// context information. Only used for lookup.
    statement_to_context: IndexMap<F::Statement, ContextId>,

    /// The current flow model.
    current: FlowModel<F>,

    /// If a pattern is being analyzed, flow model representing all code
    /// paths accumulated so far in which the pattern fails to match.
    /// Otherwise `None`.
    unmatched: Option<FlowModel<F>>,

    /// If a pattern is being analyzed, and the scrutinee is something that
    /// might be relevant to type promotion as a consequence of the pattern
    /// match, reference to the scrutinee. Otherwise `None`.
    scrutinee_reference: Option<ExpressionInfo<F>>,

    /// The assigned variables (owns the promotion key store).
    assigned_variables: AssignedVariablesImpl<F::Node, VariableF<F>>,

    /// For debugging only: the set of variables that have been passed to
    /// `declare` so far. This is used to detect unnecessary calls to
    /// `declare`.
    debug_declared_variables: IndexSet<VariableF<F>>,

    /// SSA node representing the implicit pseudo-variable `super`. Although
    /// `super` and `this` represent the same object, flow analysis considers
    /// them distinct so that if the class being compiled both inherits *and*
    /// overrides a field `_f`, type promotions for `this._f` and `super._f`
    /// will be tracked separately.
    super_ssa_node: SsaNode<F>,

    /// Stack of SSA nodes representing the implicit variable `this` (the
    /// last one is current).
    this_ssa_nodes: Vec<SsaNode<F>>,

    /// Stack of information about the targets of any cascade expressions
    /// that are currently being visited.
    cascade_target_stack: Vec<ExpressionInfo<F>>,

    /// The context for the immediately enclosing block-bodied anonymous
    /// method, if there is one. Otherwise `None`.
    anonymous_block_context: Option<ContextId>,

    /// Stack of the nodes whose `AssignedVariablesNodeInfo` is that of any
    /// local function, function expression, or late variable initializer
    /// expression that encloses the point in flow control that's currently
    /// being analyzed. (Dart stores the info objects.)
    enclosing_function_expression_info_stack: Vec<F::Node>,

    /// `FlowModelHelper.reader`.
    reader: RefCell<FlowLinkReader<PromotionModel<F>>>,
}

impl<F: FlowTypes> FlowAnalysisImpl<F> {
    /// Creates flow analysis for one method, field, or top level variable
    /// (Dart factory `FlowAnalysis(operations, assignedVariables,
    /// typeAnalyzerOptions: ...)`).
    pub fn new(
        operations: F::Ops,
        mut assigned_variables: AssignedVariablesImpl<F::Node, VariableF<F>>,
        type_analyzer_options: TypeAnalyzerOptions,
    ) -> Self {
        if !assigned_variables.is_finished() {
            assigned_variables.finish();
        }
        let super_ssa_node = SsaNode::new();
        FlowAnalysisImpl {
            type_analyzer_options,
            operations,
            contexts: Vec::new(),
            stack: Vec::new(),
            statement_to_context: IndexMap::new(),
            current: FlowModel::new(Reachability::initial()),
            unmatched: None,
            scrutinee_reference: None,
            assigned_variables,
            debug_declared_variables: IndexSet::new(),
            super_ssa_node,
            this_ssa_nodes: vec![SsaNode::new()],
            cascade_target_stack: Vec::new(),
            anonymous_block_context: None,
            enclosing_function_expression_info_stack: Vec::new(),
            reader: RefCell::new(FlowLinkReader::new()),
        }
    }

    /// The assigned variables passed to the constructor.
    pub fn assigned_variables(&self) -> &AssignedVariablesImpl<F::Node, VariableF<F>> {
        &self.assigned_variables
    }

    /// The current flow model (for testing).
    pub fn current(&self) -> &FlowModel<F> {
        &self.current
    }

    /// Gets the SSA node associated with `variable` at the current point in
    /// control flow, or `None` if the variable has been write captured.
    /// **For testing only!** (Dart `FlowAnalysis.ssaNodeForTesting`.)
    pub fn ssa_node_for_testing(&self, variable: VariableF<F>) -> Option<SsaNode<F>> {
        let key = self
            .promotion_key_store()
            .existing_key_for_variable(variable)?;
        promotion_info_get(&self.current.promotion_info, self, key)?
            .ssa_node
            .clone()
    }

    // ------------------------------------------------------------ contexts

    fn push_context(&mut self, context: FlowContext<F>) -> ContextId {
        let id = self.contexts.len();
        self.contexts.push(context);
        self.stack.push(id);
        id
    }

    fn pop_context(&mut self) -> ContextId {
        self.stack
            .pop()
            .expect("flow analysis context stack is empty")
    }

    fn top_context_id(&self) -> ContextId {
        *self
            .stack
            .last()
            .expect("flow analysis context stack is empty")
    }

    fn context(&self, id: ContextId) -> &FlowContext<F> {
        &self.contexts[id]
    }

    fn context_mut(&mut self, id: ContextId) -> &mut FlowContext<F> {
        &mut self.contexts[id]
    }

    /// `(_stack.last as _PatternContext)._matchedValueInfo`.
    fn top_pattern_matched_value_info(&self) -> ExpressionInfo<F> {
        let context = self.context(self.top_context_id());
        context
            .matched_value_info()
            .unwrap_or_else(|| panic!("expected a _PatternContext, got {context:?}"))
            .clone()
    }

    fn bad_context(&self, id: ContextId, expected: &str) -> ! {
        panic!("expected {expected}, got {:?}", self.context(id))
    }

    // ------------------------------------------------- _PropertyTargetHelper

    /// `SsaNode get _thisSsaNode`.
    fn this_ssa_node(&self) -> SsaNode<F> {
        self.this_ssa_nodes.last().unwrap().clone()
    }

    /// `PropertyTarget._getSsaNode`: retrieves the SSA node of the value
    /// accessed by this property target.
    fn get_ssa_node(&self, target: &PropertyTarget<ExpressionInfo<F>>) -> Option<SsaNode<F>> {
        match target {
            PropertyTarget::Expression(info) => {
                get_expression_reference(info.as_ref()).map(|r| r.reference_data().ssa_node.clone())
            }
            PropertyTarget::Cascade => Some(
                self.cascade_target_stack
                    .last()
                    .unwrap()
                    .reference_data()
                    .ssa_node
                    .clone(),
            ),
            PropertyTarget::Super => Some(self.super_ssa_node.clone()),
            PropertyTarget::This => Some(self.this_ssa_node()),
        }
    }

    // ------------------------------------------------------- private helpers

    /// The promotion key of `variable` for a query that takes `&self`.
    fn query_key(&self, variable: VariableF<F>) -> Option<PromotionKey> {
        self.promotion_key_store()
            .existing_key_for_variable(variable)
    }

    fn promotion_model(&self, key: PromotionKey) -> Option<PromotionModel<F>> {
        promotion_info_get(&self.current.promotion_info, self, key)
    }

    fn join(&self, first: Option<&FlowModel<F>>, second: Option<&FlowModel<F>>) -> FlowModel<F> {
        FlowModel::join(self, first, second)
    }

    /// Computes a [`FlowModel`] representing the state of execution after
    /// the statement `try B1 finally B2`.
    ///
    /// `after_try` is the flow model from `B1` after the `try` block (`B1`).
    ///
    /// `before_finally` and `after_finally` are the flow models from before
    /// and after the `finally` block (`B2`), respectively.
    fn attach_finally(
        &self,
        after_try: &FlowModel<F>,
        before_finally: &FlowModel<F>,
        after_finally: &FlowModel<F>,
    ) -> FlowModel<F> {
        // See the `attachFinally` function in
        // https://github.com/dart-lang/language/blob/main/resources/type-system/flow-analysis.md#models.

        // Let `afterTry = FlowModel(r1, VI1)`,
        // `beforeFinally = FlowModel(r2, VI2)`, and
        // `afterFinally = FlowModel(r3, VI3)`.
        let r1 = &after_try.reachable;
        let vi1 = &after_try.promotion_info;
        let vi2 = &before_finally.promotion_info;
        let r3 = &after_finally.reachable;
        let vi3 = &after_finally.promotion_info;

        // Let `r4` be defined as follows:
        // - If `top(r3)` is `true`, then let `r4 = r1`.
        // - Otherwise, let `r4 = unreachable(r1)`.
        debug_assert!(Reachability::opt_ptr_eq(
            r1.parent.as_ref(),
            r3.parent.as_ref()
        ));
        let r4 = if r3.locally_reachable {
            r1.clone()
        } else {
            r1.set_unreachable()
        };

        // Let `VI4` be the map which maps each variable `v` in the domain of
        // either `VI1` or `VI3` as follows (OPTIMIZATION: we implement this
        // by using `afterTry` as a starting point, and iterating through the
        // promotion keys that differ between `VI1` and `VI3`):
        let mut result = after_try.set_reachability(r4);
        let mut field_promotions_to_reapply: Vec<(SsaNode<F>, SsaNode<F>)> = Vec::new();
        let entries = self.reader.borrow_mut().diff(vi1, vi3).entries;
        for entry in entries {
            let promotion_key = entry.key as PromotionKey;
            let v1 = entry.left().map(|l| l.value.clone());
            let v3 = entry.right().map(|l| l.value.clone());

            // - If `v` is in the domain of `VI1` but not `VI3`, then
            //   `VI4(v) = VI1(v)`.
            let Some(v3) = v3 else {
                match v1 {
                    None => {
                        // This should never happen, because we are iterating
                        // through promotion keys that are different between
                        // the `afterTry` and `afterFinally` models.
                        debug_assert!(false);
                    }
                    Some(v1) => {
                        result = result.update_promotion_info(self, promotion_key, v1);
                    }
                }
                continue;
            };

            // - If `v` is in the domain of `VI3` but not `VI1`, then
            //   `VI4(v) = VI3(v)`.
            let Some(v1) = v1 else {
                result = result.update_promotion_info(self, promotion_key, v3);
                continue;
            };

            // - If `v` is in the domain of both `VI1` and `VI3`, then
            //   `VI4(v) = attachFinallyV(VI1(v), VI2(v), VI3(v))`. Note that if
            //   `v` is in the domain of both `VI1` and `VI3`, it must have been
            //   declared before the `try-finally` statement, therefore it must
            //   also be in the domain of `VI2`.
            //   (UNSPECIFIED: however, field promotion breaks this, because
            //   there could be a field that's accessed, and promoted, in both
            //   the `try` and `finally` blocks, but not accessed before the
            //   `try-finally` statement, and in that case its promotion key
            //   would appear in `VI1` and `VI3` but not `VI2`.)
            let v2 = promotion_info_get(vi2, self, promotion_key);

            let new_model =
                self.attach_finally_v(&v1, v2.as_ref(), &v3, &mut field_promotions_to_reapply);
            result = result.update_promotion_info(self, promotion_key, new_model);
        }

        // (UNSPECIFIED: if any variable was written in the try block but not
        // the finally block, then it has a different SSA node now than it had
        // in the finally block. Hence, if any fields of that variable were
        // promoted in the finally block, those field promotions need to be
        // reapplied to the new SSA node for the variable.)
        for (from, to) in field_promotions_to_reapply {
            result = to.apply_property_promotions(
                self,
                &to,
                &from,
                &before_finally.promotion_info,
                &after_finally.promotion_info,
                result,
            );
        }
        result
    }

    fn attach_finally_v(
        &self,
        after_try: &PromotionModel<F>,
        before_finally: Option<&PromotionModel<F>>,
        after_finally: &PromotionModel<F>,
        field_promotions_to_reapply: &mut Vec<(SsaNode<F>, SsaNode<F>)>,
    ) -> PromotionModel<F> {
        // See the `attachFinally` function in
        // https://github.com/dart-lang/language/blob/main/resources/type-system/flow-analysis.md#models.

        // Let `afterTry = VariableModel(d1, p1, t1, a1, u1, c1)`.
        // (UNSPECIFIED: and we denote the SSA node of the variable in
        // `afterTry` as `v1`, since the plan is to rename "SSA node" to
        // "version").
        let p1 = &after_try.promoted_types;
        let a1 = after_try.assigned;
        let v1 = &after_try.ssa_node;
        // Let `afterFinally = VariableModel(d3, p3, t3, a3, u3, c3)`.
        let p3 = &after_finally.promoted_types;
        let t3 = &after_finally.tested;
        let a3 = after_finally.assigned;
        let u3 = after_finally.unassigned;
        let v3 = &after_finally.ssa_node;

        // Let `d4 = d3`.
        // (OPTIMIZATION: flow analysis doesn't store the declared types of
        // variables, so we don't need to do anything here.)

        // Let `p4` be determined as follows:
        let p4: TypeList<F>;
        // (UNSPECIFIED: and also let `v4`, the SSA node after the
        // `try-finally` statement, be determined as follows.)
        let v4: Option<SsaNode<F>>;
        // - If the variable's value might have been changed by the `finally`
        //   block, then `p4 = p3`.
        // (UNSPECIFIED: a necessary and sufficient check for whether the
        // variable might have been changed by the `finally` block is to see
        // if (a) the variable was write captured at some point before the
        // conclusion of the `finally` block (this is represented using a
        // `null` SSA node), or (b) the variable's SSA node after the
        // `finally` block is different from its SSA node before the
        // `finally` block.)
        let variable_was_write_captured = v3.is_none();
        let variable_might_have_changed = variable_was_write_captured
            || before_finally.is_some_and(|b| !opt_ptr_eq(b.ssa_node.as_ref(), v3.as_ref()));
        if variable_might_have_changed {
            p4 = p3.clone();
            // (UNSPECIFIED: and the SSA node after the `try-finally`
            // statement is the SSA node after the `finally` block.)
            v4 = v3.clone();
        } else {
            // UNSPECIFIED: the variable must not have been write captured, so
            // its SSA node can't be `null`.
            let v1 = v1.clone().unwrap();
            // - Otherwise, `p4 = rebasePromotedTypes(p1, p3)`.
            p4 = if self.type_analyzer_options.sound_flow_analysis_enabled {
                PromotionModel::rebase_promoted_types(p1, p3, self)
            } else {
                // (UNSPECIFIED: reproduce old buggy behavior prior to the fix
                // for https://github.com/dart-lang/language/issues/4382.)
                PromotionModel::rebase_promoted_types(p3, p1, self)
            };
            // (UNSPECIFIED: and the SSA node after the `try-finally`
            // statement is the SSA node after the `try` block.)
            if !opt_ptr_eq(Some(&v1), v3.as_ref()) {
                // (UNSPECIFIED: if the `try` block wrote to the variable, any
                // field promotions that were applied in the `finally` block
                // should be reapplied to the new SSA node for the variable.)
                field_promotions_to_reapply.push((v3.clone().unwrap(), v1.clone()));
            }
            v4 = Some(v1);
        }
        // Let `t4 = t3`.
        let t4 = t3.clone();
        // Let `a4 = a1 || a3`.
        let a4 = a1 || a3;
        // Let `u4 = u3`.
        let u4 = u3;
        // Let `c4 = c3`.
        // (OPTIMIZATION: write-captured variables are represented using a
        // `null` SSA node. So this is handled implicitly.)
        PromotionModel::identical_or_new(after_try, after_finally, p4, t4, a4, u4, v4)
    }

    /// Analyzes an equality check between the operands described by
    /// `lhs_info` and `rhs_info`, having static types `lhs_type` and
    /// `rhs_type`.
    fn equality_check(
        &self,
        lhs_info: Option<&ExpressionInfo<F>>,
        lhs_type: TypeViewF<F>,
        rhs_info: Option<&ExpressionInfo<F>>,
        rhs_type: TypeViewF<F>,
    ) -> EqualityCheckResult<F> {
        let left_operand_type_classification = self.operations.classify_type(lhs_type);
        let right_operand_type_classification = self.operations.classify_type(rhs_type);
        if left_operand_type_classification == TypeClassification::NullOrEquivalent
            && right_operand_type_classification == TypeClassification::NullOrEquivalent
        {
            EqualityCheckResult::GuaranteedEqual
        } else if (left_operand_type_classification == TypeClassification::NullOrEquivalent
            && right_operand_type_classification == TypeClassification::NonNullable)
            || (right_operand_type_classification == TypeClassification::NullOrEquivalent
                && left_operand_type_classification == TypeClassification::NonNullable)
        {
            // In strong mode the test is guaranteed to produce a "not equal"
            // result, but weak mode it might produce an "equal" result. If
            // sound flow analysis is enabled, we assume that the user isn't
            // running in weak mode and so we propagate the known "not equal"
            // result. Otherwise, we conservatively assume that either result
            // is possible.
            if self.type_analyzer_options.sound_flow_analysis_enabled {
                EqualityCheckResult::GuaranteedNotEqual
            } else {
                EqualityCheckResult::NoEqualityInformation
            }
        } else if lhs_info.is_some_and(|i| i.is_null()) {
            EqualityCheckResult::IsNullCheck {
                reference: get_expression_reference(rhs_info).cloned(),
                is_reference_on_right: true,
            }
        } else if rhs_info.is_some_and(|i| i.is_null()) {
            EqualityCheckResult::IsNullCheck {
                reference: get_expression_reference(lhs_info).cloned(),
                is_reference_on_right: false,
            }
        } else {
            EqualityCheckResult::NoEqualityInformation
        }
    }

    fn function_expression_begin_impl(&mut self, node: F::Node) {
        let info = self.assigned_variables.get_info_for_node(node);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        self.enclosing_function_expression_info_stack.push(node);
        self.current = self.current.conservative_join(self, [], written, None);
        let previous = self.current.clone();
        let previous_anonymous_block_context = self.anonymous_block_context;
        self.push_context(FlowContext::FunctionExpression {
            previous,
            previous_anonymous_block_context,
        });
        self.anonymous_block_context = None;
        let anywhere = self.assigned_variables.anywhere();
        let written: Vec<PromotionKey> = anywhere.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = anywhere.captured.iter().copied().collect();
        self.current = self
            .current
            .conservative_join(self, written, captured, None);
    }

    fn function_expression_end_impl(&mut self) {
        let id = self.pop_context();
        let FlowContext::FunctionExpression {
            previous,
            previous_anonymous_block_context,
        } = self.context(id)
        else {
            self.bad_context(id, "_FunctionExpressionContext")
        };
        let (previous, previous_anonymous_block_context) =
            (previous.clone(), *previous_anonymous_block_context);
        self.enclosing_function_expression_info_stack.pop();
        self.current = previous;
        self.anonymous_block_context = previous_anonymous_block_context;
    }

    /// Gets the matched value type that should be used to type check the
    /// pattern currently being analyzed.
    ///
    /// May only be called in the context of a pattern.
    fn get_matched_value_type_impl(&self) -> TypeViewF<F> {
        let matched_value_info = self.top_pattern_matched_value_info();
        self.promotion_model(matched_value_info.reference_data().promotion_key)
            .and_then(|m| m.promoted_types.last().copied())
            .unwrap_or(matched_value_info.type_)
    }

    /// Computes the non-promotion reasons for `reference`.
    ///
    /// Dart returns a closure that computes the map lazily, calling
    /// `operations.isSubtypeOf` and `operations.variableType`; here the map
    /// is computed eagerly (the operations are pure), so the returned
    /// closure does not borrow the flow analysis.
    fn get_non_promotion_reasons(
        &self,
        reference: &ExpressionInfo<F>,
        current_promotion_info: Option<&PromotionModel<F>>,
    ) -> WhyNotPromoted<TypeOfF<F>, NonPromotionReasonF<F>>
    where
        TypeOfF<F>: 'static,
        NonPromotionReasonF<F>: 'static,
    {
        let r = reference.reference_data();
        if let ReferenceKind::Property {
            property_name,
            property_member,
        } = &r.reference_kind
        {
            if let Some(property_member) = property_member {
                let why_not_promotable = if self.operations.is_private_name(*property_name) {
                    self.operations
                        .why_property_is_not_promotable(property_member)
                } else {
                    Some(PropertyNonPromotabilityReason::IsNotPrivate)
                };
                let mut ssa_node = r
                    .ssa_node
                    .property
                    .as_ref()
                    .expect("not a _PropertySsaNode")
                    .previous_ssa_node
                    .clone();
                let mut all_previously_promoted_types: Option<Vec<TypeList<F>>> = None;
                while let Some(node) = ssa_node {
                    let previous_promotion_info =
                        self.current
                            .info_for(self, node.property_promotion_key(), &node);
                    let promoted_types = previous_promotion_info.promoted_types.clone();
                    if !promoted_types.is_empty() {
                        all_previously_promoted_types
                            .get_or_insert_with(Vec::new)
                            .push(promoted_types);
                    }
                    ssa_node = node.property.as_ref().unwrap().previous_ssa_node.clone();
                }
                if let Some(all_previously_promoted_types) = all_previously_promoted_types {
                    let mut result: IndexMap<TypeViewF<F>, NonPromotionReasonF<F>> =
                        IndexMap::new();
                    for previously_promoted_types in all_previously_promoted_types {
                        for ty in previously_promoted_types.iter() {
                            let reason = match why_not_promotable {
                                None => {
                                    NonPromotionReason::PropertyNotPromotedForNonInherentReason {
                                        property_name: *property_name,
                                        property_member: Some(property_member.clone()),
                                        field_promotion_enabled: self
                                            .type_analyzer_options
                                            .field_promotion_enabled,
                                    }
                                }
                                Some(why_not_promotable) => {
                                    NonPromotionReason::PropertyNotPromotedForInherentReason {
                                        property_name: *property_name,
                                        property_member: Some(property_member.clone()),
                                        why_not_promotable,
                                        field_promotion_enabled: self
                                            .type_analyzer_options
                                            .field_promotion_enabled,
                                    }
                                }
                            };
                            result.insert(*ty, reason);
                        }
                    }
                    let result: Vec<_> = result.into_iter().collect();
                    return Box::new(move || result);
                }
            }
        } else if let Some(current_promotion_info) = current_promotion_info {
            let variable = self.promotion_key_store().variable_for_key(r.promotion_key);
            match variable {
                None => {
                    if !self.type_analyzer_options.this_promotion_enabled {
                        let promoted_types = current_promotion_info.promoted_types.clone();
                        if !promoted_types.is_empty() {
                            let mut result: IndexMap<TypeViewF<F>, NonPromotionReasonF<F>> =
                                IndexMap::new();
                            for ty in promoted_types.iter() {
                                result.insert(*ty, NonPromotionReason::ThisNotPromoted);
                            }
                            let result: Vec<_> = result.into_iter().collect();
                            return Box::new(move || result);
                        }
                    }
                }
                Some(variable) => {
                    let mut result: IndexMap<TypeViewF<F>, NonPromotionReasonF<F>> =
                        IndexMap::new();
                    let current_type = current_promotion_info
                        .promoted_types
                        .last()
                        .copied()
                        .unwrap_or_else(|| self.operations.variable_type(variable));
                    let mut non_promotion_history =
                        current_promotion_info.non_promotion_history.clone();
                    while let Some(history) = non_promotion_history {
                        let non_promoted_type = history.type_;
                        if !self
                            .operations
                            .is_subtype_of(current_type, non_promoted_type)
                        {
                            result
                                .entry(non_promoted_type)
                                .or_insert_with(|| history.non_promotion_reason.clone());
                        }
                        non_promotion_history = history.previous.clone();
                    }
                    let result: Vec<_> = result.into_iter().collect();
                    return Box::new(move || result);
                }
            }
        }
        Box::new(Vec::new)
    }

    /// Common code for handling patterns that perform an equality check.
    /// `operand_info` is the expression info for the expression that the
    /// matched value is being compared to, and `operand_type` is its type.
    ///
    /// If `not_equal` is `true`, the pattern matches if the matched value is
    /// *not* equal to the operand; otherwise, it matches if the matched value
    /// is *equal* to the operand.
    fn handle_equality_check_pattern(
        &mut self,
        operand_info: Option<&ExpressionInfo<F>>,
        operand_type: TypeViewF<F>,
        not_equal: bool,
        matched_value_type: TypeViewF<F>,
    ) {
        debug_assert!(matched_value_type == self.get_matched_value_type_impl());
        let matched_value_info = self.top_pattern_matched_value_info();
        // Create a `_Reference` to represent the matched value; this will be
        // the LHS of the equality comparison. Note that it's not necessary to
        // use `restoreConditionVariableState` because `_equalityCheck` uses
        // the `_Reference` solely to decide if the matched value needs to be
        // promoted to non-null; it doesn't attempt to read any stored
        // condition variable state from it.
        let lhs_reference =
            create_reference(&matched_value_info, matched_value_type, &self.current);
        match self.equality_check(
            Some(&lhs_reference),
            matched_value_type,
            operand_info,
            operand_type,
        ) {
            EqualityCheckResult::NoEqualityInformation => {
                // We have no information so we have to assume the pattern
                // might or might not match.
                self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
            }
            EqualityCheckResult::IsNullCheck {
                is_reference_on_right,
                ..
            } => {
                let if_not_null = if !is_reference_on_right {
                    // The `null` literal is on the right hand side of the
                    // implicit equality check, meaning it is the constant
                    // value.  So the user is doing something like this:
                    //
                    //     if (v case == null) { ... }
                    //
                    // So we want to promote the type of `v` in the case where
                    // the constant pattern *didn't* match.
                    //
                    // `_nullCheckPattern` returns `null` in the case where the
                    // matched value type is non-nullable.  In fully sound
                    // programs, this would mean that the pattern cannot
                    // possibly match.  However, in mixed mode programs it
                    // might match due to unsoundness.  Since we don't want type
                    // inference results to change when a program becomes
                    // fully sound, we have to assume that we're in mixed mode,
                    // and thus the pattern might match.
                    self.null_check_pattern(matched_value_type)
                        .unwrap_or_else(|| self.current.clone())
                } else {
                    // The `null` literal is on the left hand side of the
                    // implicit equality check, meaning it is the scrutinee.
                    // So the user is doing something silly like this:
                    //
                    //     if (null case == c) { ... }
                    //
                    // (where `c` is some constant).  There's no variable to
                    // promote.
                    //
                    // Since flow analysis can't make use of the results of
                    // constant evaluation, we can't really assume anything; as
                    // far as we know, the pattern might or might not match.
                    self.current.clone()
                };
                if not_equal {
                    self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
                    self.current = if_not_null;
                } else {
                    self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&if_not_null)));
                }
            }
            EqualityCheckResult::GuaranteedEqual => {
                if not_equal {
                    // Both operands are known by flow analysis to compare
                    // equal, so the pattern is guaranteed *not* to match.
                    self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
                    self.current = self.current.set_unreachable();
                } else {
                    // Both operands are known by flow analysis to compare
                    // equal, so the pattern is guaranteed to match.  Since our
                    // approach to handling patterns in flow analysis uses
                    // "implicit and" semantics (initially assuming that the
                    // pattern always matches, and then updating the `_current`
                    // and `_unmatched` states to reflect what values the
                    // pattern rejects), we don't have to do any updates.
                }
            }
            EqualityCheckResult::GuaranteedNotEqual => {
                if not_equal {
                    // Both operands are known by flow analysis to compare
                    // unequal, so the pattern is guaranteed to match. No
                    // updates are needed.
                } else {
                    // Both operands are known by flow analysis to compare
                    // unequal, so the pattern is guaranteed *not* to match.
                    self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
                    self.current = self.current.set_unreachable();
                }
            }
        }
    }

    fn handle_property(
        &self,
        target_ssa_node: &SsaNode<F>,
        property_name: NameF<F>,
        property_member: Option<&PropertyMemberF<F>>,
        unpromoted_type: TypeViewF<F>,
    ) -> (Option<TypeViewF<F>>, SsaNode<F>) {
        // Find the SSA node for the target of the property access, and figure
        // out whether the property in question is promotable.
        let is_promotable = property_member.is_some_and(|m| {
            self.type_analyzer_options.field_promotion_enabled
                && self.operations.is_property_promotable(m)
        });
        let property_ssa_node = target_ssa_node.get_or_create_property_node(
            property_name,
            self.promotion_key_store(),
            is_promotable,
        );
        let mut promoted_type = None;
        if is_promotable {
            let promotion_info = self.promotion_model(property_ssa_node.property_promotion_key());
            if let Some(promotion_info) = &promotion_info {
                debug_assert!(opt_ptr_eq(
                    promotion_info.ssa_node.as_ref(),
                    Some(&property_ssa_node)
                ));
            }
            promoted_type = promotion_info.and_then(|i| i.promoted_types.last().copied());
            if let Some(t) = promoted_type
                && !self.operations.is_subtype_of(t, unpromoted_type)
            {
                promoted_type = None;
            }
        }
        (promoted_type, property_ssa_node)
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_impl(
        &mut self,
        promotion_key: PromotionKey,
        matched_type: TypeViewF<F>,
        mut expression_info: Option<ExpressionInfo<F>>,
        is_final: bool,
        is_late: bool,
        is_implicitly_typed: bool,
        unpromoted_type: TypeViewF<F>,
        inherit_promotable_properties: bool,
    ) {
        if is_late {
            // Don't use expression info for late variables, since we don't
            // know when they'll be initialized.
            expression_info = None;
        } else if is_implicitly_typed
            && !self
                .type_analyzer_options
                .respect_implicitly_typed_var_initializers
        {
            // If the language version is too old, SSA analysis has to ignore
            // initializer expressions for implicitly typed variables, in
            // order to preserve the buggy behavior of
            // https://github.com/dart-lang/language/issues/1785.
            expression_info = None;
        }
        let new_ssa_node = match &expression_info {
            Some(info) if inherit_promotable_properties && info.as_reference().is_some() => {
                info.reference_data().ssa_node.clone()
            }
            _ => SsaNode::with_condition_variable_state(
                expression_info.filter(|i| i.is_non_trivial()),
            ),
        };
        self.current = self.current.write(
            self,
            None,
            promotion_key,
            matched_type,
            new_ssa_node,
            !is_implicitly_typed && !is_final,
            unpromoted_type,
        );
        if is_implicitly_typed && self.operations.is_type_parameter_type(matched_type) {
            let reference = self.variable_reference(promotion_key, unpromoted_type);
            self.current = self
                .current
                .try_promote_for_type_check(self, &reference, matched_type)
                .if_true
                .clone();
        }
    }

    /// Determines whether an expression having the given `static_type` is
    /// guaranteed to fail an `is` or `as` check using `checked_type` due to
    /// sound null safety.
    ///
    /// If sound flow analysis is disabled, this method will return `false`
    /// regardless of its input. This reflects the fact that in language
    /// versions prior to the introduction of sound flow analysis, flow
    /// analysis assumed that the program might be executing in unsound null
    /// safety mode.
    fn is_type_check_guaranteed_to_fail_with_sound_null_safety(
        &self,
        static_type: TypeViewF<F>,
        checked_type: TypeViewF<F>,
    ) -> bool {
        if !self.type_analyzer_options.sound_flow_analysis_enabled {
            return false;
        }
        match self.operations.classify_type(static_type) {
            TypeClassification::NonNullable
                if self.operations.classify_type(checked_type)
                    == TypeClassification::NullOrEquivalent =>
            {
                // Guaranteed to fail due to nullability mismatch.
                true
            }
            TypeClassification::NullOrEquivalent
                if self.operations.classify_type(checked_type)
                    == TypeClassification::NonNullable =>
            {
                true
            }
            _ => false,
        }
    }

    /// Whether an expression having the given `static_type` is guaranteed to
    /// succeed an `is` or `as` check using `checked_type` due to sound null
    /// safety.
    fn is_type_check_guaranteed_to_succeed_with_sound_null_safety(
        &self,
        static_type: TypeViewF<F>,
        checked_type: TypeViewF<F>,
    ) -> bool {
        self.type_analyzer_options.sound_flow_analysis_enabled
            && self.operations.is_subtype_of(static_type, checked_type)
    }

    /// Creates a promotion key representing a temporary variable that
    /// doesn't correspond to any variable in the user's source code. This is
    /// used by flow analysis to model the synthetic variables used during
    /// pattern matching to cache the values that the pattern, and its
    /// subpatterns, are being matched against.
    fn make_temporary_reference(
        &mut self,
        ssa_node: SsaNode<F>,
        ty: TypeViewF<F>,
    ) -> ExpressionInfo<F> {
        let promotion_key = self.promotion_key_store().make_temporary_key();
        self.current = self.current.update_promotion_info(
            self,
            promotion_key,
            PromotionModel::new(
                empty_type_list::<F>(),
                empty_type_list::<F>(),
                true,
                false,
                Some(ssa_node.clone()),
                None,
            ),
        );
        ExpressionInfo::trivial_variable_reference(
            ty,
            self.current.clone(),
            promotion_key,
            false,
            ssa_node,
        )
    }

    /// Creates a fresh [`ExpressionInfo`] recording the current flow
    /// analysis state.
    fn make_trivial_expression_info(&self, ty: TypeViewF<F>) -> ExpressionInfo<F> {
        ExpressionInfo::trivial(ty, self.current.clone())
    }

    fn null_aware_access_right_begin_impl(
        &mut self,
        target_info: Option<&ExpressionInfo<F>>,
        target_type: TypeViewF<F>,
        guard_variable: Option<VariableF<F>>,
    ) -> Option<ExpressionInfo<F>> {
        self.current = self.current.split();
        let mut shortcut_control_path = self.current.clone();
        let target_reference = get_expression_reference(target_info).cloned();
        if let Some(target_reference) = &target_reference {
            self.current = self
                .current
                .try_mark_non_nullable(self, target_reference)
                .if_true
                .clone();
        }
        match self.operations.classify_type(target_type) {
            TypeClassification::NullOrEquivalent => {
                // The control flow path containing the null-aware code is
                // unreachable.
                self.current = self.current.set_unreachable();
            }
            TypeClassification::NonNullable => {
                // The control flow path that skips the null-aware code is
                // unreachable, assuming sound null safety.
                if self.type_analyzer_options.sound_flow_analysis_enabled {
                    shortcut_control_path = shortcut_control_path.set_unreachable();
                }
            }
            TypeClassification::PotentiallyNullable => {
                // Both control flow paths are reachable.
            }
        }
        self.push_context(FlowContext::NullAwareAccess {
            previous: shortcut_control_path,
        });
        let mut target_ssa_node = None;
        let mut null_aware_expression_info = target_reference.clone();
        if self.type_analyzer_options.sound_flow_analysis_enabled {
            // Pick up the target SSA node so that it can be used for field
            // promotion.
            target_ssa_node = target_reference
                .as_ref()
                .map(|r| r.reference_data().ssa_node.clone());
        } else {
            // Field promotion was broken for null-aware field accesses prior
            // to the implementation of sound flow analysis. So to replicate
            // the bug, destroy the target reference so that it can't be used
            // for field promotion.
            null_aware_expression_info = None;
        }
        if let Some(guard_variable) = guard_variable {
            // Promote the guard variable as well.
            let promotion_key = self.promotion_key_store().key_for_variable(guard_variable);
            let non_null_type = self.operations.promote_to_non_null(target_type);
            self.current = self.current.update_promotion_info(
                self,
                promotion_key,
                PromotionModel::new(
                    if non_null_type == target_type {
                        empty_type_list::<F>()
                    } else {
                        std::rc::Rc::from(vec![non_null_type])
                    },
                    empty_type_list::<F>(),
                    true,
                    false,
                    Some(target_ssa_node.unwrap_or_default()),
                    None,
                ),
            );
        }
        null_aware_expression_info
    }

    /// Computes an updated flow model representing the result of a null
    /// check performed by a pattern. The returned flow model represents what
    /// is known about the program state if the matched value is determined
    /// to be not equal to `null`.
    ///
    /// If the matched value's type is non-nullable, then `None` is returned.
    fn null_check_pattern(&self, matched_value_type: TypeViewF<F>) -> Option<FlowModel<F>> {
        let matched_value_info = self.top_pattern_matched_value_info();
        debug_assert!(matched_value_type == self.get_matched_value_type_impl());
        let matched_value_reference =
            create_reference(&matched_value_info, matched_value_type, &self.current);
        // Promote
        let type_classification = self.operations.classify_type(matched_value_type);
        if type_classification == TypeClassification::NonNullable {
            None
        } else {
            let mut if_not_null = self
                .current
                .try_mark_non_nullable(self, &matched_value_reference)
                .if_true
                .clone();
            // If the scrutinee is a variable reference, and the variable
            // hasn't changed since the start of the matching operation,
            // promote it too.
            //
            // If the scrutinee is a property reference, promote it too. (This
            // is safe even if the underlying variable whose property is being
            // referenced has changed, because the next time the property is
            // accessed, it will be accessed through a new SSA node, and thus
            // a new promotion key).
            if let Some(scrutinee_reference) = &self.scrutinee_reference
                && (scrutinee_reference.is_property_reference()
                    || self
                        .same_ssa_node_as_scrutinee(&matched_value_reference, scrutinee_reference))
            {
                if_not_null = if_not_null
                    .try_mark_non_nullable(self, scrutinee_reference)
                    .if_true
                    .clone();
            }
            if type_classification == TypeClassification::NullOrEquivalent {
                if_not_null = if_not_null.set_unreachable();
            }
            Some(if_not_null)
        }
    }

    /// `_current.promotionInfo?.get(this, matchedValueReference.promotionKey)!.ssaNode ==
    /// _current.promotionInfo?.get(this, scrutineeReference.promotionKey)?.ssaNode`.
    fn same_ssa_node_as_scrutinee(
        &self,
        matched_value_reference: &ExpressionInfo<F>,
        scrutinee_reference: &ExpressionInfo<F>,
    ) -> bool {
        if self.current.promotion_info.is_none() {
            // Both sides are `null`.
            return true;
        }
        let matched = self
            .promotion_model(matched_value_reference.reference_data().promotion_key)
            .unwrap();
        let scrutinee = self.promotion_model(scrutinee_reference.reference_data().promotion_key);
        opt_ptr_eq(
            matched.ssa_node.as_ref(),
            scrutinee.as_ref().and_then(|s| s.ssa_node.as_ref()),
        )
    }

    fn pop_pattern(&mut self, guard_info: Option<ExpressionInfo<F>>) -> FlowModel<F> {
        let id = self.pop_context();
        let FlowContext::TopPattern {
            previous_unmatched, ..
        } = self.context(id)
        else {
            self.bad_context(id, "_TopPatternContext")
        };
        let previous_unmatched = previous_unmatched.clone();
        let mut unmatched = self.unmatched.take().unwrap();
        self.unmatched = previous_unmatched;
        let guard_info = guard_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        self.current = guard_info.if_true.clone();
        unmatched = self.join(Some(&unmatched), Some(&guard_info.if_false));
        self.current = self.current.unsplit();
        unmatched.unsplit()
    }

    fn pop_scrutinee(&mut self) {
        let id = self.pop_context();
        let FlowContext::Scrutinee {
            previous_scrutinee_reference,
        } = self.context(id)
        else {
            self.bad_context(id, "_ScrutineeContext")
        };
        self.scrutinee_reference = previous_scrutinee_reference.clone();
    }

    /// Updates the stack to reflect the fact that flow analysis is entering
    /// into a pattern or subpattern match. `matched_value_info` should be the
    /// reference representing the value being matched.
    fn push_pattern(&mut self, matched_value_info: ExpressionInfo<F>) {
        self.current = self.current.split();
        let previous_unmatched = self.unmatched.clone();
        self.push_context(FlowContext::TopPattern {
            matched_value_info,
            previous_unmatched,
        });
        self.unmatched = Some(self.current.set_unreachable());
    }

    /// Updates the stack to reflect the fact that flow analysis is entering
    /// into a construct that performs pattern matching. `scrutinee_info`
    /// should be the expression info for the expression that is being matched
    /// (or `None` if there is no expression that's being matched directly, as
    /// happens when in `for-in` loops). `scrutinee_type` should be the static
    /// type of the scrutinee.
    ///
    /// `allow_scrutinee_promotion` indicates whether pattern matches should
    /// cause the scrutinee to be promoted.
    ///
    /// The returned value is the reference representing the value being
    /// matched. It should be passed to `push_pattern`.
    fn push_scrutinee(
        &mut self,
        scrutinee_info: Option<ExpressionInfo<F>>,
        scrutinee_type: TypeViewF<F>,
        allow_scrutinee_promotion: bool,
    ) -> ExpressionInfo<F> {
        let previous_scrutinee_reference = self.scrutinee_reference.clone();
        self.push_context(FlowContext::Scrutinee {
            previous_scrutinee_reference,
        });
        let scrutinee_reference = get_expression_reference(scrutinee_info.as_ref()).cloned();
        self.scrutinee_reference = scrutinee_reference.clone();
        let mut scrutinee_ssa_node = None;
        if allow_scrutinee_promotion && let Some(scrutinee_reference) = &scrutinee_reference {
            scrutinee_ssa_node = Some(scrutinee_reference.reference_data().ssa_node.clone());
        }
        let reference =
            self.make_temporary_reference(scrutinee_ssa_node.unwrap_or_default(), scrutinee_type);
        reference.restore_condition_variable_state(scrutinee_info.as_ref(), self, &self.current)
    }

    fn this_or_super_reference(
        &self,
        static_type: TypeViewF<F>,
        is_super: bool,
    ) -> ExpressionInfo<F> {
        let ssa_node = if is_super {
            self.super_ssa_node.clone()
        } else {
            self.this_ssa_node()
        };
        ExpressionInfo::trivial_variable_reference(
            static_type,
            self.current.clone(),
            self.promotion_key_store().this_promotion_key(),
            true,
            ssa_node.clone(),
        )
        .restore_condition_variable_state(
            ssa_node.condition_variable_state.as_ref(),
            self,
            &self.current,
        )
    }

    fn variable_reference(
        &self,
        variable_key: PromotionKey,
        unpromoted_type: TypeViewF<F>,
    ) -> ExpressionInfo<F> {
        let info = self.promotion_model(variable_key).unwrap();
        ExpressionInfo::trivial_variable_reference(
            info.promoted_types
                .last()
                .copied()
                .unwrap_or(unpromoted_type),
            self.current.clone(),
            variable_key,
            false,
            info.ssa_node.clone().unwrap_or_default(),
        )
    }

    /// Common logic for handling writes to variables, whether they occur as
    /// part of an ordinary assignment or a pattern assignment.
    ///
    /// If `is_postfix_inc_dec` is `true`, the `node` is a postfix expression
    /// and we won't store information about `variable`.
    fn write_impl(
        &mut self,
        node: F::Node,
        variable: VariableF<F>,
        written_type: TypeViewF<F>,
        expression_info: Option<ExpressionInfo<F>>,
        is_postfix_inc_dec: bool,
    ) -> Option<ExpressionInfo<F>> {
        let unpromoted_type = self.operations.variable_type(variable);
        let variable_key = self.promotion_key_store().key_for_variable(variable);
        let new_ssa_node =
            SsaNode::with_condition_variable_state(expression_info.filter(|i| i.is_non_trivial()));
        let reason = NonPromotionReason::DemoteViaExplicitWrite { variable, node };
        self.current = self.current.write(
            self,
            Some(&reason),
            variable_key,
            written_type,
            new_ssa_node,
            true,
            unpromoted_type,
        );

        // Update the type of the variable for looking up the write
        // expression.
        let mut reference = None;
        if self.type_analyzer_options.inference_update4_enabled
            && F::is_expression(node)
            && !is_postfix_inc_dec
        {
            reference = Some(self.variable_reference(variable_key, unpromoted_type));
        }
        reference
    }
}

/// `_PatternContext.createReference`: creates a reference to the matched
/// value having type `matched_type`.
fn create_reference<F: FlowTypes>(
    matched_value_info: &ExpressionInfo<F>,
    matched_type: TypeViewF<F>,
    current: &FlowModel<F>,
) -> ExpressionInfo<F> {
    ExpressionInfo::trivial_variable_reference(
        matched_type,
        current.clone(),
        matched_value_info.reference_data().promotion_key,
        false,
        SsaNode::new(),
    )
}

impl<F: FlowTypes> FlowModelHelper<F> for FlowAnalysisImpl<F> {
    fn reader(&self) -> &RefCell<FlowLinkReader<PromotionModel<F>>> {
        &self.reader
    }

    fn bool_type(&self) -> TypeViewF<F> {
        self.operations.bool_type()
    }

    fn promotion_key_store(&self) -> &PromotionKeyStore<VariableF<F>> {
        &self.assigned_variables.promotion_key_store
    }

    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions {
        &self.type_analyzer_options
    }

    fn type_operations(&self) -> &F::Ops {
        &self.operations
    }

    fn is_final(&self, variable_key: PromotionKey) -> bool {
        if !self.type_analyzer_options.inference_update4_enabled {
            return false;
        }
        let variable = self.promotion_key_store().variable_for_key(variable_key);
        if let Some(variable) = variable
            && self.operations.is_final(variable)
        {
            return true;
        }
        false
    }

    fn is_valid_promotion_step(&self, previous_type: TypeViewF<F>, new_type: TypeViewF<F>) -> bool {
        // Caller must ensure that `newType <: previousType`.
        debug_assert!(
            self.operations.is_subtype_of(new_type, previous_type),
            "Expected {new_type:?} to be a subtype of {previous_type:?}."
        );
        if self.type_analyzer_options.sound_flow_analysis_enabled {
            // Promotion to a mutual subtype is not allowed. Since the caller
            // has already ensured that `newType <: previousType`, it's only
            // necessary to check whether `previousType <: newType`.
            !self.operations.is_subtype_of(previous_type, new_type)
        } else {
            // Repeated promotion to the same type is not allowed.
            new_type != previous_type
        }
    }
}

impl<F: FlowTypes> FlowAnalysisNullShortingInterface for FlowAnalysisImpl<F> {
    type Expression = F::Expression;
    type Variable = VariableF<F>;
    type Type = TypeOfF<F>;
    type ExpressionInfo = ExpressionInfo<F>;

    fn null_aware_access_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::NullAwareAccess { previous } = self.context(id) else {
            self.bad_context(id, "_NullAwareAccessContext")
        };
        let previous = previous.clone();
        self.current = self.join(Some(&self.current), Some(&previous)).unsplit();
    }

    fn null_aware_access_right_begin(
        &mut self,
        target_info: Option<ExpressionInfo<F>>,
        target_type: TypeViewF<F>,
        guard_variable: Option<VariableF<F>>,
    ) -> Option<ExpressionInfo<F>> {
        self.null_aware_access_right_begin_impl(target_info.as_ref(), target_type, guard_variable)
    }
}

impl<F: FlowTypes> FlowAnalysis for FlowAnalysisImpl<F>
where
    TypeOfF<F>: 'static,
    VariableF<F>: 'static,
    PropertyMemberF<F>: 'static,
    NameF<F>: 'static,
{
    type Node = F::Node;
    type Statement = F::Statement;
    type Operations = F::Ops;

    fn is_reachable(&self) -> bool {
        self.current.reachable.overall_reachable
    }

    fn operations(&self) -> &F::Ops {
        &self.operations
    }

    fn promoted_type_of_this(&self) -> Option<TypeViewF<F>> {
        if !self.type_analyzer_options.this_promotion_enabled {
            return None;
        }
        self.promotion_model(self.promotion_key_store().this_promotion_key())
            .and_then(|m| m.promoted_types.last().copied())
    }

    fn anonymous_block_body_begin(&mut self) {
        self.current = self.current.split();
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let previous_anonymous_block_context = self.anonymous_block_context;
        let id = self.push_context(FlowContext::AnonymousBlock {
            return_model: None,
            checkpoint,
            previous_anonymous_block_context,
        });
        self.anonymous_block_context = Some(id);
    }

    fn anonymous_block_body_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::AnonymousBlock {
            return_model,
            previous_anonymous_block_context,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_AnonymousBlockContext")
        };
        let (return_model, previous) = (return_model.clone(), *previous_anonymous_block_context);
        self.current = self
            .join(Some(&self.current), return_model.as_ref())
            .unsplit();
        self.anonymous_block_context = previous;
    }

    fn as_expression_end(
        &mut self,
        sub_expression_info: Option<ExpressionInfo<F>>,
        sub_expression_type: TypeViewF<F>,
        cast_type: TypeViewF<F>,
    ) {
        // Depending on types, flow analysis may be able to prove that the
        // `as` expression is guaranteed to fail.
        if self
            .is_type_check_guaranteed_to_fail_with_sound_null_safety(sub_expression_type, cast_type)
        {
            self.current = self.current.set_unreachable();
        }

        let Some(reference) = get_expression_reference(sub_expression_info.as_ref()) else {
            return;
        };
        self.current = self
            .current
            .try_promote_for_type_cast(self, reference, cast_type);
    }

    fn assert_after_condition(&mut self, condition_info: Option<ExpressionInfo<F>>) {
        let id = self.top_context_id();
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        let FlowContext::Assert { condition_true, .. } = self.context_mut(id) else {
            self.bad_context(id, "_AssertContext")
        };
        *condition_true = Some(condition_info.if_true.clone());
        self.current = condition_info.if_false.clone();
    }

    fn assert_begin(&mut self) {
        self.current = self.current.split();
        let previous = self.current.clone();
        self.push_context(FlowContext::Assert {
            previous,
            condition_true: None,
        });
    }

    fn assert_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::Assert {
            previous,
            condition_true,
        } = self.context(id)
        else {
            self.bad_context(id, "_AssertContext")
        };
        let (previous, condition_true) = (previous.clone(), condition_true.clone().unwrap());
        self.current = self.join(Some(&previous), Some(&condition_true)).unsplit();
    }

    fn assigned_variable_pattern(
        &mut self,
        node: F::Node,
        variable: VariableF<F>,
        written_type: TypeViewF<F>,
    ) {
        let matched_value_info = self.top_pattern_matched_value_info();
        self.write_impl(
            node,
            variable,
            written_type,
            Some(matched_value_info),
            false,
        );
    }

    fn assign_matched_pattern_variable(
        &mut self,
        variable: VariableF<F>,
        promotion_key: PromotionKey,
    ) {
        let merged_key = self.promotion_key_store().key_for_variable(variable);
        let mut info = self
            .promotion_model(promotion_key)
            .unwrap_or_else(|| PromotionModel::fresh(false, Some(SsaNode::new())));
        // Normally flow analysis is responsible for tracking whether
        // variables are definitely assigned; however for variables appearing
        // in patterns we have other logic to make sure that a value is
        // definitely assigned (e.g. the rule that a variable appearing on one
        // side of an `||` must also appear on the other side).  So to avoid
        // reporting redundant errors, we pretend that the variable is
        // definitely assigned, even if it isn't.
        info = info.set_assigned();
        self.current = self.current.update_promotion_info(self, merged_key, info);
    }

    fn boolean_literal(&mut self, value: bool) -> ExpressionInfo<F> {
        let unreachable = self.current.set_unreachable();
        if value {
            ExpressionInfo::new(
                self.operations.bool_type(),
                self.current.clone(),
                unreachable,
            )
        } else {
            ExpressionInfo::new(
                self.operations.bool_type(),
                unreachable,
                self.current.clone(),
            )
        }
    }

    fn cascade_expression_after_target(
        &mut self,
        target_info: Option<ExpressionInfo<F>>,
        target_type: TypeViewF<F>,
        is_null_aware: bool,
        guard_variable: Option<VariableF<F>>,
    ) -> TypeViewF<F> {
        // If the cascade is null-aware, then during the cascade sections, the
        // effective type of the target is promoted to non-null.
        let promoted_target_type = if is_null_aware {
            self.operations.promote_to_non_null(target_type)
        } else {
            target_type
        };
        // Retrieve the SSA node for the cascade target, if one has been
        // created already, so that field accesses within cascade sections
        // will receive the benefit of previous field promotions. If an SSA
        // node for the target hasn't been created yet (e.g. because it's not
        // a read of a local variable), create a fresh SSA node for it, so
        // that field promotions that occur during cascade sections will
        // persist in later cascade sections.
        let expression_reference = get_expression_reference(target_info.as_ref()).cloned();
        let ssa_node = expression_reference
            .as_ref()
            .map(|r| r.reference_data().ssa_node.clone())
            .unwrap_or_default();
        // Create a temporary reference to represent the implicit temporary
        // variable that holds the cascade target. It is important that this
        // is different from `expressionReference`, because if the target is a
        // local variable, and that variable is written during one of the
        // cascade sections, future cascade sections should still be
        // understood to act on the value the variable had before the write.
        // (e.g. in `x.._field!.f(x = g()).._field.h()`, no `!` is needed on
        // the second access to `_field`, even though `x` has been written
        // to).
        let reference = self.make_temporary_reference(ssa_node, promoted_target_type);
        self.cascade_target_stack.push(reference);
        if is_null_aware {
            self.null_aware_access_right_begin_impl(
                expression_reference.as_ref(),
                target_type,
                guard_variable,
            );
        }
        promoted_target_type
    }

    fn cascade_expression_end(&mut self) -> ExpressionInfo<F> {
        // TODO(paulberry): if the cascade expression is null-aware, do the
        // equivalent of `nullAwareAccess_end`, so that the caller doesn't
        // have to have a separate call to `nullAwareAccess_end`.

        // Pop the reference for the temporary variable that holds the target
        // of the cascade stack. It becomes the reference for the whole
        // expression. This ensures that field accesses performed on the whole
        // cascade expression (e.g. `(x..f())._field` will still receive the
        // benefit of field promotion.
        self.cascade_target_stack.pop().unwrap()
    }

    fn conditional_condition_begin(&mut self) {
        self.current = self.current.split();
    }

    fn conditional_else_begin(
        &mut self,
        then_expression_info: Option<ExpressionInfo<F>>,
        then_type: TypeViewF<F>,
    ) {
        let id = self.top_context_id();
        let then_info =
            then_expression_info.unwrap_or_else(|| self.make_trivial_expression_info(then_type));
        let current = self.current.clone();
        let FlowContext::Conditional {
            branch_model,
            then_info: ctx_then_info,
            then_model,
        } = self.context_mut(id)
        else {
            self.bad_context(id, "_ConditionalContext")
        };
        *ctx_then_info = Some(then_info);
        *then_model = Some(current);
        let branch_model = branch_model.clone();
        self.current = branch_model;
    }

    fn conditional_end(
        &mut self,
        conditional_expression_type: TypeViewF<F>,
        else_expression_info: Option<ExpressionInfo<F>>,
        else_type: TypeViewF<F>,
    ) -> ExpressionInfo<F> {
        let id = self.pop_context();
        let FlowContext::Conditional {
            then_info,
            then_model,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_ConditionalContext")
        };
        let then_info = then_info.clone().unwrap();
        let then_model = then_model.clone().unwrap();
        let else_expression_info =
            else_expression_info.unwrap_or_else(|| self.make_trivial_expression_info(else_type));
        let else_model = self.current.clone();
        self.current = self.join(Some(&then_model), Some(&else_model)).unsplit();
        ExpressionInfo::new(
            conditional_expression_type,
            self.join(
                Some(&then_info.if_true),
                Some(&else_expression_info.if_true),
            )
            .unsplit(),
            self.join(
                Some(&then_info.if_false),
                Some(&else_expression_info.if_false),
            )
            .unsplit(),
        )
    }

    fn conditional_then_begin(
        &mut self,
        condition_info: Option<ExpressionInfo<F>>,
        _conditional_expression: F::Node,
    ) {
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        self.push_context(FlowContext::Conditional {
            branch_model: condition_info.if_false.clone(),
            then_info: None,
            then_model: None,
        });
        self.current = condition_info.if_true.clone();
    }

    fn constant_pattern_end(
        &mut self,
        expression_info: Option<ExpressionInfo<F>>,
        ty: TypeViewF<F>,
        patterns_enabled: bool,
        matched_value_type: TypeViewF<F>,
    ) {
        debug_assert!(self.context(self.top_context_id()).is_pattern_context());
        if patterns_enabled {
            self.handle_equality_check_pattern(
                expression_info.as_ref(),
                ty,
                false,
                matched_value_type,
            );
        } else {
            // Before pattern support was added to Dart, flow analysis didn't
            // do any promotion based on the constants in individual case
            // clauses.  Also, it assumed that all case clauses were equally
            // reachable.  So, when analyzing legacy code that targets a
            // language version before patterns were supported, we need to
            // mimic that old behavior.  The easiest way to do that is to
            // simply assume that the pattern might or might not match,
            // regardless of the constant expression.
            self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
        }
    }

    fn copy_promotion_data(&mut self, source_key: PromotionKey, destination_key: PromotionKey) {
        let model = self
            .promotion_model(source_key)
            .unwrap_or_else(|| PromotionModel::fresh(false, Some(SsaNode::new())));
        self.current = self
            .current
            .update_promotion_info(self, destination_key, model);
    }

    fn declare(&mut self, variable: VariableF<F>, static_type: TypeViewF<F>, initialized: bool) {
        debug_assert!(static_type == self.operations.variable_type(variable));
        debug_assert!(
            self.debug_declared_variables.insert(variable),
            "Variable {variable:?} already declared"
        );
        let key = self.promotion_key_store().key_for_variable(variable);
        self.current = self.current.declare(self, key, initialized);
    }

    fn declared_variable_pattern(
        &mut self,
        matched_type: TypeViewF<F>,
        static_type: TypeViewF<F>,
        is_final: bool,
        is_late: bool,
        is_implicitly_typed: bool,
    ) -> PromotionKey {
        let matched_value_info = self.top_pattern_matched_value_info();
        // Choose a fresh promotion key to represent the temporary variable
        // that stores the matched value, and mark it as initialized.
        let promotion_key = self.promotion_key_store().make_temporary_key();
        self.current = self.current.declare(self, promotion_key, true);
        self.initialize_impl(
            promotion_key,
            matched_type,
            Some(matched_value_info),
            is_final,
            is_late,
            is_implicitly_typed,
            static_type,
            false,
        );
        promotion_key
    }

    fn do_statement_body_begin(&mut self, do_statement: F::Statement) {
        let info = self
            .assigned_variables
            .get_info_for_node(F::statement_to_node(do_statement));
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        let checkpoint = self.current.reachable.clone();
        let id = self.push_context(FlowContext::BranchTarget(BranchTargetData::new(checkpoint)));
        self.current = self
            .current
            .split()
            .conservative_join(self, written, captured, None);
        self.statement_to_context.insert(do_statement, id);
    }

    fn do_statement_condition_begin(&mut self) {
        let id = self.top_context_id();
        let FlowContext::BranchTarget(target) = self.context(id) else {
            self.bad_context(id, "_BranchTargetContext")
        };
        let continue_model = target.continue_model.clone();
        self.current = self.join(Some(&self.current), continue_model.as_ref());
    }

    fn do_statement_end(&mut self, condition_info: Option<ExpressionInfo<F>>) {
        let id = self.pop_context();
        let FlowContext::BranchTarget(target) = self.context(id) else {
            self.bad_context(id, "_BranchTargetContext")
        };
        let break_model = target.break_model.clone();
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        self.current = self
            .join(Some(&condition_info.if_false), break_model.as_ref())
            .unsplit();
    }

    fn equality_operation_end(
        &mut self,
        left_operand_info: Option<ExpressionInfo<F>>,
        left_operand_type: TypeViewF<F>,
        right_operand_info: Option<ExpressionInfo<F>>,
        right_operand_type: TypeViewF<F>,
        not_equal: bool,
    ) -> Option<ExpressionInfo<F>> {
        // Note: leftOperandInfo and rightOperandInfo are nullable in the base
        // class to account for the fact that legacy type promotion doesn't
        // record information about legacy operands.  But since we are
        // currently in full (post null safety) flow analysis logic, we can
        // safely assume that they are not null.
        match self.equality_check(
            left_operand_info.as_ref(),
            left_operand_type,
            right_operand_info.as_ref(),
            right_operand_type,
        ) {
            EqualityCheckResult::GuaranteedEqual => {
                // Both operands are known by flow analysis to compare equal,
                // so the whole expression behaves equivalently to a boolean
                // (either `true` or `false` depending whether the check uses
                // the `!=` operator).
                Some(self.boolean_literal(!not_equal))
            }
            EqualityCheckResult::GuaranteedNotEqual => {
                // Both operands are known by flow analysis to compare
                // unequal, so the whole expression behaves equivalently to a
                // boolean.
                Some(self.boolean_literal(not_equal))
            }
            EqualityCheckResult::IsNullCheck { reference, .. } => {
                // One side of the equality check is `null`, but the other
                // side is not a promotable reference.  So there's no
                // promotion to do.
                let reference = reference?;
                // The equality check is a null check of something
                // potentially promotable (e.g. a local variable).  Record the
                // necessary information so that if this null check winds up
                // being used for a conditional branch, the variable's will be
                // promoted on the appropriate code path.
                let equality_info = self.current.try_mark_non_nullable(self, &reference);
                Some(if not_equal {
                    equality_info
                } else {
                    equality_info.invert()
                })
            }
            EqualityCheckResult::NoEqualityInformation => {
                // Since flow analysis can't garner any information from this
                // equality check, nothing needs to be done; by not returning
                // any expression info, we ensure that if this expression
                // winds up being used for a conditional branch, flow analysis
                // will consider both code paths reachable and won't perform
                // any promotions on either path.
                None
            }
        }
    }

    fn equality_relational_pattern_end(
        &mut self,
        operand_info: Option<ExpressionInfo<F>>,
        operand_type: TypeViewF<F>,
        not_equal: bool,
        matched_value_type: TypeViewF<F>,
    ) {
        self.handle_equality_check_pattern(
            operand_info.as_ref(),
            operand_type,
            not_equal,
            matched_value_type,
        );
    }

    fn finish(&mut self) {
        debug_assert!(self.stack.is_empty());
        debug_assert!(self.current.reachable.parent.is_none());
        debug_assert!(self.unmatched.is_none());
        debug_assert!(self.scrutinee_reference.is_none());
        debug_assert!(self.enclosing_function_expression_info_stack.is_empty());
    }

    fn for_body_begin(
        &mut self,
        node: Option<F::Statement>,
        condition_info: Option<ExpressionInfo<F>>,
    ) {
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let id = self.push_context(FlowContext::While {
            target: BranchTargetData::new(checkpoint),
            condition_false: condition_info.if_false.clone(),
        });
        if let Some(node) = node {
            self.statement_to_context.insert(node, id);
        }
        self.current = condition_info.if_true.clone();
    }

    fn for_condition_begin(&mut self, node: F::Node) {
        let info = self.assigned_variables.get_info_for_node(node);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        self.current = self
            .current
            .split()
            .conservative_join(self, written, captured, None);
    }

    fn for_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::While {
            target,
            condition_false,
        } = self.context(id)
        else {
            self.bad_context(id, "_WhileContext")
        };
        // Tail of the stack: falseCondition, break
        let break_state = target.break_model.clone();
        let false_condition = condition_false.clone();

        self.current = self
            .join(Some(&false_condition), break_state.as_ref())
            .inherit_tested(self, &self.current)
            .unsplit();
    }

    fn for_updater_begin(&mut self) {
        let id = self.top_context_id();
        let FlowContext::While { target, .. } = self.context(id) else {
            self.bad_context(id, "_WhileContext")
        };
        let continue_model = target.continue_model.clone();
        self.current = self.join(Some(&self.current), continue_model.as_ref());
    }

    fn for_each_body_begin(&mut self, node: F::Node) {
        let info = self.assigned_variables.get_info_for_node(node);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        self.current = self
            .current
            .split()
            .conservative_join(self, written, captured, None);
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let previous = self.current.clone();
        self.push_context(FlowContext::SimpleStatement {
            target: BranchTargetData::new(checkpoint),
            previous,
        });
    }

    fn for_each_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::SimpleStatement { previous, .. } = self.context(id) else {
            self.bad_context(id, "_SimpleStatementContext")
        };
        let previous = previous.clone();
        self.current = self.join(Some(&self.current), Some(&previous)).unsplit();
    }

    fn function_expression_begin(&mut self, node: F::Node) {
        self.function_expression_begin_impl(node);
    }

    fn function_expression_end(&mut self) {
        self.function_expression_end_impl();
    }

    fn get_matched_value_type(&self) -> TypeViewF<F> {
        self.get_matched_value_type_impl()
    }

    fn handle_break(&mut self, target: Option<F::Statement>) {
        let context = target.and_then(|t| self.statement_to_context.get(&t).copied());
        if let Some(id) = context {
            let current = self.current.clone();
            let checkpoint = match self.contexts[id].branch_target_mut() {
                Some(t) => t.checkpoint.clone(),
                None => self.bad_context(id, "_BranchTargetContext"),
            };
            let unsplit = current.unsplit_to(&checkpoint);
            let break_model = self.contexts[id]
                .branch_target_mut()
                .unwrap()
                .break_model
                .clone();
            let joined = self.join(break_model.as_ref(), Some(&unsplit));
            self.contexts[id].branch_target_mut().unwrap().break_model = Some(joined);
        }
        self.current = self.current.set_unreachable();
    }

    fn handle_continue(&mut self, target: Option<F::Statement>) {
        let context = target.and_then(|t| self.statement_to_context.get(&t).copied());
        if let Some(id) = context {
            let current = self.current.clone();
            let checkpoint = match self.contexts[id].branch_target_mut() {
                Some(t) => t.checkpoint.clone(),
                None => self.bad_context(id, "_BranchTargetContext"),
            };
            let unsplit = current.unsplit_to(&checkpoint);
            let continue_model = self.contexts[id]
                .branch_target_mut()
                .unwrap()
                .continue_model
                .clone();
            let joined = self.join(continue_model.as_ref(), Some(&unsplit));
            self.contexts[id]
                .branch_target_mut()
                .unwrap()
                .continue_model = Some(joined);
        }
        self.current = self.current.set_unreachable();
    }

    fn handle_exit(&mut self) {
        self.current = self.current.set_unreachable();
    }

    fn handle_return(&mut self) {
        if let Some(id) = self.anonymous_block_context {
            // There is a control flow path from the current point to the
            // exit of the anonymous method.
            let FlowContext::AnonymousBlock {
                return_model,
                checkpoint,
                ..
            } = self.context(id)
            else {
                self.bad_context(id, "_AnonymousBlockContext")
            };
            let unsplit = self.current.unsplit_to(checkpoint);
            let joined = self.join(return_model.as_ref(), Some(&unsplit));
            let FlowContext::AnonymousBlock { return_model, .. } = self.context_mut(id) else {
                unreachable!()
            };
            *return_model = Some(joined);
        }
        self.current = self.current.set_unreachable();
    }

    fn if_case_statement_after_expression(
        &mut self,
        scrutinee_info: Option<ExpressionInfo<F>>,
        scrutinee_type: TypeViewF<F>,
    ) {
        // If S0 is the statement `if (E0 case P when E1) S1 else S2`, then:
        // - before(P) = after(E0),
        // - before(E1) = matched(P).
        // Note that we don't need to take any action to handle
        // `before(E1) = matched(P)`, because we store both the "matched"
        // state for patterns and the "before" state for expressions in
        // `_current`.
        let reference = self.push_scrutinee(scrutinee_info, scrutinee_type, true);
        self.push_pattern(reference);
    }

    fn if_case_statement_begin(&mut self) {
        // If S0 is the statement `if (E0 case P when E1) S1 else S2`, then:
        // - before(E0) = split(before(S0)).
        self.current = self.current.split();
    }

    fn if_case_statement_then_begin(&mut self, guard_info: Option<ExpressionInfo<F>>) {
        // If S0 is the statement `if (E0 case P when E1) S1 else S2`, then:
        // - before(S1) = true(E1).
        let branch_model = self.pop_pattern(guard_info);
        self.pop_scrutinee();
        self.push_context(FlowContext::If {
            branch_model,
            after_then: None,
        });
    }

    fn if_null_expression_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::IfNullExpression { shortcut_state } = self.context(id) else {
            self.bad_context(id, "_IfNullExpressionContext")
        };
        let shortcut_state = shortcut_state.clone();
        self.current = self
            .join(Some(&self.current), Some(&shortcut_state))
            .unsplit();
    }

    fn if_null_expression_right_begin(
        &mut self,
        left_hand_side_info: Option<ExpressionInfo<F>>,
        left_hand_side_type: TypeViewF<F>,
    ) {
        let lhs_reference = get_expression_reference(left_hand_side_info.as_ref());
        self.current = self.current.split();
        let mut shortcut_state = match lhs_reference {
            Some(r) => self.current.try_mark_non_nullable(self, r).if_true.clone(),
            None => self.current.clone(),
        };
        match self.operations.classify_type(left_hand_side_type) {
            TypeClassification::NullOrEquivalent => {
                // The control path that skips the "if null" code is
                // unreachable.
                shortcut_state = shortcut_state.set_unreachable();
            }
            TypeClassification::NonNullable => {
                // The control path containing the "if null" code is
                // unreachable, assuming sound null safety.
                if self.type_analyzer_options.sound_flow_analysis_enabled {
                    self.current = self.current.set_unreachable();
                }
            }
            TypeClassification::PotentiallyNullable => {
                // Both control flow paths are reachable.
            }
        }
        self.push_context(FlowContext::IfNullExpression { shortcut_state });
    }

    fn if_statement_condition_begin(&mut self) {
        self.current = self.current.split();
    }

    fn if_statement_else_begin(&mut self) {
        let id = self.top_context_id();
        let current = self.current.clone();
        let FlowContext::If {
            branch_model,
            after_then,
        } = self.context_mut(id)
        else {
            self.bad_context(id, "_IfContext")
        };
        *after_then = Some(current);
        let branch_model = branch_model.clone();
        self.current = branch_model;
    }

    fn if_statement_end(&mut self, has_else: bool) {
        let id = self.pop_context();
        let FlowContext::If {
            branch_model,
            after_then,
        } = self.context(id)
        else {
            self.bad_context(id, "_IfContext")
        };
        let (after_then, after_else) = if has_else {
            (after_then.clone().unwrap(), self.current.clone())
        } else {
            // no `else`, so `then` is still current
            (self.current.clone(), branch_model.clone())
        };
        self.current = self.join(Some(&after_then), Some(&after_else)).unsplit();
    }

    fn if_statement_then_begin(
        &mut self,
        condition_info: Option<ExpressionInfo<F>>,
        _if_node: F::Node,
    ) {
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        self.push_context(FlowContext::If {
            branch_model: condition_info.if_false.clone(),
            after_then: None,
        });
        self.current = condition_info.if_true.clone();
    }

    fn initialize(
        &mut self,
        variable: VariableF<F>,
        matched_type: TypeViewF<F>,
        initializer_expression_info: Option<ExpressionInfo<F>>,
        is_final: bool,
        is_late: bool,
        is_implicitly_typed: bool,
        inherit_promotable_properties: bool,
    ) {
        let unpromoted_type = self.operations.variable_type(variable);
        let variable_key = self.promotion_key_store().key_for_variable(variable);
        self.initialize_impl(
            variable_key,
            matched_type,
            initializer_expression_info,
            is_final,
            is_late,
            is_implicitly_typed,
            unpromoted_type,
            inherit_promotable_properties,
        );
    }

    fn is_assigned(&self, variable: VariableF<F>) -> bool {
        self.query_key(variable)
            .and_then(|k| self.promotion_model(k))
            .is_some_and(|m| m.assigned)
    }

    fn is_expression_end(
        &mut self,
        sub_expression_info: Option<ExpressionInfo<F>>,
        is_not: bool,
        sub_expression_type: TypeViewF<F>,
        checked_type: TypeViewF<F>,
    ) -> Option<ExpressionInfo<F>> {
        if self.operations.is_bottom_type(checked_type)
            || self.is_type_check_guaranteed_to_fail_with_sound_null_safety(
                sub_expression_type,
                checked_type,
            )
        {
            Some(self.boolean_literal(is_not))
        } else if let Some(sub_expression_reference) =
            get_expression_reference(sub_expression_info.as_ref())
        {
            let expression_info = self.current.try_promote_for_type_check(
                self,
                sub_expression_reference,
                checked_type,
            );
            Some(if is_not {
                expression_info.invert()
            } else {
                expression_info
            })
        } else if self.is_type_check_guaranteed_to_succeed_with_sound_null_safety(
            sub_expression_type,
            checked_type,
        ) {
            Some(self.boolean_literal(!is_not))
        } else {
            None
        }
    }

    fn is_unassigned(&self, variable: VariableF<F>) -> bool {
        self.query_key(variable)
            .and_then(|k| self.promotion_model(k))
            .is_none_or(|m| m.unassigned)
    }

    fn labeled_statement_begin(&mut self, node: F::Statement) {
        self.current = self.current.split();
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let id = self.push_context(FlowContext::BranchTarget(BranchTargetData::new(checkpoint)));
        self.statement_to_context.insert(node, id);
    }

    fn labeled_statement_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::BranchTarget(target) = self.context(id) else {
            self.bad_context(id, "_BranchTargetContext")
        };
        let break_model = target.break_model.clone();
        self.current = self
            .join(Some(&self.current), break_model.as_ref())
            .unsplit();
    }

    fn late_initializer_begin(&mut self, node: F::Node) {
        // Late initializers are treated the same as function expressions.
        // Essentially we act as though `late x = expr;` is syntactic sugar
        // for `late x = LAZY_MAGIC(() => expr);` (where `LAZY_MAGIC` creates
        // a lazy evaluation thunk that gets replaced by the result of `expr`
        // once it is evaluated).
        self.function_expression_begin_impl(node);
    }

    fn late_initializer_end(&mut self) {
        // Late initializers are treated the same as function expressions.
        self.function_expression_end_impl();
    }

    fn logical_binary_op_begin(&mut self) {
        self.current = self.current.split();
    }

    fn logical_binary_op_end(
        &mut self,
        right_operand_info: Option<ExpressionInfo<F>>,
        is_and: bool,
    ) -> ExpressionInfo<F> {
        let id = self.pop_context();
        let FlowContext::Branch { branch_model } = self.context(id) else {
            self.bad_context(id, "_BranchContext")
        };
        let branch_model = branch_model.clone();
        let right_operand_info = right_operand_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));

        let (true_result, false_result) = if is_and {
            (
                right_operand_info.if_true.clone(),
                self.join(Some(&branch_model), Some(&right_operand_info.if_false)),
            )
        } else {
            (
                self.join(Some(&branch_model), Some(&right_operand_info.if_true)),
                right_operand_info.if_false.clone(),
            )
        };
        self.current = self.join(Some(&true_result), Some(&false_result)).unsplit();
        ExpressionInfo::new(
            self.operations.bool_type(),
            true_result.unsplit(),
            false_result.unsplit(),
        )
    }

    fn logical_binary_op_right_begin(
        &mut self,
        left_operand_info: Option<ExpressionInfo<F>>,
        _whole_expression: F::Node,
        is_and: bool,
    ) {
        let condition_info = left_operand_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        self.push_context(FlowContext::Branch {
            branch_model: if is_and {
                condition_info.if_false.clone()
            } else {
                condition_info.if_true.clone()
            },
        });
        self.current = if is_and {
            condition_info.if_true.clone()
        } else {
            condition_info.if_false.clone()
        };
    }

    fn logical_not_end(
        &mut self,
        operand_info: Option<ExpressionInfo<F>>,
    ) -> Option<ExpressionInfo<F>> {
        operand_info.map(|i| i.invert())
    }

    fn logical_or_pattern_after_lhs(&mut self) {
        let id = self.top_context_id();
        let current = self.current.clone();
        let FlowContext::OrPattern {
            previous_unmatched,
            lhs_matched,
            ..
        } = self.context_mut(id)
        else {
            self.bad_context(id, "_OrPatternContext")
        };
        // The current flow state represents the state if the left hand side
        // matched.  Save this so that we can later join it with the state if
        // the right hand side matched.
        *lhs_matched = Some(current);
        let previous_unmatched = previous_unmatched.clone();
        // An attempt to match the right hand side will only be made if the
        // left hand side failed to match, so set the current flow state to
        // the "unmatched" flow state from the left hand side.
        self.current = self.unmatched.clone().unwrap();
        // And reset `_unmatched` to the value it had prior to visiting the
        // left hand side, so that if the right hand side fails to match, the
        // failure will be accumulated into it.
        self.unmatched = Some(previous_unmatched);
    }

    fn logical_or_pattern_begin(&mut self) {
        let matched_value_info = self.top_pattern_matched_value_info();
        // Save the pieces of the current flow state that will be needed
        // later.
        let previous_unmatched = self.unmatched.clone().unwrap();
        self.push_context(FlowContext::OrPattern {
            matched_value_info,
            previous_unmatched,
            lhs_matched: None,
        });
        // Initialize `_unmatched` to a fresh unreachable flow state, so that
        // after we visit the left hand side, `_unmatched` will represent the
        // flow state if the left hand side failed to match.
        self.unmatched = Some(self.current.set_unreachable());
    }

    fn logical_or_pattern_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::OrPattern { lhs_matched, .. } = self.context(id) else {
            self.bad_context(id, "_OrPatternContext")
        };
        let lhs_matched = lhs_matched.clone();
        // If either the left hand side or the right hand side matched, the
        // logical-or pattern is considered to have matched.
        self.current = self.join(lhs_matched.as_ref(), Some(&self.current));
    }

    fn non_equality_relational_pattern_end(&mut self) {
        // Flow analysis has no way of knowing whether the operator will
        // return `true` or `false`, so just assume the worst case--both cases
        // are reachable and no promotions can be done in either case.
        self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
    }

    fn non_null_assert_end(&mut self, operand_info: Option<ExpressionInfo<F>>) {
        if let Some(operand_reference) = get_expression_reference(operand_info.as_ref()) {
            self.current = self
                .current
                .try_mark_non_nullable(self, operand_reference)
                .if_true
                .clone();
        }
    }

    fn null_aware_map_entry_end(&mut self, is_key_null_aware: bool) {
        if !is_key_null_aware {
            return;
        }
        let id = self.pop_context();
        let FlowContext::NullAwareMapEntry { shortcut_state } = self.context(id) else {
            self.bad_context(id, "_NullAwareMapEntryContext")
        };
        let shortcut_state = shortcut_state.clone();
        self.current = self
            .join(Some(&self.current), Some(&shortcut_state))
            .unsplit();
    }

    fn null_aware_map_entry_value_begin(
        &mut self,
        key_info: Option<ExpressionInfo<F>>,
        key_type: TypeViewF<F>,
        is_key_null_aware: bool,
    ) {
        if !is_key_null_aware {
            return;
        }
        let key_reference = get_expression_reference(key_info.as_ref()).cloned();
        self.current = self.current.split();
        let mut shortcut_state = match &key_reference {
            Some(key_reference) => {
                let expression_info = self.current.try_mark_non_nullable(self, key_reference);
                self.current = expression_info.if_true.clone();
                expression_info.if_false.clone()
            }
            None => self.current.clone(),
        };
        match self.operations.classify_type(key_type) {
            TypeClassification::NonNullable => {
                // The control flow path that skips the value expression is
                // unreachable.
                shortcut_state = shortcut_state.set_unreachable();
            }
            TypeClassification::NullOrEquivalent => {
                // The control flow path containing the value expression is
                // unreachable. This functionality was added as part of the
                // `sound-flow-analysis` language feature, even though it would
                // have been a sound reasoning step before then.
                if self.type_analyzer_options.sound_flow_analysis_enabled {
                    self.current = self.current.set_unreachable();
                }
            }
            TypeClassification::PotentiallyNullable => {
                // Both control flow paths are reachable.
            }
        }
        self.push_context(FlowContext::NullAwareMapEntry { shortcut_state });
    }

    fn null_check_or_assert_pattern_begin(
        &mut self,
        is_assert: bool,
        matched_value_type: TypeViewF<F>,
    ) -> bool {
        if !is_assert {
            if self.type_analyzer_options.sound_flow_analysis_enabled
                && self.operations.classify_type(matched_value_type)
                    == TypeClassification::NonNullable
            {
                // The pattern is guaranteed to match.
            } else {
                // The pattern might not match, either because
                // matchedValueType is nullable, or because sound flow
                // analysis is disabled (in which case we presume the user
                // might be running under an older version of Dart that
                // supported weak null safety mode).
                self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
            }
        }
        let if_not_null = self.null_check_pattern(matched_value_type);
        let result = if_not_null.is_none();
        if let Some(if_not_null) = if_not_null {
            self.current = if_not_null;
        }
        // Note: we don't need to push a new pattern context for the
        // subpattern, because (a) the subpattern matches the same value as
        // the outer pattern, and (b) promotion of the synthetic cache variable
        // takes care of establishing the correct matched value type.
        result
    }

    fn null_check_or_assert_pattern_end(&mut self) {}

    fn null_literal(&mut self, ty: TypeViewF<F>) -> ExpressionInfo<F> {
        ExpressionInfo::null_info(ty, self.current.clone())
    }

    fn parenthesized_expression(
        &mut self,
        expression_info: Option<ExpressionInfo<F>>,
    ) -> Option<ExpressionInfo<F>> {
        expression_info
    }

    fn pattern_assignment_after_rhs(
        &mut self,
        rhs_info: Option<ExpressionInfo<F>>,
        rhs_type: TypeViewF<F>,
    ) {
        let reference = self.push_scrutinee(rhs_info, rhs_type, false);
        self.push_pattern(reference);
    }

    fn pattern_assignment_end(&mut self) {
        self.pop_pattern(None);
        self.pop_scrutinee();
    }

    fn pattern_for_in_after_expression(&mut self, element_type: TypeViewF<F>) {
        let reference = self.push_scrutinee(None, element_type, false);
        self.push_pattern(reference);
    }

    fn pattern_for_in_end(&mut self) {
        self.pop_pattern(None);
        self.pop_scrutinee();
    }

    fn pattern_variable_declaration_after_initializer(
        &mut self,
        initializer_info: Option<ExpressionInfo<F>>,
        initializer_type: TypeViewF<F>,
    ) {
        let reference = self.push_scrutinee(initializer_info, initializer_type, false);
        self.push_pattern(reference);
    }

    fn pattern_variable_declaration_end(&mut self) {
        self.pop_pattern(None);
        self.pop_scrutinee();
    }

    fn pop_property_subpattern(&mut self) {
        let id = self.pop_context();
        let FlowContext::PropertyPattern {
            previous_scrutinee, ..
        } = self.context(id)
        else {
            self.bad_context(id, "_PropertyPatternContext")
        };
        self.scrutinee_reference = previous_scrutinee.clone();
    }

    fn pop_subpattern(&mut self) {
        let id = self.pop_context();
        debug_assert!(self.context(id).is_pattern_context());
    }

    fn post_inc_dec(&mut self, node: F::Node, variable: VariableF<F>, written_type: TypeViewF<F>) {
        self.write_impl(node, variable, written_type, None, true);
    }

    fn promoted_property_type(
        &mut self,
        target: PropertyTarget<ExpressionInfo<F>>,
        property_name: NameF<F>,
        property_member: Option<PropertyMemberF<F>>,
        unpromoted_type: TypeViewF<F>,
    ) -> Option<TypeViewF<F>> {
        let target_ssa_node = self.get_ssa_node(&target)?;
        let (ty, _) = self.handle_property(
            &target_ssa_node,
            property_name,
            property_member.as_ref(),
            unpromoted_type,
        );
        ty
    }

    fn promoted_type(&self, variable: VariableF<F>) -> Option<TypeViewF<F>> {
        self.query_key(variable)
            .and_then(|k| self.promotion_model(k))
            .and_then(|m| m.promoted_types.last().copied())
    }

    fn promote_for_pattern(
        &mut self,
        matched_type: TypeViewF<F>,
        mut known_type: TypeViewF<F>,
        match_fails_if_wrong_type: bool,
        match_may_fail_even_if_correct_type: bool,
    ) -> bool {
        if self
            .operations
            .shared_type_kind(known_type.unwrap_type_view())
            == SharedTypeKind::Invalid
        {
            self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&self.current)));
            return false;
        }

        let mut cannot_match = false;
        match self.operations.classify_type(matched_type) {
            TypeClassification::NonNullable => {
                if self.type_analyzer_options.sound_flow_analysis_enabled
                    && self.operations.classify_type(known_type)
                        == TypeClassification::NullOrEquivalent
                {
                    // `Null()` cannot match a non-nullable matched value,
                    // assuming sound null safety.
                    cannot_match = true;
                }
                // The matched type is non-nullable, so promote to a
                // non-nullable type. This allows for code like `case int? x?`
                // to promote `x` to non-nullable.
                known_type = self.operations.promote_to_non_null(known_type);
            }
            TypeClassification::NullOrEquivalent => {
                if self.type_analyzer_options.sound_flow_analysis_enabled
                    && self.operations.classify_type(known_type) == TypeClassification::NonNullable
                {
                    // If `T` is a non-nullable type, `T()` cannot match a
                    // matched value of type `Null`. This reasoning step is
                    // sound regardless of whether sound null safety, but it is
                    // a new reasoning step that was added to flow analysis as
                    // part of the `sound-flow-analysis` feature.
                    cannot_match = true;
                }
            }
            TypeClassification::PotentiallyNullable => {
                // No conclusions can be drawn about `cannotMatch` or
                // `knownType`.
            }
        }
        let matched_value_info = self.top_pattern_matched_value_info();
        let matched_value_reference =
            create_reference(&matched_value_info, matched_type, &self.current);
        let covers_matched_type = self.operations.is_subtype_of(
            self.operations.extension_type_erasure(matched_type),
            self.operations.extension_type_erasure(known_type),
        );
        // Promote the synthetic cache variable the pattern is being matched
        // against.
        let promotion_info =
            self.current
                .try_promote_for_type_check(self, &matched_value_reference, known_type);
        let mut if_true = promotion_info.if_true.clone();
        let mut if_false = promotion_info.if_false.clone();
        // If the scrutinee is a variable reference, and the variable hasn't
        // changed since the start of the matching operation, promote it too.
        //
        // If the scrutinee is a property reference, promote it too. (This is
        // safe even if the underlying variable whose property is being
        // referenced has changed, because the next time the property is
        // accessed, it will be accessed through a new SSA node, and thus a
        // new promotion key).
        //
        // If the scrutinee is `this`, promote it too.
        if let Some(scrutinee_reference) = self.scrutinee_reference.clone()
            && (scrutinee_reference.is_property_reference()
                || (scrutinee_reference.reference_data().is_this_or_super
                    && self.type_analyzer_options.this_promotion_enabled)
                || self.same_ssa_node_as_scrutinee(&matched_value_reference, &scrutinee_reference))
        {
            if_true = if_true
                .try_promote_for_type_check(self, &scrutinee_reference, known_type)
                .if_true
                .clone();
            if_false = if_false
                .try_promote_for_type_check(self, &scrutinee_reference, known_type)
                .if_false
                .clone();
        }
        self.current = if_true.clone();
        if cannot_match {
            self.current = self.current.set_unreachable();
        }
        if match_fails_if_wrong_type && !covers_matched_type {
            // There's a reachable control flow path where the match might
            // fail due to a type mismatch. Therefore, we must update the
            // `_unmatched` flow state based on the state of flow analysis
            // assuming the type check failed.
            self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&if_false)));
        }
        if match_may_fail_even_if_correct_type {
            // There's a reachable control flow path where the type might
            // match, but the match might nonetheless fail for some other
            // reason. Therefore, we must update the `_unmatched` flow state
            // based on the state of flow analysis assuming the type check
            // succeeded.
            self.unmatched = Some(self.join(self.unmatched.as_ref(), Some(&if_true)));
        }
        covers_matched_type
    }

    fn property_get(
        &mut self,
        target: PropertyTarget<ExpressionInfo<F>>,
        property_name: NameF<F>,
        property_member: Option<PropertyMemberF<F>>,
        unpromoted_type: TypeViewF<F>,
    ) -> (Option<TypeViewF<F>>, Option<ExpressionInfo<F>>) {
        let Some(target_ssa_node) = self.get_ssa_node(&target) else {
            return (None, None);
        };
        let (promoted_type, property_ssa_node) = self.handle_property(
            &target_ssa_node,
            property_name,
            property_member.as_ref(),
            unpromoted_type,
        );
        let property_reference = ExpressionInfo::property_reference(
            promoted_type.unwrap_or(unpromoted_type),
            self.current.clone(),
            property_name,
            property_member,
            property_ssa_node.property_promotion_key(),
            property_ssa_node,
        );
        (promoted_type, Some(property_reference))
    }

    fn property_promotion_chain_for_testing(
        &mut self,
        target: PropertyTarget<ExpressionInfo<F>>,
        property_name: NameF<F>,
        property_member: Option<PropertyMemberF<F>>,
    ) -> Vec<TypeViewF<F>> {
        let Some(target_ssa_node) = self.get_ssa_node(&target) else {
            return Vec::new();
        };
        // Find the SSA node for the target of the property access, and figure
        // out whether the property in question is promotable.
        let is_promotable = property_member.as_ref().is_some_and(|m| {
            self.type_analyzer_options.field_promotion_enabled
                && self.operations.is_property_promotable(m)
        });
        if !is_promotable {
            return Vec::new();
        }
        let property_ssa_node = target_ssa_node.get_or_create_property_node(
            property_name,
            self.promotion_key_store(),
            is_promotable,
        );
        let Some(promotion_info) = self.promotion_model(property_ssa_node.property_promotion_key())
        else {
            return Vec::new();
        };
        debug_assert!(opt_ptr_eq(
            promotion_info.ssa_node.as_ref(),
            Some(&property_ssa_node)
        ));
        promotion_info.promoted_types.to_vec()
    }

    fn push_property_subpattern(
        &mut self,
        property_name: NameF<F>,
        property_member: Option<PropertyMemberF<F>>,
        unpromoted_type: TypeViewF<F>,
    ) -> Option<TypeViewF<F>> {
        let matched_value_info = self.top_pattern_matched_value_info();
        debug_assert!(self.unmatched.is_some());
        let (promoted_type, property_ssa_node) = self.handle_property(
            &matched_value_info.reference_data().ssa_node,
            property_name,
            property_member.as_ref(),
            unpromoted_type,
        );
        let property_reference = ExpressionInfo::property_reference(
            promoted_type.unwrap_or(unpromoted_type),
            self.current.clone(),
            property_name,
            property_member,
            property_ssa_node.property_promotion_key(),
            property_ssa_node.clone(),
        );
        let reference = self
            .make_temporary_reference(property_ssa_node, promoted_type.unwrap_or(unpromoted_type));
        let previous_scrutinee = self.scrutinee_reference.clone();
        self.push_context(FlowContext::PropertyPattern {
            matched_value_info: reference,
            previous_scrutinee,
        });
        self.scrutinee_reference = Some(property_reference);
        promoted_type
    }

    fn push_subpattern(&mut self, matched_type: TypeViewF<F>) {
        debug_assert!(self.context(self.top_context_id()).is_pattern_context());
        debug_assert!(self.unmatched.is_some());
        let reference = self.make_temporary_reference(SsaNode::new(), matched_type);
        self.push_context(FlowContext::Pattern {
            matched_value_info: reference,
        });
    }

    fn suspension(&mut self, node: F::Node) {
        // During an async suspension or yield, other code may execute. If the
        // current point in flow control is inside a local function, this
        // means that enclosing functions may resume executing.
        //
        // Therefore, any variables that are read within the current local
        // function, and written to anywhere, but not declared in the current
        // local function, might potentially get written to, blowing away any
        // promotions that are currently in effect.
        if let Some(info_node) = self
            .enclosing_function_expression_info_stack
            .last()
            .copied()
        {
            let info = self.assigned_variables.get_info_for_node(info_node);
            let anywhere_written = &self.assigned_variables.anywhere().written;
            let variables_to_demote: Vec<PromotionKey> = info
                .read
                .iter()
                .copied()
                .filter(|k| anywhere_written.contains(k) && !info.declared.contains(k))
                .collect();
            let key_store = self.promotion_key_store();
            let get_non_promotion_reason =
                |variable_key: PromotionKey| -> Option<NonPromotionReasonF<F>> {
                    let variable = key_store.variable_for_key(variable_key);
                    // `variableKey` should be one of the keys in
                    // `variableToDemote`; those keys in turn should always
                    // correspond to actual variables declared by the user. So
                    // `variable` should never be `null`.
                    debug_assert!(variables_to_demote.contains(&variable_key));
                    Some(NonPromotionReason::DemoteViaSuspension {
                        variable: variable.unwrap(),
                        node,
                    })
                };
            self.current = self.current.conservative_join(
                self,
                variables_to_demote.iter().copied(),
                [],
                Some(&get_non_promotion_reason),
            );
        }
    }

    fn switch_statement_after_case(&mut self) -> bool {
        let id = self.top_context_id();
        let is_locally_reachable = self.current.reachable.locally_reachable;
        self.current = self.current.unsplit();
        if is_locally_reachable {
            let FlowContext::SwitchStatement { target, .. } = self.context(id) else {
                self.bad_context(id, "_SwitchStatementContext")
            };
            let joined = self.join(target.break_model.as_ref(), Some(&self.current));
            let FlowContext::SwitchStatement { target, .. } = self.context_mut(id) else {
                unreachable!()
            };
            target.break_model = Some(joined);
        } else if !matches!(self.context(id), FlowContext::SwitchStatement { .. }) {
            self.bad_context(id, "_SwitchStatementContext")
        }
        is_locally_reachable
    }

    fn switch_statement_begin_alternative(&mut self) {
        let id = self.top_context_id();
        let FlowContext::SwitchAlternatives {
            switch_statement_context,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_SwitchAlternativesContext")
        };
        let FlowContext::SwitchStatement {
            unmatched,
            matched_value_info,
            ..
        } = self.context(*switch_statement_context)
        else {
            self.bad_context(*switch_statement_context, "_SwitchStatementContext")
        };
        let (unmatched, matched_value_info) = (unmatched.clone(), matched_value_info.clone());
        self.current = unmatched;
        self.push_pattern(matched_value_info);
    }

    fn switch_statement_begin_alternatives(&mut self) {
        let id = self.top_context_id();
        if !matches!(self.context(id), FlowContext::SwitchStatement { .. }) {
            self.bad_context(id, "_SwitchStatementContext")
        }
        self.push_context(FlowContext::SwitchAlternatives {
            switch_statement_context: id,
            pattern_variable_info: PatternVariableInfo::default(),
            combined_model: None,
        });
    }

    fn switch_statement_end(&mut self, is_exhaustive: bool) -> bool {
        let id = self.pop_context();
        let FlowContext::SwitchStatement {
            target,
            previous,
            unmatched,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_SwitchStatementContext")
        };
        let is_proven_exhaustive = !unmatched.reachable.locally_reachable;
        let mut break_state = target.break_model.clone();
        let unmatched = unmatched.clone();
        let previous = previous.clone();

        // If there is an implicit fall-through default, join it to any
        // breaks.
        if !is_exhaustive {
            break_state = Some(self.join(break_state.as_ref(), Some(&unmatched)));
        }

        // If there were no breaks (neither implicit nor explicit), then
        // `breakState` will be `null`.  This means this is an empty switch
        // statement and the type of the scrutinee is an exhaustive type.
        // This could happen, for instance, if the scrutinee type is an
        // abstract sealed class that has no subclasses.  It makes the most
        // sense to treat the code after the switch as unreachable, because
        // that's the normal behavior of a switch over an exhaustive type with
        // no `break`s.  It is sound to do so because the type is
        // uninhabited, therefore the body of the switch statement itself will
        // never be reached.
        let break_state = break_state.unwrap_or_else(|| previous.set_unreachable());

        self.current = break_state.unsplit();
        self.pop_scrutinee();
        is_proven_exhaustive
    }

    fn switch_statement_end_alternative(
        &mut self,
        guard_info: Option<ExpressionInfo<F>>,
        variables: &[(NameF<F>, VariableF<F>)],
    ) {
        let unmatched = self.pop_pattern(guard_info);
        let id = self.top_context_id();
        let FlowContext::SwitchAlternatives {
            switch_statement_context,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_SwitchAlternativesContext")
        };
        let switch_id = *switch_statement_context;
        // Future alternatives will be analyzed under the assumption that this
        // alternative didn't match.  This models the fact that a switch
        // statement behaves like a chain of if/else tests.
        let FlowContext::SwitchStatement {
            unmatched: switch_unmatched,
            ..
        } = self.context_mut(switch_id)
        else {
            unreachable!()
        };
        *switch_unmatched = unmatched;

        for (variable_name, variable) in variables.iter().copied() {
            let promotion_key = self.promotion_key_store().key_for_variable(variable);
            let FlowContext::SwitchAlternatives {
                pattern_variable_info,
                ..
            } = self.context_mut(id)
            else {
                unreachable!()
            };
            match pattern_variable_info
                .component_variables
                .iter_mut()
                .find(|(n, _)| *n == variable_name)
            {
                Some((_, vars)) => vars.push(variable),
                None => pattern_variable_info
                    .component_variables
                    .push((variable_name, vec![variable])),
            }
            // See if this variable appeared in any previous patterns that
            // share the same case body.
            let previous_promotion_key = pattern_variable_info
                .pattern_variable_promotion_keys
                .iter()
                .find(|(n, _)| *n == variable_name)
                .map(|(_, k)| *k);
            match previous_promotion_key {
                None => {
                    // This variable hasn't been seen in any previous patterns
                    // that share the same body.  So we can safely use the
                    // promotion key we have to store information about this
                    // variable.
                    pattern_variable_info
                        .pattern_variable_promotion_keys
                        .push((variable_name, promotion_key));
                }
                Some(previous_promotion_key) => {
                    // This variable has been seen in previous patterns, so we
                    // have to copy promotion data into the previously-used
                    // promotion key, to ensure that the promotion information
                    // is properly joined.
                    self.copy_promotion_data(promotion_key, previous_promotion_key);
                }
            }
        }
        let FlowContext::SwitchAlternatives { combined_model, .. } = self.context(id) else {
            unreachable!()
        };
        let joined = self.join(combined_model.as_ref(), Some(&self.current));
        let FlowContext::SwitchAlternatives { combined_model, .. } = self.context_mut(id) else {
            unreachable!()
        };
        *combined_model = Some(joined);
    }

    fn switch_statement_end_alternatives(
        &mut self,
        node: Option<F::Statement>,
        has_labels: bool,
    ) -> PatternVariableInfo<NameF<F>, VariableF<F>> {
        let alternatives_id = self.pop_context();
        let FlowContext::SwitchAlternatives {
            pattern_variable_info,
            combined_model,
            ..
        } = self.context(alternatives_id)
        else {
            self.bad_context(alternatives_id, "_SwitchAlternativesContext")
        };
        let (pattern_variable_info, combined_model) =
            (pattern_variable_info.clone(), combined_model.clone());
        let switch_id = self.top_context_id();
        let FlowContext::SwitchStatement {
            previous,
            unmatched,
            ..
        } = self.context(switch_id)
        else {
            self.bad_context(switch_id, "_SwitchStatementContext")
        };
        let (previous, unmatched) = (previous.clone(), unmatched.clone());
        if has_labels {
            let info = self
                .assigned_variables
                .get_info_for_node(F::statement_to_node(node.unwrap()));
            let written: Vec<PromotionKey> = info.written.iter().copied().collect();
            let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
            self.current = previous.conservative_join(self, written, captured, None);
        } else {
            self.current = combined_model.unwrap_or(unmatched);
        }
        // Do a control flow split so that in switchStatement_afterCase, we'll
        // be able to tell whether the end of the case body was reachable from
        // its start.
        self.current = self.current.split();
        pattern_variable_info
    }

    fn switch_statement_expression_end(
        &mut self,
        switch_statement: Option<F::Statement>,
        scrutinee_info: Option<ExpressionInfo<F>>,
        scrutinee_type: TypeViewF<F>,
    ) {
        let matched_value_info = self.push_scrutinee(scrutinee_info, scrutinee_type, true);
        self.current = self.current.split();
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let previous = self.current.clone();
        let id = self.push_context(FlowContext::SwitchStatement {
            target: BranchTargetData::new(checkpoint),
            previous: previous.clone(),
            matched_value_info,
            unmatched: previous,
        });
        if let Some(switch_statement) = switch_statement {
            self.statement_to_context.insert(switch_statement, id);
        }
    }

    fn this_binding_begin(&mut self, target_info: Option<ExpressionInfo<F>>) {
        let expression_reference = get_expression_reference(target_info.as_ref());
        let ssa_node = match expression_reference {
            Some(r) => r.reference_data().ssa_node.clone(),
            None => {
                SsaNode::with_condition_variable_state(target_info.filter(|i| i.is_non_trivial()))
            }
        };
        self.this_ssa_nodes.push(ssa_node);
    }

    fn this_binding_end(&mut self) {
        self.this_ssa_nodes.pop();
    }

    fn this_or_super(&mut self, static_type: TypeViewF<F>, is_super: bool) -> ExpressionInfo<F> {
        self.this_or_super_reference(static_type, is_super)
    }

    fn try_catch_statement_body_begin(&mut self) {
        self.current = self.current.split();
        let previous = self.current.clone();
        self.push_context(FlowContext::Try {
            previous,
            before_catch: None,
            after_body_and_catches: None,
        });
    }

    fn try_catch_statement_body_end(&mut self, body: F::Node) {
        let after_body = self.current.clone();

        let id = self.top_context_id();
        let FlowContext::Try { previous, .. } = self.context(id) else {
            self.bad_context(id, "_TryContext")
        };
        let before_body = previous.clone();

        let info = self.assigned_variables.get_info_for_node(body);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        let before_catch = before_body.conservative_join(self, written, captured, None);

        let FlowContext::Try {
            before_catch: ctx_before_catch,
            after_body_and_catches,
            ..
        } = self.context_mut(id)
        else {
            unreachable!()
        };
        *ctx_before_catch = Some(before_catch);
        *after_body_and_catches = Some(after_body);
    }

    fn try_catch_statement_catch_begin(
        &mut self,
        exception_variable: Option<VariableF<F>>,
        stack_trace_variable: Option<VariableF<F>>,
    ) {
        let id = self.top_context_id();
        let FlowContext::Try { before_catch, .. } = self.context(id) else {
            self.bad_context(id, "_TryContext")
        };
        self.current = before_catch.clone().unwrap();
        if let Some(exception_variable) = exception_variable {
            let exception_variable_key = self
                .promotion_key_store()
                .key_for_variable(exception_variable);
            self.current = self.current.declare(self, exception_variable_key, true);
        }
        if let Some(stack_trace_variable) = stack_trace_variable {
            let stack_trace_variable_key = self
                .promotion_key_store()
                .key_for_variable(stack_trace_variable);
            self.current = self.current.declare(self, stack_trace_variable_key, true);
        }
    }

    fn try_catch_statement_catch_end(&mut self) {
        let id = self.top_context_id();
        let FlowContext::Try {
            after_body_and_catches,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_TryContext")
        };
        let joined = self.join(after_body_and_catches.as_ref(), Some(&self.current));
        let FlowContext::Try {
            after_body_and_catches,
            ..
        } = self.context_mut(id)
        else {
            unreachable!()
        };
        *after_body_and_catches = Some(joined);
    }

    fn try_catch_statement_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::Try {
            after_body_and_catches,
            ..
        } = self.context(id)
        else {
            self.bad_context(id, "_TryContext")
        };
        self.current = after_body_and_catches.as_ref().unwrap().unsplit();
    }

    fn try_finally_statement_body_begin(&mut self) {
        let before_try = self.current.clone();
        self.push_context(FlowContext::TryFinally {
            before_try,
            after_try: None,
            before_finally: None,
        });
    }

    fn try_finally_statement_end(&mut self) {
        // See the "try finally" bullet in
        // https://github.com/dart-lang/language/blob/main/resources/type-system/flow-analysis.md#statements.
        let id = self.pop_context();
        let FlowContext::TryFinally {
            before_try,
            after_try,
            before_finally,
        } = self.context(id)
        else {
            self.bad_context(id, "_TryFinallyContext")
        };
        let before_try = before_try.clone();
        let after_try = after_try.clone().unwrap();
        let before_finally = before_finally.clone().unwrap();
        let after_finally = self.current.clone();

        // (OPTIMIZATION: the computation of `attachFinally` may be skipped in
        // two circumstances:
        // - If `before(B2)` and `after(B2)` are identical flow models
        //   (meaning nothing of consequence to flow analysis occurred in
        //   `B2`), then `after(N) = after(B1)`.
        if before_finally.ptr_eq(&after_finally) {
            self.current = after_try;
            return;
        }
        // - If `before(B1)`, `after(B1)`, and `before(B2)` are identical flow
        //   models (meaning nothing of consequence to flow analysis happened
        //   in `B1`), then `after(N) = after(B2)`.)
        if before_finally.ptr_eq(&before_try) && before_try.ptr_eq(&after_try) {
            self.current = after_finally;
            return;
        }

        // - Let `after(N) = attachFinally(after(B1), before(B2), after(B2))`.
        self.current = self.attach_finally(&after_try, &before_finally, &after_finally);
    }

    fn try_finally_statement_finally_begin(&mut self, body: F::Node) {
        let info = self.assigned_variables.get_info_for_node(body);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        let id = self.top_context_id();
        let FlowContext::TryFinally { before_try, .. } = self.context(id) else {
            self.bad_context(id, "_TryFinallyContext")
        };
        let before_try = before_try.clone();
        let after_try = self.current.clone();
        let joined_before = before_try.conservative_join(self, written, captured, None);
        self.current = self.join(Some(&self.current), Some(&joined_before));
        let before_finally = self.current.clone();
        let FlowContext::TryFinally {
            after_try: ctx_after_try,
            before_finally: ctx_before_finally,
            ..
        } = self.context_mut(id)
        else {
            unreachable!()
        };
        *ctx_after_try = Some(after_try);
        *ctx_before_finally = Some(before_finally);
    }

    fn variable_promotion_chain_for_testing(&self, variable: VariableF<F>) -> Vec<TypeViewF<F>> {
        self.query_key(variable)
            .and_then(|k| self.promotion_model(k))
            .map(|m| m.promoted_types.to_vec())
            .unwrap_or_default()
    }

    fn variable_read(
        &mut self,
        variable: VariableF<F>,
    ) -> (Option<TypeViewF<F>>, ExpressionInfo<F>) {
        let unpromoted_type = self.operations.variable_type(variable);
        let variable_key = self.promotion_key_store().key_for_variable(variable);
        let promotion_model = match self.promotion_model(variable_key) {
            Some(m) => m,
            None => {
                let m = PromotionModel::fresh(false, Some(SsaNode::new()));
                self.current = self
                    .current
                    .update_promotion_info(self, variable_key, m.clone());
                m
            }
        };
        let condition_variable_state = promotion_model
            .ssa_node
            .as_ref()
            .and_then(|n| n.condition_variable_state.clone());
        let expression_info = self
            .variable_reference(variable_key, unpromoted_type)
            .restore_condition_variable_state(
                condition_variable_state.as_ref(),
                self,
                &self.current,
            );
        (
            promotion_model.promoted_types.last().copied(),
            expression_info,
        )
    }

    fn while_statement_body_begin(
        &mut self,
        while_statement: F::Statement,
        condition_info: Option<ExpressionInfo<F>>,
    ) {
        let condition_info = condition_info
            .unwrap_or_else(|| self.make_trivial_expression_info(self.operations.bool_type()));
        let checkpoint = self.current.reachable.parent.clone().unwrap();
        let id = self.push_context(FlowContext::While {
            target: BranchTargetData::new(checkpoint),
            condition_false: condition_info.if_false.clone(),
        });
        self.statement_to_context.insert(while_statement, id);
        self.current = condition_info.if_true.clone();
    }

    fn while_statement_condition_begin(&mut self, node: F::Node) {
        self.current = self.current.split();
        let info = self.assigned_variables.get_info_for_node(node);
        let written: Vec<PromotionKey> = info.written.iter().copied().collect();
        let captured: Vec<PromotionKey> = info.captured.iter().copied().collect();
        self.current = self
            .current
            .conservative_join(self, written, captured, None);
    }

    fn while_statement_end(&mut self) {
        let id = self.pop_context();
        let FlowContext::While {
            target,
            condition_false,
        } = self.context(id)
        else {
            self.bad_context(id, "_WhileContext")
        };
        let (break_model, condition_false) = (target.break_model.clone(), condition_false.clone());
        self.current = self
            .join(Some(&condition_false), break_model.as_ref())
            .unsplit()
            .inherit_tested(self, &self.current);
    }

    fn why_not_promoted(
        &mut self,
        target_info: Option<ExpressionInfo<F>>,
    ) -> WhyNotPromoted<TypeOfF<F>, NonPromotionReasonOf<Self>> {
        match get_expression_reference(target_info.as_ref()) {
            Some(reference) => {
                let current_promotion_info =
                    self.promotion_model(reference.reference_data().promotion_key);
                self.get_non_promotion_reasons(reference, current_promotion_info.as_ref())
            }
            None => Box::new(Vec::new),
        }
    }

    fn why_not_promoted_implicit_this(
        &mut self,
        static_type: TypeViewF<F>,
    ) -> WhyNotPromoted<TypeOfF<F>, NonPromotionReasonOf<Self>> {
        if self.type_analyzer_options.this_promotion_enabled {
            return Box::new(Vec::new);
        }
        let Some(current_this_info) =
            self.promotion_model(self.promotion_key_store().this_promotion_key())
        else {
            return Box::new(Vec::new);
        };
        let reference = self.this_or_super_reference(static_type, false);
        self.get_non_promotion_reasons(&reference, Some(&current_this_info))
    }

    fn write(
        &mut self,
        node: F::Node,
        variable: VariableF<F>,
        written_type: TypeViewF<F>,
        written_expression_info: Option<ExpressionInfo<F>>,
    ) -> Option<ExpressionInfo<F>> {
        self.write_impl(node, variable, written_type, written_expression_info, false)
    }
}
