// Dart source: pkg/analysis_server/lib/src/services/correction/dart/join_else_with_if.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/join_if_with_inner.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/join_if_with_outer.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/split_and_condition.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_if_else_with_conditional.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_switch_expression.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_switch_statement.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_if_case_statement.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_if_case_statement_chain.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/surround_with.dart

//! Control-flow, switch, if-case, and surround-with assists.

use dartr_ast::*;
use dartr_element::{ElementId, Tag, TypeKind};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::super::fix_kind::FixKind;
use super::super::generated::assist_kinds as a;
use super::super::producer::*;
use super::create::static_type;
use super::simple::assist;

// ---------------------------------------------------------------------------
// JoinElseWithIf & JoinIfWithElse
// ---------------------------------------------------------------------------

fn get_else_chain_statements(ast: &Ast, inner_if: Id<IfStatement>) -> Vec<NodeId> {
    let mut elses = vec![inner_if.raw()];
    let mut cur = ast[inner_if].else_statement.map(|s| s.raw());
    while let Some(s) = cur {
        if let Some(ifs) = ast.cast::<IfStatement>(s) {
            elses.push(s);
            cur = ast[ifs].else_statement.map(|e| e.raw());
        } else {
            elses.push(s);
            break;
        }
    }
    elses
}

fn join_comments_sources(
    c: &ProducerContext<'_>,
    comments: &[TokenId],
    prefix: &str,
    eol: &str,
) -> Option<String> {
    if comments.is_empty() {
        return None;
    }
    let mut source = String::new();
    let next_start = format!("{eol}{prefix}  ");
    for &comment in comments {
        let mut comments_source = c.lexeme(comment).to_string();
        let mut next_comment = c.ast.tokens.get(comment).next.get();
        while let Some(nc) = next_comment {
            if !c.ast.tokens.get(nc).is_comment() {
                break;
            }
            comments_source.push_str(&next_start);
            comments_source.push_str(c.lexeme(nc));
            next_comment = c.ast.tokens.get(nc).next.get();
        }
        source = format!("{source}{eol}{comments_source}");
    }
    Some(format!("{prefix}  {}", source.trim()))
}

fn block_source_for_join_else(
    c: &ProducerContext<'_>,
    block: Id<Block>,
    start_comments: Option<&str>,
    prefix: &str,
    end_comment: Option<&str>,
    eol: &str,
) -> String {
    let block_text = c.utils.get_range_text(c.range().node(block));
    let mut block_source = c
        .utils
        .replace_source_indent(&block_text, "  ", "", true, true)
        .trim_end()
        .to_string();
    let Some(right_brace_idx) = block_source.rfind('}') else {
        return block_source;
    };
    let after_right_brace = block_source[right_brace_idx..].to_string();
    if let Some(start_c) = start_comments {
        if let Some(first_nl) = block_source.find(eol) {
            let before = &block_source[..first_nl];
            let after = &block_source[first_nl..right_brace_idx];
            block_source = format!("{before}{eol}{start_c}{after}");
        } else if let Some(left_brace_idx) = block_source.find('{') {
            let mut after = block_source[left_brace_idx + 1..right_brace_idx].to_string();
            let expected_indent = format!("{prefix}  ");
            if !after.starts_with(&expected_indent) {
                after = format!("{expected_indent}{after}");
            }
            block_source = format!("{{{eol}{start_c}{eol}{after}");
        }
    } else {
        block_source.truncate(right_brace_idx);
    }
    if let Some(end_c) = end_comment {
        block_source = block_source.trim_end().to_string();
        block_source = format!("{block_source}{eol}{end_c}{eol}{prefix}");
    }
    block_source.push_str(&after_right_brace);
    block_source
}

fn compute_join_if_with_else_block(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    statements: &[NodeId],
    outer_if: Id<IfStatement>,
) {
    let ast = c.ast;
    let Some(else_kw) = ast[outer_if].else_keyword else {
        return;
    };
    let Some(else_stmt) = ast[outer_if].else_statement else {
        return;
    };
    if ast
        .tokens
        .get(ast.begin_token(else_stmt))
        .preceding_comments
        .is_some()
    {
        return;
    }
    let prefix = c.utils.get_node_prefix(outer_if.raw());
    let eol = c.eol().to_string();
    let mut source = String::new();
    for &stmt_init in statements {
        let mut statement = stmt_init;
        source.push_str(" else ");
        let mut before_if_comments = None;
        let mut before_cond_comments = None;
        if let Some(ifs) = ast.cast::<IfStatement>(statement) {
            before_if_comments = ast
                .tokens
                .get(ast.begin_token(ifs))
                .preceding_comments
                .get();
            before_cond_comments = ast
                .tokens
                .get(ast.tokens.next(ast[ifs].if_keyword))
                .preceding_comments
                .get();
            let mut cond_src = c.utils.get_node_text(ast[ifs].expression);
            if let Some(cc) = ast[ifs].case_clause {
                cond_src = format!("{cond_src} {}", c.utils.get_node_text(cc));
            }
            source.push_str(&format!("if ({cond_src}) "));
            statement = ast[ifs].then_statement.raw();
        }
        let end_comments_vec: Vec<TokenId> = ast
            .tokens
            .get(ast.tokens.next(ast.end_token(statement)))
            .preceding_comments
            .get()
            .into_iter()
            .collect();
        let end_comment_src = join_comments_sources(c, &end_comments_vec, &prefix, &eol);
        let mut begin_vec = Vec::new();
        if let Some(t) = before_if_comments {
            begin_vec.push(t);
        }
        if let Some(t) = before_cond_comments {
            begin_vec.push(t);
        }
        if let Some(t) = ast
            .tokens
            .get(ast.begin_token(statement))
            .preceding_comments
            .get()
        {
            begin_vec.push(t);
        }
        let begin_comments_src = join_comments_sources(c, &begin_vec, &prefix, &eol);
        let new_block_source = if let Some(block) = ast.cast::<Block>(statement) {
            block_source_for_join_else(
                c,
                block,
                begin_comments_src.as_deref(),
                &prefix,
                end_comment_src.as_deref(),
                &eol,
            )
        } else {
            let mut stmt_src = c.utils.get_node_text(statement);
            if !stmt_src.starts_with(&prefix) {
                stmt_src = format!("{prefix}{stmt_src}");
            }
            source.push_str(&format!("{{{eol}"));
            if let Some(bc) = &begin_comments_src {
                source.push_str(&format!("{bc}{eol}"));
            }
            let mut nb = format!("  {stmt_src}");
            if let Some(ec) = &end_comment_src {
                nb.push_str(&format!("{eol}{ec}"));
            }
            nb.push_str(&format!("{eol}{prefix}}}"));
            nb
        };
        source.push_str(&new_block_source);
    }
    let start_offset = c.token_offset(else_kw).saturating_sub(1);
    let end_offset = ast.end(else_stmt);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(start_offset, end_offset - start_offset, &source);
    });
}

assist!(JoinElseWithIf, a::JOIN_ELSE_WITH_IF, |c, builder| {
    let ast = c.ast;
    let Some(enclosing_if) = ast.cast::<IfStatement>(c.node) else {
        return;
    };
    let Some(else_kw) = ast[enclosing_if].else_keyword else {
        return;
    };
    let sel_end = c.selection_offset + c.selection_length;
    if c.token_offset(else_kw) > c.selection_offset || c.token_end(else_kw) < sel_end {
        return;
    }
    let Some(else_stmt) = ast[enclosing_if].else_statement else {
        return;
    };
    let Some(block) = ast.cast::<Block>(else_stmt) else {
        return;
    };
    let stmts = ast.list_raw(ast[block].statements);
    if stmts.len() != 1 {
        return;
    }
    let Some(inner_if) = ast.cast::<IfStatement>(stmts[0]) else {
        return;
    };
    let chain = get_else_chain_statements(ast, inner_if);
    compute_join_if_with_else_block(c, builder, &chain, enclosing_if);
});

