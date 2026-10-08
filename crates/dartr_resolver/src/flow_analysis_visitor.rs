// Dart source: pkg/analyzer/lib/src/dart/resolver/flow_analysis_visitor.dart
// (FlowAnalysisHelper, _AssignedVariablesVisitor,
// _LocalVariableTypeProvider; TypeSystemOperations is in
// dartr_typesystem::type_system_operations)

//! The flow analysis glue of the resolver: [`FlowAnalysisHelper`] owns the
//! flow analysis of the current body or initializer
//! ([`FlowAnalysisImpl`] over [`ResolverFlowTypes`]) and the expression
//! infos; [`compute_assigned_variables`] is the assigned variables pre-pass.
//!
//! Not ported: `FlowAnalysisDataForTesting` (the test data of the
//! analyzer's flow analysis tests) and `transferTestData`.
//!
//! # `FlowTypes::is_expression`
//!
//! Flow analysis asks the client whether the node of a `write` is an
//! expression (`node is Expression` in Dart). [`FlowTypes::is_expression`]
//! is an associated function over a [`NodeId`] without access to the AST.
//! Flow analysis calls it only in `write`, so [`FlowAnalysisHelper::write`]
//! puts the answer for the node into a thread-local before it calls
//! `flow.write` and clears it after. Other ways need more changes for the
//! same result: a `Node` type that carries the kind changes every flow
//! analysis call of the resolver, and a reference to the AST in the
//! operations does not work because the resolver changes the AST while the
//! flow analysis lives.

use std::cell::Cell;
use std::marker::PhantomData;

use dartr_ast::{
    AnonymousMethodInvocation, AsExpression, AssignedVariablePattern, AssignmentExpression, Ast,
    AstVisitor, BinaryExpression, BreakStatement, CaseClause, CatchClause, CommentReference,
    CompilationUnit, ConditionalExpression, ConstructorDeclaration, ContinueStatement, DartPattern,
    DeclaredVariablePattern, DoStatement, Expression, FieldDeclaration, ForEachPartsWithDeclaration,
    ForEachPartsWithIdentifier, ForEachPartsWithPattern, ForElement, ForLoopParts,
    ForPartsWithDeclarations, ForPartsWithExpression, ForPartsWithPattern, ForStatement,
    FormalParameter, FunctionDeclaration, FunctionExpression, GuardedPattern, Id, IfElement,
    IfStatement, IsExpression, Label, LabelReference, LabeledStatement, LogicalOrPattern,
    MethodDeclaration, NodeId, NodeKind, NodeList, NodeMap, PatternVariableDeclaration,
    PostfixExpression, PrefixExpression, SimpleIdentifier, Statement, SwitchCase,
    SwitchExpression, SwitchPatternCase, SwitchStatement, TopLevelVariableDeclaration,
    TryStatement, VariableDeclaration, VariableDeclarationList, WhileStatement,
};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, ExecutableElement, FormalParameterElement, FragmentId,
    PatternVariableFragment, PromotableElement, ResolutionTables, Tag, TypeId,
};
use dartr_flow::assigned_variables::{AssignedVariables, AssignedVariablesImpl};
use dartr_syntax::TokenType;
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::flow_analysis_impl::FlowAnalysisImpl;
use dartr_flow::flow_analysis_impl::model::{ExpressionInfo, FlowTypes};
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzerOptions;
use dartr_typesystem::type_system_operations::TypeSystemOperations;

