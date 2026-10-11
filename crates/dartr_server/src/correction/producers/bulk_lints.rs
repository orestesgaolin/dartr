// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_trailing_comma.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/sort_constructor_first.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/sort_unnamed_constructor_first.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/make_required_named_parameters_first.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_tear_off.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_argument.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_library_directive.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_library_name.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/split_multiple_declarations.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_raw_string.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_raw_string.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/sort_combinators.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_await.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_leading_underscore.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_boolean_expression.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/inline_typedef.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_invocation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_break.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_if_null.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_for_each_to_for_loop.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_const.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_leading_newline_to_string.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_function_declaration.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_interpolation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_comparison.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unused_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_return_type_future.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/move_doc_comment_to_library_directive.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_cast.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_string_interpolation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_decorated_box.dart

//! Bulk-fixable lint and hint correction producers.

use std::collections::HashSet;

use dartr_ast::*;
use dartr_element::{ElementId, FormalParameterElement, Nullability, Tag, TypeId, TypeKind};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::ChangeBuilder;
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::flutter::{URI_CONTAINER, is_exact_widget_type};
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::super::utils::Range;
use super::assists2::invert_condition;
use super::create::static_type;
use super::simple::{begin_token_after_comment, producer};

fn unparenthesized(ast: &Ast, mut node: NodeId) -> NodeId {
    while let Some(pe) = ast.cast::<ParenthesizedExpression>(node) {
        node = ast[pe].expression.raw();
    }
    node
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ExprPrecedence {
    Assignment = 1,
    Cascade = 2,
    Conditional = 3,
    IfNull = 4,
    LogicalOr = 5,
    LogicalAnd = 6,
    Equality = 7,
    Relational = 8,
    BitwiseOr = 9,
    BitwiseXor = 10,
    BitwiseAnd = 11,
    Shift = 12,
    Additive = 13,
    Multiplicative = 14,
    Prefix = 15,
    Postfix = 16,
    Primary = 17,
}

fn expr_precedence(ast: &Ast, expr: NodeId) -> ExprPrecedence {
    if let Some(bin) = ast.cast::<BinaryExpression>(expr) {
        return match ast.tokens.ty(ast[bin].operator) {
            TokenType::QUESTION_QUESTION => ExprPrecedence::IfNull,
            TokenType::BAR_BAR => ExprPrecedence::LogicalOr,
            TokenType::AMPERSAND_AMPERSAND => ExprPrecedence::LogicalAnd,
            TokenType::EQ_EQ | TokenType::BANG_EQ => ExprPrecedence::Equality,
            TokenType::LT | TokenType::GT | TokenType::LT_EQ | TokenType::GT_EQ => {
                ExprPrecedence::Relational
            }
            TokenType::BAR => ExprPrecedence::BitwiseOr,
            TokenType::CARET => ExprPrecedence::BitwiseXor,
            TokenType::AMPERSAND => ExprPrecedence::BitwiseAnd,
            TokenType::LT_LT | TokenType::GT_GT | TokenType::GT_GT_GT => ExprPrecedence::Shift,
            TokenType::PLUS | TokenType::MINUS => ExprPrecedence::Additive,
            TokenType::STAR | TokenType::SLASH | TokenType::TILDE_SLASH | TokenType::PERCENT => {
                ExprPrecedence::Multiplicative
            }
            _ => ExprPrecedence::Primary,
        };
    }
    if ast.is::<AssignmentExpression>(expr)
        || ast.is::<ThrowExpression>(expr)
        || ast.is::<FunctionExpression>(expr)
    {
        ExprPrecedence::Assignment
    } else if ast.is::<CascadeExpression>(expr) {
        ExprPrecedence::Cascade
    } else if ast.is::<ConditionalExpression>(expr) {
        ExprPrecedence::Conditional
    } else if ast.is::<IsExpression>(expr) || ast.is::<AsExpression>(expr) {
        ExprPrecedence::Relational
    } else if ast.is::<PrefixExpression>(expr) || ast.is::<AwaitExpression>(expr) {
        ExprPrecedence::Prefix
    } else if ast.is::<PostfixExpression>(expr)
        || ast.is::<MethodInvocation>(expr)
        || ast.is::<FunctionExpressionInvocation>(expr)
        || ast.is::<PropertyAccess>(expr)
        || ast.is::<PrefixedIdentifier>(expr)
        || ast.is::<IndexExpression>(expr)
    {
        ExprPrecedence::Postfix
    } else {
        ExprPrecedence::Primary
    }
}

/// Dart `RangeFactory.nodeInListWithComments`.
fn node_in_list_with_comments(
    c: &ProducerContext<'_>,
    list: &[NodeId],
    node: NodeId,
) -> Option<Range> {
    let ast = c.ast;
    if list.len() == 1 {
        if list[0] != node {
            return None;
        }
        let leading = c
            .range()
            .leading_comment(c.line_info, ast.begin_token(node));
        let (trailing, _) = c
            .range()
            .trailing_comment(c.line_info, ast.end_token(node), true);
        return Some(c.range().token_start_end(leading, trailing));
    }
    let index = list.iter().position(|&n| n == node)?;
    if index == 0 {
        let this_leading = c
            .range()
            .leading_comment(c.line_info, ast.begin_token(node));
        let next_leading = c
            .range()
            .leading_comment(c.line_info, ast.begin_token(list[1]));
        Some(c.range().token_start_start(this_leading, next_leading))
    } else {
        let (mut prev_token, prev_includes_comma) =
            c.range()
                .trailing_comment(c.line_info, ast.end_token(list[index - 1]), false);
        let (this_trailing, this_includes_comma) =
            c.range()
                .trailing_comment(c.line_info, ast.end_token(node), prev_includes_comma);
        if !prev_includes_comma && this_includes_comma {
            prev_token = ast.tokens.next(prev_token);
        }
        Some(c.range().token_end_end(prev_token, this_trailing))
    }
}

// ---------------------------------------------------------------------------
// 1. AddTrailingComma
// ---------------------------------------------------------------------------

producer!(
    AddTrailingComma,
    k::ADD_TRAILING_COMMA,
    Some(&k::ADD_TRAILING_COMMA_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let n = c.node;
        let last_node = if let Some(al) = ast.cast::<ArgumentList>(n) {
            ast.list_raw(ast[al].arguments).last().copied()
        } else if let Some(fpl) = ast.cast::<FormalParameterList>(n) {
            ast.list_raw(ast[fpl].parameters).last().copied()
        } else if let Some(ai) = ast.cast::<AssertInitializer>(n) {
            Some(
                ast[ai]
                    .message
                    .map(|m| m.raw())
                    .unwrap_or_else(|| ast[ai].condition.raw()),
            )
        } else if let Some(as_) = ast.cast::<AssertStatement>(n) {
            Some(
                ast[as_]
                    .message
                    .map(|m| m.raw())
                    .unwrap_or_else(|| ast[as_].condition.raw()),
            )
        } else if let Some(ll) = ast.cast::<ListLiteral>(n) {
            ast.list_raw(ast[ll].elements).last().copied()
        } else if let Some(sml) = ast.cast::<SetOrMapLiteral>(n) {
            ast.list_raw(ast[sml].elements).last().copied()
        } else if let Some(pe) = ast.cast::<ParenthesizedExpression>(n) {
            Some(ast[pe].expression.raw())
        } else if let Some(rl) = ast.cast::<RecordLiteral>(n) {
            ast.list_raw(ast[rl].fields).last().copied()
        } else if let Some(rta) = ast.cast::<RecordTypeAnnotation>(n) {
            ast.list_raw(ast[rta].positional_fields).last().copied()
        } else {
            None
        };
        let Some(last) = last_node else {
            return;
        };
        let off = ast.end(last);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(off, ",");
        });
    }
);

// ---------------------------------------------------------------------------
// 2. SortConstructorFirst
// ---------------------------------------------------------------------------

producer!(
    SortConstructorFirst,
    k::SORT_CONSTRUCTOR_FIRST,
    Some(&k::SORT_CONSTRUCTOR_FIRST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(covering) = c.covering_node() else {
            return;
        };
        let Some(ctor) = ast
            .parent(covering)
            .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
            .or_else(|| ast.cast::<ConstructorDeclaration>(covering))
        else {
            return;
        };
        let Some(body) = ast.parent(ctor) else {
            return;
        };
        let ins_off = if let Some(bcb) = ast.cast::<BlockClassBody>(body) {
            c.token_end(ast[bcb].left_bracket)
        } else if let Some(beb) = ast.cast::<BlockEnumBody>(body) {
            ast[beb]
                .semicolon
                .map(|t| c.token_end(t))
                .or_else(|| ast.list(ast[beb].constants).last().map(|&ec| ast.end(ec)))
                .unwrap_or_else(|| c.token_end(ast[beb].left_bracket))
        } else {
            return;
        };
        let first = begin_token_after_comment(ast, ctor.raw());
        let del = c
            .range()
            .token_end_end(ast.tokens.previous(first), ast.end_token(ctor));
        let text = c.utils.get_range_text(del);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
            b.add_simple_insertion(ins_off, &text);
        });
    }
);

// ---------------------------------------------------------------------------
// 3. SortUnnamedConstructorFirst
// ---------------------------------------------------------------------------

producer!(
    SortUnnamedConstructorFirst,
    k::SORT_UNNAMED_CONSTRUCTOR_FIRST,
    Some(&k::SORT_UNNAMED_CONSTRUCTOR_FIRST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(clazz) = c
            .covering_node()
            .and_then(|n| ast.this_or_ancestor_of_type::<ClassDeclaration>(n))
        else {
            return;
        };
        let Some(bcb) = ast.cast::<BlockClassBody>(ast[clazz].body) else {
            return;
        };
        let ctors: Vec<Id<ConstructorDeclaration>> = ast
            .list_raw(ast[bcb].members)
            .iter()
            .filter_map(|&m| ast.cast::<ConstructorDeclaration>(m))
            .collect();
        let Some(&first_ctor) = ctors.first() else {
            return;
        };
        let Some(&unnamed_ctor) = ctors
            .iter()
            .find(|&&cd| ast[cd].name.is_none_or(|n| c.lexeme(n) == "new"))
        else {
            return;
        };
        if unnamed_ctor == first_ctor {
            return;
        }
        let u_first = begin_token_after_comment(ast, unnamed_ctor.raw());
        let del = c
            .range()
            .token_end_end(ast.tokens.previous(u_first), ast.end_token(unnamed_ctor));
        let f_first = begin_token_after_comment(ast, first_ctor.raw());
        let ins_off = c.token_end(ast.tokens.previous(f_first));
        let text = c.utils.get_range_text(del);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
            b.add_simple_insertion(ins_off, &text);
        });
    }
);

// ---------------------------------------------------------------------------
// 4. MakeRequiredNamedParametersFirst
// ---------------------------------------------------------------------------

fn formal_param_is_named_and_required(ast: &Ast, param: NodeId) -> Option<bool> {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        if ast[rfp].kind.is_named() {
            return Some(ast[rfp].required_keyword.is_some());
        }
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        if ast[ffp].kind.is_named() {
            return Some(ast[ffp].required_keyword.is_some());
        }
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        if ast[sfp].kind.is_named() {
            return Some(ast[sfp].required_keyword.is_some());
        }
    }
    None
}

