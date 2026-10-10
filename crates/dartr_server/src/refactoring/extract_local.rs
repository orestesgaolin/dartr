// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/extract_local.dart

//! `ExtractLocalRefactoring`: extracts a selected expression or string literal
//! part into a local variable declaration.

use std::collections::{HashMap, HashSet};

use dartr_ast::*;
use dartr_element::{AnyElement, ElementId, NoopSink, Tag, TypeKind};
use dartr_format::text::{substring_utf16, utf16_len};
use dartr_syntax::{Keyword, TokenType};

use super::host::RefactoringHost;
use super::naming;
use super::status::{Location, RefactoringStatus};
use crate::correction::change::{LinkedEditSuggestionKind, SourceChange};
use crate::correction::change_builder::{ChangeBuilder, end_of_line};
use crate::correction::utils::CorrectionUtils;
use crate::element_locator::{Unit, get_element};
use crate::server::ResolvedUnitRef;

const TOKEN_SEPARATOR: &str = "\u{FFFF}";

/// Dart `ExtractLocalRefactoringImpl`.
pub struct ExtractLocal {
    pub unit: ResolvedUnitRef,
    pub selection_offset: u32,
    pub selection_length: u32,
    pub selection_range: (u32, u32),
    pub name: String,
    pub extract_all: bool,
    pub covering_expression_offsets: Vec<u32>,
    pub covering_expression_lengths: Vec<u32>,
    pub names: Vec<String>,
    pub offsets: Vec<u32>,
    pub lengths: Vec<u32>,
    pub covering_function_body: Option<NodeId>,
    pub single_expression: Option<NodeId>,
    pub string_literal_part: Option<String>,
    pub excluded_variable_names: HashSet<String>,
    pub occurrences: Vec<(u32, u32)>,
}

impl ExtractLocal {
    pub fn new(unit: &ResolvedUnitRef, selection_offset: u32, selection_length: u32) -> Self {
        Self {
            unit: unit.clone(),
            selection_offset,
            selection_length,
            selection_range: (selection_offset, selection_length),
            name: String::new(),
            extract_all: true,
            covering_expression_offsets: Vec::new(),
            covering_expression_lengths: Vec::new(),
            names: Vec::new(),
            offsets: Vec::new(),
            lengths: Vec::new(),
            covering_function_body: None,
            single_expression: None,
            string_literal_part: None,
            excluded_variable_names: HashSet::new(),
            occurrences: Vec::new(),
        }
    }

    pub fn is_available(
        unit: &ResolvedUnitRef,
        selection_offset: u32,
        selection_length: u32,
    ) -> bool {
        let mut r = Self::new(unit, selection_offset, selection_length);
        !r.check_selection().has_fatal_error()
    }

    pub fn check_initial_conditions(&mut self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(self.check_selection());
        if result.has_fatal_error() {
            return result;
        }
        self.prepare_occurrences();
        self.prepare_offsets_lengths();
        self.excluded_variable_names = self.find_possible_local_variable_conflicts();
        self.prepare_names();
        result
    }

    pub fn check_name(&self) -> RefactoringStatus {
        let mut result = naming::validate_variable_name(&self.name);
        if self.excluded_variable_names.contains(&self.name) {
            result.add_error(
                format!("The name '{}' is already used in the scope.", self.name),
                None,
            );
        }
        result
    }