assist!(JoinIfWithElse, a::JOIN_IF_WITH_ELSE, |c, builder| {
    let ast = c.ast;
    let Some(inner_if) = ast.cast::<IfStatement>(c.node) else {
        return;
    };
    let kw = ast[inner_if].if_keyword;
    let sel_end = c.selection_offset + c.selection_length;
    if c.token_offset(kw) > c.selection_offset || c.token_end(kw) < sel_end {
        return;
    }
    let Some(block) = ast.parent(inner_if).and_then(|p| ast.cast::<Block>(p)) else {
        return;
    };
    let stmts = ast.list_raw(ast[block].statements);
    if stmts.len() != 1 || stmts[0] != inner_if.raw() {
        return;
    }
    let Some(enclosing_if) = ast.parent(block).and_then(|p| ast.cast::<IfStatement>(p)) else {
        return;
    };
    if ast[enclosing_if].else_statement.map(|s| s.raw()) != Some(block.raw()) {
        return;
    }
    let chain = get_else_chain_statements(ast, inner_if);
    compute_join_if_with_else_block(c, builder, &chain, enclosing_if);
});

// ---------------------------------------------------------------------------
// JoinIfWithInner & JoinIfWithOuter
// ---------------------------------------------------------------------------

fn enclosing_if_statement(ast: &Ast, node: NodeId) -> Option<Id<IfStatement>> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(ifs) = ast.cast::<IfStatement>(n) {
            return Some(ifs);
        } else if !ast.is::<Expression>(n) {
            return None;
        }
        cur = ast.parent(n);
    }
    None
}

fn should_wrap_parenthesis_before_and(ast: &Ast, expr: NodeId) -> bool {
    let Some(be) = ast.cast::<BinaryExpression>(expr) else {
        return false;
    };
    matches!(
        ast.tokens.ty(ast[be].operator),
        TokenType::BAR_BAR | TokenType::QUESTION_QUESTION
    )
}

fn single_statement_of(ast: &Ast, stmt: NodeId) -> Option<NodeId> {
    if let Some(b) = ast.cast::<Block>(stmt) {
        let list = ast.list_raw(ast[b].statements);
        if list.len() == 1 { Some(list[0]) } else { None }
    } else {
        Some(stmt)
    }
}

fn compute_join_outer_inner_if(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    outer_if: Id<IfStatement>,
    inner_if: Id<IfStatement>,
) {
    let ast = c.ast;
    if ast[outer_if].else_statement.is_some() || ast[inner_if].else_statement.is_some() {
        return;
    }
    if ast[inner_if].case_clause.is_some() {
        return;
    }
    let prefix = c.utils.get_node_prefix(outer_if.raw());
    let outer_cond_expr = ast[outer_if].expression.raw();
    let inner_cond_expr = ast[inner_if].expression.raw();
    let mut outer_cond_src = c.utils.get_node_text(outer_cond_expr);
    let mut inner_cond_src = c.utils.get_node_text(inner_cond_expr);
    if should_wrap_parenthesis_before_and(ast, outer_cond_expr) {
        outer_cond_src = format!("({outer_cond_src})");
    }
    if should_wrap_parenthesis_before_and(ast, inner_cond_expr) {
        inner_cond_src = format!("({inner_cond_src})");
    }
    let mut condition = format!("{outer_cond_src} && {inner_cond_src}");
    if let Some(outer_cc) = ast[outer_if].case_clause {
        let gp = ast[outer_cc].guarded_pattern;
        let case_pat = c.utils.get_node_text(ast[gp].pattern);
        if let Some(wc) = ast[gp].when_clause {
            let when_expr = ast[wc].expression.raw();
            let mut when_src = c.utils.get_node_text(when_expr);
            if should_wrap_parenthesis_before_and(ast, when_expr) {
                when_src = format!("({when_src})");
            }
            condition =
                format!("{outer_cond_src} case {case_pat} when {when_src} && {inner_cond_src}");
        } else {
            condition = format!("{outer_cond_src} case {case_pat} when {inner_cond_src}");
        }
    }
    let inner_then = ast[inner_if].then_statement.raw();
    let inner_stmts: Vec<NodeId> = if let Some(b) = ast.cast::<Block>(inner_then) {
        ast.list_raw(ast[b].statements).to_vec()
    } else {
        vec![inner_then]
    };
    let new_source = if let Some(&first) = inner_stmts.first() {
        let line_ranges = c.utils.get_lines_range(
            c.range().start_end(first, *inner_stmts.last().unwrap()),
            false,
        );
        let old_source = c.utils.get_range_text(line_ranges);
        c.utils
            .replace_source_indent(&old_source, "  ", "", true, true)
    } else {
        String::new()
    };
    let eol = c.eol();
    let r = c.range().node(outer_if);
    let replacement = format!("if ({condition}) {{{eol}{new_source}{prefix}}}");
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(r.offset, r.length, &replacement);
    });
}

assist!(JoinIfWithInner, a::JOIN_IF_WITH_INNER, |c, builder| {
    let ast = c.ast;
    let Some(outer_if) = enclosing_if_statement(ast, c.node) else {
        return;
    };
    let Some(single) = single_statement_of(ast, ast[outer_if].then_statement.raw()) else {
        return;
    };
    let Some(inner_if) = ast.cast::<IfStatement>(single) else {
        return;
    };
    compute_join_outer_inner_if(c, builder, outer_if, inner_if);
});

assist!(JoinIfWithOuter, a::JOIN_IF_WITH_OUTER, |c, builder| {
    let ast = c.ast;
    let Some(inner_if) = enclosing_if_statement(ast, c.node) else {
        return;
    };
    let parent = ast.parent(inner_if);
    let outer_node = if let Some(b) = parent.and_then(|p| ast.cast::<Block>(p)) {
        if ast.list_raw(ast[b].statements).len() != 1 {
            return;
        }
        ast.parent(b)
    } else {
        parent
    };
    let Some(outer_if) = outer_node.and_then(|p| ast.cast::<IfStatement>(p)) else {
        return;
    };
    compute_join_outer_inner_if(c, builder, outer_if, inner_if);
});

// ---------------------------------------------------------------------------
// SplitAndCondition
// ---------------------------------------------------------------------------

assist!(SplitAndCondition, a::SPLIT_AND_CONDITION, |c, builder| {
    if c.selection_length > 0 {
        return;
    }
    let ast = c.ast;
    let Some(bin) = ast.cast::<BinaryExpression>(c.node) else {
        return;
    };
    if ast.tokens.ty(ast[bin].operator) != TokenType::AMPERSAND_AMPERSAND {
        return;
    }
    if c.token != ast[bin].operator {
        return;
    }
    let mut top_expr = bin.raw();
    while let Some(p) = ast.parent(top_expr) {
        if let Some(pb) = ast.cast::<BinaryExpression>(p) {
            if ast.tokens.ty(ast[pb].operator) == TokenType::AMPERSAND_AMPERSAND {
                top_expr = p;
                continue;
            }
        }
        break;
    }
    let Some(if_stmt) = ast
        .parent(top_expr)
        .and_then(|p| ast.cast::<IfStatement>(p))
    else {
        return;
    };
    if ast[if_stmt].else_statement.is_some()
        || ast[if_stmt].case_clause.is_some()
        || ast[if_stmt].expression.raw() != top_expr
    {
        return;
    }
    let prefix = c.utils.get_node_prefix(if_stmt.raw());
    let right_cond = c
        .utils
        .get_range_text(c.range().start_end(ast[bin].right_operand, top_expr));
    let then_stmt = ast[if_stmt].then_statement.raw();
    let then_text = c.utils.get_node_text(then_stmt);
    let indented_then =
        c.utils
            .replace_source_indent(&then_text, &prefix, &format!("{prefix}  "), false, false);
    let r = c.range().token_end_end(
        ast.end_token(ast[bin].left_operand),
        ast.end_token(then_stmt),
    );
    let eol = c.eol();
    let replacement =
        format!(") {{{eol}{prefix}  if ({right_cond}) {indented_then}{eol}{prefix}}}");
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(r.offset, r.length, &replacement);
    });
});

