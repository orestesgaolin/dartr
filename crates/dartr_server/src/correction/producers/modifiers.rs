// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_const.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/make_final.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_initializer.dart

//! Producers that add or change modifiers: `const`, `final`, and remove
//! initializers.

use dartr_ast::*;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::super::utils::{Range, first_token_after_comment_and_metadata};
use super::simple::producer;

/// Dart `AddConst._computeTargetNode`.
fn add_const_target(c: &ProducerContext<'_>) -> Option<NodeId> {
    let ast = c.ast;
    let mut target = Some(c.node);
    if let Some(t) = target.filter(|t| ast.is::<SimpleIdentifier>(*t)) {
        target = ast.parent(t);
    }
    if let Some(t) = target.filter(|t| ast.is::<FormalParameterList>(*t)) {
        target = ast.parent(t);
    }
    if target.is_some_and(|t| ast.is::<TypeArgumentList>(t)) {
        while let Some(t) = target {
            if ast.is::<CompilationUnit>(t) || ast.is::<ConstantPattern>(t) {
                break;
            }
            target = ast.parent(t);
        }
    }
    let mut target = target?;
    if ast.is::<CompilationUnit>(target) {
        return None;
    }
    for is_skipped in [
        |a: &Ast, n: NodeId| a.is::<NamedType>(n),
        |a: &Ast, n: NodeId| a.is::<ConstructorName>(n),
        |a: &Ast, n: NodeId| a.is::<PrimaryConstructorName>(n),
    ] {
        if is_skipped(ast, target) {
            target = ast.parent(target)?;
        }
    }
    Some(target)
}

/// Dart `_ConstRangeFinder`: the `const` keywords inside [node] (not in
/// closures).
fn const_ranges(c: &ProducerContext<'_>, node: NodeId, out: &mut Vec<Range>) {
    let ast = c.ast;
    if ast.is::<FunctionExpression>(node) {
        return;
    }
    let keyword = if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        ast[i].keyword
    } else if let Some(l) = ast.cast::<ListLiteral>(node) {
        ast[l].const_keyword
    } else if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
        ast[l].const_keyword
    } else {
        None
    };
    if let Some(keyword) = keyword {
        if ast.tokens.lexeme(keyword) == "const" {
            out.push(
                c.range()
                    .token_start_start(keyword, ast.tokens.next(keyword)),
            );
        }
    }
    for child in ast.children(node) {
        const_ranges(c, child, out);
    }
}

/// Dart `AddConst._insertBeforeNode`.
fn insert_before_node(c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>, target: NodeId) {
    let ast = c.ast;
    let mut ranges = Vec::new();
    // Dart `targetNode.accept(finder)`: the visitor visits the target node
    // itself too.
    const_ranges(c, target, &mut ranges);
    builder.add_dart_file_edit(c.path, |b| {
        // Dart `_isAncestorConstant`.
        let edits: Vec<u32> = b
            .data()
            .file_edit
            .edits
            .iter()
            .filter(|e| e.replacement.starts_with("const"))
            .map(|e| e.offset)
            .collect();
        if !edits.is_empty() {
            let mut child = ast.parent(target);
            while let Some(n) = child {
                if !(ast.is::<Expression>(n)
                    || ast.is::<ArgumentList>(n)
                    || ast.is::<VariableDeclaration>(n)
                    || ast.is::<VariableDeclarationList>(n))
                {
                    break;
                }
                if edits.contains(&ast.offset(n)) {
                    return;
                }
                child = ast.parent(n);
            }
        }
        b.add_simple_insertion(ast.offset(target), "const ");
        for r in &ranges {
            b.add_deletion(r.offset, r.length);
        }
    });
}

/// Dart `AddConst._declarationListIsFullyConst`.
fn declaration_list_is_fully_const(
    c: &ProducerContext<'_>,
    variables: &[Id<VariableDeclaration>],
) -> bool {
    let ast = c.ast;
    let ranges: Vec<(u32, u32)> = c
        .diagnostics
        .iter()
        .filter(|d| d.code.unique_name == "prefer_const_constructors")
        .map(|d| (d.offset as u32, d.length as u32))
        .collect();
    variables.iter().all(|v| {
        let node = match ast[*v].initializer {
            Some(i) => i.raw(),
            None => v.raw(),
        };
        ranges.contains(&(ast.offset(node), ast.length(node)))
    })
}

