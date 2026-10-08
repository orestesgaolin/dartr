// Dart source: pkg/_fe_analyzer_shared/test/mini_ast.dart (classes `Harness`,
// `_MiniAstTypeAnalyzer`, `_MiniAstErrors`, `_VariableBinder`, `PreVisitor`,
// `BodyContext`, and the `preVisit`/`visit`/`computeSchema` methods of the
// AST classes)

//! The test harness: [`Harness`] configures a test and runs statements
//! through the shared type analyzer ([`MiniAstTypeAnalyzer`]).
//!
//! In Dart each AST class has `preVisit`, `visit` and `computeSchema`
//! methods. Here they are `match` arms in [`PreVisitor::pre_visit`],
//! [`MiniAstTypeAnalyzer::visit_expression`],
//! [`MiniAstTypeAnalyzer::visit_statement`],
//! [`MiniAstTypeAnalyzer::visit_pattern`],
//! [`MiniAstTypeAnalyzer::visit_collection_element`] and
//! [`MiniAstTypeAnalyzer::compute_schema`].
//!
//! Flow analysis is the stand-in [`MiniFlow`]; the pre-visit therefore
//! does not build `AssignedVariables` (only flow analysis reads them).

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use dartr_flow::body_inference_context::SharedBodyInferenceContext;
use dartr_flow::flow_analysis::{FlowAnalysis, FlowAnalysisNullShortingInterface, PropertyTarget};
use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
    TypeClassification,
};
use dartr_flow::null_shorting::TypeAnalysisNullShortingInterface;
use dartr_flow::shared_type::{
    SharedTypeKind, SharedTypeOperations, SharedTypeSchemaView, SharedTypeView,
};
use dartr_flow::type_analysis_result::{ExpressionTypeAnalysisResult, PatternResult};
use dartr_flow::type_analyzer::{
    CaseHeadOrDefaultInfo, JoinedPatternVariableInconsistency, JoinedPatternVariableLocation,
    MapPatternEntry, MatchContextOf, RecordPatternField, RecordPatternFieldOf,
    RelationalOperatorKind, RelationalOperatorResolution, SwitchExpressionMemberInfo,
    SwitchStatementMemberInfo, TypeAnalyzer, TypeAnalyzerErrors, TypeAnalyzerErrorsBase,
    TypeAnalyzerOptions,
};
use dartr_flow::type_analysis_result::UnnecessaryWildcardKind;
use dartr_flow::type_analyzer_operations::{KeyValueTypes, TypeAnalyzerOperations};
use dartr_flow::variable_bindings::{VariableBinder, VariableBinderErrors};
use dartr_type_analyzer::variable_bindings::VariableBinderState;

use super::mini_flow::MiniFlow;
use super::mini_ir::{Kind, MiniIrBuilder, MiniIrTmp};
use super::mini_types::{Name, Type, TypeKind};
use super::node::{
    loc_str, take_unused_error_ids, CatchClause, CollectionElementContext, ExprResult,
    ExprResultDetail, Label,
    Node, NodeKind, Promotable, PropertyElement, Var,
};
use super::operations::MiniAstOperations;

type View = SharedTypeView<Type>;
type SchemaView = SharedTypeSchemaView<Type>;

/// The type analyzer's match context in the mini-AST.
pub type SharedMatchContext = MatchContextOf<MiniAstTypeAnalyzer>;

/// `BodyContext`: the body inference context.
#[derive(Clone, Copy, Debug)]
pub struct BodyContext {
    /// `isAsync`.
    pub is_async: bool,
    /// `yieldContext`.
    pub yield_context: Type,
}

impl BodyContext {
    /// `BodyContext(isAsync: ..., yieldContext: Type(...))`.
    pub fn new(is_async: bool, yield_context: &str) -> Self {
        BodyContext {
            is_async,
            yield_context: Type::parse(yield_context),
        }
    }
}

impl SharedBodyInferenceContext<Type> for BodyContext {
    fn is_async(&self) -> bool {
        self.is_async
    }

    fn shared_yield_context(&self) -> SchemaView {
        SharedTypeSchemaView::new(self.yield_context)
    }
}

// ================================================================== errors

/// `_MiniAstErrors`: records each reported error as a string.
#[derive(Default)]
pub struct MiniAstErrors {
    /// `_accumulatedErrors` (Dart `Set<String>`).
    pub accumulated_errors: BTreeSet<String>,
    /// `_assertInErrorRecoveryStack`: set (to a description) if
    /// `assertInErrorRecovery` was called before any error was reported.
    pub assert_in_error_recovery_stack: Option<String>,
}

/// An argument of a recorded error (Dart `_recordError` `argumentStr`).
enum Arg {
    Bool(bool),
    Int(usize),
    Str(String),
    Node(Node),
    Var(Var),
    Type(View),
}

impl Arg {
    fn render(self) -> String {
        match self {
            Arg::Bool(b) => b.to_string(),
            Arg::Int(i) => i.to_string(),
            Arg::Str(s) => s,
            Arg::Node(n) => n.error_id_value(),
            Arg::Var(v) => v.error_id_value(),
            Arg::Type(t) => t.unwrap_type_view().type_string(),
        }
    }
}

impl MiniAstErrors {
    fn record_error(&mut self, name: &str, named_arguments: Vec<(&str, Arg)>) {
        let arguments_str: Vec<String> = named_arguments
            .into_iter()
            .map(|(key, value)| format!("{key}: {}", value.render()))
            .collect();
        let error_text = format!("{name}({})", arguments_str.join(", "));
        self.assert_in_error_recovery_stack = None;
        if !self.accumulated_errors.insert(error_text.clone()) {
            panic!("Same error reported twice: {error_text}");
        }
    }
}

impl TypeAnalyzerErrorsBase for MiniAstErrors {
    fn assert_in_error_recovery(&mut self) {
        if self.accumulated_errors.is_empty() && self.assert_in_error_recovery_stack.is_none() {
            self.assert_in_error_recovery_stack =
                Some(std::backtrace::Backtrace::force_capture().to_string());
        }
    }
}

impl TypeAnalyzerErrors for MiniAstErrors {
    type Node = Node;
    type Statement = Node;
    type Expression = Node;
    type Variable = Var;
    type Pattern = Node;
    type Error = ();
    type Type = Type;
    type Name = Name;

    fn case_expression_type_mismatch(
        &mut self,
        scrutinee: Node,
        case_expression: Node,
        scrutinee_type: View,
        case_expression_type: View,
    ) {
        self.record_error(
            "caseExpressionTypeMismatch",
            vec![
                ("scrutinee", Arg::Node(scrutinee)),
                ("caseExpression", Arg::Node(case_expression)),
                ("scrutineeType", Arg::Type(scrutinee_type)),
                ("caseExpressionType", Arg::Type(case_expression_type)),
            ],
        );
    }

    fn duplicate_assignment_pattern_variable(
        &mut self,
        variable: Var,
        original: Node,
        duplicate: Node,
    ) {
        self.record_error(
            "duplicateAssignmentPatternVariable",
            vec![
                ("variable", Arg::Var(variable)),
                ("original", Arg::Node(original)),
                ("duplicate", Arg::Node(duplicate)),
            ],
        );
    }

    fn duplicate_record_pattern_field(
        &mut self,
        object_or_record_pattern: Node,
        name: Name,
        original: RecordPatternField<Node, Node, Name>,
        duplicate: RecordPatternField<Node, Node, Name>,
    ) {
        self.record_error(
            "duplicateRecordPatternField",
            vec![
                ("objectOrRecordPattern", Arg::Node(object_or_record_pattern)),
                ("name", Arg::Str(name.to_string())),
                ("original", Arg::Node(original.node)),
                ("duplicate", Arg::Node(duplicate.node)),
            ],
        );
    }

    fn duplicate_rest_pattern(&mut self, map_or_list_pattern: Node, original: Node, duplicate: Node) {
        self.record_error(
            "duplicateRestPattern",
            vec![
                ("mapOrListPattern", Arg::Node(map_or_list_pattern)),
                ("original", Arg::Node(original)),
                ("duplicate", Arg::Node(duplicate)),
            ],
        );
    }

    fn empty_map_pattern(&mut self, pattern: Node) {
        self.record_error("emptyMapPattern", vec![("pattern", Arg::Node(pattern))]);
    }

    fn inconsistent_joined_pattern_variable(&mut self, variable: Var, component: Var) {
        self.record_error(
            "inconsistentJoinedPatternVariable",
            vec![
                ("variable", Arg::Str(variable.string_to_check_variables())),
                ("component", Arg::Var(component)),
            ],
        );
    }

    fn matched_type_is_strictly_non_nullable(&mut self, pattern: Node, matched_type: View) -> Option<()> {
        self.record_error(
            "matchedTypeIsStrictlyNonNullable",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("matchedType", Arg::Type(matched_type)),
            ],
        );
        Some(())
    }

    fn matched_type_is_subtype_of_required(
        &mut self,
        pattern: Node,
        matched_type: View,
        required_type: View,
    ) {
        self.record_error(
            "matchedTypeIsSubtypeOfRequired",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("matchedType", Arg::Type(matched_type)),
                ("requiredType", Arg::Type(required_type)),
            ],
        );
    }

    fn non_boolean_condition(&mut self, node: Node) {
        self.record_error("nonBooleanCondition", vec![("node", Arg::Node(node))]);
    }

    fn pattern_for_in_expression_is_not_iterable(
        &mut self,
        node: Node,
        expression: Node,
        expression_type: View,
    ) {
        self.record_error(
            "patternForInExpressionIsNotIterable",
            vec![
                ("node", Arg::Node(node)),
                ("expression", Arg::Node(expression)),
                ("expressionType", Arg::Type(expression_type)),
            ],
        );
    }

    fn pattern_type_mismatch_in_irrefutable_context(
        &mut self,
        pattern: Node,
        context: Node,
        matched_type: View,
        required_type: View,
    ) {
        self.record_error(
            "patternTypeMismatchInIrrefutableContext",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("context", Arg::Node(context)),
                ("matchedType", Arg::Type(matched_type)),
                ("requiredType", Arg::Type(required_type)),
            ],
        );
    }

    fn refutable_pattern_in_irrefutable_context(&mut self, pattern: Node, context: Node) {
        self.record_error(
            "refutablePatternInIrrefutableContext",
            vec![("pattern", Arg::Node(pattern)), ("context", Arg::Node(context))],
        );
    }

    fn relational_pattern_operand_type_not_assignable(
        &mut self,
        pattern: Node,
        operand_type: View,
        parameter_type: View,
    ) {
        self.record_error(
            "relationalPatternOperandTypeNotAssignable",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("operandType", Arg::Type(operand_type)),
                ("parameterType", Arg::Type(parameter_type)),
            ],
        );
    }

    fn relational_pattern_operator_return_type_not_assignable_to_bool(
        &mut self,
        pattern: Node,
        return_type: View,
    ) {
        self.record_error(
            "relationalPatternOperatorReturnTypeNotAssignableToBool",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("returnType", Arg::Type(return_type)),
            ],
        );
    }

    fn rest_pattern_in_map(&mut self, node: Node, element: Node) {
        self.record_error(
            "restPatternInMap",
            vec![("node", Arg::Node(node)), ("element", Arg::Node(element))],
        );
    }

    fn switch_case_completes_normally(&mut self, node: Node, case_index: usize) {
        self.record_error(
            "switchCaseCompletesNormally",
            vec![("node", Arg::Node(node)), ("caseIndex", Arg::Int(case_index))],
        );
    }

    fn unnecessary_wildcard_pattern(&mut self, pattern: Node, kind: UnnecessaryWildcardKind) {
        let kind_name = match kind {
            UnnecessaryWildcardKind::LogicalAndPatternOperand => "logicalAndPatternOperand",
        };
        self.record_error(
            "unnecessaryWildcardPattern",
            vec![
                ("pattern", Arg::Node(pattern)),
                ("kind", Arg::Str(kind_name.to_string())),
            ],
        );
    }
}

impl VariableBinderErrors for MiniAstErrors {
    type Node = Node;
    type Variable = Var;
    type Name = Name;

    fn duplicate_variable_pattern(&mut self, name: Name, original: Var, duplicate: Var) {
        self.record_error(
            "duplicateVariablePattern",
            vec![
                ("name", Arg::Str(name.to_string())),
                ("original", Arg::Var(original)),
                ("duplicate", Arg::Var(duplicate)),
            ],
        );
    }

    fn logical_or_pattern_branch_missing_variable(
        &mut self,
        node: Node,
        has_in_left: bool,
        name: Name,
        variable: Var,
    ) {
        self.record_error(
            "logicalOrPatternBranchMissingVariable",
            vec![
                ("node", Arg::Node(node)),
                ("hasInLeft", Arg::Bool(has_in_left)),
                ("name", Arg::Str(name.to_string())),
                ("variable", Arg::Var(variable)),
            ],
        );
    }
}

// ============================================================ pre-visit

/// `_VariableBinder`: joins pattern variables into the
/// `PatternVariableJoin` that the test created for them.
pub struct MiniVariableBinder;

impl VariableBinder for MiniVariableBinder {
    type Node = Node;
    type Variable = Var;
    type Key = Node;

    fn join_pattern_variables(
        &mut self,
        _key: Node,
        components: Vec<Var>,
        inconsistency: JoinedPatternVariableInconsistency,
    ) -> Var {
        let joined_variable = match components[0].data().joined_var {
            Some(joined) => joined,
            None => panic!("No joined variable for {}", components[0].location()),
        };
        // `PatternVariableJoin._handleJoin`.
        let join = joined_variable.data().join.expect("join data");
        assert!(!join.is_joined, "at {}", joined_variable.location());
        let identities: Vec<String> = components.iter().map(|c| c.identity()).collect();
        let expected_identities: Vec<String> = join
            .expected_components
            .iter()
            .map(|c| c.identity())
            .collect();
        assert_eq!(identities, expected_identities, "at {}", joined_variable.location());
        assert_eq!(
            components, join.expected_components,
            "at {}",
            joined_variable.location()
        );
        joined_variable.update(|d| {
            let join = d.join.as_mut().unwrap();
            join.inconsistency = inconsistency;
            join.is_joined = true;
        });
        joined_variable
    }
}

/// `PreVisitor` together with the `preVisit` methods of the AST classes.
pub struct PreVisitor<'a> {
    /// `errors`.
    pub errors: &'a mut MiniAstErrors,
}

/// The state of a Dart `_VariableBinder` created for one pre-visit.
type BinderState = VariableBinderState<Name, Var, Node>;