// ---------------------------------------------------------------------------
// ReplaceIfElseWithConditional
// ---------------------------------------------------------------------------

assist!(
    ReplaceIfElseWithConditional,
    a::REPLACE_IF_ELSE_WITH_CONDITIONAL,
    |c, builder| {
        let ast = c.ast;
        let Some(if_stmt) = ast.cast::<IfStatement>(c.node) else {
            return;
        };
        if ast[if_stmt].case_clause.is_some() {
            return;
        }
        let Some(else_stmt_raw) = ast[if_stmt].else_statement.map(|s| s.raw()) else {
            return;
        };
        let Some(then_single) = single_statement_of(ast, ast[if_stmt].then_statement.raw()) else {
            return;
        };
        let Some(else_single) = single_statement_of(ast, else_stmt_raw) else {
            return;
        };
        let cond = c.utils.get_node_text(ast[if_stmt].expression);
        let r = c.range().node(if_stmt);
        if let (Some(tr), Some(er)) = (
            ast.cast::<ReturnStatement>(then_single),
            ast.cast::<ReturnStatement>(else_single),
        ) {
            if let (Some(te), Some(ee)) = (ast[tr].expression, ast[er].expression) {
                let ts = c.utils.get_node_text(te);
                let es = c.utils.get_node_text(ee);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(
                        r.offset,
                        r.length,
                        &format!("return {cond} ? {ts} : {es};"),
                    );
                });
            }
        } else if let (Some(ts), Some(es)) = (
            ast.cast::<ExpressionStatement>(then_single),
            ast.cast::<ExpressionStatement>(else_single),
        ) {
            if let (Some(ta), Some(ea)) = (
                ast.cast::<AssignmentExpression>(ast[ts].expression),
                ast.cast::<AssignmentExpression>(ast[es].expression),
            ) {
                if c.lexeme(ast[ta].operator) == "=" && c.lexeme(ast[ea].operator) == "=" {
                    let tlhs = c.utils.get_node_text(ast[ta].left_hand_side);
                    let elhs = c.utils.get_node_text(ast[ea].left_hand_side);
                    if tlhs == elhs {
                        let trhs = c.utils.get_node_text(ast[ta].right_hand_side);
                        let erhs = c.utils.get_node_text(ast[ea].right_hand_side);
                        builder.add_dart_file_edit(c.path, |b| {
                            b.add_simple_replacement(
                                r.offset,
                                r.length,
                                &format!("{tlhs} = {cond} ? {trhs} : {erhs};"),
                            );
                        });
                    }
                }
            }
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertToSwitchExpression
// ---------------------------------------------------------------------------

fn is_effectively_exhaustive(c: &ProducerContext<'_>, switch_stmt: Id<SwitchStatement>) -> bool {
    let ast = c.ast;
    let ctx = c.ctx;
    let Some(ty) = static_type(c, ast[switch_stmt].expression.raw()) else {
        return false;
    };
    if dartr_typesystem::type_system::TypeSystem::new(*ctx).is_always_exhaustive(ty) {
        return true;
    }
    let Some(&last) = ast.list_raw(ast[switch_stmt].members).last() else {
        return false;
    };
    if let Some(spc) = ast.cast::<SwitchPatternCase>(last) {
        return ast.is::<WildcardPattern>(ast[ast[spc].guarded_pattern].pattern);
    }
    ast.is::<SwitchDefault>(last)
}

fn get_break_range(c: &ProducerContext<'_>, bs: Id<BreakStatement>) -> super::super::utils::Range {
    let ast = c.ast;
    let begin = ast.begin_token(bs);
    let prev = ast
        .tokens
        .get(begin)
        .preceding_comments
        .get()
        .unwrap_or_else(|| ast.tokens.previous(begin));
    c.range().token_end_end(prev, ast.end_token(bs))
}

enum SwitchExprGroup {
    Default {
        node: Id<SwitchDefault>,
        statements: Vec<NodeId>,
    },
    JoinedCase {
        pattern_cases: Vec<Id<SwitchPatternCase>>,
        statements: Vec<NodeId>,
    },
}

assist!(
    ConvertToSwitchExpression,
    a::CONVERT_TO_SWITCH_EXPRESSION,
    |c, builder| {
        let ast = c.ast;
        let Some(switch_stmt) = ast.cast::<SwitchStatement>(c.node) else {
            return;
        };
        let mut following_throw: Option<(Id<ExpressionStatement>, Id<ThrowExpression>)> = None;
        if !is_effectively_exhaustive(c, switch_stmt) {
            if let Some(block) = ast.parent(switch_stmt).and_then(|p| ast.cast::<Block>(p)) {
                let stmts = ast.list_raw(ast[block].statements);
                if let Some(pos) = stmts.iter().position(|&s| s == switch_stmt.raw()) {
                    if let Some(&next) = stmts.get(pos + 1) {
                        if let Some(es) = ast.cast::<ExpressionStatement>(next) {
                            if let Some(te) = ast.cast::<ThrowExpression>(ast[es].expression) {
                                following_throw = Some((es, te));
                            }
                        }
                    }
                }
            }
            if following_throw.is_none() {
                return;
            }
        }

        let members = ast.list_raw(ast[switch_stmt].members);
        if members.is_empty() {
            return;
        }
        let mut member_groups: Vec<(Vec<NodeId>, Vec<NodeId>)> = Vec::new();
        let mut cur_members: Vec<NodeId> = Vec::new();
        for &m in members {
            cur_members.push(m);
            let stmts = if let Some(sd) = ast.cast::<SwitchDefault>(m) {
                ast.list_raw(ast[sd].statements)
            } else if let Some(sc) = ast.cast::<SwitchCase>(m) {
                ast.list_raw(ast[sc].statements)
            } else if let Some(spc) = ast.cast::<SwitchPatternCase>(m) {
                ast.list_raw(ast[spc].statements)
            } else {
                &[]
            };
            if !stmts.is_empty() {
                member_groups.push((std::mem::take(&mut cur_members), stmts.to_vec()));
            }
        }
        if !cur_members.is_empty() {
            member_groups.push((cur_members, Vec::new()));
        }
        if member_groups.is_empty() {
            return;
        }

        let mut can_be_return = true;
        let mut can_be_assignment = true;
        let mut can_be_argument = true;
        let mut write_element: Option<ElementId> = None;
        let mut assignment_operator: Option<TokenId> = None;
        let mut function_element: Option<ElementId> = None;
        let mut groups: Vec<SwitchExprGroup> = Vec::new();

        for (g_members, g_stmts) in &member_groups {
            for &m in g_members {
                let has_labels = if let Some(sd) = ast.cast::<SwitchDefault>(m) {
                    !ast.list(ast[sd].labels).is_empty()
                } else if let Some(sc) = ast.cast::<SwitchCase>(m) {
                    !ast.list(ast[sc].labels).is_empty()
                } else if let Some(spc) = ast.cast::<SwitchPatternCase>(m) {
                    !ast.list(ast[spc].labels).is_empty()
                } else {
                    false
                };
                if has_labels {
                    return;
                }
            }

            if g_members.len() == 1
                && let Some(sd) = ast.cast::<SwitchDefault>(g_members[0])
            {
                groups.push(SwitchExprGroup::Default {
                    node: sd,
                    statements: g_stmts.clone(),
                });
            } else {
                let mut pattern_cases = Vec::new();
                for &m in g_members {
                    let Some(spc) = ast.cast::<SwitchPatternCase>(m) else {
                        return;
                    };
                    pattern_cases.push(spc);
                }
                if pattern_cases.len() != 1
                    && pattern_cases
                        .iter()
                        .any(|&spc| ast[ast[spc].guarded_pattern].when_clause.is_some())
                {
                    return;
                }
                groups.push(SwitchExprGroup::JoinedCase {
                    pattern_cases,
                    statements: g_stmts.clone(),
                });
            }

            if g_stmts.is_empty() || g_stmts.len() > 2 {
                return;
            }
            if g_stmts.len() == 2 {
                if !ast.is::<BreakStatement>(g_stmts[1]) {
                    return;
                }
                can_be_return = false;
            }

            let stmt = g_stmts[0];
            if let Some(es) = ast.cast::<ExpressionStatement>(stmt) {
                let expr = ast[es].expression.raw();
                if ast.is::<ThrowExpression>(expr) {
                    if member_groups.len() == 1 {
                        can_be_assignment = false;
                        can_be_argument = false;
                    }
                    continue;
                }
                can_be_return = false;
                if can_be_argument && let Some(mi) = ast.cast::<MethodInvocation>(expr) {
                    can_be_assignment = false;
                    let Some(el) = c.locator().element(ast[mi].method_name) else {
                        return;
                    };
                    if el.tag() != Tag::TopLevelFunction {
                        return;
                    }
                    if let Some(fe) = function_element {
                        if fe != el {
                            return;
                        }
                    } else {
                        function_element = Some(el);
                    }
                } else if can_be_assignment && let Some(ae) = ast.cast::<AssignmentExpression>(expr)
                {
                    can_be_argument = false;
                    let Some(lhs) = ast.cast::<SimpleIdentifier>(ast[ae].left_hand_side) else {
                        return;
                    };
                    let Some(el) = c.locator().write_or_read_element(lhs) else {
                        return;
                    };
                    if let Some(we) = write_element {
                        if we != el
                            || assignment_operator.is_none_or(|op| {
                                ast.tokens.ty(op) != ast.tokens.ty(ast[ae].operator)
                            })
                        {
                            return;
                        }
                    } else {
                        if el.tag() != Tag::LocalVariable {
                            return;
                        }
                        write_element = Some(el);
                        assignment_operator = Some(ast[ae].operator);
                    }
                } else {
                    return;
                }
            } else {
                let Some(rs) = ast.cast::<ReturnStatement>(stmt) else {
                    return;
                };
                if !can_be_return || ast[rs].expression.is_none() {
                    return;
                }
                can_be_assignment = false;
                can_be_argument = false;
            }

            if !can_be_return && !can_be_assignment && !can_be_argument {
                return;
            }
        }

        let group_count = groups.len();
        if can_be_return {
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_insertion(ast.offset(switch_stmt), "return ");
                b.add_simple_insertion(ast.end(switch_stmt), ";");
                for (i, group) in groups.iter().enumerate() {
                    let (stmt, last_colon, with_trailing_comma) = match group {
                        SwitchExprGroup::Default { node, statements } => {
                            let r = c
                                .range()
                                .token_start_end(ast[*node].keyword, ast[*node].colon);
                            b.add_simple_replacement(r.offset, r.length, "_ =>");
                            (statements[0], ast[*node].colon, false)
                        }
                        SwitchExprGroup::JoinedCase {
                            pattern_cases,
                            statements,
                        } => {
                            let first_case = pattern_cases[0];
                            let last_case = *pattern_cases.last().unwrap();
                            let pattern_code = pattern_cases
                                .iter()
                                .map(|&spc| c.utils.get_node_text(ast[spc].guarded_pattern))
                                .collect::<Vec<_>>()
                                .join(" || ");
                            let r = c
                                .range()
                                .token_start_end(ast[first_case].keyword, ast[last_case].colon);
                            b.add_simple_replacement(
                                r.offset,
                                r.length,
                                &format!("{pattern_code} =>"),
                            );
                            (
                                statements[0],
                                ast[last_case].colon,
                                i + 1 < group_count || following_throw.is_some(),
                            )
                        }
                    };
                    let has_comment = ast
                        .tokens
                        .get(ast.begin_token(stmt))
                        .preceding_comments
                        .is_some();
                    if let Some(rs) = ast.cast::<ReturnStatement>(stmt) {
                        let expr = ast[rs].expression.unwrap();
                        if !has_comment {
                            let r = c.range().token_end_node_start(last_colon, expr);
                            b.add_simple_replacement(r.offset, r.length, " ");
                        } else {
                            let r = c
                                .range()
                                .token_start_node_start(ast[rs].return_keyword, expr);
                            b.add_deletion(r.offset, r.length);
                        }
                    } else if let Some(es) = ast.cast::<ExpressionStatement>(stmt) {
                        if let Some(te) = ast.cast::<ThrowExpression>(ast[es].expression) {
                            if !has_comment {
                                let r = c.range().token_end_node_start(last_colon, te);
                                b.add_simple_replacement(r.offset, r.length, " ");
                            }
                        }
                    }
                    let end_r = c.range().token(ast.end_token(stmt));
                    b.add_simple_replacement(
                        end_r.offset,
                        end_r.length,
                        if with_trailing_comma { "," } else { "" },
                    );
                }
                if let Some((throw_stmt, throw_expr)) = following_throw {
                    let throw_text = c.utils.get_node_text(throw_expr);
                    let next_line_offset = c.token_offset(ast[switch_stmt].right_bracket);
                    let insert_offset = c.utils.get_line_content_start(next_line_offset);
                    let next_line_prefix = c.utils.get_line_prefix(next_line_offset);
                    let indent = c.utils.one_indent();
                    let eol = c.eol();
                    let text = format!("{next_line_prefix}_ => {throw_text},");
                    let indented = text
                        .split(eol)
                        .map(|line| format!("{indent}{line}"))
                        .collect::<Vec<_>>()
                        .join(eol);
                    b.add_simple_insertion(insert_offset, &format!("{indented}{eol}"));
                    let del_r = c.utils.get_lines_range(c.range().node(throw_stmt), false);
                    b.add_deletion(del_r.offset, del_r.length);
                }
            });
        } else if can_be_assignment {
            let Some(we) = write_element else {
                return;
            };
            let Some(op_tok) = assignment_operator else {
                return;
            };
            let var_name = c.ctx.element_name(we).unwrap_or("");
            let op_lex = c.lexeme(op_tok);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_insertion(ast.offset(switch_stmt), &format!("{var_name} {op_lex} "));
                for (i, group) in groups.iter().enumerate() {
                    let (stmts, last_colon, with_trailing_comma) = match group {
                        SwitchExprGroup::Default { node, .. } => {
                            let r = c
                                .range()
                                .token_start_end(ast[*node].keyword, ast[*node].colon);
                            b.add_simple_replacement(r.offset, r.length, "_ =>");
                            (ast.list_raw(ast[*node].statements), ast[*node].colon, false)
                        }
                        SwitchExprGroup::JoinedCase { pattern_cases, .. } => {
                            let first_case = pattern_cases[0];
                            let last_case = *pattern_cases.last().unwrap();
                            let last_colon = ast[last_case].colon;
                            let pattern_code = pattern_cases
                                .iter()
                                .map(|&spc| c.utils.get_node_text(ast[spc].guarded_pattern))
                                .collect::<Vec<_>>()
                                .join(" || ");
                            let r = c
                                .range()
                                .token_start_end(ast[first_case].keyword, last_colon);
                            b.add_simple_replacement(
                                r.offset,
                                r.length,
                                &format!("{pattern_code} =>"),
                            );
                            (
                                ast.list_raw(ast[last_case].statements),
                                last_colon,
                                i + 1 < group_count,
                            )
                        }
                    };
                    for &stmt in stmts {
                        if let Some(es) = ast.cast::<ExpressionStatement>(stmt) {
                            let expr = ast[es].expression.raw();
                            if let Some(ae) = ast.cast::<AssignmentExpression>(expr) {
                                let has_comment = ast
                                    .tokens
                                    .get(ast.begin_token(stmt))
                                    .preceding_comments
                                    .is_some();
                                let del_r = if !has_comment {
                                    c.range().token_end_end(last_colon, ast[ae].operator)
                                } else {
                                    c.range().start_start(ae, ast[ae].right_hand_side)
                                };
                                b.add_deletion(del_r.offset, del_r.length);
                            } else if ast.is::<ThrowExpression>(expr) {
                                let del_r = c.range().start_offset_end_offset(
                                    c.token_end(last_colon),
                                    ast.offset(stmt).saturating_sub(1),
                                );
                                b.add_deletion(del_r.offset, del_r.length);
                            }
                        }
                        if let Some(bs) = ast.cast::<BreakStatement>(stmt) {
                            let del_r = get_break_range(c, bs);
                            b.add_deletion(del_r.offset, del_r.length);
                        } else {
                            let end_r = c.range().token(ast.end_token(stmt));
                            b.add_simple_replacement(
                                end_r.offset,
                                end_r.length,
                                if with_trailing_comma { "," } else { "" },
                            );
                        }
                    }
                }
                b.add_simple_insertion(ast.end(switch_stmt), ";");
            });
        } else if can_be_argument {
            let Some(fe) = function_element else {
                return;
            };
            let fn_name = c.ctx.element_name(fe).unwrap_or("");
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_insertion(ast.offset(switch_stmt), &format!("{fn_name}("));
                for (i, group) in groups.iter().enumerate() {
                    let (stmts, last_colon, with_trailing_comma) = match group {
                        SwitchExprGroup::Default { node, statements } => {
                            let r = c
                                .range()
                                .token_start_end(ast[*node].keyword, ast[*node].colon);
                            b.add_simple_replacement(r.offset, r.length, "_ =>");
                            (statements.as_slice(), ast[*node].colon, false)
                        }
                        SwitchExprGroup::JoinedCase {
                            pattern_cases,
                            statements,
                        } => {
                            let first_case = pattern_cases[0];
                            let last_case = *pattern_cases.last().unwrap();
                            let last_colon = ast[last_case].colon;
                            let pattern_code = pattern_cases
                                .iter()
                                .map(|&spc| c.utils.get_node_text(ast[spc].guarded_pattern))
                                .collect::<Vec<_>>()
                                .join(" || ");
                            let r = c
                                .range()
                                .token_start_end(ast[first_case].keyword, last_colon);
                            b.add_simple_replacement(
                                r.offset,
                                r.length,
                                &format!("{pattern_code} =>"),
                            );
                            (statements.as_slice(), last_colon, i + 1 < group_count)
                        }
                    };
                    for &stmt in stmts {
                        let has_comment = ast
                            .tokens
                            .get(ast.begin_token(stmt))
                            .preceding_comments
                            .is_some();
                        if let Some(es) = ast.cast::<ExpressionStatement>(stmt) {
                            let expr = ast[es].expression.raw();
                            if let Some(mi) = ast.cast::<MethodInvocation>(expr) {
                                let lparen = ast[ast[mi].argument_list].left_parenthesis;
                                let rparen = ast[ast[mi].argument_list].right_parenthesis;
                                let del_r = if !has_comment {
                                    c.range().token_end_end(last_colon, lparen)
                                } else {
                                    c.range().node_start_token_end(mi, lparen)
                                };
                                if has_comment {
                                    b.add_deletion(del_r.offset, del_r.length);
                                } else {
                                    b.add_simple_replacement(del_r.offset, del_r.length, " ");
                                }
                                let rp_r = c.range().token(rparen);
                                b.add_deletion(rp_r.offset, rp_r.length);
                            }
                            if !has_comment && ast.is::<ThrowExpression>(expr) {
                                let del_r = c.range().start_offset_end_offset(
                                    c.token_end(last_colon),
                                    ast.offset(stmt).saturating_sub(1),
                                );
                                b.add_deletion(del_r.offset, del_r.length);
                            }
                        }
                        if let Some(bs) = ast.cast::<BreakStatement>(stmt) {
                            let del_r = get_break_range(c, bs);
                            b.add_deletion(del_r.offset, del_r.length);
                        } else {
                            let end_r = c.range().token(ast.end_token(stmt));
                            b.add_simple_replacement(
                                end_r.offset,
                                end_r.length,
                                if with_trailing_comma { "," } else { "" },
                            );
                        }
                    }
                }
                b.add_simple_insertion(ast.end(switch_stmt), ");");
            });
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertIfStatementToSwitchStatement & ConvertSwitchExpressionToSwitchStatement
// ---------------------------------------------------------------------------

