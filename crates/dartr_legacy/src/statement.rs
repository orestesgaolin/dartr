// Dart source: pkg/analysis_server/lib/src/services/completion/statement/statement_completion.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_statement_completion.dart

use dartr_ast::{
    Ast, Block, BlockClassBody, BlockFunctionBody, ClassDeclaration, DoStatement,
    EmptyFunctionBody, EmptyStatement, ExpressionStatement, ForStatement, FunctionDeclaration,
    IfStatement, MethodDeclaration, NodeId, NodeKind, ReturnStatement, SimpleIdentifier,
    SwitchStatement, TryStatement, VariableDeclarationStatement, WhileStatement,
};

use crate::protocol;
use crate::search::ResolvedUnitRef;

pub fn get_statement_completion(
    resolved: &ResolvedUnitRef,
    file: &str,
    offset: i64,
) -> protocol::EditGetStatementCompletionResult {
    let unit = resolved.unit();
    let ast = &unit.ast;
    let source = &ast.tokens.source;
    let Ok(selection_offset) = u32::try_from(offset) else {
        return empty_result(file, offset);
    };
    let proc = StatementProcessor {
        ast,
        root: unit.unit.raw(),
        source,
        file,
        selection_offset,
    };
    let change = proc
        .compute()
        .unwrap_or_else(|| default_change(file, offset));
    protocol::EditGetStatementCompletionResult {
        change,
        whitespace_only: false,
    }
}

fn default_change(file: &str, offset: i64) -> protocol::SourceChange {
    protocol::SourceChange {
        message: "Complete statement".to_string(),
        edits: Vec::new(),
        linked_edit_groups: Vec::new(),
        selection: Some(protocol::Position {
            file: file.to_string(),
            offset,
        }),
        selection_length: None,
        id: None,
    }
}

fn empty_result(file: &str, offset: i64) -> protocol::EditGetStatementCompletionResult {
    protocol::EditGetStatementCompletionResult {
        change: default_change(file, offset),
        whitespace_only: false,
    }
}

struct StatementProcessor<'a> {
    ast: &'a Ast,
    root: NodeId,
    source: &'a str,
    file: &'a str,
    selection_offset: u32,
}

impl<'a> StatementProcessor<'a> {
    fn eol(&self) -> &'static str {
        if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn line_indent(&self, offset: u32) -> String {
        let idx = (offset as usize).min(self.source.len());
        let line_start = self.source[..idx].rfind('\n').map_or(0, |p| p + 1);
        self.source[line_start..idx]
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect()
    }

    fn line_end_without_eol(&self, offset: u32) -> usize {
        let idx = (offset as usize).min(self.source.len());
        match self.source[idx..].find('\n') {
            Some(pos) => {
                let end = idx + pos;
                if end > 0 && self.source.as_bytes()[end - 1] == b'\r' {
                    end - 1
                } else {
                    end
                }
            }
            None => self.source.len(),
        }
    }

    fn compute(&self) -> Option<protocol::SourceChange> {
        let root = self.root;
        let mut node = self
            .ast
            .node_covering(root, self.selection_offset, 0)
            .or_else(|| {
                if self.selection_offset > 0 {
                    self.ast.node_covering(root, self.selection_offset - 1, 0)
                } else {
                    None
                }
            })?;
        if let Some(block) = self.ast.cast::<Block>(node)
            && let Some(&last_stmt) = self.ast.list(self.ast[block].statements).last()
            && self.ast.end(last_stmt.raw()) <= self.selection_offset
        {
            node = last_stmt.raw();
        }
        let mut cur = Some(node);
        while let Some(n) = cur {
            if let Some(change) = self.try_node(n) {
                return Some(change);
            }
            cur = self.ast.parent(n);
        }
        None
    }

    fn try_node(&self, node: NodeId) -> Option<protocol::SourceChange> {
        match self.ast.kind(node) {
            NodeKind::IfStatement => self.complete_if_statement(node),
            NodeKind::WhileStatement => self.complete_while_statement(node),
            NodeKind::DoStatement => self.complete_do_statement(node),
            NodeKind::ForStatement => self.complete_for_statement(node),
            NodeKind::SwitchStatement => self.complete_switch_statement(node),
            NodeKind::TryStatement => self.complete_try_statement(node),
            NodeKind::ClassDeclaration => self.complete_class_declaration(node),
            NodeKind::FunctionDeclaration => self.complete_function_declaration(node),
            NodeKind::MethodDeclaration => self.complete_method_declaration(node),
            NodeKind::VariableDeclarationStatement => self.complete_variable_declaration(node),
            NodeKind::ExpressionStatement => self.complete_expression_statement(node),
            NodeKind::ReturnStatement => self.complete_return_statement(node),
            NodeKind::Block => self.complete_block(node),
            _ => None,
        }
    }