producer!(
    AddConst,
    k::ADD_CONST,
    Some(&k::ADD_CONST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(target) = add_const_target(c) else {
            return;
        };
        if ast.is::<PrimaryConstructorDeclaration>(target) {
            let offset = ast.offset(target);
            builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "const "));
            return;
        }
        if ast.is::<ConstructorDeclaration>(target) {
            let offset = ast
                .tokens
                .get(first_token_after_comment_and_metadata(ast, target))
                .offset;
            builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "const "));
            return;
        }
        let add_parens_and_const = |builder: &mut ChangeBuilder<'_>, parent: NodeId| {
            let offset = ast.offset(parent);
            let length = ast.length(parent);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_insertion(offset + length, ")");
                b.add_simple_insertion(offset, "const (");
            });
        };
        if let Some(pattern) = ast.cast::<ConstantPattern>(target) {
            // Dart `expression.canBeConst` is not ported: only type literals.
            let expression = ast[pattern].expression;
            if ast.is::<TypeLiteral>(expression) {
                let Some(parent) = ast.parent(pattern) else {
                    return;
                };
                if ast.is::<ParenthesizedPattern>(parent) {
                    let offset = ast.offset(parent);
                    builder
                        .add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "const "));
                } else {
                    add_parens_and_const(builder, parent);
                }
            }
            return;
        }
        if ast.is::<BinaryExpression>(target) || ast.is::<PrefixExpression>(target) {
            let Some(parent) = ast.parent(target) else {
                return;
            };
            match ast.parent(parent) {
                Some(pp) if ast.is::<ParenthesizedPattern>(pp) => {
                    let offset = ast.offset(pp);
                    builder
                        .add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "const "));
                }
                _ => add_parens_and_const(builder, parent),
            }
            return;
        }
        if ast.is::<ListLiteral>(target) || ast.is::<SetOrMapLiteral>(target) {
            insert_before_node(c, builder, target);
            return;
        }
        let keyword = if let Some(i) = ast.cast::<InstanceCreationExpression>(target) {
            Some(ast[i].keyword)
        } else {
            ast.cast::<DotShorthandConstructorInvocation>(target)
                .map(|d| ast[d].const_keyword)
        };
        let Some(keyword) = keyword else { return };
        let parent = ast.parent(target);
        if let Some(variable) = parent.and_then(|p| ast.cast::<VariableDeclaration>(p)) {
            if c.code_style().prefer_const_declarations() {
                if let Some(list) = ast
                    .parent(variable)
                    .and_then(|p| ast.cast::<VariableDeclarationList>(p))
                {
                    let final_keyword = ast[list]
                        .keyword
                        .filter(|t| ast.tokens.lexeme(*t) == "final");
                    let variables = ast.list(ast[list].variables).to_vec();
                    if let Some(final_keyword) = final_keyword {
                        if declaration_list_is_fully_const(c, &variables) {
                            let range = c.range().token(final_keyword);
                            builder.add_dart_file_edit(c.path, |b| {
                                b.add_simple_replacement(range.offset, range.length, "const")
                            });
                            return;
                        }
                    }
                }
            }
        }
        if keyword.is_none() {
            insert_before_node(c, builder, target);
        }
    }
);

/// Dart `MakeFinal._getVariableDeclarationList`.
fn variable_declaration_list(ast: &Ast, node: NodeId) -> Option<Id<VariableDeclarationList>> {
    if let Some(l) = ast.cast::<VariableDeclarationList>(node) {
        return Some(l);
    }
    let parent = ast.parent(node);
    if ast.is::<VariableDeclaration>(node) || ast.is::<NamedType>(node) {
        if let Some(l) = parent.and_then(|p| ast.cast::<VariableDeclarationList>(p)) {
            return Some(l);
        }
    }
    let parent = parent?;
    if ast.is::<NamedType>(parent) {
        return ast
            .parent(parent)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p));
    }
    None
}

fn is_var(c: &ProducerContext<'_>, t: Option<dartr_syntax::TokenId>) -> bool {
    t.is_some_and(|t| c.lexeme(t) == "var")
}

