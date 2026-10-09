// Dart source: pkg/analysis_server/lib/src/computer/computer_folding.dart

//! Folding regions of a compilation unit (Dart `DartUnitFoldingComputer`).

use std::collections::HashSet;

use dartr_ast::*;
use dartr_syntax::{LineInfo, TokenId, TokenType};

/// Dart `FoldingKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldingKind {
    Annotations,
    Block,
    ClassBody,
    Comment,
    Directives,
    DocumentationComment,
    FileHeader,
    FunctionBody,
    Invocation,
    Literal,
    Parameters,
}

/// Dart `FoldingRegion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoldingRegion {
    pub kind: FoldingKind,
    pub offset: u32,
    pub length: u32,
}

/// Dart `DartUnitFoldingComputer`.
pub struct DartUnitFoldingComputer<'a> {
    ast: &'a Ast,
    unit: Id<CompilationUnit>,
    line_info: &'a LineInfo,
    first_directive: Option<(NodeId, TokenId)>,
    last_directive: Option<(NodeId, TokenId)>,
    regions: Vec<FoldingRegion>,
    lines_with_regions: HashSet<u32>,
}

impl<'a> DartUnitFoldingComputer<'a> {
    pub fn new(ast: &'a Ast, unit: Id<CompilationUnit>, line_info: &'a LineInfo) -> Self {
        DartUnitFoldingComputer {
            ast,
            unit,
            line_info,
            first_directive: None,
            last_directive: None,
            regions: Vec::new(),
            lines_with_regions: HashSet::new(),
        }
    }

    /// Dart `compute`.
    pub fn compute(mut self) -> Vec<FoldingRegion> {
        let ast = self.ast;
        ast.accept(self.unit, &mut Visitor { c: &mut self });
        if let (Some(first), Some(last)) = (self.first_directive, self.last_directive) {
            if first.0 != last.0 {
                let start = ast.tokens.get(first.1).end();
                let end = ast.end(last.0);
                self.add_region(start, end, FoldingKind::Directives, None);
            }
        }
        self.add_comment_regions();
        self.regions
    }

    fn tok_end(&self, t: TokenId) -> u32 {
        self.ast.tokens.get(t).end()
    }

    fn tok_offset(&self, t: TokenId) -> u32 {
        self.ast.tokens.offset(t)
    }

    /// Dart `addRegionForConditionalBlock`.
    fn add_region_for_conditional_block(&mut self, block: Id<Block>) {
        let ast = self.ast;
        let b = &ast[block];
        let start = self.tok_end(b.left_bracket);
        let end_token = ast.end_token(block);
        if let Some(last_comment) = ast.tokens.comments(end_token).last() {
            self.add_region(start, self.tok_end(last_comment), FoldingKind::Block, None);
        } else if let Some(&last) = ast.list(b.statements).last() {
            self.add_region(start, ast.end(last), FoldingKind::Block, None);
        }
    }

    /// Dart `_addCommentRegion`. Returns the next comment to process.
    fn add_comment_region(&mut self, comment: TokenId, may_be_file_header: bool) -> TokenId {
        let tokens = &self.ast.tokens;
        let t = tokens.get(comment);
        let lexeme = tokens.lexeme(comment);
        let offset;
        let end;
        let next_comment;
        let mut is_file_header = false;
        if t.ty == TokenType::MULTI_LINE_COMMENT {
            // The region starts at the end of the first line.
            let eol = lexeme
                .encode_utf16()
                .position(|u| u == b'\r' as u16 || u == b'\n' as u16)
                .unwrap_or(0) as u32;
            offset = t.offset + eol;
            end = t.end();
            next_comment = t.next;
        } else {
            let is_triple_slash = lexeme.starts_with("///");
            let mut last = comment;
            let mut current = t.next;
            while let Some(c) = current.get() {
                let ct = tokens.get(c);
                let lt = tokens.get(last);
                if ct.ty == lt.ty
                    && tokens.lexeme(c).starts_with("///") == is_triple_slash
                    && !self.has_blank_line_between(lt.end(), ct.offset)
                {
                    last = c;
                    current = ct.next;
                } else {
                    break;
                }
            }
            offset = t.end();
            end = tokens.get(last).end();
            next_comment = tokens.get(last).next;
            let unit_begin = self.tok_offset(self.ast[self.unit].begin_token);
            is_file_header = may_be_file_header
                && (next_comment.is_some() || self.has_blank_line_between(end, unit_begin));
        }
        let kind = if is_file_header {
            FoldingKind::FileHeader
        } else if lexeme.starts_with("///") || lexeme.starts_with("/**") {
            FoldingKind::DocumentationComment
        } else {
            FoldingKind::Comment
        };
        self.add_region(offset, end, kind, None);
        next_comment
    }

