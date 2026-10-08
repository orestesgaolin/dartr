// No Dart source: test-only stand-in for `FlowAnalysis` (the real flow
// analysis, `flow_analysis.dart`, is ported in `dartr_flow` by units A9-A11).

//! [`MiniFlow`]: a minimal implementation of the `FlowAnalysis` trait for the
//! type analyzer tests, until the real flow analysis is available.
//!
//! It is NOT a port of flow analysis. It models only what the shared type
//! analyzer reads back from flow analysis:
//!
//! - the matched value type of patterns (`getMatchedValueType`): a stack of
//!   pattern contexts, with `promoteForPattern` and null-check patterns
//!   promoting the top context through `tryPromoteToType` and
//!   `promoteToNonNull`, as `_FlowAnalysisImpl.promoteForPattern` does for
//!   the synthetic matched-value variable;
//! - reachability (`isReachable`, `switchStatement_afterCase`): a boolean with
//!   simple joins for `if`, `switch`, labeled statements and local functions;
//!   loops and `try` statements are treated as completing normally;
//! - the pattern variable information of shared switch case scopes.
//!
//! Everything else (variable and property promotion, definite assignment,
//! `why not promoted`) is a no-op: queries return "not promoted",
//! "assigned", `None`. Tests that check such results are marked
//! `#[ignore = "needs flow analysis"]`.

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;

use dartr_flow::flow_analysis::{
    FlowAnalysis, FlowAnalysisNullShortingInterface, NonPromotionReasonOf, PatternVariableInfo,
    PromotionKey, PropertyTarget, WhyNotPromoted,
};
use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, TypeClassification,
};
use dartr_flow::shared_type::{NameOf, SharedTypeKind, SharedTypeOperations, SharedTypeView};

/// A pattern context: the matched value type and its promotion.
#[derive(Clone, Copy, Debug)]
struct PatternContext<T> {
    matched: SharedTypeView<T>,
    promoted: Option<SharedTypeView<T>>,
}

impl<T: Copy> PatternContext<T> {
    fn current(&self) -> SharedTypeView<T> {
        self.promoted.unwrap_or(self.matched)
    }
}

/// A saved promotion of a logical-or pattern: (before, after the LHS).
type SavedPromotion<T> = (Option<SharedTypeView<T>>, Option<SharedTypeView<T>>);

/// A flow control construct that joins reachability.
#[derive(Debug)]
enum Frame<N> {
    If {
        start: bool,
        after_then: Option<bool>,
    },
    Switch {
        node: Option<N>,
        start: bool,
        break_reachable: bool,
    },
    Labeled {
        node: N,
        break_reachable: bool,
    },
    Loop {
        node: Option<N>,
    },
    Function {
        outer: bool,
    },
    TryCatch {
        start: bool,
        end_reachable: bool,
    },
    TryFinally {
        start: bool,
        body_reachable: bool,
    },
    Conditional,
    SwitchAlternatives,
}

/// Test-only stand-in for flow analysis; see the module docs.
pub struct MiniFlow<O: FlowAnalysisOperations, N> {
    operations: Rc<O>,
    reachable: bool,
    frames: Vec<Frame<N>>,
    pattern_stack: Vec<PatternContext<O::Type>>,
    /// Saved promotions of logical-or patterns: (saved, lhs result).
    logical_or_stack: Vec<SavedPromotion<O::Type>>,
    next_promotion_key: PromotionKey,
    variable_keys: HashMap<O::Variable, PromotionKey>,
    pattern_variable_infos: Vec<PatternVariableInfo<NameOf<O>, O::Variable>>,
}

impl<O: FlowAnalysisOperations, N: Copy + Eq + Hash + Debug> MiniFlow<O, N> {
    /// Creates the stand-in.
    pub fn new(operations: Rc<O>) -> Self {
        MiniFlow {
            operations,
            reachable: true,
            frames: Vec::new(),
            pattern_stack: Vec::new(),
            logical_or_stack: Vec::new(),
            next_promotion_key: 0,
            variable_keys: HashMap::new(),
            pattern_variable_infos: Vec::new(),
        }
    }