impl PreVisitor<'_> {
    /// `preVisit` of a statement, expression or collection element.
    pub fn pre_visit(&mut self, node: Node) {
        use NodeKind::*;
        match node.kind() {
            As { target, .. } => self.pre_visit(target),
            Assert { condition, message } => {
                self.pre_visit(condition);
                if let Some(message) = message {
                    self.pre_visit(message);
                }
            }
            Await { operand } => self.pre_visit(operand),
            Block { statements } => statements.into_iter().for_each(|s| self.pre_visit(s)),
            BooleanLiteral { .. }
            | Break { .. }
            | CascadePlaceholder
            | CheckAssigned { .. }
            | CheckReachable { .. }
            | CheckUnassigned { .. }
            | Continue { .. }
            | DotShorthandHead { .. }
            | IntLiteral { .. }
            | NullLiteral
            | PlaceholderExpression { .. }
            | Return
            | This
            | ThisOrSuperProperty { .. }
            | VariableReference { .. } => {}
            Cascade {
                target, sections, ..
            } => {
                self.pre_visit(target);
                sections.into_iter().for_each(|s| self.pre_visit(s));
            }
            CheckPromoted { promotable, .. } | CheckPromotionChain { promotable, .. } => {
                if let Promotable::Node(n) = promotable {
                    self.pre_visit(n);
                }
            }
            Conditional {
                condition,
                if_true,
                if_false,
            } => {
                self.pre_visit(condition);
                self.pre_visit(if_true);
                self.pre_visit(if_false);
            }
            Do { body, condition } => {
                self.pre_visit(body);
                self.pre_visit(condition);
            }
            DotShorthand { expr } => self.pre_visit(expr),
            Equal { lhs, rhs, .. } | IfNull { lhs, rhs } | Logical { lhs, rhs, .. } => {
                self.pre_visit(lhs);
                self.pre_visit(rhs);
            }
            ExpressionCollectionElement { expression } => self.pre_visit(expression),
            ExpressionInTypeSchema { expr, .. } | ExpressionStatement { expr } => {
                self.pre_visit(expr)
            }
            For {
                initializer,
                condition,
                updater,
                body,
                ..
            } => {
                if let Some(initializer) = initializer {
                    self.pre_visit(initializer);
                }
                if let Some(condition) = condition {
                    self.pre_visit(condition);
                }
                self.pre_visit(body);
                if let Some(updater) = updater {
                    self.pre_visit(updater);
                }
            }
            ForEach { iterable, body, .. } => {
                self.pre_visit(iterable);
                self.pre_visit(body);
            }
            If {
                condition,
                if_true,
                if_false,
            } => {
                self.pre_visit(condition);
                self.pre_visit(if_true);
                if let Some(if_false) = if_false {
                    self.pre_visit(if_false);
                }
            }
            IfCase {
                expression,
                pattern,
                guard,
                if_true,
                if_false,
                ..
            } => {
                self.pre_visit(expression);
                let mut binder = BinderState::new();
                binder.case_pattern_start();
                self.pre_visit_pattern(pattern, &mut binder, false);
                let candidate = binder.case_pattern_finish(None);
                binder.finish();
                node.update_kind(|k| {
                    if let IfCase {
                        candidate_variables,
                        ..
                    } = k
                    {
                        *candidate_variables = candidate.into_iter().collect();
                    }
                });
                if let Some(guard) = guard {
                    self.pre_visit(guard);
                }
                self.pre_visit(if_true);
                if let Some(if_false) = if_false {
                    self.pre_visit(if_false);
                }
            }
            IfCaseElement {
                expression,
                pattern,
                guard,
                if_true,
                if_false,
                ..
            } => {
                self.pre_visit(expression);
                let mut binder = BinderState::new();
                binder.case_pattern_start();
                self.pre_visit_pattern(pattern, &mut binder, false);
                let found = binder.case_pattern_finish(None);
                binder.finish();
                node.update_kind(|k| {
                    if let IfCaseElement { variables, .. } = k {
                        *variables = found.into_iter().collect();
                    }
                });
                if let Some(guard) = guard {
                    self.pre_visit(guard);
                }
                self.pre_visit(if_true);
                if let Some(if_false) = if_false {
                    self.pre_visit(if_false);
                }
            }
            IfElement {
                condition,
                if_true,
                if_false,
            } => {
                self.pre_visit(condition);
                self.pre_visit(if_true);
                if let Some(if_false) = if_false {
                    self.pre_visit(if_false);
                }
            }
            InvokeAnonymousMethod { target, body, .. } => {
                self.pre_visit(target);
                self.pre_visit(body);
            }
            InvokeMethod {
                target, arguments, ..
            } => {
                self.pre_visit(target);
                arguments.into_iter().for_each(|a| self.pre_visit(a));
            }
            Is { target, .. } => self.pre_visit(target),
            LabeledStatement { body, .. } => self.pre_visit(body),
            ListLiteral { elements, .. } | MapLiteral { elements, .. } => {
                elements.into_iter().for_each(|e| self.pre_visit(e))
            }
            LocalFunction { body, .. } => self.pre_visit(body),
            MapEntry { key, value, .. } => {
                self.pre_visit(key);
                self.pre_visit(value);
            }
            NonNullAssert { operand } | Not { operand } | Throw { operand } => {
                self.pre_visit(operand)
            }
            ParenthesizedExpression { expr } => self.pre_visit(expr),
            PatternAssignment { lhs, rhs } => {
                let mut binder = BinderState::new();
                binder.case_pattern_start();
                self.pre_visit_pattern(lhs, &mut binder, true);
                binder.case_pattern_finish(None);
                binder.finish();
                self.pre_visit(rhs);
            }
            PatternForIn {
                pattern,
                expression,
                body,
                ..
            }
            | PatternForInElement {
                pattern,
                expression,
                body,
                ..
            } => {
                self.pre_visit(expression);
                let mut binder = BinderState::new();
                binder.case_pattern_start();
                self.pre_visit_pattern(pattern, &mut binder, false);
                binder.case_pattern_finish(None);
                binder.finish();
                self.pre_visit(body);
            }
            PatternVariableDeclaration {
                pattern,
                initializer,
                ..
            } => {
                let mut binder = BinderState::new();
                binder.case_pattern_start();
                self.pre_visit_pattern(pattern, &mut binder, false);
                binder.case_pattern_finish(None);
                binder.finish();
                self.pre_visit(initializer);
            }
            PostIncDec { lhs } | PreIncDec { lhs } => self.pre_visit(lhs),
            Property { target, .. } => self.pre_visit(target),
            Second { first, second } => {
                self.pre_visit(first);
                self.pre_visit(second);
            }
            SwitchExpression { scrutinee, cases } => {
                self.pre_visit(scrutinee);
                for case_ in cases {
                    // `ExpressionCase._preVisit`.
                    let NodeKind::ExpressionCase {
                        guarded_pattern,
                        expression,
                    } = case_.kind()
                    else {
                        panic!("{case_:?} is not an expression case");
                    };
                    if let Some(guarded_pattern) = guarded_pattern {
                        let NodeKind::GuardedPattern { pattern, .. } = guarded_pattern.kind()
                        else {
                            unreachable!()
                        };
                        let mut binder = BinderState::new();
                        binder.case_pattern_start();
                        self.pre_visit_pattern(pattern, &mut binder, false);
                        let found = binder.case_pattern_finish(None);
                        set_guarded_pattern_variables(guarded_pattern, found.into_iter().collect());
                        binder.finish();
                    }
                    self.pre_visit(expression);
                }
            }
            SwitchStatement {
                scrutinee, cases, ..
            } => {
                self.pre_visit(scrutinee);
                for case_ in cases {
                    self.pre_visit_switch_statement_member(case_);
                }
            }
            TryStatement {
                body,
                catches,
                finally_statement,
            } => {
                self.pre_visit(body);
                for catch_ in catches {
                    self.pre_visit(catch_.body);
                }
                if let Some(finally_statement) = finally_statement {
                    self.pre_visit(finally_statement);
                }
            }
            VariableDeclaration { initializer, .. } => {
                if let Some(initializer) = initializer {
                    self.pre_visit(initializer);
                }
            }
            While { condition, body } => {
                self.pre_visit(condition);
                self.pre_visit(body);
            }
            WrappedExpression {
                before,
                expr,
                after,
            } => {
                if let Some(before) = before {
                    self.pre_visit(before);
                }
                self.pre_visit(expr);
                if let Some(after) = after {
                    self.pre_visit(after);
                }
            }
            Write { lhs, rhs } => {
                self.pre_visit(lhs);
                self.pre_visit(rhs);
            }
            YieldStatement { operand, .. } => self.pre_visit(operand),
            other => panic!("preVisit of {}", other.class_name()),
        }
    }

    /// `SwitchStatementMember._preVisit`.
    fn pre_visit_switch_statement_member(&mut self, member: Node) {
        let NodeKind::SwitchStatementMember {
            elements,
            body,
            has_labels,
            ..
        } = member.kind()
        else {
            panic!("{member:?} is not a switch statement member");
        };
        let mut binder = BinderState::new();
        binder.switch_statement_shared_case_scope_start(member);
        for element in elements {
            match element.kind() {
                NodeKind::SwitchHeadCase { guarded_pattern } => {
                    let NodeKind::GuardedPattern { pattern, guard, .. } = guarded_pattern.kind()
                    else {
                        unreachable!()
                    };
                    binder.case_pattern_start();
                    self.pre_visit_pattern(pattern, &mut binder, false);
                    if let Some(guard) = guard {
                        self.pre_visit(guard);
                    }
                    let found = binder.case_pattern_finish(Some(&member));
                    set_guarded_pattern_variables(guarded_pattern, found.into_iter().collect());
                }
                _ => binder.switch_statement_shared_case_scope_empty(&member),
            }
        }
        if has_labels {
            binder.switch_statement_shared_case_scope_empty(&member);
        }
        let candidate = binder.switch_statement_shared_case_scope_finish(&mut MiniVariableBinder, member);
        member.update_kind(|k| {
            if let NodeKind::SwitchStatementMember {
                candidate_variables,
                ..
            } = k
            {
                *candidate_variables = Some(candidate.into_iter().collect());
            }
        });
        self.pre_visit(body);
    }

    /// `Pattern.preVisit` (and `ListOrMapPatternElement.preVisit`).
    pub fn pre_visit_pattern(&mut self, node: Node, binder: &mut BinderState, is_in_assignment: bool) {
        use NodeKind::*;
        match node.kind() {
            CastPattern { inner, .. }
            | NullCheckOrAssertPattern { inner, .. }
            | ParenthesizedPattern { inner } => {
                self.pre_visit_pattern(inner, binder, is_in_assignment)
            }
            ConstantPattern { constant } => self.pre_visit(constant),
            ListPattern { elements, .. } | MapPattern { elements, .. } => {
                for element in elements {
                    self.pre_visit_pattern(element, binder, is_in_assignment);
                }
            }
            LogicalAndPattern { lhs, rhs } => {
                self.pre_visit_pattern(lhs, binder, is_in_assignment);
                self.pre_visit_pattern(rhs, binder, is_in_assignment);
            }
            LogicalOrPattern { lhs, rhs } => {
                binder.logical_or_pattern_start();
                self.pre_visit_pattern(lhs, binder, is_in_assignment);
                binder.logical_or_pattern_finish_left();
                self.pre_visit_pattern(rhs, binder, is_in_assignment);
                binder.logical_or_pattern_finish(&mut MiniVariableBinder, Some(&mut *self.errors), node);
            }
            MapPatternEntry { value, .. } => self.pre_visit_pattern(value, binder, is_in_assignment),
            ObjectPattern { fields, .. } | RecordPattern { fields } => {
                for field in fields {
                    let RecordPatternField { pattern, .. } = field.kind() else {
                        panic!("{field:?} is not a record pattern field");
                    };
                    self.pre_visit_pattern(pattern, binder, is_in_assignment);
                }
            }
            RelationalPattern { operand, .. } => self.pre_visit(operand),
            RestPattern { sub_pattern } => {
                if let Some(sub_pattern) = sub_pattern {
                    self.pre_visit_pattern(sub_pattern, binder, is_in_assignment);
                }
            }
            VariablePattern {
                declared_type,
                variable,
                ..
            } => {
                let is_assigned_variable = is_in_assignment;
                node.update_kind(|k| {
                    if let VariablePattern {
                        is_assigned_variable: slot,
                        ..
                    } = k
                    {
                        *slot = Some(is_assigned_variable);
                    }
                });
                if !is_assigned_variable {
                    binder.add(Some(&mut *self.errors), variable.name(), variable);
                }
                if is_assigned_variable {
                    assert!(
                        declared_type.is_none(),
                        "Variables in pattern assignments can't have declared types"
                    );
                }
            }
            WildcardPattern { .. } => {}
            other => panic!("preVisit (pattern) of {}", other.class_name()),
        }
    }
}

fn set_guarded_pattern_variables(guarded_pattern: Node, found: Vec<(Name, Var)>) {
    guarded_pattern.update_kind(|k| {
        if let NodeKind::GuardedPattern { variables, .. } = k {
            *variables = Some(found);
        }
    });
}

// ================================================================= harness

/// Options of [`Harness::run_with`] (Dart named parameters of `run`).
#[derive(Default)]
pub struct RunOptions {
    /// `errorRecoveryOK`.
    pub error_recovery_ok: bool,
    /// `expectedErrors`.
    pub expected_errors: Vec<String>,
    /// `bodyContext`.
    pub body_context: Option<BodyContext>,
}

/// `Harness`: the configuration of a test.
pub struct Harness {
    /// `operations`.
    pub operations: Rc<MiniAstOperations>,
    started: bool,
    inference_update3_enabled: Option<bool>,
    inference_update4_enabled: Option<bool>,
    this_promotion_enabled: Option<bool>,
    sound_flow_analysis_enabled: Option<bool>,
    patterns_enabled: Option<bool>,
    this_type: Option<Type>,
    /// `_members`: `None` means "explicitly no such member".
    members: HashMap<String, Option<PropertyElement>>,
    respect_implicitly_typed_var_initializers: bool,
    field_promotion_enabled: bool,
}