use crate::{ast_ext, element_ext};

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
        if self.flow.is_none() {
            return;
        }
        let expression = ast[node].expression;
        let type_annotation = ast[node].type_;
        let info = self.get_expression_info(Some(expression));
        let sub_expression_type = type_or_throw(tables, expression.raw());
        let cast_type = annotation_type(tables, type_annotation.raw());
        let flow = self.flow.as_mut().expect("flow");
        flow.as_expression_end(
            info,
            SharedTypeView::new(sub_expression_type),
            SharedTypeView::new(cast_type),
        );
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
        assert!(self.flow.is_none(), "flow analysis is already active");
        let ctx = self.type_operations.type_system.ctx;
        let assigned_variables =
            compute_assigned_variables(ast, tables, &ctx, node, parameters, visit);
        self.flow = Some(FlowAnalysisImpl::new(
            self.type_operations,
            assigned_variables,
            self.type_analyzer_options,
        ));
    }

    /// Dart `bodyOrInitializer_exit`.
    pub fn body_or_initializer_exit(&mut self) {
        // Set `self.flow` to `None` before doing any clean-up so that if a
        // panic happens, the state is already updated correctly.
        let flow = self.flow.take();
        flow.expect("flow analysis is not active").finish();
    }

    /// Dart `breakStatement`.
    pub fn break_statement(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<Statement>) {
        let label = ast
            .cast::<BreakStatement>(node)
            .and_then(|b| ast[b].label);
        let ctx = self.type_operations.type_system.ctx;
        let element = label.and_then(|l| label_reference_element(tables, l));
        let target = get_label_target(ast, tables, &ctx, node.raw(), element, true);
        if let Some(flow) = self.flow.as_mut() {
            flow.handle_break(target);
        }
    }

    /// Dart `continueStatement`.
    pub fn continue_statement(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<Statement>) {
        let label = ast
            .cast::<ContinueStatement>(node)
            .and_then(|c| ast[c].label);
        let ctx = self.type_operations.type_system.ctx;
        let element = label.and_then(|l| label_reference_element(tables, l));
        let target = get_label_target(ast, tables, &ctx, node.raw(), element, false);
        if let Some(flow) = self.flow.as_mut() {
            flow.handle_continue(target);
        }
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
        let ctx = self.type_operations.type_system.ctx;
        let flow = self.flow.as_mut().expect("flow analysis is not active");
        for &parameter in parameters {
            let ty = element_ext::variable_type(&ctx, parameter.raw());
            flow.declare(parameter.upcast(), SharedTypeView::new(ty), true);
        }
    }

    /// Dart `executableDeclaration_enter`.
    pub fn executable_declaration_enter(
        &mut self,
        node: NodeId,
        parameters: Option<&[EId<FormalParameterElement>]>,
        is_closure: bool,
    ) {
        let ctx = self.type_operations.type_system.ctx;
        let flow = self.flow.as_mut().expect("flow analysis is not active");
        if is_closure {
            flow.function_expression_begin(node);
        }
        if let Some(parameters) = parameters {
            for &parameter in parameters {
                let ty = element_ext::variable_type(&ctx, parameter.raw());
                flow.declare(parameter.upcast(), SharedTypeView::new(ty), true);
            }
        }
    }

    /// Dart `executableDeclaration_exit`. Dart records the bodies that do
    /// not complete for testing only; this port does not keep test data.
    pub fn executable_declaration_exit(&mut self, body: NodeId, is_closure: bool) {
        let _ = body;
        let flow = self.flow.as_mut().expect("flow analysis is not active");
        if is_closure {
            flow.function_expression_end();
        }
    }

    /// Dart `for_bodyBegin`.
    pub fn for_body_begin(&mut self, ast: &Ast, node: NodeId, condition: Option<Id<Expression>>) {
        if self.flow.is_none() {
            return;
        }
        let statement = ast.cast::<Statement>(node);
        let condition_info = match condition {
            None => self.flow.as_mut().map(|f| f.boolean_literal(true)),
            Some(condition) => self.get_expression_info(Some(condition)),
        };
        let flow = self.flow.as_mut().expect("flow");
        flow.for_body_begin(statement, condition_info);
    }

    /// Dart `for_conditionBegin`.
    pub fn for_condition_begin(&mut self, node: NodeId) {
        if let Some(flow) = self.flow.as_mut() {
            flow.for_condition_begin(node);
        }
    }

    /// Dart `isDefinitelyAssigned`. Dart records the node for testing only;
    /// so this port takes only the element.
    pub fn is_definitely_assigned(&mut self, element: EId<PromotableElement>) -> bool {
        let flow = self.flow.as_ref().expect("flow analysis is not active");
        flow.is_assigned(element)
    }

    /// Dart `isDefinitelyUnassigned`.
    pub fn is_definitely_unassigned(&mut self, element: EId<PromotableElement>) -> bool {
        let flow = self.flow.as_ref().expect("flow analysis is not active");
        flow.is_unassigned(element)
    }

    /// Dart `isExpression`.
    pub fn is_expression(&mut self, ast: &Ast, tables: &ResolutionTables, node: Id<IsExpression>) {
        if self.flow.is_none() {
            return;
        }
        let expression = ast[node].expression;
        let type_annotation = ast[node].type_;
        let info = self.get_expression_info(Some(expression));
        let sub_expression_type = type_or_throw(tables, expression.raw());
        let checked_type = annotation_type(tables, type_annotation.raw());
        let flow = self.flow.as_mut().expect("flow");
        let result = flow.is_expression_end(
            info,
            ast[node].not_operator.is_some(),
            SharedTypeView::new(sub_expression_type),
            SharedTypeView::new(checked_type),
        );
        self.store_expression_info(node.upcast(), result);
    }

    /// Dart `labeledStatement_enter`.
    pub fn labeled_statement_enter(&mut self, node: Id<LabeledStatement>) {
        if let Some(flow) = self.flow.as_mut() {
            flow.labeled_statement_begin(node.upcast());
        }
    }

    /// Dart `labeledStatement_exit`.
    pub fn labeled_statement_exit(&mut self, node: Id<LabeledStatement>) {
        let _ = node;
        if let Some(flow) = self.flow.as_mut() {
            flow.labeled_statement_end();
        }
    }

    /// Dart `variableDeclarationList`: declares the variables of [node].
    pub fn variable_declaration_list(
        &mut self,
        ast: &Ast,
        tables: &ResolutionTables,
        ctx: &dartr_element::Ctx<'_>,
        node: Id<VariableDeclarationList>,
    ) {
        let Some(flow) = self.flow.as_mut() else {
            return;
        };
        for &variable in ast.list(ast[node].variables) {
            let Some(element) = declared_element(ctx, tables, variable.raw())
                .and_then(|e| e.cast::<PromotableElement>())
            else {
                continue;
            };
            let ty = element_ext::variable_type(ctx, element.raw());
            flow.declare(
                element,
                SharedTypeView::new(ty),
                ast[variable].initializer.is_some(),
            );
        }
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
        if let (Some(promotable), Some(flow)) =
            (variable.cast::<PromotableElement>(), self.flow.as_mut())
        {
            let promoted_type = if is_read {
                let (promoted_type, expression_info) = flow.variable_read(promotable);
                self.store_expression_info(node, Some(expression_info));
                promoted_type
            } else {
                flow.promoted_type(promotable)
            };
            if let Some(promoted_type) = promoted_type {
                return promoted_type.unwrap_type_view();
            }
        }
        element_ext::variable_type(ctx, variable)
    }
}