producer!(
    MakeRequiredNamedParametersFirst,
    k::MAKE_REQUIRED_NAMED_PARAMETERS_FIRST,
    Some(&k::MAKE_REQUIRED_NAMED_PARAMETERS_FIRST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(param_list) = ast.this_or_ancestor_of_type::<FormalParameterList>(c.node) else {
            return;
        };
        let Some(target_param) = ast.this_or_ancestor_of_type::<FormalParameter>(c.node) else {
            return;
        };
        let params = ast.list_raw(ast[param_list].parameters);
        let mut first_optional_idx: Option<usize> = None;
        let mut required_indices: Vec<usize> = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            if let Some(is_req) = formal_param_is_named_and_required(ast, p) {
                if is_req {
                    if first_optional_idx.is_some() {
                        if p == target_param.raw() && !required_indices.is_empty() {
                            return;
                        }
                        required_indices.push(i);
                    }
                } else if first_optional_idx.is_none() {
                    first_optional_idx = Some(i);
                }
            }
        }
        let Some(first_opt) = first_optional_idx else {
            return;
        };
        if required_indices.is_empty() {
            return;
        }
        let first_param = params[first_opt];
        let first_begin = ast.begin_token(first_param);
        let first_offset = ast
            .tokens
            .get(first_begin)
            .preceding_comments
            .get()
            .map(|t| c.token_offset(t))
            .unwrap_or_else(|| ast.offset(first_param));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_insertion(first_offset, |e| {
                for &idx in &required_indices {
                    let nr = c.range().node_with_comments(c.line_info, params[idx]);
                    let text = c.utils.get_range_text(nr);
                    e.write(&format!("{text}, "));
                }
            });
            let mut last_end = 0u32;
            for &idx in &required_indices {
                if let Some(sr) = node_in_list_with_comments(c, params, params[idx]) {
                    if sr.offset >= last_end {
                        b.add_deletion(sr.offset, sr.length);
                    }
                    last_end = sr.end();
                }
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 5. ReplaceWithTearOff
// ---------------------------------------------------------------------------

producer!(
    ReplaceWithTearOff,
    k::REPLACE_WITH_TEAR_OFF,
    Some(&k::REPLACE_WITH_TEAR_OFF_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(ancestor) = ast.this_or_ancestor_of_type::<FunctionExpression>(c.node) else {
            return;
        };
        let body = ast[ancestor].body;
        let expr = if let Some(efb) = ast.cast::<ExpressionFunctionBody>(body) {
            Some(ast[efb].expression.raw())
        } else if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
            let stmts = ast.list_raw(ast[ast[bfb].block].statements);
            if stmts.len() == 1 {
                if let Some(es) = ast.cast::<ExpressionStatement>(stmts[0]) {
                    Some(ast[es].expression.raw())
                } else if let Some(rs) = ast.cast::<ReturnStatement>(stmts[0]) {
                    ast[rs].expression.map(|e| e.raw())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let Some(expr) = expr else {
            return;
        };
        let mut rep = String::new();
        if let Some(mi) = ast.cast::<MethodInvocation>(expr) {
            if let Some(t) = ast[mi].target {
                rep.push_str(&c.utils.get_node_text(t));
                if let Some(op) = ast[mi].operator {
                    rep.push_str(c.lexeme(op));
                }
            }
            rep.push_str(&c.utils.get_node_text(ast[mi].method_name));
            if let Some(ta) = ast[mi].type_arguments {
                rep.push_str(&c.utils.get_node_text(ta));
            }
        } else if let Some(fei) = ast.cast::<FunctionExpressionInvocation>(expr) {
            rep.push_str(&c.utils.get_node_text(ast[fei].function));
            if let Some(ta) = ast[fei].type_arguments {
                rep.push_str(&c.utils.get_node_text(ta));
            }
        } else if let Some(ice) = ast.cast::<InstanceCreationExpression>(expr) {
            let cn = ast[ice].constructor_name;
            rep.push_str(&c.utils.get_node_text(cn));
            if ast[cn].name.is_none() {
                rep.push_str(".new");
            }
        } else {
            return;
        }
        let r = c.range().node(ancestor);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, &rep);
        });
    }
);

// ---------------------------------------------------------------------------
// 6. RemoveArgument
// ---------------------------------------------------------------------------

producer!(
    RemoveArgument,
    k::REMOVE_ARGUMENT,
    Some(&k::REMOVE_ARGUMENT_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let mut arg = c.node;
        if let Some(parent) = ast.parent(arg) {
            if ast.is::<NamedArgument>(parent) {
                arg = parent;
            }
        }
        if !ast.is::<Expression>(arg) && !ast.is::<NamedArgument>(arg) {
            return;
        }
        let Some(arg_list) = ast
            .parent(arg)
            .and_then(|p| ast.this_or_ancestor_of_type::<ArgumentList>(p))
        else {
            return;
        };
        let args = ast.list_raw(ast[arg_list].arguments);
        let Some(del) = node_in_list_with_comments(c, args, arg) else {
            return;
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 7. RemoveUnnecessaryLibraryDirective
// ---------------------------------------------------------------------------

producer!(
    RemoveUnnecessaryLibraryDirective,
    k::REMOVE_UNNECESSARY_LIBRARY_DIRECTIVE,
    None,
    AutomaticallyButOncePerFile,
    |c, builder| {
        let ast = c.ast;
        let Some(directive) = ast.this_or_ancestor_of_type::<LibraryDirective>(c.node) else {
            return;
        };
        let del = c.utils.get_lines_range(c.range().node(directive), false);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 8. RemoveLibraryName
// ---------------------------------------------------------------------------

producer!(
    RemoveLibraryName,
    k::REMOVE_LIBRARY_NAME,
    None,
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(directive) = ast.this_or_ancestor_of_type::<LibraryDirective>(c.node) else {
            return;
        };
        let del = c
            .range()
            .token_end_start(ast[directive].library_keyword, ast[directive].semicolon);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 9. SplitMultipleDeclarations
// ---------------------------------------------------------------------------

producer!(
    SplitMultipleDeclarations,
    k::SPLIT_MULTIPLE_DECLARATIONS,
    Some(&k::SPLIT_MULTIPLE_DECLARATIONS_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(declared_var) = ast.cast::<VariableDeclaration>(c.node) else {
            return;
        };
        let Some(var_list) = ast
            .parent(declared_var)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
        else {
            return;
        };
        if ast
            .tokens
            .get(ast.begin_token(var_list))
            .preceding_comments
            .is_some()
        {
            return;
        }
        let Some(decl) = ast.parent(var_list) else {
            return;
        };
        let has_metadata_or_comments = if let Some(fd) = ast.cast::<FieldDeclaration>(decl) {
            !ast.list_raw(ast[fd].metadata).is_empty() || ast[fd].documentation_comment.is_some()
        } else if let Some(tlvd) = ast.cast::<TopLevelVariableDeclaration>(decl) {
            !ast.list_raw(ast[tlvd].metadata).is_empty()
                || ast[tlvd].documentation_comment.is_some()
        } else if let Some(vds) = ast.cast::<VariableDeclarationStatement>(decl) {
            !ast.list_raw(ast[ast[vds].variables].metadata).is_empty()
        } else {
            true
        };
        if has_metadata_or_comments {
            return;
        }
        if ast
            .tokens
            .get(ast.begin_token(decl))
            .preceding_comments
            .is_some()
        {
            return;
        }
        let vars = ast.list(ast[var_list].variables);
        if vars.len() < 2 {
            return;
        }
        let modifiers_type_last = ast.tokens.previous(ast[vars[0]].name);
        if modifiers_type_last.is_none() {
            return;
        }
        let modifiers_type = c
            .utils
            .get_range_text(c.range().node_start_token_end(decl, modifiers_type_last));
        let spaces_before = c.utils.get_line_prefix(ast.offset(decl)).to_string();
        let eol = c.eol().to_string();
        let rep = format!(";{eol}{spaces_before}{modifiers_type} ");
        let ranges: Vec<Range> = (1..vars.len())
            .map(|i| c.range().end_start(vars[i - 1], vars[i]))
            .collect();
        builder.add_dart_file_edit(c.path, |b| {
            for r in &ranges {
                b.add_simple_replacement(r.offset, r.length, &rep);
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 10. RemoveUnnecessaryRawString
// ---------------------------------------------------------------------------

producer!(
    RemoveUnnecessaryRawString,
    k::REMOVE_UNNECESSARY_RAW_STRING,
    Some(&k::REMOVE_UNNECESSARY_RAW_STRING_MULTI),
    Automatically,
    |c, builder| {
        let off = c.ast.offset(c.node);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(off, 1);
        });
    }
);

// ---------------------------------------------------------------------------
// 11. ConvertToRawString
// ---------------------------------------------------------------------------

producer!(
    ConvertToRawString,
    k::CONVERT_TO_RAW_STRING,
    Some(&k::CONVERT_TO_RAW_STRING_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(lit) = ast.cast::<SimpleStringLiteral>(c.node) else {
            return;
        };
        let tok = ast[lit].literal;
        let lex = c.lexeme(tok);
        if lex.starts_with('r') || lex.starts_with('R') {
            return;
        }
        let quote_len: u32 = if lex.starts_with("\"\"\"") || lex.starts_with("'''") {
            3
        } else {
            1
        };
        let contents_offset = c.token_offset(tok) + quote_len;
        let contents_end = c.token_end(tok) - quote_len;
        let mut deletions: Vec<u32> = Vec::new();
        let mut off = contents_offset;
        while off < contents_end {
            if c.utils.text.at(off) == b'\\' as u16 {
                deletions.push(off);
                off += 2;
            } else {
                off += 1;
            }
        }
        if deletions.is_empty() {
            return;
        }
        let lit_off = ast.offset(lit);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(lit_off, "r");
            for &d in &deletions {
                b.add_deletion(d, 1);
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 12. SortCombinators
// ---------------------------------------------------------------------------

producer!(
    SortCombinators,
    k::SORT_COMBINATORS,
    Some(&k::SORT_COMBINATORS_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(node) = c.covering_node() else {
            return;
        };
        let ids: &[Id<SimpleIdentifier>] = if let Some(hc) = ast.cast::<HideCombinator>(node) {
            ast.list(ast[hc].hidden_names)
        } else if let Some(sc) = ast.cast::<ShowCombinator>(node) {
            ast.list(ast[sc].shown_names)
        } else {
            return;
        };
        if ids.len() < 2 {
            return;
        }
        let mut sorted: Vec<&str> = ids.iter().map(|&id| c.lexeme(ast[id].token)).collect();
        sorted.sort_unstable();
        builder.add_dart_file_edit(c.path, |b| {
            for (&id, &new_name) in ids.iter().zip(sorted.iter()) {
                let r = c.range().node(id);
                b.add_simple_replacement(r.offset, r.length, new_name);
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 13. RemoveAwait
// ---------------------------------------------------------------------------

producer!(
    RemoveAwait,
    k::REMOVE_AWAIT,
    Some(&k::REMOVE_AWAIT_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(aw) = ast.this_or_ancestor_of_type::<AwaitExpression>(c.node) else {
            return;
        };
        let kw = ast[aw].await_keyword;
        let del = c.range().token_start_start(kw, ast.tokens.next(kw));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 14. RemoveLeadingUnderscore
// ---------------------------------------------------------------------------

fn formal_param_name_token(ast: &Ast, node: NodeId) -> Option<TokenId> {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(node) {
        ast[rfp].name
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(node) {
        Some(ast[ffp].name)
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(node) {
        Some(ast[sfp].name)
    } else {
        None
    }
}

fn collect_element_refs(
    c: &ProducerContext<'_>,
    root: NodeId,
    target_elem: ElementId,
    out: &mut Vec<TokenId>,
) {
    let ast = c.ast;
    if let Some(id) = ast.cast::<SimpleIdentifier>(root) {
        if c.locator().write_or_read_element(id) == Some(target_elem)
            || c.locator().element(id) == Some(target_elem)
        {
            out.push(ast[id].token);
        }
    } else if let Some(avp) = ast.cast::<AssignedVariablePattern>(root) {
        if c.locator().element(avp) == Some(target_elem) {
            out.push(ast[avp].name);
        }
    } else if let Some(ipr) = ast.cast::<ImportPrefixReference>(root) {
        if c.locator().element(ipr) == Some(target_elem) {
            out.push(ast[ipr].name);
        }
    }
    for child in ast.children(root) {
        collect_element_refs(c, child, target_elem, out);
    }
}

fn collect_declared_names(ast: &Ast, root: NodeId, out: &mut HashSet<String>) {
    if let Some(vd) = ast.cast::<VariableDeclaration>(root) {
        out.insert(ast.tokens.lexeme(ast[vd].name).to_string());
    } else if let Some(dvp) = ast.cast::<DeclaredVariablePattern>(root) {
        out.insert(ast.tokens.lexeme(ast[dvp].name).to_string());
    } else if let Some(tok) = formal_param_name_token(ast, root) {
        out.insert(ast.tokens.lexeme(tok).to_string());
    } else if let Some(fd) = ast.cast::<FunctionDeclaration>(root) {
        out.insert(ast.tokens.lexeme(ast[fd].name).to_string());
    }
    for child in ast.children(root) {
        collect_declared_names(ast, child, out);
    }
}

producer!(
    RemoveLeadingUnderscore,
    k::REMOVE_LEADING_UNDERSCORE,
    Some(&k::REMOVE_LEADING_UNDERSCORE_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let node = c.node;
        let (name_token, element) = if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
            (Some(ast[id].token), c.locator().element(id))
        } else if let Some(vd) = ast.cast::<VariableDeclaration>(node) {
            (Some(ast[vd].name), c.locator().locate(vd.raw()))
        } else if let Some(dvp) = ast.cast::<DeclaredVariablePattern>(node) {
            (Some(ast[dvp].name), c.locator().locate(dvp.raw()))
        } else if let Some(fd) = ast.cast::<FunctionDeclaration>(node) {
            (Some(ast[fd].name), c.locator().locate(fd.raw()))
        } else if let Some(tok) = formal_param_name_token(ast, node) {
            (Some(tok), c.locator().locate(node))
        } else {
            (None, None)
        };
        let (Some(name_token), Some(element)) = (name_token, element) else {
            return;
        };
        let old_name = c.lexeme(name_token);
        if old_name.len() < 2 || !old_name.starts_with('_') {
            return;
        }
        let mut new_name = old_name[1..].to_string();

        let mut ref_tokens: Vec<TokenId> = Vec::new();
        if let Some(fp_elem) = element.cast::<FormalParameterElement>() {
            if c.ctx.get(fp_elem).kind.is_named() {
                return;
            }
            let mut root: Option<NodeId> = None;
            let mut cur = Some(node);
            while let Some(n) = cur {
                if ast.is::<FunctionDeclaration>(n)
                    || ast.is::<MethodDeclaration>(n)
                    || ast.is::<ConstructorDeclaration>(n)
                    || ast.is::<FunctionExpression>(n)
                {
                    root = Some(n);
                    break;
                }
                if let Some(pcb) = ast.cast::<PrimaryConstructorDeclaration>(n) {
                    root = ast.parent(pcb);
                    break;
                }
                cur = ast.parent(n);
            }
            let Some(root) = root else {
                return;
            };
            collect_element_refs(c, root, element, &mut ref_tokens);
        } else if element.tag() == Tag::Prefix {
            collect_element_refs(c, c.unit.raw(), element, &mut ref_tokens);
        } else {
            let Some(block) = ast.this_or_ancestor_of_type::<Block>(node) else {
                return;
            };
            collect_element_refs(c, block.raw(), element, &mut ref_tokens);
            let mut enc = ast.parent(block.raw());
            while let Some(n) = enc {
                if ast.is::<MethodDeclaration>(n) || ast.is::<FunctionDeclaration>(n) {
                    break;
                }
                enc = ast.parent(n);
            }
            if let Some(enc_node) = enc {
                let mut declared = HashSet::new();
                collect_declared_names(ast, enc_node, &mut declared);
                if declared.contains(&new_name) {
                    let base = new_name.clone();
                    let mut suffix = 0usize;
                    loop {
                        let cand = format!("{base}{suffix}");
                        if !declared.contains(&cand) {
                            new_name = cand;
                            break;
                        }
                        suffix += 1;
                    }
                }
            }
        }

        let mut all_tokens = vec![name_token];
        for t in ref_tokens {
            if !all_tokens.contains(&t) {
                all_tokens.push(t);
            }
        }
        builder.add_dart_file_edit(c.path, |b| {
            for t in all_tokens {
                let r = c.range().token(t);
                b.add_simple_replacement(r.offset, r.length, &new_name);
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 15. ConvertToBooleanExpression
// ---------------------------------------------------------------------------

fn is_negated_bool(ast: &Ast, lit: Id<BooleanLiteral>) -> bool {
    let value = ast.tokens.lexeme(ast[lit].literal) == "true";
    let mut cur = ast.parent(lit.raw());
    while let Some(p) = cur {
        if ast.is::<ParenthesizedExpression>(p) {
            cur = ast.parent(p);
            continue;
        }
        if let Some(pref) = ast.cast::<PrefixExpression>(p) {
            if ast.tokens.ty(ast[pref].operator) == TokenType::BANG {
                return value;
            }
        }
        break;
    }
    !value
}

fn bool_expr_needs_parens(ast: &Ast, expr: NodeId, operator: Option<TokenType>) -> bool {
    let target = match operator {
        Some(TokenType::BAR_BAR) => ExprPrecedence::LogicalOr,
        Some(TokenType::AMPERSAND_AMPERSAND) => ExprPrecedence::LogicalAnd,
        _ => ExprPrecedence::Prefix,
    };
    expr_precedence(ast, expr) <= target
}

#[allow(clippy::too_many_arguments)]
fn add_bool_expr_edit(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    node: NodeId,
    target_expr: NodeId,
    parens_range: Range,
    needs_parens: bool,
    delete_ranges: &[Range],
    negated: bool,
    bang_before_parens: bool,
    replace: Option<&str>,
    parens_range2: Option<Range>,
) {
    let node_range = c.range().node(node);
    let inverted = if negated && !needs_parens && parens_range != node_range {
        Some(invert_condition(c, target_expr))
    } else {
        None
    };
    builder.add_dart_file_edit(c.path, |b| {
        if negated {
            if needs_parens {
                b.add_simple_insertion(
                    parens_range.offset,
                    if bang_before_parens { "!(" } else { "(!" },
                );
                b.add_simple_insertion(parens_range.end(), ")");
            } else if let Some(ref inv) = inverted {
                b.add_simple_replacement(parens_range.offset, parens_range.length, inv);
            } else {
                b.add_simple_insertion(parens_range.offset, "!");
            }
        } else if needs_parens {
            b.add_simple_insertion(parens_range.offset, "(");
            b.add_simple_insertion(parens_range.end(), ")");
        }
        if let Some(pr2) = parens_range2 {
            b.add_simple_insertion(pr2.offset, "(");
            b.add_simple_insertion(pr2.end(), ")");
        }
        if !delete_ranges.is_empty() {
            for (i, r) in delete_ranges.iter().enumerate() {
                if i == 0 {
                    if let Some(rep) = replace {
                        b.add_simple_replacement(r.offset, r.length, rep);
                        continue;
                    }
                }
                b.add_deletion(r.offset, r.length);
            }
        } else if let Some(rep) = replace {
            b.add_simple_replacement(parens_range.offset, parens_range.length, rep);
        }
    });
}

producer!(
    ConvertToBooleanExpression,
    k::CONVERT_TO_BOOL_EXPRESSION,
    Some(&k::CONVERT_TO_BOOL_EXPRESSION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let mut node = c.node;
        if ast.is::<BooleanLiteral>(node) {
            let Some(p) = ast.parent(node) else {
                return;
            };
            node = p;
        }
        if let Some(bin) = ast.cast::<BinaryExpression>(node) {
            let left_expr = ast[bin].left_operand.raw();
            let right_expr = ast[bin].right_operand.raw();
            let mut lit_opt: Option<Id<BooleanLiteral>> = None;
            let mut expr = left_expr;
            let mut current_is_left = true;
            let right_unp = unparenthesized(ast, right_expr);
            if let Some(bl) = ast.cast::<BooleanLiteral>(right_unp) {
                lit_opt = Some(bl);
                expr = left_expr;
                current_is_left = false;
            }
            let left_unp = unparenthesized(ast, left_expr);
            if let Some(bl) = ast.cast::<BooleanLiteral>(left_unp) {
                lit_opt = Some(bl);
                expr = right_expr;
                current_is_left = true;
            } else if current_is_left {
                return;
            }
            let Some(literal) = lit_opt else {
                return;
            };
            let delete_ranges = if current_is_left {
                vec![c.range().start_start(left_expr, right_expr)]
            } else {
                vec![c.range().end_end(left_expr, right_expr)]
            };
            let negated = is_negated_bool(ast, literal);
            let op_ty = ast.tokens.ty(ast[bin].operator);
            let parens_op = match op_ty {
                TokenType::BAR if negated => Some(TokenType::BAR_BAR),
                TokenType::AMPERSAND if !negated => Some(TokenType::AMPERSAND_AMPERSAND),
                _ => None,
            };
            let needs_parens = bool_expr_needs_parens(ast, expr, parens_op);
            let parens_range = c.range().node(expr);
            match op_ty {
                TokenType::BAR | TokenType::BAR_BAR => {
                    if negated {
                        add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            expr,
                            parens_range,
                            false,
                            &delete_ranges,
                            false,
                            false,
                            None,
                            None,
                        );
                    } else {
                        add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            expr,
                            parens_range,
                            false,
                            &[],
                            false,
                            false,
                            Some("true"),
                            None,
                        );
                    }
                }
                TokenType::AMPERSAND | TokenType::AMPERSAND_AMPERSAND => {
                    if negated {
                        add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            expr,
                            parens_range,
                            false,
                            &[],
                            false,
                            false,
                            Some("false"),
                            None,
                        );
                    } else {
                        add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            expr,
                            parens_range,
                            false,
                            &delete_ranges,
                            false,
                            false,
                            None,
                            None,
                        );
                    }
                }
                TokenType::EQ_EQ => {
                    add_bool_expr_edit(
                        c,
                        builder,
                        node,
                        expr,
                        parens_range,
                        needs_parens,
                        &delete_ranges,
                        negated,
                        false,
                        None,
                        None,
                    );
                }
                TokenType::BANG_EQ | TokenType::CARET => {
                    add_bool_expr_edit(
                        c,
                        builder,
                        node,
                        expr,
                        parens_range,
                        needs_parens,
                        &delete_ranges,
                        !negated,
                        false,
                        None,
                        None,
                    );
                }
                _ => {}
            }
            return;
        }

        if let Some(cond) = ast.cast::<ConditionalExpression>(node) {
            let condition = ast[cond].condition.raw();
            let then_exp = ast[cond].then_expression.raw();
            let then_unp = unparenthesized(ast, then_exp);
            let else_exp = ast[cond].else_expression.raw();
            let else_unp = unparenthesized(ast, else_exp);
            match (
                ast.cast::<BooleanLiteral>(then_unp),
                ast.cast::<BooleanLiteral>(else_unp),
            ) {
                (Some(then_lit), Some(else_lit)) => {
                    let neg_then = is_negated_bool(ast, then_lit);
                    let neg_else = is_negated_bool(ast, else_lit);
                    let delete_ranges = vec![c.range().end_end(condition, else_exp)];
                    let needs_parens = bool_expr_needs_parens(ast, condition, None);
                    let parens_range = c.range().node(condition);
                    match (neg_then, neg_else) {
                        (false, false) => add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            condition,
                            parens_range,
                            false,
                            &delete_ranges,
                            false,
                            false,
                            Some("true"),
                            None,
                        ),
                        (true, true) => add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            condition,
                            parens_range,
                            false,
                            &delete_ranges,
                            false,
                            false,
                            Some("false"),
                            None,
                        ),
                        (false, true) => add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            condition,
                            parens_range,
                            false,
                            &delete_ranges,
                            false,
                            false,
                            None,
                            None,
                        ),
                        (true, false) => add_bool_expr_edit(
                            c,
                            builder,
                            node,
                            condition,
                            parens_range,
                            needs_parens,
                            &delete_ranges,
                            true,
                            false,
                            None,
                            None,
                        ),
                    }
                }
                (Some(then_lit), None) => {
                    let negated = is_negated_bool(ast, then_lit);
                    let delete_ranges = vec![c.range().end_start(condition, else_exp)];
                    let op = if negated {
                        Some(TokenType::AMPERSAND_AMPERSAND)
                    } else {
                        Some(TokenType::BAR_BAR)
                    };
                    let needs_parens = bool_expr_needs_parens(ast, condition, op);
                    let parens_range = c.range().node(condition);
                    let needs_parens_else = bool_expr_needs_parens(ast, else_exp, op);
                    add_bool_expr_edit(
                        c,
                        builder,
                        node,
                        condition,
                        parens_range,
                        needs_parens,
                        &delete_ranges,
                        negated,
                        true,
                        Some(if negated { " && " } else { " || " }),
                        needs_parens_else.then(|| c.range().node(else_exp)),
                    );
                }
                (None, Some(else_lit)) => {
                    let negated = is_negated_bool(ast, else_lit);
                    let delete_ranges = vec![
                        c.range().end_start(condition, then_exp),
                        c.range().end_end(then_exp, else_exp),
                    ];
                    let op = if negated {
                        Some(TokenType::AMPERSAND_AMPERSAND)
                    } else {
                        Some(TokenType::BAR_BAR)
                    };
                    let needs_parens = bool_expr_needs_parens(ast, condition, op);
                    let parens_range = c.range().node(condition);
                    let needs_parens_then = bool_expr_needs_parens(ast, then_exp, op);
                    add_bool_expr_edit(
                        c,
                        builder,
                        node,
                        condition,
                        parens_range,
                        needs_parens,
                        &delete_ranges,
                        !negated,
                        true,
                        Some(if negated { " && " } else { " || " }),
                        needs_parens_then.then(|| c.range().node(then_exp)),
                    );
                }
                (None, None) => {}
            }
            return;
        }

        if let Some(pref) = ast.cast::<PrefixExpression>(node) {
            if ast.tokens.ty(ast[pref].operator) != TokenType::BANG {
                return;
            }
            let operand = ast[pref].operand.raw();
            let op_unp = unparenthesized(ast, operand);
            let Some(lit) = ast.cast::<BooleanLiteral>(op_unp) else {
                return;
            };
            let negated = is_negated_bool(ast, lit);
            let delete_ranges = vec![
                c.range()
                    .token_start_start(ast[pref].operator, ast.begin_token(operand)),
            ];
            add_bool_expr_edit(
                c,
                builder,
                node,
                operand,
                c.range().node(pref),
                false,
                &delete_ranges,
                !negated,
                false,
                None,
                None,
            );
        }
    }
);

// ---------------------------------------------------------------------------
// 16. InlineTypedef
// ---------------------------------------------------------------------------

producer!(
    InlineTypedef,
    k::INLINE_TYPEDEF,
    Some(&k::INLINE_TYPEDEF_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let node = c.node;
        let (name_tok, return_type, type_params, parameters) =
            if let Some(fta) = ast.cast::<FunctionTypeAlias>(node) {
                (
                    ast[fta].name,
                    ast[fta].return_type,
                    ast[fta].type_parameters,
                    ast[fta].parameters,
                )
            } else if let Some(gta) = ast.cast::<GenericTypeAlias>(node) {
                if ast[gta].type_parameters.is_some() {
                    return;
                }
                let Some(gft) = ast.cast::<GenericFunctionType>(ast[gta].type_) else {
                    return;
                };
                (
                    ast[gta].name,
                    ast[gft].return_type,
                    ast[gft].type_parameters,
                    ast[gft].parameters,
                )
            } else {
                return;
            };
        let params_list = ast.list_raw(ast[parameters].parameters);
        let td_name = c.lexeme(name_tok);
        let td_elem = c.locator().element(node);
        fn find_typedef_ref(
            c: &ProducerContext<'_>,
            n: NodeId,
            td_name: &str,
            td_elem: Option<ElementId>,
            out: &mut Vec<Id<NamedType>>,
        ) {
            let ast = c.ast;
            if let Some(nt) = ast.cast::<NamedType>(n) {
                if c.lexeme(ast[nt].name) == td_name {
                    if td_elem.is_none() || c.locator().element(nt) == td_elem {
                        out.push(nt);
                    }
                }
            }
            for child in ast.children(n) {
                find_typedef_ref(c, child, td_name, td_elem, out);
            }
        }
        let mut refs = Vec::new();
        find_typedef_ref(c, c.unit.raw(), td_name, td_elem, &mut refs);
        if refs.len() != 1 {
            return;
        }
        let Some(nt) = refs.pop() else {
            return;
        };
        let full_del = c.utils.get_lines_range(c.range().node(node), false);
        let r_nt = c.range().node(nt);
        let ret_str = return_type
            .map(|rt| format!("{} ", c.utils.get_node_text(rt)))
            .unwrap_or_default();
        let tp_str = type_params
            .map(|tp| c.utils.get_node_text(tp))
            .unwrap_or_default();
        let mut param_strs = Vec::new();
        for &p in params_list {
            if let Some(rfp) = ast.cast::<RegularFormalParameter>(p) {
                if let Some(fts) = ast[rfp].function_typed_suffix {
                    let p_ret = ast[rfp]
                        .type_
                        .map(|t| format!("{} ", c.utils.get_node_text(t)))
                        .unwrap_or_default();
                    let p_tp = ast[fts]
                        .type_parameters
                        .map(|t| c.utils.get_node_text(t))
                        .unwrap_or_default();
                    let p_fp = c.utils.get_node_text(ast[fts].formal_parameters);
                    let p_q = ast[fts].question.map(|_| "?").unwrap_or_default();
                    let p_name = ast[rfp]
                        .name
                        .map(|n| format!(" {}", c.lexeme(n)))
                        .unwrap_or_default();
                    param_strs.push(format!("{p_ret}Function{p_tp}{p_fp}{p_q}{p_name}"));
                } else {
                    let meta = ast
                        .list_raw(ast[rfp].metadata)
                        .iter()
                        .map(|&m| format!("{} ", c.utils.get_node_text(m)))
                        .collect::<String>();
                    let req = ast[rfp]
                        .required_keyword
                        .map(|_| "required ")
                        .unwrap_or_default();
                    let cov = ast[rfp]
                        .covariant_keyword
                        .map(|_| "covariant ")
                        .unwrap_or_default();
                    if let Some(t) = ast[rfp].type_ {
                        let ty = c.utils.get_node_text(t);
                        if ast[rfp].kind.is_named() {
                            let name_s = ast[rfp]
                                .name
                                .map(|n| format!(" {}", c.lexeme(n)))
                                .unwrap_or_default();
                            param_strs.push(format!("{meta}{req}{cov}{ty}{name_s}"));
                        } else {
                            param_strs.push(format!("{meta}{req}{cov}{ty}"));
                        }
                    } else if ast[rfp].kind.is_named() {
                        let name_s = ast[rfp]
                            .name
                            .map(|n| format!(" {}", c.lexeme(n)))
                            .unwrap_or_default();
                        param_strs.push(format!("{meta}{req}{cov}dynamic{name_s}"));
                    } else {
                        let name_s = ast[rfp].name.map(|n| c.lexeme(n)).unwrap_or("dynamic");
                        param_strs.push(format!("{meta}{req}{cov}{name_s}"));
                    }
                }
            } else {
                param_strs.push(c.utils.get_node_text(p));
            }
        }
        let group_open = ast[parameters]
            .left_delimiter
            .map(|t| c.lexeme(t))
            .unwrap_or("");
        let group_close = ast[parameters]
            .right_delimiter
            .map(|t| c.lexeme(t))
            .unwrap_or("");
        let mut params_str = String::from("(");
        if group_open.is_empty() {
            params_str.push_str(&param_strs.join(", "));
        } else {
            let mut pos_parts = Vec::new();
            let mut grp_parts = Vec::new();
            for (i, &p) in params_list.iter().enumerate() {
                let is_opt = if let Some(rfp) = ast.cast::<RegularFormalParameter>(p) {
                    ast[rfp].kind.is_optional()
                } else {
                    false
                };
                if is_opt {
                    grp_parts.push(param_strs[i].as_str());
                } else {
                    pos_parts.push(param_strs[i].as_str());
                }
            }
            if !pos_parts.is_empty() {
                params_str.push_str(&pos_parts.join(", "));
                params_str.push_str(", ");
            }
            params_str.push_str(group_open);
            params_str.push_str(&grp_parts.join(", "));
            params_str.push_str(group_close);
        }
        params_str.push(')');
        let rep = format!("{ret_str}Function{tp_str}{params_str}");
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(full_del.offset, full_del.length);
            b.add_simple_replacement(r_nt.offset, r_nt.length, &rep);
        });
    }
);

// ---------------------------------------------------------------------------
// 17. RemoveInvocation
// ---------------------------------------------------------------------------

producer!(
    RemoveInvocation,
    k::REMOVE_INVOCATION,
    Some(&k::REMOVE_INVOCATION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        let Some(mi) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            return;
        };
        let is_cascaded = ast[mi].operator.is_some_and(|op| {
            matches!(
                ast.tokens.ty(op),
                TokenType::PERIOD_PERIOD | TokenType::QUESTION_PERIOD_PERIOD
            )
        });
        let del = if is_cascaded {
            c.range().node(mi)
        } else if let Some(target) = ast[mi].target {
            c.range().end_end(target, mi)
        } else {
            return;
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 18. RemoveBreak
// ---------------------------------------------------------------------------

producer!(
    RemoveBreak,
    k::REMOVE_BREAK,
    Some(&k::REMOVE_BREAK_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(brk) = ast.cast::<BreakStatement>(c.node) else {
            return;
        };
        let del = c.utils.get_lines_range(c.range().node(brk), false);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 19. ConvertToIfNull
// ---------------------------------------------------------------------------

fn outer_parentheses_needed_for_if_null(ast: &Ast, node: NodeId) -> bool {
    ast.parent(node).is_some_and(|p| {
        ast.is::<Expression>(p)
            && !ast.is::<ParenthesizedExpression>(p)
            && expr_precedence(ast, p) > ExprPrecedence::IfNull
    })
}

pub struct ConvertToIfNull {
    pub prefer_if_null: bool,
}

impl CorrectionProducer for ConvertToIfNull {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CONVERT_TO_IF_NULL)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CONVERT_TO_IF_NULL_MULTI)
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        if self.prefer_if_null {
            let Some(cond) = ast.this_or_ancestor_of_type::<ConditionalExpression>(c.node) else {
                return;
            };
            if ast.offset(cond) != c.selection_offset || ast.length(cond) != c.selection_length {
                return;
            }
            let Some(condition) = ast.cast::<BinaryExpression>(ast[cond].condition) else {
                return;
            };
            let op_ty = ast.tokens.ty(ast[condition].operator);
            let left = ast[condition].left_operand.raw();
            let right = ast[condition].right_operand.raw();
            let (nullable_expr, default_expr) = if ast.is::<NullLiteral>(right) {
                if op_ty == TokenType::EQ_EQ {
                    (
                        ast[cond].else_expression.raw(),
                        ast[cond].then_expression.raw(),
                    )
                } else {
                    (
                        ast[cond].then_expression.raw(),
                        ast[cond].else_expression.raw(),
                    )
                }
            } else if ast.is::<NullLiteral>(left) {
                if op_ty == TokenType::EQ_EQ {
                    (
                        ast[cond].else_expression.raw(),
                        ast[cond].then_expression.raw(),
                    )
                } else {
                    (
                        ast[cond].then_expression.raw(),
                        ast[cond].else_expression.raw(),
                    )
                }
            } else {
                return;
            };
            if ast.is::<NullLiteral>(default_expr) {
                return;
            }
            let inner_parens = expr_precedence(ast, default_expr) < ExprPrecedence::IfNull;
            let outer_parens = outer_parentheses_needed_for_if_null(ast, cond.raw());
            let null_text = c.utils.get_node_text(nullable_expr);
            let def_text = c.utils.get_node_text(default_expr);
            let mut rep = String::new();
            if outer_parens {
                rep.push('(');
            }
            rep.push_str(&null_text);
            rep.push_str(" ?? ");
            if inner_parens {
                rep.push('(');
            }
            rep.push_str(&def_text);
            if inner_parens {
                rep.push(')');
            }
            if outer_parens {
                rep.push(')');
            }
            let r = c.range().node(cond);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r.offset, r.length, &rep);
            });
        } else {
            let Some(bin) = ast.cast::<BinaryExpression>(c.node) else {
                return;
            };
            if ast.offset(bin) != c.selection_offset || ast.length(bin) != c.selection_length {
                return;
            }
            let op_ty = ast.tokens.ty(ast[bin].operator);
            if op_ty != TokenType::EQ_EQ && op_ty != TokenType::BANG_EQ {
                return;
            }
            let left = ast[bin].left_operand.raw();
            let right = ast[bin].right_operand.raw();
            let nullable_expr = if !ast.is::<BooleanLiteral>(left) {
                left
            } else if !ast.is::<BooleanLiteral>(right) {
                right
            } else {
                return;
            };
            let outer_parens = outer_parentheses_needed_for_if_null(ast, bin.raw());
            let rhs = if op_ty == TokenType::EQ_EQ {
                "false"
            } else {
                "true"
            };
            let null_text = c.utils.get_node_text(nullable_expr);
            let rep = if outer_parens {
                format!("({null_text} ?? {rhs})")
            } else {
                format!("{null_text} ?? {rhs}")
            };
            let r = c.range().node(bin);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r.offset, r.length, &rep);
            });
        }
    }
}

// ---------------------------------------------------------------------------
// 20. ConvertForEachToForLoop
// ---------------------------------------------------------------------------

producer!(
    ConvertForEachToForLoop,
    k::CONVERT_FOR_EACH_TO_FOR_LOOP,
    Some(&k::CONVERT_FOR_EACH_TO_FOR_LOOP_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(mi) = ast.this_or_ancestor_of_type::<MethodInvocation>(c.node) else {
            return;
        };
        let Some(stmt) = ast
            .parent(mi)
            .and_then(|p| ast.cast::<ExpressionStatement>(p))
        else {
            return;
        };
        let args = ast.list_raw(ast[ast[mi].argument_list].arguments);
        if args.len() != 1 {
            return;
        }
        let Some(fe) = ast.cast::<FunctionExpression>(args[0]) else {
            return;
        };
        let Some(target) = ast[mi].target else {
            return;
        };
        let Some(param_list) = ast[fe].parameters else {
            return;
        };
        let params = ast.list_raw(ast[param_list].parameters);
        if params.len() != 1 {
            return;
        }
        let Some(rfp) = ast.cast::<RegularFormalParameter>(params[0]) else {
            return;
        };
        if ast[rfp].function_typed_suffix.is_some() {
            return;
        }
        let Some(loop_var_name) = ast[rfp].name else {
            return;
        };
        let var_name = c.lexeme(loop_var_name).to_string();
        let explicit_type = ast[rfp].type_;
        let specify_types = c.code_style().specify_types();
        let prefer_final = c.code_style().is_lint_enabled("prefer_final_in_for_each");
        let param_ty = if specify_types && explicit_type.is_none() {
            c.locator()
                .element(rfp)
                .and_then(|e| e.cast::<FormalParameterElement>())
                .and_then(|fp| c.ctx.get(fp).type_.get())
                .filter(|&ty| !matches!(c.ctx.ty(ty), TypeKind::Invalid))
        } else {
            None
        };

        let body = ast[fe].body;
        if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
            if ast[bfb].keyword.is_some() {
                return;
            }
            let r_prefix = c.range().start_start(stmt, ast[bfb].block);
            let r_end = c.range().end_end(bfb, stmt);
            let target_src = c.utils.get_node_text(target);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_replacement(r_prefix.offset, r_prefix.length, |e| {
                    e.write("for (");
                    if prefer_final {
                        e.write("final ");
                    }
                    if let Some(et) = explicit_type {
                        if !prefer_final && c.lexeme(ast.begin_token(rfp)) == "final" {
                            e.write("final ");
                        }
                        e.write(&c.utils.get_node_text(et));
                        e.write(" ");
                    } else if let Some(ty) = param_ty {
                        if !e.write_type(Some(ty), &WriteType::default()) && !prefer_final {
                            e.write("var ");
                        } else {
                            e.write(" ");
                        }
                    } else if !prefer_final {
                        e.write("var ");
                    }
                    e.write(&var_name);
                    e.write(" in ");
                    e.write(&target_src);
                    e.write(") ");
                });
                b.add_deletion(r_end.offset, r_end.length);
            });
        } else if let Some(efb) = ast.cast::<ExpressionFunctionBody>(body) {
            if ast[efb].keyword.is_some() {
                return;
            }
            let expr = ast[efb].expression;
            if ast.is::<SetOrMapLiteral>(expr) || ast.is::<CascadeExpression>(expr) {
                return;
            }
            let prefix = c.utils.get_node_prefix(stmt.raw());
            let indent = c.utils.one_indent().to_string();
            let eol = c.eol().to_string();
            let r_prefix = c.range().start_start(stmt, expr);
            let r_end = c.range().end_end(expr, stmt);
            let target_src = c.utils.get_node_text(target);
            let end_rep = format!(";{eol}{prefix}}}");
            builder.add_dart_file_edit(c.path, |b| {
                b.add_replacement(r_prefix.offset, r_prefix.length, |e| {
                    e.write("for (");
                    if prefer_final {
                        e.write("final ");
                    }
                    if let Some(et) = explicit_type {
                        if !prefer_final && c.lexeme(ast.begin_token(rfp)) == "final" {
                            e.write("final ");
                        }
                        e.write(&c.utils.get_node_text(et));
                        e.write(" ");
                    } else if let Some(ty) = param_ty {
                        if !e.write_type(Some(ty), &WriteType::default()) && !prefer_final {
                            e.write("var ");
                        } else {
                            e.write(" ");
                        }
                    } else if !prefer_final {
                        e.write("var ");
                    }
                    e.write(&var_name);
                    e.write(" in ");
                    e.write(&target_src);
                    e.write(") {");
                    e.write(&eol);
                    e.write(&prefix);
                    e.write(&indent);
                });
                b.add_simple_replacement(r_end.offset, r_end.length, &end_rep);
            });
        }
    }
);

// ---------------------------------------------------------------------------
// 21. RemoveConst & RemoveUnnecessaryConst
// ---------------------------------------------------------------------------

pub struct RemoveConstProducer {
    pub unnecessary: bool,
}

impl CorrectionProducer for RemoveConstProducer {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        if self.unnecessary {
            Some(&k::REMOVE_UNNECESSARY_CONST)
        } else {
            Some(&k::REMOVE_CONST)
        }
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        if self.unnecessary {
            Some(&k::REMOVE_UNNECESSARY_CONST_MULTI)
        } else {
            None
        }
    }

    fn applicability(&self) -> Applicability {
        if self.unnecessary {
            Applicability::Automatically
        } else {
            Applicability::SingleLocation
        }
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        let const_token = if let Some(cd) = ast.cast::<ClassDeclaration>(node) {
            let Some(nh) = ast.cast::<PrimaryConstructorDeclaration>(ast[cd].name_part) else {
                return;
            };
            ast[nh].const_keyword
        } else if let Some(cd) = ast.cast::<ConstructorDeclaration>(node) {
            ast[cd].const_keyword
        } else if let Some(ed) = ast.cast::<EnumDeclaration>(node) {
            let Some(nh) = ast.cast::<PrimaryConstructorDeclaration>(ast[ed].name_part) else {
                return;
            };
            ast[nh].const_keyword
        } else if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(node) {
            ast[pcd].const_keyword
        } else if let Some(ice) = ast.cast::<InstanceCreationExpression>(node) {
            ast[ice].keyword
        } else if let Some(ll) = ast.cast::<ListLiteral>(node) {
            ast[ll].const_keyword
        } else if let Some(sml) = ast.cast::<SetOrMapLiteral>(node) {
            ast[sml].const_keyword
        } else if let Some(rl) = ast.cast::<RecordLiteral>(node) {
            ast[rl].const_keyword
        } else if let Some(cp) = ast.cast::<ConstantPattern>(node) {
            ast[cp].const_keyword
        } else if let Some(vdl) = ast.cast::<VariableDeclarationList>(node) {
            let Some(kw) = ast[vdl].keyword else {
                return;
            };
            let Some(decl) = ast.parent(vdl) else {
                return;
            };
            let is_top_or_static = ast.is::<TopLevelVariableDeclaration>(decl)
                || ast
                    .cast::<FieldDeclaration>(decl)
                    .is_some_and(|fd| ast[fd].static_keyword.is_some());
            if is_top_or_static {
                let r = c.range().token(kw);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, "final");
                });
            } else if ast[vdl].type_.is_some() {
                let del = c.range().token_start_start(kw, ast.tokens.next(kw));
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_deletion(del.offset, del.length);
                });
            } else {
                let r = c.range().token(kw);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, "var");
                });
            }
            return;
        } else {
            None
        };
        let Some(tok) = const_token else {
            return;
        };
        let del = c.range().token_start_start(tok, ast.tokens.next(tok));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
}

// ---------------------------------------------------------------------------
// 22. AddLeadingNewlineToString
// ---------------------------------------------------------------------------

producer!(
    AddLeadingNewlineToString,
    k::ADD_LEADING_NEWLINE_TO_STRING,
    Some(&k::ADD_LEADING_NEWLINE_TO_STRING_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(lit) = ast.cast::<SimpleStringLiteral>(c.node) else {
            return;
        };
        let tok = ast[lit].literal;
        let lex = c.lexeme(tok);
        let is_raw = lex.starts_with('r') || lex.starts_with('R');
        let rest = if is_raw { &lex[1..] } else { lex };
        if !rest.starts_with("\"\"\"") && !rest.starts_with("'''") {
            return;
        }
        let off = c.token_offset(tok) + if is_raw { 4 } else { 3 };
        let eol = c.eol().to_string();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(off, &eol);
        });
    }
);

