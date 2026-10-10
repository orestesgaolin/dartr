// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (DartFileEditBuilderImpl.insertIntoUnitMember, insertMethod, insertField, insertGetter, insertConstructor, _InsertionPreparer, CompilationUnitMember.leftBracket/members/rightBracket)

//! Inserting members into class-like declarations.

use dartr_ast::*;
use dartr_syntax::{TokenId, TokenType};

use super::change_builder::{EditBuilder, FileEditBuilder};
use super::code_style::CodeStyleOptions;
use super::utils::first_token_after_comment_and_metadata;

/// The body of a class-like declaration.
fn body_of(ast: &Ast, member: NodeId) -> Option<NodeId> {
    if let Some(c) = ast.cast::<ClassDeclaration>(member) {
        Some(ast[c].body.raw())
    } else if let Some(e) = ast.cast::<EnumDeclaration>(member) {
        Some(ast[e].body.raw())
    } else if let Some(e) = ast.cast::<ExtensionDeclaration>(member) {
        Some(ast[e].body.raw())
    } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(member) {
        Some(ast[e].body.raw())
    } else {
        ast.cast::<MixinDeclaration>(member)
            .map(|m| ast[m].body.raw())
    }
}

/// Dart `CompilationUnitMember.leftBracket`.
fn left_bracket(ast: &Ast, member: NodeId) -> Option<TokenId> {
    let body = body_of(ast, member)?;
    if let Some(b) = ast.cast::<BlockClassBody>(body) {
        return Some(ast[b].left_bracket);
    }
    ast.cast::<BlockEnumBody>(body).map(|b| ast[b].left_bracket)
}

/// Dart `CompilationUnitMember.rightBracket`.
fn right_bracket(ast: &Ast, member: NodeId) -> Option<TokenId> {
    let body = body_of(ast, member)?;
    if let Some(b) = ast.cast::<BlockClassBody>(body) {
        return Some(ast[b].right_bracket);
    }
    ast.cast::<BlockEnumBody>(body)
        .map(|b| ast[b].right_bracket)
}

/// Dart `CompilationUnitMember.members`.
pub fn members_of(ast: &Ast, member: NodeId) -> Option<Vec<NodeId>> {
    let body = body_of(ast, member)?;
    if let Some(b) = ast.cast::<BlockClassBody>(body) {
        return Some(ast.list_raw(ast[b].members).to_vec());
    }
    if let Some(b) = ast.cast::<BlockEnumBody>(body) {
        return Some(ast.list_raw(ast[b].members).to_vec());
    }
    Some(Vec::new())
}

fn enum_constants(ast: &Ast, member: NodeId) -> Vec<NodeId> {
    let Some(e) = ast.cast::<EnumDeclaration>(member) else {
        return Vec::new();
    };
    match ast.cast::<BlockEnumBody>(ast[e].body) {
        Some(b) => ast.list_raw(ast[b].constants).to_vec(),
        None => Vec::new(),
    }
}

/// Dart `_InsertionPreparer`.
struct InsertionPreparer {
    declaration: NodeId,
    members: Vec<NodeId>,
    found_target_member: bool,
    trailing_comma: Option<TokenId>,
}

