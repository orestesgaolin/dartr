// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_dead_code.dart

//! Dart `RemoveDeadCode`.

use dartr_ast::*;
use dartr_syntax::{TokenId, TokenType};

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::super::utils::Range;

/// Dart `RemoveDeadCode`.
pub struct RemoveDeadCode;

/// The edits (deletions) of the fix.
struct Edits<'c, 'a> {
    c: &'c ProducerContext<'a>,
    error_offset: u32,
    error_end: u32,
    deletions: Vec<Range>,
}

impl Edits<'_, '_> {
    fn delete(&mut self, r: Range) {
        self.deletions.push(r);
    }

    /// Dart `_deleteLineRange`.
    fn delete_lines(&mut self, r: Range) {
        let r = self.c.utils.get_lines_range(r, false);
        self.deletions.push(r);
    }

    fn find_biggest_fully_dead_ancestor(&self, mut node: NodeId) -> NodeId {
        let ast = self.c.ast;
        loop {
            let Some(parent) = ast.parent(node) else {
                return node;
            };
            if ast.offset(parent) < self.error_offset || ast.end(parent) > self.error_end {
                return node;
            }
            node = parent;
        }
    }

    /// Dart `_handleDeadAtEndOfNode`.
    fn dead_at_end_of_node(&mut self, node: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let diagnostic_offset = self.error_offset;
        if let Some(b) = ast.cast::<BinaryExpression>(node) {
            let operator = ast[b].operator;
            if c.token_offset(operator) == diagnostic_offset
                && matches!(c.lexeme(operator), "&&" | "||" | "??")
            {
                let r = c.range().end_end(ast[b].left_operand, node);
                self.delete(r);
            }
        } else if let Some(b) = ast.cast::<Block>(node) {
            if c.token_offset(ast[b].right_bracket) == diagnostic_offset {
                self.remove_dead_code_after(node);
            }
        } else if let Some(d) = ast.cast::<DoStatement>(node) {
            if c.token_offset(ast[d].while_keyword) == diagnostic_offset {
                self.remove_dead_code_after(ast[d].body.raw());
            }
        }
    }