    fn new_key(&mut self) -> PromotionKey {
        let key = self.next_promotion_key;
        self.next_promotion_key += 1;
        key
    }

    fn key_for_variable(&mut self, variable: O::Variable) -> PromotionKey {
        if let Some(&key) = self.variable_keys.get(&variable) {
            return key;
        }
        let key = self.new_key();
        self.variable_keys.insert(variable, key);
        key
    }

    fn push_pattern(&mut self, matched: SharedTypeView<O::Type>) {
        self.pattern_stack.push(PatternContext {
            matched,
            promoted: None,
        });
    }

    fn pop_pattern(&mut self) {
        self.pattern_stack.pop().expect("pattern context");
    }

    fn promote_top(&mut self, to: SharedTypeView<O::Type>) {
        let ops = Rc::clone(&self.operations);
        let top = self.pattern_stack.last_mut().expect("pattern context");
        if let Some(promoted) = ops.try_promote_to_type(to, top.current()) {
            top.promoted = Some(promoted);
        }
    }

    fn break_to(&mut self, target: Option<N>) {
        if let Some(target) = target {
            for frame in self.frames.iter_mut().rev() {
                match frame {
                    Frame::Switch {
                        node: Some(node),
                        break_reachable,
                        ..
                    }
                    | Frame::Labeled {
                        node,
                        break_reachable,
                    } if *node == target => {
                        if self.reachable {
                            *break_reachable = true;
                        }
                        break;
                    }
                    Frame::Loop { node: Some(node) } if *node == target => break,
                    _ => {}
                }
            }
        }
        self.reachable = false;
    }
}

impl<O: FlowAnalysisOperations, N: Copy + Eq + Hash + Debug> FlowAnalysisNullShortingInterface
    for MiniFlow<O, N>
{
    type Expression = N;
    type Variable = O::Variable;
    type Type = O::Type;
    type ExpressionInfo = ();

    fn null_aware_access_end(&mut self) {}

    fn null_aware_access_right_begin(
        &mut self,
        target_info: Option<()>,
        _target_type: SharedTypeView<O::Type>,
        _guard_variable: Option<O::Variable>,
    ) -> Option<()> {
        target_info
    }
}

