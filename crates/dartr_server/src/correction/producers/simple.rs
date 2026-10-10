// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unused_import.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_override.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_this_expression.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_interpolation_braces.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_new.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_parentheses.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_empty_constructor_body.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_method_declaration.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_final_with_const.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_conditional_assignment.dart

//! Small producers that delete or replace a few tokens.

use dartr_ast::*;
use dartr_syntax::TokenId;

use super::super::change::Position;
use super::super::change_builder::ChangeBuilder;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;

/// Declares a producer without state: name, fix kind, multi fix kind,
/// applicability and the body of `compute`.
macro_rules! producer {
    ($name:ident, $kind:expr, $multi:expr, $app:ident, |$c:ident, $b:ident| $body:block) => {
        producer!($name, $kind, $multi, $app, assist: None, |$c, $b| $body);
    };
    ($name:ident, $kind:expr, $multi:expr, $app:ident, assist: $assist:expr, |$c:ident, $b:ident| $body:block) => {
        pub struct $name;
        impl CorrectionProducer for $name {
            fn fix_kind(&self) -> Option<&'static $crate::correction::fix_kind::FixKind> {
                Some(&$kind)
            }
            fn assist_kind(&self) -> Option<&'static $crate::correction::fix_kind::FixKind> {
                $assist
            }
            fn multi_fix_kind(&self) -> Option<&'static $crate::correction::fix_kind::FixKind> {
                $multi
            }
            fn applicability(&self) -> Applicability {
                Applicability::$app
            }
            fn compute(&mut self, $c: &ProducerContext<'_>, $b: &mut ChangeBuilder<'_>) $body
        }
    };
}
pub(crate) use producer;

/// A producer that is only an assist (Dart `assistKind` without `fixKind`).
macro_rules! assist {
    ($name:ident, $kind:expr, |$c:ident, $b:ident| $body:block) => {
        pub struct $name;
        impl CorrectionProducer for $name {
            fn fix_kind(&self) -> Option<&'static $crate::correction::fix_kind::FixKind> {
                None
            }
            fn assist_kind(&self) -> Option<&'static $crate::correction::fix_kind::FixKind> {
                Some(&$kind)
            }
            fn applicability(&self) -> Applicability {
                Applicability::SingleLocation
            }
            fn compute(&mut self, $c: &ProducerContext<'_>, $b: &mut ChangeBuilder<'_>) $body
        }
    };
}
pub(crate) use assist;

/// The first token of [node] after its documentation comment (Dart:
/// `beginToken`, or the parent of a comment token).
pub fn begin_token_after_comment(ast: &Ast, node: NodeId) -> TokenId {
    let begin = ast.begin_token(node);
    if !ast.tokens.get(begin).is_comment() {
        return begin;
    }
    // The token that the comment precedes: the first token of the node
    // that is not a comment.
    let mut t = begin;
    let end = ast.end_token(node);
    while ast.tokens.get(t).is_comment() && t != end {
        let next = ast.tokens.get(t).next;
        if next.is_none() {
            break;
        }
        t = next;
    }
    if ast.tokens.get(t).is_comment() {
        // Comments are not linked to the next token: find the token after
        // the comment by offset.
        let comment_end = ast.tokens.get(t).end();
        let mut x = ast.begin_token(node);
        for e in ast.child_entities(node) {
            let offset = ast.entity_offset(e);
            if offset >= comment_end {
                x = match e {
                    Entity::Node(n) => begin_token_after_comment(ast, n),
                    Entity::Token(tok) => tok,
                };
                break;
            }
        }
        return x;
    }
    t
}

producer!(
    RemoveUnusedImport,
    k::REMOVE_UNUSED_IMPORT,
    Some(&k::REMOVE_UNUSED_IMPORT_MULTI),
    AcrossSingleFile,
    |c, builder| {
        let Some(directive) = c.ast.this_or_ancestor_matching(c.node, |ast, n| {
            ast.is::<ImportDirective>(n)
                || ast.is::<ExportDirective>(n)
                || ast.is::<PartDirective>(n)
        }) else {
            return;
        };
        let range = c.utils.get_lines_range(c.range().node(directive), false);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
    }
);