/// `Harness._coreMemberTypes`.
const CORE_MEMBER_TYPES: &[(&str, &str)] = &[
    ("int.<", "bool Function(num)"),
    ("int.<=", "bool Function(num)"),
    ("int.>", "bool Function(num)"),
    ("int.>=", "bool Function(num)"),
    ("int.abs", "int Function()"),
    ("int.isEven", "bool"),
    ("num.+", "num Function(num)"),
    ("num.sign", "num"),
    ("Object.toString", "String Function()"),
];

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    /// `Harness()`.
    pub fn new() -> Self {
        let members = CORE_MEMBER_TYPES
            .iter()
            .map(|(key, ty)| {
                let name = key.rsplit('.').next().unwrap();
                (
                    key.to_string(),
                    Some(PropertyElement::new(Type::parse(ty), name, false, None)),
                )
            })
            .collect();
        Harness {
            operations: Rc::new(MiniAstOperations::new()),
            started: false,
            inference_update3_enabled: None,
            inference_update4_enabled: None,
            this_promotion_enabled: None,
            sound_flow_analysis_enabled: None,
            patterns_enabled: None,
            this_type: None,
            members,
            respect_implicitly_typed_var_initializers: true,
            field_promotion_enabled: true,
        }
    }

    /// `patternsEnabled`.
    pub fn patterns_enabled(&self) -> bool {
        self.patterns_enabled.unwrap_or(true)
    }

    /// `thisType = type`.
    pub fn set_this_type(&mut self, ty: &str) {
        assert!(!self.started);
        self.this_type = Some(Type::parse(ty));
    }

    /// `addDownwardInfer(name:, context:, result:)`.
    pub fn add_downward_infer(&mut self, name: &str, context: &str, result: &str) {
        self.operations.add_downward_infer(name, context, result);
    }

    /// `addExhaustiveness(type, isExhaustive)`.
    pub fn add_exhaustiveness(&mut self, ty: &str, is_exhaustive: bool) {
        self.operations.add_exhaustiveness(ty, is_exhaustive);
    }

    /// `addExtensionTypeErasure(type, representation)`.
    pub fn add_extension_type_erasure(&mut self, ty: &str, representation: &str) {
        self.operations.add_extension_type_erasure(ty, representation);
    }

    /// `addLub(type1, type2, resultType)`.
    pub fn add_lub(&mut self, type1: &str, type2: &str, result_type: &str) {
        self.operations.add_lub(type1, type2, result_type);
    }

    /// `addMember(targetType, memberName, type, {promotable,
    /// whyNotPromotable})`.
    pub fn add_member(
        &mut self,
        target_type: &str,
        member_name: &str,
        ty: Option<&str>,
        promotable: bool,
        why_not_promotable: Option<PropertyNonPromotabilityReason>,
    ) {
        if promotable {
            assert!(why_not_promotable.is_none());
        }
        let query = format!("{target_type}.{member_name}");
        match ty {
            None => {
                if promotable {
                    panic!(
                        "It doesn't make sense to specify `promotable: true` when the type is `null`"
                    );
                }
                self.members.insert(query, None);
            }
            Some(ty) => {
                self.members.insert(
                    query,
                    Some(PropertyElement::new(
                        Type::parse(ty),
                        member_name,
                        promotable,
                        why_not_promotable,
                    )),
                );
            }
        }
    }

    /// `addPromotionException(from, to, result)`.
    pub fn add_promotion_exception(&mut self, from: &str, to: &str, result: &str) {
        self.operations.add_promotion_exception(from, to, result);
    }

    /// `addSuperInterfaces(className, template)`.
    pub fn add_super_interfaces(
        &mut self,
        class_name: &str,
        template: impl Fn(&[Type]) -> Vec<Type> + 'static,
    ) {
        self.operations.add_super_interfaces(class_name, template);
    }

    /// `computeTypeAnalyzerOptions()`.
    pub fn compute_type_analyzer_options(&self) -> TypeAnalyzerOptions {
        TypeAnalyzerOptions {
            patterns_enabled: self.patterns_enabled(),
            inference_update3_enabled: self.inference_update3_enabled.unwrap_or(true),
            respect_implicitly_typed_var_initializers: self
                .respect_implicitly_typed_var_initializers,
            field_promotion_enabled: self.field_promotion_enabled,
            inference_update4_enabled: self.inference_update4_enabled.unwrap_or(true),
            this_promotion_enabled: self.this_promotion_enabled.unwrap_or(true),
            sound_flow_analysis_enabled: self.sound_flow_analysis_enabled.unwrap_or(true),
        }
    }

    /// `disableFieldPromotion()`.
    pub fn disable_field_promotion(&mut self) {
        assert!(!self.started);
        self.field_promotion_enabled = false;
    }

    /// `disableInferenceUpdate3()`.
    pub fn disable_inference_update3(&mut self) {
        assert!(!self.started);
        self.inference_update3_enabled = Some(false);
    }

    /// `disableInferenceUpdate4()`.
    pub fn disable_inference_update4(&mut self) {
        assert!(!self.started);
        self.inference_update4_enabled = Some(false);
    }

    /// `disablePatterns()`.
    pub fn disable_patterns(&mut self) {
        assert!(!self.started);
        self.patterns_enabled = Some(false);
    }

    /// `disableRespectImplicitlyTypedVarInitializers()`.
    pub fn disable_respect_implicitly_typed_var_initializers(&mut self) {
        assert!(!self.started);
        self.respect_implicitly_typed_var_initializers = false;
    }

    /// `disableSoundFlowAnalysis()`.
    pub fn disable_sound_flow_analysis(&mut self) {
        assert!(!self.started);
        self.sound_flow_analysis_enabled = Some(false);
    }

    /// `disableThisPromotion()`.
    pub fn disable_this_promotion(&mut self) {
        assert!(!self.started);
        self.this_promotion_enabled = Some(false);
    }

    /// `run(statements)`.
    #[track_caller]
    pub fn run(&mut self, statements: Vec<Node>) {
        self.run_with(statements, RunOptions::default());
    }

    /// `run(statements, expectedErrors: {...})`.
    #[track_caller]
    pub fn run_expecting_errors(&mut self, statements: Vec<Node>, expected_errors: &[&str]) {
        self.run_with(
            statements,
            RunOptions {
                expected_errors: expected_errors.iter().map(|s| s.to_string()).collect(),
                ..RunOptions::default()
            },
        );
    }

    /// `run(statements, {errorRecoveryOK, expectedErrors, bodyContext})`:
    /// runs the statements through the type analyzer, checking any
    /// assertions they contain.
    #[track_caller]
    pub fn run_with(&mut self, statements: Vec<Node>, options: RunOptions) {
        self.started = true;
        let mut errors = MiniAstErrors::default();
        let b = super::node::block(statements);
        PreVisitor {
            errors: &mut errors,
        }
        .pre_visit(b);
        let mut type_analyzer = MiniAstTypeAnalyzer {
            harness_members: self.members.clone(),
            this_type: self.this_type,
            patterns_enabled: self.patterns_enabled(),
            operations: Rc::clone(&self.operations),
            flow: MiniFlow::new(Rc::clone(&self.operations)),
            errors,
            current_break_target: None,
            current_continue_target: None,
            ir_builder: MiniIrBuilder::new(),
            type_analyzer_options: self.compute_type_analyzer_options(),
            current_cascade_target_ir: None,
            current_cascade_target_type: None,
            body_context: Some(
                options
                    .body_context
                    .unwrap_or(BodyContext {
                        is_async: false,
                        yield_context: super::mini_types::UnknownType::new(),
                    }),
            ),
            guards: Vec::new(),
            dot_shorthands: Vec::new(),
            pending_detail: None,
        };
        type_analyzer.dispatch_statement(b);
        type_analyzer.finish();
        let expected: BTreeSet<String> = options.expected_errors.into_iter().collect();
        assert_eq!(type_analyzer.errors.accumulated_errors, expected);
        if !options.error_recovery_ok
            && let Some(stack) = &type_analyzer.errors.assert_in_error_recovery_stack
        {
            panic!("assertInErrorRecovery called but no errors reported: {stack}");
        }
        let unused = take_unused_error_ids();
        if !unused.is_empty() {
            panic!("Unused error ids: {}", unused.join(", "));
        }
    }
}

// =========================================================== type analyzer

/// `_MiniAstTypeAnalyzer`.
pub struct MiniAstTypeAnalyzer {
    harness_members: HashMap<String, Option<PropertyElement>>,
    /// `Harness._thisType`.
    this_type: Option<Type>,
    patterns_enabled: bool,
    operations: Rc<MiniAstOperations>,
    flow: MiniFlow<MiniAstOperations, Node>,
    /// `errors`.
    pub errors: MiniAstErrors,
    current_break_target: Option<Node>,
    current_continue_target: Option<Node>,
    /// `_irBuilder`.
    pub ir_builder: MiniIrBuilder,
    type_analyzer_options: TypeAnalyzerOptions,
    current_cascade_target_ir: Option<MiniIrTmp>,
    current_cascade_target_type: Option<View>,
    /// `bodyContext`.
    pub body_context: Option<BodyContext>,
    guards: Vec<MiniIrTmp>,
    dot_shorthands: Vec<(Node, SchemaView)>,
    /// The subclass fields of the result of the expression being visited
    /// (see [`ExprResultDetail`]).
    pending_detail: Option<ExprResultDetail>,
}

impl TypeAnalysisNullShortingInterface for MiniAstTypeAnalyzer {
    type Expression = Node;
    type Variable = Var;
    type Operations = MiniAstOperations;
    type Flow = MiniFlow<MiniAstOperations, Node>;
    type Guard = MiniIrTmp;

    fn flow(&mut self) -> &mut Self::Flow {
        &mut self.flow
    }

    fn operations(&self) -> &MiniAstOperations {
        &self.operations
    }

    fn guards(&self) -> &[MiniIrTmp] {
        &self.guards
    }

    fn guards_mut(&mut self) -> &mut Vec<MiniIrTmp> {
        &mut self.guards
    }

    fn handle_null_shorting_step(
        &mut self,
        _inner_result: ExprResult,
        guard: MiniIrTmp,
        inferred_type: View,
    ) -> ExprResult {
        let location = guard.location.clone();
        self.ir_builder.if_not_null(&guard, &location);
        ExpressionTypeAnalysisResult::new(inferred_type)
    }
}

fn loc(node: Node) -> String {
    node.location()
}

impl MiniAstTypeAnalyzer {
    fn ops(&self) -> Rc<MiniAstOperations> {
        Rc::clone(&self.operations)
    }

    fn unknown(&self) -> SchemaView {
        self.operations.unknown_type()
    }

    fn null_type(&self) -> Type {
        super::mini_types::NullType::instance()
    }

    /// `thisType`.
    fn this_type(&self) -> Type {
        self.this_type.expect("thisType")
    }

    // ------------------------------------------------------ analyze helpers

    fn analyze_assert_statement(&mut self, node: Node, condition: Node, message: Option<Node>) {
        self.flow.assert_begin();
        let unknown = self.unknown();
        let condition_analysis_result = self.analyze_expression(condition, unknown, false, false, false);
        self.flow
            .assert_after_condition(condition_analysis_result.flow_analysis_info);
        match message {
            Some(message) => {
                self.analyze_expression(message, unknown, false, false, false);
            }
            None => self.handle_no_message(node),
        }
        self.flow.assert_end();
    }

    fn analyze_binary_expression(&mut self, node: Node, lhs: Node, operator_name: &str, rhs: Node) -> ExprResult {
        let mut is_equals = false;
        let mut is_not = false;
        let mut is_logical = false;
        let mut is_and = false;
        let mut operator_name = operator_name;
        match operator_name {
            "==" => is_equals = true,
            "!=" => {
                is_equals = true;
                is_not = true;
                operator_name = "==";
            }
            "&&" => {
                is_logical = true;
                is_and = true;
            }
            "||" => is_logical = true,
            _ => {}
        }
        if operator_name == "==" {
            is_equals = true;
        } else if operator_name == "!=" {
            is_equals = true;
            is_not = true;
        }
        if is_logical {
            self.flow.logical_binary_op_begin();
        }
        let unknown = self.unknown();
        let left_analysis_result = self.analyze_expression(lhs, unknown, false, false, false);
        let left_type = left_analysis_result.type_;
        let mut left_info = None;
        if is_equals {
            left_info = left_analysis_result.flow_analysis_info;
        } else if is_logical {
            self.flow.logical_binary_op_right_begin(
                left_analysis_result.flow_analysis_info,
                node,
                is_and,
            );
        }
        let right_analysis_result = self.analyze_expression(rhs, unknown, false, false, false);
        let right_type = right_analysis_result.type_;
        let mut flow_analysis_info = None;
        if is_equals {
            flow_analysis_info = self.flow.equality_operation_end(
                left_info,
                left_type,
                right_analysis_result.flow_analysis_info,
                right_type,
                is_not,
            );
        } else if is_logical {
            self.flow
                .logical_binary_op_end(right_analysis_result.flow_analysis_info, is_and);
            flow_analysis_info = Some(());
        }
        ExpressionTypeAnalysisResult {
            type_: self.operations.bool_type(),
            flow_analysis_info,
        }
    }

    fn analyze_block(&mut self, statements: &[Node]) {
        for &statement in statements {
            self.dispatch_statement(statement);
        }
    }

    fn analyze_bool_literal(&mut self, value: bool) -> ExprResult {
        self.flow.boolean_literal(value);
        ExpressionTypeAnalysisResult {
            type_: self.operations.bool_type(),
            flow_analysis_info: Some(()),
        }
    }

    fn analyze_conditional_expression(
        &mut self,
        node: Node,
        condition: Node,
        if_true: Node,
        if_false: Node,
    ) -> ExprResult {
        self.flow.conditional_condition_begin();
        let unknown = self.unknown();
        let condition_analysis_result = self.analyze_expression(condition, unknown, false, false, false);
        self.flow
            .conditional_then_begin(condition_analysis_result.flow_analysis_info, node);
        let if_true_analysis_result = self.analyze_expression(if_true, unknown, false, false, false);
        let if_true_type = if_true_analysis_result.type_;
        self.flow
            .conditional_else_begin(if_true_analysis_result.flow_analysis_info, if_true_type);
        let if_false_analysis_result = self.analyze_expression(if_false, unknown, false, false, false);
        let if_false_type = if_false_analysis_result.type_;
        let lub_type = self.operations.lub(if_true_type, if_false_type);
        self.flow.conditional_end(
            lub_type,
            if_false_analysis_result.flow_analysis_info,
            if_false_type,
        );
        ExpressionTypeAnalysisResult {
            type_: lub_type,
            flow_analysis_info: Some(()),
        }
    }

    fn analyze_do_loop(&mut self, node: Node, body: Node, condition: Node) {
        self.flow.do_statement_body_begin(node);
        self.visit_loop_body(node, body);
        self.flow.do_statement_condition_begin();
        let unknown = self.unknown();
        let condition_analysis_result = self.analyze_expression(condition, unknown, false, false, false);
        self.flow
            .do_statement_end(condition_analysis_result.flow_analysis_info);
    }

    fn analyze_dot_shorthand_expression(&mut self, expression: Node, schema: SchemaView) -> ExprResult {
        let ty = self.analyze_dot_shorthand(expression, schema);
        ExpressionTypeAnalysisResult::new(ty)
    }

    fn analyze_dot_shorthand_head_expression(&mut self, node: Node, name: &str) -> ExprResult {
        self.ir_builder.atom(name, Kind::Expression, &loc(node));
        let context = self.get_dot_shorthand_context();
        ExpressionTypeAnalysisResult::new(SharedTypeView::new(context.unwrap_type_schema_view()))
    }

    fn analyze_expression_statement(&mut self, expression: Node) {
        let unknown = self.unknown();
        self.analyze_expression(expression, unknown, false, false, false);
    }

    fn analyze_if_null_expression(&mut self, lhs: Node, rhs: Node) -> ExprResult {
        let unknown = self.unknown();
        let left_analysis_result = self.analyze_expression(lhs, unknown, false, false, false);
        let left_type = left_analysis_result.type_;
        self.flow
            .if_null_expression_right_begin(left_analysis_result.flow_analysis_info, left_type);
        let right_type = self.analyze_expression(rhs, unknown, false, false, false).type_;
        self.flow.if_null_expression_end();
        let ops = self.ops();
        ExpressionTypeAnalysisResult::new(ops.lub(ops.promote_to_non_null(left_type), right_type))
    }

    fn analyze_labeled_statement(&mut self, node: Node, body: Node) {
        self.flow.labeled_statement_begin(node);
        self.dispatch_statement(body);
        self.flow.labeled_statement_end();
    }

    fn analyze_logical_not(&mut self, expression: Node) -> ExprResult {
        let unknown = self.unknown();
        let expression_analysis_result = self.analyze_expression(expression, unknown, false, false, false);
        let flow_analysis_info = self
            .flow
            .logical_not_end(expression_analysis_result.flow_analysis_info);
        ExpressionTypeAnalysisResult {
            type_: self.operations.bool_type(),
            flow_analysis_info,
        }
    }

