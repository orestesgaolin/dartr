// Dart source: pkg/analyzer/lib/src/dart/resolver/flow_analysis_visitor.dart
// (FlowAnalysisHelper, _AssignedVariablesVisitor,
// _LocalVariableTypeProvider; TypeSystemOperations is in
// dartr_typesystem::type_system_operations)

//! The flow analysis glue of the resolver: [`FlowAnalysisHelper`] owns the
//! flow analysis of the current body or initializer
//! ([`FlowAnalysisImpl`] over [`ResolverFlowTypes`]) and the expression
//! infos; [`compute_assigned_variables`] is the assigned variables pre-pass.
//!
//! STUB (unit C2, flow glue): the public API is fixed; bodies marked
//! `todo!()` are filled by the flow glue port.

use std::cell::Cell;
use std::marker::PhantomData;

use dartr_ast::{
    Ast, AsExpression, Expression, Id, IsExpression, LabeledStatement, NodeId, NodeMap, Statement,
    VariableDeclarationList,
};
use dartr_element::{EId, FormalParameterElement, PromotableElement, ResolutionTables, TypeId};
use dartr_flow::assigned_variables::AssignedVariablesImpl;
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::flow_analysis_impl::FlowAnalysisImpl;
use dartr_flow::flow_analysis_impl::model::{ExpressionInfo, FlowTypes};
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzerOptions;
use dartr_typesystem::type_system_operations::TypeSystemOperations;

/// The types of the analyzer's flow analysis (Dart `FlowAnalysis<AstNodeImpl,
/// StatementImpl, ExpressionImpl, PromotableElementImpl>`).
pub struct ResolverFlowTypes<'a>(PhantomData<&'a ()>);

thread_local! {
    /// The node of the current [`FlowAnalysisHelper::write`] call when it is
    /// an expression. [`FlowTypes::is_expression`] has no access to the AST
    /// (it is a static method over node ids), and flow analysis asks it only
    /// for the node of a write; so the helper records the answer before it
    /// calls `write`. Writes from the shared type analyzer (patterns) never
    /// pass an expression, and get `false`.
    static WRITE_EXPRESSION: Cell<Option<NodeId>> = const { Cell::new(None) };
}

impl<'a> FlowTypes for ResolverFlowTypes<'a> {
    type Ops = TypeSystemOperations<'a>;
    type Node = NodeId;
    type Statement = Id<Statement>;
    type Expression = Id<Expression>;

    fn statement_to_node(statement: Id<Statement>) -> NodeId {
        statement.raw()
    }

    fn is_expression(node: NodeId) -> bool {
        WRITE_EXPRESSION.with(|c| c.get() == Some(node))
    }
}

/// The flow analysis of the resolver.
pub type ResolverFlow<'a> = FlowAnalysisImpl<ResolverFlowTypes<'a>>;

/// Flow analysis expression info of the resolver.
pub type ResolverExpressionInfo<'a> = ExpressionInfo<ResolverFlowTypes<'a>>;

/// The assigned variables of the resolver.
pub type ResolverAssignedVariables = AssignedVariablesImpl<NodeId, EId<PromotableElement>>;

/// Dart `FlowAnalysisHelper`.
pub struct FlowAnalysisHelper<'a> {
    /// The reused operations for creating new flow analysis instances.
    pub type_operations: TypeSystemOperations<'a>,
    pub type_analyzer_options: TypeAnalyzerOptions,
    /// The current flow, when resolving a function body, or `None`
    /// otherwise.
    pub flow: Option<ResolverFlow<'a>>,
    /// The mapping from expressions to their expression infos (Dart
    /// `_expressionInfoMap`).
    expression_info: NodeMap<Option<ResolverExpressionInfo<'a>>>,
}

impl<'a> FlowAnalysisHelper<'a> {
    pub fn new(
        type_operations: TypeSystemOperations<'a>,
        type_analyzer_options: TypeAnalyzerOptions,
    ) -> Self {
        FlowAnalysisHelper {
            type_operations,
            type_analyzer_options,
            flow: None,
            expression_info: NodeMap::new(),
        }
    }

