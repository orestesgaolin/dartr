// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (the `setUp` of `main`, and the helpers at the end of the file:
// `_asserts`, `_matchOfInterestSet`, `_matchPromotionChain`,
// `_matchVariableModel`, `_MockNonPromotionReason`, `extension on
// FlowModel`, `extension on PromotionInfo?`)

//! Shared prelude and helpers of the translated flow analysis tests.

#![allow(dead_code, unused_imports)]

pub use dartr_flow::assigned_variables::AssignedVariablesImpl;
pub use dartr_flow::flow_analysis::{
    FlowAnalysis, NonPromotionDocumentationLink, PromotionKey, PropertyTarget,
};
pub use dartr_flow::flow_analysis_impl::FlowAnalysisImpl;
pub use dartr_flow::flow_analysis_impl::model::{
    FlowModelHelper, Reachability, opt_ptr_eq, promotion_info_get,
};
pub use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
};
pub use dartr_flow::shared_type::SharedTypeView;
pub use dartr_mini_ast::flow_analysis_mini_ast::*;
pub use dartr_mini_ast::harness::{BodyContext, Harness, RunOptions};
pub use dartr_mini_ast::mini_types::{Type, TypeRegistry, type_registry_scope};
pub use dartr_mini_ast::node::*;
pub use dartr_mini_ast::nodes;
pub use dartr_mini_ast::operations::{MiniAstOperations, MiniAstTypes};
pub use dartr_mini_ast::test_util::{Late, asserts_enabled, expect_asserts};
pub use indexmap::IndexMap;

use dartr_flow::flow_analysis_impl::model;

/// `SsaNode` of the mini AST.
pub type SsaNode = model::SsaNode<MiniAstTypes>;
/// `FlowModel` of the mini AST.
pub type FlowModel = model::FlowModel<MiniAstTypes>;
/// `PromotionModel` of the mini AST.
pub type PromotionModel = model::PromotionModel<MiniAstTypes>;
/// `ExpressionInfo` of the mini AST.
pub type ExpressionInfo = model::ExpressionInfo<MiniAstTypes>;
/// `PromotionInfo?` of the mini AST.
pub type PromotionInfo = model::PromotionInfoRef<MiniAstTypes>;

/// The Dart `setUp` of `main`: initializes the type registry, adds the
/// interface type names `A`..`F`, and creates the harness. The registry is
/// un-initialized when the harness is dropped (Dart `tearDown`).
pub fn set_up() -> FlowAnalysisTestHarness {
    let scope = type_registry_scope();
    for name in ["A", "B", "C", "D", "E", "F"] {
        TypeRegistry::add_interface_type_name(name);
    }
    FlowAnalysisTestHarness::new().with_registry_scope(scope)
}

/// Dart `Type(typeStr)`.
pub fn ty(type_str: &str) -> Type {
    Type::parse(type_str)
}

/// Dart `expectedErrors: {...}` (for [`Harness::run_with`]).
pub fn errors(expected: &[&str]) -> RunOptions {
    RunOptions {
        expected_errors: expected.iter().map(|s| s.to_string()).collect(),
        ..RunOptions::default()
    }
}

/// Dart `expect(actual, same(expected))` / `isNot(same(...))` for optional
/// SSA nodes (identity).
pub fn same_ssa(actual: Option<&SsaNode>, expected: Option<&SsaNode>) -> bool {
    opt_ptr_eq(actual, expected)
}

// ------------------------------------------------- matchers (test helpers)

/// Dart `_matchVariableModel(chain:, ofInterest:, assigned:, unassigned:,
/// writeCaptured:)`. `None` fields match anything. `chain` is compared in
/// order (Dart `_matchPromotionChain`), `of_interest` without order (Dart
/// `_matchOfInterestSet`, `unorderedEquals`); `isEmpty` is `&[]`.
#[derive(Default, Clone)]
pub struct VariableModelMatcher {
    pub chain: Option<Vec<String>>,
    pub of_interest: Option<Vec<String>>,
    pub of_interest_ordered: bool,
    pub assigned: Option<bool>,
    pub unassigned: Option<bool>,
    pub write_captured: Option<bool>,
}

/// Dart `_matchVariableModel(...)`: start with no requirements, then add
/// them with the builder methods.
pub fn match_variable_model() -> VariableModelMatcher {
    VariableModelMatcher::default()
}

