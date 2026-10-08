// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (the
// `TypeAnalyzer` and `NullShortingMixin` members of ResolverVisitor:
// dispatchExpression, dispatchStatement, dispatchPattern, handle_*,
// finishNullShorting, setVariableType, ...)

//! The `TypeAnalyzer` mixin applied to [`ResolverVisitor`]: the shared type
//! analyzer (`dartr_type_analyzer`) runs over the resolver's nodes, types
//! and flow analysis.
//!
//! Associated types: nodes are [`NodeId`] / typed ids
//! (`Id<Expression>`, `Id<Statement>`, `Id<DartPattern>`), variables are
//! `EId<PromotableElement>`, types are [`TypeId`], the operations are
//! [`TypeSystemOperations`], the flow analysis is [`ResolverFlow`].

use dartr_ast::{DartPattern, Expression, Id, NodeId, Statement};
use dartr_element::{EId, Name, PromotableElement, TypeId};
use dartr_flow::null_shorting::{ExpressionResultOf, TypeAnalysisNullShortingInterface};
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analysis_result::PatternResult;
use dartr_flow::type_analyzer::{
    JoinedPatternVariableInconsistency, JoinedPatternVariableLocation, MapPatternEntry,
    MatchContextOf, PropertyMemberOf, RecordPatternFieldOf, RelationalOperatorResolution,
    SwitchExpressionMemberInfo, SwitchStatementMemberInfo, TypeAnalyzer, TypeAnalyzerOptions,
};
use dartr_typesystem::type_system_operations::TypeSystemOperations;

use crate::body_inference_context::BodyInferenceContext;
use crate::flow_analysis_visitor::ResolverFlow;
use crate::resolver::{CollectionLiteralContext, ExprResult, ResolverVisitor, SchemaOf, TypeViewOf};
use crate::shared_type_analyzer::SharedTypeAnalyzerErrors;

impl<'a> TypeAnalysisNullShortingInterface for ResolverVisitor<'a> {
    type Expression = Id<Expression>;
    type Variable = EId<PromotableElement>;
    type Operations = TypeSystemOperations<'a>;
    type Flow = ResolverFlow<'a>;
    type Guard = ();

    /// Dart `flow` (`flowAnalysis.flow!`).
    fn flow(&mut self) -> &mut ResolverFlow<'a> {
        self.flow_analysis
            .flow
            .as_mut()
            .expect("flow analysis is not active")
    }

    /// Dart `operations` (`flowAnalysis.typeOperations`).
    fn operations(&self) -> &TypeSystemOperations<'a> {
        &self.flow_analysis.type_operations
    }

    fn guards(&self) -> &[()] {
        &self.guards
    }

    fn guards_mut(&mut self) -> &mut Vec<()> {
        &mut self.guards
    }

    /// Dart `finishNullShorting`: the expression info stored for the
    /// null-aware expression was valid only where the target is not null.
    fn finish_null_shorting(
        &mut self,
        target_depth: usize,
        inner_result: ExpressionResultOf<Self>,
        whole_expression: Id<Expression>,
    ) -> ExpressionResultOf<Self> {
        let inferred_type = dartr_flow::flow_analysis_operations::FlowAnalysisTypeOperations::make_nullable(
            &self.flow_analysis.type_operations,
            inner_result.type_,
        );
        let mut inner_result = inner_result;
        loop {
            // End non-nullable promotion of the null-aware variable.
            dartr_flow::flow_analysis::FlowAnalysisNullShortingInterface::null_aware_access_end(
                self.flow(),
            );
            #[allow(clippy::let_unit_value)] // Dart: the guard is `Null`.
            let guard = self.guards.pop().expect("null shorting guard");
            inner_result = self.handle_null_shorting_step(inner_result, guard, inferred_type);
            if self.guards.len() <= target_depth {
                break;
            }
        }
        self.handle_null_shorting_finished(inferred_type);
        self.flow_analysis
            .store_expression_info(whole_expression, None);
        inner_result
    }

    /// Dart `handleNullShortingFinished`.
    fn handle_null_shorting_finished(&mut self, inferred_type: TypeViewOf) {
        let expression = self.peek_rewrite().expect("null-shorted expression");
        // Dart `recordNullShortedType`.
        self.record_static_type(expression, inferred_type.unwrap_type_view());
    }
}