    pub fn check_final_conditions(&self) -> RefactoringStatus {
        self.check_name()
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
            out.message = "Extract Local Variable".to_string();
        });
        out
    }

    fn build_change(&self, builder: &mut ChangeBuilder<'_>, path: &str) {
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let utils = CorrectionUtils::new(ast, self.unit.line_info());

        let mut occurrences = if self.extract_all {
            self.occurrences.clone()
        } else {
            vec![self.selection_range]
        };
        occurrences.sort_by_key(|r| r.0);

        let keyword_and_type = self.declaration_keyword_and_type();

        if let Some(single_expr) = self.single_expression
            && ast
                .parent(single_expr)
                .is_some_and(|p| ast.is::<ExpressionStatement>(p))
            && occurrences.len() == 1
        {
            let decl_code = format!("{keyword_and_type} {} = ", self.name);
            let expr_offset = ast.offset(single_expr);
            builder.add_dart_file_edit(path, |b| {
                b.add_simple_insertion(expr_offset, &decl_code);
            });
            return;
        }

        let declaration_prefix = format!("{keyword_and_type} ");
        let declaration_initializer = if let Some(part) = &self.string_literal_part {
            format!(" = '{part}';")
        } else {
            let init_code = substring_utf16(
                source,
                self.selection_range.0 as usize,
                (self.selection_range.0 + self.selection_range.1) as usize,
            );
            format!(" = {init_code};")
        };

        let target = self.find_declaration_target(&occurrences);
        let eol = &utils.end_of_line;

        builder.add_dart_file_edit(path, |b| {
            if let Some(target_node) = target {
                if ast.is::<Statement>(target_node) {
                    let prefix = utils.get_node_prefix(target_node);
                    let target_offset = ast.offset(target_node);
                    b.add_insertion(target_offset, |eb| {
                        eb.write(&declaration_prefix);
                        eb.add_simple_linked_edit(
                            "variableName",
                            &self.name,
                            Some((LinkedEditSuggestionKind::Variable, &self.names)),
                        );
                        eb.write(&format!("{declaration_initializer}{eol}{prefix}"));
                    });
                } else if let Some(efb) = ast.cast::<ExpressionFunctionBody>(target_node) {
                    let prefix = ast
                        .parent(target_node)
                        .map(|p| utils.get_node_prefix(p))
                        .unwrap_or_default();
                    let indent = "  ";
                    let expr = ast[efb].expression.raw();
                    let target_start = ast.offset(target_node);
                    let expr_start = ast.offset(expr);
                    let expr_end = ast.end(expr);
                    let target_end = ast.end(target_node);
                    b.add_replacement(target_start, expr_start - target_start, |eb| {
                        eb.write(&format!("{{{eol}{prefix}{indent}{declaration_prefix}"));
                        eb.add_simple_linked_edit(
                            "variableName",
                            &self.name,
                            Some((LinkedEditSuggestionKind::Variable, &self.names)),
                        );
                        eb.write(&declaration_initializer);
                        eb.write(&format!("{eol}{prefix}{indent}return "));
                    });
                    b.add_simple_replacement(
                        expr_end,
                        target_end - expr_end,
                        &format!(";{eol}{prefix}}}"),
                    );
                }
            }

            let (occ_prefix, occ_suffix) = if self.string_literal_part.is_some() {
                ("${", "}")
            } else {
                ("", "")
            };
            for &(occ_off, occ_len) in &occurrences {
                b.add_replacement(occ_off, occ_len, |eb| {
                    eb.write(occ_prefix);
                    eb.add_simple_linked_edit("variableName", &self.name, None);
                    eb.write(occ_suffix);
                });
            }
        });
    }

    fn declaration_keyword_and_type(&self) -> &'static str {
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let use_const = self.string_literal_part.is_none()
            && self
                .single_expression
                .is_some_and(|e| in_constant_context(ast, e));
        if use_const { "const" } else { "var" }
    }

    fn check_selection(&mut self) -> RefactoringStatus {
        self.single_expression = None;
        self.string_literal_part = None;
        self.covering_expression_offsets.clear();
        self.covering_expression_lengths.clear();
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let content_len = utf16_len(source) as u32;

        if self.selection_offset == 0 {
            return RefactoringStatus::fatal("The selection offset must be greater than zero.");
        }
        if self.selection_offset + self.selection_length >= content_len {
            return RefactoringStatus::fatal(
                "The selection end offset must be less than the length of the file.",
            );
        }

        let selection_str = substring_utf16(
            source,
            self.selection_offset as usize,
            (self.selection_offset + self.selection_length) as usize,
        );
        let num_leading = count_leading_whitespaces(selection_str);
        let num_trailing = count_trailing_whitespaces(selection_str);
        let offset = self.selection_offset + num_leading;
        let end = (self.selection_offset + self.selection_length).saturating_sub(num_trailing);
        let length = end.saturating_sub(offset);
        self.selection_range = (offset, length);
        let trimmed_str = substring_utf16(source, offset as usize, end as usize);

        let Some(covering_node) = ast.node_covering(unit.unit, offset, length) else {
            return RefactoringStatus::fatal(
                "An expression inside a function must be selected to activate this refactoring.",
            );
        };

        let mut fn_body = None;
        let mut p = Some(covering_node);
        while let Some(n) = p {
            if ast.is::<FunctionBody>(n) {
                fn_body = Some(n);
                break;
            }
            p = ast.parent(n);
        }
        self.covering_function_body = fn_body;
        if self.covering_function_body.is_none() {
            return RefactoringStatus::fatal(
                "An expression inside a function must be selected to activate this refactoring.",
            );
        }

        if ast.is::<StringLiteral>(covering_node)
            && length != 0
            && offset > ast.offset(covering_node)
            && end < ast.end(covering_node)
        {
            self.string_literal_part = Some(trimmed_str.to_string());
            return RefactoringStatus::new();
        }

        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let file_path = self.unit.path().to_string();

        let mut cur = Some(covering_node);
        while let Some(node) = cur {
            let parent = ast.parent(node);
            if matches!(
                ast.kind(node),
                NodeKind::ArgumentList
                    | NodeKind::AssignmentExpression
                    | NodeKind::NamedArgument
                    | NodeKind::TypeArgumentList
            ) {
                cur = parent;
                continue;
            }
            if matches!(
                ast.kind(node),
                NodeKind::ConstructorName | NodeKind::Label | NodeKind::NamedType
            ) {
                self.single_expression = None;
                self.covering_expression_offsets.clear();
                self.covering_expression_lengths.clear();
                cur = parent;
                continue;
            }
            if let Some(p_id) = parent {
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(p_id)
                    && ast[pi].identifier.raw() == node
                {
                    cur = parent;
                    continue;
                }
                if let Some(pa) = ast.cast::<PropertyAccess>(p_id)
                    && ast[pa].property_name.raw() == node
                {
                    cur = parent;
                    continue;
                }
            }
            if !ast.is::<Expression>(node) {
                break;
            }
            if let Some(mi) = ast.cast::<MethodInvocation>(node) {
                let is_void = u
                    .tables
                    .static_type
                    .get(node)
                    .is_some_and(|&ty| matches!(u.ctx.ty(ty), TypeKind::Void))
                    || u.element(ast[mi].method_name)
                        .and_then(|el| match u.ctx.any(el) {
                            AnyElement::Method(m) => m.return_type.get(),
                            AnyElement::TopLevelFunction(f) => f.return_type.get(),
                            AnyElement::LocalFunction(f) => f.return_type.get(),
                            _ => None,
                        })
                        .is_some_and(|ty| matches!(u.ctx.ty(ty), TypeKind::Void));
                if is_void {
                    if self.single_expression.is_none() {
                        let mut st = RefactoringStatus::new();
                        st.add_fatal_error(
                            "Cannot extract the void expression.",
                            Some(Location {
                                file: file_path,
                                offset: ast.offset(node),
                                length: ast.length(node),
                            }),
                        );
                        return st;
                    }
                    break;
                }
            }
            if self.covering_expression_offsets.is_empty() {
                if let Some(sid) = ast.cast::<SimpleIdentifier>(node) {
                    if in_declaration_context(ast, sid) {
                        let mut st = RefactoringStatus::new();
                        st.add_fatal_error(
                            "Cannot extract the name part of a declaration.",
                            Some(Location {
                                file: file_path,
                                offset: ast.offset(node),
                                length: ast.length(node),
                            }),
                        );
                        return st;
                    }
                    if let Some(el) = u.element(sid)
                        && matches!(
                            el.tag(),
                            Tag::LocalFunction | Tag::Method | Tag::TopLevelFunction
                        )
                    {
                        cur = parent;
                        continue;
                    }
                }
                if let Some(p_id) = parent
                    && let Some(assign) = ast.cast::<AssignmentExpression>(p_id)
                    && ast[assign].left_hand_side.raw() == node
                {
                    let mut st = RefactoringStatus::new();
                    st.add_fatal_error(
                        "Cannot extract the left-hand side of an assignment.",
                        Some(Location {
                            file: file_path,
                            offset: ast.offset(node),
                            length: ast.length(node),
                        }),
                    );
                    return st;
                }
            }
            if self.single_expression.is_none() {
                self.single_expression = Some(node);
            }
            self.covering_expression_offsets.push(ast.offset(node));
            self.covering_expression_lengths.push(ast.length(node));
            cur = parent;
        }
        if let Some(expr) = self.single_expression {
            self.selection_range = (ast.offset(expr), ast.length(expr));
            return RefactoringStatus::new();
        }
        RefactoringStatus::fatal("Expression must be selected to activate this refactoring.")
    }

    fn prepare_occurrences(&mut self) {
        self.occurrences.clear();
        let Some(fn_body) = self.covering_function_body else {
            return;
        };
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };

        if let Some(part) = self.string_literal_part.clone() {
            let part_len = utf16_len(&part) as u32;
            if part_len == 0 {
                return;
            }
            let mut stack = vec![fn_body];
            while let Some(cur) = stack.pop() {
                if ast.is::<StringLiteral>(cur) {
                    let lit_off = ast.offset(cur);
                    let lit_end = ast.end(cur);
                    let lit_text = substring_utf16(source, lit_off as usize, lit_end as usize);
                    let mut search_from = 0usize;
                    while let Some(rel_byte) = lit_text[search_from..].find(&part) {
                        let match_byte = search_from + rel_byte;
                        let match_utf16 = utf16_len(&lit_text[..match_byte]) as u32;
                        self.occurrences.push((lit_off + match_utf16, part_len));
                        search_from = match_byte + part.len();
                    }
                    continue;
                }
                let children = ast.children(cur);
                for child in children.into_iter().rev() {
                    stack.push(child);
                }
            }
            return;
        }

        let mut element_ids: HashMap<ElementId, usize> = HashMap::new();
        let selection_source = self
            .single_expression
            .map(|expr| encode_expression_tokens(&u, expr, &mut element_ids));
        let Some(selection_source) = selection_source else {
            return;
        };

        struct OccVisitor<'a, 'b, 'c> {
            u: &'a Unit<'b, 'c>,
            sel_range: (u32, u32),
            sel_source: &'a str,
            element_ids: &'a mut HashMap<ElementId, usize>,
            occurrences: &'a mut Vec<(u32, u32)>,
        }
        impl OccVisitor<'_, '_, '_> {
            fn visit(&mut self, node: NodeId) {
                let ast = self.u.ast;
                if let Some(sid) = ast.cast::<SimpleIdentifier>(node)
                    && let Some(p) = ast.parent(node)
                    && let Some(assign) = ast.cast::<AssignmentExpression>(p)
                    && ast[assign].left_hand_side.raw() == sid.raw()
                {
                    return;
                }
                if ast.is::<Expression>(node) {
                    let encoded = encode_expression_tokens(self.u, node, self.element_ids);
                    if encoded == self.sel_source {
                        let r = (ast.offset(node), ast.length(node));
                        if ranges_intersect(r, self.sel_range) {
                            self.occurrences.push(self.sel_range);
                        } else {
                            self.occurrences.push(r);
                        }
                    }
                }
                for child in ast.children(node) {
                    self.visit(child);
                }
            }
        }

        let mut visitor = OccVisitor {
            u: &u,
            sel_range: self.selection_range,
            sel_source: &selection_source,
            element_ids: &mut element_ids,
            occurrences: &mut self.occurrences,
        };
        visitor.visit(fn_body);
    }

    fn prepare_offsets_lengths(&mut self) {
        self.offsets.clear();
        self.lengths.clear();
        for &(off, len) in &self.occurrences {
            self.offsets.push(off);
            self.lengths.push(len);
        }
    }

    fn prepare_names(&mut self) {
        self.names.clear();
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        if let Some(part) = &self.string_literal_part {
            self.names
                .extend(naming::get_variable_name_suggestions_for_text(
                    part,
                    &self.excluded_variable_names,
                ));
        } else if let Some(expr) = self.single_expression {
            let static_type = unit.tables.static_type.get(expr).copied();
            self.names
                .extend(naming::get_variable_name_suggestions_for_expression(
                    &ctx,
                    ast,
                    &unit.tables,
                    static_type,
                    Some(expr),
                    &self.excluded_variable_names,
                    false,
                ));
        }
    }

    fn find_possible_local_variable_conflicts(&self) -> HashSet<String> {
        let mut conflicts = HashSet::new();
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(fn_body) = self.covering_function_body else {
            return conflicts;
        };
        let root_scope = ast.parent(fn_body).unwrap_or(fn_body);
        let mut stack = vec![root_scope];
        while let Some(cur) = stack.pop() {
            if let Some(vd) = ast.cast::<VariableDeclaration>(cur) {
                let lex = ast.tokens.lexeme(ast[vd].name);
                if !lex.is_empty() {
                    conflicts.insert(lex.to_string());
                }
            } else if let Some(fd) = ast.cast::<FunctionDeclaration>(cur) {
                let lex = ast.tokens.lexeme(ast[fd].name);
                if !lex.is_empty() {
                    conflicts.insert(lex.to_string());
                }
            } else if let Some(sid) = ast.cast::<SimpleIdentifier>(cur) {
                if let Some(el) = get_element(&u, sid.raw())
                    && matches!(
                        el.tag(),
                        Tag::LocalVariable
                            | Tag::FormalParameter
                            | Tag::FieldFormalParameter
                            | Tag::SuperFormalParameter
                            | Tag::LocalFunction
                    )
                {
                    let lex = ast.tokens.lexeme(ast[sid].token);
                    if !lex.is_empty() {
                        conflicts.insert(lex.to_string());
                    }
                }
            } else if ast.is::<FormalParameter>(cur) {
                if let Some(el) = u.declared_element(cur)
                    && let Some(n) = ctx.element_data(el).and_then(|d| d.name)
                {
                    conflicts.insert(ctx.name_str(n).to_string());
                }
            }
            for child in ast.children(cur) {
                stack.push(child);
            }
        }
        conflicts
    }

    fn find_declaration_target(&self, occurrences: &[(u32, u32)]) -> Option<NodeId> {
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let mut nodes = Vec::new();
        for &(off, _) in occurrences {
            if let Some(node) = ast.node_covering(unit.unit, off, 0) {
                nodes.push(node);
            }
        }
        if nodes.is_empty() {
            return None;
        }
        let common_parent = nearest_common_ancestor(ast, &nodes)?;
        if ast.is::<Block>(common_parent)
            || ast.is::<SwitchCase>(common_parent)
            || ast.is::<SwitchPatternCase>(common_parent)
        {
            let first_parents = get_parents(ast, nodes[0]);
            if let Some(idx) = first_parents.iter().position(|&p| p == common_parent)
                && idx + 1 < first_parents.len()
            {
                return Some(first_parents[idx + 1]);
            }
        }
        if let Some(efb) = enclosing_expression_body(ast, Some(common_parent)) {
            return Some(efb);
        }
        let mut target = this_or_ancestor_statement(ast, common_parent);
        while let Some(t) = target {
            if let Some(parent) = ast.parent(t)
                && (ast.is::<Block>(parent)
                    || ast.is::<SwitchCase>(parent)
                    || ast.is::<SwitchPatternCase>(parent))
            {
                break;
            }
            target = ast.parent(t);
        }
        target
    }
}