/// Dart `expression.typeOrThrow`. A missing type is a bug of the resolver;
/// this port answers `InvalidType` then, so that one missing type does not
/// stop the analysis of the unit.
fn type_or_throw(tables: &ResolutionTables, expression: NodeId) -> TypeId {
    let ty = tables.static_type.get(expression).copied();
    debug_assert!(ty.is_some(), "no static type for {expression:?}");
    ty.unwrap_or(TypeId::INVALID)
}

/// Dart `typeAnnotation.typeOrThrow`; see [`type_or_throw`].
fn annotation_type(tables: &ResolutionTables, type_annotation: NodeId) -> TypeId {
    let ty = tables.annotation_type.get(type_annotation).copied();
    debug_assert!(ty.is_some(), "no type for {type_annotation:?}");
    ty.unwrap_or(TypeId::INVALID)
}

/// Dart `node.declaredFragment?.element`.
fn declared_element(ctx: &Ctx<'_>, tables: &ResolutionTables, node: NodeId) -> Option<ElementId> {
    let fragment = *tables.declared_fragment.get(node)?;
    fragment_element(ctx, fragment)
}

/// Dart `fragment.element`.
fn fragment_element(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<ElementId> {
    ctx.fragment_data(fragment)?.element.try_get().copied()
}

/// Dart `node.element` of an identifier when it is a
/// `PromotableElementImpl`.
fn promotable_element(tables: &ResolutionTables, node: NodeId) -> Option<EId<PromotableElement>> {
    match tables.element.get(node)? {
        ElemRef::Base(e) => e.cast::<PromotableElement>(),
        ElemRef::Member(_) => None,
    }
}

/// Dart `labelReference.element`.
fn label_reference_element(tables: &ResolutionTables, label: Id<LabelReference>) -> Option<ElementId> {
    match tables.element.get(label)? {
        ElemRef::Base(e) => Some(*e),
        ElemRef::Member(_) => None,
    }
}

/// Dart `FlowAnalysisHelper.computeAssignedVariables`: the assigned
/// variables pre-pass over [node] (or over the nodes in [visit]).
///
/// The pass reads resolution data of the resolution visitor:
/// `tables.element` of the `SimpleIdentifier`s and
/// `AssignedVariablePattern`s that refer to promotable elements,
/// `tables.declared_fragment` of the local declarations, and the `join`
/// links of the pattern variable fragments.
pub fn compute_assigned_variables(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    node: NodeId,
    parameters: Option<&[EId<FormalParameterElement>]>,
    visit: Option<&[NodeId]>,
) -> ResolverAssignedVariables {
    let mut visitor = AssignedVariablesVisitor {
        ctx,
        tables,
        assigned_variables: ResolverAssignedVariables::new(),
    };
    visitor.declare_parameters(parameters);
    match visit {
        Some(nodes) => {
            for &n in nodes {
                ast.accept(n, &mut visitor);
            }
        }
        None => ast.visit_children(node, &mut visitor),
    }
    let mut assigned_variables = visitor.assigned_variables;
    assigned_variables.finish();
    assigned_variables
}

/// Dart `FlowAnalysisHelper.getLabelTarget`: the target statement of a
/// `break` / `continue` with the label element [element] (or the default
/// target when there is no label). [node] is the `break` or `continue`
/// statement (the search starts there and goes up the parents).
pub fn get_label_target(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    node: NodeId,
    element: Option<ElementId>,
    is_break: bool,
) -> Option<Id<Statement>> {
    let mut current = Some(node);
    while let Some(node) = current {
        match element {
            None => match ast.kind(node) {
                NodeKind::DoStatement | NodeKind::ForStatement | NodeKind::WhileStatement => {
                    return ast.cast::<Statement>(node);
                }
                NodeKind::SwitchStatement if is_break => return ast.cast::<Statement>(node),
                _ => {}
            },
            Some(element) => {
                if let Some(labeled) = ast.cast::<LabeledStatement>(node) {
                    if has_label(ast, tables, ctx, ast[labeled].labels, element) {
                        let statement = ast[labeled].statement;
                        // The inner statement is returned for labeled loops
                        // and switch statements, while the LabeledStatement
                        // is returned for the other known targets.
                        return match ast.kind(statement) {
                            NodeKind::ForStatement
                            | NodeKind::SwitchStatement
                            | NodeKind::WhileStatement
                            | NodeKind::DoStatement => Some(statement),
                            _ => Some(labeled.upcast()),
                        };
                    }
                }
                if let Some(switch) = ast.cast::<SwitchStatement>(node) {
                    for &member in ast.list(ast[switch].members) {
                        let labels = ast_ext::switch_member_labels(ast, member);
                        if has_label(ast, tables, ctx, labels, element) {
                            return Some(switch.upcast());
                        }
                    }
                }
            }
        }
        current = ast.parent(node);
    }
    None
}

/// Dart `FlowAnalysisHelper._hasLabel`.
fn has_label(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    labels: NodeList<Label>,
    element: ElementId,
) -> bool {
    ast.list(labels).iter().any(|&label| {
        // Dart: identical
        declared_element(ctx, tables, label.raw()) == Some(element)
    })
}

// ============================================================ pattern variables

/// The variables that the resolution visitor computes for a pattern (Dart
/// `ResolutionVisitor._computePatternVariables`, a run of the shared
/// `VariableBinder`), recomputed from the resolution data: the bind
/// variables of the `DeclaredVariablePattern`s (`declared_fragment`), and
/// for a logical-or pattern the join variables that the resolution visitor
/// links to their components (`PatternVariableFragmentImpl.join`).
///
/// In name order of the binder (first seen first). A duplicate name keeps
/// the first variable (Dart `VariableBinder.add`).
pub fn pattern_variables(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    pattern: Id<DartPattern>,
) -> Vec<(String, ElementId)> {
    let mut out = Vec::new();
    collect_pattern_variables(ast, tables, ctx, pattern.raw(), &mut out);
    out
}

fn add_pattern_variable(out: &mut Vec<(String, ElementId)>, name: String, variable: ElementId) {
    if !out.iter().any(|(n, _)| *n == name) {
        out.push((name, variable));
    }
}

fn collect_pattern_variables(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    node: NodeId,
    out: &mut Vec<(String, ElementId)>,
) {
    if let Some(declared) = ast.cast::<DeclaredVariablePattern>(node) {
        let name = ast.tokens.lexeme(ast[declared].name).to_string();
        if let Some(variable) = declared_element(ctx, tables, node) {
            add_pattern_variable(out, name, variable);
        }
    } else if let Some(logical_or) = ast.cast::<LogicalOrPattern>(node) {
        // Dart `VariableBinder.logicalOrPatternFinish`.
        let mut left = Vec::new();
        collect_pattern_variables(ast, tables, ctx, ast[logical_or].left_operand.raw(), &mut left);
        let mut right = Vec::new();
        collect_pattern_variables(ast, tables, ctx, ast[logical_or].right_operand.raw(), &mut right);
        for (name, variable) in left {
            right.retain(|(n, _)| *n != name);
            add_pattern_variable(out, name, join_or_self(ctx, variable));
        }
        for (name, variable) in right {
            add_pattern_variable(out, name, join_or_self(ctx, variable));
        }
    } else {
        for child in ast.children(node) {
            collect_pattern_variables(ast, tables, ctx, child, out);
        }
    }
}

/// The join variable of the pattern variable [variable] (Dart
/// `PatternVariableElementImpl.join`), or [variable] when it has none.
fn join_or_self(ctx: &Ctx<'_>, variable: ElementId) -> ElementId {
    let join = (|| {
        let first_fragment = ctx.element_data(variable)?.first_fragment;
        let fragment = first_fragment.cast::<PatternVariableFragment>()?;
        let join = ctx.fragment(fragment).pattern.join.get()?;
        fragment_element(ctx, join.raw())
    })();
    join.unwrap_or(variable)
}

/// Dart `_computeDeclaredPatternVariables`: the bind variables of
/// [pattern_variables] (Dart `PatternVariableDeclarationImpl.elements`,
/// `ForEachPartsWithPatternImpl.variables`).
pub fn declared_pattern_variables(
    ast: &Ast,
    tables: &ResolutionTables,
    ctx: &Ctx<'_>,
    pattern: Id<DartPattern>,
) -> Vec<ElementId> {
    pattern_variables(ast, tables, ctx, pattern)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| v.tag() == Tag::BindPatternVariable)
        .collect()
}

