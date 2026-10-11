// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/inline_local.dart

//! `InlineLocalRefactoring`: inlines a local variable's initializer into all of
//! its references and removes the variable declaration statement.

use dartr_ast::*;
use dartr_element::{ElementId, NoopSink, Tag};
use dartr_syntax::TokenType;

use super::host::RefactoringHost;
use super::status::{Location, RefactoringStatus};
use crate::completion::candidate::display_name;
use crate::correction::change::SourceChange;
use crate::correction::change_builder::{ChangeBuilder, end_of_line};
use crate::correction::utils::{CorrectionUtils, Range, RangeFactory};
use crate::element_locator::{Unit, get_element};
use crate::server::ResolvedUnitRef;

/// Dart `InlineLocalRefactoringImpl`.
pub struct InlineLocal {
    pub unit: ResolvedUnitRef,
    pub offset: u32,
    pub variable_name: Option<String>,
    pub reference_count: usize,
    variable_element: Option<ElementId>,
    variable_node: Option<Id<VariableDeclaration>>,
    initializer_node: Option<NodeId>,
    declaration_statement: Option<Id<VariableDeclarationStatement>>,
    references: Vec<Range>,
}

impl InlineLocal {
    pub fn new(unit: &ResolvedUnitRef, offset: u32) -> Self {
        Self {
            unit: unit.clone(),
            offset,
            variable_name: None,
            reference_count: 0,
            variable_element: None,
            variable_node: None,
            initializer_node: None,
            declaration_statement: None,
            references: Vec::new(),
        }
    }

    pub fn is_available(unit: &ResolvedUnitRef, offset: u32) -> bool {
        let r = Self::new(unit, offset);
        !r.check_offset().has_fatal_error()
    }

    fn check_offset(&self) -> RefactoringStatus {
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(offset_node) = ast.node_covering(unit.unit, self.offset, 0) else {
            return no_local_variable_status();
        };
        let element = if ast.is::<SimpleIdentifier>(offset_node) {
            get_element(&u, offset_node)
        } else if let Some(vd) = ast.cast::<VariableDeclaration>(offset_node) {
            u.declared_element(vd)
        } else {
            None
        };
        let Some(el) = element else {
            return no_local_variable_status();
        };
        if el.tag() != Tag::LocalVariable {
            return no_local_variable_status();
        }
        RefactoringStatus::new()
    }

    pub fn check_initial_conditions(&mut self) -> RefactoringStatus {
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let file_path = self.unit.path().to_string();

        let Some(offset_node) = ast.node_covering(unit.unit, self.offset, 0) else {
            return no_local_variable_status();
        };
        let element = if ast.is::<SimpleIdentifier>(offset_node) {
            get_element(&u, offset_node)
        } else if let Some(vd) = ast.cast::<VariableDeclaration>(offset_node) {
            u.declared_element(vd)
        } else {
            None
        };
        let Some(el) = element else {
            return no_local_variable_status();
        };
        if el.tag() != Tag::LocalVariable {
            return no_local_variable_status();
        }

        // Find declaring VariableDeclaration and all references in enclosing unit
        let mut decl_node: Option<Id<VariableDeclaration>> = None;
        let mut refs: Vec<(Id<SimpleIdentifier>, bool)> = Vec::new();
        let mut stack = vec![unit.unit.raw()];
        while let Some(cur) = stack.pop() {
            if let Some(vd) = ast.cast::<VariableDeclaration>(cur)
                && u.declared_element(vd) == Some(el)
            {
                decl_node = Some(vd);
            } else if let Some(sid) = ast.cast::<SimpleIdentifier>(cur)
                && u.element(sid) == Some(el)
            {
                let is_write = is_write_reference(ast, sid);
                refs.push((sid, is_write));
            } else if let Some(avp) = ast.cast::<AssignedVariablePattern>(cur)
                && u.element(avp) == Some(el)
            {
                let mut st = RefactoringStatus::new();
                st.add_fatal_error(
                    format!(
                        "Local variable '{}' is assigned more than once.",
                        display_name(&ctx, el)
                    ),
                    Some(Location {
                        file: file_path,
                        offset: ast.offset(avp.raw()),
                        length: ast.length(avp.raw()),
                    }),
                );
                return st;
            }
            let children = ast.children(cur);
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }

        let Some(vd) = decl_node else {
            return no_local_variable_status();
        };
        let Some(decl_stmt) = declaration_statement(ast, vd) else {
            return no_local_variable_status();
        };
        let var_name = display_name(&ctx, el);
        let Some(init) = ast[vd].initializer else {
            let mut st = RefactoringStatus::new();
            st.add_fatal_error(
                format!("Local variable '{var_name}' is not initialized at declaration."),
                Some(Location {
                    file: file_path,
                    offset: ast.offset(vd.raw()),
                    length: ast.length(vd.raw()),
                }),
            );
            return st;
        };

        for &(sid, is_write) in &refs {
            if is_write {
                let tok = ast.tokens.get(ast[sid].token);
                let mut st = RefactoringStatus::new();
                st.add_fatal_error(
                    format!("Local variable '{var_name}' is assigned more than once."),
                    Some(Location {
                        file: file_path,
                        offset: tok.offset,
                        length: tok.length,
                    }),
                );
                return st;
            }
        }

        let ref_ranges: Vec<Range> = refs
            .into_iter()
            .map(|(sid, _)| {
                let tok = ast.tokens.get(ast[sid].token);
                Range::new(tok.offset, tok.length)
            })
            .collect();

        self.variable_name = Some(var_name);
        self.reference_count = ref_ranges.len();
        self.variable_element = Some(el);
        self.variable_node = Some(vd);
        self.initializer_node = Some(init.raw());
        self.declaration_statement = Some(decl_stmt);
        self.references = ref_ranges;
        RefactoringStatus::new()
    }