enum IfCaseItem {
    Then {
        expression_code: String,
        pattern_code: String,
        statement: NodeId,
    },
    Else {
        statement: NodeId,
    },
}

impl IfCaseItem {
    fn statement(&self) -> NodeId {
        match *self {
            IfCaseItem::Then { statement, .. } | IfCaseItem::Else { statement } => statement,
        }
    }
}

fn pattern_of_bool_condition(c: &ProducerContext<'_>, node: NodeId) -> Option<(String, String)> {
    let ast = c.ast;
    if let Some(be) = ast.cast::<BinaryExpression>(node) {
        let op = ast.tokens.ty(ast[be].operator);
        let lhs = ast[be].left_operand.raw();
        let rhs = ast[be].right_operand.raw();
        if op == TokenType::BANG_EQ && ast.is::<NullLiteral>(rhs) {
            return Some((c.utils.get_node_text(lhs), "_?".to_string()));
        } else if matches!(
            op,
            TokenType::LT | TokenType::LT_EQ | TokenType::GT | TokenType::GT_EQ
        ) && ast.is::<Literal>(rhs)
        {
            let pat_r = c.range().token_start_node_end(ast[be].operator, rhs);
            return Some((c.utils.get_node_text(lhs), c.utils.get_range_text(pat_r)));
        }
    } else if let Some(is_expr) = ast.cast::<IsExpression>(node) {
        if ast[is_expr].not_operator.is_some() {
            return None;
        }
        let expr_code = c.utils.get_node_text(ast[is_expr].expression);
        let ty_node = ast[is_expr].type_.raw();
        let type_code = c.utils.get_node_text(ty_node);
        let pat_code = match c.tables.annotation_type.get(ty_node).copied() {
            Some(ty) if matches!(c.ctx.ty(ty), TypeKind::Interface { .. }) => {
                format!("{type_code}()")
            }
            _ => format!("{type_code} _"),
        };
        return Some((expr_code, pat_code));
    }
    None
}