fn encode_expression_tokens(
    u: &Unit<'_, '_>,
    expr: NodeId,
    element_ids: &mut HashMap<ElementId, usize>,
) -> String {
    let ast = u.ast;
    let mut lexeme_to_local: HashMap<String, ElementId> = HashMap::new();
    let mut stack = vec![expr];
    while let Some(cur) = stack.pop() {
        if let Some(sid) = ast.cast::<SimpleIdentifier>(cur)
            && let Some(el) = u.element(sid)
            && el.tag() == Tag::LocalVariable
        {
            let lex = ast.tokens.lexeme(ast[sid].token).to_string();
            lexeme_to_local.insert(lex, el);
        }
        for child in ast.children(cur) {
            stack.push(child);
        }
    }

    let mut out = String::new();
    let mut tok_id = ast.begin_token(expr);
    let end_tok = ast.end_token(expr);
    let mut first = true;
    loop {
        if !first {
            out.push_str(TOKEN_SEPARATOR);
        }
        first = false;
        let lex = ast.tokens.lexeme(tok_id);
        out.push_str(lex);
        if let Some(&el) = lexeme_to_local.get(lex) {
            let next_id = element_ids.len();
            let id = *element_ids.entry(el).or_insert(next_id);
            out.push('-');
            out.push_str(&id.to_string());
        }
        let tok = ast.tokens.get(tok_id);
        if tok_id == end_tok || tok.ty == TokenType::EOF {
            break;
        }
        tok_id = tok.next;
    }
    out.push_str(TOKEN_SEPARATOR);
    out
}