    /// Dart `isActive`.
    pub fn is_active(&self) -> bool {
        self.flow.is_some()
    }

    /// Dart `getExpressionInfo`.
    pub fn get_expression_info(
        &self,
        expression: Option<Id<Expression>>,
    ) -> Option<ResolverExpressionInfo<'a>> {
        expression.and_then(|e| self.expression_info.get(e).cloned().flatten())
    }

    /// Dart `storeExpressionInfo`.
    pub fn store_expression_info(
        &mut self,
        expression: Id<Expression>,
        info: Option<ResolverExpressionInfo<'a>>,
    ) {
        self.expression_info.insert(expression, info);
    }

    /// Dart `flow.write(node, variable, writtenType, writtenExpressionInfo)`,
    /// with [`FlowTypes::is_expression`] answered for [node].
    pub fn write(
        &mut self,
        ast: &Ast,
        node: NodeId,
        variable: EId<PromotableElement>,
        written_type: TypeId,
        written_expression_info: Option<ResolverExpressionInfo<'a>>,
    ) -> Option<ResolverExpressionInfo<'a>> {
        let is_expression = crate::generated::dispatch::is_expression_kind(ast.kind(node));
        let flow = self.flow.as_mut()?;
        WRITE_EXPRESSION.with(|c| c.set(if is_expression { Some(node) } else { None }));
        let info = flow.write(
            node,
            variable,
            SharedTypeView::new(written_type),
            written_expression_info,
        );
        WRITE_EXPRESSION.with(|c| c.set(None));
        info
    }

    /// Dart `asExpression`.
    pub fn as_expression(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<AsExpression>) {
        let _ = (ast, tables, node);
        todo!("FlowAnalysisHelper.asExpression (flow glue)")
    }

    /// Dart `bodyOrInitializer_enter`: computes the assigned variables of
    /// [node] (or of the nodes that [visit] lists, Dart `visit` callback) and
    /// creates the flow analysis.
    pub fn body_or_initializer_enter(
        &mut self,
        ast: &Ast,
        tables: &ResolutionTables,
        node: NodeId,
        parameters: Option<&[EId<FormalParameterElement>]>,
        visit: Option<&[NodeId]>,
    ) {
        let _ = (ast, tables, node, parameters, visit);
        todo!("FlowAnalysisHelper.bodyOrInitializer_enter (flow glue)")
    }

    /// Dart `bodyOrInitializer_exit`.
    pub fn body_or_initializer_exit(&mut self) {
        todo!("FlowAnalysisHelper.bodyOrInitializer_exit (flow glue)")
    }

    /// Dart `breakStatement`.
    pub fn break_statement(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<Statement>) {
        let _ = (ast, tables, node);
        todo!("FlowAnalysisHelper.breakStatement (flow glue)")
    }

    /// Dart `continueStatement`.
    pub fn continue_statement(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<Statement>) {
        let _ = (ast, tables, node);
        todo!("FlowAnalysisHelper.continueStatement (flow glue)")
    }

    /// Dart `checkUnreachableNode` (testing data only; no effect here).
    pub fn check_unreachable_node(&mut self, node: NodeId) {
        let _ = node;
    }

    /// Dart `declarePrimaryConstructorParameters`.
    pub fn declare_primary_constructor_parameters(
        &mut self,
        parameters: &[EId<FormalParameterElement>],
    ) {
        let _ = parameters;
        todo!("FlowAnalysisHelper.declarePrimaryConstructorParameters (flow glue)")
    }

    /// Dart `executableDeclaration_enter`.
    pub fn executable_declaration_enter(
        &mut self,
        node: NodeId,
        parameters: Option<&[EId<FormalParameterElement>]>,
        is_closure: bool,
    ) {
        let _ = (node, parameters, is_closure);
        todo!("FlowAnalysisHelper.executableDeclaration_enter (flow glue)")
    }

    /// Dart `executableDeclaration_exit`.
    pub fn executable_declaration_exit(&mut self, body: NodeId, is_closure: bool) {
        let _ = (body, is_closure);
        todo!("FlowAnalysisHelper.executableDeclaration_exit (flow glue)")
    }

    /// Dart `for_bodyBegin`.
    pub fn for_body_begin(&mut self, ast: &Ast, node: NodeId, condition: Option<Id<Expression>>) {
        let _ = (ast, node, condition);
        todo!("FlowAnalysisHelper.for_bodyBegin (flow glue)")
    }

    /// Dart `for_conditionBegin`.
    pub fn for_condition_begin(&mut self, node: NodeId) {
        let _ = node;
        todo!("FlowAnalysisHelper.for_conditionBegin (flow glue)")
    }

    /// Dart `isDefinitelyAssigned`.
    pub fn is_definitely_assigned(&mut self, element: EId<PromotableElement>) -> bool {
        let _ = element;
        todo!("FlowAnalysisHelper.isDefinitelyAssigned (flow glue)")
    }

    /// Dart `isDefinitelyUnassigned`.
    pub fn is_definitely_unassigned(&mut self, element: EId<PromotableElement>) -> bool {
        let _ = element;
        todo!("FlowAnalysisHelper.isDefinitelyUnassigned (flow glue)")
    }

    /// Dart `isExpression`.
    pub fn is_expression(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<IsExpression>) {
        let _ = (ast, tables, node);
        todo!("FlowAnalysisHelper.isExpression (flow glue)")
    }

    /// Dart `labeledStatement_enter`.
    pub fn labeled_statement_enter(&mut self, node: Id<LabeledStatement>) {
        let _ = node;
        todo!("FlowAnalysisHelper.labeledStatement_enter (flow glue)")
    }

    /// Dart `labeledStatement_exit`.
    pub fn labeled_statement_exit(&mut self, node: Id<LabeledStatement>) {
        let _ = node;
        todo!("FlowAnalysisHelper.labeledStatement_exit (flow glue)")
    }

    /// Dart `variableDeclarationList`: declares the variables of [node].
    pub fn variable_declaration_list(
        &mut self,
        ast: &Ast,
        tables: &ResolutionTables,
        ctx: &dartr_element::Ctx<'_>,
        node: Id<VariableDeclarationList>,
    ) {
        let _ = (ast, tables, ctx, node);
        todo!("FlowAnalysisHelper.variableDeclarationList (flow glue)")
    }

    /// Dart `_LocalVariableTypeProvider.getType`: the promoted or declared
    /// type of the variable that [node] (a `SimpleIdentifier`) refers to;
    /// when [is_read], records the read in flow analysis and stores the
    /// expression info of [node].
    pub fn local_variable_type(
        &mut self,
        ctx: &dartr_element::Ctx<'a>,
        node: Id<Expression>,
        variable: dartr_element::ElementId,
        is_read: bool,
    ) -> TypeId {
        let _ = (ctx, node, variable, is_read);
        todo!("_LocalVariableTypeProvider.getType (flow glue)")
    }
}

/// Dart `FlowAnalysisHelper.computeAssignedVariables`: the assigned
/// variables pre-pass over [node] (or over the nodes in [visit]).
pub fn compute_assigned_variables(
    ast: &Ast,
    tables: &ResolutionTables,
    node: NodeId,
    parameters: Option<&[EId<FormalParameterElement>]>,
    visit: Option<&[NodeId]>,
) -> ResolverAssignedVariables {
    let _ = (ast, tables, node, parameters, visit);
    todo!("FlowAnalysisHelper.computeAssignedVariables (flow glue)")
}

/// Dart `FlowAnalysisHelper.getLabelTarget`: the target statement of a
/// `break` / `continue` with the label element [element] (or the default
/// target when there is no label).
pub fn get_label_target(
    ast: &Ast,
    tables: &ResolutionTables,
    node: NodeId,
    element: Option<dartr_element::ElementId>,
    is_break: bool,
) -> Option<Id<Statement>> {
    let _ = (ast, tables, node, element, is_break);
    todo!("FlowAnalysisHelper.getLabelTarget (flow glue)")
}

/// Unused: keeps the trait in scope for the helper bodies.
#[allow(dead_code)]
fn _uses_flow_analysis<F: FlowAnalysis>(_: &F) {}