impl VariableModelMatcher {
    /// `chain: [...]` (promoted types, in order).
    pub fn chain(mut self, chain: &[&str]) -> Self {
        self.chain = Some(chain.iter().map(|s| s.to_string()).collect());
        self
    }

    /// `ofInterest: [...]` (a list of type strings, any order).
    pub fn of_interest(mut self, types: &[&str]) -> Self {
        self.of_interest = Some(types.iter().map(|s| s.to_string()).collect());
        self.of_interest_ordered = false;
        self
    }

    /// `ofInterest: [Type(...), ...]` (a list of types: compared in order,
    /// Dart `equals`).
    pub fn of_interest_types(mut self, types: &[Type]) -> Self {
        self.of_interest = Some(types.iter().map(|t| t.type_string()).collect());
        self.of_interest_ordered = true;
        self
    }

    /// `assigned: ...`.
    pub fn assigned(mut self, value: bool) -> Self {
        self.assigned = Some(value);
        self
    }

    /// `unassigned: ...`.
    pub fn unassigned(mut self, value: bool) -> Self {
        self.unassigned = Some(value);
        self
    }

    /// `writeCaptured: ...`.
    pub fn write_captured(mut self, value: bool) -> Self {
        self.write_captured = Some(value);
        self
    }

    /// Whether `model` matches.
    pub fn matches(&self, model: &PromotionModel) -> bool {
        let strings = |l: &[SharedTypeView<Type>]| -> Vec<String> {
            l.iter()
                .map(|t| t.unwrap_type_view().type_string())
                .collect()
        };
        if let Some(chain) = &self.chain
            && strings(&model.promoted_types) != *chain
        {
            return false;
        }
        if let Some(of_interest) = &self.of_interest {
            let mut actual = strings(&model.tested);
            let mut expected = of_interest.clone();
            if !self.of_interest_ordered {
                actual.sort();
                expected.sort();
            }
            if actual != expected {
                return false;
            }
        }
        if let Some(a) = self.assigned
            && model.assigned != a
        {
            return false;
        }
        if let Some(u) = self.unassigned
            && model.unassigned != u
        {
            return false;
        }
        if let Some(w) = self.write_captured
            && model.write_captured() != w
        {
            return false;
        }
        true
    }

    /// Dart `expect(model, _matchVariableModel(...))`.
    #[track_caller]
    pub fn check(&self, model: &PromotionModel) {
        assert!(
            self.matches(model),
            "VariableModel(chain: {:?}, ofInterest: {:?}, assigned: {:?}, unassigned: {:?}, \
             writeCaptured: {:?}) does not match {model:?}",
            self.chain,
            self.of_interest,
            self.assigned,
            self.unassigned,
            self.write_captured
        );
    }
}

/// Dart `expect(map, {key: matcher, ...})` for a map from promotion keys to
/// promotion models.
#[track_caller]
pub fn expect_promotion_models(
    actual: &IndexMap<PromotionKey, PromotionModel>,
    expected: &[(PromotionKey, VariableModelMatcher)],
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "expected keys {:?}, got {:?}",
        expected.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>()
    );
    for (key, matcher) in expected {
        let model = actual
            .get(key)
            .unwrap_or_else(|| panic!("missing key {key}"));
        matcher.check(model);
    }
}

/// Dart `_MockNonPromotionReason()`: any non-promotion reason (the tests
/// only check that one is passed).
pub fn mock_non_promotion_reason() -> NonPromotionReason {
    NonPromotionReason::ThisNotPromoted
}

// ----------------------------------------- `extension on PromotionInfo?`

/// Dart `promotionInfo.unwrap(h)`: the map from promotion key to model.
pub fn unwrap_promotion_info(
    info: &PromotionInfo,
    h: &FlowAnalysisTestHarness,
) -> IndexMap<PromotionKey, PromotionModel> {
    let diff = h.reader.borrow_mut().diff(&None, info);
    diff.entries
        .iter()
        .map(|entry| {
            let right = entry.right().expect("right");
            (entry.key as PromotionKey, right.value.clone())
        })
        .collect()
}

// ---------------------------------------------- `extension on FlowModel`