    fn analyze_method_invocation(
        &mut self,
        node: Node,
        target: Option<Node>,
        method_name: &str,
        arguments: &[Node],
        is_null_aware: bool,
    ) -> ExprResult {
        // Analyze the target, generate its IR, and look up the method's type.
        let method_type = self
            .handle_property_target_and_member_lookup(None, target, method_name, &loc(node), is_null_aware)
            .type_
            .unwrap_type_view();
        let mut return_type = self.operations.dynamic_type().unwrap_type_view();
        let function_type = method_type.as_function_type();
        if let Some(function_type) = &function_type {
            return_type = function_type.return_type;
            if !function_type.named_parameters.is_empty() {
                panic!("Named parameters are not supported yet");
            } else if function_type.required_positional_parameter_count
                != function_type.positional_parameters.len()
            {
                panic!("Optional positional parameters are not supported yet");
            }
        }
        // Recursively analyze each argument.
        let mut input_kinds = vec![Kind::Expression];
        for (i, &argument) in arguments.iter().enumerate() {
            input_kinds.push(Kind::Expression);
            let schema = match &function_type {
                Some(function_type) if !method_type.is_question_type() => self
                    .operations
                    .type_to_schema(SharedTypeView::new(function_type.positional_parameters[i])),
                _ => self.unknown(),
            };
            self.analyze_expression(argument, schema, false, false, false);
        }
        // Form the IR for the member invocation.
        self.ir_builder
            .apply(method_name, &input_kinds, Kind::Expression, &loc(node), &[]);
        ExpressionTypeAnalysisResult::new(SharedTypeView::new(return_type))
    }

    fn analyze_non_null_assert(&mut self, expression: Node) -> ExprResult {
        let unknown = self.unknown();
        let expression_analysis_result = self.analyze_expression(expression, unknown, true, false, false);
        let ty = expression_analysis_result.type_;
        self.flow
            .non_null_assert_end(expression_analysis_result.flow_analysis_info);
        ExpressionTypeAnalysisResult::new(self.operations.promote_to_non_null(ty))
    }

    fn analyze_null_literal(&mut self) -> ExprResult {
        let null_type = SharedTypeView::new(self.null_type());
        self.flow.null_literal(null_type);
        ExpressionTypeAnalysisResult {
            type_: null_type,
            flow_analysis_info: Some(()),
        }
    }

    fn analyze_property_get(
        &mut self,
        node: Node,
        target: Option<Node>,
        property_name: &str,
        is_null_aware: bool,
    ) -> ExprResult {
        // Analyze the target, generate its IR, and look up the property's
        // type.
        let analysis_result = self.handle_property_target_and_member_lookup(
            Some(node),
            target,
            property_name,
            &loc(node),
            is_null_aware,
        );
        // Build the property get IR.
        self.ir_builder.property_get(property_name, &loc(node));
        analysis_result
    }

    fn analyze_this(&mut self) -> ExprResult {
        let promoted_type_of_this = self.flow.promoted_type_of_this().map(|t| t.unwrap_type_view());
        let this_type = promoted_type_of_this.unwrap_or_else(|| self.this_type());
        self.flow.this_or_super(SharedTypeView::new(this_type), false);
        ExpressionTypeAnalysisResult {
            type_: SharedTypeView::new(this_type),
            flow_analysis_info: Some(()),
        }
    }

    fn analyze_this_or_super_property_get(&mut self, property_name: &str, is_super_access: bool) -> ExprResult {
        let member = self.lookup_member(self.this_type(), property_name);
        let member_type = member
            .as_ref()
            .map(|m| m.ty)
            .unwrap_or_else(|| self.operations.dynamic_type().unwrap_type_view());
        let target = if is_super_access {
            PropertyTarget::Super
        } else {
            PropertyTarget::This
        };
        let (wrapped_promoted_type, flow_analysis_info) = self.flow.property_get(
            target,
            super::mini_types::intern(property_name),
            member,
            SharedTypeView::new(member_type),
        );
        let promoted_type = wrapped_promoted_type.map(|t| t.unwrap_type_view());
        ExpressionTypeAnalysisResult {
            type_: SharedTypeView::new(promoted_type.unwrap_or(member_type)),
            flow_analysis_info,
        }
    }

    fn analyze_throw(&mut self, expression: Node) -> ExprResult {
        let unknown = self.unknown();
        self.analyze_expression(expression, unknown, false, false, false);
        self.flow.handle_exit();
        ExpressionTypeAnalysisResult::new(self.operations.never_type())
    }

    fn analyze_try_statement(
        &mut self,
        node: Node,
        body: Node,
        catch_clauses: &[CatchClause],
        finally_block: Option<Node>,
    ) {
        if finally_block.is_some() {
            self.flow.try_finally_statement_body_begin();
        }
        if !catch_clauses.is_empty() {
            self.flow.try_catch_statement_body_begin();
        }
        self.dispatch_statement(body);
        if !catch_clauses.is_empty() {
            self.flow.try_catch_statement_body_end(body);
            for catch_ in catch_clauses {
                if let Some(exception) = catch_.exception {
                    exception.set_type_unchecked(
                        catch_
                            .exception_type
                            .unwrap_or_else(|| Type::parse("dynamic")),
                    );
                }
                if let Some(stack_trace) = catch_.stack_trace {
                    stack_trace.set_type_unchecked(Type::parse("StackTrace"));
                }
                self.flow
                    .try_catch_statement_catch_begin(catch_.exception, catch_.stack_trace);
                self.dispatch_statement(catch_.body);
                self.flow.try_catch_statement_catch_end();
            }
            self.flow.try_catch_statement_end();
        }
        match finally_block {
            Some(finally_block) => {
                self.flow.try_finally_statement_finally_begin(if catch_clauses.is_empty() {
                    body
                } else {
                    node
                });
                self.dispatch_statement(finally_block);
                self.flow.try_finally_statement_end();
            }
            None => self.handle_no_statement(node),
        }
    }

    fn analyze_type_cast(&mut self, expression: Node, ty: Type) -> ExprResult {
        let unknown = self.unknown();
        let sub_expression_analysis_result = self.analyze_expression(expression, unknown, false, false, false);
        let sub_expression_type = sub_expression_analysis_result.type_;
        self.flow.as_expression_end(
            sub_expression_analysis_result.flow_analysis_info,
            sub_expression_type,
            SharedTypeView::new(ty),
        );
        ExpressionTypeAnalysisResult::new(SharedTypeView::new(ty))
    }

    fn analyze_type_test(&mut self, expression: Node, ty: Type, is_inverted: bool) -> ExprResult {
        let unknown = self.unknown();
        let sub_expression_analysis_result = self.analyze_expression(expression, unknown, false, false, false);
        let sub_expression_type = sub_expression_analysis_result.type_;
        let flow_analysis_info = self.flow.is_expression_end(
            sub_expression_analysis_result.flow_analysis_info,
            is_inverted,
            sub_expression_type,
            SharedTypeView::new(ty),
        );
        ExpressionTypeAnalysisResult {
            type_: self.operations.bool_type(),
            flow_analysis_info,
        }
    }

    fn analyze_variable_get(&mut self, variable: Var, callback: Option<super::node::PromotedTypeCallback>) -> ExprResult {
        let (promoted_type, flow_analysis_info) = self.flow.variable_read(variable);
        if let Some(callback) = callback {
            callback(promoted_type.map(|t| t.unwrap_type_view()));
        }
        ExpressionTypeAnalysisResult {
            type_: promoted_type.unwrap_or_else(|| SharedTypeView::new(variable.type_())),
            flow_analysis_info: Some(flow_analysis_info),
        }
    }

    fn analyze_while_loop(&mut self, node: Node, condition: Node, body: Node) {
        self.flow.while_statement_condition_begin(node);
        let unknown = self.unknown();
        let condition_analysis_result = self.analyze_expression(condition, unknown, false, false, false);
        self.flow
            .while_statement_body_begin(node, condition_analysis_result.flow_analysis_info);
        self.visit_loop_body(node, body);
        self.flow.while_statement_end();
    }

    fn create_null_aware_guard(&mut self, target: Node, target_analysis_result: ExprResult) -> ExprResult {
        let tmp = self.ir_builder.allocate_tmp(&loc(target));
        let flow_analysis_info = self.start_null_shorting(
            tmp.clone(),
            target_analysis_result.flow_analysis_info,
            target_analysis_result.type_,
            None,
        );
        self.ir_builder.read_tmp(&tmp, &loc(target));
        ExpressionTypeAnalysisResult {
            type_: self.operations.promote_to_non_null(target_analysis_result.type_),
            flow_analysis_info,
        }
    }

    /// `finish()`.
    pub fn finish(&mut self) {
        self.flow.finish();
    }

    fn handle_assigned_variable_pattern(&mut self, node: Node, variable: Var, expect_inferred_type: Option<String>) {
        self.ir_builder.atom(variable.name(), Kind::Variable, &loc(node));
        self.ir_builder
            .apply("assignedVarPattern", &[Kind::Variable], Kind::Pattern, &loc(node), &[]);
        assert!(
            expect_inferred_type.is_none(),
            "assigned variable patterns don't get an inferred type"
        );
    }

    fn handle_declared_variable_pattern(
        &mut self,
        node: Node,
        variable: Var,
        expect_inferred_type: Option<String>,
        matched_type: Type,
        static_type: Type,
    ) {
        let location = loc(node);
        self.ir_builder.atom(variable.name(), Kind::Variable, &location);
        self.ir_builder
            .atom(&matched_type.type_string(), Kind::Type, &location);
        self.ir_builder
            .atom(&static_type.type_string(), Kind::Type, &location);
        self.ir_builder.apply(
            "varPattern",
            &[Kind::Variable, Kind::Type, Kind::Type],
            Kind::Pattern,
            &location,
            &["matchedType", "staticType"],
        );
        if let Some(expect_inferred_type) = expect_inferred_type {
            assert_eq!(
                static_type.type_string(),
                expect_inferred_type,
                "at {location}"
            );
        }
    }

    fn handle_no_condition(&mut self, node: Node) {
        self.ir_builder.atom("true", Kind::Expression, &loc(node));
    }

    fn handle_no_initializer(&mut self, node: Node) {
        self.ir_builder
            .atom("uninitialized", Kind::Statement, &loc(node));
    }

    fn handle_no_message(&mut self, node: Node) {
        self.ir_builder.atom("failure", Kind::Expression, &loc(node));
    }

    /// `lookupInterfaceMember` / `_lookupMember`.
    fn lookup_member(&self, receiver_type: Type, member_name: &str) -> Option<PropertyElement> {
        self.get_member(receiver_type, member_name)
    }

    /// `Harness.getMember`.
    fn get_member(&self, ty: Type, member_name: &str) -> Option<PropertyElement> {
        let query = format!("{ty}.{member_name}");
        if let Some(member) = self.harness_members.get(&query) {
            // An explicit map entry (even `null`) is the answer.
            return member.clone();
        }
        match member_name {
            // Assume that all types implement `Object.toString`.
            "toString" => self.harness_members[&format!("Object.{member_name}")].clone(),
            _ => {
                // It's legal to look up any member on the type `dynamic`.
                if ty.kind() == TypeKind::Dynamic {
                    return None;
                }
                // But an attempt to look up an unknown member on any other
                // type results in a test failure.
                panic!("Unknown member query: {query}");
            }
        }
    }

    /// `Harness.resolveRelationalPatternOperator`.
    fn harness_resolve_relational_pattern_operator(
        &self,
        matched_value_type: Type,
        operator: &str,
    ) -> Option<RelationalOperatorResolution<Type>> {
        if operator == "==" || operator == "!=" {
            return Some(RelationalOperatorResolution {
                kind: if operator == "==" {
                    RelationalOperatorKind::Equals
                } else {
                    RelationalOperatorKind::NotEquals
                },
                parameter_type: SharedTypeView::new(Type::parse("Object")),
                return_type: SharedTypeView::new(Type::parse("bool")),
            });
        }
        let member = self.get_member(matched_value_type, operator)?;
        let member_type = member.ty;
        let Some(function_type) = member_type.as_function_type().filter(|_| !member_type.is_question_type())
        else {
            panic!("{matched_value_type}.operator{operator} has type {member_type}; must be a function type");
        };
        if function_type.positional_parameters.is_empty() {
            panic!("{matched_value_type}.operator{operator} has type {member_type}; must accept a parameter");
        }
        Some(RelationalOperatorResolution {
            kind: RelationalOperatorKind::Other,
            parameter_type: SharedTypeView::new(function_type.positional_parameters[0]),
            return_type: SharedTypeView::new(function_type.return_type),
        })
    }

    /// `Harness._getIteratedType`.
    fn get_iterated_type(&self, iterable_type: Type) -> Type {
        let type_str = iterable_type.type_string();
        if type_str.starts_with("List<") && type_str.ends_with('>') {
            Type::parse(&type_str[5..type_str.len() - 1])
        } else {
            panic!("TODO(paulberry): getIteratedType({type_str})");
        }
    }

    /// `_handlePropertyTargetAndMemberLookup`.
    fn handle_property_target_and_member_lookup(
        &mut self,
        _property_get_node: Option<Node>,
        target: Option<Node>,
        property_name: &str,
        location: &str,
        is_null_aware: bool,
    ) -> ExprResult {
        // Analyze the target, and generate its IR.
        let property_target;
        let target_type;
        match target {
            None => {
                if is_null_aware {
                    panic!(
                        "at {location}: cascaded accesses shouldn't be marked as null-aware (the cascade itself should be marked as null-aware instead)."
                    );
                }
                // This is a cascaded access so the IR we need to generate is
                // an implicit read of the temporary variable holding the
                // cascade target.
                property_target = PropertyTarget::Cascade;
                let tmp = self.current_cascade_target_ir.clone().expect("cascade target");
                self.ir_builder.read_tmp(&tmp, location);
                target_type = self.current_cascade_target_type.expect("cascade type");
            }
            Some(target) => {
                let unknown = self.unknown();
                let mut target_analysis_result = self.analyze_expression(target, unknown, true, false, false);
                if is_null_aware {
                    target_analysis_result = self.create_null_aware_guard(target, target_analysis_result);
                }
                target_type = target_analysis_result.type_;
                property_target = PropertyTarget::Expression(target_analysis_result.flow_analysis_info);
            }
        }
        // Look up the type of the member, applying type promotion if
        // necessary.
        let member = self.lookup_member(target_type.unwrap_type_view(), property_name);
        let member_type = member
            .as_ref()
            .map(|m| m.ty)
            .unwrap_or_else(|| self.operations.dynamic_type().unwrap_type_view());
        let (wrapped_promoted_type, flow_analysis_info) = self.flow.property_get(
            property_target,
            super::mini_types::intern(property_name),
            member,
            SharedTypeView::new(member_type),
        );
        ExpressionTypeAnalysisResult {
            type_: wrapped_promoted_type.unwrap_or(SharedTypeView::new(member_type)),
            flow_analysis_info,
        }
    }

    /// `_irVariables`.
    fn ir_variables(&mut self, node: Node, variables: &[Var]) {
        for &variable in variables {
            self.ir_builder.atom(
                &variable.string_to_check_variables(),
                Kind::Variable,
                &variable.location(),
            );
        }
        let kinds = vec![Kind::Variable; variables.len()];
        self.ir_builder
            .apply("variables", &kinds, Kind::Variables, &loc(node), &[]);
    }

    /// `_visitLoopBody`.
    fn visit_loop_body(&mut self, lp: Node, body: Node) {
        let previous_break_target = self.current_break_target;
        let previous_continue_target = self.current_continue_target;
        self.current_break_target = Some(lp);
        self.current_continue_target = Some(lp);
        self.dispatch_statement(body);
        self.current_break_target = previous_break_target;
        self.current_continue_target = previous_continue_target;
    }

    fn record_pattern_fields(fields: &[Node]) -> Vec<RecordPatternFieldOf<Self>> {
        fields
            .iter()
            .map(|&field| {
                let NodeKind::RecordPatternField { name, pattern } = field.kind() else {
                    panic!("{field:?} is not a record pattern field");
                };
                RecordPatternField {
                    node: field,
                    name,
                    pattern,
                }
            })
            .collect()
    }

    // ------------------------------------------------------------ promotable