producer!(
    AddOverride,
    k::ADD_OVERRIDE,
    Some(&k::ADD_OVERRIDE_MULTI),
    Automatically,
    |c, builder| {
        let Some(member) = c.ast.this_or_ancestor_of_type::<ClassMember>(c.node) else {
            return;
        };
        let token = begin_token_after_comment(c.ast, member.raw());
        let offset = c.token_offset(token);
        let indent = c.utils.one_indent();
        builder.add_dart_file_edit(c.path, |b| {
            let eol = b.eol();
            b.add_simple_replacement(offset, 0, &format!("@override{eol}{indent}"));
        });
        builder.set_selection(Position {
            file: c.path.to_string(),
            offset: offset.saturating_sub(1),
        });
    }
);

producer!(
    RemoveThisExpression,
    k::REMOVE_THIS_EXPRESSION,
    Some(&k::REMOVE_THIS_EXPRESSION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let r = c.range();
        if let Some(init) = ast.cast::<ConstructorFieldInitializer>(c.node) {
            if let Some(this_keyword) = ast[init].this_keyword {
                let range = r.token_start_node_start(this_keyword, ast[init].field_name);
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
            }
            return;
        }
        let Some(parent) = ast.parent(c.node) else {
            return;
        };
        if let Some(p) = ast.cast::<PropertyAccess>(parent) {
            if ast[p].target.is_some_and(|t| ast.is::<ThisExpression>(t)) {
                let range = r.node_start_token_end(p, ast[p].operator);
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
            }
        } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
            if let (Some(target), Some(operator)) = (ast[m].target, ast[m].operator) {
                if ast.is::<ThisExpression>(target) {
                    let range = r.node_start_token_end(m, operator);
                    builder
                        .add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
                }
            }
        }
    }
);

producer!(
    RemoveInterpolationBraces,
    k::REMOVE_INTERPOLATION_BRACES,
    Some(&k::REMOVE_INTERPOLATION_BRACES_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(node) = ast.cast::<InterpolationExpression>(c.node) else {
            return;
        };
        let Some(right) = ast[node].right_bracket else {
            return;
        };
        let r = c.range();
        let start = r.start_start(node, ast[node].expression);
        let right = r.token(right);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(start.offset, start.length, "$");
            b.add_deletion(right.offset, right.length);
        });
    }
);

/// Dart `_RemoveNew.compute`.
fn remove_new(c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
    let ast = c.ast;
    let Some(creation) = ast.cast::<InstanceCreationExpression>(c.node) else {
        return;
    };
    let Some(new_token) = ast[creation].keyword else {
        return;
    };
    let range = c
        .range()
        .token_start_start(new_token, ast.tokens.next(new_token));
    builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
}

producer!(
    RemoveNew,
    k::REMOVE_NEW,
    None,
    SingleLocation,
    |c, builder| {
        remove_new(c, builder);
    }
);

producer!(
    RemoveUnnecessaryNew,
    k::REMOVE_UNNECESSARY_NEW,
    Some(&k::REMOVE_UNNECESSARY_NEW_MULTI),
    Automatically,
    |c, builder| {
        remove_new(c, builder);
    }
);

producer!(
    RemoveUnnecessaryParentheses,
    k::REMOVE_UNNECESSARY_PARENTHESES,
    Some(&k::REMOVE_UNNECESSARY_PARENTHESES_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(outer) = c
            .covering_node()
            .and_then(|n| ast.cast::<ParenthesizedExpression>(n))
        else {
            return;
        };
        if ast.is::<ParenthesizedExpression>(ast[outer].expression) {
            return;
        }
        let left = ast[outer].left_parenthesis;
        let right = ast[outer].right_parenthesis;
        let previous = ast.tokens.previous(left);
        let needs_space = previous.is_some()
            && ast.tokens.get(previous).is_keyword_or_identifier()
            && ast.tokens.get(previous).end() == ast.tokens.get(left).offset;
        let left = c.range().token(left);
        let right = c.range().token(right);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(left.offset, left.length, |e| {
                if needs_space {
                    e.write(" ");
                }
            });
            b.add_deletion(right.offset, right.length);
        });
    }
);

