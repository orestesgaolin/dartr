// Dart source: pkg/analysis_server/lib/src/services/completion/postfix/postfix_completion.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_list_postfix_completion_templates.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_is_postfix_completion_applicable.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_postfix_completion.dart

use dartr_ast::{
    Ast, BinaryExpression, Block, Expression, ExpressionStatement, FunctionBody, NodeId, NodeKind,
    NullLiteral, ParenthesizedExpression, PrefixExpression, PropertyAccess, SimpleIdentifier,
    Statement,
};
use dartr_element::{Ctx, ElementId, NoopSink, Nullability, TypeId, TypeKind};
use dartr_syntax::TokenType;
use dartr_typesystem::TypeSystem;

use crate::protocol;
use crate::search::ResolvedUnitRef;

pub struct PostfixTemplateSpec {
    pub name: &'static str,
    pub key: &'static str,
    pub example: &'static str,
}

pub const ALL_TEMPLATES: &[PostfixTemplateSpec] = &[
    PostfixTemplateSpec {
        name: "assert",
        key: ".assert",
        example: "expr.assert -> assert(expr);",
    },
    PostfixTemplateSpec {
        name: "fori",
        key: ".fori",
        example: "limit.fori -> for(var i = 0; i < limit; i++) {}",
    },
    PostfixTemplateSpec {
        name: "for",
        key: ".for",
        example: "values.for -> for(var value in values) {}",
    },
    PostfixTemplateSpec {
        name: "iter",
        key: ".iter",
        example: "values.iter -> for(var value in values) {}",
    },
    PostfixTemplateSpec {
        name: "not",
        key: ".not",
        example: "bool.not -> !bool",
    },
    PostfixTemplateSpec {
        name: "!",
        key: "!",
        example: "bool! -> !bool",
    },
    PostfixTemplateSpec {
        name: "else",
        key: ".else",
        example: "bool.else -> if (!bool) {}",
    },
    PostfixTemplateSpec {
        name: "if",
        key: ".if",
        example: "bool.if -> if (bool) {}",
    },
    PostfixTemplateSpec {
        name: "nn",
        key: ".nn",
        example: "expr.nn -> if (expr != null) {}",
    },
    PostfixTemplateSpec {
        name: "notnull",
        key: ".notnull",
        example: "expr.notnull -> if (expr != null) {}",
    },
    PostfixTemplateSpec {
        name: "null",
        key: ".null",
        example: "expr.null -> if (expr == null) {}",
    },
    PostfixTemplateSpec {
        name: "par",
        key: ".par",
        example: "expr.par -> (expr)",
    },
    PostfixTemplateSpec {
        name: "return",
        key: ".return",
        example: "expr.return -> return expr",
    },
    PostfixTemplateSpec {
        name: "switch",
        key: ".switch",
        example: "expr.switch -> switch (expr) {}",
    },
    PostfixTemplateSpec {
        name: "try",
        key: ".try",
        example: "stmt.try -> try {stmt} catch (e,s) {}",
    },
    PostfixTemplateSpec {
        name: "tryon",
        key: ".tryon",
        example: "stmt.try -> try {stmt} on Exception catch (e,s) {}",
    },
    PostfixTemplateSpec {
        name: "while",
        key: ".while",
        example: "expr.while -> while (expr) {}",
    },
];

pub fn list_postfix_completion_templates() -> protocol::EditListPostfixCompletionTemplatesResult {
    let templates = ALL_TEMPLATES
        .iter()
        .map(|t| protocol::PostfixTemplateDescriptor {
            name: t.name.to_string(),
            key: t.key.to_string(),
            example: t.example.to_string(),
        })
        .collect();
    protocol::EditListPostfixCompletionTemplatesResult { templates }
}

pub fn has_template_key(key: &str) -> bool {
    ALL_TEMPLATES.iter().any(|t| t.key == key)
}

pub fn is_postfix_completion_applicable(
    resolved: &ResolvedUnitRef,
    file: &str,
    key: &str,
    offset: i64,
) -> bool {
    let Ok(selection_offset) = u32::try_from(offset) else {
        return false;
    };
    if !has_template_key(key) {
        return false;
    }
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let proc = PostfixProcessor {
        ctx: &ctx,
        resolved,
        file,
        selection_offset,
    };
    proc.find_target(key).is_some()
}