// ---------------------------------------------------------------------------
// 23. ConvertToFunctionDeclaration
// ---------------------------------------------------------------------------

fn is_param_explicitly_typed(ast: &Ast, param: NodeId) -> bool {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp].type_.is_some() || ast[rfp].function_typed_suffix.is_some()
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        ast[ffp].type_.is_some() || ast[ffp].function_typed_suffix.is_some()
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        ast[sfp].type_.is_some() || ast[sfp].function_typed_suffix.is_some()
    } else {
        true
    }
}

fn body_has_returns(ast: &Ast, n: NodeId) -> bool {
    if ast.is::<ExpressionFunctionBody>(n) || ast.is::<ReturnStatement>(n) {
        return true;
    }
    if ast.is::<FunctionExpression>(n) {
        return false;
    }
    for child in ast.children(n) {
        if body_has_returns(ast, child) {
            return true;
        }
    }
    false
}

producer!(
    ConvertToFunctionDeclaration,
    k::CONVERT_TO_FUNCTION_DECLARATION,
    Some(&k::CONVERT_TO_FUNCTION_DECLARATION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(v) = ast.cast::<VariableDeclaration>(c.node) else {
            return;
        };
        let Some(eq_tok) = ast[v].equals else {
            return;
        };
        let Some(init) = ast[v].initializer else {
            return;
        };
        let Some(var_list) = ast
            .parent(v)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
        else {
            return;
        };
        let keyword = ast[var_list].keyword;
        let type_ = ast[var_list].type_;
        let vars = ast.list(ast[var_list].variables);
        let Some(grand_parent) = ast.parent(var_list) else {
            return;
        };
        let grand_parent_semi =
            if let Some(vds) = ast.cast::<VariableDeclarationStatement>(grand_parent) {
                ast[vds].semicolon
            } else if let Some(fd) = ast.cast::<FieldDeclaration>(grand_parent) {
                ast[fd].semicolon
            } else if let Some(tlvd) = ast.cast::<TopLevelVariableDeclaration>(grand_parent) {
                ast[tlvd].semicolon
            } else {
                return;
            };
        let Some(pos) = vars.iter().position(|&var| var == v) else {
            return;
        };
        let prev = if pos > 0 { Some(vars[pos - 1]) } else { None };
        let next = if pos + 1 < vars.len() {
            Some(vars[pos + 1])
        } else {
            None
        };
        let eol = c.eol().to_string();

        let mut return_type: Option<TypeId> = None;
        let mut param_types: Option<Vec<Option<TypeId>>> = None;
        if let Some(ty) = type_ {
            if ast.is::<NamedType>(ty) {
                if let Some(&tid) = c.tables.annotation_type.get(ty.raw()) {
                    if let TypeKind::Function(ref f) = *c.ctx.ty(tid) {
                        return_type = Some(f.ret);
                        param_types =
                            Some(c.ctx.list(f.params).iter().map(|p| Some(p.ty)).collect());
                    }
                }
            } else if let Some(gft) = ast.cast::<GenericFunctionType>(ty) {
                return_type = ast[gft]
                    .return_type
                    .and_then(|rt| c.tables.annotation_type.get(rt.raw()))
                    .copied();
                param_types = Some(
                    ast.list_raw(ast[ast[gft].parameters].parameters)
                        .iter()
                        .map(|&p| {
                            c.locator()
                                .locate(p)
                                .and_then(|e| e.cast::<FormalParameterElement>())
                                .and_then(|fp| c.ctx.get(fp).type_.get())
                        })
                        .collect(),
                );
            }
        } else if let Some(fe) = ast.cast::<FunctionExpression>(init) {
            if let Some(ft) = static_type(c, fe.raw()) {
                if let TypeKind::Function(ref f) = *c.ctx.ty(ft) {
                    let mut rt = f.ret;
                    if !body_has_returns(ast, ast[fe].body.raw()) {
                        if rt == c.ctx.tp.null_type() {
                            rt = c.ctx.tp.void_type();
                        } else if rt == c.ctx.tp.future_null_type() {
                            rt = c.ctx.tp.future_type(&c.ctx, c.ctx.tp.void_type());
                        }
                    }
                    return_type = Some(rt);
                }
            }
        }

        builder.add_dart_file_edit(c.path, |b| {
            if prev.is_none() {
                if let Some(kw) = keyword {
                    let del = c.range().token_start_start(kw, ast.tokens.next(kw));
                    b.add_deletion(del.offset, del.length);
                }
                if let Some(ty) = type_ {
                    let del = c
                        .range()
                        .token_start_start(ast.begin_token(ty), ast.tokens.next(ast.end_token(ty)));
                    b.add_deletion(del.offset, del.length);
                }
            } else if let Some(p) = prev {
                if !ast[p]
                    .initializer
                    .is_some_and(|i| ast.is::<FunctionExpression>(i))
                {
                    let p_end = ast.end_token(p);
                    let r = c
                        .range()
                        .token_end_start(p_end, ast.tokens.next(ast.tokens.next(p_end)));
                    let line_prefix = c.utils.get_line_prefix(r.offset).to_string();
                    b.add_simple_replacement(r.offset, r.length, &format!(";{eol}{line_prefix}"));
                }
            }

            if let Some(rt) = return_type {
                if b.can_write_type_at(ast.offset(v), rt) {
                    b.add_insertion(ast.offset(v), |e| {
                        e.write_type(Some(rt), &WriteType::default());
                        e.write(" ");
                    });
                }
            }

            let del_eq = c
                .range()
                .token_end_start(ast.tokens.previous(eq_tok), ast.tokens.next(eq_tok));
            b.add_deletion(del_eq.offset, del_eq.length);

            if let Some(ref static_params) = param_types {
                if let Some(fe) = ast.cast::<FunctionExpression>(init) {
                    if let Some(fpl) = ast[fe].parameters {
                        for (idx, &param) in ast.list_raw(ast[fpl].parameters).iter().enumerate() {
                            if is_param_explicitly_typed(ast, param) {
                                continue;
                            }
                            if let Some(spt) = static_params.get(idx).copied().flatten() {
                                if b.can_write_type_at(ast.offset(param), spt) {
                                    b.add_insertion(ast.offset(param), |e| {
                                        e.write_type(Some(spt), &WriteType::default());
                                        e.write(" ");
                                    });
                                }
                            }
                        }
                    }
                }
            }

            if let Some(n) = next {
                let v_end = ast.end_token(v);
                let r = c
                    .range()
                    .token_end_start(v_end, ast.tokens.next(ast.tokens.next(v_end)));
                let line_prefix = c.utils.get_line_prefix(r.offset).to_string();
                if ast[n]
                    .initializer
                    .is_some_and(|i| ast.is::<FunctionExpression>(i))
                {
                    b.add_simple_replacement(r.offset, r.length, &format!("{eol}{line_prefix}"));
                } else {
                    let mut after = String::new();
                    if let Some(kw) = keyword {
                        after.push_str(c.lexeme(kw));
                        after.push(' ');
                    }
                    if let Some(ty) = type_ {
                        after.push_str(&c.utils.get_node_text(ty));
                        after.push(' ');
                    }
                    b.add_simple_replacement(
                        r.offset,
                        r.length,
                        &format!("{eol}{line_prefix}{after}"),
                    );
                }
            } else if let Some(fe) = ast.cast::<FunctionExpression>(init) {
                if ast.is::<BlockFunctionBody>(ast[fe].body) {
                    let r = c.range().token(grand_parent_semi);
                    b.add_deletion(r.offset, r.length);
                }
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 24. ReplaceWithInterpolation
// ---------------------------------------------------------------------------

struct InterpStyle {
    single_quotes: bool,
    multiline: bool,
    raw: bool,
}

fn string_interp_style(c: &ProducerContext<'_>, lit: NodeId) -> Option<InterpStyle> {
    let ast = c.ast;
    let tok = if let Some(ssl) = ast.cast::<SimpleStringLiteral>(lit) {
        ast[ssl].literal
    } else if let Some(si) = ast.cast::<StringInterpolation>(lit) {
        let first = *ast.list_raw(ast[si].elements).first()?;
        let ie = ast.cast::<InterpolationString>(first)?;
        ast[ie].contents
    } else {
        return None;
    };
    let lex = c.lexeme(tok);
    let raw = lex.starts_with('r') || lex.starts_with('R');
    let rest = if raw { &lex[1..] } else { lex };
    let single_quotes = rest.starts_with('\'');
    let multiline = rest.starts_with("\"\"\"") || rest.starts_with("'''");
    Some(InterpStyle {
        single_quotes,
        multiline,
        raw,
    })
}

fn validate_string_combination(
    c: &ProducerContext<'_>,
    node: Id<BinaryExpression>,
    components: &mut Vec<NodeId>,
) -> Option<InterpStyle> {
    let ast = c.ast;
    if ast.tokens.ty(ast[node].operator) != TokenType::PLUS {
        return None;
    }
    let left = ast[node].left_operand.raw();
    let right = ast[node].right_operand.raw();
    let style = if ast.is::<SimpleStringLiteral>(left) || ast.is::<StringInterpolation>(left) {
        let s = string_interp_style(c, left)?;
        if s.raw {
            return None;
        }
        components.push(left);
        s
    } else if let Some(left_bin) = ast.cast::<BinaryExpression>(left) {
        validate_string_combination(c, left_bin, components)?
    } else if ast.is::<SimpleIdentifier>(left) {
        components.push(left);
        let s = string_interp_style(c, right)?;
        if s.raw {
            return None;
        }
        components.push(right);
        return Some(s);
    } else {
        return None;
    };

    if ast.is::<SimpleStringLiteral>(right) || ast.is::<StringInterpolation>(right) {
        let rs = string_interp_style(c, right)?;
        if rs.raw || rs.single_quotes != style.single_quotes || rs.multiline != style.multiline {
            return None;
        }
    } else if ast.is::<AdjacentStrings>(right) {
        return None;
    }
    components.push(right);
    Some(style)
}

fn string_literal_contents(c: &ProducerContext<'_>, node: NodeId, style: &InterpStyle) -> String {
    let quote_len: u32 = if style.multiline { 3 } else { 1 };
    let start = c.ast.offset(node) + quote_len;
    let end = c.ast.end(node) - quote_len;
    c.utils.get_range_text(Range {
        offset: start,
        length: end - start,
    })
}

producer!(
    ReplaceWithInterpolation,
    k::REPLACE_WITH_INTERPOLATION,
    Some(&k::REPLACE_WITH_INTERPOLATION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(mut binary) = ast.cast::<BinaryExpression>(c.node) else {
            return;
        };
        while let Some(p) = ast
            .parent(binary)
            .and_then(|p| ast.cast::<BinaryExpression>(p))
        {
            binary = p;
        }
        let mut components = Vec::new();
        let Some(style) = validate_string_combination(c, binary, &mut components) else {
            return;
        };
        let q = match (style.single_quotes, style.multiline) {
            (false, false) => "\"",
            (false, true) => "\"\"\"",
            (true, false) => "'",
            (true, true) => "'''",
        };
        let mut out = String::from(q);
        for (i, &comp) in components.iter().enumerate() {
            if ast.is::<SimpleStringLiteral>(comp) || ast.is::<StringInterpolation>(comp) {
                let contents = string_literal_contents(c, comp, &style);
                if contents.ends_with('\\') {
                    return;
                }
                out.push_str(&contents);
            } else if let Some(id) = ast.cast::<SimpleIdentifier>(comp) {
                let next_starts_ident = components.get(i + 1).is_some_and(|&nxt| {
                    if ast.is::<SimpleStringLiteral>(nxt) || ast.is::<StringInterpolation>(nxt) {
                        let s = string_literal_contents(c, nxt, &style);
                        s.chars()
                            .next()
                            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
                    } else {
                        false
                    }
                });
                let lex = c.lexeme(ast[id].token);
                if next_starts_ident || lex.contains('$') {
                    out.push_str("${");
                    out.push_str(lex);
                    out.push('}');
                } else {
                    out.push('$');
                    out.push_str(lex);
                }
            } else {
                let mut src = c.utils.get_node_text(comp);
                if let Some(pe) = ast.cast::<ParenthesizedExpression>(comp) {
                    src = c.utils.get_node_text(ast[pe].expression);
                }
                out.push_str("${");
                out.push_str(&src);
                out.push('}');
            }
        }
        out.push_str(q);
        let r = c.range().node(binary);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, &out);
        });
    }
);

// ---------------------------------------------------------------------------
// 25. RemoveComparison
// ---------------------------------------------------------------------------

pub struct RemoveComparison {
    pub is_type_check: bool,
}

fn comparison_evaluates_color(c: &ProducerContext<'_>, node: NodeId) -> (bool, bool) {
    let camel = c.diagnostic.map(|d| d.code.camel_case_name);
    let is_true = matches!(
        camel,
        Some(
            "unnecessaryNullComparisonAlwaysNullTrue"
                | "unnecessaryNullComparisonNeverNullTrue"
                | "unnecessaryNanComparisonTrue"
                | "unnecessaryTypeCheckTrue"
        )
    ) || matches!(
        c.diagnostic.map(|d| d.code.name),
        Some("unnecessary_type_check_true")
    );
    let is_false = matches!(
        camel,
        Some(
            "unnecessaryNullComparisonAlwaysNullFalse"
                | "unnecessaryNullComparisonNeverNullFalse"
                | "unnecessaryNanComparisonFalse"
                | "unnecessaryTypeCheckFalse"
                | "avoidNullChecksInEqualityOperators"
        )
    ) || matches!(
        c.diagnostic.map(|d| d.code.name),
        Some("unnecessary_type_check_false" | "avoid_null_checks_in_equality_operators")
    );
    if is_true || is_false {
        return (is_true, is_false);
    }
    let ast = c.ast;
    if let Some(bin) = ast.cast::<BinaryExpression>(node) {
        let op = ast.tokens.ty(ast[bin].operator);
        let left_is_null = ast.is::<NullLiteral>(ast[bin].left_operand);
        let right_is_null = ast.is::<NullLiteral>(ast[bin].right_operand);
        let left_nullable = static_type(c, ast[bin].left_operand.raw())
            .is_some_and(|t| c.ctx.nullability_suffix(t) != Nullability::None);
        let right_nullable = static_type(c, ast[bin].right_operand.raw())
            .is_some_and(|t| c.ctx.nullability_suffix(t) != Nullability::None);
        let t = match op {
            TokenType::EQ_EQ => {
                (left_is_null && left_nullable) || (right_is_null && right_nullable)
            }
            TokenType::BANG_EQ => {
                (left_is_null && !right_nullable) || (right_is_null && !left_nullable)
            }
            _ => false,
        };
        return (t, !t);
    }
    (false, false)
}

impl CorrectionProducer for RemoveComparison {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        if self.is_type_check {
            Some(&k::REMOVE_TYPE_CHECK)
        } else {
            Some(&k::REMOVE_COMPARISON)
        }
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        if self.is_type_check {
            Some(&k::REMOVE_TYPE_CHECK_MULTI)
        } else {
            Some(&k::REMOVE_COMPARISON_MULTI)
        }
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        if !ast.is::<BinaryExpression>(node) && !ast.is::<IsExpression>(node) {
            return;
        }
        let Some(parent) = ast.parent(node) else {
            return;
        };
        let (is_true, is_false) = comparison_evaluates_color(c, node);

        if let Some(ai) = ast.cast::<AssertInitializer>(parent) {
            if !is_true {
                return;
            }
            let Some(ctor) = ast
                .parent(ai)
                .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
            else {
                return;
            };
            let inits = ast.list_raw(ast[ctor].initializers);
            let del = if inits.len() == 1 {
                c.range().end_end(ast[ctor].parameters, ai)
            } else {
                c.range().node_in_list(inits, ai.raw())
            };
            builder.add_dart_file_edit(c.path, |b| {
                b.add_deletion(del.offset, del.length);
            });
        } else if let Some(as_) = ast.cast::<AssertStatement>(parent) {
            if !is_true {
                return;
            }
            let del = c.utils.get_lines_range(c.range().node(as_), false);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_deletion(del.offset, del.length);
            });
        } else if let Some(p_bin) = ast.cast::<BinaryExpression>(parent) {
            let p_op = ast.tokens.ty(ast[p_bin].operator);
            if (p_op == TokenType::AMPERSAND_AMPERSAND && is_true)
                || (p_op == TokenType::BAR_BAR && is_false)
            {
                let del = if ast[p_bin].left_operand.raw() == node {
                    c.range().start_start(node, ast[p_bin].right_operand)
                } else {
                    c.range().end_end(ast[p_bin].left_operand, node)
                };
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_deletion(del.offset, del.length);
                });
            }
        } else if let Some(ce) = ast.cast::<ConditionalExpression>(parent) {
            let keep_expr = if is_true {
                ast[ce].then_expression.raw()
            } else if is_false {
                ast[ce].else_expression.raw()
            } else {
                return;
            };
            let mut text = c.utils.get_node_text(keep_expr);
            let wrap = ast.parent(ce).is_some_and(|gp| {
                ast.is::<Expression>(gp)
                    && !ast.is::<ParenthesizedExpression>(gp)
                    && expr_precedence(ast, keep_expr) < expr_precedence(ast, gp)
            });
            if wrap {
                text = format!("({text})");
            }
            let r_ce = c.range().node(ce);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r_ce.offset, r_ce.length, &text);
            });
        } else if let Some(ifs) = ast.cast::<IfStatement>(parent) {
            if ast[ifs].else_statement.is_none() && is_true {
                let then_stmt = ast[ifs].then_statement;
                if let Some(blk) = ast.cast::<Block>(then_stmt) {
                    let stmts = ast.list_raw(ast[blk].statements);
                    if stmts.is_empty() {
                        let del = c.utils.get_lines_range(c.range().node(ifs), false);
                        builder.add_dart_file_edit(c.path, |b| {
                            b.add_deletion(del.offset, del.length);
                        });
                    } else {
                        let r_lines = c.utils.get_lines_range(
                            c.range()
                                .start_end(*stmts.first().unwrap(), *stmts.last().unwrap()),
                            false,
                        );
                        let text = c.utils.replace_source_indent(
                            &c.utils.get_range_text(r_lines),
                            &format!(
                                "{}{}",
                                c.utils.get_node_prefix(ifs.raw()),
                                c.utils.one_indent()
                            ),
                            &c.utils.get_node_prefix(ifs.raw()),
                            false,
                            false,
                        );
                        let r_ifs = c.utils.get_lines_range(c.range().node(ifs), false);
                        builder.add_dart_file_edit(c.path, |b| {
                            b.add_simple_replacement(r_ifs.offset, r_ifs.length, &text);
                        });
                    }
                } else {
                    let text = c.utils.get_node_text(then_stmt);
                    let r_ifs = c.range().node(ifs);
                    builder.add_dart_file_edit(c.path, |b| {
                        b.add_simple_replacement(r_ifs.offset, r_ifs.length, &text);
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 26. RemoveUnusedParameter
// ---------------------------------------------------------------------------

producer!(
    RemoveUnusedParameter,
    k::REMOVE_UNUSED_PARAMETER,
    Some(&k::REMOVE_UNUSED_PARAMETER_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let param = c.node;
        let Some(param_list) = ast
            .parent(param)
            .and_then(|p| ast.cast::<FormalParameterList>(p))
        else {
            return;
        };
        let params = ast.list_raw(ast[param_list].parameters);
        let Some(idx) = params.iter().position(|&p| p == param) else {
            return;
        };
        let del = if idx == 0 {
            if params.len() == 1 {
                if let (Some(ld), Some(rd)) = (
                    ast[param_list].left_delimiter,
                    ast[param_list].right_delimiter,
                ) {
                    c.range().token_start_end(ld, rd)
                } else {
                    c.range().node(param)
                }
            } else {
                let prev = ast.tokens.previous(ast.begin_token(param));
                let next = params[1];
                if let Some(ld) = ast[param_list].left_delimiter {
                    if prev == ld && ast.offset(next) > c.token_offset(ld) {
                        if let Some(rd) = ast[param_list].right_delimiter {
                            if ast.tokens.next(ast.end_token(param)) == rd {
                                c.range().token_start_end(ld, rd)
                            } else {
                                c.range().start_start(param, next)
                            }
                        } else {
                            c.range().start_start(param, next)
                        }
                    } else if ast.offset(next) > c.token_offset(ld)
                        && c.token_offset(ld) > ast.offset(param)
                    {
                        let mut preceding = ast.tokens.previous(ld);
                        if ast.tokens.ty(preceding) == TokenType::COMMA {
                            preceding = ast.tokens.previous(preceding);
                        }
                        c.range().token_start_end(ast.begin_token(param), preceding)
                    } else {
                        c.range().start_start(param, next)
                    }
                } else {
                    c.range().start_start(param, next)
                }
            }
        } else {
            let prev = ast.end_token(params[idx - 1]);
            let mut end_tok = ast.end_token(param);
            if idx == params.len() - 1 {
                if let Some(rd) = ast[param_list].right_delimiter {
                    if let Some(ld) = ast[param_list].left_delimiter {
                        if c.token_offset(ld) > c.token_offset(prev) {
                            end_tok = rd;
                        }
                    }
                }
                let next_tok = ast.tokens.next(end_tok);
                if ast.tokens.ty(next_tok) == TokenType::COMMA {
                    end_tok = next_tok;
                }
            }
            c.range().token_end_end(prev, end_tok)
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 27. ReplaceReturnTypeFuture
// ---------------------------------------------------------------------------

producer!(
    ReplaceReturnTypeFuture,
    k::REPLACE_RETURN_TYPE_FUTURE,
    Some(&k::REPLACE_RETURN_TYPE_FUTURE_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let ty = if let Some(fd) = ast.this_or_ancestor_of_type::<FunctionDeclaration>(c.node) {
            ast[fd].return_type
        } else if let Some(md) = ast.this_or_ancestor_of_type::<MethodDeclaration>(c.node) {
            ast[md].return_type
        } else {
            ast.this_or_ancestor_of_type::<TypeAnnotation>(c.node)
        };
        let Some(ty) = ty else {
            return;
        };
        builder.add_dart_file_edit(c.path, |b| {
            super::more::replace_type_with_future(c, b, Some(ty));
        });
    }
);

// ---------------------------------------------------------------------------
// 28. MoveDocCommentToLibraryDirective
// ---------------------------------------------------------------------------

producer!(
    MoveDocCommentToLibraryDirective,
    k::MOVE_DOC_COMMENT_TO_LIBRARY_DIRECTIVE,
    None,
    AutomaticallyButOncePerFile,
    |c, builder| {
        let ast = c.ast;
        let Some(comment) = ast.this_or_ancestor_of_type::<Comment>(c.node) else {
            return;
        };
        let eol = c.eol().to_string();
        if let Some(first_dir) = ast.list_raw(ast[c.unit].directives).first().copied() {
            if let Some(ld) = ast.cast::<LibraryDirective>(first_dir) {
                let first_tok = ast
                    .list_raw(ast[ld].metadata)
                    .first()
                    .map(|&m| ast.begin_token(m))
                    .unwrap_or_else(|| ast[ld].library_keyword);
                if first_tok == ast.begin_token(ld) {
                    return;
                }
                let del = c.range().token_start_start(ast.begin_token(ld), first_tok);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_deletion(del.offset, del.length);
                });
                return;
            }
        }
        let comment_range = c.utils.get_lines_range(c.range().node(comment), false);
        let token = ast[c.unit].begin_token;
        let prefix = if c.token_offset(token) == 0
            && ast.tokens.get(token).preceding_comments.get() == Some(ast.begin_token(comment))
        {
            let text = c.utils.get_range_text(comment_range);
            let ins = format!("{text}library;{eol}{eol}");
            let del = c.utils.get_lines_range(comment_range, true);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_deletion(del.offset, del.length);
                b.add_simple_insertion(0, &ins);
            });
            return;
        } else if c.token_offset(ast.begin_token(comment)) == 0 {
            String::new()
        } else {
            let line_idx = (c.line_info.get_location(comment_range.end()).line_number - 1) as usize;
            let line_start = c.line_info.get_offset_of_line(line_idx).unwrap_or(0);
            if line_start < comment_range.end() {
                eol.clone()
            } else {
                String::new()
            }
        };
        let off = comment_range.end();
        let ins = format!("{prefix}library;{eol}");
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(off, &ins);
        });
    }
);

// ---------------------------------------------------------------------------
// 29. RemoveUnnecessaryCast
// ---------------------------------------------------------------------------

producer!(
    RemoveUnnecessaryCast,
    k::REMOVE_UNNECESSARY_CAST,
    Some(&k::REMOVE_UNNECESSARY_CAST_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(as_expr) = ast.this_or_ancestor_of_type::<AsExpression>(c.node) else {
            return;
        };
        let expr = ast[as_expr].expression;
        let del = c.range().end_end(expr, as_expr);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del.offset, del.length);
        });
    }
);