    /// Dart `_addCommentRegions`.
    fn add_comment_regions(&mut self) {
        let ast = self.ast;
        let mut token = ast[self.unit].begin_token;
        if ast.tokens.ty(token) == TokenType::SCRIPT_TAG {
            token = ast.tokens.next(token);
        }
        let mut is_first_token = true;
        loop {
            let mut comment = ast.tokens.get(token).preceding_comments;
            while let Some(c) = comment.get() {
                comment = self.add_comment_region(c, is_first_token);
            }
            is_first_token = false;
            if ast.tokens.get(token).is_eof() {
                break;
            }
            let next = ast.tokens.next(token);
            if next.is_none() {
                break;
            }
            token = next;
        }
    }

    /// Dart `_addRegion`.
    fn add_region(
        &mut self,
        start_offset: u32,
        end_offset: u32,
        kind: FoldingKind,
        fallback_start: Option<u32>,
    ) {
        let mut start_offset = start_offset;
        let mut start = self.line_info.get_location(start_offset);
        let end = self.line_info.get_location(end_offset);
        if self.lines_with_regions.contains(&start.line_number) {
            let Some(fallback) = fallback_start else {
                return;
            };
            start_offset = fallback;
            start = self.line_info.get_location(start_offset);
            if self.lines_with_regions.contains(&start.line_number) {
                return;
            }
        }
        if end.line_number > start.line_number {
            self.regions.push(FoldingRegion {
                kind,
                offset: start_offset,
                length: end_offset - start_offset,
            });
            self.lines_with_regions.insert(start.line_number);
        }
    }

    /// Dart `_addRegionForAnnotations`.
    fn add_region_for_annotations(&mut self, annotations: NodeList<Annotation>) {
        let ast = self.ast;
        let list = ast.list(annotations);
        if let (Some(&first), Some(&last)) = (list.first(), list.last()) {
            let start = ast.end(ast[first].name);
            self.add_region(start, ast.end(last), FoldingKind::Annotations, None);
        }
    }

    fn has_blank_line_between(&self, offset: u32, end: u32) -> bool {
        let first = self.line_info.get_location(offset);
        let second = self.line_info.get_location(end);
        second.line_number as i64 - first.line_number as i64 > 1
    }

    fn record_directive(&mut self, node: NodeId, keyword: TokenId) {
        if self.first_directive.is_none() {
            self.first_directive = Some((node, keyword));
        }
        self.last_directive = Some((node, keyword));
    }
}

/// Dart `_DartUnitFoldingComputerVisitor`.
struct Visitor<'c, 'a> {
    c: &'c mut DartUnitFoldingComputer<'a>,
}