/// Dart `SwitchStatementCaseGroup.variables` (Dart
/// `VariableBinder.switchStatementSharedCaseScopeFinish`): for each name
/// in the cases of the group, the variable of the case when only one case
/// declares it in a group without other members, else the join variable
/// of the group that the resolution visitor links to the case variables.
pub fn switch_case_group_variables(
    ctx: &Ctx<'_>,
    case_variables: &[Vec<(String, ElementId)>],
) -> Vec<(String, ElementId)> {
    let mut out: Vec<(String, ElementId)> = Vec::new();
    for variables in case_variables {
        for (name, variable) in variables {
            if !out.iter().any(|(n, _)| n == name) {
                out.push((name.clone(), join_or_self(ctx, *variable)));
            }
        }
    }
    out
}

// ============================================================ the visitor

/// Dart `_AssignedVariablesVisitor`: gathers the local variables that are
/// potentially assigned in loops, `switch`, `try`, closures, ...
struct AssignedVariablesVisitor<'v, 'c> {
    ctx: &'v Ctx<'c>,
    tables: &'v ResolutionTables,
    assigned_variables: ResolverAssignedVariables,
}

impl AssignedVariablesVisitor<'_, '_> {
    fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        declared_element(self.ctx, self.tables, node)
    }

    fn declare(&mut self, variable: ElementId, ignore_duplicates: bool) {
        if let Some(variable) = variable.cast::<PromotableElement>() {
            self.assigned_variables.declare(variable, ignore_duplicates);
        }
    }

    fn write_identifier(&mut self, node: NodeId) {
        if let Some(element) = promotable_element(self.tables, node) {
            self.assigned_variables.write(element);
        }
    }

    /// Dart `_declareParameters`.
    fn declare_parameters(&mut self, parameters: Option<&[EId<FormalParameterElement>]>) {
        for &parameter in parameters.unwrap_or_default() {
            self.assigned_variables.declare(parameter.upcast(), false);
        }
    }

    /// The formal parameters of the executable element declared by [node]
    /// (Dart `node.declaredFragment!.element.formalParameters`).
    fn declare_parameters_of(&mut self, node: NodeId) {
        let Some(element) = self
            .declared_element(node)
            .and_then(|e| e.cast::<ExecutableElement>())
        else {
            return;
        };
        let parameters = self.ctx.executable(element).formal_params.clone();
        self.declare_parameters(Some(&parameters));
    }

    fn guarded_pattern_variables(
        &self,
        ast: &Ast,
        guarded_pattern: Id<GuardedPattern>,
    ) -> Vec<(String, ElementId)> {
        pattern_variables(ast, self.tables, self.ctx, ast[guarded_pattern].pattern)
    }

    /// Dart `_handleFor`.
    fn handle_for(&mut self, ast: &Ast, node: NodeId, for_loop_parts: Id<ForLoopParts>, body: NodeId) {
        let parts: NodeId = for_loop_parts.raw();
        let (condition, updaters) = if let Some(p) = ast.cast::<ForPartsWithExpression>(parts) {
            if let Some(initialization) = ast[p].initialization {
                ast.accept(initialization, self);
            }
            (ast[p].condition, ast[p].updaters)
        } else if let Some(p) = ast.cast::<ForPartsWithDeclarations>(parts) {
            ast.accept(ast[p].variables, self);
            (ast[p].condition, ast[p].updaters)
        } else if let Some(p) = ast.cast::<ForPartsWithPattern>(parts) {
            ast.accept(ast[p].variables, self);
            (ast[p].condition, ast[p].updaters)
        } else {
            let iterable;
            if let Some(p) = ast.cast::<ForEachPartsWithIdentifier>(parts) {
                iterable = ast[p].iterable;
                ast.accept(iterable, self);
                self.write_identifier(ast[p].identifier.raw());
            } else if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(parts) {
                iterable = ast[p].iterable;
                ast.accept(iterable, self);
                if let Some(variable) = self.declared_element(ast[p].loop_variable.raw()) {
                    self.declare(variable, false);
                }
            } else if let Some(p) = ast.cast::<ForEachPartsWithPattern>(parts) {
                iterable = ast[p].iterable;
                ast.accept(iterable, self);
                for variable in declared_pattern_variables(ast, self.tables, self.ctx, ast[p].pattern) {
                    self.declare(variable, false);
                }
            } else {
                panic!("Unrecognized for loop parts: {:?}", ast.kind(parts));
            }
            self.assigned_variables.begin_node();
            ast.accept(body, self);
            self.assigned_variables.end_node(node, false);
            return;
        };
        self.assigned_variables.begin_node();
        if let Some(condition) = condition {
            ast.accept(condition, self);
        }
        ast.accept(body, self);
        for &updater in ast.list(updaters) {
            ast.accept(updater, self);
        }
        self.assigned_variables.end_node(node, false);
    }

    /// Dart `_visitIf`.
    fn visit_if(
        &mut self,
        ast: &Ast,
        node: NodeId,
        expression: Id<Expression>,
        case_clause: Option<Id<CaseClause>>,
        if_true: NodeId,
        if_false: Option<NodeId>,
    ) {
        ast.accept(expression, self);
        if let Some(case_clause) = case_clause {
            let guarded_pattern = ast[case_clause].guarded_pattern;
            self.assigned_variables.begin_node();
            for (_, variable) in self.guarded_pattern_variables(ast, guarded_pattern) {
                self.declare(variable, false);
            }
            if let Some(when_clause) = ast[guarded_pattern].when_clause {
                ast.accept(when_clause, self);
            }
        } else {
            self.assigned_variables.begin_node();
        }
        ast.accept(if_true, self);
        self.assigned_variables.end_node(node, false);
        if let Some(if_false) = if_false {
            ast.accept(if_false, self);
        }
    }
}