impl InsertionPreparer {
    /// Dart `insertionLocation`.
    fn insertion_location(
        &mut self,
        ast: &Ast,
        last_member_filter: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> Option<u32> {
        let tokens = &ast.tokens;
        let target = self
            .members
            .iter()
            .rev()
            .find(|m| last_member_filter(ast, **m))
            .copied();
        self.found_target_member = target.is_some();
        if let Some(target) = target {
            return Some(ast.end(target));
        }
        if let Some(e) = ast.cast::<EnumDeclaration>(self.declaration) {
            let body = ast[e].body;
            let mut semicolon = None;
            let mut has_constants = false;
            let mut last_constant = None;
            let mut token = None;
            if let Some(b) = ast.cast::<BlockEnumBody>(body) {
                semicolon = ast[b].semicolon;
                let constants = ast.list_raw(ast[b].constants);
                has_constants = !constants.is_empty();
                last_constant = constants.last().copied();
                let comment_or = |t: TokenId| {
                    let c = tokens.get(t).preceding_comments;
                    if c.is_some() { c } else { t }
                };
                token = Some(match last_constant {
                    Some(c) => comment_or(tokens.next(ast.end_token(c))),
                    None => match ast.list_raw(ast[b].members).first() {
                        Some(&m) => comment_or(ast.begin_token(m)),
                        None => comment_or(ast[b].right_bracket),
                    },
                });
            } else if let Some(b) = ast.cast::<EmptyEnumBody>(body) {
                semicolon = Some(ast[b].semicolon);
            }
            if let Some(s) = semicolon {
                return Some(tokens.get(s).end());
            } else if has_constants {
                let last = last_constant.unwrap();
                let next = tokens.next(ast.end_token(last));
                if tokens.ty(next) == TokenType::COMMA {
                    self.trailing_comma = Some(next);
                }
                return Some(ast.end(last));
            } else if let Some(t) = token {
                return Some(tokens.get(t).offset);
            }
        }
        let left = left_bracket(ast, self.declaration)?;
        if tokens.get(left).is_synthetic() {
            let previous = tokens.previous(left);
            if previous.is_none() {
                return None;
            }
            return Some(tokens.get(previous).end());
        }
        Some(tokens.get(left).end())
    }

    /// Dart `writePrefix`.
    fn write_prefix(&self, ast: &Ast, b: &mut EditBuilder<'_, '_, '_>, indent: bool) {
        let tokens = &ast.tokens;
        if left_bracket(ast, self.declaration).is_some_and(|l| tokens.get(l).is_synthetic()) {
            b.write(" {");
        }
        let is_enum = ast.is::<EnumDeclaration>(self.declaration);
        let mut has_semicolon = false;
        if let Some(e) = ast.cast::<EnumDeclaration>(self.declaration) {
            let body = ast[e].body;
            has_semicolon = ast
                .cast::<BlockEnumBody>(body)
                .is_some_and(|b| ast[b].semicolon.is_some())
                || ast.is::<EmptyEnumBody>(body);
        }
        let has_constants = !enum_constants(ast, self.declaration).is_empty();
        if is_enum && !has_semicolon {
            b.write(";");
            if !has_constants {
                b.newline();
            }
        }
        if self.found_target_member || is_enum && has_constants {
            b.newline();
        }
        b.newline();
        if indent {
            b.write_indent(1);
        }
    }

    /// Dart `writeSuffix`.
    fn write_suffix(
        &self,
        ast: &Ast,
        line_info: &dartr_syntax::LineInfo,
        b: &mut EditBuilder<'_, '_, '_>,
    ) {
        if self.found_target_member {
            return;
        }
        let tokens = &ast.tokens;
        let is_enum = ast.is::<EnumDeclaration>(self.declaration);
        let has_constants = !enum_constants(ast, self.declaration).is_empty();
        let first = first_token_after_comment_and_metadata(ast, self.declaration);
        let single_line =
            line_info.on_same_line(tokens.get(first).offset, ast.end(self.declaration));
        if is_enum && has_constants && !single_line {
            return;
        }
        if !self.members.is_empty() {
            b.newline();
            return;
        }
        if single_line {
            b.newline();
        }
        let Some(right) = right_bracket(ast, self.declaration) else {
            return;
        };
        if tokens.get(right).is_synthetic() {
            let next = tokens.next(right);
            if tokens.ty(next) != TokenType::CLOSE_CURLY_BRACKET {
                b.newline();
                b.write("}");
            }
        }
    }
}

impl FileEditBuilder<'_, '_> {
    /// Dart `insertIntoUnitMember`.
    pub fn insert_into_unit_member(
        &mut self,
        member: NodeId,
        last_member_filter: Option<&dyn Fn(&Ast, NodeId) -> bool>,
        indent: bool,
        build: impl FnOnce(&mut EditBuilder<'_, '_, '_>),
    ) {
        let resolved = self.resolved();
        let unit = resolved.unit();
        let ast = &unit.ast;
        let line_info = resolved.line_info().clone();
        let tokens = &ast.tokens;
        // An empty body (`class A;`).
        if let Some(body) = body_of(ast, member) {
            let empty = ast.is::<EmptyClassBody>(body) || ast.is::<EmptyEnumBody>(body);
            if empty {
                let is_enum_body = ast.is::<EmptyEnumBody>(body);
                let (offset, length) = (ast.offset(body), ast.length(body));
                self.add_replacement(offset, length, |b| {
                    b.writeln(" {");
                    if indent {
                        b.write("  ");
                        if is_enum_body {
                            b.writeln(";");
                            b.newline();
                            b.write("  ");
                        }
                    }
                    build(b);
                    b.newline();
                    b.write("}");
                });
                return;
            }
        }
        // A single-line enum: break after the left bracket.
        let mut enum_right_bracket = None;
        if let Some(e) = ast.cast::<EnumDeclaration>(member) {
            if let Some(body) = ast.cast::<BlockEnumBody>(ast[e].body) {
                let left = ast[body].left_bracket;
                let right = ast[body].right_bracket;
                if line_info.on_same_line(tokens.get(left).offset, tokens.get(right).offset) {
                    enum_right_bracket = Some(right);
                    let comment_or = |t: TokenId| {
                        let c = tokens.get(t).preceding_comments;
                        if c.is_some() { c } else { t }
                    };
                    let token = match ast.list_raw(ast[body].constants).first() {
                        Some(&c) => ast.begin_token(c),
                        None => {
                            let next = tokens.next(left);
                            if next.is_some() {
                                comment_or(next)
                            } else {
                                comment_or(right)
                            }
                        }
                    };
                    let start = tokens.get(left).end();
                    let end = tokens.get(token).offset;
                    self.add_replacement(start, end - start, |b| {
                        b.newline();
                        b.write_indent(1);
                    });
                }
            }
        }
        let mut preparer = InsertionPreparer {
            declaration: member,
            members: members_of(ast, member).unwrap_or_default(),
            found_target_member: false,
            trailing_comma: None,
        };
        let filter: &dyn Fn(&Ast, NodeId) -> bool = match last_member_filter {
            Some(f) => f,
            None => &|_, _| true,
        };
        let Some(offset) = preparer.insertion_location(ast, filter) else {
            return;
        };
        let write = |b: &mut EditBuilder<'_, '_, '_>| {
            preparer.write_prefix(ast, b, indent);
            build(b);
            preparer.write_suffix(ast, &line_info, b);
        };
        if let Some(right) = enum_right_bracket {
            let comments = tokens.get(right).preceding_comments;
            let start = if comments.is_some() {
                tokens.get(comments).end()
            } else {
                tokens.get(tokens.previous(right)).end()
            };
            let end = tokens.get(right).offset;
            self.add_replacement(start, end - start, write);
        } else if let Some(comma) = preparer.trailing_comma {
            let t = tokens.get(comma);
            self.add_replacement(t.offset, t.length, write);
        } else {
            self.add_insertion_with(offset, false, write);
        }
    }