// ---------------------------------------------------------------------------
// 30. RemoveUnnecessaryStringInterpolation
// ---------------------------------------------------------------------------

producer!(
    RemoveUnnecessaryStringInterpolation,
    k::REMOVE_UNNECESSARY_STRING_INTERPOLATION,
    Some(&k::REMOVE_UNNECESSARY_STRING_INTERPOLATION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(interp) = ast.cast::<StringInterpolation>(c.node) else {
            return;
        };
        let elems = ast.list_raw(ast[interp].elements);
        if elems.len() != 3 {
            return;
        }
        let (Some(open), Some(ie), Some(close)) = (
            ast.cast::<InterpolationString>(elems[0]),
            ast.cast::<InterpolationExpression>(elems[1]),
            ast.cast::<InterpolationString>(elems[2]),
        ) else {
            return;
        };
        let expr = ast[ie].expression.raw();
        let r_start = c.range().start_start(open, expr);
        let r_end = c.range().end_end(expr, close);
        let wrap = expr_precedence(ast, expr) < ExprPrecedence::Prefix
            && ast
                .parent(interp)
                .is_some_and(|p| ast.is::<PrefixExpression>(p));
        builder.add_dart_file_edit(c.path, |b| {
            if wrap {
                b.add_simple_replacement(r_start.offset, r_start.length, "(");
                b.add_simple_replacement(r_end.offset, r_end.length, ")");
            } else {
                b.add_deletion(r_start.offset, r_start.length);
                b.add_deletion(r_end.offset, r_end.length);
            }
        });
    }
);

