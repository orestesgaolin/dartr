// Dart source: pkg/analyzer/lib/src/dart/resolver/exit_detector.dart

//! Detects AST nodes that always terminate control flow.

use dartr_ast::*;
use dartr_element::{Ctx, ExecutableElement, ResolutionTables, TypeId};
use dartr_syntax::TokenType;
use indexmap::IndexSet;

use crate::ast_ext;
use crate::tables::ResolverTables;

/// Returns whether [node] exits. Element-based checks are unavailable.
pub fn exits(ast: &Ast, node: impl Into<NodeId>) -> bool {
    ExitDetector::new(None).node_exits(ast, Some(node.into()))
}

/// Returns whether [node] exits, including resolved executable return types
/// and labelled-break targets.
pub fn exits_resolved(
    ast: &Ast,
    tables: &ResolutionTables,
    resolver_tables: &ResolverTables,
    ctx: Ctx<'_>,
    node: impl Into<NodeId>,
) -> bool {
    ExitDetector::new(Some(ResolvedContext {
        tables,
        resolver_tables,
        ctx,
    }))
    .node_exits(ast, Some(node.into()))
}

#[derive(Clone, Copy)]
struct ResolvedContext<'a> {
    tables: &'a ResolutionTables,
    resolver_tables: &'a ResolverTables,
    ctx: Ctx<'a>,
}

struct ExitDetector<'a> {
    resolved: Option<ResolvedContext<'a>>,
    result: bool,
    enclosing_block_contains_break: bool,
    enclosing_block_contains_continue: bool,
    enclosing_block_breaks_label: IndexSet<NodeId>,
}

impl<'a> ExitDetector<'a> {
    fn new(resolved: Option<ResolvedContext<'a>>) -> Self {
        Self {
            resolved,
            result: false,
            enclosing_block_contains_break: false,
            enclosing_block_contains_continue: false,
            enclosing_block_breaks_label: IndexSet::new(),
        }
    }

    fn node_exits(&mut self, ast: &Ast, node: Option<NodeId>) -> bool {
        let Some(node) = node else {
            return false;
        };
        let outer = self.result;
        self.result = false;
        ast.accept_generalizing(node, self);
        let result = self.result;
        self.result = outer;
        result
    }

    fn visit_nodes<T: ?Sized>(&mut self, ast: &Ast, nodes: NodeList<T>) -> bool {
        ast.list(nodes)
            .iter()
            .rev()
            .any(|node| self.node_exits(ast, Some(node.raw())))
    }

    fn visit_statements(&mut self, ast: &Ast, statements: NodeList<Statement>) -> bool {
        ast.list(statements)
            .iter()
            .any(|node| self.node_exits(ast, Some(node.raw())))
    }

    fn visit_variable_declarations(
        &mut self,
        ast: &Ast,
        variables: NodeList<VariableDeclaration>,
    ) -> bool {
        ast.list(variables)
            .iter()
            .rev()
            .any(|node| self.node_exits(ast, Some(node.raw())))
    }

    fn known_condition_value(ast: &Ast, expression: Id<Expression>) -> Option<bool> {
        ast.cast::<BooleanLiteral>(expression)
            .map(|literal| ast[literal].value)
    }

    fn real_target(
        ast: &Ast,
        node: NodeId,
        target: Option<Id<Expression>>,
    ) -> Option<Id<Expression>> {
        if target.is_some() {
            return target;
        }
        let mut parent = ast.parent(node);
        while let Some(current) = parent {
            if let Some(cascade) = ast.cast::<CascadeExpression>(current) {
                return Some(ast[cascade].target);
            }
            parent = ast.parent(current);
        }
        None
    }

    fn element_exits(&self, node: Id<SimpleIdentifier>) -> bool {
        let Some(resolved) = self.resolved else {
            return false;
        };
        let Some(&element) = resolved.tables.element.get(node.raw()) else {
            return false;
        };
        let base = dartr_typesystem::member::base_element(&resolved.ctx, element);
        base.is::<ExecutableElement>()
            && dartr_typesystem::member::return_type(&resolved.ctx, element) == TypeId::NEVER
    }