    fn make_change(
        &self,
        message: &str,
        offset: usize,
        length: usize,
        replacement: String,
        cursor: usize,
    ) -> protocol::SourceChange {
        let edits = if length == 0 && replacement.is_empty() {
            Vec::new()
        } else {
            vec![protocol::SourceFileEdit {
                file: self.file.to_string(),
                file_stamp: 0,
                edits: vec![protocol::SourceEdit {
                    offset: offset as i64,
                    length: length as i64,
                    replacement,
                    id: None,
                    description: None,
                }],
            }]
        };
        protocol::SourceChange {
            message: message.to_string(),
            edits,
            linked_edit_groups: Vec::new(),
            selection: Some(protocol::Position {
                file: self.file.to_string(),
                offset: cursor as i64,
            }),
            selection_length: None,
            id: None,
        }
    }

    fn is_empty_or_synthetic_expr(&self, expr: NodeId) -> bool {
        if let Some(sid) = self.ast.cast::<SimpleIdentifier>(expr) {
            let tok = self.ast.tokens.get(self.ast[sid].token);
            tok.is_synthetic() || self.ast.tokens.lexeme(self.ast[sid].token).is_empty()
        } else {
            self.ast.length(expr) == 0
        }
    }

    fn is_empty_or_synthetic_stmt(&self, stmt: NodeId) -> bool {
        if self.ast.is::<EmptyStatement>(stmt) {
            let tok = self.ast.begin_token(stmt);
            return self.ast.tokens.get(tok).is_synthetic();
        }
        if let Some(es) = self.ast.cast::<ExpressionStatement>(stmt) {
            return self.is_empty_or_synthetic_expr(self.ast[es].expression.raw());
        }
        false
    }