fn build_then_case(c: &ProducerContext<'_>, if_stmt: Id<IfStatement>) -> Option<IfCaseItem> {
    let ast = c.ast;
    let expr = ast[if_stmt].expression.raw();
    if let Some(cc) = ast[if_stmt].case_clause {
        if !ast.is::<SimpleIdentifier>(expr)
            && ast[if_stmt]
                .else_statement
                .is_some_and(|e| ast.is::<IfStatement>(e))
        {
            return None;
        }
        let gp = ast[cc].guarded_pattern;
        return Some(IfCaseItem::Then {
            expression_code: c.utils.get_node_text(expr),
            pattern_code: c.utils.get_node_text(gp),
            statement: ast[if_stmt].then_statement.raw(),
        });
    }
    let (expression_code, pattern_code) = pattern_of_bool_condition(c, expr)?;
    Some(IfCaseItem::Then {
        expression_code,
        pattern_code,
        statement: ast[if_stmt].then_statement.raw(),
    })
}

fn build_switch_cases(
    c: &ProducerContext<'_>,
    if_stmt: Id<IfStatement>,
) -> Option<Vec<IfCaseItem>> {
    let ast = c.ast;
    let then_case = build_then_case(c, if_stmt)?;
    let IfCaseItem::Then {
        expression_code: ref then_expr,
        ..
    } = then_case
    else {
        return None;
    };
    let then_expr = then_expr.clone();
    let mut cases = vec![then_case];
    if let Some(else_stmt) = ast[if_stmt].else_statement.map(|s| s.raw()) {
        if let Some(else_if) = ast.cast::<IfStatement>(else_stmt) {
            let else_cases = build_switch_cases(c, else_if)?;
            for ec in else_cases {
                if let IfCaseItem::Then {
                    ref expression_code,
                    ..
                } = ec
                {
                    if *expression_code != then_expr {
                        return None;
                    }
                }
                cases.push(ec);
            }
        } else {
            cases.push(IfCaseItem::Else {
                statement: else_stmt,
            });
        }
    }
    Some(cases)
}