// ---------------------------------------------------------------------------
// 31. ReplaceWithDecoratedBox
// ---------------------------------------------------------------------------

fn is_container_with_only_decoration(
    c: &ProducerContext<'_>,
    ice: Id<InstanceCreationExpression>,
) -> bool {
    let ast = c.ast;
    if !is_exact_widget_type(c.ctx, static_type(c, ice.raw()), "Container", URI_CONTAINER) {
        return false;
    }
    let args = ast.list_raw(ast[ast[ice].argument_list].arguments);
    let mut has_decoration = false;
    for &arg in args {
        let Some(na) = ast.cast::<NamedArgument>(arg) else {
            return false;
        };
        match c.lexeme(ast[na].name) {
            "key" | "child" => {}
            "decoration" => has_decoration = true,
            _ => return false,
        }
    }
    has_decoration
}

fn can_decorated_box_be_const(
    c: &ProducerContext<'_>,
    ice: Id<InstanceCreationExpression>,
    already_const: &[Id<InstanceCreationExpression>],
) -> bool {
    let ast = c.ast;
    for &arg in ast.list_raw(ast[ast[ice].argument_list].arguments) {
        let expr = if let Some(na) = ast.cast::<NamedArgument>(arg) {
            ast[na].argument_expression.raw()
        } else {
            arg
        };
        if let Some(child_ice) = ast.cast::<InstanceCreationExpression>(expr) {
            let is_const = ast[child_ice]
                .keyword
                .is_some_and(|kw| c.lexeme(kw) == "const")
                || already_const.contains(&child_ice);
            if !is_const {
                return false;
            }
        } else if !ast.is::<NullLiteral>(expr)
            && !ast.is::<BooleanLiteral>(expr)
            && !ast.is::<IntegerLiteral>(expr)
            && !ast.is::<DoubleLiteral>(expr)
            && !ast.is::<SimpleStringLiteral>(expr)
        {
            return false;
        }
    }
    true
}