impl<O: FlowAnalysisOperations + 'static, N: Copy + Eq + Hash + Debug + 'static> FlowAnalysis
    for MiniFlow<O, N>
{
    type Node = N;
    type Statement = N;
    type Operations = O;

    fn is_reachable(&self) -> bool {
        self.reachable
    }

    fn operations(&self) -> &O {
        &self.operations
    }

    fn promoted_type_of_this(&self) -> Option<SharedTypeView<O::Type>> {
        None
    }

    fn anonymous_block_body_begin(&mut self) {}

    fn anonymous_block_body_end(&mut self) {}

    fn as_expression_end(
        &mut self,
        _sub_expression_info: Option<()>,
        _sub_expression_type: SharedTypeView<O::Type>,
        _cast_type: SharedTypeView<O::Type>,
    ) {
    }

    fn assert_after_condition(&mut self, _condition_info: Option<()>) {}

    fn assert_begin(&mut self) {}

    fn assert_end(&mut self) {}

    fn assigned_variable_pattern(
        &mut self,
        _node: N,
        _variable: O::Variable,
        _written_type: SharedTypeView<O::Type>,
    ) {
    }

    fn assign_matched_pattern_variable(&mut self, _variable: O::Variable, _key: PromotionKey) {}

    fn boolean_literal(&mut self, _value: bool) {}

    fn cascade_expression_after_target(
        &mut self,
        _target_info: Option<()>,
        target_type: SharedTypeView<O::Type>,
        is_null_aware: bool,
        _guard_variable: Option<O::Variable>,
    ) -> SharedTypeView<O::Type> {
        if is_null_aware {
            self.operations.promote_to_non_null(target_type)
        } else {
            target_type
        }
    }

    fn cascade_expression_end(&mut self) {}

    fn conditional_condition_begin(&mut self) {
        self.frames.push(Frame::Conditional);
    }

    fn conditional_else_begin(
        &mut self,
        _then_expression_info: Option<()>,
        _then_type: SharedTypeView<O::Type>,
    ) {
    }

    fn conditional_end(
        &mut self,
        _conditional_expression_type: SharedTypeView<O::Type>,
        _else_expression_info: Option<()>,
        _else_type: SharedTypeView<O::Type>,
    ) {
        if let Some(Frame::Conditional) = self.frames.last() {
            self.frames.pop();
        }
    }

    fn conditional_then_begin(&mut self, _condition_info: Option<()>, _node: N) {}

    fn constant_pattern_end(
        &mut self,
        _expression_info: Option<()>,
        _ty: SharedTypeView<O::Type>,
        _patterns_enabled: bool,
        _matched_value_type: SharedTypeView<O::Type>,
    ) {
    }

    fn copy_promotion_data(&mut self, _source_key: PromotionKey, _destination_key: PromotionKey) {}

    fn declare(
        &mut self,
        variable: O::Variable,
        _static_type: SharedTypeView<O::Type>,
        _initialized: bool,
    ) {
        self.key_for_variable(variable);
    }

    fn declared_variable_pattern(
        &mut self,
        _matched_type: SharedTypeView<O::Type>,
        _static_type: SharedTypeView<O::Type>,
        _is_final: bool,
        _is_late: bool,
        _is_implicitly_typed: bool,
    ) -> PromotionKey {
        self.new_key()
    }

    fn do_statement_body_begin(&mut self, do_statement: N) {
        self.frames.push(Frame::Loop {
            node: Some(do_statement),
        });
    }

    fn do_statement_condition_begin(&mut self) {
        self.reachable = true;
    }

    fn do_statement_end(&mut self, _condition_info: Option<()>) {
        self.frames.pop();
        self.reachable = true;
    }

    fn equality_operation_end(
        &mut self,
        _left_operand_info: Option<()>,
        _left_operand_type: SharedTypeView<O::Type>,
        _right_operand_info: Option<()>,
        _right_operand_type: SharedTypeView<O::Type>,
        _not_equal: bool,
    ) -> Option<()> {
        None
    }

    fn equality_relational_pattern_end(
        &mut self,
        _operand_info: Option<()>,
        _operand_type: SharedTypeView<O::Type>,
        _not_equal: bool,
        _matched_value_type: SharedTypeView<O::Type>,
    ) {
    }

    fn finish(&mut self) {
        assert!(self.pattern_stack.is_empty(), "unbalanced pattern contexts");
    }

    fn for_body_begin(&mut self, _node: Option<N>, _condition_info: Option<()>) {}

    fn for_condition_begin(&mut self, node: N) {
        self.frames.push(Frame::Loop { node: Some(node) });
    }

    fn for_end(&mut self) {
        self.frames.pop();
        self.reachable = true;
    }

    fn for_updater_begin(&mut self) {
        self.reachable = true;
    }

    fn for_each_body_begin(&mut self, node: N) {
        self.frames.push(Frame::Loop { node: Some(node) });
    }

    fn for_each_end(&mut self) {
        self.frames.pop();
        self.reachable = true;
    }

    fn function_expression_begin(&mut self, _node: N) {
        self.frames.push(Frame::Function {
            outer: self.reachable,
        });
        self.reachable = true;
    }

    fn function_expression_end(&mut self) {
        match self.frames.pop() {
            Some(Frame::Function { outer }) => self.reachable = outer,
            other => panic!("unbalanced function frame: {other:?}"),
        }
    }

    fn get_matched_value_type(&self) -> SharedTypeView<O::Type> {
        self.pattern_stack
            .last()
            .expect("pattern context")
            .current()
    }

    fn handle_break(&mut self, target: Option<N>) {
        self.break_to(target);
    }

    fn handle_continue(&mut self, _target: Option<N>) {
        self.reachable = false;
    }

    fn handle_exit(&mut self) {
        self.reachable = false;
    }

    fn handle_return(&mut self) {
        self.reachable = false;
    }

    fn if_case_statement_after_expression(
        &mut self,
        _scrutinee_info: Option<()>,
        scrutinee_type: SharedTypeView<O::Type>,
    ) {
        self.push_pattern(scrutinee_type);
    }

    fn if_case_statement_begin(&mut self) {}

    fn if_case_statement_then_begin(&mut self, _guard_info: Option<()>) {
        self.pop_pattern();
        self.frames.push(Frame::If {
            start: self.reachable,
            after_then: None,
        });
    }

    fn if_null_expression_end(&mut self) {}

    fn if_null_expression_right_begin(
        &mut self,
        _left_hand_side_info: Option<()>,
        _left_hand_side_type: SharedTypeView<O::Type>,
    ) {
    }

    fn if_statement_condition_begin(&mut self) {}

    fn if_statement_else_begin(&mut self) {
        if let Some(Frame::If { start, after_then }) = self.frames.last_mut() {
            *after_then = Some(self.reachable);
            self.reachable = *start;
        } else {
            panic!("if_statement_else_begin without if");
        }
    }

    fn if_statement_end(&mut self, has_else: bool) {
        match self.frames.pop() {
            Some(Frame::If { start, after_then }) => {
                self.reachable = if has_else {
                    after_then.expect("else") || self.reachable
                } else {
                    self.reachable || start
                };
            }
            other => panic!("unbalanced if frame: {other:?}"),
        }
    }

    fn if_statement_then_begin(&mut self, _condition_info: Option<()>, _if_node: N) {
        self.frames.push(Frame::If {
            start: self.reachable,
            after_then: None,
        });
    }

    fn initialize(
        &mut self,
        _variable: O::Variable,
        _matched_type: SharedTypeView<O::Type>,
        _initializer_expression_info: Option<()>,
        _is_final: bool,
        _is_late: bool,
        _is_implicitly_typed: bool,
        _inherit_promotable_properties: bool,
    ) {
    }

    fn is_assigned(&self, _variable: O::Variable) -> bool {
        true
    }

    fn is_expression_end(
        &mut self,
        _sub_expression_info: Option<()>,
        _is_not: bool,
        _sub_expression_type: SharedTypeView<O::Type>,
        _checked_type: SharedTypeView<O::Type>,
    ) -> Option<()> {
        None
    }

    fn is_unassigned(&self, _variable: O::Variable) -> bool {
        false
    }

    fn labeled_statement_begin(&mut self, node: N) {
        self.frames.push(Frame::Labeled {
            node,
            break_reachable: false,
        });
    }

    fn labeled_statement_end(&mut self) {
        match self.frames.pop() {
            Some(Frame::Labeled {
                break_reachable, ..
            }) => self.reachable = self.reachable || break_reachable,
            other => panic!("unbalanced labeled frame: {other:?}"),
        }
    }

    fn late_initializer_begin(&mut self, _node: N) {}

    fn late_initializer_end(&mut self) {}

    fn logical_binary_op_begin(&mut self) {}

    fn logical_binary_op_end(&mut self, _right_operand_info: Option<()>, _is_and: bool) {}

    fn logical_binary_op_right_begin(
        &mut self,
        _left_operand_info: Option<()>,
        _whole_expression: N,
        _is_and: bool,
    ) {
    }

    fn logical_not_end(&mut self, _operand_info: Option<()>) -> Option<()> {
        None
    }

    fn logical_or_pattern_after_lhs(&mut self) {
        let lhs = self.pattern_stack.last().and_then(|c| c.promoted);
        let entry = self.logical_or_stack.last_mut().expect("logical-or");
        entry.1 = lhs;
        if let Some(top) = self.pattern_stack.last_mut() {
            top.promoted = entry.0;
        }
    }

    fn logical_or_pattern_begin(&mut self) {
        let saved = self.pattern_stack.last().and_then(|c| c.promoted);
        self.logical_or_stack.push((saved, None));
    }

    fn logical_or_pattern_end(&mut self) {
        let (saved, lhs) = self.logical_or_stack.pop().expect("logical-or");
        if let Some(top) = self.pattern_stack.last_mut()
            && top.promoted != lhs
        {
            top.promoted = saved;
        }
    }

    fn non_equality_relational_pattern_end(&mut self) {}

    fn non_null_assert_end(&mut self, _operand_info: Option<()>) {}

    fn null_aware_map_entry_end(&mut self, _is_key_null_aware: bool) {}

    fn null_aware_map_entry_value_begin(
        &mut self,
        _key_info: Option<()>,
        _key_type: SharedTypeView<O::Type>,
        _is_key_null_aware: bool,
    ) {
    }

    fn null_check_or_assert_pattern_begin(
        &mut self,
        _is_assert: bool,
        matched_value_type: SharedTypeView<O::Type>,
    ) -> bool {
        if self.operations.classify_type(matched_value_type) == TypeClassification::NonNullable {
            true
        } else {
            let non_null = self.operations.promote_to_non_null(matched_value_type);
            self.promote_top(non_null);
            false
        }
    }

    fn null_check_or_assert_pattern_end(&mut self) {}

    fn null_literal(&mut self, _ty: SharedTypeView<O::Type>) {}

    fn parenthesized_expression(&mut self, expression_info: Option<()>) -> Option<()> {
        expression_info
    }

    fn pattern_assignment_after_rhs(
        &mut self,
        _rhs_info: Option<()>,
        rhs_type: SharedTypeView<O::Type>,
    ) {
        self.push_pattern(rhs_type);
    }

    fn pattern_assignment_end(&mut self) {
        self.pop_pattern();
    }

    fn pattern_for_in_after_expression(&mut self, element_type: SharedTypeView<O::Type>) {
        self.push_pattern(element_type);
    }

    fn pattern_for_in_end(&mut self) {
        self.pop_pattern();
    }

    fn pattern_variable_declaration_after_initializer(
        &mut self,
        _initializer_info: Option<()>,
        initializer_type: SharedTypeView<O::Type>,
    ) {
        self.push_pattern(initializer_type);
    }

    fn pattern_variable_declaration_end(&mut self) {
        self.pop_pattern();
    }

    fn pop_property_subpattern(&mut self) {
        self.pop_pattern();
    }

    fn pop_subpattern(&mut self) {
        self.pop_pattern();
    }

    fn post_inc_dec(&mut self, _node: N, _variable: O::Variable, _ty: SharedTypeView<O::Type>) {}

    fn promoted_property_type(
        &mut self,
        _target: PropertyTarget<()>,
        _property_name: NameOf<O>,
        _property_member: Option<O::PropertyMember>,
        _unpromoted_type: SharedTypeView<O::Type>,
    ) -> Option<SharedTypeView<O::Type>> {
        None
    }

    fn promoted_type(&self, _variable: O::Variable) -> Option<SharedTypeView<O::Type>> {
        None
    }

    fn promote_for_pattern(
        &mut self,
        matched_type: SharedTypeView<O::Type>,
        known_type: SharedTypeView<O::Type>,
        _match_fails_if_wrong_type: bool,
        _match_may_fail_even_if_correct_type: bool,
    ) -> bool {
        let ops = Rc::clone(&self.operations);
        if ops.shared_type_kind(known_type.unwrap_type_view()) == SharedTypeKind::Invalid {
            return false;
        }
        let mut known_type = known_type;
        if ops.classify_type(matched_type) == TypeClassification::NonNullable {
            known_type = ops.promote_to_non_null(known_type);
        }
        let covers_matched_type = ops.is_subtype_of(
            ops.extension_type_erasure(matched_type),
            ops.extension_type_erasure(known_type),
        );
        self.promote_top(known_type);
        covers_matched_type
    }

    fn property_get(
        &mut self,
        _target: PropertyTarget<()>,
        _property_name: NameOf<O>,
        _property_member: Option<O::PropertyMember>,
        _unpromoted_type: SharedTypeView<O::Type>,
    ) -> (Option<SharedTypeView<O::Type>>, Option<()>) {
        (None, None)
    }

    fn property_promotion_chain_for_testing(
        &mut self,
        _target: PropertyTarget<()>,
        _property_name: NameOf<O>,
        _property_member: Option<O::PropertyMember>,
    ) -> Vec<SharedTypeView<O::Type>> {
        Vec::new()
    }

    fn push_property_subpattern(
        &mut self,
        _property_name: NameOf<O>,
        _property_member: Option<O::PropertyMember>,
        unpromoted_type: SharedTypeView<O::Type>,
    ) -> Option<SharedTypeView<O::Type>> {
        self.push_pattern(unpromoted_type);
        None
    }

    fn push_subpattern(&mut self, matched_type: SharedTypeView<O::Type>) {
        self.push_pattern(matched_type);
    }

    fn suspension(&mut self, _node: N) {}

    fn switch_statement_after_case(&mut self) -> bool {
        let is_locally_reachable = self.reachable;
        if let Some(Frame::Switch {
            break_reachable, ..
        }) = self.frames.last_mut()
            && is_locally_reachable
        {
            *break_reachable = true;
        }
        is_locally_reachable
    }

    fn switch_statement_begin_alternative(&mut self) {
        let scrutinee = *self.pattern_stack.last().expect("scrutinee");
        if let Some(Frame::Switch { start, .. }) = self.frames.iter().rev().find(|f| matches!(f, Frame::Switch { .. })) {
            self.reachable = *start;
        }
        self.push_pattern(scrutinee.matched);
    }

    fn switch_statement_begin_alternatives(&mut self) {
        self.frames.push(Frame::SwitchAlternatives);
        self.pattern_variable_infos
            .push(PatternVariableInfo::default());
    }

    fn switch_statement_end(&mut self, is_exhaustive: bool) -> bool {
        match self.frames.pop() {
            Some(Frame::Switch {
                start,
                break_reachable,
                ..
            }) => {
                self.reachable = break_reachable || (!is_exhaustive && start);
            }
            other => panic!("unbalanced switch frame: {other:?}"),
        }
        self.pop_pattern();
        false
    }

    fn switch_statement_end_alternative(
        &mut self,
        _guard_info: Option<()>,
        variables: &[(NameOf<O>, O::Variable)],
    ) {
        self.pop_pattern();
        for &(variable_name, variable) in variables {
            let promotion_key = self.key_for_variable(variable);
            let info = self
                .pattern_variable_infos
                .last_mut()
                .expect("switch alternatives");
            match info
                .component_variables
                .iter_mut()
                .find(|(name, _)| *name == variable_name)
            {
                Some(entry) => entry.1.push(variable),
                None => info
                    .component_variables
                    .push((variable_name, vec![variable])),
            }
            if !info
                .pattern_variable_promotion_keys
                .iter()
                .any(|(name, _)| *name == variable_name)
            {
                info.pattern_variable_promotion_keys
                    .push((variable_name, promotion_key));
            }
        }
    }

    fn switch_statement_end_alternatives(
        &mut self,
        _node: Option<N>,
        _has_labels: bool,
    ) -> PatternVariableInfo<NameOf<O>, O::Variable> {
        match self.frames.pop() {
            Some(Frame::SwitchAlternatives) => {}
            other => panic!("unbalanced switch alternatives frame: {other:?}"),
        }
        if let Some(Frame::Switch { start, .. }) = self.frames.last() {
            self.reachable = *start;
        }
        self.pattern_variable_infos
            .pop()
            .expect("switch alternatives")
    }

    fn switch_statement_expression_end(
        &mut self,
        switch_statement: Option<N>,
        _scrutinee_info: Option<()>,
        scrutinee_type: SharedTypeView<O::Type>,
    ) {
        self.push_pattern(scrutinee_type);
        self.frames.push(Frame::Switch {
            node: switch_statement,
            start: self.reachable,
            break_reachable: false,
        });
    }

    fn this_binding_begin(&mut self, _target_info: Option<()>) {}

    fn this_binding_end(&mut self) {}

    fn this_or_super(&mut self, _static_type: SharedTypeView<O::Type>, _is_super: bool) {}

    fn try_catch_statement_body_begin(&mut self) {
        self.frames.push(Frame::TryCatch {
            start: self.reachable,
            end_reachable: false,
        });
    }

    fn try_catch_statement_body_end(&mut self, _body: N) {
        if let Some(Frame::TryCatch { end_reachable, .. }) = self.frames.last_mut() {
            *end_reachable = self.reachable;
        }
    }

    fn try_catch_statement_catch_begin(
        &mut self,
        _exception_variable: Option<O::Variable>,
        _stack_trace_variable: Option<O::Variable>,
    ) {
        if let Some(Frame::TryCatch { start, .. }) = self.frames.last() {
            self.reachable = *start;
        }
    }

    fn try_catch_statement_catch_end(&mut self) {
        let reachable = self.reachable;
        if let Some(Frame::TryCatch { end_reachable, .. }) = self.frames.last_mut() {
            *end_reachable = *end_reachable || reachable;
        }
    }

    fn try_catch_statement_end(&mut self) {
        match self.frames.pop() {
            Some(Frame::TryCatch { end_reachable, .. }) => self.reachable = end_reachable,
            other => panic!("unbalanced try/catch frame: {other:?}"),
        }
    }

    fn try_finally_statement_body_begin(&mut self) {
        self.frames.push(Frame::TryFinally {
            start: self.reachable,
            body_reachable: false,
        });
    }

    fn try_finally_statement_end(&mut self) {
        match self.frames.pop() {
            Some(Frame::TryFinally { body_reachable, .. }) => {
                self.reachable = self.reachable && body_reachable;
            }
            other => panic!("unbalanced try/finally frame: {other:?}"),
        }
    }

    fn try_finally_statement_finally_begin(&mut self, _body: N) {
        let reachable = self.reachable;
        if let Some(Frame::TryFinally {
            start,
            body_reachable,
        }) = self.frames.last_mut()
        {
            *body_reachable = reachable;
            self.reachable = *start;
        }
    }

    fn variable_promotion_chain_for_testing(
        &self,
        _variable: O::Variable,
    ) -> Vec<SharedTypeView<O::Type>> {
        Vec::new()
    }

    fn variable_read(&mut self, _variable: O::Variable) -> (Option<SharedTypeView<O::Type>>, ()) {
        (None, ())
    }

    fn while_statement_body_begin(&mut self, _while_statement: N, _condition_info: Option<()>) {}

    fn while_statement_condition_begin(&mut self, node: N) {
        self.frames.push(Frame::Loop { node: Some(node) });
    }

    fn while_statement_end(&mut self) {
        self.frames.pop();
        self.reachable = true;
    }

    fn why_not_promoted(
        &mut self,
        _target_info: Option<()>,
    ) -> WhyNotPromoted<O::Type, NonPromotionReasonOf<Self>> {
        Box::new(Vec::new)
    }

    fn why_not_promoted_implicit_this(
        &mut self,
        _static_type: SharedTypeView<O::Type>,
    ) -> WhyNotPromoted<O::Type, NonPromotionReasonOf<Self>> {
        Box::new(Vec::new)
    }

    fn write(
        &mut self,
        _node: N,
        _variable: O::Variable,
        _written_type: SharedTypeView<O::Type>,
        _written_expression_info: Option<()>,
    ) -> Option<()> {
        None
    }
}