    /// `Promotable._getPromotedType`.
    fn get_promoted_type(&mut self, promotable: Promotable) -> Option<Type> {
        match promotable {
            Promotable::Var(variable) => {
                self.ir_builder
                    .atom(variable.name(), Kind::Expression, &variable.location());
                self.flow.promoted_type(variable).map(|t| t.unwrap_type_view())
            }
            Promotable::Node(node) => match node.kind() {
                NodeKind::This => {
                    self.ir_builder.atom("this", Kind::Expression, &loc(node));
                    self.flow.promoted_type_of_this().map(|t| t.unwrap_type_view())
                }
                NodeKind::Property {
                    target,
                    property_name,
                    is_null_aware,
                } => {
                    let (member, flow_analysis_info) =
                        self.compute_member_and_flow_analysis_info(node, target, &property_name, is_null_aware);
                    let member_type = member.as_ref().expect("member").ty;
                    self.flow
                        .promoted_property_type(
                            PropertyTarget::Expression(flow_analysis_info),
                            super::mini_types::intern(&property_name),
                            member,
                            SharedTypeView::new(member_type),
                        )
                        .map(|t| t.unwrap_type_view())
                }
                NodeKind::ThisOrSuperProperty {
                    property_name,
                    is_super_access,
                } => {
                    let member = self.compute_this_or_super_member(node, &property_name, is_super_access);
                    let member_type = member.as_ref().expect("member").ty;
                    self.flow
                        .promoted_property_type(
                            if is_super_access {
                                PropertyTarget::Super
                            } else {
                                PropertyTarget::This
                            },
                            super::mini_types::intern(&property_name),
                            member,
                            SharedTypeView::new(member_type),
                        )
                        .map(|t| t.unwrap_type_view())
                }
                other => panic!("{} is not promotable", other.class_name()),
            },
        }
    }

    /// `Promotable._getPromotionChain`.
    fn get_promotion_chain(&mut self, promotable: Promotable) -> Vec<Type> {
        match promotable {
            Promotable::Var(variable) => {
                self.ir_builder
                    .atom(variable.name(), Kind::Expression, &variable.location());
                self.flow
                    .variable_promotion_chain_for_testing(variable)
                    .into_iter()
                    .map(|t| t.unwrap_type_view())
                    .collect()
            }
            Promotable::Node(node) => match node.kind() {
                NodeKind::This => {
                    self.ir_builder.atom("this", Kind::Expression, &loc(node));
                    self.flow
                        .promoted_type_of_this()
                        .map(|t| vec![t.unwrap_type_view()])
                        .unwrap_or_default()
                }
                NodeKind::Property {
                    target,
                    property_name,
                    is_null_aware,
                } => {
                    let (member, flow_analysis_info) =
                        self.compute_member_and_flow_analysis_info(node, target, &property_name, is_null_aware);
                    self.flow
                        .property_promotion_chain_for_testing(
                            PropertyTarget::Expression(flow_analysis_info),
                            super::mini_types::intern(&property_name),
                            member,
                        )
                        .into_iter()
                        .map(|t| t.unwrap_type_view())
                        .collect()
                }
                NodeKind::ThisOrSuperProperty {
                    property_name,
                    is_super_access,
                } => {
                    let member = self.compute_this_or_super_member(node, &property_name, is_super_access);
                    self.flow
                        .property_promotion_chain_for_testing(
                            if is_super_access {
                                PropertyTarget::Super
                            } else {
                                PropertyTarget::This
                            },
                            super::mini_types::intern(&property_name),
                            member,
                        )
                        .into_iter()
                        .map(|t| t.unwrap_type_view())
                        .collect()
                }
                other => panic!("{} is not promotable", other.class_name()),
            },
        }
    }

    /// `Property._computeMemberAndFlowAnalysisInfo`.
    fn compute_member_and_flow_analysis_info(
        &mut self,
        node: Node,
        target: Node,
        property_name: &str,
        is_null_aware: bool,
    ) -> (Option<PropertyElement>, Option<()>) {
        if is_null_aware {
            panic!(
                "at {}: it doesn't make sense to compute the promoted type of a null-aware property.",
                loc(node)
            );
        }
        let unknown = self.unknown();
        let analysis_result = self.analyze_expression(target, unknown, false, false, false);
        let receiver_type = analysis_result.type_.unwrap_type_view();
        let member = self.lookup_member(receiver_type, property_name);
        (member, analysis_result.flow_analysis_info)
    }

    /// `ThisOrSuperProperty._computeMember`.
    fn compute_this_or_super_member(
        &mut self,
        node: Node,
        property_name: &str,
        is_super_access: bool,
    ) -> Option<PropertyElement> {
        let this_or_super = if is_super_access { "super" } else { "this" };
        self.ir_builder.atom(
            &format!("{this_or_super}.{property_name}"),
            Kind::Expression,
            &loc(node),
        );
        self.lookup_member(self.this_type(), property_name)
    }

    // ---------------------------------------------------- LValue writes

    /// `LValue._visitPostIncDec`.
    fn visit_post_inc_dec(&mut self, lhs: Node, post_inc_dec_expression: Node, written_type: Type) {
        match lhs.kind() {
            NodeKind::VariableReference { variable, .. } => {
                self.flow
                    .post_inc_dec(post_inc_dec_expression, variable, SharedTypeView::new(written_type));
            }
            NodeKind::Property { is_null_aware, .. } => {
                // TODO(paulberry): implement null-aware support
                assert!(!is_null_aware);
                // No flow analysis impact
            }
            NodeKind::ThisOrSuperProperty { .. } => {
                // No flow analysis impact
            }
            other => panic!("{} is not an LValue", other.class_name()),
        }
    }

    /// `LValue._visitWrite`.
    fn visit_write(
        &mut self,
        lhs: Node,
        assignment_expression: Node,
        written_type: Type,
        rhs_info: Option<()>,
    ) -> Option<()> {
        match lhs.kind() {
            NodeKind::VariableReference { variable, .. } => self.flow.write(
                assignment_expression,
                variable,
                SharedTypeView::new(written_type),
                rhs_info,
            ),
            NodeKind::Property { is_null_aware, .. } => {
                // TODO(paulberry): implement null-aware support
                assert!(!is_null_aware);
                // No flow analysis impact
                None
            }
            NodeKind::ThisOrSuperProperty { .. } => {
                // No flow analysis impact
                None
            }
            other => panic!("{} is not an LValue", other.class_name()),
        }
    }

    // ================================================================ visit