producer!(
    ReplaceWithDecoratedBox,
    k::REPLACE_WITH_DECORATED_BOX,
    Some(&k::REPLACE_WITH_DECORATED_BOX_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let Some(ice) = ast.this_or_ancestor_of_type::<InstanceCreationExpression>(c.node) else {
            return;
        };
        let mut linted_nodes = Vec::new();
        let mut cur = Some(ice);
        while let Some(curr) = cur {
            if is_container_with_only_decoration(c, curr) {
                linted_nodes.push(curr);
            }
            cur = ast
                .parent(curr)
                .and_then(|p| ast.this_or_ancestor_of_type::<InstanceCreationExpression>(p));
        }
        let mut add_const: Vec<Id<InstanceCreationExpression>> = Vec::new();
        let mut remove_const: Vec<TokenId> = Vec::new();
        for &curr in linted_nodes.iter().rev() {
            if ast[curr].keyword.is_none() && can_decorated_box_be_const(c, curr, &add_const) {
                add_const.push(curr);
                for &child_arg in ast.list_raw(ast[ast[curr].argument_list].arguments) {
                    if let Some(na) = ast.cast::<NamedArgument>(child_arg) {
                        let expr = ast[na].argument_expression.raw();
                        if let Some(child_ice) = ast.cast::<InstanceCreationExpression>(expr) {
                            if linted_nodes.contains(&child_ice) {
                                if let Some(kw) = ast[child_ice].keyword {
                                    if c.lexeme(kw) == "const" {
                                        remove_const.push(kw);
                                    }
                                }
                            } else if let Some(kw) = ast[child_ice].keyword {
                                if c.lexeme(kw) == "const" {
                                    remove_const.push(kw);
                                }
                            }
                        }
                    }
                }
            }
        }
        let cn = ast[ice].constructor_name;
        let r_cn = c.range().node(cn);
        builder.add_dart_file_edit(c.path, |b| {
            if add_const.contains(&ice) {
                b.add_simple_replacement(r_cn.offset, r_cn.length, "const DecoratedBox");
            } else if let Some(kw) = ast[ice].keyword.filter(|k| remove_const.contains(k)) {
                let r = c.range().token_start_end(kw, ast.end_token(cn));
                b.add_simple_replacement(r.offset, r.length, "DecoratedBox");
            } else {
                b.add_simple_replacement(r_cn.offset, r_cn.length, "DecoratedBox");
            }
        });
    }
);

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use dartr_project::collection::CollectionOptions;

    use crate::correction::bulk_fix_processor::apply_edits;
    use crate::server::fix_cli::bulk_fixes;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    fn run_bulk_fix(codes: &[&str], source: &str) -> String {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "dartr_bulk_lints_test_{}_{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(
            dir.join("pubspec.yaml"),
            "name: test_pkg\nenvironment:\n  sdk: ^3.5.0\n",
        )
        .unwrap();
        let file_path = dir.join("lib").join("main.dart");
        std::fs::write(&file_path, source).unwrap();
        let file_str = file_path.to_string_lossy().to_string();
        let code_strings: Vec<String> = codes.iter().map(|s| s.to_string()).collect();
        let res = bulk_fixes(
            std::slice::from_ref(&file_str),
            &CollectionOptions::default(),
            Some(&code_strings),
        );
        let _ = std::fs::remove_dir_all(&dir);
        let (edits, _) = res.unwrap();
        let mut out = source.to_string();
        for fe in &edits {
            out = apply_edits(&out, &fe.edits);
        }
        out
    }

    #[test]
    fn test_add_trailing_comma_and_sort_constructors() {
        let src = "// @dart=3.5\nvoid g(\n  int a\n) {}\nclass A {\n  int x = 0;\n  A.named();\n  A(this.x);\n}\n";
        let fixed = run_bulk_fix(&["require_trailing_commas", "sort_constructors_first"], src);
        assert!(fixed.contains("int a,\n)"));
        assert!(fixed.find("A.named()").unwrap() < fixed.find("int x = 0;").unwrap());
    }

    #[test]
    fn test_make_required_named_parameters_first() {
        let src = "void f({int? a, required int b, required int c}) {}\n";
        let fixed = run_bulk_fix(&["always_put_required_named_parameters_first"], src);
        assert_eq!(
            fixed,
            "void f({required int b, required int c, int? a}) {}\n"
        );
    }

    #[test]
    fn test_remove_argument_and_unnecessary_lambdas() {
        let src = "void g({int a = 1, int b = 2}) {}\nvoid h(List<String> xs) {\n  g(a: 1, b: 3);\n  xs.forEach((x) => print(x));\n}\n";
        let fixed = run_bulk_fix(
            &["avoid_redundant_argument_values", "unnecessary_lambdas"],
            src,
        );
        assert!(fixed.contains("g(b: 3);"));
        assert!(fixed.contains("xs.forEach(print);"));
    }

    #[test]
    fn test_library_directives_and_combinators_ordering() {
        let src = "/// Doc\n\nimport 'dart:math'\n    show\n        sin,\n        cos;\n";
        let fixed = run_bulk_fix(
            &["dangling_library_doc_comments", "combinators_ordering"],
            src,
        );
        assert!(fixed.starts_with("/// Doc\nlibrary;\n"));
        assert!(fixed.contains("    show\n        cos,\n        sin;"));
    }

    #[test]
    fn test_remove_leading_underscore_and_split_declarations() {
        let src = "void f(int _p) {\n  int _a = _p, b = 2;\n  _a = 3;\n  print(_a + b);\n}\n";
        let fixed = run_bulk_fix(
            &[
                "no_leading_underscores_for_local_identifiers",
                "avoid_multiple_declarations_per_line",
            ],
            src,
        );
        assert!(fixed.contains("void f(int p)"));
        assert!(fixed.contains("int a = p;\n  int b = 2;"));
        assert!(fixed.contains("a = 3;"));
    }

    #[test]
    fn test_boolean_expression_and_if_null() {
        let src = "bool f(bool? a, int? b, bool c, bool d) {\n  final x = b == null ? 0 : b;\n  final y = a == true;\n  final z = d == true;\n  return c ? true : false;\n}\n";
        let fixed = run_bulk_fix(
            &[
                "prefer_if_null_operators",
                "use_if_null_to_convert_nulls_to_bools",
                "no_literal_bool_comparisons",
                "avoid_bool_literals_in_conditional_expressions",
            ],
            src,
        );
        assert!(fixed.contains("final x = b ?? 0;"));
        assert!(fixed.contains("final y = a ?? false;"));
        assert!(fixed.contains("final z = d;"));
        assert!(fixed.contains("return c;"));
    }

    #[test]
    fn test_inline_typedef_and_convert_to_function_declaration() {
        let src = "typedef _Pred = bool Function(String value);\nclass C {\n  final _Pred p;\n  C(this.p);\n}\nvoid g() {\n  var fn = (int x) => x + 1;\n  print(fn(1));\n}\n";
        let fixed = run_bulk_fix(
            &[
                "avoid_private_typedef_functions",
                "prefer_function_declarations_over_variables",
            ],
            src,
        );
        assert!(!fixed.contains("typedef _Pred"));
        assert!(fixed.contains("final bool Function(String) p;"));
        assert!(fixed.contains("int fn(int x) => x + 1;"));
    }

    #[test]
    fn test_strings_and_interpolations() {
        let src = "String f(String a) {\n  final s1 = r'hello';\n  final s2 = '\\\\d+';\n  final s3 = '$a';\n  final s4 = 'x: ' + a;\n  return s1 + s2 + s3 + s4;\n}\n";
        let fixed = run_bulk_fix(
            &[
                "unnecessary_raw_strings",
                "use_raw_strings",
                "unnecessary_string_interpolations",
                "prefer_interpolation_to_compose_strings",
            ],
            src,
        );
        assert!(fixed.contains("final s1 = 'hello';"));
        assert!(fixed.contains("final s2 = r'\\d+';"));
        assert!(fixed.contains("final s3 = a;"));
        assert!(fixed.contains("final s4 = 'x: $a';"));
    }
}