    /// Dart `insertMethod`.
    pub fn insert_method(
        &mut self,
        member: NodeId,
        build: impl FnOnce(&mut EditBuilder<'_, '_, '_>),
    ) {
        let filter = |ast: &Ast, m: NodeId| {
            ast.is::<FieldDeclaration>(m)
                || ast.is::<ConstructorDeclaration>(m)
                || ast.is::<MethodDeclaration>(m)
        };
        self.insert_into_unit_member(member, Some(&filter), true, build);
    }

    /// Dart `insertField`.
    pub fn insert_field(
        &mut self,
        member: NodeId,
        build: impl FnOnce(&mut EditBuilder<'_, '_, '_>),
    ) {
        let options = self.change.workspace.analysis_options(&self.path);
        let sort_constructors_first =
            CodeStyleOptions { options: &options }.sort_constructors_first();
        let filter = move |ast: &Ast, m: NodeId| {
            if sort_constructors_first {
                ast.is::<ConstructorDeclaration>(m) || ast.is::<FieldDeclaration>(m)
            } else {
                ast.is::<FieldDeclaration>(m)
            }
        };
        self.insert_into_unit_member(member, Some(&filter), true, build);
    }

    /// Dart `insertGetter`.
    pub fn insert_getter(
        &mut self,
        member: NodeId,
        build: impl FnOnce(&mut EditBuilder<'_, '_, '_>),
    ) {
        let filter = |ast: &Ast, m: NodeId| {
            ast.is::<FieldDeclaration>(m)
                || ast.is::<ConstructorDeclaration>(m)
                || ast.cast::<MethodDeclaration>(m).is_some_and(|d| {
                    ast[d]
                        .property_keyword
                        .is_some_and(|k| ast.tokens.lexeme(k) == "get")
                })
        };
        self.insert_into_unit_member(member, Some(&filter), true, build);
    }

    /// Dart `insertConstructor`.
    pub fn insert_constructor(
        &mut self,
        member: NodeId,
        is_named: bool,
        build: impl FnOnce(&mut EditBuilder<'_, '_, '_>),
    ) {
        let options = self.change.workspace.analysis_options(&self.path);
        let style = CodeStyleOptions { options: &options };
        let constructors_first = style.sort_constructors_first();
        let unnamed_first = !is_named && style.sort_unnamed_constructors_first();
        let filter = move |ast: &Ast, m: NodeId| {
            if constructors_first {
                ast.is::<ConstructorDeclaration>(m)
            } else if unnamed_first {
                ast.cast::<ConstructorDeclaration>(m)
                    .is_some_and(|c| ast[c].name.is_none())
                    || ast.is::<FieldDeclaration>(m)
            } else {
                ast.is::<ConstructorDeclaration>(m) || ast.is::<FieldDeclaration>(m)
            }
        };
        self.insert_into_unit_member(member, Some(&filter), true, build);
    }
}