fn write_switch_statement_branch(
    c: &ProducerContext<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    statement: NodeId,
    if_indent: &str,
    is_last_branch: bool,
) {
    let ast = c.ast;
    let stmts: Vec<NodeId> = if let Some(b) = ast.cast::<Block>(statement) {
        ast.list_raw(ast[b].statements).to_vec()
    } else {
        vec![statement]
    };
    let single_indent = c.utils.one_indent();
    let new_indent = format!("{if_indent}{single_indent}");
    if stmts.is_empty() {
        if !is_last_branch {
            e.write(&new_indent);
            e.write(single_indent);
            e.writeln("break;");
        }
        return;
    }
    let range = c
        .utils
        .get_lines_range(c.range().start_end(stmts[0], *stmts.last().unwrap()), false);
    let src = c.utils.get_range_text(range);
    let code = c
        .utils
        .replace_source_indent(&src, if_indent, &new_indent, true, true);
    e.write(&code);
}

assist!(
    ConvertIfStatementToSwitchStatement,
    a::CONVERT_TO_SWITCH_STATEMENT,
    |c, builder| {
        if !c.ctx.features.is_enabled("patterns") {
            return;
        }
        let ast = c.ast;
        let Some(if_stmt) = ast.cast::<IfStatement>(c.node) else {
            return;
        };
        let Some(cases) = build_switch_cases(c, if_stmt) else {
            return;
        };
        let Some(IfCaseItem::Then {
            expression_code, ..
        }) = cases.first()
        else {
            return;
        };
        let expression_code = expression_code.clone();
        let indent = c.utils.get_line_prefix(ast.offset(if_stmt));
        let single_indent = c.utils.one_indent();
        let case_indent = format!("{indent}{single_indent}");
        let r = c.range().node(if_stmt);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(r.offset, r.length, |e| {
                e.writeln(&format!("switch ({expression_code}) {{"));
                for (index, case_item) in cases.iter().enumerate() {
                    match case_item {
                        IfCaseItem::Then { pattern_code, .. } => {
                            e.writeln(&format!("{case_indent}case {pattern_code}:"));
                        }
                        IfCaseItem::Else { .. } => {
                            e.writeln(&format!("{case_indent}default:"));
                        }
                    }
                    write_switch_statement_branch(
                        c,
                        e,
                        case_item.statement(),
                        &indent,
                        index + 1 == cases.len(),
                    );
                }
                e.write(&format!("{indent}}}"));
            });
        });
    }
);

fn is_pure_untyped_wildcard(ast: &Ast, gp: Id<GuardedPattern>) -> bool {
    if ast[gp].when_clause.is_none() {
        if let Some(wp) = ast.cast::<WildcardPattern>(ast[gp].pattern) {
            return ast[wp].type_.is_none();
        }
    }
    false
}

fn rewrite_switch_expression_cases(
    c: &ProducerContext<'_>,
    b: &mut super::super::change_builder::FileEditBuilder<'_, '_>,
    switch_expr: Id<SwitchExpression>,
    indent: &str,
    before_case_expr: &str,
    semicolon: TokenId,
) {
    let ast = c.ast;
    let eol = c.eol();
    for &case_ in ast.list(ast[switch_expr].cases) {
        let gp = ast[case_].guarded_pattern;
        if is_pure_untyped_wildcard(ast, gp) {
            let r = c.range().node(gp);
            b.add_simple_replacement(r.offset, r.length, "default");
        } else {
            b.add_simple_insertion(ast.offset(gp), "case ");
        }
        let expr = ast[case_].expression;
        let r_mid = c.range().end_start(gp, expr);
        b.add_simple_replacement(
            r_mid.offset,
            r_mid.length,
            &format!(":{eol}{indent}    {before_case_expr}"),
        );
        let comma = ast.tokens.next(ast.end_token(expr));
        if ast.tokens.ty(comma) == TokenType::COMMA {
            let r_c = c.range().token(comma);
            b.add_simple_replacement(r_c.offset, r_c.length, ";");
        } else {
            b.add_simple_insertion(ast.end(expr), ";");
        }
    }
    let r_semi = c.range().token(semicolon);
    b.add_deletion(r_semi.offset, r_semi.length);
}

