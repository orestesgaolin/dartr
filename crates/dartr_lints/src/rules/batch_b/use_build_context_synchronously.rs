// Dart source: pkg/linter/lib/src/rules/use_build_context_synchronously.dart
use super::flutter::{is_build_context, is_state};
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};
use dartr_element::{ElemRef, ElementId, InterfaceElement, Tag, TypeId};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use indexmap::IndexMap;

const MOUNTED_NAME: &str = "mounted";

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if c.is_in_test_directory() {
        return;
    }
    const NAME: &str = "use_build_context_synchronously";
    r.add(NodeKind::MethodInvocation, NAME, visit_method_invocation);
    r.add(NodeKind::InstanceCreationExpression, NAME, visit_instance_creation_expression);
    r.add(NodeKind::FunctionExpressionInvocation, NAME, visit_function_expression_invocation);
    r.add(NodeKind::PrefixedIdentifier, NAME, visit_prefixed_identifier);
}

/// Dart `AsyncState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AsyncState {
    Asynchronous,
    MountedCheck,
    NotMountedCheck,
}

use AsyncState::*;

/// Dart `AsyncState.asynchronousOrNull`.
fn asynchronous_or_null(state: Option<AsyncState>) -> Option<AsyncState> {
    state.filter(|&s| s == Asynchronous)
}

/// Dart `AsyncState?.isGuarded`.
fn is_guarded(state: Option<AsyncState>) -> bool {
    matches!(state, Some(MountedCheck | NotMountedCheck))
}

/// Dart `AsyncStateVisitor` (with `AsyncStateTracker`).
struct AsyncStateVisitor<'c, 'a> {
    c: &'c LinterContext<'a>,
    reference: NodeId,
    mounted_element: ElementId,
    state_cache: IndexMap<NodeId, Option<AsyncState>>,
    has_unrelated_mounted_check: bool,
}

impl<'c, 'a> AsyncStateVisitor<'c, 'a> {
    /// Dart `AsyncStateTracker.asyncStateFor`.
    fn async_state_for(&mut self, reference: NodeId, mounted_element: ElementId) -> Option<AsyncState> {
        self.reference = reference;
        self.mounted_element = mounted_element;
        let parent = self.c.ast.parent(reference)?;
        let state = self.accept(Some(parent));
        self.state_cache.insert(parent, state);
        state
    }

    fn is_reference(&self, node: Option<NodeId>) -> bool {
        node == Some(self.reference)
    }