producer!(
    RemoveEmptyConstructorBody,
    k::REMOVE_EMPTY_CONSTRUCTOR_BODY,
    Some(&k::REMOVE_EMPTY_CONSTRUCTOR_BODY_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(block) = ast.cast::<Block>(c.node) else {
            return;
        };
        if ast.tokens.get(ast[block].left_bracket).is_synthetic()
            || ast.tokens.get(ast[block].right_bracket).is_synthetic()
        {
            return;
        }
        let Some(body) = ast
            .parent(block)
            .and_then(|p| ast.cast::<BlockFunctionBody>(p))
        else {
            return;
        };
        let range = c.utils.get_lines_range(c.range().node(body), false);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, ";")
        });
    }
);

producer!(
    RemoveMethodDeclaration,
    k::REMOVE_METHODDECLARATION,
    Some(&k::REMOVE_METHOD_DECLARATION_MULTI),
    Automatically,
    |c, builder| {
        let Some(declaration) = c.ast.this_or_ancestor_of_type::<MethodDeclaration>(c.node) else {
            return;
        };
        let range = c.utils.get_lines_range(c.range().node(declaration), false);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
    }
);

producer!(
    ReplaceFinalWithConst,
    k::REPLACE_FINAL_WITH_CONST,
    Some(&k::REPLACE_FINAL_WITH_CONST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(list) = ast.cast::<VariableDeclarationList>(c.node) else {
            return;
        };
        let r = c.range();
        if let Some(keyword) = ast[list].keyword {
            let range = r.token(keyword);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, "const")
            });
        }
        for &variable in ast.list(ast[list].variables) {
            let Some(initializer) = ast[variable].initializer else {
                continue;
            };
            let const_token = if let Some(i) = ast.cast::<InstanceCreationExpression>(initializer) {
                ast[i].keyword
            } else if let Some(d) = ast.cast::<DotShorthandConstructorInvocation>(initializer) {
                ast[d].const_keyword
            } else if let Some(l) = ast.cast::<ListLiteral>(initializer) {
                ast[l].const_keyword
            } else if let Some(l) = ast.cast::<SetOrMapLiteral>(initializer) {
                ast[l].const_keyword
            } else {
                None
            };
            let Some(const_token) = const_token else {
                continue;
            };
            let range = r.token_start_start(const_token, ast.tokens.next(const_token));
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        }
    }
);

/// Dart `ReplaceWithConditionalAssignment._uniqueStatement`.
fn unique_statement(ast: &Ast, statement: NodeId) -> Option<NodeId> {
    if let Some(block) = ast.cast::<Block>(statement) {
        let first = *ast.list_raw(ast[block].statements).first()?;
        return unique_statement(ast, first);
    }
    Some(statement)
}

/// Dart `Expression.unParenthesized`.
pub fn un_parenthesized(ast: &Ast, mut e: NodeId) -> NodeId {
    while let Some(p) = ast.cast::<ParenthesizedExpression>(e) {
        e = ast[p].expression.raw();
    }
    e
}

producer!(
    ReplaceWithConditionalAssignment,
    k::REPLACE_WITH_CONDITIONAL_ASSIGNMENT,
    Some(&k::REPLACE_WITH_CONDITIONAL_ASSIGNMENT_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(if_statement) = ast.this_or_ancestor_of_type::<IfStatement>(c.node) else {
            return;
        };
        let Some(then) = unique_statement(ast, ast[if_statement].then_statement.raw()) else {
            return;
        };
        let Some(statement) = ast.cast::<ExpressionStatement>(then) else {
            return;
        };
        let expression = un_parenthesized(ast, ast[statement].expression.raw());
        let Some(assignment) = ast.cast::<AssignmentExpression>(expression) else {
            return;
        };
        let left = c.utils.get_node_text(ast[assignment].left_hand_side);
        let right = c.utils.get_node_text(ast[assignment].right_hand_side);
        let range = c.range().node(if_statement);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                e.write(&left);
                e.write(" ??= ");
                e.write(&right);
                e.write(";");
            });
        });
    }
);