impl AstVisitor for AssignedVariablesVisitor<'_, '_> {
    fn visit_anonymous_method_invocation(&mut self, ast: &Ast, node: Id<AnonymousMethodInvocation>) {
        if let Some(target) = ast[node].target {
            ast.accept(target, self);
        }
        if let Some(parameters) = ast[node].parameters {
            for &parameter in ast.list(ast[parameters].parameters) {
                if let Some(element) = self.declared_element(parameter.raw()) {
                    if element.is::<FormalParameterElement>() {
                        self.declare(element, false);
                    }
                }
            }
            ast.accept(parameters, self);
        }
        ast.accept(ast[node].body, self);
    }

    fn visit_assigned_variable_pattern(&mut self, _ast: &Ast, node: Id<AssignedVariablePattern>) {
        self.write_identifier(node.raw());
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        ast.visit_children(node, self);
        let left = ast[node].left_hand_side;
        if ast.is::<SimpleIdentifier>(left) {
            self.write_identifier(left.raw());
        }
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        if ast.tokens.ty(ast[node].operator) == TokenType::AMPERSAND_AMPERSAND {
            ast.accept(ast[node].left_operand, self);
            self.assigned_variables.begin_node();
            ast.accept(ast[node].right_operand, self);
            self.assigned_variables.end_node(node.raw(), false);
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        let parameters = [ast[node].exception_parameter, ast[node].stack_trace_parameter];
        for parameter in parameters.into_iter().flatten() {
            if let Some(element) = self.declared_element(parameter.raw()) {
                self.declare(element, false);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_conditional_expression(&mut self, ast: &Ast, node: Id<ConditionalExpression>) {
        ast.accept(ast[node].condition, self);
        self.assigned_variables.begin_node();
        ast.accept(ast[node].then_expression, self);
        self.assigned_variables.end_node(node.raw(), false);
        ast.accept(ast[node].else_expression, self);
    }

    fn visit_constructor_declaration(&mut self, _ast: &Ast, _node: Id<ConstructorDeclaration>) {
        panic!("Should not visit top level declarations");
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        self.assigned_variables.begin_node();
        ast.visit_children(node, self);
        self.assigned_variables.end_node(node.raw(), false);
    }

    fn visit_for_element(&mut self, ast: &Ast, node: Id<ForElement>) {
        self.handle_for(ast, node.raw(), ast[node].for_loop_parts, ast[node].body.raw());
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        self.handle_for(ast, node.raw(), ast[node].for_loop_parts, ast[node].body.raw());
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if ast.parent(node).is_some_and(|p| ast.is::<CompilationUnit>(p)) {
            panic!("Should not visit top level declarations");
        }
        self.assigned_variables.begin_node();
        self.declare_parameters_of(node.raw());
        ast.visit_children(node, self);
        self.assigned_variables.end_node(node.raw(), true);
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        if ast.parent(node).is_some_and(|p| ast.is::<FunctionDeclaration>(p)) {
            // A FunctionExpression just inside a FunctionDeclaration is an
            // analyzer artifact: it is not a separate closure. So skip the
            // usual processing.
            ast.visit_children(node, self);
            return;
        }
        self.assigned_variables.begin_node();
        self.declare_parameters_of(node.raw());
        ast.visit_children(node, self);
        self.assigned_variables.end_node(node.raw(), true);
    }

    fn visit_if_element(&mut self, ast: &Ast, node: Id<IfElement>) {
        let n = &ast[node];
        self.visit_if(
            ast,
            node.raw(),
            n.expression,
            n.case_clause,
            n.then_element.raw(),
            n.else_element.map(|e| e.raw()),
        );
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        let n = &ast[node];
        self.visit_if(
            ast,
            node.raw(),
            n.expression,
            n.case_clause,
            n.then_statement.raw(),
            n.else_statement.map(|e| e.raw()),
        );
    }

    fn visit_method_declaration(&mut self, _ast: &Ast, _node: Id<MethodDeclaration>) {
        panic!("Should not visit top level declarations");
    }

    fn visit_pattern_variable_declaration(&mut self, ast: &Ast, node: Id<PatternVariableDeclaration>) {
        for variable in declared_pattern_variables(ast, self.tables, self.ctx, ast[node].pattern) {
            self.declare(variable, false);
        }
        ast.visit_children(node, self);
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        ast.visit_children(node, self);
        if ast_ext::is_increment_operator(ast.tokens.ty(ast[node].operator)) {
            let operand = ast[node].operand;
            if ast.is::<SimpleIdentifier>(operand) {
                self.write_identifier(operand.raw());
            }
        }
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        ast.visit_children(node, self);
        if ast_ext::is_increment_operator(ast.tokens.ty(ast[node].operator)) {
            let operand = ast[node].operand;
            if ast.is::<SimpleIdentifier>(operand) {
                self.write_identifier(operand.raw());
            }
        }
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let Some(element) = promotable_element(self.tables, node.raw()) else {
            return;
        };
        let Some(parent) = ast.parent(node) else {
            return;
        };
        if ast_ext::simple_identifier_in_getter_context(ast, node)
            && !ast.is::<FormalParameter>(parent)
            && !ast.is::<CatchClause>(parent)
            && !ast.is::<CommentReference>(parent)
        {
            self.assigned_variables.read(element);
        }
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        ast.accept(ast[node].expression, self);
        for &case in ast.list(ast[node].cases) {
            let guarded_pattern = ast[case].guarded_pattern;
            for (_, variable) in self.guarded_pattern_variables(ast, guarded_pattern) {
                self.declare(variable, false);
            }
            ast.accept(case, self);
        }
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        ast.accept(ast[node].expression, self);

        self.assigned_variables.begin_node();
        for group in ast_ext::switch_statement_member_groups(ast, node) {
            let mut case_variables = Vec::new();
            for &member in &group.members {
                if let Some(case) = ast.cast::<SwitchCase>(member) {
                    ast.accept(ast[case].expression, self);
                } else if let Some(case) = ast.cast::<SwitchPatternCase>(member) {
                    let guarded_pattern = ast[case].guarded_pattern;
                    ast.accept(ast[guarded_pattern].pattern, self);
                    let variables = self.guarded_pattern_variables(ast, guarded_pattern);
                    for (_, variable) in &variables {
                        self.declare(*variable, false);
                    }
                    case_variables.push(variables);
                    if let Some(when_clause) = ast[guarded_pattern].when_clause {
                        ast.accept(when_clause, self);
                    }
                }
            }
            for (_, variable) in switch_case_group_variables(self.ctx, &case_variables) {
                // `ignore_duplicates: true` because this variable might be
                // the same as one of the variables declared earlier under a
                // specific switch case.
                self.declare(variable, true);
            }
            for &statement in ast.list(group.statements(ast)) {
                ast.accept(statement, self);
            }
        }
        self.assigned_variables.end_node(node.raw(), false);
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        let body = ast[node].body;
        self.assigned_variables.begin_node(); // Begin info for [node].
        self.assigned_variables.begin_node(); // Begin info for [node.body].
        ast.accept(body, self);
        self.assigned_variables.end_node(body.raw(), false);

        for &catch_clause in ast.list(ast[node].catch_clauses) {
            ast.accept(catch_clause, self);
        }
        self.assigned_variables.end_node(node.raw(), false);

        if let Some(finally_block) = ast[node].finally_block {
            ast.accept(finally_block, self);
        }
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let grand_parent = ast.parent(node).and_then(|p| ast.parent(p));
        if grand_parent.is_some_and(|g| {
            ast.is::<TopLevelVariableDeclaration>(g) || ast.is::<FieldDeclaration>(g)
        }) {
            panic!("Should not visit top level declarations");
        }
        let declared_element = self.declared_element(node.raw());
        if let Some(element) = declared_element {
            self.declare(element, false);
        }
        let is_late = declared_element.is_some_and(|e| element_ext::is_late(self.ctx, e));
        if is_late && ast[node].initializer.is_some() {
            self.assigned_variables.begin_node();
            ast.visit_children(node, self);
            self.assigned_variables.end_node(node.raw(), true);
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        self.assigned_variables.begin_node();
        ast.visit_children(node, self);
        self.assigned_variables.end_node(node.raw(), false);
    }
}
