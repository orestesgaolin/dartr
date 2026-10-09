// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (the
// `visitX` methods of the statements and of the function bodies,
// _finishFunctionBodyInference)

//! The statement and function body visitors of [`ResolverVisitor`].

use dartr_ast::*;
use dartr_element::{EId, PromotableElement, TypeId, TypeKind};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::type_analyzer::TypeAnalyzer;

use crate::body_inference_context::BodyInferenceContext;
use crate::error::dead_code_verifier;
use crate::resolver::ResolverVisitor;
use crate::{for_resolver, pattern_resolver, yield_statement_resolver};

impl<'a> ResolverVisitor<'a> {
    pub fn visit_assert_statement(&mut self, node: Id<AssertStatement>) {
        self.check_unreachable_node(node);
        self.resolve_assert(self.ast[node].condition, self.ast[node].message);
    }

    pub fn visit_assert_initializer(&mut self, node: Id<AssertInitializer>) {
        self.resolve_assert(self.ast[node].condition, self.ast[node].message);
    }

    /// The common part of Dart `visitAssertStatement` and
    /// `visitAssertInitializer`.
    fn resolve_assert(&mut self, condition: Id<Expression>, message: Option<Id<Expression>>) {
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.assert_begin();
        }
        let bool_type = self.ctx.tp.bool_type();
        let condition = self.resolve_expression(condition, bool_type);
        self.check_for_non_bool_expression(condition);
        let info = self.flow_analysis.get_expression_info(Some(condition));
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.assert_after_condition(info);
        }
        if let Some(message) = message {
            self.resolve_expression(message, TypeId::UNKNOWN);
        }
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.assert_end();
        }
    }

    pub fn visit_block(&mut self, node: Id<Block>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_break_statement(&mut self, node: Id<BreakStatement>) {
        // We do not visit the label because it needs to be visited in the
        // context of the statement.
        self.check_unreachable_node(node);
        let ast = &*self.ast;
        self.flow_analysis
            .break_statement(ast, self.tables, node.upcast());
    }

    pub fn visit_continue_statement(&mut self, node: Id<ContinueStatement>) {
        // We do not visit the label because it needs to be visited in the
        // context of the statement.
        self.check_unreachable_node(node);
        let ast = &*self.ast;
        self.flow_analysis
            .continue_statement(ast, self.tables, node.upcast());
    }

    pub fn visit_do_statement(&mut self, node: Id<DoStatement>) {
        self.check_unreachable_node(node);
        let condition = self.ast[node].condition;
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.do_statement_body_begin(node.upcast());
        }
        let body = self.ast[node].body;
        self.visit_node(body.raw());
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.do_statement_condition_begin();
        }
        let bool_type = self.ctx.tp.bool_type();
        let condition = self.resolve_expression(condition, bool_type);
        self.check_for_non_bool_condition(condition);
        let info = self.flow_analysis.get_expression_info(Some(condition));
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.do_statement_end(info);
        }
    }

    pub fn visit_empty_statement(&mut self, node: Id<EmptyStatement>) {
        self.check_unreachable_node(node);
    }

    pub fn visit_expression_statement(&mut self, node: Id<ExpressionStatement>) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        self.resolve_expression(expression, TypeId::UNKNOWN);
    }

    pub fn visit_for_statement(&mut self, node: Id<ForStatement>) {
        self.check_unreachable_node(node);
        for_resolver::visit_for_statement(self, node);
    }

    pub fn visit_function_declaration_statement(&mut self, node: Id<FunctionDeclarationStatement>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_if_statement(&mut self, node: Id<IfStatement>) {
        self.check_unreachable_node(node);
        if self.ast[node].case_clause.is_some() {
            pattern_resolver::visit_if_case_statement(self, node);
            return;
        }
        let condition = self.ast[node].expression;
        let then_statement = self.ast[node].then_statement;
        let else_statement = self.ast[node].else_statement;
        self.analyze_if_statement(node.upcast(), condition, then_statement, else_statement);
    }

    pub fn visit_labeled_statement(&mut self, node: Id<LabeledStatement>) {
        self.flow_analysis.labeled_statement_enter(node);
        self.check_unreachable_node(node);
        self.visit_children(node);
        self.flow_analysis.labeled_statement_exit(node);
    }

    pub fn visit_pattern_variable_declaration_statement(
        &mut self,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        self.check_unreachable_node(node);
        let declaration = self.ast[node].declaration;
        self.visit_node(declaration.raw());
    }

    pub fn visit_return_statement(&mut self, node: Id<ReturnStatement>) {
        self.check_unreachable_node(node);
        let mut expression = self.ast[node].expression;
        if let Some(e) = expression {
            let context = self
                .body_context
                .as_ref()
                .and_then(|b| b.context_type)
                .unwrap_or(TypeId::UNKNOWN);
            // Pick up the expression again in case it was rewritten.
            expression = Some(self.resolve_expression(e, context));
        }
        let expression_type = expression.map(|e| self.type_or_throw(e));
        let ts = self.type_system;
        if let Some(body_context) = self.body_context.as_mut() {
            body_context.add_return_expression(&ts, expression_type);
        }
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.handle_return();
        }
    }

    pub fn visit_switch_statement(&mut self, node: Id<SwitchStatement>) {
        self.check_unreachable_node(node);
        pattern_resolver::visit_switch_statement(self, node);
    }

    pub fn visit_try_statement(&mut self, node: Id<TryStatement>) {
        self.check_unreachable_node(node);
        let body = self.ast[node].body;
        let catch_clauses = self.ast.list(self.ast[node].catch_clauses).to_vec();
        let finally_block = self.ast[node].finally_block;

        if finally_block.is_some() {
            self.flow().try_finally_statement_body_begin();
        }
        if !catch_clauses.is_empty() {
            self.flow().try_catch_statement_body_begin();
        }
        self.visit_node(body.raw());
        dead_code_verifier::flow_end(self, body);
        dead_code_verifier::try_statement_enter(self, node);
        if !catch_clauses.is_empty() {
            self.flow().try_catch_statement_body_end(body.raw());
            for catch_clause in catch_clauses {
                dead_code_verifier::verify_catch_clause(self, catch_clause);
                let exception =
                    self.catch_parameter_element(self.ast[catch_clause].exception_parameter);
                let stack_trace =
                    self.catch_parameter_element(self.ast[catch_clause].stack_trace_parameter);
                self.flow()
                    .try_catch_statement_catch_begin(exception, stack_trace);
                self.visit_node(catch_clause.raw());
                self.flow().try_catch_statement_catch_end();
                let catch_body = self.ast[catch_clause].body;
                dead_code_verifier::flow_end(self, catch_body);
            }
            self.flow().try_catch_statement_end();
        }
        dead_code_verifier::try_statement_exit(self, node);
        if let Some(finally_block) = finally_block {
            let target = if !self.ast.list(self.ast[node].catch_clauses).is_empty() {
                node.raw()
            } else {
                body.raw()
            };
            self.flow().try_finally_statement_finally_begin(target);
            self.visit_node(finally_block.raw());
            self.flow().try_finally_statement_end();
        }
    }

    /// The element of a catch clause parameter (Dart
    /// `parameter?.declaredFragment?.element as PromotableElementImpl?`).
    fn catch_parameter_element(
        &self,
        parameter: Option<Id<CatchClauseParameter>>,
    ) -> Option<EId<PromotableElement>> {
        let fragment = *self.tables.declared_fragment.get(parameter?)?;
        let element = *self.ctx.fragment_data(fragment)?.element.try_get()?;
        element.cast::<PromotableElement>()
    }

    pub fn visit_catch_clause(&mut self, node: Id<CatchClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_catch_clause_parameter(&mut self, node: Id<CatchClauseParameter>) {
        self.visit_children(node);
    }

    pub fn visit_variable_declaration_statement(&mut self, node: Id<VariableDeclarationStatement>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_while_statement(&mut self, node: Id<WhileStatement>) {
        self.check_unreachable_node(node);
        let condition = self.ast[node].condition;
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.while_statement_condition_begin(node.raw());
        }
        let bool_type = self.ctx.tp.bool_type();
        let condition = self.resolve_expression(condition, bool_type);
        self.check_for_non_bool_condition(condition);
        let info = self.flow_analysis.get_expression_info(Some(condition));
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.while_statement_body_begin(node.upcast(), info);
        }
        let body = self.ast[node].body;
        self.visit_node(body.raw());
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.while_statement_end();
        }
        dead_code_verifier::flow_end(self, body);
    }

    pub fn visit_yield_statement(&mut self, node: Id<YieldStatement>) {
        self.check_unreachable_node(node);
        yield_statement_resolver::visit_yield_statement(self, node);
    }

    // ------------------------------------------------------------ bodies

    /// Dart `FunctionBodyImpl.resolve(resolver, imposedType)`: resolves the
    /// body [node] and returns its inferred return type.
    pub fn resolve_function_body(
        &mut self,
        node: Id<FunctionBody>,
        imposed_type: Option<TypeId>,
    ) -> TypeId {
        let raw = node.raw();
        match self.ast.kind(raw) {
            NodeKind::BlockFunctionBody => {
                self.resolve_block_function_body(Id::from_raw(raw), imposed_type)
            }
            NodeKind::ExpressionFunctionBody => {
                self.resolve_expression_function_body(Id::from_raw(raw), imposed_type)
            }
            NodeKind::EmptyFunctionBody => {
                self.check_unreachable_node(raw);
                imposed_type.unwrap_or(TypeId::DYNAMIC)
            }
            NodeKind::NativeFunctionBody => {
                self.check_unreachable_node(raw);
                self.visit_children(raw);
                imposed_type.unwrap_or(TypeId::DYNAMIC)
            }
            kind => unreachable!("not a function body: {kind:?}"),
        }
    }

    /// Dart `FunctionBody.isAsynchronous` / `isGenerator` of [node].
    pub fn function_body_modifiers(&self, node: NodeId) -> (bool, bool) {
        let (keyword, star) = if let Some(b) = self.ast.cast::<BlockFunctionBody>(node) {
            (self.ast[b].keyword, self.ast[b].star)
        } else if let Some(b) = self.ast.cast::<ExpressionFunctionBody>(node) {
            (self.ast[b].keyword, self.ast[b].star)
        } else {
            (None, None)
        };
        let is_async = keyword.is_some_and(|k| self.lexeme(k) == "async");
        (is_async, star.is_some())
    }

    /// Dart `visitBlockFunctionBody(node, imposedType:)`.
    fn resolve_block_function_body(
        &mut self,
        node: Id<BlockFunctionBody>,
        imposed_type: Option<TypeId>,
    ) -> TypeId {
        let (is_async, is_generator) = self.function_body_modifiers(node.raw());
        let body_context =
            BodyInferenceContext::new(&self.type_system, is_async, is_generator, imposed_type);
        let old_body_context = self.body_context.replace(body_context);
        self.check_unreachable_node(node);
        self.visit_children(node);
        let result = self.finish_function_body_inference();
        self.body_context = old_body_context;
        result
    }

    /// Dart `visitExpressionFunctionBody(node, imposedType:)`.
    fn resolve_expression_function_body(
        &mut self,
        node: Id<ExpressionFunctionBody>,
        imposed_type: Option<TypeId>,
    ) -> TypeId {
        let (is_async, is_generator) = self.function_body_modifiers(node.raw());
        let body_context =
            BodyInferenceContext::new(&self.type_system, is_async, is_generator, imposed_type);
        let context_type = body_context.context_type.unwrap_or(TypeId::UNKNOWN);
        let old_body_context = self.body_context.replace(body_context);
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        let expression = self.resolve_expression(expression, context_type);
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.handle_return();
        }
        let expression_type = self.type_or_throw(expression);
        let ts = self.type_system;
        self.body_context
            .as_mut()
            .unwrap()
            .add_return_expression(&ts, Some(expression_type));
        let result = self.finish_function_body_inference();
        self.body_context = old_body_context;
        result
    }

    /// Dart `visitAnonymousBlockBody(node, imposedType:)`. STUB (C8, anonymous
    /// methods).
    pub fn visit_anonymous_block_body(&mut self, node: Id<AnonymousBlockBody>) {
        let _ = node;
    }

    /// Dart `visitAnonymousExpressionBody(node, imposedType:)`. STUB (C8,
    /// anonymous methods).
    pub fn visit_anonymous_expression_body(&mut self, node: Id<AnonymousExpressionBody>) {
        let _ = node;
    }

    pub fn visit_block_function_body(&mut self, node: Id<BlockFunctionBody>) {
        self.resolve_block_function_body(node, None);
    }

    pub fn visit_expression_function_body(&mut self, node: Id<ExpressionFunctionBody>) {
        self.resolve_expression_function_body(node, None);
    }

    pub fn visit_empty_function_body(&mut self, node: Id<EmptyFunctionBody>) {
        self.check_unreachable_node(node);
    }

    pub fn visit_native_function_body(&mut self, node: Id<NativeFunctionBody>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    /// Dart `_finishFunctionBodyInference`.
    pub(crate) fn finish_function_body_inference(&mut self) -> TypeId {
        let end_of_block_is_reachable = match &self.flow_analysis.flow {
            None => true,
            Some(flow) => flow.is_reachable(),
        };
        let ts = self.type_system;
        let body_context = self.body_context.as_ref().expect("body context");
        let ty = body_context.compute_inferred_return_type(&ts, end_of_block_is_reachable);
        let _ = TypeKind::Dynamic;
        ty
    }
}