producer!(
    MakeFinal,
    k::MAKE_FINAL,
    Some(&k::MAKE_FINAL_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let node = c.node;
        let parent = ast.parent(node);
        let r = c.range();
        if let Some(id) = ast.cast::<DeclaredIdentifier>(node) {
            if parent.is_some_and(|p| ast.is::<ForEachPartsWithDeclaration>(p)) {
                let keyword = ast[id].keyword;
                builder.add_dart_file_edit(c.path, |b| {
                    if is_var(c, keyword) {
                        let t = r.token(keyword.unwrap());
                        b.add_simple_replacement(t.offset, t.length, "final");
                    } else if keyword.is_none() {
                        b.add_simple_insertion(ast.offset(id), "final ");
                    }
                });
                return;
            }
        }
        if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
            builder.add_dart_file_edit(c.path, |b| {
                let keyword = ast[p].const_final_or_var_keyword;
                if is_var(c, keyword) {
                    let t = r.token(keyword.unwrap());
                    b.add_simple_replacement(t.offset, t.length, "final");
                } else if let Some(ty) = ast[p].type_ {
                    b.add_simple_insertion(ast.offset(ty), "final ");
                } else if let Some(name) = ast[p].name {
                    b.add_simple_insertion(c.token_offset(name), "final ");
                } else {
                    b.add_simple_insertion(ast.offset(p), "final ");
                }
            });
            return;
        }
        if let Some(d) = ast.cast::<PatternVariableDeclaration>(node) {
            let keyword = ast[d].keyword;
            builder.add_dart_file_edit(c.path, |b| {
                if c.lexeme(keyword) == "var" {
                    let t = r.token(keyword);
                    b.add_simple_replacement(t.offset, t.length, "final");
                }
            });
            return;
        }
        // Dart `forEachPartsParent`.
        let for_pattern = parent.and_then(|p| {
            if ast.is::<ForEachPartsWithPattern>(p) {
                Some(p)
            } else {
                ast.parent(p)
            }
        });
        if let Some(f) = for_pattern.and_then(|f| ast.cast::<ForEachPartsWithPattern>(f)) {
            let keyword = ast[f].keyword;
            builder.add_dart_file_edit(c.path, |b| {
                if c.lexeme(keyword) == "var" {
                    let t = r.token(keyword);
                    b.add_simple_replacement(t.offset, t.length, "final");
                }
            });
            return;
        }
        if let Some(p) = ast.cast::<DeclaredVariablePattern>(node) {
            // Dart `patternContext`: the enclosing `for (var (...) in ...)`.
            let mut context = ast.parent(p);
            while let Some(n) = context {
                if ast.is::<ForEachPartsWithPattern>(n)
                    || ast.is::<PatternVariableDeclaration>(n)
                    || ast.is::<PatternAssignment>(n)
                    || ast.is::<GuardedPattern>(n)
                {
                    break;
                }
                context = ast.parent(n);
            }
            if let Some(f) = context.and_then(|n| ast.cast::<ForEachPartsWithPattern>(n)) {
                let t = r.token(ast[f].keyword);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(t.offset, t.length, "final")
                });
            } else if let Some(keyword) = ast[p].keyword {
                if ast[p].type_.is_none() {
                    let t = r.token(keyword);
                    builder.add_dart_file_edit(c.path, |b| {
                        b.add_simple_replacement(t.offset, t.length, "final")
                    });
                }
            } else {
                let offset = ast.offset(p);
                builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "final "));
            }
            return;
        }
        let Some(list) = variable_declaration_list(ast, node) else {
            return;
        };
        if ast.list(ast[list].variables).len() != 1 {
            return;
        }
        let keyword = ast[list].keyword;
        let late_keyword = ast[list].late_keyword;
        builder.add_dart_file_edit(c.path, |b| {
            if is_var(c, keyword) {
                let t = r.token(keyword.unwrap());
                b.add_simple_replacement(t.offset, t.length, "final");
            } else if let Some(late) = late_keyword {
                b.add_simple_insertion(c.token_end(late), " final");
            } else if keyword.is_none() {
                b.add_simple_insertion(ast.offset(list), "final ");
            }
        });
    }
);

/// Dart `RemoveInitializer` (the three constructors).
pub struct RemoveInitializer {
    pub applicability: Applicability,
    pub remove_late: bool,
}

/// The name, type and default value of a formal parameter.
fn parameter_name_and_default(
    ast: &Ast,
    parameter: NodeId,
) -> (Option<dartr_syntax::TokenId>, Option<NodeId>) {
    let default = |d: Option<Id<FormalParameterDefaultClause>>| d.map(|d| ast[d].value.raw());
    if let Some(p) = ast.cast::<RegularFormalParameter>(parameter) {
        (ast[p].name, default(ast[p].default_clause))
    } else if let Some(p) = ast.cast::<FieldFormalParameter>(parameter) {
        (Some(ast[p].name), default(ast[p].default_clause))
    } else if let Some(p) = ast.cast::<SuperFormalParameter>(parameter) {
        (Some(ast[p].name), default(ast[p].default_clause))
    } else {
        (None, None)
    }
}

impl CorrectionProducer for RemoveInitializer {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_INITIALIZER)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_INITIALIZER_MULTI)
    }

    fn applicability(&self) -> Applicability {
        self.applicability
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let r = c.range();
        if let Some(parameter) = ast.this_or_ancestor_of_type::<FormalParameter>(c.node) {
            let (name, default) = parameter_name_and_default(ast, parameter.raw());
            if let (Some(name), Some(default)) = (name, default) {
                let range = r.token_end_node_end(name, default);
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
            }
            return;
        }
        let variable = ast.this_or_ancestor_of_type::<VariableDeclaration>(c.node);
        if let Some(variable) = variable {
            if let Some(initializer) = ast[variable].initializer {
                let range = r.token_end_node_end(ast[variable].name, initializer);
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
                let is_late = ast
                    .parent(variable)
                    .and_then(|p| ast.cast::<VariableDeclarationList>(p))
                    .is_some_and(|l| ast[l].late_keyword.is_some());
                if self.remove_late && is_late {
                    if let Some(parent) = ast.parent(c.node) {
                        let begin = ast.begin_token(parent);
                        let token = ast.tokens.get(begin);
                        let (offset, length) = (token.offset, token.length + 1);
                        builder.add_dart_file_edit(c.path, |b| b.add_deletion(offset, length));
                    }
                }
                return;
            }
        }
        let initializer = ast.this_or_ancestor_of_type::<ConstructorFieldInitializer>(c.node);
        let Some(initializer) = initializer else {
            return;
        };
        let Some(constructor) = ast
            .parent(initializer)
            .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
        else {
            return;
        };
        let list = ast.list_raw(ast[constructor].initializers).to_vec();
        let range = r.node_in_list(&list, initializer.raw());
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
    }
}