    /// `Expression.visit`.
    pub fn visit_expression(&mut self, node: Node, schema: SchemaView) -> ExprResult {
        use NodeKind::*;
        let location = loc(node);
        match node.kind() {
            As { target, ty } => self.analyze_type_cast(target, ty),
            Await { operand } => {
                let result = self.analyze_await_expression(node, operand, schema);
                self.ir_builder
                    .apply("awaitExpr", &[Kind::Expression], Kind::Expression, &location, &[]);
                let operand_type = result.operand_type.unwrap_type_view();
                let result: ExprResult = result.into();
                self.pending_detail = Some(ExprResultDetail {
                    result: result.clone(),
                    operand_type: Some(operand_type),
                    converted_to_double: None,
                });
                result
            }
            BooleanLiteral { value } => {
                self.ir_builder
                    .atom(&value.to_string(), Kind::Expression, &location);
                self.analyze_bool_literal(value)
            }
            Cascade {
                target,
                sections,
                is_null_aware,
            } => {
                // Form the IR for evaluating the LHS
                let target_analysis_result = self.analyze_expression(target, schema, false, false, false);
                let target_type = target_analysis_result.type_;
                let previous_cascade_target_ir = self.current_cascade_target_ir.take();
                let previous_cascade_type = self.current_cascade_target_type;
                // Create a let-variable that will be initialized to the value
                // of the LHS
                let target_tmp = self.ir_builder.allocate_tmp(&location);
                self.current_cascade_target_ir = Some(target_tmp.clone());
                self.current_cascade_target_type = Some(self.flow.cascade_expression_after_target(
                    target_analysis_result.flow_analysis_info,
                    target_type,
                    is_null_aware,
                    None,
                ));
                if is_null_aware {
                    // Push `targetTmp == null` and `targetTmp` on the IR
                    // builder stack, because they'll be needed later to form
                    // the conditional expression that does the null-aware
                    // guarding.
                    self.ir_builder.read_tmp(&target_tmp, &location);
                    self.ir_builder.atom("null", Kind::Expression, &location);
                    self.ir_builder.apply(
                        "==",
                        &[Kind::Expression, Kind::Expression],
                        Kind::Expression,
                        &location,
                        &[],
                    );
                    self.ir_builder.read_tmp(&target_tmp, &location);
                }
                // Form the IR for evaluating each section
                let mut section_tmps = Vec::new();
                for section in sections {
                    let unknown = self.unknown();
                    self.analyze_expression(section, unknown, false, false, false);
                    // Create a let-variable that will be initialized to the
                    // value of the section (which will be discarded)
                    section_tmps.push(self.ir_builder.allocate_tmp(&location));
                }
                // For the final IR, `let targetTmp = target in let
                // section1Tmp = section1 in section2Tmp = section2 ... in
                // targetTmp`, or, for null-aware cascades, `let targetTmp =
                // target in targetTmp == null ? targetTmp : let section1Tmp =
                // section1 in section2Tmp = section2 ... in targetTmp`.
                self.ir_builder.read_tmp(&target_tmp, &location);
                for tmp in section_tmps.iter().rev() {
                    self.ir_builder.let_(tmp, &location);
                }
                if is_null_aware {
                    self.ir_builder.apply(
                        "if",
                        &[Kind::Expression, Kind::Expression, Kind::Expression],
                        Kind::Expression,
                        &location,
                        &[],
                    );
                    self.flow.null_aware_access_end();
                }
                self.ir_builder.let_(&target_tmp, &location);
                self.flow.cascade_expression_end();
                self.current_cascade_target_ir = previous_cascade_target_ir;
                self.current_cascade_target_type = previous_cascade_type;
                ExpressionTypeAnalysisResult {
                    type_: target_type,
                    flow_analysis_info: Some(()),
                }
            }
            CascadePlaceholder => {
                let tmp = self.current_cascade_target_ir.clone().expect("cascade target");
                self.ir_builder.read_tmp(&tmp, &location);
                ExpressionTypeAnalysisResult::new(self.current_cascade_target_type.expect("cascade type"))
            }
            CheckAssigned { variable, expected } => {
                assert_eq!(self.flow.is_assigned(variable), expected, "at {location}");
                self.ir_builder.atom("null", Kind::Expression, &location);
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(self.null_type()))
            }
            CheckPromoted {
                promotable,
                expected,
            } => {
                let promoted_type = self.get_promoted_type(promotable);
                assert_eq!(
                    promoted_type.map(|t| t.type_string()),
                    expected,
                    "at {location}"
                );
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(self.null_type()))
            }
            CheckPromotionChain {
                promotable,
                expected,
            } => {
                let promotion_chain = self.get_promotion_chain(promotable);
                let chain: Vec<String> = promotion_chain.iter().map(|t| t.type_string()).collect();
                assert_eq!(chain, expected, "at {location}");
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(self.null_type()))
            }
            CheckReachable { expected } => {
                assert_eq!(self.flow.is_reachable(), expected, "at {location}");
                self.ir_builder.atom("null", Kind::Expression, &location);
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(self.null_type()))
            }
            CheckUnassigned { variable, expected } => {
                assert_eq!(self.flow.is_unassigned(variable), expected, "at {location}");
                self.ir_builder.atom("null", Kind::Expression, &location);
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(self.null_type()))
            }
            Conditional {
                condition,
                if_true,
                if_false,
            } => {
                let result = self.analyze_conditional_expression(node, condition, if_true, if_false);
                self.ir_builder.apply(
                    "if",
                    &[Kind::Expression, Kind::Expression, Kind::Expression],
                    Kind::Expression,
                    &location,
                    &[],
                );
                result
            }
            DotShorthand { expr } => self.analyze_dot_shorthand_expression(expr, schema),
            DotShorthandHead { name } => self.analyze_dot_shorthand_head_expression(node, &name),
            Equal {
                lhs,
                rhs,
                is_inverted,
            } => {
                let operator_name = if is_inverted { "!=" } else { "==" };
                let result = self.analyze_binary_expression(node, lhs, operator_name, rhs);
                self.ir_builder.apply(
                    operator_name,
                    &[Kind::Expression, Kind::Expression],
                    Kind::Expression,
                    &location,
                    &[],
                );
                result
            }
            IfNull { lhs, rhs } => {
                let result = self.analyze_if_null_expression(lhs, rhs);
                self.ir_builder.apply(
                    "ifNull",
                    &[Kind::Expression, Kind::Expression],
                    Kind::Expression,
                    &location,
                    &[],
                );
                result
            }
            IntLiteral { value } => {
                let result = self.analyze_int_literal(schema);
                let atom = if result.converted_to_double {
                    format!("{}f", dart_double_to_string(value as f64))
                } else {
                    value.to_string()
                };
                self.ir_builder.atom(&atom, Kind::Expression, &location);
                let converted_to_double = result.converted_to_double;
                let result: ExprResult = result.into();
                self.pending_detail = Some(ExprResultDetail {
                    result: result.clone(),
                    operand_type: None,
                    converted_to_double: Some(converted_to_double),
                });
                result
            }
            InvokeAnonymousMethod {
                target,
                body,
                return_type,
                is_null_aware,
                is_parameterless,
                parameter,
            } => {
                // Analyze the target, and generate its IR.
                let unknown = self.unknown();
                let mut target_result = self.analyze_expression(target, unknown, true, false, false);
                if is_null_aware {
                    target_result = self.create_null_aware_guard(target, target_result);
                }
                let target_info = target_result.flow_analysis_info;
                let previous_this_type = self.this_type;
                if is_parameterless {
                    self.flow.this_binding_begin(target_info);
                    self.this_type = Some(target_result.type_.unwrap_type_view());
                }
                self.flow.anonymous_block_body_begin();
                if let Some(parameter) = parameter {
                    let is_implicitly_typed = parameter.type_if_known().is_none();
                    if is_implicitly_typed {
                        parameter.set_type_unchecked(target_result.type_.unwrap_type_view());
                    }
                    self.flow
                        .declare(parameter, SharedTypeView::new(parameter.type_()), false);
                    self.flow.initialize(
                        parameter,
                        target_result.type_,
                        target_info,
                        false,
                        false,
                        is_implicitly_typed,
                        is_parameterless,
                    );
                }
                // Analyze the block, and generate its IR.
                self.visit_statement(body);
                self.flow.anonymous_block_body_end();
                if is_parameterless {
                    self.this_type = previous_this_type;
                    self.flow.this_binding_end();
                }
                // Form the IR for the anonymous method invocation.
                self.ir_builder.apply(
                    "anonymous-method",
                    &[Kind::Expression, Kind::Statement],
                    Kind::Expression,
                    &location,
                    &[],
                );
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(return_type))
            }
            InvokeMethod {
                target,
                method_name,
                arguments,
                is_null_aware,
            } => {
                let target = if matches!(target.kind(), CascadePlaceholder) {
                    None
                } else {
                    Some(target)
                };
                self.analyze_method_invocation(node, target, &method_name, &arguments, is_null_aware)
            }
            Is {
                target,
                ty,
                is_inverted,
            } => self.analyze_type_test(target, ty, is_inverted),
            ListLiteral {
                elements,
                element_type,
            } => {
                for &element in &elements {
                    self.dispatch_collection_element(
                        element,
                        CollectionElementContext::Type {
                            element_type_schema: SharedTypeSchemaView::new(element_type),
                        },
                    );
                }
                let kinds = vec![Kind::CollectionElement; elements.len()];
                self.ir_builder
                    .apply("list", &kinds, Kind::Expression, &location, &[]);
                ExpressionTypeAnalysisResult::new(
                    self.operations.list_type(SharedTypeView::new(element_type)),
                )
            }
            LocalFunction { body, ty } => {
                self.flow.function_expression_begin(node);
                self.dispatch_statement(body);
                self.flow.function_expression_end();
                self.ir_builder.apply(
                    "localFunction",
                    &[Kind::Statement],
                    Kind::Expression,
                    &location,
                    &[],
                );
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(ty))
            }
            Logical { lhs, rhs, is_and } => {
                let operator_name = if is_and { "&&" } else { "||" };
                let result = self.analyze_binary_expression(node, lhs, operator_name, rhs);
                self.ir_builder.apply(
                    operator_name,
                    &[Kind::Expression, Kind::Expression],
                    Kind::Expression,
                    &location,
                    &[],
                );
                result
            }
            MapLiteral {
                elements,
                key_type,
                value_type,
            } => {
                let context = CollectionElementContext::MapEntry {
                    key_type,
                    value_type,
                };
                for &element in &elements {
                    self.dispatch_collection_element(element, context);
                }
                let kinds = vec![Kind::CollectionElement; elements.len()];
                self.ir_builder
                    .apply("map", &kinds, Kind::Expression, &location, &[]);
                ExpressionTypeAnalysisResult::new(self.operations.map_type(
                    SharedTypeView::new(key_type),
                    SharedTypeView::new(value_type),
                ))
            }
            NonNullAssert { operand } => self.analyze_non_null_assert(operand),
            Not { operand } => self.analyze_logical_not(operand),
            NullLiteral => {
                let result = self.analyze_null_literal();
                self.ir_builder.atom("null", Kind::Expression, &location);
                result
            }
            ParenthesizedExpression { expr } => self.analyze_expression(expr, schema, false, false, false),
            PatternAssignment { lhs, rhs } => {
                let result = self.analyze_pattern_assignment(node, lhs, rhs);
                self.ir_builder.apply(
                    "patternAssignment",
                    &[Kind::Expression, Kind::Pattern],
                    Kind::Expression,
                    &location,
                    &[],
                );
                result.into()
            }
            PlaceholderExpression { ty } => {
                self.ir_builder.atom(&ty.type_string(), Kind::Type, &location);
                self.ir_builder
                    .apply("expr", &[Kind::Type], Kind::Expression, &location, &[]);
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(ty))
            }
            PostIncDec { lhs } => {
                let unknown = self.unknown();
                let operand_type = self
                    .analyze_expression(lhs, unknown, false, false, false)
                    .type_
                    .unwrap_type_view();
                let Some(member) = self.get_member(operand_type, "+") else {
                    panic!("No + operator found for {operand_type}");
                };
                let Some(member_type) = member.ty.as_function_type() else {
                    panic!("Expected function type for + operator, got {}", member.ty);
                };
                let written_type = member_type.return_type;
                self.visit_post_inc_dec(lhs, node, written_type);
                ExpressionTypeAnalysisResult::new(SharedTypeView::new(operand_type))
            }
            PreIncDec { lhs } => {
                let unknown = self.unknown();
                let operand_type = self
                    .analyze_expression(lhs, unknown, false, false, false)
                    .type_
                    .unwrap_type_view();
                let Some(member) = self.get_member(operand_type, "+") else {
                    panic!("No + operator found for {operand_type}");
                };
                let Some(member_type) = member.ty.as_function_type() else {
                    panic!("Expected function type for + operator, got {}", member.ty);
                };
                let written_type = member_type.return_type;
                let flow_analysis_info = self.visit_write(lhs, node, written_type, None);
                ExpressionTypeAnalysisResult {
                    type_: SharedTypeView::new(written_type),
                    flow_analysis_info,
                }
            }
            Property {
                target,
                property_name,
                is_null_aware,
            } => {
                let target = if matches!(target.kind(), CascadePlaceholder) {
                    None
                } else {
                    Some(target)
                };
                self.analyze_property_get(node, target, &property_name, is_null_aware)
            }
            Second { first, second } => {
                let unknown = self.unknown();
                self.analyze_expression(first, unknown, false, false, false);
                let ty = self.analyze_expression(second, schema, false, false, false).type_;
                self.ir_builder.apply(
                    "second",
                    &[Kind::Expression, Kind::Expression],
                    Kind::Expression,
                    &location,
                    &[],
                );
                ExpressionTypeAnalysisResult::new(ty)
            }
            SwitchExpression { scrutinee, cases } => {
                let result = self.analyze_switch_expression(node, scrutinee, cases.len(), schema);
                let mut kinds = vec![Kind::Expression];
                kinds.extend(std::iter::repeat_n(Kind::ExpressionCase, cases.len()));
                self.ir_builder
                    .apply("switchExpr", &kinds, Kind::Expression, &location, &[]);
                result.into()
            }
            This => {
                let result = self.analyze_this();
                self.ir_builder.atom("this", Kind::Expression, &location);
                result
            }
            ThisOrSuperProperty {
                property_name,
                is_super_access,
            } => {
                let result = self.analyze_this_or_super_property_get(&property_name, is_super_access);
                let this_or_super = if is_super_access { "super" } else { "this" };
                self.ir_builder.atom(
                    &format!("{this_or_super}.{property_name}"),
                    Kind::Expression,
                    &location,
                );
                result
            }
            Throw { operand } => self.analyze_throw(operand),
            VariableReference { variable, callback } => {
                let result = self.analyze_variable_get(variable, callback);
                self.ir_builder
                    .atom(variable.name(), Kind::Expression, &location);
                result
            }
            WrappedExpression {
                before,
                expr,
                after,
            } => {
                let mut before_tmp = None;
                if let Some(before) = before {
                    self.dispatch_statement(before);
                    self.ir_builder
                        .apply("expr", &[Kind::Statement], Kind::Expression, &location, &[]);
                    before_tmp = Some(self.ir_builder.allocate_tmp(&location));
                }
                let unknown = self.unknown();
                let analysis_result = self.analyze_expression(expr, unknown, false, false, false);
                if let Some(after) = after {
                    let expr_tmp = self.ir_builder.allocate_tmp(&location);
                    self.dispatch_statement(after);
                    self.ir_builder
                        .apply("expr", &[Kind::Statement], Kind::Expression, &location, &[]);
                    let after_tmp = self.ir_builder.allocate_tmp(&location);
                    self.ir_builder.read_tmp(&expr_tmp, &location);
                    self.ir_builder.let_(&after_tmp, &location);
                    self.ir_builder.let_(&expr_tmp, &location);
                }
                if let Some(before_tmp) = before_tmp {
                    self.ir_builder.let_(&before_tmp, &location);
                }
                analysis_result
            }
            Write { lhs, rhs } => {
                let unknown = self.unknown();
                let rhs_analysis_result = self.analyze_expression(rhs, unknown, false, false, false);
                let flow_analysis_info = self.visit_write(
                    lhs,
                    node,
                    rhs_analysis_result.type_.unwrap_type_view(),
                    rhs_analysis_result.flow_analysis_info,
                );
                // TODO(paulberry): null shorting
                ExpressionTypeAnalysisResult {
                    type_: SharedTypeView::new(rhs_analysis_result.type_.unwrap_type_view()),
                    flow_analysis_info,
                }
            }
            other => panic!("{} is not an expression", other.class_name()),
        }
    }

    /// `Statement.visit`.
    pub fn visit_statement(&mut self, node: Node) {
        use NodeKind::*;
        let location = loc(node);
        match node.kind() {
            Assert { condition, message } => {
                self.analyze_assert_statement(node, condition, message);
                self.ir_builder.apply(
                    "assert",
                    &[Kind::Expression, Kind::Expression],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            Block { statements } => {
                self.analyze_block(&statements);
                let kinds = vec![Kind::Statement; statements.len()];
                self.ir_builder
                    .apply("block", &kinds, Kind::Statement, &location, &[]);
            }
            Break { target } => {
                let target = match target {
                    None => self.current_break_target,
                    Some(label) => label.get_binding(),
                };
                self.flow.handle_break(target);
                self.ir_builder
                    .apply("break", &[], Kind::Statement, &location, &[]);
            }
            Continue { target } => {
                let target = match target {
                    None => self.current_continue_target,
                    Some(label) => label.get_binding(),
                };
                self.flow.handle_continue(target);
                self.ir_builder
                    .apply("continue", &[], Kind::Statement, &location, &[]);
            }
            Do { body, condition } => {
                self.analyze_do_loop(node, body, condition);
                self.ir_builder.apply(
                    "do",
                    &[Kind::Statement, Kind::Expression],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            ExpressionInTypeSchema { expr, type_schema } => {
                self.analyze_expression(expr, type_schema, false, false, false);
                self.ir_builder
                    .apply("stmt", &[Kind::Expression], Kind::Statement, &location, &[]);
            }
            ExpressionStatement { expr } => {
                self.analyze_expression_statement(expr);
                self.ir_builder
                    .apply("stmt", &[Kind::Expression], Kind::Statement, &location, &[]);
            }
            For {
                initializer,
                condition,
                updater,
                body,
                for_collection,
            } => {
                match initializer {
                    Some(initializer) => self.dispatch_statement(initializer),
                    None => self.handle_no_initializer(node),
                }
                self.flow.for_condition_begin(node);
                let condition_flow_analysis_info = match condition {
                    Some(condition) => {
                        let unknown = self.unknown();
                        self.analyze_expression(condition, unknown, false, false, false)
                            .flow_analysis_info
                    }
                    None => {
                        self.handle_no_condition(node);
                        self.flow.boolean_literal(true);
                        Some(())
                    }
                };
                self.flow.for_body_begin(
                    if for_collection { None } else { Some(node) },
                    condition_flow_analysis_info,
                );
                self.visit_loop_body(node, body);
                self.flow.for_updater_begin();
                match updater {
                    Some(updater) => {
                        let unknown = self.unknown();
                        self.analyze_expression(updater, unknown, false, false, false);
                    }
                    None => self.handle_no_condition(node),
                }
                self.flow.for_end();
                self.ir_builder.apply(
                    "for",
                    &[Kind::Statement, Kind::Expression, Kind::Statement, Kind::Expression],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            ForEach {
                variable,
                iterable,
                body,
                declares_variable,
            } => {
                let unknown = self.unknown();
                let iterable_type = self
                    .analyze_expression(iterable, unknown, false, false, false)
                    .type_
                    .unwrap_type_view();
                let iterated_type = self.get_iterated_type(iterable_type);
                self.flow.for_each_body_begin(node);
                if let Some(variable) = variable
                    && !declares_variable
                {
                    self.flow
                        .write(node, variable, SharedTypeView::new(iterated_type), None);
                }
                self.visit_loop_body(node, body);
                self.flow.for_each_end();
                self.ir_builder.apply(
                    "forEach",
                    &[Kind::Expression, Kind::Statement],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            If {
                condition,
                if_true,
                if_false,
            } => {
                self.analyze_if_statement(node, condition, if_true, if_false);
                self.ir_builder.apply(
                    "if",
                    &[Kind::Expression, Kind::Statement, Kind::Statement],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            IfCase {
                expression,
                pattern,
                guard,
                if_true,
                if_false,
                candidate_variables,
            } => {
                self.analyze_if_case_statement(
                    node,
                    expression,
                    pattern,
                    guard,
                    if_true,
                    if_false,
                    &candidate_variables,
                );
                self.ir_builder.apply(
                    "ifCase",
                    &[
                        Kind::Expression,
                        Kind::Pattern,
                        Kind::Variables,
                        Kind::Expression,
                        Kind::Statement,
                        Kind::Statement,
                    ],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            LabeledStatement { body, .. } => self.analyze_labeled_statement(node, body),
            PatternForIn {
                pattern,
                expression,
                body,
                has_await,
            } => {
                self.analyze_pattern_for_in(node, has_await, pattern, expression, &mut |a: &mut Self| {
                    a.dispatch_statement(body)
                });
                self.ir_builder.apply(
                    "forEach",
                    &[Kind::Expression, Kind::Pattern, Kind::Statement],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            PatternVariableDeclaration {
                pattern,
                initializer,
                is_final,
            } => {
                self.analyze_pattern_variable_declaration(node, pattern, initializer, is_final);
                let name = if is_final { "match_final" } else { "match" };
                self.ir_builder.apply(
                    name,
                    &[Kind::Expression, Kind::Pattern],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            Return => {
                self.flow.handle_return();
                self.ir_builder
                    .apply("return", &[], Kind::Statement, &location, &[]);
            }
            SwitchStatement {
                scrutinee,
                cases,
                is_legacy_exhaustive,
                expect_has_default,
                expect_is_exhaustive,
                expect_last_case_terminates,
                expect_requires_exhaustiveness_validation,
                expect_scrutinee_type,
            } => {
                let needs_legacy_exhaustive = !self.patterns_enabled;
                if !needs_legacy_exhaustive && is_legacy_exhaustive.is_some() {
                    panic!("isLegacyExhaustive should not be specified at {location}");
                } else if needs_legacy_exhaustive && is_legacy_exhaustive.is_none() {
                    panic!("isLegacyExhaustive should be specified at {location}");
                }
                let previous_break_target = self.current_break_target;
                self.current_break_target = Some(node);
                let previous_continue_target = self.current_continue_target;
                self.current_continue_target = Some(node);
                let analysis_result = self.analyze_switch_statement(node, scrutinee, cases.len());
                if let Some(expected) = expect_has_default {
                    assert_eq!(analysis_result.has_default, expected, "hasDefault at {location}");
                }
                if let Some(expected) = expect_is_exhaustive {
                    assert_eq!(analysis_result.is_exhaustive, expected, "isExhaustive at {location}");
                }
                if let Some(expected) = expect_last_case_terminates {
                    assert_eq!(
                        analysis_result.last_case_terminates, expected,
                        "lastCaseTerminates at {location}"
                    );
                }
                if let Some(expected) = expect_requires_exhaustiveness_validation {
                    assert_eq!(
                        analysis_result.requires_exhaustiveness_validation, expected,
                        "requiresExhaustivenessValidation at {location}"
                    );
                }
                if let Some(expected) = expect_scrutinee_type {
                    assert_eq!(
                        analysis_result.scrutinee_type.unwrap_type_view().type_string(),
                        expected,
                        "scrutineeType at {location}"
                    );
                }
                let mut kinds = vec![Kind::Expression];
                kinds.extend(std::iter::repeat_n(Kind::StatementCase, cases.len()));
                self.ir_builder
                    .apply("switch", &kinds, Kind::Statement, &location, &[]);
                self.current_break_target = previous_break_target;
                self.current_continue_target = previous_continue_target;
            }
            TryStatement {
                body,
                catches,
                finally_statement,
            } => {
                self.analyze_try_statement(node, body, &catches, finally_statement);
                let mut kinds = vec![Kind::Statement];
                kinds.extend(std::iter::repeat_n(Kind::Statement, catches.len()));
                kinds.push(Kind::Statement);
                self.ir_builder
                    .apply("try", &kinds, Kind::Statement, &location, &[]);
            }
            VariableDeclaration {
                variable,
                is_late,
                is_final,
                declared_type,
                initializer,
                expect_inferred_type,
            } => {
                let arg_kinds;
                let names: &[&str];
                self.ir_builder
                    .atom(variable.name(), Kind::Variable, &location);
                let static_type;
                match initializer {
                    None => {
                        // Use the shared logic for analyzing uninitialized
                        // variable declarations.
                        static_type = self
                            .analyze_uninitialized_variable_declaration(
                                node,
                                variable,
                                declared_type.map(SharedTypeView::new),
                                is_final,
                            )
                            .unwrap_type_view();
                        self.ir_builder
                            .atom(&static_type.type_string(), Kind::Type, &location);
                        arg_kinds = vec![Kind::Variable, Kind::Type];
                        names = &["staticType"];
                    }
                    Some(initializer) => {
                        // There's no shared logic for analyzing initialized
                        // variable declarations, so analyze the declaration
                        // directly.
                        if is_late {
                            self.flow.late_initializer_begin(node);
                        }
                        let schema = match declared_type {
                            Some(declared_type) => SharedTypeSchemaView::new(declared_type),
                            None => self.unknown(),
                        };
                        let initializer_analysis_result =
                            self.analyze_expression(initializer, schema, false, false, false);
                        let initializer_type = initializer_analysis_result.type_.unwrap_type_view();
                        if is_late {
                            self.flow.late_initializer_end();
                        }
                        static_type = match declared_type {
                            Some(declared_type) => declared_type,
                            None => self
                                .variable_type_from_initializer_type(SharedTypeView::new(initializer_type))
                                .unwrap_type_view(),
                        };
                        variable.set_type(static_type);
                        self.flow
                            .declare(variable, SharedTypeView::new(static_type), true);
                        self.flow.initialize(
                            variable,
                            SharedTypeView::new(initializer_type),
                            initializer_analysis_result.flow_analysis_info,
                            is_final,
                            is_late,
                            declared_type.is_none(),
                            false,
                        );
                        self.ir_builder
                            .atom(&initializer_type.type_string(), Kind::Type, &location);
                        self.ir_builder
                            .atom(&static_type.type_string(), Kind::Type, &location);
                        arg_kinds = vec![Kind::Variable, Kind::Expression, Kind::Type, Kind::Type];
                        names = &["initializerType", "staticType"];
                    }
                }
                // Finally, double check the inferred variable type, if
                // necessary for the test.
                if let Some(expect_inferred_type) = expect_inferred_type {
                    assert_eq!(static_type.type_string(), expect_inferred_type, "at {location}");
                }
                let mut ir_name = vec!["declare"];
                if is_late {
                    ir_name.push("late");
                }
                if is_final {
                    ir_name.push("final");
                }
                self.ir_builder.apply(
                    &ir_name.join("_"),
                    &arg_kinds,
                    Kind::Statement,
                    &location,
                    names,
                );
            }
            While { condition, body } => {
                self.analyze_while_loop(node, condition, body);
                self.ir_builder.apply(
                    "while",
                    &[Kind::Expression, Kind::Statement],
                    Kind::Statement,
                    &location,
                    &[],
                );
            }
            YieldStatement {
                operand,
                is_yield_star,
            } => {
                self.analyze_yield_statement(node, operand, is_yield_star);
                self.ir_builder
                    .apply("yieldStmt", &[Kind::Expression], Kind::Statement, &location, &[]);
            }
            other => panic!("{} is not a statement", other.class_name()),
        }
    }

    /// `CollectionElement.visit`.
    pub fn visit_collection_element(&mut self, node: Node, context: CollectionElementContext) {
        use NodeKind::*;
        let location = loc(node);
        match node.kind() {
            ExpressionCollectionElement { expression } => {
                let type_schema = match context {
                    CollectionElementContext::Type {
                        element_type_schema,
                    } => element_type_schema,
                    _ => self.unknown(),
                };
                self.analyze_expression(expression, type_schema, false, false, false);
                self.ir_builder.apply(
                    "celt",
                    &[Kind::Expression],
                    Kind::CollectionElement,
                    &location,
                    &[],
                );
            }
            IfCaseElement {
                expression,
                pattern,
                guard,
                if_true,
                if_false,
                variables,
            } => {
                self.analyze_if_case_element(
                    node, expression, pattern, &variables, guard, if_true, if_false, context,
                );
                self.ir_builder.apply(
                    "if",
                    &[
                        Kind::Expression,
                        Kind::Pattern,
                        Kind::Expression,
                        Kind::CollectionElement,
                        Kind::CollectionElement,
                    ],
                    Kind::CollectionElement,
                    &location,
                    &["expression", "pattern", "guard", "ifTrue", "ifFalse"],
                );
            }
            IfElement {
                condition,
                if_true,
                if_false,
            } => {
                self.analyze_if_element(node, condition, if_true, if_false, context);
                self.ir_builder.apply(
                    "if",
                    &[Kind::Expression, Kind::CollectionElement, Kind::CollectionElement],
                    Kind::CollectionElement,
                    &location,
                    &[],
                );
            }
            MapEntry {
                key,
                value,
                is_key_null_aware,
            } => {
                let (key_schema, value_schema) = match context {
                    CollectionElementContext::MapEntry {
                        key_type,
                        value_type,
                    } => (
                        SharedTypeSchemaView::new(key_type),
                        SharedTypeSchemaView::new(value_type),
                    ),
                    _ => (self.unknown(), self.unknown()),
                };
                let key_analysis_result = self.analyze_expression(key, key_schema, false, false, false);
                let key_type = key_analysis_result.type_;
                self.flow.null_aware_map_entry_value_begin(
                    key_analysis_result.flow_analysis_info,
                    key_type,
                    is_key_null_aware,
                );
                self.analyze_expression(value, value_schema, false, false, false);
                self.flow.null_aware_map_entry_end(is_key_null_aware);
                self.ir_builder.apply(
                    "mapEntry",
                    &[Kind::Expression, Kind::Expression],
                    Kind::CollectionElement,
                    &location,
                    &[],
                );
            }
            PatternForInElement {
                pattern,
                expression,
                body,
                has_await,
            } => {
                self.analyze_pattern_for_in(node, has_await, pattern, expression, &mut |a: &mut Self| {
                    a.dispatch_collection_element(body, context)
                });
                self.ir_builder.apply(
                    "forEach",
                    &[Kind::Expression, Kind::Pattern, Kind::CollectionElement],
                    Kind::CollectionElement,
                    &location,
                    &[],
                );
            }
            other => panic!("{} is not a collection element", other.class_name()),
        }
    }

    /// `Pattern.visit`.
    pub fn visit_pattern(&mut self, context: &SharedMatchContext, node: Node) -> PatternResult<Type> {
        use NodeKind::*;
        let location = loc(node);
        match node.kind() {
            CastPattern { inner, ty } => {
                let analysis_result = self.analyze_cast_pattern(context, node, inner, SharedTypeView::new(ty));
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder.atom(&ty.type_string(), Kind::Type, &location);
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    "castPattern",
                    &[Kind::Pattern, Kind::Type, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result
            }
            ConstantPattern { constant } => {
                let analysis_result = self.analyze_constant_pattern(context, node, constant);
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    "const",
                    &[Kind::Expression, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result.into()
            }
            ListPattern {
                element_type,
                elements,
            } => {
                let list_pattern_result =
                    self.analyze_list_pattern(context, node, element_type.map(SharedTypeView::new), &elements);
                let matched_type = list_pattern_result.matched_value_type.unwrap_type_view();
                let required_type = list_pattern_result.required_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder
                    .atom(&required_type.type_string(), Kind::Type, &location);
                let mut kinds = vec![Kind::Pattern; elements.len()];
                kinds.extend([Kind::Type, Kind::Type]);
                self.ir_builder.apply(
                    "listPattern",
                    &kinds,
                    Kind::Pattern,
                    &location,
                    &["matchedType", "requiredType"],
                );
                list_pattern_result.into()
            }
            LogicalAndPattern { lhs, rhs } => {
                let analysis_result = self.analyze_logical_and_pattern(context, node, lhs, rhs);
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    "logicalAndPattern",
                    &[Kind::Pattern, Kind::Pattern, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result
            }
            LogicalOrPattern { lhs, rhs } => {
                let analysis_result = self.analyze_logical_or_pattern(context, node, lhs, rhs);
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    "logicalOrPattern",
                    &[Kind::Pattern, Kind::Pattern, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result.into()
            }
            MapPattern {
                type_arguments,
                elements,
            } => {
                let type_arguments = type_arguments.map(|(k, v)| KeyValueTypes {
                    key_type: SharedTypeView::new(k),
                    value_type: SharedTypeView::new(v),
                });
                let map_pattern_result = self.analyze_map_pattern(context, node, type_arguments, &elements);
                let matched_type = map_pattern_result.matched_value_type.unwrap_type_view();
                let required_type = map_pattern_result.required_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder
                    .atom(&required_type.type_string(), Kind::Type, &location);
                let mut kinds = vec![Kind::MapPatternElement; elements.len()];
                kinds.extend([Kind::Type, Kind::Type]);
                self.ir_builder.apply(
                    "mapPattern",
                    &kinds,
                    Kind::Pattern,
                    &location,
                    &["matchedType", "requiredType"],
                );
                map_pattern_result.into()
            }
            NullCheckOrAssertPattern { inner, is_assert } => {
                let analysis_result = self.analyze_null_check_or_assert_pattern(context, node, inner, is_assert);
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    if is_assert {
                        "nullAssertPattern"
                    } else {
                        "nullCheckPattern"
                    },
                    &[Kind::Pattern, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result.into()
            }
            ObjectPattern { fields, .. } => {
                let fields_info = Self::record_pattern_fields(&fields);
                let object_pattern_result = self.analyze_object_pattern(context, node, &fields_info);
                let matched_type = object_pattern_result.matched_value_type.unwrap_type_view();
                let required_type = object_pattern_result.required_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder
                    .atom(&required_type.type_string(), Kind::Type, &location);
                let mut kinds = vec![Kind::Pattern; fields.len()];
                kinds.extend([Kind::Type, Kind::Type]);
                self.ir_builder.apply(
                    "objectPattern",
                    &kinds,
                    Kind::Pattern,
                    &location,
                    &["matchedType", "requiredType"],
                );
                object_pattern_result.into()
            }
            ParenthesizedPattern { inner } => self.visit_pattern(context, inner),
            RecordPattern { fields } => {
                let fields_info = Self::record_pattern_fields(&fields);
                let record_pattern_result = self.analyze_record_pattern(context, node, &fields_info);
                let matched_type = record_pattern_result.matched_value_type.unwrap_type_view();
                let required_type = record_pattern_result.required_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder
                    .atom(&required_type.type_string(), Kind::Type, &location);
                let mut kinds = vec![Kind::Pattern; fields.len()];
                kinds.extend([Kind::Type, Kind::Type]);
                self.ir_builder.apply(
                    "recordPattern",
                    &kinds,
                    Kind::Pattern,
                    &location,
                    &["matchedType", "requiredType"],
                );
                record_pattern_result.into()
            }
            RelationalPattern { operator, operand } => {
                let analysis_result = self.analyze_relational_pattern(context, node, operand);
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    &operator,
                    &[Kind::Expression, Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                analysis_result.into()
            }
            VariablePattern {
                declared_type,
                variable,
                expect_inferred_type,
                is_assigned_variable,
            } => {
                if is_assigned_variable.expect("isAssignedVariable (preVisit)") {
                    let analysis_result = self.analyze_assigned_variable_pattern(context, node, variable);
                    self.handle_assigned_variable_pattern(node, variable, expect_inferred_type);
                    analysis_result.into()
                } else {
                    let declared_variable_pattern_result = self.analyze_declared_variable_pattern(
                        context,
                        node,
                        variable,
                        variable.name(),
                        declared_type.map(SharedTypeView::new),
                    );
                    let matched_type = declared_variable_pattern_result
                        .matched_value_type
                        .unwrap_type_view();
                    let static_type = declared_variable_pattern_result.static_type.unwrap_type_view();
                    self.handle_declared_variable_pattern(
                        node,
                        variable,
                        expect_inferred_type,
                        matched_type,
                        static_type,
                    );
                    declared_variable_pattern_result.into()
                }
            }
            WildcardPattern {
                declared_type,
                expect_inferred_type,
            } => {
                let analysis_result =
                    self.analyze_wildcard_pattern(context, node, declared_type.map(SharedTypeView::new));
                let matched_type = analysis_result.matched_value_type.unwrap_type_view();
                self.ir_builder
                    .atom(&matched_type.type_string(), Kind::Type, &location);
                self.ir_builder.apply(
                    "wildcardPattern",
                    &[Kind::Type],
                    Kind::Pattern,
                    &location,
                    &["matchedType"],
                );
                if let Some(expect_inferred_type) = expect_inferred_type {
                    assert_eq!(matched_type.type_string(), expect_inferred_type, "at {location}");
                }
                analysis_result.into()
            }
            other => panic!("{} is not a pattern", other.class_name()),
        }
    }

    /// `Pattern.computeSchema`.
    pub fn compute_schema(&mut self, node: Node) -> SchemaView {
        use NodeKind::*;
        match node.kind() {
            CastPattern { .. } => self.analyze_cast_pattern_schema(),
            ConstantPattern { .. } => self.analyze_constant_pattern_schema(),
            ListPattern {
                element_type,
                elements,
            } => self.analyze_list_pattern_schema(element_type.map(SharedTypeView::new), &elements),
            LogicalAndPattern { lhs, rhs } => self.analyze_logical_and_pattern_schema(lhs, rhs),
            LogicalOrPattern { lhs, rhs } => self.analyze_logical_or_pattern_schema(lhs, rhs),
            MapPattern {
                type_arguments,
                elements,
            } => {
                let type_arguments = type_arguments.map(|(k, v)| KeyValueTypes {
                    key_type: SharedTypeView::new(k),
                    value_type: SharedTypeView::new(v),
                });
                self.analyze_map_pattern_schema(type_arguments, &elements)
            }
            NullCheckOrAssertPattern { inner, is_assert } => {
                self.analyze_null_check_or_assert_pattern_schema(inner, is_assert)
            }
            ObjectPattern { required_type, .. } => {
                self.analyze_object_pattern_schema(SharedTypeView::new(required_type))
            }
            ParenthesizedPattern { inner } => self.compute_schema(inner),
            RecordPattern { fields } => {
                let fields_info = Self::record_pattern_fields(&fields);
                self.analyze_record_pattern_schema(&fields_info)
            }
            RelationalPattern { .. } => self.analyze_relational_pattern_schema(),
            VariablePattern {
                declared_type,
                variable,
                is_assigned_variable,
                ..
            } => {
                if is_assigned_variable.expect("isAssignedVariable (preVisit)") {
                    self.analyze_assigned_variable_pattern_schema(variable)
                } else {
                    self.analyze_declared_variable_pattern_schema(declared_type.map(SharedTypeView::new))
                }
            }
            WildcardPattern { declared_type, .. } => {
                self.analyze_wildcard_pattern_schema(declared_type.map(SharedTypeView::new))
            }
            other => panic!("{} is not a pattern", other.class_name()),
        }
    }
}

/// Dart `double.toString()` for the integral values produced by
/// `int.toDouble()` (e.g. `1.0`).
fn dart_double_to_string(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

impl TypeAnalyzer for MiniAstTypeAnalyzer {
    type Node = Node;
    type Statement = Node;
    type Pattern = Node;
    type Error = ();
    type Errors = MiniAstErrors;
    type BodyContext = BodyContext;
    type CollectionElementContext = CollectionElementContext;

    fn body_context(&self) -> Option<&BodyContext> {
        self.body_context.as_ref()
    }

    fn errors(&mut self) -> &mut MiniAstErrors {
        &mut self.errors
    }

    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions {
        &self.type_analyzer_options
    }

    fn dot_shorthands(&mut self) -> &mut Vec<(Node, SchemaView)> {
        &mut self.dot_shorthands
    }

    fn dispatch_collection_element(&mut self, element: Node, context: CollectionElementContext) {
        let guard = self.ir_builder.guard_begin();
        self.visit_collection_element(element, context);
        self.ir_builder.guard_end(guard, &|| format!("{element:?}"));
        if let Some(expected_ir) = element.data().expected_ir {
            self.ir_builder
                .check(&expected_ir, Kind::CollectionElement, &loc(element));
        }
    }

    fn dispatch_expression(
        &mut self,
        expression: Node,
        schema: SchemaView,
        _is_void_allowed: bool,
        _needs_coercion: bool,
    ) -> ExprResult {
        let data = expression.data();
        if let Some(expected_schema) = &data.expected_schema {
            assert_eq!(
                &schema.unwrap_type_schema_view().type_string(),
                expected_schema,
                "schema at {}",
                loc_str(data.location)
            );
        }
        let guard = self.ir_builder.guard_begin();
        self.pending_detail = None;
        let result = self.visit_expression(expression, schema);
        let detail = self.pending_detail.take();
        self.ir_builder.guard_end(guard, &|| format!("{expression:?}"));
        if let Some(checker) = &data.check_expression_result {
            checker(&detail.unwrap_or(ExprResultDetail {
                result: result.clone(),
                operand_type: None,
                converted_to_double: None,
            }));
        }
        if let Some(expected_type) = &data.expected_type {
            assert_eq!(
                &result.type_.unwrap_type_view().type_string(),
                expected_type,
                "at {}",
                loc_str(data.location)
            );
        }
        if let Some(expected_ir) = &data.expected_ir {
            self.ir_builder
                .check(expected_ir, Kind::Expression, &loc_str(data.location));
        }
        result
    }

    fn dispatch_pattern(&mut self, context: &SharedMatchContext, pattern: Node) -> PatternResult<Type> {
        self.visit_pattern(context, pattern)
    }

    fn dispatch_pattern_schema(&mut self, pattern: Node) -> SchemaView {
        self.compute_schema(pattern)
    }

    fn dispatch_statement(&mut self, statement: Node) {
        let guard = self.ir_builder.guard_begin();
        self.visit_statement(statement);
        self.ir_builder.guard_end(guard, &|| format!("{statement:?}"));
        let data = statement.data();
        if let Some(checker) = &data.check_statement_result {
            checker();
        }
        if let Some(expected_ir) = &data.expected_ir {
            self.ir_builder
                .check(expected_ir, Kind::Statement, &loc_str(data.location));
        }
    }

    fn downward_infer_object_pattern_required_type(&mut self, matched_type: View, pattern: Node) -> View {
        let NodeKind::ObjectPattern { required_type, .. } = pattern.kind() else {
            panic!("{pattern:?} is not an object pattern");
        };
        let primary = required_type.as_primary_type().expect("primary type");
        if !primary.args.is_empty() {
            SharedTypeView::new(required_type)
        } else {
            SharedTypeView::new(
                self.operations
                    .downward_infer(primary.name(), matched_type.unwrap_type_view()),
            )
        }
    }

    fn finish_expression_case(&mut self, node: Node, _case_index: usize) {
        self.ir_builder.apply(
            "case",
            &[Kind::CaseHead, Kind::Expression],
            Kind::ExpressionCase,
            &loc(node),
            &[],
        );
    }

    fn finish_joined_pattern_variable(
        &mut self,
        variable: Var,
        _location: JoinedPatternVariableLocation,
        inconsistency: JoinedPatternVariableInconsistency,
        is_final: bool,
        type_: View,
    ) {
        variable.update(|d| d.is_final = is_final);
        variable.set_type(type_.unwrap_type_view());
        variable.update(|d| {
            let join = d.join.as_mut().expect("PatternVariableJoin");
            join.inconsistency = join.inconsistency.max_with(inconsistency);
        });
    }

    fn get_map_pattern_entry(&mut self, element: Node) -> Option<MapPatternEntry<Node, Node>> {
        match element.kind() {
            NodeKind::MapPatternEntry { key, value } => Some(MapPatternEntry { key, value }),
            _ => None,
        }
    }

    fn get_rest_pattern_element_pattern(&mut self, node: Node) -> Option<Node> {
        match node.kind() {
            NodeKind::RestPattern { sub_pattern } => sub_pattern,
            _ => None,
        }
    }

    fn get_switch_expression_member_info(
        &mut self,
        node: Node,
        index: usize,
    ) -> SwitchExpressionMemberInfo<Node, Node, Var, Name> {
        let NodeKind::SwitchExpression { cases, .. } = node.kind() else {
            panic!("{node:?} is not a switch expression");
        };
        let NodeKind::ExpressionCase {
            guarded_pattern,
            expression,
        } = cases[index].kind()
        else {
            panic!("not an expression case");
        };
        let head = match guarded_pattern.map(|g| g.kind()) {
            Some(NodeKind::GuardedPattern {
                pattern,
                guard,
                variables,
            }) => CaseHeadOrDefaultInfo {
                pattern: Some(pattern),
                variables: variables.unwrap_or_default(),
                guard,
            },
            _ => CaseHeadOrDefaultInfo {
                pattern: None,
                variables: Vec::new(),
                guard: None,
            },
        };
        SwitchExpressionMemberInfo { head, expression }
    }

    fn get_switch_statement_member_info(
        &mut self,
        node: Node,
        case_index: usize,
    ) -> SwitchStatementMemberInfo<Node, Node, Node, Var, Name> {
        let NodeKind::SwitchStatement { cases, .. } = node.kind() else {
            panic!("{node:?} is not a switch statement");
        };
        let NodeKind::SwitchStatementMember {
            elements,
            body,
            has_labels,
            candidate_variables,
        } = cases[case_index].kind()
        else {
            panic!("not a switch statement member");
        };
        let heads = elements
            .iter()
            .map(|element| match element.kind() {
                NodeKind::SwitchHeadCase { guarded_pattern } => {
                    let NodeKind::GuardedPattern {
                        pattern,
                        guard,
                        variables,
                    } = guarded_pattern.kind()
                    else {
                        unreachable!()
                    };
                    CaseHeadOrDefaultInfo {
                        pattern: Some(pattern),
                        variables: variables.expect("variables (preVisit)"),
                        guard,
                    }
                }
                _ => CaseHeadOrDefaultInfo {
                    pattern: None,
                    variables: Vec::new(),
                    guard: None,
                },
            })
            .collect();
        let NodeKind::Block { statements } = body.kind() else {
            unreachable!()
        };
        SwitchStatementMemberInfo {
            heads,
            has_labels,
            body: statements,
            variables: candidate_variables.expect("candidate variables (preVisit)"),
        }
    }

    fn handle_if_case_statement_after_pattern(&mut self, node: Node) {
        let NodeKind::IfCase {
            candidate_variables,
            ..
        } = node.kind()
        else {
            panic!("{node:?} is not an if-case statement");
        };
        let variables: Vec<Var> = candidate_variables.iter().map(|(_, v)| *v).collect();
        self.ir_variables(node, &variables);
    }

    fn handle_case_after_case_heads(&mut self, node: Node, case_index: usize, variables: &[Var]) {
        let NodeKind::SwitchStatement { cases, .. } = node.kind() else {
            panic!("{node:?} is not a switch statement");
        };
        let NodeKind::SwitchStatementMember { elements, .. } = cases[case_index].kind() else {
            unreachable!()
        };
        self.ir_variables(node, variables);
        let mut kinds = vec![Kind::CaseHead; elements.len()];
        kinds.push(Kind::Variables);
        self.ir_builder
            .apply("heads", &kinds, Kind::CaseHeads, &loc(node), &[]);
    }

    fn handle_case_head(&mut self, node: Node, case_index: usize, sub_index: usize) {
        let variables: Vec<Var> = match node.kind() {
            NodeKind::SwitchExpression { cases, .. } => {
                let NodeKind::ExpressionCase {
                    guarded_pattern, ..
                } = cases[case_index].kind()
                else {
                    unreachable!()
                };
                match guarded_pattern.map(|g| g.kind()) {
                    Some(NodeKind::GuardedPattern { variables, .. }) => variables
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(_, v)| v)
                        .collect(),
                    _ => Vec::new(),
                }
            }
            NodeKind::SwitchStatement { cases, .. } => {
                let NodeKind::SwitchStatementMember { elements, .. } = cases[case_index].kind()
                else {
                    unreachable!()
                };
                match elements[sub_index].kind() {
                    NodeKind::SwitchHeadCase { guarded_pattern } => {
                        let NodeKind::GuardedPattern { variables, .. } = guarded_pattern.kind()
                        else {
                            unreachable!()
                        };
                        variables
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(_, v)| v)
                            .collect()
                    }
                    _ => Vec::new(),
                }
            }
            other => panic!("({}) {node:?}", other.class_name()),
        };
        self.ir_variables(node, &variables);
        self.ir_builder.apply(
            "head",
            &[Kind::Pattern, Kind::Expression, Kind::Variables],
            Kind::CaseHead,
            &loc(node),
            &[],
        );
    }

    fn handle_default(&mut self, node: Node, _case_index: usize, _sub_index: usize) {
        self.ir_builder.atom("default", Kind::CaseHead, &loc(node));
    }

    fn handle_list_pattern_rest_element(&mut self, _container: Node, rest_element: Node) {
        let NodeKind::RestPattern { sub_pattern } = rest_element.kind() else {
            unreachable!()
        };
        if sub_pattern.is_some() {
            self.ir_builder
                .apply("...", &[Kind::Pattern], Kind::Pattern, &loc(rest_element), &[]);
        } else {
            self.ir_builder
                .atom("...", Kind::Pattern, &loc(rest_element));
        }
    }

    fn handle_map_pattern_entry(&mut self, _container: Node, entry_element: Node, _key_type: View) {
        self.ir_builder.apply(
            "mapPatternEntry",
            &[Kind::Expression, Kind::Pattern],
            Kind::MapPatternElement,
            &loc(entry_element),
            &[],
        );
    }

    fn handle_map_pattern_rest_element(&mut self, _container: Node, rest_element: Node) {
        let NodeKind::RestPattern { sub_pattern } = rest_element.kind() else {
            unreachable!()
        };
        if sub_pattern.is_some() {
            self.ir_builder.apply(
                "...",
                &[Kind::Pattern],
                Kind::MapPatternElement,
                &loc(rest_element),
                &[],
            );
        } else {
            self.ir_builder
                .atom("...", Kind::MapPatternElement, &loc(rest_element));
        }
    }

    fn handle_merged_statement_case(&mut self, node: Node, case_index: usize, is_terminating: bool) {
        let NodeKind::SwitchStatement { cases, .. } = node.kind() else {
            unreachable!()
        };
        let NodeKind::SwitchStatementMember { body, .. } = cases[case_index].kind() else {
            unreachable!()
        };
        let NodeKind::Block { statements } = body.kind() else {
            unreachable!()
        };
        let mut num_statements = statements.len();
        let location = loc(node);
        if !is_terminating {
            self.ir_builder
                .apply("synthetic-break", &[], Kind::Statement, &location, &[]);
            num_statements += 1;
        }
        let kinds = vec![Kind::Statement; num_statements];
        self.ir_builder
            .apply("block", &kinds, Kind::Statement, &location, &[]);
        self.ir_builder.apply(
            "case",
            &[Kind::CaseHeads, Kind::Statement],
            Kind::StatementCase,
            &location,
            &[],
        );
    }

    fn handle_no_collection_element(&mut self, node: Node) {
        self.ir_builder
            .atom("noop", Kind::CollectionElement, &loc(node));
    }

    fn handle_no_guard(&mut self, node: Node, _case_index: usize) {
        self.ir_builder.atom("true", Kind::Expression, &loc(node));
    }

    fn handle_no_statement(&mut self, node: Node) {
        self.ir_builder.atom("noop", Kind::Statement, &loc(node));
    }

    fn handle_switch_before_alternative(&mut self, _node: Node, _case_index: usize, _sub_index: usize) {}

    fn handle_switch_scrutinee(&mut self, _type_: View) {}

    fn is_dot_shorthand(&mut self, node: Node) -> bool {
        matches!(node.kind(), NodeKind::DotShorthand { .. })
    }

    fn is_legacy_switch_exhaustive(&mut self, node: Node, _expression_type: View) -> bool {
        match node.kind() {
            NodeKind::SwitchStatement {
                is_legacy_exhaustive,
                ..
            } => is_legacy_exhaustive.expect("isLegacyExhaustive"),
            _ => panic!("{node:?} is not a switch statement"),
        }
    }

    fn is_rest_pattern_element(&mut self, node: Node) -> bool {
        matches!(node.kind(), NodeKind::RestPattern { .. })
    }

    fn is_variable_pattern(&mut self, pattern: Node) -> bool {
        matches!(pattern.kind(), NodeKind::VariablePattern { .. })
    }

    fn resolve_object_pattern_property_get(
        &mut self,
        _object_pattern: Node,
        receiver_type: View,
        field: &RecordPatternFieldOf<Self>,
    ) -> (Option<PropertyElement>, View) {
        let property_member = self.get_member(
            receiver_type.unwrap_type_view(),
            field.name.expect("field name"),
        );
        let ty = property_member
            .as_ref()
            .map(|m| m.ty)
            .unwrap_or_else(|| self.operations.dynamic_type().unwrap_type_view());
        (property_member, SharedTypeView::new(ty))
    }

    fn resolve_relational_pattern_operator(
        &mut self,
        node: Node,
        matched_value_type: View,
    ) -> Option<RelationalOperatorResolution<Type>> {
        let NodeKind::RelationalPattern { operator, .. } = node.kind() else {
            panic!("{node:?} is not a relational pattern");
        };
        self.harness_resolve_relational_pattern_operator(matched_value_type.unwrap_type_view(), &operator)
    }

    fn set_variable_type(&mut self, variable: Var, type_: View) {
        variable.set_type(type_.unwrap_type_view());
    }

    fn variable_type_from_initializer_type(&mut self, type_: View) -> View {
        // Variables whose initializer has type `Null` receive the inferred
        // type `dynamic`.
        let mut type_ = type_;
        if self.operations.classify_type(type_) == TypeClassification::NullOrEquivalent {
            type_ = self.operations.dynamic_type();
        }
        // Variables whose initializer type includes a promoted type variable
        // receive the nearest supertype that could be expressed in Dart
        // source code (e.g. `T&int` is demoted to `T`).
        // TODO(paulberry): add language tests to verify that the behavior of
        // `type.recursivelyDemote` matches what the analyzer and CFE do.
        let unwrapped = type_.unwrap_type_view();
        SharedTypeView::new(unwrapped.recursively_demote(true).unwrap_or(unwrapped))
    }

    dartr_type_analyzer::type_analyzer_mixin!();
}

#[allow(dead_code)]
fn _unused(_: Label, _: SharedTypeKind) {}

#[allow(dead_code)]
fn _ops_bound<O: FlowAnalysisOperations + FlowAnalysisTypeOperations + SharedTypeOperations + TypeAnalyzerOperations>() {}

#[allow(dead_code)]
fn _errors_bound<E: VariableBinderErrors + TypeAnalyzerErrors>() {}