    fn for_parts_exits(&mut self, ast: &Ast, parts: Id<ForLoopParts>, body: NodeId) -> bool {
        if let Some(parts) = ast.cast::<ForEachPartsWithDeclaration>(parts) {
            let iterable_exits = self.node_exits(ast, Some(ast[parts].iterable.raw()));
            self.node_exits(ast, Some(body));
            return iterable_exits;
        }
        if let Some(parts) = ast.cast::<ForEachPartsWithIdentifier>(parts) {
            let iterable_exits = self.node_exits(ast, Some(ast[parts].iterable.raw()));
            self.node_exits(ast, Some(body));
            return iterable_exits;
        }
        if let Some(parts) = ast.cast::<ForEachPartsWithPattern>(parts) {
            let iterable_exits = self.node_exits(ast, Some(ast[parts].iterable.raw()));
            self.node_exits(ast, Some(body));
            return iterable_exits;
        }

        let (initialization, variables, condition, updaters) =
            if let Some(parts) = ast.cast::<ForPartsWithDeclarations>(parts) {
                (
                    None,
                    Some(ast[parts].variables),
                    ast[parts].condition,
                    ast[parts].updaters,
                )
            } else if let Some(parts) = ast.cast::<ForPartsWithExpression>(parts) {
                (
                    ast[parts].initialization,
                    None,
                    ast[parts].condition,
                    ast[parts].updaters,
                )
            } else if let Some(parts) = ast.cast::<ForPartsWithPattern>(parts) {
                if self.node_exits(ast, Some(ast[parts].variables.raw())) {
                    return true;
                }
                (None, None, ast[parts].condition, ast[parts].updaters)
            } else {
                return false;
            };

        if variables.is_some_and(|variables| {
            self.visit_variable_declarations(ast, ast[variables].variables)
        }) || self.node_exits(ast, initialization.map(|node| node.raw()))
            || self.node_exits(ast, condition.map(|node| node.raw()))
            || self.visit_nodes(ast, updaters)
        {
            return true;
        }
        let body_exits = self.node_exits(ast, Some(body));
        let implicit_or_explicit_true = condition.is_none()
            || condition.and_then(|node| Self::known_condition_value(ast, node)) == Some(true);
        implicit_or_explicit_true && (body_exits || !self.enclosing_block_contains_break)
    }

    fn switch_member_statements(ast: &Ast, member: Id<SwitchMember>) -> NodeList<Statement> {
        if let Some(member) = ast.cast::<SwitchCase>(member) {
            ast[member].statements
        } else if let Some(member) = ast.cast::<SwitchDefault>(member) {
            ast[member].statements
        } else if let Some(member) = ast.cast::<SwitchPatternCase>(member) {
            ast[member].statements
        } else {
            NodeList::EMPTY
        }
    }
}