pub(crate) fn count_leading_whitespaces(s: &str) -> u32 {
    let mut count = 0u32;
    for c in s.chars() {
        if c.is_whitespace() {
            count += c.len_utf16() as u32;
        } else {
            break;
        }
    }
    count
}

pub(crate) fn count_trailing_whitespaces(s: &str) -> u32 {
    let mut count = 0u32;
    for c in s.chars().rev() {
        if c.is_whitespace() {
            count += c.len_utf16() as u32;
        } else {
            break;
        }
    }
    count
}

pub(crate) fn in_declaration_context(ast: &Ast, sid: Id<SimpleIdentifier>) -> bool {
    let node = sid.raw();
    let tok = ast[sid].token;
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(ccp) = ast.cast::<CatchClauseParameter>(parent) {
        return ast[ccp].name == tok;
    }
    if let Some(cd) = ast.cast::<ConstructorDeclaration>(parent) {
        return ast[cd].name == Some(tok)
            || (ast[cd].name.is_none() && ast[cd].type_name.map(|t| t.raw()) == Some(node));
    }
    if let Some(di) = ast.cast::<DeclaredIdentifier>(parent) {
        return ast[di].name == tok;
    }
    if let Some(ecd) = ast.cast::<EnumConstantDeclaration>(parent) {
        return ast[ecd].name == tok;
    }
    if let Some(tp) = ast.cast::<TypeParameter>(parent) {
        return ast[tp].name == tok;
    }
    if let Some(vd) = ast.cast::<VariableDeclaration>(parent) {
        return ast[vd].name == tok;
    }
    if let Some(fd) = ast.cast::<FunctionDeclaration>(parent) {
        return ast[fd].name == tok;
    }
    if let Some(md) = ast.cast::<MethodDeclaration>(parent) {
        return ast[md].name == tok;
    }
    if let Some(label) = ast.cast::<Label>(parent) {
        return ast[label].name == tok;
    }
    in_formal_parameter_declaration(ast, parent, tok)
}