assist!(
    ConvertSwitchExpressionToSwitchStatement,
    a::CONVERT_TO_SWITCH_STATEMENT,
    |c, builder| {
        let ast = c.ast;
        let Some(switch_expr) = ast.cast::<SwitchExpression>(c.node) else {
            return;
        };
        let Some(parent) = ast.parent(switch_expr) else {
            return;
        };
        if let Some(vd) = ast.cast::<VariableDeclaration>(parent) {
            let Some(vdl) = ast
                .parent(vd)
                .and_then(|p| ast.cast::<VariableDeclarationList>(p))
            else {
                return;
            };
            if ast.list(ast[vdl].variables).len() != 1 {
                return;
            }
            let Some(vds) = ast
                .parent(vdl)
                .and_then(|p| ast.cast::<VariableDeclarationStatement>(p))
            else {
                return;
            };
            let indent = c.utils.get_line_prefix(ast.offset(vds));
            let eol = c.eol();
            let var_name = c.lexeme(ast[vd].name);
            let switch_ty = static_type(c, switch_expr.raw());
            builder.add_dart_file_edit(c.path, |b| {
                if ast[vdl].type_.is_none() {
                    if let Some(kw) = ast[vdl].keyword {
                        if c.lexeme(kw) == "final" {
                            b.add_replacement(c.token_offset(ast[vd].name), 0, |e| {
                                e.write_type(
                                    switch_ty,
                                    &super::super::dart_edit::WriteType::default(),
                                );
                                e.write(" ");
                            });
                        } else {
                            let kr = c.range().token(kw);
                            b.add_replacement(kr.offset, kr.length, |e| {
                                e.write_type(
                                    switch_ty,
                                    &super::super::dart_edit::WriteType::default(),
                                );
                            });
                        }
                    }
                }
                let r_split = c.range().token_end_node_start(ast[vd].name, switch_expr);
                b.add_simple_replacement(
                    r_split.offset,
                    r_split.length,
                    &format!(";{eol}{indent}"),
                );
                rewrite_switch_expression_cases(
                    c,
                    b,
                    switch_expr,
                    &indent,
                    &format!("{var_name} = "),
                    ast[vds].semicolon,
                );
            });
        } else if let Some(ae) = ast.cast::<AssignmentExpression>(parent) {
            if ast[ae].right_hand_side.raw() != switch_expr.raw() {
                return;
            }
            let Some(var_id) = ast.cast::<SimpleIdentifier>(ast[ae].left_hand_side) else {
                return;
            };
            let Some(es) = ast
                .parent(ae)
                .and_then(|p| ast.cast::<ExpressionStatement>(p))
            else {
                return;
            };
            let Some(semicolon) = ast[es].semicolon else {
                return;
            };
            let indent = c.utils.get_line_prefix(ast.offset(es));
            let var_lex = c.lexeme(ast[var_id].token);
            let op_lex = c.lexeme(ast[ae].operator);
            builder.add_dart_file_edit(c.path, |b| {
                let r_del = c.range().start_start(var_id, switch_expr);
                b.add_simple_replacement(r_del.offset, r_del.length, "");
                rewrite_switch_expression_cases(
                    c,
                    b,
                    switch_expr,
                    &indent,
                    &format!("{var_lex} {op_lex} "),
                    semicolon,
                );
            });
        } else if let Some(rs) = ast.cast::<ReturnStatement>(parent) {
            let indent = c.utils.get_line_prefix(ast.offset(rs));
            builder.add_dart_file_edit(c.path, |b| {
                let r_del = c
                    .range()
                    .token_start_node_start(ast[rs].return_keyword, switch_expr);
                b.add_simple_replacement(r_del.offset, r_del.length, "");
                rewrite_switch_expression_cases(
                    c,
                    b,
                    switch_expr,
                    &indent,
                    "return ",
                    ast[rs].semicolon,
                );
            });
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertToIfCaseStatement & ConvertToIfCaseStatementChain
// ---------------------------------------------------------------------------

assist!(
    ConvertToIfCaseStatement,
    a::CONVERT_TO_IF_CASE_STATEMENT,
    |c, builder| {
        if !c.ctx.features.is_enabled("patterns") {
            return;
        }
        let ast = c.ast;
        let Some(if_stmt) = ast.cast::<IfStatement>(c.node) else {
            return;
        };
        let Some(parent) = ast.parent(if_stmt) else {
            return;
        };
        let stmts = if let Some(b) = ast.cast::<Block>(parent) {
            ast.list_raw(ast[b].statements)
        } else if let Some(spc) = ast.cast::<SwitchPatternCase>(parent) {
            ast.list_raw(ast[spc].statements)
        } else if let Some(sd) = ast.cast::<SwitchDefault>(parent) {
            ast.list_raw(ast[sd].statements)
        } else {
            return;
        };
        let Some(idx) = stmts.iter().position(|&s| s == if_stmt.raw()) else {
            return;
        };
        if idx == 0 {
            return;
        }
        let prev_stmt = stmts[idx - 1];
        let following = &stmts[idx + 1..];
        let Some(vds) = ast.cast::<VariableDeclarationStatement>(prev_stmt) else {
            return;
        };
        let vdl = ast[vds].variables;
        let vars = ast.list(ast[vdl].variables);
        if vars.len() != 1 {
            return;
        }
        let decl = vars[0];
        let Some(var_elem) = c.locator().declared_element(decl) else {
            return;
        };
        if var_elem.tag() != Tag::LocalVariable {
            return;
        }
        let Some(initializer) = ast[decl].initializer else {
            return;
        };
        let is_final = ast[vdl].keyword.is_some_and(|k| c.lexeme(k) == "final");
        let name = c.lexeme(ast[decl].name);

        let has_references_after_then = || {
            let mut has_ref = false;
            let mut check = |n: NodeId| {
                super::variables::visit(ast, n, &mut |sub| {
                    if let Some(sid) = ast.cast::<SimpleIdentifier>(sub) {
                        if c.locator().element(sid) == Some(var_elem) {
                            has_ref = true;
                        }
                    }
                });
            };
            if let Some(else_s) = ast[if_stmt].else_statement {
                check(else_s.raw());
            }
            for &f in following {
                check(f);
            }
            has_ref
        };

        let cond = ast[if_stmt].expression.raw();
        let pattern_code = if let Some(is_expr) = ast.cast::<IsExpression>(cond) {
            if ast[is_expr].not_operator.is_some() {
                return;
            }
            let Some(left_id) = ast.cast::<SimpleIdentifier>(ast[is_expr].expression) else {
                return;
            };
            if c.lexeme(ast[left_id].token) != name || has_references_after_then() {
                return;
            }
            let kw = if is_final { "final " } else { "" };
            let type_code = c.utils.get_node_text(ast[is_expr].type_);
            format!("{kw}{type_code} {name}")
        } else if let Some(be) = ast.cast::<BinaryExpression>(cond) {
            if ast.tokens.ty(ast[be].operator) != TokenType::BANG_EQ
                || !ast.is::<NullLiteral>(ast[be].right_operand.raw())
            {
                return;
            }
            let Some(left_id) = ast.cast::<SimpleIdentifier>(ast[be].left_operand) else {
                return;
            };
            if c.lexeme(ast[left_id].token) != name || has_references_after_then() {
                return;
            }
            let kw = if is_final { "final" } else { "var" };
            format!("{kw} {name}?")
        } else {
            return;
        };

        let init_code = c.utils.get_node_text(initializer);
        let r_del = c.range().start_start(vds, if_stmt);
        let r_cond = c.range().node(cond);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(r_del.offset, r_del.length);
            b.add_simple_replacement(
                r_cond.offset,
                r_cond.length,
                &format!("{init_code} case {pattern_code}"),
            );
        });
    }
);

enum SwitchGroupKind {
    Single(Id<GuardedPattern>),
    Joined(Vec<NodeId>),
    Default,
}

assist!(
    ConvertToIfCaseStatementChain,
    a::CONVERT_TO_IF_CASE_STATEMENT_CHAIN,
    |c, builder| {
        let ast = c.ast;
        let Some(switch_stmt) = ast.cast::<SwitchStatement>(c.node) else {
            return;
        };
        let members = ast.list_raw(ast[switch_stmt].members);
        if members.is_empty() {
            return;
        }
        // Build member groups.
        let mut raw_groups: Vec<(Vec<NodeId>, Vec<NodeId>)> = Vec::new();
        let mut cur_members: Vec<NodeId> = Vec::new();
        for &m in members {
            cur_members.push(m);
            let stmts = if let Some(sd) = ast.cast::<SwitchDefault>(m) {
                ast.list_raw(ast[sd].statements)
            } else if let Some(sc) = ast.cast::<SwitchCase>(m) {
                ast.list_raw(ast[sc].statements)
            } else if let Some(spc) = ast.cast::<SwitchPatternCase>(m) {
                ast.list_raw(ast[spc].statements)
            } else {
                &[]
            };
            if !stmts.is_empty() {
                raw_groups.push((std::mem::take(&mut cur_members), stmts.to_vec()));
            }
        }
        if !cur_members.is_empty() {
            raw_groups.push((cur_members, Vec::new()));
        }

        let mut groups: Vec<(SwitchGroupKind, Vec<NodeId>)> = Vec::new();
        for (g_members, g_stmts) in raw_groups {
            if g_members.iter().any(|&m| ast.is::<SwitchDefault>(m)) {
                if g_members.len() != 1 {
                    return;
                }
                groups.push((SwitchGroupKind::Default, g_stmts));
                continue;
            }
            let mut guarded = Vec::new();
            for &m in &g_members {
                let Some(spc) = ast.cast::<SwitchPatternCase>(m) else {
                    return;
                };
                guarded.push(ast[spc].guarded_pattern);
            }
            if guarded.len() == 1 {
                groups.push((SwitchGroupKind::Single(guarded[0]), g_stmts));
            } else {
                if guarded.iter().any(|&gp| ast[gp].when_clause.is_some()) {
                    return;
                }
                let pats = guarded.iter().map(|&gp| ast[gp].pattern.raw()).collect();
                groups.push((SwitchGroupKind::Joined(pats), g_stmts));
            }
        }

        let if_indent = c.utils.get_line_prefix(ast.offset(switch_stmt));
        let single_indent = c.utils.one_indent();
        let expr_code = c.utils.get_node_text(ast[switch_stmt].expression);
        let r = c.range().node(switch_stmt);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(r.offset, r.length, |e| {
                for (idx, (kind, stmts)) in groups.iter().enumerate() {
                    if idx > 0 {
                        e.write(" else ");
                    }
                    match kind {
                        SwitchGroupKind::Single(gp) => {
                            let pat_code = c.utils.get_node_text(*gp);
                            e.writeln(&format!("if ({expr_code} case {pat_code}) {{"));
                        }
                        SwitchGroupKind::Joined(pats) => {
                            let pat_code = pats
                                .iter()
                                .map(|&p| c.utils.get_node_text(p))
                                .collect::<Vec<_>>()
                                .join(" || ");
                            e.writeln(&format!("if ({expr_code} case {pat_code}) {{"));
                        }
                        SwitchGroupKind::Default => {
                            e.writeln("{");
                        }
                    }
                    let body_stmts = match stmts.last() {
                        Some(&last) if ast.is::<BreakStatement>(last) => &stmts[..stmts.len() - 1],
                        _ => stmts.as_slice(),
                    };
                    if let Some(&first) = body_stmts.first() {
                        let range = c.utils.get_lines_range(
                            c.range().start_end(first, *body_stmts.last().unwrap()),
                            false,
                        );
                        let first_indent = c.utils.get_line_prefix(ast.offset(first));
                        let new_indent = format!("{if_indent}{single_indent}");
                        let code = c.utils.replace_source_indent(
                            &c.utils.get_range_text(range),
                            &first_indent,
                            &new_indent,
                            true,
                            true,
                        );
                        e.write(&code);
                    }
                    e.write(&format!("{if_indent}}}"));
                }
            });
        });
    }
);