    pub fn check_final_conditions(&self) -> RefactoringStatus {
        RefactoringStatus::new()
    }

    pub fn check_all_conditions(&mut self) -> RefactoringStatus {
        let mut result = self.check_initial_conditions();
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_final_conditions());
        result
    }

    pub fn create_change(&self, host: &mut dyn RefactoringHost) -> SourceChange {
        let path = self.unit.path().to_string();
        let source = &self.unit.unit().ast.tokens.source;
        let default_eol = end_of_line(source);
        let mut out = SourceChange::default();
        host.with_change_workspace(&mut |workspace| {
            let mut builder = ChangeBuilder::new(workspace, default_eol);
            self.build_change(&mut builder, &path);
            out = builder.source_change();
            out.message = "Inline Local Variable".to_string();
        });
        out
    }

    fn build_change(&self, builder: &mut ChangeBuilder<'_>, path: &str) {
        let (Some(decl_stmt), Some(initializer)) =
            (self.declaration_statement, self.initializer_node)
        else {
            return;
        };
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let utils = CorrectionUtils::new(ast, self.unit.line_info());
        let rf = RangeFactory::new(ast);

        let stmt_range = rf.node(decl_stmt);
        let del_range = utils.get_lines_range(stmt_range, false);
        let initializer_code = utils.get_node_text(initializer);

        builder.add_dart_file_edit(path, |b| {
            b.add_deletion(del_range.offset, del_range.length);

            for &ref_range in &self.references {
                let mut edit_range = ref_range;
                let Some(node) = utils.find_node(unit.unit.raw(), edit_range.offset) else {
                    continue;
                };
                let parent = ast.parent(node);

                let code_for_ref = if let Some(p) = parent
                    && let Some(interp_expr) = ast.cast::<InterpolationExpression>(p)
                {
                    let target = ast.parent(p);
                    if let Some(t) = target
                        && let Some(target_si) = ast.cast::<StringInterpolation>(t)
                        && let Some((
                            init_contents_off,
                            init_contents_end,
                            init_raw,
                            init_sq,
                            init_ml,
                        )) = single_string_literal_info(ast, initializer)
                        && !init_raw
                        && {
                            let (_, _, _, target_sq, target_ml) =
                                string_interpolation_quote_info(ast, target_si);
                            init_sq == target_sq && (!init_ml || target_ml)
                        }
                    {
                        edit_range = rf.node(p);
                        utils.get_text(init_contents_off, init_contents_end - init_contents_off)
                    } else if should_be_expression_interpolation(ast, interp_expr, initializer) {
                        format!("{{{initializer_code}}}")
                    } else {
                        initializer_code.clone()
                    }
                } else if should_use_parenthesis(ast, initializer, node) {
                    format!("({initializer_code})")
                } else {
                    initializer_code.clone()
                };

                b.add_simple_replacement(edit_range.offset, edit_range.length, &code_for_ref);
            }
        });
    }
}