/// The Dart test extension on `FlowModel` (private names: the leading `_`
/// becomes a `_var` suffix where the name would clash with a `FlowModel`
/// method).
pub trait FlowModelTestExt {
    /// `_conservativeJoin(h, writtenVariables, capturedVariables)`.
    fn conservative_join_vars(
        &self,
        h: &FlowAnalysisTestHarness,
        written_variables: &[Var],
        captured_variables: &[Var],
    ) -> FlowModel;
    /// `_declare(h, variable, initialized)`.
    fn declare_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
        initialized: bool,
    ) -> FlowModel;
    /// `_infoFor(h, variable)`.
    fn info_for_var(&self, h: &FlowAnalysisTestHarness, variable: Var) -> PromotionModel;
    /// `_setInfo(h, newInfo)`.
    fn set_info(
        &self,
        h: &FlowAnalysisTestHarness,
        new_info: &[(PromotionKey, PromotionModel)],
    ) -> FlowModel;
    /// `_tryMarkNonNullable(h, variable)`.
    fn try_mark_non_nullable_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
    ) -> ExpressionInfo;
    /// `_tryPromoteForTypeCheck(h, variable, type)`.
    fn try_promote_for_type_check_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
        type_: &str,
    ) -> ExpressionInfo;
    /// `_varRef(h, variable)`.
    fn var_ref(&self, h: &FlowAnalysisTestHarness, variable: Var) -> PromotionKey;
    /// `_varRefWithType(h, variable)`.
    fn var_ref_with_type(&self, h: &FlowAnalysisTestHarness, variable: Var) -> ExpressionInfo;
    /// `_write(h, nonPromotionReason, variable, writtenType, newSsaNode)`.
    fn write_var(
        &self,
        h: &FlowAnalysisTestHarness,
        non_promotion_reason: Option<&NonPromotionReason>,
        variable: Var,
        written_type: SharedTypeView<Type>,
        new_ssa_node: SsaNode,
    ) -> FlowModel;
}

impl FlowModelTestExt for FlowModel {
    fn conservative_join_vars(
        &self,
        h: &FlowAnalysisTestHarness,
        written_variables: &[Var],
        captured_variables: &[Var],
    ) -> FlowModel {
        self.conservative_join(
            h,
            written_variables.iter().map(|v| h.key_for_variable(*v)),
            captured_variables.iter().map(|v| h.key_for_variable(*v)),
            None,
        )
    }

    fn declare_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
        initialized: bool,
    ) -> FlowModel {
        self.declare(h, h.key_for_variable(variable), initialized)
    }

    fn info_for_var(&self, h: &FlowAnalysisTestHarness, variable: Var) -> PromotionModel {
        self.info_for(h, h.key_for_variable(variable), &SsaNode::new())
    }

    fn set_info(
        &self,
        h: &FlowAnalysisTestHarness,
        new_info: &[(PromotionKey, PromotionModel)],
    ) -> FlowModel {
        let mut result = self.clone();
        for (key, value) in new_info {
            let current = promotion_info_get(&result.promotion_info, h, *key);
            if !opt_ptr_eq(current.as_ref(), Some(value)) {
                result = result.update_promotion_info(h, *key, value.clone());
            }
        }
        result
    }

    fn try_mark_non_nullable_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
    ) -> ExpressionInfo {
        self.try_mark_non_nullable(h, &self.var_ref_with_type(h, variable))
    }

    fn try_promote_for_type_check_var(
        &self,
        h: &FlowAnalysisTestHarness,
        variable: Var,
        type_: &str,
    ) -> ExpressionInfo {
        self.try_promote_for_type_check(
            h,
            &self.var_ref_with_type(h, variable),
            SharedTypeView::new(Type::parse(type_)),
        )
    }

    fn var_ref(&self, h: &FlowAnalysisTestHarness, variable: Var) -> PromotionKey {
        h.key_for_variable(variable)
    }

    fn var_ref_with_type(&self, h: &FlowAnalysisTestHarness, variable: Var) -> ExpressionInfo {
        let type_ = promotion_info_get(&self.promotion_info, h, h.key_for_variable(variable))
            .and_then(|m| m.promoted_types.last().copied())
            .unwrap_or_else(|| SharedTypeView::new(variable.type_()));
        ExpressionInfo::trivial_variable_reference(
            type_,
            self.clone(),
            self.var_ref(h, variable),
            false,
            SsaNode::new(),
        )
    }

    fn write_var(
        &self,
        h: &FlowAnalysisTestHarness,
        non_promotion_reason: Option<&NonPromotionReason>,
        variable: Var,
        written_type: SharedTypeView<Type>,
        new_ssa_node: SsaNode,
    ) -> FlowModel {
        self.write(
            h,
            non_promotion_reason,
            h.key_for_variable(variable),
            written_type,
            new_ssa_node,
            true,
            SharedTypeView::new(variable.type_()),
        )
    }
}