    fn complete_if_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let ifs = self.ast.cast::<IfStatement>(node)?;
        let data = &self.ast[ifs];
        let cond = data.expression.raw();
        let cond_empty = self.is_empty_or_synthetic_expr(cond);
        let rparen_tok = self.ast.tokens.get(data.right_parenthesis);
        let then_stmt = data.then_statement.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        if self.is_empty_or_synthetic_stmt(then_stmt) {
            let insert_offset = if rparen_tok.is_synthetic() {
                self.ast.end(cond) as usize
            } else {
                rparen_tok.end() as usize
            };
            let prefix = if rparen_tok.is_synthetic() { ")" } else { "" };
            let block_before = format!("{prefix} {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = if cond_empty {
                self.ast.tokens.get(data.left_parenthesis).end() as usize
            } else {
                insert_offset + block_before.len()
            };
            return Some(self.make_change(
                "Insert a newline at the end of the current line",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_while_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let ws = self.ast.cast::<WhileStatement>(node)?;
        let data = &self.ast[ws];
        let cond = data.condition.raw();
        let cond_empty = self.is_empty_or_synthetic_expr(cond);
        let rparen_tok = self.ast.tokens.get(data.right_parenthesis);
        let body = data.body.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        if self.is_empty_or_synthetic_stmt(body) {
            let insert_offset = if rparen_tok.is_synthetic() {
                self.ast.end(cond) as usize
            } else {
                rparen_tok.end() as usize
            };
            let prefix = if rparen_tok.is_synthetic() { ")" } else { "" };
            let block_before = format!("{prefix} {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = if cond_empty {
                self.ast.tokens.get(data.left_parenthesis).end() as usize
            } else {
                insert_offset + block_before.len()
            };
            return Some(self.make_change(
                "Complete while-statement",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_do_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let ds = self.ast.cast::<DoStatement>(node)?;
        let data = &self.ast[ds];
        let body = data.body.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        if self.is_empty_or_synthetic_stmt(body) {
            let do_end = self.ast.tokens.get(data.do_keyword).end() as usize;
            let before = format!(" {{{eol}{indent}  {eol}{indent}}} while (");
            let after = ");";
            let replacement = format!("{before}{after}");
            let cursor = do_end + before.len();
            return Some(self.make_change("Complete do-statement", do_end, 0, replacement, cursor));
        }
        let semi_tok = self.ast.tokens.get(data.semicolon);
        if semi_tok.is_synthetic() {
            let rparen = self.ast.tokens.get(data.right_parenthesis);
            let cond = data.condition.raw();
            let cond_empty = self.is_empty_or_synthetic_expr(cond);
            let insert_offset = if rparen.is_synthetic() {
                self.ast.end(cond) as usize
            } else {
                rparen.end() as usize
            };
            let rep = if rparen.is_synthetic() { ");" } else { ";" };
            let cursor = if cond_empty {
                self.ast.tokens.get(data.left_parenthesis).end() as usize
            } else {
                insert_offset + rep.len()
            };
            return Some(self.make_change(
                "Complete do-statement",
                insert_offset,
                0,
                rep.to_string(),
                cursor,
            ));
        }
        None
    }

    fn complete_for_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let fs = self.ast.cast::<ForStatement>(node)?;
        let data = &self.ast[fs];
        let body = data.body.raw();
        let rparen_tok = self.ast.tokens.get(data.right_parenthesis);
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();
        let is_for_each = matches!(
            self.ast.kind(data.for_loop_parts.raw()),
            NodeKind::ForEachPartsWithDeclaration
                | NodeKind::ForEachPartsWithIdentifier
                | NodeKind::ForEachPartsWithPattern
        );
        let msg = if is_for_each {
            "Complete for-each statement"
        } else {
            "Complete for-statement"
        };

        if self.is_empty_or_synthetic_stmt(body) {
            let insert_offset = if rparen_tok.is_synthetic() {
                self.ast.end(data.for_loop_parts.raw()) as usize
            } else {
                rparen_tok.end() as usize
            };
            let prefix = if rparen_tok.is_synthetic() { ")" } else { "" };
            let block_before = format!("{prefix} {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let lparen_end = self.ast.tokens.get(data.left_parenthesis).end() as usize;
            let parts_len = self.ast.length(data.for_loop_parts.raw());
            let cursor = if parts_len == 0 {
                lparen_end
            } else {
                insert_offset + block_before.len()
            };
            return Some(self.make_change(msg, insert_offset, 0, replacement, cursor));
        }
        None
    }

    fn complete_switch_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let ss = self.ast.cast::<SwitchStatement>(node)?;
        let data = &self.ast[ss];
        let expr = data.expression.raw();
        let expr_empty = self.is_empty_or_synthetic_expr(expr);
        let rparen_tok = self.ast.tokens.get(data.right_parenthesis);
        let lbracket_tok = self.ast.tokens.get(data.left_bracket);
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        if lbracket_tok.is_synthetic() {
            let insert_offset = if rparen_tok.is_synthetic() {
                self.ast.end(expr) as usize
            } else {
                rparen_tok.end() as usize
            };
            let prefix = if rparen_tok.is_synthetic() { ")" } else { "" };
            let block_before = format!("{prefix} {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = if expr_empty {
                self.ast.tokens.get(data.left_parenthesis).end() as usize
            } else {
                insert_offset + block_before.len()
            };
            return Some(self.make_change(
                "Complete switch-statement",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_try_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let ts = self.ast.cast::<TryStatement>(node)?;
        let data = &self.ast[ts];
        let body = data.body;
        let lbracket = self.ast.tokens.get(self.ast[body].left_bracket);
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        if lbracket.is_synthetic() {
            let try_end = self.ast.tokens.get(data.try_keyword).end() as usize;
            let block_before = format!(" {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = try_end + block_before.len();
            return Some(self.make_change(
                "Complete try-statement",
                try_end,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_class_declaration(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let cd = self.ast.cast::<ClassDeclaration>(node)?;
        let data = &self.ast[cd];
        let body = data.body.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        let is_missing_braces = match self.ast.kind(body) {
            NodeKind::EmptyClassBody => true,
            NodeKind::BlockClassBody => {
                let b = self.ast.cast::<BlockClassBody>(body)?;
                self.ast.tokens.get(self.ast[b].left_bracket).is_synthetic()
            }
            _ => false,
        };
        if is_missing_braces {
            let insert_offset = self.line_end_without_eol(self.ast.offset(node));
            let block_before = format!(" {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = insert_offset + block_before.len();
            return Some(self.make_change(
                "Complete class declaration",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_function_declaration(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let fd = self.ast.cast::<FunctionDeclaration>(node)?;
        let data = &self.ast[fd];
        let fe = data.function_expression;
        let body = self.ast[fe].body.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        let is_missing_body = match self.ast.kind(body) {
            NodeKind::EmptyFunctionBody => {
                let efb = self.ast.cast::<EmptyFunctionBody>(body)?;
                self.ast.tokens.get(self.ast[efb].semicolon).is_synthetic()
            }
            NodeKind::BlockFunctionBody => {
                let bfb = self.ast.cast::<BlockFunctionBody>(body)?;
                let blk = self.ast[bfb].block;
                self.ast
                    .tokens
                    .get(self.ast[blk].left_bracket)
                    .is_synthetic()
            }
            _ => false,
        };
        if is_missing_body {
            let insert_offset = self.line_end_without_eol(self.ast.offset(node));
            let block_before = format!(" {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = insert_offset + block_before.len();
            return Some(self.make_change(
                "Complete function declaration",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_method_declaration(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let md = self.ast.cast::<MethodDeclaration>(node)?;
        let data = &self.ast[md];
        let body = data.body.raw();
        let indent = self.line_indent(self.ast.offset(node));
        let eol = self.eol();

        let is_missing_body = match self.ast.kind(body) {
            NodeKind::EmptyFunctionBody => {
                let efb = self.ast.cast::<EmptyFunctionBody>(body)?;
                self.ast.tokens.get(self.ast[efb].semicolon).is_synthetic()
            }
            NodeKind::BlockFunctionBody => {
                let bfb = self.ast.cast::<BlockFunctionBody>(body)?;
                let blk = self.ast[bfb].block;
                self.ast
                    .tokens
                    .get(self.ast[blk].left_bracket)
                    .is_synthetic()
            }
            _ => false,
        };
        if is_missing_body {
            let insert_offset = self.line_end_without_eol(self.ast.offset(node));
            let block_before = format!(" {{{eol}{indent}  ");
            let block_after = format!("{eol}{indent}}}");
            let replacement = format!("{block_before}{block_after}");
            let cursor = insert_offset + block_before.len();
            return Some(self.make_change(
                "Complete method declaration",
                insert_offset,
                0,
                replacement,
                cursor,
            ));
        }
        None
    }

    fn complete_variable_declaration(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let vds = self.ast.cast::<VariableDeclarationStatement>(node)?;
        let semi = self.ast[vds].semicolon;
        if self.ast.tokens.get(semi).is_synthetic() {
            let end_offset = self.ast.end(self.ast[vds].variables.raw()) as usize;
            return Some(self.insert_semicolon(node, end_offset));
        }
        self.append_newline(node)
    }

    fn complete_expression_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let es = self.ast.cast::<ExpressionStatement>(node)?;
        let expr = self.ast[es].expression.raw();
        if self.is_empty_or_synthetic_expr(expr) {
            return None;
        }
        let semi_missing = match self.ast[es].semicolon {
            Some(s) => self.ast.tokens.get(s).is_synthetic(),
            None => true,
        };
        if semi_missing {
            let end_offset = self.ast.end(expr) as usize;
            return Some(self.insert_semicolon(node, end_offset));
        }
        self.append_newline(node)
    }

    fn complete_return_statement(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let rs = self.ast.cast::<ReturnStatement>(node)?;
        let semi = self.ast[rs].semicolon;
        if self.ast.tokens.get(semi).is_synthetic() {
            let end_offset = match self.ast[rs].expression {
                Some(e) => self.ast.end(e.raw()) as usize,
                None => self.ast.tokens.get(self.ast[rs].return_keyword).end() as usize,
            };
            return Some(self.insert_semicolon(node, end_offset));
        }
        self.append_newline(node)
    }

    fn complete_block(&self, node: NodeId) -> Option<protocol::SourceChange> {
        let blk = self.ast.cast::<Block>(node)?;
        let rbracket = self.ast.tokens.get(self.ast[blk].right_bracket);
        if rbracket.is_synthetic() {
            let lbracket = self.ast.tokens.get(self.ast[blk].left_bracket);
            if !lbracket.is_synthetic() {
                let indent = self.line_indent(lbracket.offset);
                let eol = self.eol();
                let insert_offset = lbracket.end() as usize;
                let before = format!("{eol}{indent}  ");
                let after = format!("{eol}{indent}}}");
                let replacement = format!("{before}{after}");
                let cursor = insert_offset + before.len();
                return Some(self.make_change(
                    "Insert a newline at the end of the current line",
                    insert_offset,
                    0,
                    replacement,
                    cursor,
                ));
            }
        }
        None
    }

    fn insert_semicolon(&self, stmt: NodeId, end_offset: usize) -> protocol::SourceChange {
        if self.selection_offset as usize == end_offset {
            self.make_change(
                "Insert a semicolon at the end of the current line",
                end_offset,
                0,
                ";".to_string(),
                end_offset + 1,
            )
        } else {
            let indent = self.line_indent(self.ast.offset(stmt));
            let eol = self.eol();
            let rep = format!(";{eol}{indent}");
            let cursor = end_offset + rep.len();
            self.make_change(
                "Insert a semicolon and a newline at the end of the current line",
                end_offset,
                0,
                rep,
                cursor,
            )
        }
    }

    fn append_newline(&self, _stmt: NodeId) -> Option<protocol::SourceChange> {
        let indent = self.line_indent(self.selection_offset);
        let eol = self.eol();
        let loc = match self.source[self.selection_offset as usize..].find('\n') {
            Some(p) => self.selection_offset as usize + p + 1,
            None => self.source.len(),
        };
        let rep = format!("{indent}{eol}");
        let cursor = loc + indent.len();
        Some(self.make_change(
            "Insert a newline at the end of the current line",
            loc,
            0,
            rep,
            cursor,
        ))
    }
}