    /// Dart `_handleDeadAtStartOfNode`.
    fn dead_at_start_of_node(&mut self, node: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let Some(parent) = ast.parent(node) else {
            return;
        };
        if let Some(catch) = ast.cast::<CatchClause>(node) {
            let Some(try_statement) = ast.cast::<TryStatement>(parent) else {
                return;
            };
            let clauses = ast.list(ast[try_statement].catch_clauses);
            let Some(index) = clauses.iter().position(|x| *x == catch) else {
                return;
            };
            let previous: NodeId = if index == 0 {
                ast[try_statement].body.raw()
            } else {
                clauses[index - 1].raw()
            };
            let r = c.range().end_end(previous, node);
            self.delete(r);
            return;
        }
        if ast.is::<Expression>(node) {
            let updaters = if let Some(p) = ast.cast::<ForPartsWithDeclarations>(parent) {
                Some(ast[p].updaters)
            } else if let Some(p) = ast.cast::<ForPartsWithExpression>(parent) {
                Some(ast[p].updaters)
            } else if let Some(p) = ast.cast::<ForPartsWithPattern>(parent) {
                Some(ast[p].updaters)
            } else {
                None
            };
            if let Some(updaters) = updaters {
                let updaters = ast.list_raw(updaters);
                if updaters.contains(&node) {
                    let Some(for_statement) = ast.parent(parent) else {
                        return;
                    };
                    let right_parenthesis = if let Some(f) = ast.cast::<ForStatement>(for_statement)
                    {
                        ast[f].right_parenthesis
                    } else if let Some(f) = ast.cast::<ForElement>(for_statement) {
                        ast[f].right_parenthesis
                    } else {
                        return;
                    };
                    let is_first = updaters.first() == Some(&node);
                    let before_paren = ast.tokens.previous(right_parenthesis);
                    let is_comma = !is_first && ast.tokens.ty(before_paren) == TokenType::COMMA;
                    let previous = ast.tokens.previous(ast.begin_token(node));
                    let r = if is_comma {
                        c.range().token_end_start(previous, right_parenthesis)
                    } else if is_first {
                        c.range().node_start_token_start(node, right_parenthesis)
                    } else {
                        c.range().token_start_start(previous, right_parenthesis)
                    };
                    self.delete(r);
                    return;
                }
            }
            if let Some(conditional) = ast.cast::<ConditionalExpression>(parent) {
                let then_expression = ast[conditional].then_expression.raw();
                let else_expression = ast[conditional].else_expression.raw();
                if (node == then_expression || node == else_expression)
                    && looks_side_effect_free(c, ast[conditional].condition.raw())
                {
                    let keep = if node == then_expression {
                        else_expression
                    } else {
                        then_expression
                    };
                    if ast.is::<ThrowExpression>(keep) {
                        if let Some(cascade) = ast
                            .parent(parent)
                            .and_then(|g| ast.cast::<CascadeExpression>(g))
                        {
                            if ast[cascade].target.raw() == parent {
                                return;
                            }
                        }
                    }
                    let r1 = c.range().start_start(parent, keep);
                    let r2 = c.range().end_end(keep, parent);
                    self.delete(r1);
                    self.delete(r2);
                }
                return;
            }
        }
        if ast.is::<Statement>(node) {
            if let Some(if_statement) = ast.cast::<IfStatement>(parent) {
                if ast[if_statement].case_clause.is_some()
                    || !looks_side_effect_free(c, ast[if_statement].expression.raw())
                {
                    return;
                }
                let then_statement = ast[if_statement].then_statement.raw();
                let else_statement = ast[if_statement].else_statement.map(|s| s.raw());
                if node == then_statement {
                    match else_statement {
                        None => self.remove_statement(parent),
                        Some(e) => self.replace_statement_with_inner(parent, e),
                    }
                } else if Some(node) == else_statement {
                    self.replace_statement_with_inner(parent, then_statement);
                }
                return;
            }
            if let Some(block) = ast.cast::<Block>(parent) {
                let Some(&last) = ast.list(ast[block].statements).last() else {
                    return;
                };
                let r = c.range().start_end(node, last);
                self.delete_lines(r);
                return;
            }
        }
        if let Some((colon, _)) = switch_member_parts(ast, node) {
            let Some(switch) = ast.cast::<SwitchStatement>(parent) else {
                return;
            };
            let members = ast.list_raw(ast[switch].members);
            let Some(index) = members.iter().position(|m| *m == node) else {
                return;
            };
            let mut override_end: Option<TokenId> = None;
            if index > 0 {
                if let Some((_, statements)) = switch_member_parts(ast, members[index - 1]) {
                    if statements == 0 {
                        override_end = Some(colon);
                    }
                }
            }
            let r = c.range().deletion_range_with_end(node, override_end);
            self.delete(r);
        }
    }

    /// Dart `_removeDeadCodeAfter`.
    fn remove_dead_code_after(&mut self, partially_dead: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let Some(parent) = ast.parent(partially_dead) else {
            return;
        };
        let Some(d) = ast.cast::<DoStatement>(parent) else {
            return;
        };
        if ast[d].body.raw() != partially_dead || self.error_end < ast.end(parent) {
            return;
        }
        if has_break_statement(ast, partially_dead) {
            return;
        }
        let do_keyword = ast[d].do_keyword;
        let while_keyword = ast[d].while_keyword;
        let semicolon = ast[d].semicolon;
        if let Some(block) = ast.cast::<Block>(partially_dead) {
            let r1 = c
                .range()
                .token_start_end(do_keyword, ast[block].left_bracket);
            let r2 = c
                .range()
                .token_start_end(ast[block].right_bracket, semicolon);
            self.delete_lines(r1);
            self.delete_lines(r2);
        } else {
            let next = ast.tokens.next(do_keyword);
            let r1 = c.range().token_start_start(do_keyword, next);
            let r2 = c.range().token_start_end(while_keyword, semicolon);
            self.delete(r1);
            self.delete_lines(r2);
        }
    }

    /// Dart `_removeStatement`.
    fn remove_statement(&mut self, statement: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let Some(parent) = ast.parent(statement) else {
            return;
        };
        if ast.is::<Block>(parent) {
            let r = c.range().node(statement);
            self.delete_lines(r);
        } else if let Some(i) = ast.cast::<IfStatement>(parent) {
            if ast[i].else_statement.map(|s| s.raw()) == Some(statement) {
                let r = c.range().end_end(ast[i].then_statement, parent);
                self.delete(r);
            }
        }
    }