pub fn get_postfix_completion(
    resolved: &ResolvedUnitRef,
    file: &str,
    key: &str,
    offset: i64,
) -> protocol::SourceChange {
    let empty_change = || protocol::SourceChange {
        message: String::new(),
        edits: Vec::new(),
        linked_edit_groups: Vec::new(),
        selection: None,
        selection_length: None,
        id: None,
    };
    let Ok(selection_offset) = u32::try_from(offset) else {
        return empty_change();
    };
    if !has_template_key(key) {
        return empty_change();
    };
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let proc = PostfixProcessor {
        ctx: &ctx,
        resolved,
        file,
        selection_offset,
    };
    proc.compute(key).unwrap_or_else(empty_change)
}

struct PostfixProcessor<'a> {
    ctx: &'a Ctx<'a>,
    resolved: &'a ResolvedUnitRef,
    file: &'a str,
    selection_offset: u32,
}

impl<'a> PostfixProcessor<'a> {
    fn ast(&self) -> &'a Ast {
        &self.resolved.unit().ast
    }

    fn source(&self) -> &'a str {
        &self.ast().tokens.source
    }

    fn eol(&self) -> &'static str {
        let src = self.source();
        if src.contains("\r\n") { "\r\n" } else { "\n" }
    }

    fn selected_node(&self, at: u32) -> Option<NodeId> {
        let unit = self.resolved.unit();
        unit.ast.node_covering(unit.unit, at, 0)
    }

    fn static_type(&self, expr: NodeId) -> Option<TypeId> {
        self.resolved.unit().tables.static_type.get(expr).copied()
    }

    fn is_bool_non_nullable(&self, ty: TypeId) -> bool {
        ty == self.ctx.tp.bool_type()
    }

    fn is_int_non_nullable(&self, ty: TypeId) -> bool {
        ty == self.ctx.tp.int_type()
    }

    fn is_iterable(&self, ty: TypeId) -> bool {
        let iterable_obj_q = self
            .ctx
            .tp
            .iterable_type(self.ctx, self.ctx.tp.object_question_type());
        TypeSystem::new(*self.ctx).is_subtype_of(ty, iterable_obj_q)
    }

    fn find_target(&self, key: &str) -> Option<NodeId> {
        let node = self.selected_node(self.selection_offset)?;
        match key {
            ".assert" => self.find_assert_expression(node),
            ".fori" => self.find_int_expression(node),
            ".for" | ".iter" => self.find_iterable_expression(node),
            ".not" | "!" | ".else" | ".if" | ".while" => self.find_bool_expression(node),
            ".nn" | ".notnull" | ".null" | ".par" | ".return" | ".switch" => {
                self.find_object_expression(node)
            }
            ".try" | ".tryon" => self.find_statement(node),
            _ => None,
        }
    }

    fn compute(&self, key: &str) -> Option<protocol::SourceChange> {
        let target = self.find_target(key)?;
        match key {
            ".assert" => self.expand_assert(key, target),
            ".fori" => self.expand_fori(key, target),
            ".for" | ".iter" => self.expand_for(key, target),
            ".not" | "!" => self.expand_negate(key, target),
            ".else" => self.expand_else(key, target),
            ".if" => self.expand_if(key, target),
            ".while" => self.expand_while(key, target),
            ".nn" | ".notnull" => self.expand_notnull(key, target),
            ".null" => self.expand_null(key, target),
            ".par" => self.expand_par(key, target),
            ".return" => self.expand_return(key, target),
            ".switch" => self.expand_switch(key, target),
            ".try" => self.expand_try(key, target, false),
            ".tryon" => self.expand_try(key, target, true),
            _ => None,
        }
    }

    fn find_statement(&self, node: NodeId) -> Option<NodeId> {
        let ast = self.ast();
        if !ast.is::<Statement>(node) {
            return None;
        }
        if let Some(es) = ast.cast::<ExpressionStatement>(node) {
            let expr = ast[es].expression.raw();
            if ast.is::<SimpleIdentifier>(expr) {
                return None;
            }
        }
        let mut stmt = node;
        while let Some(parent) = ast.parent(stmt) {
            if !ast.is::<Statement>(parent) || ast.is::<Block>(parent) {
                break;
            }
            if matches!(
                ast.kind(parent),
                NodeKind::DoStatement
                    | NodeKind::ForStatement
                    | NodeKind::IfStatement
                    | NodeKind::LabeledStatement
                    | NodeKind::SwitchStatement
                    | NodeKind::TryStatement
                    | NodeKind::WhileStatement
            ) {
                break;
            }
            stmt = parent;
        }
        if let Some(es) = ast.cast::<ExpressionStatement>(stmt) {
            let semi = ast[es].semicolon?;
            if ast.tokens.get(semi).is_synthetic() {
                return None;
            }
        }
        Some(stmt)
    }

    fn find_outer_expression(
        &self,
        start: NodeId,
        check_type: impl Fn(TypeId) -> bool,
    ) -> Option<NodeId> {
        let ast = self.ast();
        if !ast.is::<Expression>(start) {
            return None;
        }
        if let Some(sid) = ast.cast::<SimpleIdentifier>(start) {
            let u = dartr_server::element_locator::Unit {
                ctx: self.ctx,
                ast,
                tables: &self.resolved.unit().tables,
            };
            if u.element(sid).is_some_and(|el| el == ElementId::DYNAMIC) {
                return None;
            }
        }
        let mut list = Vec::new();
        let mut expr = Some(start);
        while let Some(c) = expr {
            if !ast.is::<Expression>(c) {
                break;
            }
            list.push(c);
            expr = ast.parent(c);
        }
        let found = list.into_iter().find(|&expr| {
            let Some(ty) = self.static_type(expr) else {
                return false;
            };
            check_type(ty)
        })?;
        if ast.is::<SimpleIdentifier>(found)
            && let Some(parent) = ast.parent(found)
            && ast.is::<PropertyAccess>(parent)
            && let Some(ty) = self.static_type(parent)
            && check_type(ty)
        {
            return Some(parent);
        }
        Some(found)
    }

    fn find_bool_expression(&self, node: NodeId) -> Option<NodeId> {
        self.find_outer_expression(node, |ty| self.is_bool_non_nullable(ty))
    }

    fn find_int_expression(&self, node: NodeId) -> Option<NodeId> {
        self.find_outer_expression(node, |ty| self.is_int_non_nullable(ty))
    }

    fn find_iterable_expression(&self, node: NodeId) -> Option<NodeId> {
        self.find_outer_expression(node, |ty| self.is_iterable(ty))
    }

    fn find_object_expression(&self, node: NodeId) -> Option<NodeId> {
        self.find_outer_expression(node, |ty| {
            !matches!(
                self.ctx.ty(ty),
                TypeKind::Void | TypeKind::Dynamic | TypeKind::Invalid
            )
        })
    }

    fn find_assert_expression(&self, node: NodeId) -> Option<NodeId> {
        let ast = self.ast();
        if !ast.is::<Expression>(node) {
            return None;
        }
        let expr = self.find_outer_expression(node, |ty| {
            if self.is_bool_non_nullable(ty) {
                return true;
            }
            if let TypeKind::Function(f) = self.ctx.ty(ty)
                && f.nullability == Nullability::None
                && self.is_bool_non_nullable(f.ret)
            {
                return true;
            }
            false
        })?;
        let parent = ast.parent(expr)?;
        if ast
            .cast::<ExpressionStatement>(parent)
            .is_some_and(|es| ast[es].expression.raw() == expr)
        {
            let ty = self.static_type(expr)?;
            if self.is_bool_non_nullable(ty) {
                return Some(expr);
            }
            if let TypeKind::Function(f) = self.ctx.ty(ty)
                && f.nullability == Nullability::None
                && self.is_bool_non_nullable(f.ret)
            {
                return Some(expr);
            }
        }
        None
    }

    fn node_text(&self, node: NodeId) -> String {
        let ast = self.ast();
        let start = ast.offset(node) as usize;
        let end = ast.end(node) as usize;
        let src = self.source();
        if start <= end && end <= src.len() {
            src[start..end].to_string()
        } else {
            String::new()
        }
    }

    fn line_prefix(&self, offset: u32) -> String {
        let src = self.source();
        let idx = (offset as usize).min(src.len());
        let line_start = src[..idx].rfind('\n').map_or(0, |p| p + 1);
        src[line_start..idx]
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect()
    }

    fn expr_with_stmt(&self, expr: NodeId) -> NodeId {
        let ast = self.ast();
        if let Some(parent) = ast.parent(expr)
            && ast.is::<ExpressionStatement>(parent)
        {
            parent
        } else {
            expr
        }
    }

    fn negate_text(&self, expr: NodeId) -> String {
        let ast = self.ast();
        if let Some(pe) = ast.cast::<ParenthesizedExpression>(expr) {
            let inner = ast[pe].expression.raw();
            if let Some(bin) = ast.cast::<BinaryExpression>(inner) {
                let op = ast.tokens.get(ast[bin].operator).ty;
                if op == TokenType::EQ_EQ || op == TokenType::BANG_EQ {
                    return self.negate_text(inner);
                }
            }
        }
        if let Some(bin) = ast.cast::<BinaryExpression>(expr) {
            let op_tok = ast.tokens.get(ast[bin].operator);
            if op_tok.ty == TokenType::EQ_EQ {
                let left = self.node_text(ast[bin].left_operand.raw());
                let right = self.node_text(ast[bin].right_operand.raw());
                return format!("{left} != {right}");
            } else if op_tok.ty == TokenType::BANG_EQ {
                let left = self.node_text(ast[bin].left_operand.raw());
                let right = self.node_text(ast[bin].right_operand.raw());
                return format!("{left} == {right}");
            }
        }
        if let Some(pre) = ast.cast::<PrefixExpression>(expr)
            && ast.tokens.get(ast[pre].operator).ty == TokenType::BANG
        {
            return self.node_text(ast[pre].operand.raw());
        }
        let txt = self.node_text(expr);
        if matches!(
            ast.kind(expr),
            NodeKind::BinaryExpression
                | NodeKind::ConditionalExpression
                | NodeKind::CascadeExpression
                | NodeKind::IsExpression
                | NodeKind::AsExpression
        ) {
            format!("!({txt})")
        } else {
            format!("!{txt}")
        }
    }

    fn replace_node(
        &self,
        key: &str,
        target: NodeId,
        replacement: String,
        cursor_offset_in_replacement: usize,
    ) -> Option<protocol::SourceChange> {
        let ast = self.ast();
        let start = ast.offset(target) as i64;
        let length = ast.length(target) as i64;
        let sel_offset = start + cursor_offset_in_replacement as i64;
        Some(protocol::SourceChange {
            message: format!("Expand {key}"),
            edits: vec![protocol::SourceFileEdit {
                file: self.file.to_string(),
                file_stamp: 0,
                edits: vec![protocol::SourceEdit {
                    offset: start,
                    length,
                    replacement,
                    id: None,
                    description: None,
                }],
            }],
            linked_edit_groups: Vec::new(),
            selection: Some(protocol::Position {
                file: self.file.to_string(),
                offset: sel_offset,
            }),
            selection_length: Some(0),
            id: None,
        })
    }

    fn expand_assert(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let txt = self.node_text(expr);
        let rep = format!("assert({txt});");
        let len = rep.len();
        self.replace_node(key, target, rep, len)
    }

    fn expand_negate(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let rep = self.negate_text(expr);
        let len = rep.len();
        self.replace_node(key, expr, rep, len)
    }

    fn expand_par(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let txt = self.node_text(expr);
        let rep = format!("({txt})");
        let len = rep.len();
        self.replace_node(key, expr, rep, len)
    }

    fn expand_return(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let txt = self.node_text(expr);
        let rep = format!("return {txt};");
        let len = rep.len();
        self.replace_node(key, target, rep, len)
    }

    fn expand_else(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let cond = self.negate_text(expr);
        let before_cursor = format!("if ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_if(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let cond = self.node_text(expr);
        let before_cursor = format!("if ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_while(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let cond = self.node_text(expr);
        let before_cursor = format!("while ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_notnull(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let ast = self.ast();
        let cond = if ast.is::<NullLiteral>(expr) {
            "false".to_string()
        } else {
            let txt = self.node_text(expr);
            format!("{txt} != null")
        };
        let before_cursor = format!("if ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_null(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let ast = self.ast();
        let cond = if ast.is::<NullLiteral>(expr) {
            "true".to_string()
        } else {
            let txt = self.node_text(expr);
            format!("{txt} == null")
        };
        let before_cursor = format!("if ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_switch(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let indent = self.line_prefix(self.ast().offset(target));
        let eol = self.eol();
        let cond = self.node_text(expr);
        let before_cursor = format!("switch ({cond}) {{{eol}{indent}  ");
        let cursor = before_cursor.len();
        let rep = format!("{before_cursor}{eol}{indent}}}");
        self.replace_node(key, target, rep, cursor)
    }

    fn expand_fori(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let ast = self.ast();
        let start = ast.offset(target) as i64;
        let length = ast.length(target) as i64;
        let indent = self.line_prefix(ast.offset(target));
        let eol = self.eol();
        let limit = self.node_text(expr);
        let var_name = self.new_variable_name(ast.offset(target), "i");
        let p1 = "for (var ";
        let p2 = " = 0; ";
        let p3 = format!(" < {limit}; ");
        let p4 = format!("++) {{{eol}{indent}  ");
        let p5 = format!("{eol}{indent}}}");

        let pos1 = start + p1.len() as i64;
        let pos2 = pos1 + var_name.len() as i64 + p2.len() as i64;
        let pos3 = pos2 + var_name.len() as i64 + p3.len() as i64;
        let cursor = pos3 + var_name.len() as i64 + p4.len() as i64;
        let replacement = format!("{p1}{var_name}{p2}{var_name}{p3}{var_name}{p4}{p5}");

        Some(protocol::SourceChange {
            message: format!("Expand {key}"),
            edits: vec![protocol::SourceFileEdit {
                file: self.file.to_string(),
                file_stamp: 0,
                edits: vec![protocol::SourceEdit {
                    offset: start,
                    length,
                    replacement,
                    id: None,
                    description: None,
                }],
            }],
            linked_edit_groups: vec![protocol::LinkedEditGroup {
                positions: vec![
                    protocol::Position {
                        file: self.file.to_string(),
                        offset: pos1,
                    },
                    protocol::Position {
                        file: self.file.to_string(),
                        offset: pos2,
                    },
                    protocol::Position {
                        file: self.file.to_string(),
                        offset: pos3,
                    },
                ],
                length: var_name.len() as i64,
                suggestions: Vec::new(),
            }],
            selection: Some(protocol::Position {
                file: self.file.to_string(),
                offset: cursor,
            }),
            selection_length: Some(0),
            id: None,
        })
    }

    fn expand_for(&self, key: &str, expr: NodeId) -> Option<protocol::SourceChange> {
        let target = self.expr_with_stmt(expr);
        let ast = self.ast();
        let start = ast.offset(target) as i64;
        let length = ast.length(target) as i64;
        let indent = self.line_prefix(ast.offset(target));
        let eol = self.eol();
        let iter_txt = self.node_text(expr);
        let var_name = self.new_variable_name(ast.offset(target), "value");
        let p1 = "for (var ";
        let p2 = format!(" in {iter_txt}) {{{eol}{indent}  ");
        let p3 = format!("{eol}{indent}}}");

        let pos1 = start + p1.len() as i64;
        let cursor = pos1 + var_name.len() as i64 + p2.len() as i64;
        let replacement = format!("{p1}{var_name}{p2}{p3}");

        Some(protocol::SourceChange {
            message: format!("Expand {key}"),
            edits: vec![protocol::SourceFileEdit {
                file: self.file.to_string(),
                file_stamp: 0,
                edits: vec![protocol::SourceEdit {
                    offset: start,
                    length,
                    replacement,
                    id: None,
                    description: None,
                }],
            }],
            linked_edit_groups: vec![protocol::LinkedEditGroup {
                positions: vec![protocol::Position {
                    file: self.file.to_string(),
                    offset: pos1,
                }],
                length: var_name.len() as i64,
                suggestions: Vec::new(),
            }],
            selection: Some(protocol::Position {
                file: self.file.to_string(),
                offset: cursor,
            }),
            selection_length: Some(0),
            id: None,
        })
    }

    fn expand_try(&self, key: &str, stmt: NodeId, with_on: bool) -> Option<protocol::SourceChange> {
        let ast = self.ast();
        let line_info = self.resolved.line_info();
        let start_line = (line_info.get_location(ast.offset(stmt)).line_number - 1) as usize;
        let mut end_line = (line_info.get_location(ast.end(stmt)).line_number - 1) as usize;
        if let Some(es) = ast.cast::<ExpressionStatement>(stmt)
            && let Some(semi) = ast[es].semicolon
            && !ast.tokens.get(semi).is_synthetic()
        {
            end_line += 1;
        }
        let start_offset = line_info.line_starts.get(start_line).copied().unwrap_or(0) as usize;
        let src = self.source();
        let end_offset = line_info
            .line_starts
            .get(end_line)
            .copied()
            .map(|o| o as usize)
            .unwrap_or(src.len());
        let inner_src = &src[start_offset..end_offset];
        let indent = self.line_prefix(ast.offset(stmt));
        let eol = self.eol();

        let indented_inner = if inner_src.is_empty() {
            String::new()
        } else {
            let mut s = inner_src
                .split_inclusive('\n')
                .map(|line| {
                    if line.trim().is_empty() {
                        line.to_string()
                    } else {
                        format!("  {line}")
                    }
                })
                .collect::<String>();
            if !s.ends_with('\n') {
                s.push_str(eol);
            }
            s
        };
        let before_select = format!("{indent}try {{{eol}{indented_inner}");
        let sel_offset = start_offset as i64 + before_select.len() as i64;
        let p1 = format!("{before_select}{indent}}}");
        let mut linked_edit_groups = Vec::new();
        let middle = if with_on {
            let on_part = " on ";
            let exc = "Exception";
            let after_exc = format!(" catch (e, s) {{{eol}{indent}  print(s);");
            let exc_pos = start_offset as i64 + p1.len() as i64 + on_part.len() as i64;
            linked_edit_groups.push(protocol::LinkedEditGroup {
                positions: vec![protocol::Position {
                    file: self.file.to_string(),
                    offset: exc_pos,
                }],
                length: exc.len() as i64,
                suggestions: Vec::new(),
            });
            format!("{on_part}{exc}{after_exc}")
        } else {
            format!(" catch (e, s) {{{eol}{indent}  print(s);")
        };
        let replacement = format!("{p1}{middle}{eol}{indent}}}{eol}");

        Some(protocol::SourceChange {
            message: format!("Expand {key}"),
            edits: vec![protocol::SourceFileEdit {
                file: self.file.to_string(),
                file_stamp: 0,
                edits: vec![protocol::SourceEdit {
                    offset: start_offset as i64,
                    length: (end_offset - start_offset) as i64,
                    replacement,
                    id: None,
                    description: None,
                }],
            }],
            linked_edit_groups,
            selection: Some(protocol::Position {
                file: self.file.to_string(),
                offset: sel_offset,
            }),
            selection_length: Some(0),
            id: None,
        })
    }

    fn new_variable_name(&self, offset: u32, base: &str) -> String {
        let ast = self.ast();
        // Collect identifier tokens in the enclosing function body / declaration
        let mut enclosing = self.selected_node(offset);
        while let Some(n) = enclosing {
            if ast.is::<FunctionBody>(n) {
                break;
            }
            enclosing = ast.parent(n);
        }
        let mut used = std::collections::HashSet::new();
        if let Some(body) = enclosing {
            let mut tok = ast.begin_token(body);
            let end_tok = ast.end_token(body);
            loop {
                let t = ast.tokens.get(tok);
                if t.is_identifier() {
                    used.insert(ast.tokens.lexeme(tok).to_string());
                }
                if tok == end_tok || t.ty == TokenType::EOF {
                    break;
                }
                tok = t.next;
            }
        }
        if !used.contains(base) {
            return base.to_string();
        }
        let mut idx = 1;
        loop {
            let cand = format!("{base}{idx}");
            if !used.contains(&cand) {
                return cand;
            }
            idx += 1;
        }
    }
}