fn no_local_variable_status() -> RefactoringStatus {
    RefactoringStatus::fatal(
        "Local variable declaration or reference must be selected to activate this refactoring.",
    )
}

fn declaration_statement(
    ast: &Ast,
    vd: Id<VariableDeclaration>,
) -> Option<Id<VariableDeclarationStatement>> {
    let vdl = ast.parent(vd.raw())?;
    if !ast.is::<VariableDeclarationList>(vdl) {
        return None;
    }
    let stmt = ast.parent(vdl)?;
    let vds = ast.cast::<VariableDeclarationStatement>(stmt)?;
    let parent = ast.parent(stmt)?;
    if ast.is::<Block>(parent)
        || ast.is::<SwitchCase>(parent)
        || ast.is::<SwitchPatternCase>(parent)
    {
        Some(vds)
    } else {
        None
    }
}

fn is_write_reference(ast: &Ast, sid: Id<SimpleIdentifier>) -> bool {
    let node = sid.raw();
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(assign) = ast.cast::<AssignmentExpression>(parent)
        && ast[assign].left_hand_side.raw() == node
    {
        return true;
    }
    if let Some(pre) = ast.cast::<PrefixExpression>(parent)
        && ast[pre].operand.raw() == node
        && matches!(
            ast.tokens.ty(ast[pre].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        )
    {
        return true;
    }
    if let Some(post) = ast.cast::<PostfixExpression>(parent)
        && ast[post].operand.raw() == node
        && matches!(
            ast.tokens.ty(ast[post].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        )
    {
        return true;
    }
    false
}

fn should_be_expression_interpolation(
    ast: &Ast,
    target: Id<InterpolationExpression>,
    expr: NodeId,
) -> bool {
    let begin_ty = ast.tokens.ty(ast[target].left_bracket);
    begin_ty == TokenType::STRING_INTERPOLATION_IDENTIFIER && !ast.is::<SimpleIdentifier>(expr)
}

fn single_string_literal_info(ast: &Ast, node: NodeId) -> Option<(u32, u32, bool, bool, bool)> {
    if let Some(ssl) = ast.cast::<SimpleStringLiteral>(node) {
        let tok = ast.tokens.get(ast[ssl].literal);
        let lex = ast.tokens.lexeme(ast[ssl].literal);
        let (raw, sq, ml, quote_len) = parse_quote_prefix(lex);
        let contents_off = tok.offset + (if raw { 1 } else { 0 }) + quote_len;
        let contents_end = tok.end().saturating_sub(quote_len);
        return Some((contents_off, contents_end, raw, sq, ml));
    }
    if let Some(si) = ast.cast::<StringInterpolation>(node) {
        return Some(string_interpolation_quote_info(ast, si));
    }
    None
}

fn string_interpolation_quote_info(
    ast: &Ast,
    si: Id<StringInterpolation>,
) -> (u32, u32, bool, bool, bool) {
    let elements = ast.list(ast[si].elements);
    let first_lex = elements
        .first()
        .and_then(|&e| ast.cast::<InterpolationString>(e))
        .map(|is| ast.tokens.lexeme(ast[is].contents))
        .unwrap_or("'");
    let (raw, sq, ml, quote_len) = parse_quote_prefix(first_lex);
    let start = ast.offset(si.raw()) + (if raw { 1 } else { 0 }) + quote_len;
    let end = ast.end(si.raw()).saturating_sub(quote_len);
    (start, end, raw, sq, ml)
}

fn parse_quote_prefix(mut lex: &str) -> (bool, bool, bool, u32) {
    let raw = if let Some(rest) = lex.strip_prefix('r').or_else(|| lex.strip_prefix('R')) {
        lex = rest;
        true
    } else {
        false
    };
    if lex.starts_with("'''") {
        (raw, true, true, 3)
    } else if lex.starts_with("\"\"\"") {
        (raw, false, true, 3)
    } else if lex.starts_with('\'') {
        (raw, true, false, 1)
    } else {
        (raw, false, false, 1)
    }
}

pub(crate) fn should_use_parenthesis(ast: &Ast, init: NodeId, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(sw) = ast.cast::<SwitchExpression>(parent)
        && ast[sw].expression.raw() == node
    {
        return false;
    }
    if matches!(
        ast.kind(parent),
        NodeKind::RecordLiteral | NodeKind::ListLiteral | NodeKind::SetOrMapLiteral
    ) {
        return false;
    }
    let init_prec = expression_precedence(ast, init);
    let parent_prec = expression_parent_precedence(ast, node);
    if init_prec < parent_prec {
        return true;
    }
    if let (Some(init_pre), Some(parent_pre)) = (
        ast.cast::<PrefixExpression>(init),
        ast.cast::<PrefixExpression>(parent),
    ) && ast.tokens.ty(ast[parent_pre].operator) == TokenType::MINUS
    {
        let init_op = ast.tokens.ty(ast[init_pre].operator);
        if matches!(init_op, TokenType::MINUS | TokenType::MINUS_MINUS) {
            return true;
        }
    }
    false
}

fn expression_parent_precedence(ast: &Ast, node: NodeId) -> u8 {
    let Some(parent) = ast.parent(node) else {
        return 0;
    };
    if ast.is::<ParenthesizedExpression>(parent) {
        return 1; // Precedence.assignment
    }
    if let Some(idx) = ast.cast::<IndexExpression>(parent)
        && ast[idx].index.raw() == node
    {
        return 1; // Precedence.assignment
    }
    if let Some(assign) = ast.cast::<AssignmentExpression>(parent)
        && ast[assign].right_hand_side.raw() == node
        && ast
            .parent(parent)
            .is_some_and(|pp| ast.is::<CascadeExpression>(pp))
    {
        return 3; // Precedence.conditional
    }
    expression_precedence(ast, parent)
}

pub(crate) fn expression_precedence(ast: &Ast, node: NodeId) -> u8 {
    if !ast.is::<Expression>(node) {
        return 0;
    }
    if let Some(bin) = ast.cast::<BinaryExpression>(node) {
        return binary_token_precedence(ast.tokens.ty(ast[bin].operator));
    }
    match ast.kind(node) {
        NodeKind::AssignmentExpression | NodeKind::ThrowExpression => 1,
        NodeKind::CascadeExpression => 2,
        NodeKind::ConditionalExpression
        | NodeKind::FunctionExpression
        | NodeKind::SwitchExpression => 3,
        NodeKind::IsExpression | NodeKind::AsExpression => 8,
        NodeKind::PrefixExpression | NodeKind::AwaitExpression => 14,
        NodeKind::PostfixExpression
        | NodeKind::MethodInvocation
        | NodeKind::PropertyAccess
        | NodeKind::PrefixedIdentifier
        | NodeKind::IndexExpression
        | NodeKind::FunctionExpressionInvocation => 15,
        _ => 16,
    }
}

fn binary_token_precedence(ty: TokenType) -> u8 {
    match ty {
        TokenType::QUESTION_QUESTION => 4,
        TokenType::BAR_BAR => 5,
        TokenType::AMPERSAND_AMPERSAND => 6,
        TokenType::EQ_EQ | TokenType::BANG_EQ => 7,
        TokenType::LT | TokenType::LT_EQ | TokenType::GT | TokenType::GT_EQ => 8,
        TokenType::BAR => 9,
        TokenType::CARET => 10,
        TokenType::AMPERSAND => 11,
        TokenType::LT_LT | TokenType::GT_GT | TokenType::GT_GT_GT => 12,
        TokenType::PLUS | TokenType::MINUS => 13,
        TokenType::STAR | TokenType::SLASH | TokenType::TILDE_SLASH | TokenType::PERCENT => 14,
        _ => 13,
    }
}