impl<'a> TypeAnalyzer for ResolverVisitor<'a> {
    type Node = NodeId;
    type Statement = Id<Statement>;
    type Pattern = Id<DartPattern>;
    type Error = ();
    type Errors = SharedTypeAnalyzerErrors;
    type BodyContext = BodyInferenceContext;
    type CollectionElementContext = Option<CollectionLiteralContext>;

    fn body_context(&self) -> Option<&BodyInferenceContext> {
        self.body_context.as_ref()
    }

    fn errors(&mut self) -> &mut SharedTypeAnalyzerErrors {
        &mut self.errors
    }

    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions {
        &self.type_analyzer_options
    }

    fn dot_shorthands(&mut self) -> &mut Vec<(NodeId, SchemaOf)> {
        &mut self.dot_shorthands
    }

    /// Dart `dispatchCollectionElement`.
    fn dispatch_collection_element(
        &mut self,
        element: NodeId,
        context: Option<CollectionLiteralContext>,
    ) {
        self.resolve_collection_element_dispatch(Id::from_raw(element), context);
        self.pop_rewrite();
    }

    /// Dart `dispatchExpression`.
    fn dispatch_expression(
        &mut self,
        expression: Id<Expression>,
        context: SchemaOf,
        _is_void_allowed: bool,
        _needs_coercion: bool,
    ) -> ExprResult<'a> {
        let stack_depth = self.rewrite_stack_depth();
        // Stack: ()
        self.push_rewrite(Some(expression));
        // Stack: (Expression)
        self.resolve_expression_dispatch(expression, context.unwrap_type_schema_view());
        debug_assert_eq!(self.rewrite_stack_depth(), stack_depth + 1);
        let replacement = self.peek_rewrite().expect("replacement expression");
        let static_type = match self.static_type(replacement) {
            Some(t) => t,
            // Dart asserts that only extension overrides and identifiers of
            // types, extensions and prefixes have no type.
            None => TypeId::UNKNOWN,
        };
        let flow_analysis_info = self.flow_analysis.get_expression_info(Some(expression));
        ExprResult {
            type_: SharedTypeView::new(static_type),
            flow_analysis_info,
        }
    }

    /// Dart `dispatchPattern`.
    fn dispatch_pattern(
        &mut self,
        context: &MatchContextOf<Self>,
        node: NodeId,
    ) -> PatternResult<TypeId> {
        if self.ast.is::<DartPattern>(node) {
            let pattern: Id<DartPattern> = Id::from_raw(node);
            let result = self.resolve_pattern_dispatch(pattern, context);
            let mut info = self.tables.pattern_info.get(node).copied().unwrap_or_default();
            info.matched_value_type = Some(result.matched_value_type.unwrap_type_view());
            self.tables.pattern_info.insert(node, info);
            result
        } else {
            // This can occur inside conventional switch statements, since
            // `SwitchCase` points directly to an `Expression` rather than to
            // a `ConstantPattern`. So we mimic what
            // `ConstantPatternImpl.resolvePattern` would do.
            let result = self.analyze_constant_pattern(context, node, Id::from_raw(node));
            // Stack: (Expression)
            self.pop_rewrite();
            // Stack: ()
            result.into()
        }
    }

    /// Dart `dispatchPatternSchema`.
    fn dispatch_pattern_schema(&mut self, node: NodeId) -> SchemaOf {
        self.compute_pattern_schema_dispatch(Id::from_raw(node))
    }

    /// Dart `dispatchStatement`.
    fn dispatch_statement(&mut self, statement: Id<Statement>) {
        self.visit_node(statement.raw());
    }

    fn downward_infer_object_pattern_required_type(
        &mut self,
        matched_type: TypeViewOf,
        pattern: Id<DartPattern>,
    ) -> TypeViewOf {
        crate::pattern_resolver::downward_infer_object_pattern_required_type(
            self,
            matched_type,
            pattern,
        )
    }

    /// Dart `finishExpressionCase`.
    fn finish_expression_case(&mut self, node: Id<Expression>, case_index: usize) {
        crate::pattern_resolver::finish_expression_case(self, node, case_index);
    }

    fn finish_joined_pattern_variable(
        &mut self,
        variable: EId<PromotableElement>,
        location: JoinedPatternVariableLocation,
        inconsistency: JoinedPatternVariableInconsistency,
        is_final: bool,
        type_: TypeViewOf,
    ) {
        crate::pattern_resolver::finish_joined_pattern_variable(
            self,
            variable,
            location,
            inconsistency,
            is_final,
            type_,
        );
    }

    fn get_map_pattern_entry(
        &mut self,
        element: NodeId,
    ) -> Option<MapPatternEntry<Id<Expression>, Id<DartPattern>>> {
        let entry = self.ast.cast::<dartr_ast::MapPatternEntry>(element)?;
        Some(MapPatternEntry {
            key: self.ast[entry].key,
            value: self.ast[entry].value,
        })
    }

    fn get_rest_pattern_element_pattern(&mut self, node: NodeId) -> Option<Id<DartPattern>> {
        let rest = self.ast.cast::<dartr_ast::RestPatternElement>(node)?;
        self.ast[rest].pattern
    }

    fn get_switch_expression_member_info(
        &mut self,
        node: Id<Expression>,
        index: usize,
    ) -> SwitchExpressionMemberInfo<NodeId, Id<Expression>, EId<PromotableElement>, Name> {
        crate::pattern_resolver::get_switch_expression_member_info(self, node, index)
    }

    fn get_switch_statement_member_info(
        &mut self,
        node: Id<Statement>,
        case_index: usize,
    ) -> SwitchStatementMemberInfo<NodeId, Id<Statement>, Id<Expression>, EId<PromotableElement>, Name>
    {
        crate::pattern_resolver::get_switch_statement_member_info(self, node, case_index)
    }

    /// Dart `handle_ifElement_conditionEnd`.
    fn handle_if_element_condition_end(&mut self, _node: NodeId) {
        // Stack: (Expression condition)
        let condition = self.pop_rewrite().expect("condition");
        self.check_for_non_bool_condition(condition);
    }

    /// Dart `handle_ifStatement_conditionEnd`.
    fn handle_if_statement_condition_end(&mut self, _node: Id<Statement>) {
        // Stack: (Expression condition)
        let condition = self.pop_rewrite().expect("condition");
        self.check_for_non_bool_condition(condition);
    }

    /// Dart `handle_logicalOrPattern_afterLhs`.
    fn handle_logical_or_pattern_after_lhs(&mut self, node: Id<DartPattern>) {
        if let Some(p) = self.ast.cast::<dartr_ast::LogicalOrPattern>(node) {
            let right = self.ast[p].right_operand;
            self.check_unreachable_node(right);
        }
    }

    fn handle_case_after_case_heads(
        &mut self,
        _node: Id<Statement>,
        _case_index: usize,
        _variables: &[EId<PromotableElement>],
    ) {
    }

    /// Dart `handleCaseHead`.
    fn handle_case_head(&mut self, node: NodeId, case_index: usize, sub_index: usize) {
        // Stack: (Expression)
        self.pop_rewrite(); // "when" expression
        // Stack: ()
        crate::pattern_resolver::handle_case_head(self, node, case_index, sub_index);
    }

    /// Dart `handleDefault`.
    fn handle_default(&mut self, node: NodeId, case_index: usize, sub_index: usize) {
        crate::pattern_resolver::handle_default(self, node, case_index, sub_index);
    }

    fn handle_list_pattern_rest_element(&mut self, _container: Id<DartPattern>, _rest_element: NodeId) {}

    /// Dart `handleMapPatternEntry`.
    fn handle_map_pattern_entry(
        &mut self,
        _container: Id<DartPattern>,
        _entry_element: NodeId,
        _key_type: TypeViewOf,
    ) {
        // Dart `entry.key = popRewrite()!`: the rewrite already replaced the
        // key in the AST.
        self.pop_rewrite();
    }

    fn handle_map_pattern_rest_element(&mut self, _container: Id<DartPattern>, _rest_element: NodeId) {}

    fn handle_merged_statement_case(
        &mut self,
        _node: Id<Statement>,
        _case_index: usize,
        _is_terminating: bool,
    ) {
    }

    fn handle_no_collection_element(&mut self, _node: NodeId) {}

    /// Dart `handleNoGuard`.
    fn handle_no_guard(&mut self, _node: NodeId, _case_index: usize) {
        // Stack: ()
        // We can push `null` here because there is no actual expression
        // associated with the lack of a guard, so there's nothing that will
        // need rewriting.
        self.push_rewrite(None);
        // Stack: (Expression)
    }

    fn handle_no_statement(&mut self, _node: Id<Statement>) {}

    /// Dart `handleSwitchBeforeAlternative`.
    fn handle_switch_before_alternative(&mut self, node: NodeId, case_index: usize, sub_index: usize) {
        crate::pattern_resolver::handle_switch_before_alternative(self, node, case_index, sub_index);
    }

    /// Dart `handleSwitchScrutinee`.
    fn handle_switch_scrutinee(&mut self, type_: TypeViewOf) {
        crate::pattern_resolver::handle_switch_scrutinee(self, type_);
    }

    /// Dart `isDotShorthand`.
    fn is_dot_shorthand(&mut self, node: Id<Expression>) -> bool {
        self.rt.is_dot_shorthand(node.raw())
    }

    /// Dart `isLegacySwitchExhaustive`.
    fn is_legacy_switch_exhaustive(&mut self, node: NodeId, expression_type: TypeViewOf) -> bool {
        crate::pattern_resolver::is_legacy_switch_exhaustive(self, node, expression_type)
    }

    /// Dart `isRestPatternElement`.
    fn is_rest_pattern_element(&mut self, node: NodeId) -> bool {
        self.ast.is::<dartr_ast::RestPatternElement>(node)
    }

    /// Dart `isVariablePattern`.
    fn is_variable_pattern(&mut self, pattern: NodeId) -> bool {
        self.ast.is::<dartr_ast::DeclaredVariablePattern>(pattern)
    }

    fn resolve_object_pattern_property_get(
        &mut self,
        object_pattern: Id<DartPattern>,
        receiver_type: TypeViewOf,
        field: &RecordPatternFieldOf<Self>,
    ) -> (Option<PropertyMemberOf<Self>>, TypeViewOf) {
        crate::pattern_resolver::resolve_object_pattern_property_get(
            self,
            object_pattern,
            receiver_type,
            field,
        )
    }

    fn resolve_relational_pattern_operator(
        &mut self,
        node: Id<DartPattern>,
        matched_value_type: TypeViewOf,
    ) -> Option<RelationalOperatorResolution<TypeId>> {
        crate::pattern_resolver::resolve_relational_pattern_operator(self, node, matched_value_type)
    }

    /// Dart `setVariableType`.
    fn set_variable_type(&mut self, variable: EId<PromotableElement>, type_: TypeViewOf) {
        crate::variable_declaration_resolver::set_variable_type(self, variable, type_.unwrap_type_view());
    }

    /// Dart `variableTypeFromInitializerType`.
    fn variable_type_from_initializer_type(&mut self, type_: TypeViewOf) -> TypeViewOf {
        SharedTypeView::new(crate::variable_declaration_resolver::variable_type_from_initializer_type(
            self,
            type_.unwrap_type_view(),
        ))
    }

    dartr_type_analyzer::type_analyzer_mixin!();
}

/// Keeps the schema constructor in scope for the mixin.
#[allow(dead_code)]
fn _schema(t: TypeId) -> SchemaOf {
    SharedTypeSchemaView::new(t)
}