impl AstVisitor for Visitor<'_, '_> {
    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        let n = &ast[node];
        let fallback = ast.list(n.arguments).first().map(|&a| ast.offset(a));
        self.c.add_region(
            ast.tokens.get(n.left_parenthesis).end(),
            ast.tokens.offset(n.right_parenthesis),
            FoldingKind::Invocation,
            fallback,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_assert_initializer(&mut self, ast: &Ast, node: Id<AssertInitializer>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_parenthesis).end(),
            ast.tokens.offset(n.right_parenthesis),
            FoldingKind::Invocation,
            Some(ast.offset(n.condition)),
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_assert_statement(&mut self, ast: &Ast, node: Id<AssertStatement>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_parenthesis).end(),
            ast.tokens.offset(n.right_parenthesis),
            FoldingKind::Invocation,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_block_class_body(&mut self, ast: &Ast, node: Id<BlockClassBody>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.offset(n.right_bracket),
            FoldingKind::ClassBody,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_block_enum_body(&mut self, ast: &Ast, node: Id<BlockEnumBody>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.offset(n.right_bracket),
            FoldingKind::ClassBody,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let n = &ast[node];
        self.c.add_region_for_annotations(n.metadata);
        let body_start = n
            .name
            .map(|t| ast.tokens.get(t).end())
            .or(n.type_name.map(|t| ast.end(t)))
            .or(n.new_keyword.map(|t| ast.tokens.get(t).end()));
        if let Some(start) = body_start {
            self.c
                .add_region(start, ast.end(node), FoldingKind::FunctionBody, None);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        if let Some(block) = ast.cast::<Block>(ast[node].body) {
            self.c.add_region_for_conditional_block(block);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        self.c.record_directive(node.raw(), ast[node].export_keyword);
        self.visit_node(ast, node.raw());
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_extension_type_declaration(
        &mut self,
        ast: &Ast,
        node: Id<ExtensionTypeDeclaration>,
    ) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_formal_parameter_list(&mut self, ast: &Ast, node: Id<FormalParameterList>) {
        let n = &ast[node];
        let fallback = ast.list(n.parameters).first().map(|&p| ast.offset(p));
        self.c.add_region(
            ast.tokens.get(n.left_parenthesis).end(),
            ast.tokens.offset(n.right_parenthesis),
            FoldingKind::Parameters,
            fallback,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        if let Some(block) = ast.cast::<Block>(ast[node].body) {
            let b = &ast[block];
            self.c.add_region(
                ast.tokens.offset(b.left_bracket),
                ast.tokens.get(b.right_bracket).end(),
                FoldingKind::Block,
                None,
            );
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let n = &ast[node];
        self.c.add_region_for_annotations(n.metadata);
        self.c.add_region(
            ast.tokens.get(n.name).end(),
            ast.end(node),
            FoldingKind::FunctionBody,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        if let Some(body) = ast.cast::<BlockFunctionBody>(ast[node].body) {
            let b = &ast[ast[body].block];
            self.c.add_region(
                ast.tokens.offset(b.left_bracket),
                ast.tokens.get(b.right_bracket).end(),
                FoldingKind::Block,
                None,
            );
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        let n = &ast[node];
        if let Some(block) = ast.cast::<Block>(n.then_statement) {
            self.c.add_region_for_conditional_block(block);
        }
        if let Some(block) = n.else_statement.and_then(|e| ast.cast::<Block>(e)) {
            self.c.add_region_for_conditional_block(block);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        self.c.record_directive(node.raw(), ast[node].import_keyword);
        self.visit_node(ast, node.raw());
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        self.c.record_directive(node.raw(), ast[node].library_keyword);
        self.visit_node(ast, node.raw());
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.offset(n.right_bracket),
            FoldingKind::Literal,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let n = &ast[node];
        self.c.add_region_for_annotations(n.metadata);
        self.c.add_region(
            ast.tokens.get(n.name).end(),
            ast.end(node),
            FoldingKind::FunctionBody,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        self.c.add_region_for_annotations(ast[node].metadata);
        self.visit_node(ast, node.raw());
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        self.c.record_directive(node.raw(), ast[node].part_keyword);
        self.visit_node(ast, node.raw());
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        self.c.record_directive(node.raw(), ast[node].part_keyword);
        self.visit_node(ast, node.raw());
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        let n = &ast[node];
        self.c.add_region_for_annotations(n.metadata);
        self.c.add_region(
            ast.tokens.get(n.this_keyword).end(),
            ast.end(node),
            FoldingKind::FunctionBody,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_parenthesis).end(),
            ast.tokens.offset(n.right_parenthesis),
            FoldingKind::Literal,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.offset(n.right_bracket),
            FoldingKind::Literal,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_simple_string_literal(&mut self, ast: &Ast, node: Id<SimpleStringLiteral>) {
        self.c
            .add_region(ast.offset(node), ast.end(node), FoldingKind::Literal, None);
        self.visit_node(ast, node.raw());
    }

    fn visit_string_interpolation(&mut self, ast: &Ast, node: Id<StringInterpolation>) {
        self.c
            .add_region(ast.offset(node), ast.end(node), FoldingKind::Literal, None);
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        self.c.add_region(
            ast.tokens.get(ast[node].colon).end(),
            ast.end(node),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_default(&mut self, ast: &Ast, node: Id<SwitchDefault>) {
        self.c.add_region(
            ast.tokens.get(ast[node].colon).end(),
            ast.end(node),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.get(n.right_bracket).end(),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_expression_case(&mut self, ast: &Ast, node: Id<SwitchExpressionCase>) {
        self.c.add_region(
            ast.tokens.get(ast[node].arrow).end(),
            ast.end(node),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        self.c.add_region(
            ast.tokens.get(ast[node].colon).end(),
            ast.end(node),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        let n = &ast[node];
        self.c.add_region(
            ast.tokens.get(n.left_bracket).end(),
            ast.tokens.get(n.right_bracket).end(),
            FoldingKind::Block,
            None,
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        let n = &ast[node];
        self.c.add_region_for_conditional_block(n.body);
        for &catch_clause in ast.list(n.catch_clauses) {
            self.c.add_region_for_conditional_block(ast[catch_clause].body);
        }
        if let Some(finally_block) = n.finally_block {
            self.c.add_region_for_conditional_block(finally_block);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        if let Some(block) = ast.cast::<Block>(ast[node].body) {
            self.c.add_region_for_conditional_block(block);
        }
        self.visit_node(ast, node.raw());
    }
}