fn in_formal_parameter_declaration(ast: &Ast, param: NodeId, tok: dartr_syntax::TokenId) -> bool {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        return ast[rfp].name == Some(tok);
    }
    if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        return ast[ffp].name == tok;
    }
    if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        return ast[sfp].name == tok;
    }
    false
}

fn in_constant_context(ast: &Ast, mut node: NodeId) -> bool {
    loop {
        if let Some(ice) = ast.cast::<InstanceCreationExpression>(node)
            && ast[ice]
                .keyword
                .is_some_and(|k| ast.tokens.ty(k) == Keyword::CONST)
        {
            return true;
        }
        if let Some(lit) = ast.cast::<ListLiteral>(node)
            && ast[lit].const_keyword.is_some()
        {
            return true;
        }
        if let Some(lit) = ast.cast::<SetOrMapLiteral>(node)
            && ast[lit].const_keyword.is_some()
        {
            return true;
        }
        if let Some(vdl) = ast.cast::<VariableDeclarationList>(node)
            && ast[vdl]
                .keyword
                .is_some_and(|k| ast.tokens.ty(k) == Keyword::CONST)
        {
            return true;
        }
        let Some(p) = ast.parent(node) else {
            return false;
        };
        node = p;
    }
}

pub(crate) fn ranges_intersect(a: (u32, u32), b: (u32, u32)) -> bool {
    let a_end = a.0 + a.1;
    let b_end = b.0 + b.1;
    a.0 <= b_end && b.0 <= a_end
}