impl GeneralizingAstVisitor for ExitDetector<'_> {
    fn visit_node(&mut self, _ast: &Ast, _node: NodeId) {
        // The pinned Dart implementation throws here. Returning false keeps
        // erroneous or newer syntax from failing the whole analyzed unit.
        self.result = false;
    }

    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        self.result = self.visit_nodes(ast, ast[node].arguments);
    }

    fn visit_as_expression(&mut self, ast: &Ast, node: Id<AsExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_assert_initializer(&mut self, _ast: &Ast, _node: Id<AssertInitializer>) {
        self.result = false;
    }

    fn visit_assert_statement(&mut self, _ast: &Ast, _node: Id<AssertStatement>) {
        self.result = false;
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        let left = ast[node].left_hand_side;
        if self.node_exits(ast, Some(left.raw())) {
            self.result = true;
            return;
        }
        let operator = ast.tokens.ty(ast[node].operator);
        if matches!(
            operator,
            TokenType::AMPERSAND_AMPERSAND_EQ
                | TokenType::BAR_BAR_EQ
                | TokenType::QUESTION_QUESTION_EQ
        ) {
            self.result = false;
            return;
        }
        if let Some(property) = ast.cast::<PropertyAccess>(left)
            && ast.tokens.ty(ast[property].operator) == TokenType::QUESTION_PERIOD
        {
            self.result = false;
            return;
        }
        self.result = self.node_exits(ast, Some(ast[node].right_hand_side.raw()));
    }

    fn visit_await_expression(&mut self, ast: &Ast, node: Id<AwaitExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        let left = ast[node].left_operand;
        let right = ast[node].right_operand;
        self.result = match ast.tokens.ty(ast[node].operator) {
            TokenType::BAR_BAR => {
                if Self::known_condition_value(ast, left) == Some(false) {
                    self.node_exits(ast, Some(right.raw()))
                } else {
                    self.node_exits(ast, Some(left.raw()))
                }
            }
            TokenType::AMPERSAND_AMPERSAND => {
                if Self::known_condition_value(ast, left) == Some(true) {
                    self.node_exits(ast, Some(right.raw()))
                } else {
                    self.node_exits(ast, Some(left.raw()))
                }
            }
            TokenType::QUESTION_QUESTION => self.node_exits(ast, Some(left.raw())),
            _ => self.node_exits(ast, Some(left.raw())) || self.node_exits(ast, Some(right.raw())),
        };
    }

    fn visit_block(&mut self, ast: &Ast, node: Id<Block>) {
        self.result = self.visit_statements(ast, ast[node].statements);
    }

    fn visit_block_function_body(&mut self, ast: &Ast, node: Id<BlockFunctionBody>) {
        self.result = self.node_exits(ast, Some(ast[node].block.raw()));
    }

    fn visit_break_statement(&mut self, ast: &Ast, node: Id<BreakStatement>) {
        self.enclosing_block_contains_break = true;
        if ast[node].label.is_some()
            && let Some(resolved) = self.resolved
            && let Some(&target) = resolved
                .resolver_tables
                .break_continue_target
                .get(node.raw())
        {
            self.enclosing_block_breaks_label.insert(target);
        }
        self.result = false;
    }

    fn visit_cascade_expression(&mut self, ast: &Ast, node: Id<CascadeExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].target.raw()))
            || self.visit_nodes(ast, ast[node].cascade_sections);
    }

    fn visit_conditional_expression(&mut self, ast: &Ast, node: Id<ConditionalExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].condition.raw()))
            || (self.node_exits(ast, Some(ast[node].then_expression.raw()))
                && self.node_exits(ast, Some(ast[node].else_expression.raw())));
    }

    fn visit_constructor_reference(&mut self, _ast: &Ast, _node: Id<ConstructorReference>) {
        self.result = false;
    }

    fn visit_continue_statement(&mut self, _ast: &Ast, _node: Id<ContinueStatement>) {
        self.enclosing_block_contains_continue = true;
        self.result = false;
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        let outer_break = self.enclosing_block_contains_break;
        let outer_continue = self.enclosing_block_contains_continue;
        self.enclosing_block_contains_break = false;
        self.enclosing_block_contains_continue = false;
        let result = {
            let body_exits = self.node_exits(ast, Some(ast[node].body.raw()));
            let contains_break_or_continue =
                self.enclosing_block_contains_break || self.enclosing_block_contains_continue;
            (body_exits && !contains_break_or_continue)
                || self.node_exits(ast, Some(ast[node].condition.raw()))
                || (Self::known_condition_value(ast, ast[node].condition) == Some(true)
                    && !self.enclosing_block_contains_break)
        };
        self.enclosing_block_contains_break = outer_break;
        self.enclosing_block_contains_continue = outer_continue;
        self.result = result;
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].argument_list.raw()));
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        self.result = self.element_exits(ast[node].member_name)
            || self.node_exits(ast, Some(ast[node].argument_list.raw()));
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        self.result = self.element_exits(ast[node].property_name);
    }

    fn visit_empty_statement(&mut self, _ast: &Ast, _node: Id<EmptyStatement>) {
        self.result = false;
    }

    fn visit_expression_statement(&mut self, ast: &Ast, node: Id<ExpressionStatement>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_extension_override(&mut self, _ast: &Ast, _node: Id<ExtensionOverride>) {
        self.result = false;
    }

    fn visit_for_element(&mut self, ast: &Ast, node: Id<ForElement>) {
        let outer_break = self.enclosing_block_contains_break;
        self.enclosing_block_contains_break = false;
        let result = self.for_parts_exits(ast, ast[node].for_loop_parts, ast[node].body.raw());
        self.enclosing_block_contains_break = outer_break;
        self.result = result;
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        let outer_break = self.enclosing_block_contains_break;
        self.enclosing_block_contains_break = false;
        let result = self.for_parts_exits(ast, ast[node].for_loop_parts, ast[node].body.raw());
        self.enclosing_block_contains_break = outer_break;
        self.result = result;
    }

    fn visit_function_declaration_statement(
        &mut self,
        _ast: &Ast,
        _node: Id<FunctionDeclarationStatement>,
    ) {
        self.result = false;
    }

    fn visit_function_expression(&mut self, _ast: &Ast, _node: Id<FunctionExpression>) {
        self.result = false;
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].function.raw()))
            || self.node_exits(ast, Some(ast[node].argument_list.raw()));
    }

    fn visit_function_reference(&mut self, ast: &Ast, node: Id<FunctionReference>) {
        self.result = self.node_exits(ast, Some(ast[node].function.raw()));
    }

    fn visit_generic_function_type(&mut self, _ast: &Ast, _node: Id<GenericFunctionType>) {
        self.result = false;
    }

    fn visit_identifier(&mut self, _ast: &Ast, _node: Id<Identifier>) {
        self.result = false;
    }

    fn visit_if_element(&mut self, ast: &Ast, node: Id<IfElement>) {
        let condition = ast[node].expression;
        if self.node_exits(ast, Some(condition.raw())) {
            self.result = true;
        } else if Self::known_condition_value(ast, condition) == Some(true) {
            self.result = self.node_exits(ast, Some(ast[node].then_element.raw()));
        } else if Self::known_condition_value(ast, condition) == Some(false)
            && ast[node].else_element.is_some()
        {
            self.result = self.node_exits(ast, ast[node].else_element.map(|node| node.raw()));
        } else {
            let then_exits = self.node_exits(ast, Some(ast[node].then_element.raw()));
            let else_exits = self.node_exits(ast, ast[node].else_element.map(|node| node.raw()));
            self.result = ast[node].else_element.is_some() && then_exits && else_exits;
        }
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        let condition = ast[node].expression;
        if self.node_exits(ast, Some(condition.raw())) {
            self.result = true;
        } else if Self::known_condition_value(ast, condition) == Some(true) {
            self.result = self.node_exits(ast, Some(ast[node].then_statement.raw()));
        } else if Self::known_condition_value(ast, condition) == Some(false)
            && ast[node].else_statement.is_some()
        {
            self.result = self.node_exits(ast, ast[node].else_statement.map(|node| node.raw()));
        } else {
            let then_exits = self.node_exits(ast, Some(ast[node].then_statement.raw()));
            let else_exits = self.node_exits(ast, ast[node].else_statement.map(|node| node.raw()));
            self.result = ast[node].else_statement.is_some() && then_exits && else_exits;
        }
    }

    fn visit_implicit_call_reference(&mut self, ast: &Ast, node: Id<ImplicitCallReference>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        let target = Self::real_target(ast, node.raw(), ast[node].target);
        self.result = self.node_exits(ast, target.map(|target| target.raw()))
            || self.node_exits(ast, Some(ast[node].index.raw()));
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].argument_list.raw()));
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_label(&mut self, _ast: &Ast, _node: Id<Label>) {
        self.result = false;
    }

    fn visit_labeled_statement(&mut self, ast: &Ast, node: Id<LabeledStatement>) {
        let statement = ast[node].statement.raw();
        let statement_exits = self.node_exits(ast, Some(statement));
        self.result =
            statement_exits && !self.enclosing_block_breaks_label.shift_remove(&statement);
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        self.result = ast
            .list(ast[node].elements)
            .iter()
            .any(|element| self.node_exits(ast, Some(element.raw())));
    }

    fn visit_literal(&mut self, _ast: &Ast, _node: Id<Literal>) {
        self.result = false;
    }

    fn visit_map_literal_entry(&mut self, ast: &Ast, node: Id<MapLiteralEntry>) {
        self.result = self.node_exits(ast, Some(ast[node].key.raw()))
            || self.node_exits(ast, Some(ast[node].value.raw()));
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        if let Some(target) = ast_ext::method_invocation_real_target(ast, node) {
            if self.node_exits(ast, Some(target.raw())) {
                self.result = true;
                return;
            }
            if ast[node]
                .operator
                .is_some_and(|operator| ast.tokens.ty(operator) == TokenType::QUESTION_PERIOD)
            {
                self.result = false;
                return;
            }
        }
        self.result = self.element_exits(ast[node].method_name)
            || self.node_exits(ast, Some(ast[node].argument_list.raw()));
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        self.result = self.node_exits(ast, Some(ast[node].argument_expression.raw()));
    }

    fn visit_named_type(&mut self, _ast: &Ast, _node: Id<NamedType>) {
        self.result = false;
    }

    fn visit_null_aware_element(&mut self, ast: &Ast, node: Id<NullAwareElement>) {
        self.result = self.node_exits(ast, Some(ast[node].value.raw()));
    }

    fn visit_parenthesized_expression(&mut self, ast: &Ast, node: Id<ParenthesizedExpression>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_pattern_assignment(&mut self, ast: &Ast, node: Id<PatternAssignment>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_pattern_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclaration>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_pattern_variable_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].declaration.raw()));
    }

    fn visit_postfix_expression(&mut self, _ast: &Ast, _node: Id<PostfixExpression>) {
        self.result = false;
    }

    fn visit_prefix_expression(&mut self, _ast: &Ast, _node: Id<PrefixExpression>) {
        self.result = false;
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let target = Self::real_target(ast, node.raw(), ast[node].target);
        self.result = self.node_exits(ast, target.map(|target| target.raw()));
    }

    fn visit_rethrow_expression(&mut self, _ast: &Ast, _node: Id<RethrowExpression>) {
        self.result = true;
    }

    fn visit_return_statement(&mut self, _ast: &Ast, _node: Id<ReturnStatement>) {
        self.result = true;
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        self.result = ast
            .list(ast[node].elements)
            .iter()
            .any(|element| self.node_exits(ast, Some(element.raw())));
    }

    fn visit_spread_element(&mut self, ast: &Ast, node: Id<SpreadElement>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_super_expression(&mut self, _ast: &Ast, _node: Id<SuperExpression>) {
        self.result = false;
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        self.result = self.visit_statements(ast, ast[node].statements);
    }

    fn visit_switch_default(&mut self, ast: &Ast, node: Id<SwitchDefault>) {
        self.result = self.visit_statements(ast, ast[node].statements);
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        self.result = ast
            .list(ast[node].cases)
            .iter()
            .all(|case_| self.node_exits(ast, Some(case_.raw())));
    }

    fn visit_switch_expression_case(&mut self, ast: &Ast, node: Id<SwitchExpressionCase>) {
        let guarded = ast[node].guarded_pattern;
        let when = ast[guarded]
            .when_clause
            .map(|when| ast[when].expression.raw());
        self.result =
            self.node_exits(ast, when) || self.node_exits(ast, Some(ast[node].expression.raw()));
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        self.result = self.visit_statements(ast, ast[node].statements);
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        let outer_break = self.enclosing_block_contains_break;
        self.enclosing_block_contains_break = false;
        let members = ast.list(ast[node].members);
        let mut has_default = false;
        let mut has_non_exiting_case = false;
        for (index, &member) in members.iter().enumerate() {
            let statements = Self::switch_member_statements(ast, member);
            if ast.is::<SwitchDefault>(member) {
                has_default = true;
                if statements.is_empty() && index + 1 == members.len() {
                    has_non_exiting_case = true;
                    continue;
                }
            }
            if !statements.is_empty() && !self.node_exits(ast, Some(member.raw())) {
                has_non_exiting_case = true;
            }
        }
        self.enclosing_block_contains_break = outer_break;
        self.result = !has_non_exiting_case && has_default;
    }

    fn visit_this_expression(&mut self, _ast: &Ast, _node: Id<ThisExpression>) {
        self.result = false;
    }

    fn visit_throw_expression(&mut self, _ast: &Ast, _node: Id<ThrowExpression>) {
        self.result = true;
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        if self.node_exits(ast, ast[node].finally_block.map(|block| block.raw())) {
            self.result = true;
            return;
        }
        if !self.node_exits(ast, Some(ast[node].body.raw())) {
            self.result = false;
            return;
        }
        self.result = ast
            .list(ast[node].catch_clauses)
            .iter()
            .all(|clause| self.node_exits(ast, Some(ast[*clause].body.raw())));
    }

    fn visit_type_literal(&mut self, ast: &Ast, node: Id<TypeLiteral>) {
        self.result = self.node_exits(ast, Some(ast[node].type_.raw()));
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        self.result = self.node_exits(
            ast,
            ast[node].initializer.map(|initializer| initializer.raw()),
        );
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        self.result = self.visit_variable_declarations(ast, ast[node].variables);
    }

    fn visit_variable_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<VariableDeclarationStatement>,
    ) {
        self.result = self.node_exits(ast, Some(ast[node].variables.raw()));
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        let outer_break = self.enclosing_block_contains_break;
        self.enclosing_block_contains_break = false;
        let result = if self.node_exits(ast, Some(ast[node].condition.raw())) {
            true
        } else {
            self.node_exits(ast, Some(ast[node].body.raw()));
            Self::known_condition_value(ast, ast[node].condition) == Some(true)
                && !self.enclosing_block_contains_break
        };
        self.enclosing_block_contains_break = outer_break;
        self.result = result;
    }

    fn visit_yield_statement(&mut self, ast: &Ast, node: Id<YieldStatement>) {
        self.result = self.node_exits(ast, Some(ast[node].expression.raw()));
    }
}