// ---------------------------------------------------------------------------
// SurroundWith (multi-producer)
// ---------------------------------------------------------------------------

enum SurroundKind {
    Block,
    DoWhile,
    For,
    ForIn,
    If,
    SetState,
    TryCatch,
    TryFinally,
    While,
}

struct SurroundWithProducer {
    kind: SurroundKind,
    statements: Vec<NodeId>,
}

impl CorrectionProducer for SurroundWithProducer {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        None
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(match self.kind {
            SurroundKind::Block => &a::SURROUND_WITH_BLOCK,
            SurroundKind::DoWhile => &a::SURROUND_WITH_DO_WHILE,
            SurroundKind::For => &a::SURROUND_WITH_FOR,
            SurroundKind::ForIn => &a::SURROUND_WITH_FOR_IN,
            SurroundKind::If => &a::SURROUND_WITH_IF,
            SurroundKind::SetState => &a::SURROUND_WITH_SET_STATE,
            SurroundKind::TryCatch => &a::SURROUND_WITH_TRY_CATCH,
            SurroundKind::TryFinally => &a::SURROUND_WITH_TRY_FINALLY,
            SurroundKind::While => &a::SURROUND_WITH_WHILE,
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let first = self.statements[0];
        let last = *self.statements.last().unwrap();
        let lines_range = c
            .utils
            .get_lines_range(c.range().start_end(first, last), false);
        let indent_old = c.utils.get_node_prefix(first);
        let indent_new = format!("{indent_old}  ");
        let indented_code = c.utils.replace_source_indent(
            &c.utils.get_range_text(lines_range),
            &indent_old,
            &indent_new,
            true,
            true,
        );
        let kind = &self.kind;
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(lines_range.offset, lines_range.length, |e| match kind {
                SurroundKind::Block => {
                    e.write(&indent_old);
                    e.writeln("{");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.write("}");
                    e.select_here();
                    e.newline();
                }
                SurroundKind::DoWhile => {
                    e.write(&indent_old);
                    e.writeln("do {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.write("} while (");
                    e.add_simple_linked_edit("CONDITION", "condition", None);
                    e.writeln(");");
                }
                SurroundKind::For => {
                    e.write(&indent_old);
                    e.write("for (var ");
                    e.add_simple_linked_edit("VAR", "v", None);
                    e.write(" = ");
                    e.add_simple_linked_edit("INIT", "init", None);
                    e.write("; ");
                    e.add_simple_linked_edit("CONDITION", "condition", None);
                    e.write("; ");
                    e.add_simple_linked_edit("INCREMENT", "increment", None);
                    e.writeln(") {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.writeln("}");
                }
                SurroundKind::ForIn => {
                    e.write(&indent_old);
                    e.write("for (var ");
                    e.add_simple_linked_edit("NAME", "item", None);
                    e.write(" in ");
                    e.add_simple_linked_edit("ITERABLE", "iterable", None);
                    e.writeln(") {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.writeln("}");
                }
                SurroundKind::If => {
                    e.write(&indent_old);
                    e.write("if (");
                    e.add_simple_linked_edit("CONDITION", "condition", None);
                    e.writeln(") {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.writeln("}");
                }
                SurroundKind::SetState => {
                    e.write(&indent_old);
                    e.writeln("setState(() {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.select_all(|e| e.write("});"));
                    e.newline();
                }
                SurroundKind::TryCatch => {
                    e.write(&indent_old);
                    e.writeln("try {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.write("} on ");
                    e.add_simple_linked_edit("EXCEPTION_TYPE", " Exception", None);
                    e.write(" catch (");
                    e.add_simple_linked_edit("EXCEPTION_VAR", "e", None);
                    e.writeln(") {");
                    e.write(&indent_new);
                    e.select_all(|e| e.write("// TODO"));
                    e.newline();
                    e.write(&indent_old);
                    e.writeln("}");
                }
                SurroundKind::TryFinally => {
                    e.write(&indent_old);
                    e.writeln("try {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.writeln("} finally {");
                    e.write(&indent_new);
                    e.select_all(|e| e.write("// TODO"));
                    e.newline();
                    e.write(&indent_old);
                    e.writeln("}");
                }
                SurroundKind::While => {
                    e.write(&indent_old);
                    e.write("while (");
                    e.add_simple_linked_edit("CONDITION", "condition", None);
                    e.writeln(") {");
                    e.write(&indented_code);
                    e.write(&indent_old);
                    e.writeln("}");
                }
            });
        });
    }
}

pub fn surround_with_producers(
    c: &ProducerContext<'_>,
    _workspace: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    if c.selection_length == 0 {
        return Vec::new();
    }
    let ast = c.ast;
    let Some(block) = ast.cast::<Block>(c.node) else {
        return Vec::new();
    };
    let sel_start = c.selection_offset;
    let sel_end = c.selection_offset + c.selection_length;
    let mut selected = Vec::new();
    for &s in ast.list_raw(ast[block].statements) {
        if ast.offset(s) >= sel_start && ast.end(s) <= sel_end {
            selected.push(s);
        }
    }
    if selected.is_empty() {
        return Vec::new();
    }
    let in_state_subclass = ast
        .this_or_ancestor_of_type::<ClassDeclaration>(block)
        .and_then(|cd| c.locator().declared_element(cd))
        .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
        .is_some_and(|ie| {
            c.ctx.element_all_supertypes(ie).iter().any(|&t| {
                super::super::flutter::is_exact_widget_type(
                    c.ctx,
                    Some(t),
                    "State",
                    super::super::flutter::URI_FRAMEWORK,
                )
            })
        });

    let mut kinds = vec![
        SurroundKind::Block,
        SurroundKind::DoWhile,
        SurroundKind::For,
        SurroundKind::ForIn,
        SurroundKind::If,
    ];
    if in_state_subclass {
        kinds.push(SurroundKind::SetState);
    }
    kinds.extend([
        SurroundKind::TryCatch,
        SurroundKind::TryFinally,
        SurroundKind::While,
    ]);
    kinds
        .into_iter()
        .map(|kind| {
            Box::new(SurroundWithProducer {
                kind,
                statements: selected.clone(),
            }) as Box<dyn CorrectionProducer>
        })
        .collect()
}