pub(crate) fn get_parents(ast: &Ast, node: NodeId) -> Vec<NodeId> {
    let mut parents = Vec::new();
    let mut cur = ast.parent(node);
    while let Some(p) = cur {
        parents.push(p);
        cur = ast.parent(p);
    }
    parents.reverse();
    parents.push(node);
    parents
}

pub(crate) fn nearest_common_ancestor(ast: &Ast, nodes: &[NodeId]) -> Option<NodeId> {
    let first = *nodes.first()?;
    if nodes.len() == 1 {
        return ast.parent(first);
    }
    let mut common = get_parents(ast, first);
    for &node in &nodes[1..] {
        let parents = get_parents(ast, node);
        let len = common.len().min(parents.len());
        let mut match_len = 0;
        while match_len < len && common[match_len] == parents[match_len] {
            match_len += 1;
        }
        common.truncate(match_len);
    }
    common.pop()
}

fn enclosing_expression_body(ast: &Ast, mut node: Option<NodeId>) -> Option<NodeId> {
    while let Some(n) = node {
        if ast.is::<Statement>(n) {
            return None;
        }
        if ast.is::<ExpressionFunctionBody>(n) {
            return Some(n);
        }
        node = ast.parent(n);
    }
    None
}

fn this_or_ancestor_statement(ast: &Ast, mut node: NodeId) -> Option<NodeId> {
    loop {
        if ast.is::<Statement>(node) {
            return Some(node);
        }
        node = ast.parent(node)?;
    }
}