    /// `node.accept(this)`.
    fn accept(&mut self, node: Option<NodeId>) -> Option<AsyncState> {
        let node = node?;
        let c = self.c;
        macro_rules! n {
            ($t:ident) => {
                &c.ast[Id::<$t>::from_raw(node)]
            };
        }
        match kind(c, node) {
            NodeKind::AdjacentStrings => {
                let strings = c.ast.list_raw(n!(AdjacentStrings).strings).iter().map(|&s| Some(s)).collect();
                self.asynchronous_if_any_is_async(strings)
            }
            NodeKind::AsExpression => {
                let e = n!(AsExpression).expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::AssignmentExpression => {
                let a = n!(AssignmentExpression);
                self.in_order_async_state(vec![(Some(a.left_hand_side.raw()), false), (Some(a.right_hand_side.raw()), true)])
            }
            NodeKind::AwaitExpression => {
                if let Some(&state) = self.state_cache.get(&node) {
                    return state;
                }
                if self.is_reference(Some(n!(AwaitExpression).expression.raw())) { None } else { Some(Asynchronous) }
            }
            NodeKind::BinaryExpression => self.visit_binary_expression(node),
            NodeKind::Block => {
                let statements = c.ast.list_raw(n!(Block).statements).to_vec();
                self.visit_block_like(statements, c.ast.parent(node))
            }
            NodeKind::BlockFunctionBody | NodeKind::ExpressionFunctionBody => None,
            NodeKind::CascadeExpression => {
                let e = n!(CascadeExpression);
                let mut nodes = vec![Some(e.target.raw())];
                nodes.extend(c.ast.list_raw(e.cascade_sections).iter().map(|&s| Some(s)));
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::CaseClause => {
                let p = n!(CaseClause).guarded_pattern.raw();
                self.accept(Some(p))
            }
            NodeKind::CatchClause => {
                let b = n!(CatchClause).body.raw();
                asynchronous_or_null(self.accept(Some(b)))
            }
            NodeKind::ConditionalExpression => {
                let e = n!(ConditionalExpression);
                let (cond, then, els) = (e.condition.raw(), e.then_expression.raw(), e.else_expression.raw());
                self.visit_if_like(cond, None, then, Some(els))
            }
            NodeKind::DoStatement => {
                let s = n!(DoStatement);
                let (body, condition) = (s.body.raw(), s.condition.raw());
                if self.is_reference(Some(body)) {
                    asynchronous_or_null(self.accept(Some(condition)))
                } else if self.is_reference(Some(condition)) {
                    asynchronous_or_null(self.accept(Some(body)))
                } else {
                    asynchronous_or_null(self.accept(Some(condition))).or_else(|| asynchronous_or_null(self.accept(Some(body))))
                }
            }
            NodeKind::ExpressionStatement => {
                let e = n!(ExpressionStatement).expression.raw();
                if self.is_reference(Some(e)) { None } else { asynchronous_or_null(self.accept(Some(e))) }
            }
            NodeKind::ExtensionOverride => {
                let args = self.arguments(n!(ExtensionOverride).argument_list);
                self.asynchronous_if_any_is_async(args)
            }
            NodeKind::ForElement => {
                let e = n!(ForElement);
                self.visit_for(e.for_loop_parts.raw(), e.body.raw())
            }
            NodeKind::ForStatement => {
                let s = n!(ForStatement);
                self.visit_for(s.for_loop_parts.raw(), s.body.raw())
            }
            NodeKind::FunctionExpressionInvocation => {
                let i = n!(FunctionExpressionInvocation);
                let mut nodes = vec![Some(i.function.raw())];
                nodes.extend(self.arguments(i.argument_list));
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::GuardedPattern => {
                let w = n!(GuardedPattern).when_clause.map(|w| w.raw());
                self.accept(w)
            }
            NodeKind::IfElement => {
                let e = n!(IfElement);
                let (x, cc, t, el) = (e.expression.raw(), e.case_clause.map(|c| c.raw()), e.then_element.raw(), e.else_element.map(|e| e.raw()));
                self.visit_if_like(x, cc, t, el)
            }
            NodeKind::IfStatement => {
                let s = n!(IfStatement);
                let (x, cc, t, el) = (s.expression.raw(), s.case_clause.map(|c| c.raw()), s.then_statement.raw(), s.else_statement.map(|e| e.raw()));
                self.visit_if_like(x, cc, t, el)
            }
            NodeKind::IndexExpression => {
                let e = n!(IndexExpression);
                let nodes = vec![e.target.map(|t| t.raw()), Some(e.index.raw())];
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::InstanceCreationExpression => {
                let args = self.arguments(n!(InstanceCreationExpression).argument_list);
                self.asynchronous_if_any_is_async(args)
            }
            NodeKind::InterpolationExpression => {
                let e = n!(InterpolationExpression).expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::IsExpression => {
                let e = n!(IsExpression).expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::LabeledStatement => {
                let s = n!(LabeledStatement).statement.raw();
                self.accept(Some(s))
            }
            NodeKind::ListLiteral => {
                let elements = c.ast.list_raw(n!(ListLiteral).elements).iter().map(|&e| Some(e)).collect();
                self.asynchronous_if_any_is_async(elements)
            }
            NodeKind::MapLiteralEntry => {
                let e = n!(MapLiteralEntry);
                let nodes = vec![Some(e.key.raw()), Some(e.value.raw())];
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::MethodInvocation => {
                let i = n!(MethodInvocation);
                let mut nodes = vec![i.target.map(|t| t.raw())];
                nodes.extend(self.arguments(i.argument_list));
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::NamedArgument => {
                let e = n!(NamedArgument).argument_expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::NullAwareElement => {
                let e = n!(NullAwareElement).value.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::ParenthesizedExpression => {
                let e = n!(ParenthesizedExpression).expression.raw();
                self.accept(Some(e))
            }
            NodeKind::PostfixExpression => {
                let e = n!(PostfixExpression).operand.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::PrefixedIdentifier => {
                let identifier = n!(PrefixedIdentifier).identifier;
                self.visit_identifier(identifier)
            }
            NodeKind::PrefixExpression => {
                let e = n!(PrefixExpression);
                if lexeme(c, e.operator) == "!" {
                    let operand = e.operand.raw();
                    match self.accept(Some(operand)) {
                        Some(MountedCheck) => Some(NotMountedCheck),
                        Some(NotMountedCheck) => Some(MountedCheck),
                        other => other,
                    }
                } else {
                    None
                }
            }
            NodeKind::PropertyAccess => {
                let e = n!(PropertyAccess);
                let (target, property_name) = (e.target.map(|t| t.raw()), e.property_name);
                if simple_name(c, property_name) == MOUNTED_NAME {
                    asynchronous_or_null(self.accept(target)).or_else(|| self.visit_identifier(property_name))
                } else {
                    asynchronous_or_null(self.accept(target))
                }
            }
            NodeKind::RecordLiteral => {
                let fields = c.ast.list_raw(n!(RecordLiteral).fields).iter().map(|&f| Some(f)).collect();
                self.asynchronous_if_any_is_async(fields)
            }
            NodeKind::RecordLiteralNamedField => {
                let e = n!(RecordLiteralNamedField).field_expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::SetOrMapLiteral => {
                let elements = c.ast.list_raw(n!(SetOrMapLiteral).elements).iter().map(|&e| Some(e)).collect();
                self.asynchronous_if_any_is_async(elements)
            }
            NodeKind::SimpleIdentifier => self.visit_identifier(Id::from_raw(node)),
            NodeKind::SpreadElement => {
                let e = n!(SpreadElement).expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            NodeKind::StringInterpolation => {
                let elements = c.ast.list_raw(n!(StringInterpolation).elements).iter().map(|&e| Some(e)).collect();
                self.asynchronous_if_any_is_async(elements)
            }
            NodeKind::SwitchCase => {
                let s = n!(SwitchCase);
                let mut nodes = vec![Some(s.expression.raw())];
                nodes.extend(c.ast.list_raw(s.statements).iter().map(|&s| Some(s)));
                self.in_order_async_state_guardable(nodes)
            }
            NodeKind::SwitchDefault => {
                let nodes = c.ast.list_raw(n!(SwitchDefault).statements).iter().map(|&s| Some(s)).collect();
                self.in_order_async_state_guardable(nodes)
            }
            NodeKind::SwitchExpression => {
                let s = n!(SwitchExpression);
                let mut nodes = vec![Some(s.expression.raw())];
                nodes.extend(c.ast.list_raw(s.cases).iter().map(|&s| Some(s)));
                self.asynchronous_if_any_is_async(nodes)
            }
            NodeKind::SwitchExpressionCase => {
                let s = n!(SwitchExpressionCase);
                let (guarded_pattern, expression) = (s.guarded_pattern.raw(), s.expression.raw());
                if self.is_reference(Some(guarded_pattern)) {
                    return None;
                }
                let when = c.ast[s.guarded_pattern].when_clause.map(|w| w.raw());
                let when_state = self.accept(when);
                if self.is_reference(Some(expression)) {
                    if matches!(when_state, Some(Asynchronous | MountedCheck)) {
                        return when_state;
                    }
                    return None;
                }
                asynchronous_or_null(when_state).or_else(|| asynchronous_or_null(self.accept(Some(expression))))
            }
            NodeKind::SwitchPatternCase => {
                let s = n!(SwitchPatternCase);
                let guarded_pattern = s.guarded_pattern;
                if self.is_reference(Some(guarded_pattern.raw())) {
                    return None;
                }
                let statements = c.ast.list_raw(s.statements).to_vec();
                let state = self.visit_block_like(statements.clone(), c.ast.parent(node));
                if state.is_some() {
                    return state;
                }
                if statements.contains(&self.reference) {
                    None
                } else {
                    let when = c.ast[guarded_pattern].when_clause.map(|w| w.raw());
                    asynchronous_or_null(self.accept(when))
                }
            }
            NodeKind::SwitchStatement => self.visit_switch_statement(node),
            NodeKind::TryStatement => {
                let s = n!(TryStatement);
                let body = s.body.raw();
                let catch_clauses: Vec<NodeId> = c.ast.list_raw(s.catch_clauses).to_vec();
                let finally_block = s.finally_block.map(|f| f.raw());
                let body_and_catches = || {
                    let mut nodes = vec![Some(body)];
                    nodes.extend(catch_clauses.iter().map(|&c| Some(c)));
                    nodes
                };
                if self.is_reference(Some(body)) {
                    None
                } else if catch_clauses.contains(&self.reference) {
                    asynchronous_or_null(self.accept(Some(body)))
                } else if self.is_reference(finally_block) {
                    self.asynchronous_if_any_is_async(body_and_catches())
                } else {
                    match self.accept(finally_block) {
                        Some(state) => Some(state),
                        None => self.asynchronous_if_any_is_async(body_and_catches()),
                    }
                }
            }
            NodeKind::VariableDeclaration => {
                let initializer = n!(VariableDeclaration).initializer.map(|i| i.raw());
                if self.is_reference(initializer) { None } else { asynchronous_or_null(self.accept(initializer)) }
            }
            NodeKind::VariableDeclarationList => {
                let variables = c.ast.list_raw(n!(VariableDeclarationList).variables).iter().map(|&v| Some(v)).collect();
                self.asynchronous_if_any_is_async(variables)
            }
            NodeKind::VariableDeclarationStatement => {
                let variables = n!(VariableDeclarationStatement).variables.raw();
                if self.is_reference(Some(variables)) { None } else { asynchronous_or_null(self.accept(Some(variables))) }
            }
            NodeKind::WhenClause => {
                let e = n!(WhenClause).expression.raw();
                self.accept(Some(e))
            }
            NodeKind::WhileStatement => {
                let s = n!(WhileStatement);
                let (condition, body) = (s.condition.raw(), s.body.raw());
                asynchronous_or_null(self.accept(Some(condition))).or_else(|| asynchronous_or_null(self.accept(Some(body))))
            }
            NodeKind::YieldStatement => {
                let e = n!(YieldStatement).expression.raw();
                asynchronous_or_null(self.accept(Some(e)))
            }
            _ => None,
        }
    }

    fn arguments(&self, list: Id<ArgumentList>) -> Vec<Option<NodeId>> {
        self.c.ast.list_raw(self.c.ast[list].arguments).iter().map(|&a| Some(a)).collect()
    }

    fn visit_binary_expression(&mut self, node: NodeId) -> Option<AsyncState> {
        let c = self.c;
        let e = &c.ast[Id::<BinaryExpression>::from_raw(node)];
        let (left, right) = (e.left_operand.raw(), e.right_operand.raw());
        let operator = lexeme(c, e.operator);
        let is_and = operator == "&&";
        let is_or = operator == "||";
        if self.is_reference(Some(left)) {
            return None;
        } else if self.is_reference(Some(right)) {
            let left_state = self.accept(Some(left));
            return match left_state {
                Some(Asynchronous) => Some(Asynchronous),
                Some(MountedCheck) if is_and => Some(MountedCheck),
                Some(NotMountedCheck) if is_or => Some(NotMountedCheck),
                _ => None,
            };
        }
        if is_and {
            let l = self.accept(Some(left));
            let r = self.accept(Some(right));
            return match (l, r) {
                (None, _) => r,
                (_, None) => l,
                (_, Some(Asynchronous)) => Some(Asynchronous),
                (Some(Asynchronous), _) => r,
                (Some(MountedCheck), _) => Some(MountedCheck),
                (Some(NotMountedCheck), _) => Some(NotMountedCheck),
            };
        }
        if is_or {
            let l = self.accept(Some(left));
            let r = self.accept(Some(right));
            return match (l, r) {
                (_, Some(Asynchronous)) => Some(Asynchronous),
                (_, Some(NotMountedCheck)) => Some(NotMountedCheck),
                (Some(Asynchronous), _) => Some(Asynchronous),
                (Some(MountedCheck), Some(MountedCheck)) => Some(MountedCheck),
                (Some(NotMountedCheck), _) => Some(NotMountedCheck),
                (_, _) => None,
            };
        }
        if operator == "==" || operator == "!=" {
            let negate = operator == "!=";
            let l = self.accept(Some(left));
            let r = self.accept(Some(right));
            if l == Some(Asynchronous) || r == Some(Asynchronous) {
                return Some(Asynchronous);
            }
            if matches!(l, Some(MountedCheck | NotMountedCheck)) {
                let constant = constant_bool_value(c, right)?;
                return constant_equality(l, constant != negate);
            }
            if matches!(r, Some(MountedCheck | NotMountedCheck)) {
                let constant = constant_bool_value(c, left)?;
                return constant_equality(r, constant != negate);
            }
            return None;
        }
        asynchronous_or_null(self.accept(Some(left))).or_else(|| asynchronous_or_null(self.accept(Some(right))))
    }

    fn visit_for(&mut self, parts: NodeId, body: NodeId) -> Option<AsyncState> {
        let c = self.c;
        let reference_is_body = self.is_reference(Some(body));
        match kind(c, parts) {
            NodeKind::ForPartsWithDeclarations => {
                let p = &c.ast[Id::<ForPartsWithDeclarations>::from_raw(parts)];
                let mut nodes: Vec<(Option<NodeId>, bool)> =
                    c.ast.list_raw(c.ast[p.variables].variables).iter().map(|&v| (Some(v), false)).collect();
                nodes.push((p.condition.map(|c| c.raw()), reference_is_body));
                nodes.extend(c.ast.list_raw(p.updaters).iter().map(|&u| (Some(u), false)));
                nodes.push((Some(body), false));
                self.in_order_async_state(nodes)
            }
            NodeKind::ForPartsWithExpression => {
                let p = &c.ast[Id::<ForPartsWithExpression>::from_raw(parts)];
                let mut nodes = vec![(p.initialization.map(|i| i.raw()), false), (p.condition.map(|c| c.raw()), reference_is_body)];
                nodes.extend(c.ast.list_raw(p.updaters).iter().map(|&u| (Some(u), false)));
                nodes.push((Some(body), false));
                self.in_order_async_state(nodes)
            }
            NodeKind::ForEachPartsWithDeclaration => {
                let iterable = c.ast[Id::<ForEachPartsWithDeclaration>::from_raw(parts)].iterable.raw();
                self.in_order_async_state(vec![(Some(iterable), false), (Some(body), false)])
            }
            NodeKind::ForEachPartsWithIdentifier => {
                let iterable = c.ast[Id::<ForEachPartsWithIdentifier>::from_raw(parts)].iterable.raw();
                self.in_order_async_state(vec![(Some(iterable), false), (Some(body), false)])
            }
            NodeKind::ForEachPartsWithPattern => {
                let iterable = c.ast[Id::<ForEachPartsWithPattern>::from_raw(parts)].iterable.raw();
                self.in_order_async_state(vec![(Some(iterable), false), (Some(body), false)])
            }
            _ => None,
        }
    }

    fn visit_switch_statement(&mut self, node: NodeId) -> Option<AsyncState> {
        let c = self.c;
        let s = &c.ast[Id::<SwitchStatement>::from_raw(node)];
        let expression = s.expression.raw();
        let members: Vec<NodeId> = c.ast.list_raw(s.members).to_vec();
        let all = |members: &[NodeId]| members.iter().map(|&m| Some(m)).collect::<Vec<_>>();
        // Evaluated for its effects only, as in Dart.
        if asynchronous_or_null(self.accept(Some(expression))).is_none() {
            self.asynchronous_if_any_is_async(all(&members));
        }
        if let Some(index) = members.iter().position(|&m| m == self.reference) {
            let mut checked_cases_fall_through = true;
            let mut checked_cases_are_all_mounted_checks = true;
            for i in (0..=index).rev() {
                let Some(case) = c.ast.cast::<SwitchPatternCase>(members[i]) else { continue };
                let when = c.ast[c.ast[case].guarded_pattern].when_clause.map(|w| w.raw());
                let when_state = self.accept(when);
                if when_state == Some(Asynchronous) {
                    return Some(Asynchronous);
                }
                if checked_cases_fall_through {
                    let case_is_fall_through = i == index || c.ast.list_raw(c.ast[case].statements).is_empty();
                    if case_is_fall_through {
                        checked_cases_are_all_mounted_checks &= when_state == Some(MountedCheck);
                    } else if checked_cases_are_all_mounted_checks {
                        return Some(MountedCheck);
                    }
                    checked_cases_fall_through &= case_is_fall_through;
                }
            }
            if checked_cases_fall_through && checked_cases_are_all_mounted_checks {
                return Some(MountedCheck);
            }
            None
        } else {
            asynchronous_or_null(self.accept(Some(expression))).or_else(|| self.asynchronous_if_any_is_async(all(&members)))
        }
    }

    /// Dart `_asynchronousIfAnyIsAsync`.
    fn asynchronous_if_any_is_async(&mut self, nodes: Vec<Option<NodeId>>) -> Option<AsyncState> {
        let end = nodes.iter().position(|&n| n == Some(self.reference)).unwrap_or(nodes.len());
        for &node in &nodes[..end] {
            if node.is_some() && self.accept(node) == Some(Asynchronous) {
                return Some(Asynchronous);
            }
        }
        None
    }

    /// Dart `_inOrderAsyncState`.
    fn in_order_async_state(&mut self, nodes: Vec<(Option<NodeId>, bool)>) -> Option<AsyncState> {
        let first = nodes.first()?;
        if first.0 == Some(self.reference) {
            return None;
        }
        let reference_index = nodes.iter().position(|n| n.0 == Some(self.reference));
        let starting_index = match reference_index {
            Some(i) if i > 0 => i - 1,
            _ => nodes.len() - 1,
        };
        for i in (0..=starting_index).rev() {
            let (node, mounted_can_guard) = nodes[i];
            if node.is_none() {
                continue;
            }
            let state = self.accept(node);
            if state == Some(Asynchronous) {
                return Some(Asynchronous);
            }
            if mounted_can_guard && state.is_some() {
                return state;
            }
        }
        None
    }

    /// Dart `_inOrderAsyncStateGuardable`.
    fn in_order_async_state_guardable(&mut self, nodes: Vec<Option<NodeId>>) -> Option<AsyncState> {
        self.in_order_async_state(nodes.into_iter().map(|n| (n, true)).collect())
    }

    /// Dart `_visitBlockLike`.
    fn visit_block_like(&mut self, statements: Vec<NodeId>, parent: Option<NodeId>) -> Option<AsyncState> {
        let c = self.c;
        if Statement::test(kind(c, self.reference))
            && let Some(index) = statements.iter().position(|&s| s == self.reference)
        {
            let preceding = self.in_order_async_state_guardable(statements.iter().map(|&s| Some(s)).collect());
            if preceding.is_some() {
                return preceding;
            }
            if parent.is_some_and(|p| {
                matches!(kind(c, p), NodeKind::DoStatement | NodeKind::ForStatement | NodeKind::WhileStatement)
            }) {
                let following = statements[index + 1..].iter().map(|&s| Some(s)).collect();
                return asynchronous_or_null(self.in_order_async_state_guardable(following));
            }
            return None;
        }
        for &statement in statements.iter().rev() {
            let state = self.accept(Some(statement));
            if state.is_some() {
                return state;
            }
        }
        None
    }

    /// Dart `_visitIdentifier`.
    fn visit_identifier(&mut self, node: Id<SimpleIdentifier>) -> Option<AsyncState> {
        let c = self.c;
        if simple_name(c, node) != MOUNTED_NAME {
            return None;
        }
        if c.element(node).map(|e| base(c, e)) == Some(self.mounted_element) {
            return Some(MountedCheck);
        }
        self.has_unrelated_mounted_check = true;
        None
    }

    /// Dart `_visitIfLike`.
    fn visit_if_like(
        &mut self,
        expression: NodeId,
        case_clause: Option<NodeId>,
        then_branch: NodeId,
        else_branch: Option<NodeId>,
    ) -> Option<AsyncState> {
        if self.is_reference(Some(expression)) {
            return None;
        }
        let expression_state = self.accept(Some(expression));
        if case_clause.is_some() && self.is_reference(case_clause) {
            return match expression_state {
                Some(Asynchronous) => Some(Asynchronous),
                Some(MountedCheck) => Some(MountedCheck),
                _ => None,
            };
        }
        let case_clause_state = self.accept(case_clause);
        let condition_state = match (expression_state, case_clause_state) {
            (None, _) => case_clause_state,
            (_, None) => expression_state,
            (_, Some(Asynchronous)) => Some(Asynchronous),
            (Some(Asynchronous), _) => case_clause_state,
            (Some(MountedCheck), _) => Some(MountedCheck),
            (Some(NotMountedCheck), _) => Some(NotMountedCheck),
        };
        if self.is_reference(Some(then_branch)) {
            match condition_state {
                Some(Asynchronous) => Some(Asynchronous),
                Some(MountedCheck) => Some(MountedCheck),
                _ => None,
            }
        } else if else_branch.is_some() && self.is_reference(else_branch) {
            match condition_state {
                Some(Asynchronous) => Some(Asynchronous),
                Some(NotMountedCheck) => Some(MountedCheck),
                _ => None,
            }
        } else {
            let then_state = self.accept(Some(then_branch));
            let else_state = self.accept(else_branch);
            let then_terminates = terminates_control(self.c, then_branch);
            let else_terminates = else_branch.is_some_and(|e| terminates_control(self.c, e));
            if then_state == Some(NotMountedCheck) && (else_state == Some(NotMountedCheck) || else_terminates) {
                return Some(NotMountedCheck);
            }
            if else_state == Some(NotMountedCheck) && then_terminates {
                return Some(NotMountedCheck);
            }
            if then_state == Some(Asynchronous) && !then_terminates {
                return Some(Asynchronous);
            }
            if else_state == Some(Asynchronous) && !else_terminates {
                return Some(Asynchronous);
            }
            if condition_state == Some(Asynchronous) {
                return Some(Asynchronous);
            }
            if condition_state == Some(MountedCheck) && else_terminates {
                return Some(NotMountedCheck);
            }
            if condition_state == Some(NotMountedCheck) && then_terminates {
                return Some(NotMountedCheck);
            }
            None
        }
    }
}

/// Dart `AsyncStateVisitor._constantEquality`.
fn constant_equality(state: Option<AsyncState>, constant: bool) -> Option<AsyncState> {
    match (state, constant) {
        (Some(MountedCheck), true) => Some(MountedCheck),
        (Some(NotMountedCheck), true) => Some(NotMountedCheck),
        (Some(MountedCheck), false) => Some(NotMountedCheck),
        (Some(NotMountedCheck), false) => Some(MountedCheck),
        _ => None,
    }
}

/// Dart `Expression.constantBoolValue`.
fn constant_bool_value(c: &LinterContext<'_>, node: NodeId) -> Option<bool> {
    c.constant_value(node)?.to_bool_value()
}

/// Dart `AstNode.terminatesControl` / `Statement.terminatesControl`.
fn terminates_control(c: &LinterContext<'_>, node: NodeId) -> bool {
    if let Some(block) = c.ast.cast::<Block>(node) {
        return c.ast.list_raw(c.ast[block].statements).last().is_some_and(|&s| terminates_control(c, s));
    }
    if matches!(kind(c, node), NodeKind::ReturnStatement | NodeKind::BreakStatement | NodeKind::ContinueStatement) {
        return true;
    }
    c.resolved.as_ref().and_then(|r| r.exits).is_some_and(|exits| exits(node))
}

/// Dart `ProtectedFunction`.
struct ProtectedFunction {
    library: &'static str,
    type_: &'static str,
    name: Option<&'static str>,
    positional: &'static [usize],
    named: &'static [&'static str],
}

const fn protected(
    library: &'static str,
    type_: &'static str,
    name: Option<&'static str>,
    positional: &'static [usize],
    named: &'static [&'static str],
) -> ProtectedFunction {
    ProtectedFunction { library, type_, name, positional, named }
}

const ASYNC: &str = "dart.async";

const PROTECTED_CONSTRUCTORS: &[ProtectedFunction] = &[
    protected(ASYNC, "Future", None, &[0], &[]),
    protected(ASYNC, "Future", Some("new"), &[0], &[]),
    protected(ASYNC, "Future", Some("delayed"), &[1], &[]),
    protected(ASYNC, "Future", Some("microtask"), &[0], &[]),
    protected(ASYNC, "Stream", Some("eventTransformed"), &[1], &[]),
    protected(ASYNC, "Stream", Some("multi"), &[0], &[]),
    protected(ASYNC, "Stream", Some("periodic"), &[1], &[]),
    protected(ASYNC, "StreamController", None, &[], &["onListen", "onPause", "onResume", "onCancel"]),
    protected(ASYNC, "StreamController", Some("new"), &[], &["onListen", "onPause", "onResume", "onCancel"]),
    protected(ASYNC, "StreamController", Some("broadcast"), &[], &["onListen", "onCancel"]),
];

const PROTECTED_INSTANCE_METHODS: &[ProtectedFunction] = &[
    protected(ASYNC, "Future", Some("catchError"), &[0], &["test"]),
    protected(ASYNC, "Future", Some("onError"), &[0], &["test"]),
    protected(ASYNC, "Future", Some("then"), &[0], &["onError"]),
    protected(ASYNC, "Future", Some("timeout"), &[], &["onTimeout"]),
    protected(ASYNC, "Future", Some("whenComplete"), &[0], &[]),
    protected(ASYNC, "Stream", Some("any"), &[0], &[]),
    protected(ASYNC, "Stream", Some("asBroadcastStream"), &[], &["onListen", "onCancel"]),
    protected(ASYNC, "Stream", Some("asyncExpand"), &[0], &[]),
    protected(ASYNC, "Stream", Some("asyncMap"), &[0], &[]),
    protected(ASYNC, "Stream", Some("distinct"), &[0], &[]),
    protected(ASYNC, "Stream", Some("expand"), &[0], &[]),
    protected(ASYNC, "Stream", Some("firstWhere"), &[0], &["orElse"]),
    protected(ASYNC, "Stream", Some("fold"), &[1], &[]),
    protected(ASYNC, "Stream", Some("forEach"), &[0], &[]),
    protected(ASYNC, "Stream", Some("handleError"), &[0], &["test"]),
    protected(ASYNC, "Stream", Some("lastWhere"), &[0], &["orElse"]),
    protected(ASYNC, "Stream", Some("listen"), &[0], &["onError", "onDone"]),
    protected(ASYNC, "Stream", Some("map"), &[0], &[]),
    protected(ASYNC, "Stream", Some("reduce"), &[0], &[]),
    protected(ASYNC, "Stream", Some("singleWhere"), &[0], &["orElse"]),
    protected(ASYNC, "Stream", Some("skipWhile"), &[0], &[]),
    protected(ASYNC, "Stream", Some("takeWhile"), &[0], &[]),
    protected(ASYNC, "Stream", Some("timeout"), &[], &["onTimeout"]),
    protected(ASYNC, "Stream", Some("where"), &[0], &[]),
    protected(ASYNC, "StreamSubscription", Some("onData"), &[0], &[]),
    protected(ASYNC, "StreamSubscription", Some("onDone"), &[0], &[]),
    protected(ASYNC, "StreamSubscription", Some("onError"), &[0], &[]),
];

const PROTECTED_STATIC_METHODS: &[ProtectedFunction] = &[
    protected(ASYNC, "Future", Some("doWhile"), &[0], &[]),
    protected(ASYNC, "Future", Some("forEach"), &[1], &[]),
    protected(ASYNC, "Future", Some("wait"), &[], &["cleanUp"]),
];

/// Dart `_Visitor.check`.
fn check(c: &LinterContext<'_>, node: NodeId, mounted_element: ElementId, out: &mut Vec<Diagnostic>) {
    let mut child = node;
    let mut tracker = AsyncStateVisitor {
        c,
        reference: node,
        mounted_element,
        state_cache: IndexMap::new(),
        has_unrelated_mounted_check: false,
    };
    while !FunctionBody::test(kind(c, child)) {
        let Some(parent) = c.ast.parent(child) else { break };
        let state = tracker.async_state_for(child, mounted_element);
        if is_guarded(state) {
            return;
        }
        if state == Some(Asynchronous) {
            let code: &'static DiagnosticCode = if tracker.has_unrelated_mounted_check {
                &diag::USE_BUILD_CONTEXT_SYNCHRONOUSLY_WRONG_MOUNTED
            } else {
                &diag::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE
            };
            c.report_node(out, code, node, &[]);
            return;
        }
        child = parent;
    }
    if FunctionBody::test(kind(c, child)) {
        let Some(parent) = c.ast.parent(child).filter(|&p| kind(c, p) == NodeKind::FunctionExpression) else {
            return;
        };
        let mut grandparent = c.ast.parent(parent);
        if let Some(g) = grandparent
            && kind(c, g) == NodeKind::NamedArgument
        {
            grandparent = c.ast.parent(g);
        }
        if let Some(g) = grandparent
            && kind(c, g) == NodeKind::ArgumentList
            && let Some(invocation) = c.ast.parent(g)
        {
            if kind(c, invocation) == NodeKind::InstanceCreationExpression {
                check_constructor_callback(c, invocation, parent, node, out);
            }
            if kind(c, invocation) == NodeKind::MethodInvocation {
                check_method_callback(c, invocation, parent, node, out);
            }
        }
    }
}

fn split_arguments(c: &LinterContext<'_>, list: Id<ArgumentList>) -> (Vec<NodeId>, Vec<Id<NamedArgument>>) {
    let arguments = c.ast.list_raw(c.ast[list].arguments);
    let positional = arguments.iter().copied().filter(|&a| kind(c, a) != NodeKind::NamedArgument).collect();
    let named = arguments.iter().filter_map(|&a| c.ast.cast::<NamedArgument>(a)).collect();
    (positional, named)
}

/// Dart `checkConstructorCallback`.
fn check_constructor_callback(c: &LinterContext<'_>, invocation: NodeId, callback: NodeId, error_node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(static_type) = c.static_type(invocation) else { return };
    let i = &c.ast[Id::<InstanceCreationExpression>::from_raw(invocation)];
    let (positional, named) = split_arguments(c, i.argument_list);
    let constructor_name = c.ast[i.constructor_name].name.map(|n| simple_name(c, n));
    for constructor in PROTECTED_CONSTRUCTORS {
        if constructor_name == constructor.name && is_same_as(c, static_type, constructor.type_, constructor.library) {
            check_positional_arguments(c, constructor.positional, &positional, callback, error_node, out);
            check_named_arguments(c, constructor.named, &named, callback, error_node, out);
        }
    }
}

/// Dart `checkMethodCallback`.
fn check_method_callback(c: &LinterContext<'_>, invocation: NodeId, callback: NodeId, error_node: NodeId, out: &mut Vec<Diagnostic>) {
    let i = &c.ast[Id::<MethodInvocation>::from_raw(invocation)];
    let (positional, named) = split_arguments(c, i.argument_list);
    let method_name = simple_name(c, i.method_name);
    let target = real_target(c, invocation);
    let target_element = target
        .filter(|&t| Identifier::test(kind(c, t)))
        .and_then(|t| c.element(t))
        .map(|e| base(c, e));
    if let Some(class) = target_element.filter(|e| e.tag() == Tag::Class) {
        for method in PROTECTED_STATIC_METHODS {
            if Some(method_name) == method.name && name(c, class) == Some(method.type_) {
                check_positional_arguments(c, method.positional, &positional, callback, error_node, out);
                check_named_arguments(c, method.named, &named, callback, error_node, out);
            }
        }
    } else {
        let Some(static_type) = target.and_then(|t| c.static_type(t)) else { return };
        let type_element_name = rctx(c).and_then(|ctx| type_element_name(c, &ctx, static_type));
        for method in PROTECTED_INSTANCE_METHODS {
            if Some(method_name) == method.name && type_element_name == Some(method.type_) {
                check_positional_arguments(c, method.positional, &positional, callback, error_node, out);
                check_named_arguments(c, method.named, &named, callback, error_node, out);
            }
        }
    }
}

/// Dart `DartType.element?.name`.
fn type_element_name<'a>(c: &LinterContext<'a>, ctx: &dartr_element::Ctx<'a>, ty: TypeId) -> Option<&'a str> {
    let element = match *ctx.ty(ty) {
        dartr_element::TypeKind::Interface { element, .. } => element.raw(),
        dartr_element::TypeKind::TypeParameter { param, .. } => param.raw(),
        _ => return None,
    };
    name(c, element)
}

/// Dart `checkNamedArguments`.
fn check_named_arguments(
    c: &LinterContext<'_>,
    names: &[&str],
    named_arguments: &[Id<NamedArgument>],
    callback: NodeId,
    error_node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    for &named in names {
        let Some(&argument) = named_arguments.iter().find(|&&a| lexeme(c, c.ast[a].name) == named) else { continue };
        if c.ast[argument].argument_expression.raw() == callback {
            c.report_node(out, &diag::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE, error_node, &[]);
        }
    }
}

/// Dart `checkPositionalArguments`.
fn check_positional_arguments(
    c: &LinterContext<'_>,
    positions: &[usize],
    positional_arguments: &[NodeId],
    callback: NodeId,
    error_node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    for &position in positions {
        if positional_arguments.len() > position && argument_expression(c, positional_arguments[position]) == callback {
            c.report_node(out, &diag::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE, error_node, &[]);
        }
    }
}

fn visit_function_expression_invocation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    visit_argument_list(c, c.ast[Id::<FunctionExpressionInvocation>::from_raw(node)].argument_list, out);
}

fn visit_instance_creation_expression(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    visit_argument_list(c, c.ast[Id::<InstanceCreationExpression>::from_raw(node)].argument_list, out);
}

fn visit_method_invocation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    if let Some(target) = n.target.map(|t| t.raw())
        && is_build_context(c, c.static_type(target), true)
        && let Some(element) = build_context_typed_element(c, target)
        && let Some(mounted_getter) = associated_mounted_getter(c, element)
    {
        check(c, target, mounted_getter, out);
    }
    visit_argument_list(c, n.argument_list, out);
}

fn visit_prefixed_identifier(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<PrefixedIdentifier>::from_raw(node)];
    if simple_name(c, n.identifier) == MOUNTED_NAME {
        return;
    }
    let prefix = n.prefix.raw();
    if is_build_context(c, c.static_type(prefix), true)
        && let Some(element) = build_context_typed_element(c, prefix)
        && let Some(mounted_getter) = associated_mounted_getter(c, element)
    {
        check(c, prefix, mounted_getter, out);
    }
}

/// Dart `_visitArgumentList`.
fn visit_argument_list(c: &LinterContext<'_>, list: Id<ArgumentList>, out: &mut Vec<Diagnostic>) {
    for &argument in c.ast.list_raw(c.ast[list].arguments) {
        let expression = argument_expression(c, argument);
        if let Some(element) = build_context_typed_element(c, expression)
            && let Some(mounted_getter) = associated_mounted_getter(c, element)
        {
            check(c, expression, mounted_getter, out);
        }
    }
}

/// The declared type of an executable (`returnType`) or a variable (`type`).
fn declared_type(c: &LinterContext<'_>, element: ElementId) -> Option<TypeId> {
    let ctx = rctx(c)?;
    if element.cast::<dartr_element::ExecutableElement>().is_some() {
        Some(dartr_typesystem::member::return_type(&ctx, ElemRef::Base(element)))
    } else if matches!(
        element.tag(),
        Tag::Field
            | Tag::TopLevelVariable
            | Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::FormalParameter
            | Tag::FieldFormalParameter
            | Tag::SuperFormalParameter
    ) {
        Some(dartr_typesystem::member::type_(&ctx, ElemRef::Base(element)))
    } else {
        None
    }
}

/// Dart `Expression.buildContextTypedElement`.
fn build_context_typed_element(c: &LinterContext<'_>, expression: NodeId) -> Option<ElementId> {
    let mut node = expression;
    if let Some(access) = c.ast.cast::<PropertyAccess>(node) {
        node = c.ast[access].property_name.raw();
    }
    if Identifier::test(kind(c, node)) {
        let element = c.element(node)?;
        let declaration = base(c, element);
        let arg_type = declared_type(c, declaration);
        let is_getter = matches!(declaration.tag(), Tag::Getter | Tag::Setter);
        if is_build_context(c, arg_type, is_getter) {
            return Some(declaration);
        }
    } else if let Some(p) = c.ast.cast::<ParenthesizedExpression>(node) {
        return build_context_typed_element(c, c.ast[p].expression.raw());
    } else if let Some(p) = c.ast.cast::<PostfixExpression>(node)
        && lexeme(c, c.ast[p].operator) == "!"
    {
        return build_context_typed_element(c, c.ast[p].operand.raw());
    }
    None
}

/// Dart `ElementExtension.associatedMountedGetter`.
fn associated_mounted_getter(c: &LinterContext<'_>, element: ElementId) -> Option<ElementId> {
    let ctx = rctx(c)?;
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && let Some(enclosing) = enclosing(c, element).and_then(|e| e.cast::<InterfaceElement>())
        && is_state(c, enclosing)
    {
        return mounted_getter(c, enclosing);
    }
    let ty = declared_type(c, element)?;
    let interface = ctx.interface_element(ty)?;
    mounted_getter(c, interface)
}

/// Dart `_InterfaceElementExtension.mountedGetter`.
fn mounted_getter(c: &LinterContext<'_>, element: dartr_element::EId<InterfaceElement>) -> Option<ElementId> {
    let ctx = rctx(c)?;
    let result = InheritanceManager3::new(ctx).get_member(element, Name::new(&ctx, None, MOUNTED_NAME))?;
    let result = dartr_typesystem::member::base_element(&ctx, result);
    (result.tag() == Tag::Getter).then_some(result)
}