    /// Dart `_replaceStatementWithInnerStatement`.
    fn replace_statement_with_inner(&mut self, statement: NodeId, inner: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let parent_is_block = ast.parent(statement).is_some_and(|p| ast.is::<Block>(p));
        if let (true, Some(block)) = (parent_is_block, ast.cast::<Block>(inner)) {
            let r1 = c
                .range()
                .node_start_token_end(statement, ast[block].left_bracket);
            let r2 = c
                .range()
                .token_start_node_end(ast[block].right_bracket, statement);
            self.delete_lines(r1);
            self.delete_lines(r2);
        } else {
            let r1 = c.range().start_start(statement, inner);
            let r2 = c.range().end_end(inner, statement);
            self.delete(r1);
            self.delete(r2);
        }
    }
}

/// The colon and the number of statements of a switch member.
fn switch_member_parts(ast: &Ast, node: NodeId) -> Option<(TokenId, usize)> {
    if let Some(m) = ast.cast::<SwitchCase>(node) {
        Some((ast[m].colon, ast.list(ast[m].statements).len()))
    } else if let Some(m) = ast.cast::<SwitchDefault>(node) {
        Some((ast[m].colon, ast.list(ast[m].statements).len()))
    } else if let Some(m) = ast.cast::<SwitchPatternCase>(node) {
        Some((ast[m].colon, ast.list(ast[m].statements).len()))
    } else {
        None
    }
}

/// Dart `DoStatement.hasBreakStatement`: a `break` in [body] that is not in
/// a nested loop or switch (a labeled `break` counts).
fn has_break_statement(ast: &Ast, body: NodeId) -> bool {
    fn visit(ast: &Ast, node: NodeId, nested: bool) -> bool {
        if let Some(b) = ast.cast::<BreakStatement>(node) {
            return !nested || ast[b].label.is_some();
        }
        if ast.is::<FunctionExpression>(node) {
            return false;
        }
        let nested = nested
            || ast.is::<ForStatement>(node)
            || ast.is::<WhileStatement>(node)
            || ast.is::<DoStatement>(node)
            || ast.is::<SwitchStatement>(node);
        ast.children(node)
            .into_iter()
            .any(|child| visit(ast, child, nested))
    }
    visit(ast, body, false)
}

/// Dart `_looksSideEffectFree`.
fn looks_side_effect_free(c: &ProducerContext<'_>, expression: NodeId) -> bool {
    let ast = c.ast;
    if let Some(a) = ast.cast::<AsExpression>(expression) {
        return looks_side_effect_free(c, ast[a].expression.raw());
    }
    if let Some(b) = ast.cast::<BinaryExpression>(expression) {
        return looks_side_effect_free(c, ast[b].left_operand.raw())
            && looks_side_effect_free(c, ast[b].right_operand.raw());
    }
    if ast.is::<SimpleIdentifier>(expression) || ast.is::<Literal>(expression) {
        return true;
    }
    if let Some(p) = ast.cast::<ParenthesizedExpression>(expression) {
        return looks_side_effect_free(c, ast[p].expression.raw());
    }
    if let Some(p) = ast.cast::<PostfixExpression>(expression) {
        if ast.tokens.ty(ast[p].operator) == TokenType::BANG {
            return looks_side_effect_free(c, ast[p].operand.raw());
        }
        return false;
    }
    if ast.is::<PrefixedIdentifier>(expression) {
        return true;
    }
    if let Some(p) = ast.cast::<PropertyAccess>(expression) {
        if let Some(target) = ast[p].target {
            if !looks_side_effect_free(c, target.raw()) {
                return false;
            }
        }
        return true;
    }
    ast.is::<SuperExpression>(expression)
}

impl CorrectionProducer for RemoveDeadCode {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_DEAD_CODE)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let Some(d) = c.diagnostic else { return };
        let ast = c.ast;
        let error_offset = d.offset as u32;
        let error_end = error_offset + d.length as u32;
        let Some(node) = ast.node_covering(c.unit, error_offset, 0) else {
            return;
        };
        let mut edits = Edits {
            c,
            error_offset,
            error_end,
            deletions: Vec::new(),
        };
        if ast.offset(node) == error_offset {
            let node = edits.find_biggest_fully_dead_ancestor(node);
            edits.dead_at_start_of_node(node);
        } else if ast.end(node) <= error_end {
            edits.dead_at_end_of_node(node);
        }
        let deletions = edits.deletions;
        if deletions.is_empty() {
            return;
        }
        builder.add_dart_file_edit(c.path, |b| {
            for r in &deletions {
                b.add_deletion(r.offset, r.length);
            }
        });
    }
}
