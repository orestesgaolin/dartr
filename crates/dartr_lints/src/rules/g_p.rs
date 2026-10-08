// Dart source: pkg/linter/lib/src/rules.dart
// Ported Dart sources: pkg/linter/lib/src/rules/{leading_newlines_in_multiline_strings,
// library_names,library_prefixes,lines_longer_than_80_chars,migrate_design_widgets,
// missing_code_block_language_in_doc_comment,no_adjacent_strings_in_list,
// no_leading_underscores_for_library_prefixes,no_self_assignments,
// non_constant_identifier_names,package_prefixed_library_names,
// prefer_adjacent_string_concatenation,prefer_asserts_with_message,prefer_double_quotes,
// prefer_expression_function_bodies,prefer_generic_function_type_aliases,
// prefer_if_elements_to_conditional_expressions,prefer_if_null_operators,
// prefer_inlined_adds,prefer_is_not_operator,prefer_null_aware_method_calls,
// prefer_null_aware_operators,prefer_single_quotes,prefer_spread_collections,
// prefer_typing_uninitialized_variables}.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::doc_comment::CodeBlockType;
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub const RULES: &[&str] = &[
    "leading_newlines_in_multiline_strings",
    "library_names",
    "library_prefixes",
    "lines_longer_than_80_chars",
    "migrate_design_widgets",
    "missing_code_block_language_in_doc_comment",
    "no_adjacent_strings_in_list",
    "no_leading_underscores_for_library_prefixes",
    "no_self_assignments",
    "non_constant_identifier_names",
    "package_prefixed_library_names",
    "prefer_adjacent_string_concatenation",
    "prefer_asserts_with_message",
    "prefer_double_quotes",
    "prefer_expression_function_bodies",
    "prefer_generic_function_type_aliases",
    "prefer_if_elements_to_conditional_expressions",
    "prefer_if_null_operators",
    "prefer_inlined_adds",
    "prefer_is_not_operator",
    "prefer_null_aware_method_calls",
    "prefer_null_aware_operators",
    "prefer_single_quotes",
    "prefer_spread_collections",
    "prefer_typing_uninitialized_variables",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    _context: &LinterContext<'_>,
) -> bool {
    match name {
        "leading_newlines_in_multiline_strings" => {
            registry.add(
                NodeKind::SimpleStringLiteral,
                "leading_newlines_in_multiline_strings",
                leading_newlines_in_multiline_strings,
            );
            registry.add(
                NodeKind::StringInterpolation,
                "leading_newlines_in_multiline_strings",
                leading_newlines_in_multiline_strings,
            );
        }
        "library_names" => registry.add(NodeKind::LibraryDirective, "library_names", library_names),
        "library_prefixes" => registry.add(
            NodeKind::ImportDirective,
            "library_prefixes",
            library_prefixes,
        ),
        "lines_longer_than_80_chars" => registry.add(
            NodeKind::CompilationUnit,
            "lines_longer_than_80_chars",
            lines_longer_than_80_chars,
        ),
        "migrate_design_widgets" => registry.add(
            NodeKind::ImportDirective,
            "migrate_design_widgets",
            migrate_design_widgets,
        ),
        "missing_code_block_language_in_doc_comment" => registry.add(
            NodeKind::Comment,
            "missing_code_block_language_in_doc_comment",
            missing_code_block_language_in_doc_comment,
        ),
        "no_adjacent_strings_in_list" => registry.add(
            NodeKind::AdjacentStrings,
            "no_adjacent_strings_in_list",
            no_adjacent_strings_in_list,
        ),
        "no_leading_underscores_for_library_prefixes" => registry.add(
            NodeKind::ImportDirective,
            "no_leading_underscores_for_library_prefixes",
            no_leading_underscores_for_library_prefixes,
        ),
        "no_self_assignments" => registry.add(
            NodeKind::AssignmentExpression,
            "no_self_assignments",
            no_self_assignments,
        ),
        "non_constant_identifier_names" => {
            for kind in [
                NodeKind::CatchClause,
                NodeKind::ConstructorDeclaration,
                NodeKind::DeclaredVariablePattern,
                NodeKind::ForEachPartsWithDeclaration,
                NodeKind::FormalParameterList,
                NodeKind::FunctionDeclaration,
                NodeKind::MethodDeclaration,
                NodeKind::PrimaryConstructorName,
                NodeKind::RecordLiteral,
                NodeKind::RecordTypeAnnotation,
                NodeKind::VariableDeclaration,
            ] {
                registry.add(
                    kind,
                    "non_constant_identifier_names",
                    non_constant_identifier_names,
                );
            }
        }
        // The active upstream rule is intentionally a no-op until project
        // information is restored (dart-lang/linter#3395).
        "package_prefixed_library_names" => {}
        "prefer_adjacent_string_concatenation" => registry.add(
            NodeKind::BinaryExpression,
            "prefer_adjacent_string_concatenation",
            prefer_adjacent_string_concatenation,
        ),
        "prefer_asserts_with_message" => {
            registry.add(
                NodeKind::AssertInitializer,
                "prefer_asserts_with_message",
                prefer_asserts_with_message,
            );
            registry.add(
                NodeKind::AssertStatement,
                "prefer_asserts_with_message",
                prefer_asserts_with_message,
            );
        }
        "prefer_double_quotes" => {
            registry.add(
                NodeKind::SimpleStringLiteral,
                "prefer_double_quotes",
                prefer_double_quotes,
            );
            registry.add(
                NodeKind::StringInterpolation,
                "prefer_double_quotes",
                prefer_double_quotes,
            );
        }
        "prefer_expression_function_bodies" => registry.add(
            NodeKind::BlockFunctionBody,
            "prefer_expression_function_bodies",
            prefer_expression_function_bodies,
        ),
        "prefer_generic_function_type_aliases" => registry.add(
            NodeKind::FunctionTypeAlias,
            "prefer_generic_function_type_aliases",
            prefer_generic_function_type_aliases,
        ),
        "prefer_if_elements_to_conditional_expressions" => registry.add(
            NodeKind::ConditionalExpression,
            "prefer_if_elements_to_conditional_expressions",
            prefer_if_elements_to_conditional_expressions,
        ),
        "prefer_if_null_operators" => registry.add(
            NodeKind::ConditionalExpression,
            "prefer_if_null_operators",
            prefer_if_null_operators,
        ),
        "prefer_inlined_adds" => registry.add(
            NodeKind::MethodInvocation,
            "prefer_inlined_adds",
            prefer_inlined_adds,
        ),
        "prefer_is_not_operator" => registry.add(
            NodeKind::IsExpression,
            "prefer_is_not_operator",
            prefer_is_not_operator,
        ),
        "prefer_null_aware_method_calls" => {
            registry.add(
                NodeKind::ConditionalExpression,
                "prefer_null_aware_method_calls",
                prefer_null_aware_method_calls,
            );
            registry.add(
                NodeKind::IfStatement,
                "prefer_null_aware_method_calls",
                prefer_null_aware_method_calls,
            );
        }
        "prefer_null_aware_operators" => registry.add(
            NodeKind::ConditionalExpression,
            "prefer_null_aware_operators",
            prefer_null_aware_operators,
        ),
        "prefer_single_quotes" => {
            registry.add(
                NodeKind::SimpleStringLiteral,
                "prefer_single_quotes",
                prefer_single_quotes,
            );
            registry.add(
                NodeKind::StringInterpolation,
                "prefer_single_quotes",
                prefer_single_quotes,
            );
        }
        "prefer_spread_collections" => registry.add(
            NodeKind::MethodInvocation,
            "prefer_spread_collections",
            prefer_spread_collections,
        ),
        "prefer_typing_uninitialized_variables" => registry.add(
            NodeKind::VariableDeclarationList,
            "prefer_typing_uninitialized_variables",
            prefer_typing_uninitialized_variables,
        ),
        _ => return false,
    }
    true
}

fn token_text<'a>(ctx: &'a LinterContext<'_>, token: dartr_syntax::TokenId) -> &'a str {
    ctx.ast.tokens.lexeme(token)
}

fn is_lowercase_underscore_with_dots(value: &str) -> bool {
    if value.is_empty() || value.ends_with('.') {
        return false;
    }
    value.split('.').enumerate().all(|(index, part)| {
        !part.is_empty()
            && (index == 0 || part.as_bytes()[0].is_ascii_lowercase())
            && part
                .bytes()
                .all(|c| c == b'_' || c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

fn is_valid_library_prefix(value: &str) -> bool {
    let value = value.strip_prefix('$').unwrap_or(value);
    let Some(first_non_underscore) = value.bytes().find(|&c| c != b'_') else {
        return false;
    };
    first_non_underscore.is_ascii_lowercase()
        && value
            .bytes()
            .all(|c| c == b'_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn line_index_at(ctx: &LinterContext<'_>, offset: u32) -> usize {
    ctx.parsed
        .line_info
        .line_starts
        .partition_point(|&start| start <= offset)
        .saturating_sub(1)
}

fn leading_newlines_in_multiline_strings(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let lexeme = match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => token_text(
            ctx,
            ctx.ast
                .get(ctx.ast.cast::<SimpleStringLiteral>(node).unwrap())
                .literal,
        ),
        NodeKind::StringInterpolation => {
            let interpolation = ctx
                .ast
                .get(ctx.ast.cast::<StringInterpolation>(node).unwrap());
            let Some(first) = ctx.ast.list(interpolation.elements).first() else {
                return;
            };
            let Some(first) = ctx.ast.cast::<InterpolationString>(first.raw()) else {
                return;
            };
            token_text(ctx, ctx.ast.get(first).contents)
        }
        _ => return,
    };
    let quote_index = usize::from(lexeme.starts_with('r'));
    let bytes = lexeme.as_bytes();
    let Some(quote) = bytes.get(quote_index..quote_index + 3) else {
        return;
    };
    if quote != b"'''" && quote != b"\"\"\"" {
        return;
    }
    if line_index_at(ctx, ctx.ast.offset(node)) == line_index_at(ctx, ctx.ast.end(node)) {
        return;
    }
    if !matches!(bytes.get(quote_index + 3), Some(b'\n' | b'\r')) {
        ctx.report_node(out, &diag::LEADING_NEWLINES_IN_MULTILINE_STRINGS, node, &[]);
    }
}

fn string_piece_looks_like_uri_or_path(value: &str, is_raw: bool) -> bool {
    if value.contains('/') || (is_raw && value.contains('\\')) {
        return true;
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }
        let Some(&escaped) = bytes.get(index + 1) else {
            return false;
        };
        match escaped {
            b'\\' | b'/' => return true,
            b'x' if index + 3 < bytes.len() => {
                if u8::from_str_radix(&value[index + 2..index + 4], 16)
                    .is_ok_and(|c| matches!(c, b'/' | b'\\'))
                {
                    return true;
                }
                index += 4;
            }
            b'u' if bytes.get(index + 2) == Some(&b'{') => {
                let Some(relative_end) = value[index + 3..].find('}') else {
                    return false;
                };
                let end = index + 3 + relative_end;
                if u32::from_str_radix(&value[index + 3..end], 16)
                    .is_ok_and(|c| matches!(c, 0x2f | 0x5c))
                {
                    return true;
                }
                index = end + 1;
            }
            b'u' if index + 5 < bytes.len() => {
                if u32::from_str_radix(&value[index + 2..index + 6], 16)
                    .is_ok_and(|c| matches!(c, 0x2f | 0x5c))
                {
                    return true;
                }
                index += 6;
            }
            _ => index += 2,
        }
    }
    false
}

fn collect_allowed_string_lines(
    ctx: &LinterContext<'_>,
    node: NodeId,
    allowed: &mut std::collections::BTreeSet<usize>,
) {
    match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => {
            let literal = ctx
                .ast
                .get(ctx.ast.cast::<SimpleStringLiteral>(node).unwrap());
            let start = line_index_at(ctx, ctx.ast.offset(node));
            let end = line_index_at(ctx, ctx.ast.end(node));
            let lexeme = token_text(ctx, literal.literal);
            let quoted = lexeme.strip_prefix('r').unwrap_or(lexeme);
            if quoted.starts_with("'''") || quoted.starts_with("\"\"\"") {
                allowed.extend(start..=end);
            } else if string_piece_looks_like_uri_or_path(&literal.value, lexeme.starts_with('r')) {
                allowed.insert(start);
            }
            return;
        }
        NodeKind::StringInterpolation => {
            let interpolation = ctx
                .ast
                .get(ctx.ast.cast::<StringInterpolation>(node).unwrap());
            let start = line_index_at(ctx, ctx.ast.offset(node));
            let end = line_index_at(ctx, ctx.ast.end(node));
            let first_lexeme = token_text(ctx, ctx.ast.begin_token(node));
            let quoted = first_lexeme.strip_prefix('r').unwrap_or(first_lexeme);
            if quoted.starts_with("'''") || quoted.starts_with("\"\"\"") {
                allowed.extend(start..=end);
            } else {
                let is_raw = first_lexeme.starts_with('r');
                let has_path = ctx.ast.list(interpolation.elements).iter().any(|element| {
                    ctx.ast
                        .cast::<InterpolationString>(element.raw())
                        .is_some_and(|piece| {
                            string_piece_looks_like_uri_or_path(&ctx.ast.get(piece).value, is_raw)
                        })
                });
                if has_path {
                    allowed.insert(start);
                }
            }
            // The upstream visitor overrides visitStringInterpolation without
            // visiting its children. String literals inside interpolation
            // expressions therefore do not exempt the outer source line.
            return;
        }
        _ => {}
    }
    for child in ctx.ast.children(node) {
        collect_allowed_string_lines(ctx, child, allowed);
    }
}

fn looks_like_comment_uri_or_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

/*
 * Comment text is not a string literal, so backslashes are literal and the
 * upstream rule checks them directly.
 */
fn collect_allowed_comment_lines(
    ctx: &LinterContext<'_>,
    allowed: &mut std::collections::BTreeSet<usize>,
) {
    let unit = ctx.ast.get(ctx.parsed.unit);
    for token in ctx.ast.tokens.iter_from(unit.begin_token) {
        let mut comment = ctx.ast.tokens.get(token).preceding_comments;
        while let Some(comment_id) = comment.get() {
            let content = token_text(ctx, comment_id);
            let base_line = line_index_at(ctx, ctx.ast.tokens.offset(comment_id));
            if let Some(body) = content.strip_prefix("//")
                && body.trim_start().starts_with("ignore:")
            {
                allowed.insert(base_line);
            }
            let body = if let Some(body) = content.strip_prefix("///") {
                body
            } else if let Some(body) = content.strip_prefix("//") {
                body
            } else if content.starts_with("/*") && content.ends_with("*/") {
                &content[2..content.len() - 2]
            } else {
                ""
            };
            for (index, line) in body.lines().enumerate() {
                if looks_like_comment_uri_or_path(line) {
                    allowed.insert(base_line + index);
                }
            }
            let next = ctx.ast.tokens.get(comment_id).next;
            comment = if next == token {
                dartr_syntax::TokenId::NONE
            } else {
                next
            };
        }
    }
}

fn lines_longer_than_80_chars(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let utf16: Vec<u16> = ctx.source.encode_utf16().collect();
    let starts = &ctx.parsed.line_info.line_starts;
    let mut allowed = std::collections::BTreeSet::new();
    collect_allowed_string_lines(ctx, node, &mut allowed);
    collect_allowed_comment_lines(ctx, &mut allowed);
    for (index, &start) in starts.iter().enumerate() {
        let mut end = starts.get(index + 1).copied().unwrap_or(utf16.len() as u32);
        while end > start && matches!(utf16[(end - 1) as usize], 10 | 13) {
            end -= 1;
        }
        if end - start > 80 && !allowed.contains(&index) {
            ctx.report_offset(
                out,
                &diag::LINES_LONGER_THAN_80_CHARS,
                (start + 80) as usize,
                (end - start - 80) as usize,
                &[],
            );
        }
    }
}

fn library_names(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let directive = ctx.ast.get(ctx.ast.cast::<LibraryDirective>(node).unwrap());
    let Some(name) = directive.name else {
        return;
    };
    let text = ctx.text(name);
    if !is_lowercase_underscore_with_dots(&text) {
        ctx.report_node(out, &diag::LIBRARY_NAMES, name, &[&text]);
    }
}

fn library_prefixes(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let directive = ctx.ast.get(ctx.ast.cast::<ImportDirective>(node).unwrap());
    let Some(prefix) = directive.prefix else {
        return;
    };
    let text = token_text(ctx, ctx.ast.get(prefix).token);
    if text == "_" && ctx.parsed.language_version.effective() >= (3, 7) {
        return;
    }
    if !is_valid_library_prefix(text) {
        ctx.report_node(out, &diag::LIBRARY_PREFIXES, prefix, &[text]);
    }
}

fn migrate_design_widgets(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let directive = ctx.ast.get(ctx.ast.cast::<ImportDirective>(node).unwrap());
    let Some(uri) = ctx.ast.cast::<SimpleStringLiteral>(directive.uri.raw()) else {
        return;
    };
    let value = ctx.ast.get(uri).value.as_ref();
    if matches!(
        value,
        "package:flutter/material.dart" | "package:flutter/cupertino.dart"
    ) {
        ctx.report_node(out, &diag::MIGRATE_DESIGN_WIDGETS, directive.uri, &[value]);
    }
}

fn missing_code_block_language_in_doc_comment(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let comment = ctx.ast.get(ctx.ast.cast::<Comment>(node).unwrap());
    for block in &comment.code_blocks {
        if block.info_string.is_none()
            && block.ty == CodeBlockType::Fenced
            && let Some(line) = block.lines.first()
        {
            ctx.report_offset(
                out,
                &diag::MISSING_CODE_BLOCK_LANGUAGE_IN_DOC_COMMENT,
                line.offset as usize,
                line.length as usize,
                &[],
            );
        }
    }
}

fn no_adjacent_strings_in_list(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(parent) = ctx.ast.parent(node) else {
        return;
    };
    let report = match ctx.ast.kind(parent) {
        NodeKind::ListLiteral | NodeKind::SetOrMapLiteral | NodeKind::ForElement => true,
        NodeKind::IfElement => {
            let n = ctx.ast.get(ctx.ast.cast::<IfElement>(parent).unwrap());
            n.else_element.map(|n| n.raw()) == Some(node)
                || (n.else_element.is_none() && n.then_element.raw() == node)
        }
        NodeKind::ConstantPattern => ctx
            .ast
            .parent(parent)
            .is_some_and(|p| ctx.ast.kind(p) == NodeKind::ListPattern),
        _ => false,
    };
    if report {
        ctx.report_node(out, &diag::NO_ADJACENT_STRINGS_IN_LIST, node, &[]);
    }
}

fn no_leading_underscores_for_library_prefixes(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let directive = ctx.ast.get(ctx.ast.cast::<ImportDirective>(node).unwrap());
    let Some(prefix) = directive.prefix else {
        return;
    };
    let name = token_text(ctx, ctx.ast.get(prefix).token);
    if name == "_" && ctx.parsed.language_version.effective() >= (3, 7) {
        return;
    }
    if name.starts_with('_') {
        ctx.report_node(
            out,
            &diag::NO_LEADING_UNDERSCORES_FOR_LIBRARY_PREFIXES,
            prefix,
            &[name],
        );
    }
}

fn no_self_assignments(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let assignment = ctx
        .ast
        .get(ctx.ast.cast::<AssignmentExpression>(node).unwrap());
    if token_text(ctx, assignment.operator) != "=" {
        return;
    }
    if ctx
        .ast
        .cast::<Identifier>(assignment.left_hand_side.raw())
        .is_some()
        && ctx
            .ast
            .cast::<Identifier>(assignment.right_hand_side.raw())
            .is_some()
        && ctx.text(assignment.left_hand_side) == ctx.text(assignment.right_hand_side)
    {
        ctx.report_node(out, &diag::NO_SELF_ASSIGNMENTS, node, &[]);
    }
}

fn is_lower_camel_case(name: &str) -> bool {
    if name == "_" || (name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase()) {
        return true;
    }
    let bytes = name.as_bytes();
    let mut index = bytes.iter().take_while(|&&c| c == b'_').count();
    if index == bytes.len()
        || !(bytes[index].is_ascii_lowercase() || matches!(bytes[index], b'?' | b'$'))
    {
        return false;
    }
    index += 1;
    while index < bytes.len() {
        let c = bytes[index];
        if c.is_ascii_lowercase()
            || c.is_ascii_digit()
            || matches!(c, b'?' | b'$')
            || c.is_ascii_uppercase()
        {
            index += 1;
        } else if c == b'_' {
            if index + 1 == bytes.len() {
                return true;
            }
            if !bytes[index + 1].is_ascii_digit() {
                return false;
            }
            index += 2;
        } else {
            return false;
        }
    }
    true
}

fn check_non_constant_name(
    ctx: &LinterContext<'_>,
    token: dartr_syntax::TokenId,
    underscores_ok: bool,
    out: &mut Vec<Diagnostic>,
) {
    let name = token_text(ctx, token);
    if underscores_ok && name.bytes().all(|c| c == b'_') {
        return;
    }
    if !is_lower_camel_case(name) {
        ctx.report_token(out, &diag::NON_CONSTANT_IDENTIFIER_NAMES, token, &[name]);
    }
}

fn non_constant_identifier_names(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::CatchClause => {
            let n = ctx.ast.get(ctx.ast.cast::<CatchClause>(node).unwrap());
            if let Some(p) = n.exception_parameter {
                check_non_constant_name(ctx, ctx.ast.get(p).name, true, out);
            }
            if let Some(p) = n.stack_trace_parameter {
                check_non_constant_name(ctx, ctx.ast.get(p).name, true, out);
            }
        }
        NodeKind::ConstructorDeclaration => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<ConstructorDeclaration>(node).unwrap());
            if n.augment_keyword.is_none()
                && let Some(name) = n.name
            {
                check_non_constant_name(ctx, name, true, out);
            }
        }
        NodeKind::DeclaredVariablePattern => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<DeclaredVariablePattern>(node).unwrap());
            let mut parent = ctx.ast.parent(node);
            if parent.is_some_and(|p| {
                matches!(
                    ctx.ast.kind(p),
                    NodeKind::NullCheckPattern | NodeKind::NullAssertPattern
                )
            }) {
                parent = parent.and_then(|p| ctx.ast.parent(p));
            }
            let shortcut = parent
                .and_then(|p| ctx.ast.cast::<PatternField>(p))
                .and_then(|p| ctx.ast.get(p).name)
                .is_some_and(|name| ctx.ast.get(name).name.is_none());
            if !shortcut {
                check_non_constant_name(ctx, n.name, false, out);
            }
        }
        NodeKind::ForEachPartsWithDeclaration => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<ForEachPartsWithDeclaration>(node).unwrap());
            check_non_constant_name(ctx, ctx.ast.get(n.loop_variable).name, false, out);
        }
        NodeKind::FormalParameterList => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<FormalParameterList>(node).unwrap());
            for &parameter in ctx.ast.list(n.parameters) {
                if let Some(p) = ctx.ast.cast::<RegularFormalParameter>(parameter.raw())
                    && let Some(name) = ctx.ast.get(p).name
                {
                    check_non_constant_name(ctx, name, true, out);
                }
            }
        }
        NodeKind::FunctionDeclaration => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<FunctionDeclaration>(node).unwrap());
            if n.augment_keyword.is_none() {
                check_non_constant_name(ctx, n.name, false, out);
            }
        }
        NodeKind::MethodDeclaration => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<MethodDeclaration>(node).unwrap());
            if n.operator_keyword.is_none() && n.augment_keyword.is_none() {
                check_non_constant_name(ctx, n.name, false, out);
            }
        }
        NodeKind::PrimaryConstructorName => check_non_constant_name(
            ctx,
            ctx.ast
                .get(ctx.ast.cast::<PrimaryConstructorName>(node).unwrap())
                .name,
            true,
            out,
        ),
        NodeKind::RecordLiteral => {
            let n = ctx.ast.get(ctx.ast.cast::<RecordLiteral>(node).unwrap());
            for &field in ctx.ast.list(n.fields) {
                if let Some(field) = ctx.ast.cast::<RecordLiteralNamedField>(field.raw()) {
                    check_non_constant_name(ctx, ctx.ast.get(field).name, false, out);
                }
            }
        }
        NodeKind::RecordTypeAnnotation => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<RecordTypeAnnotation>(node).unwrap());
            for &field in ctx.ast.list(n.positional_fields) {
                if let Some(name) = ctx.ast.get(field).name {
                    check_non_constant_name(ctx, name, false, out);
                }
            }
            if let Some(fields) = n.named_fields {
                for &field in ctx.ast.list(ctx.ast.get(fields).fields) {
                    check_non_constant_name(ctx, ctx.ast.get(field).name, false, out);
                }
            }
        }
        NodeKind::VariableDeclaration => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<VariableDeclaration>(node).unwrap());
            let is_const = ctx
                .ast
                .parent(node)
                .and_then(|p| ctx.ast.cast::<VariableDeclarationList>(p))
                .and_then(|p| ctx.ast.get(p).keyword)
                .is_some_and(|t| token_text(ctx, t) == "const");
            if !is_const {
                check_non_constant_name(ctx, n.name, false, out);
            }
        }
        _ => {}
    }
}

fn prefer_adjacent_string_concatenation(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let binary = ctx.ast.get(ctx.ast.cast::<BinaryExpression>(node).unwrap());
    if token_text(ctx, binary.operator) == "+"
        && ctx
            .ast
            .cast::<StringLiteral>(binary.left_operand.raw())
            .is_some()
        && ctx
            .ast
            .cast::<StringLiteral>(binary.right_operand.raw())
            .is_some()
    {
        ctx.report_token(
            out,
            &diag::PREFER_ADJACENT_STRING_CONCATENATION,
            binary.operator,
            &[],
        );
    }
}

fn prefer_asserts_with_message(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let missing = match ctx.ast.kind(node) {
        NodeKind::AssertInitializer => ctx
            .ast
            .get(ctx.ast.cast::<AssertInitializer>(node).unwrap())
            .message
            .is_none(),
        NodeKind::AssertStatement => ctx
            .ast
            .get(ctx.ast.cast::<AssertStatement>(node).unwrap())
            .message
            .is_none(),
        _ => false,
    };
    if missing {
        ctx.report_node(out, &diag::PREFER_ASSERTS_WITH_MESSAGE, node, &[]);
    }
}

fn string_is_single_quoted(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let token = match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => {
            ctx.ast
                .get(ctx.ast.cast::<SimpleStringLiteral>(node).unwrap())
                .literal
        }
        _ => ctx.ast.begin_token(node),
    };
    token_text(ctx, token)
        .trim_start_matches('r')
        .starts_with('\'')
}

fn has_string_ancestor(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let mut parent = ctx.ast.parent(node);
    while let Some(current) = parent {
        if ctx.ast.kind(current) == NodeKind::StringInterpolation {
            return true;
        }
        parent = ctx.ast.parent(current);
    }
    false
}

fn subtree_contains_nested_string(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    ctx.ast.children(node).into_iter().any(|child| {
        matches!(
            ctx.ast.kind(child),
            NodeKind::SimpleStringLiteral | NodeKind::StringInterpolation
        ) || subtree_contains_nested_string(ctx, child)
    })
}

fn quote_rule(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>, use_single: bool) {
    let single = string_is_single_quoted(ctx, node);
    let forbidden = if use_single { '\'' } else { '"' };
    if single == use_single {
        return;
    }
    match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => {
            let literal = ctx
                .ast
                .get(ctx.ast.cast::<SimpleStringLiteral>(node).unwrap());
            if literal.value.contains(forbidden) || has_string_ancestor(ctx, node) {
                return;
            }
            ctx.report_token(
                out,
                if use_single {
                    &diag::PREFER_SINGLE_QUOTES
                } else {
                    &diag::PREFER_DOUBLE_QUOTES
                },
                literal.literal,
                &[],
            );
        }
        NodeKind::StringInterpolation => {
            let interpolation = ctx
                .ast
                .get(ctx.ast.cast::<StringInterpolation>(node).unwrap());
            let contains_forbidden = ctx.ast.list(interpolation.elements).iter().any(|element| {
                ctx.ast
                    .cast::<InterpolationString>(element.raw())
                    .is_some_and(|id| ctx.ast.get(id).value.contains(forbidden))
            });
            if contains_forbidden
                || has_string_ancestor(ctx, node)
                || subtree_contains_nested_string(ctx, node)
            {
                return;
            }
            ctx.report_node(
                out,
                if use_single {
                    &diag::PREFER_SINGLE_QUOTES
                } else {
                    &diag::PREFER_DOUBLE_QUOTES
                },
                node,
                &[],
            );
        }
        _ => {}
    }
}

fn prefer_double_quotes(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    quote_rule(ctx, node, out, false);
}
fn prefer_single_quotes(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    quote_rule(ctx, node, out, true);
}

fn prefer_expression_function_bodies(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let body = ctx
        .ast
        .get(ctx.ast.cast::<BlockFunctionBody>(node).unwrap());
    let statements = ctx.ast.list(ctx.ast.get(body.block).statements);
    if statements.len() == 1
        && let Some(ret) = ctx.ast.cast::<ReturnStatement>(statements[0].raw())
        && ctx.ast.get(ret).expression.is_some()
    {
        ctx.report_node(out, &diag::PREFER_EXPRESSION_FUNCTION_BODIES, node, &[]);
    }
}

fn prefer_generic_function_type_aliases(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let alias = ctx
        .ast
        .get(ctx.ast.cast::<FunctionTypeAlias>(node).unwrap());
    if ctx.ast.tokens.get(alias.semicolon).is_synthetic() {
        return;
    }
    let mut replacement = String::new();
    if let Some(return_type) = alias.return_type {
        replacement.push_str(&ctx.text(return_type));
        replacement.push(' ');
    }
    replacement.push_str("Function");
    if let Some(parameters) = alias.type_parameters {
        replacement.push_str(&ctx.text(parameters));
    }
    replacement.push_str(&ctx.text(alias.parameters));
    ctx.report_token(
        out,
        &diag::PREFER_GENERIC_FUNCTION_TYPE_ALIASES,
        alias.name,
        &[&replacement],
    );
}

fn prefer_if_elements_to_conditional_expressions(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let mut error_node = node;
    let mut parent = ctx.ast.parent(node);
    while let Some(id) = parent.filter(|id| ctx.ast.kind(*id) == NodeKind::ParenthesizedExpression)
    {
        error_node = id;
        parent = ctx.ast.parent(id);
    }
    let report = parent.is_some_and(|parent| match ctx.ast.kind(parent) {
        NodeKind::ListLiteral | NodeKind::SetOrMapLiteral => true,
        NodeKind::IfElement => {
            let p = ctx.ast.get(ctx.ast.cast::<IfElement>(parent).unwrap());
            p.then_element.raw() == error_node
                || p.else_element.map(|n| n.raw()) == Some(error_node)
        }
        NodeKind::ForElement => {
            ctx.ast
                .get(ctx.ast.cast::<ForElement>(parent).unwrap())
                .body
                .raw()
                == error_node
        }
        _ => false,
    });
    if report {
        ctx.report_node(
            out,
            &diag::PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS,
            error_node,
            &[],
        );
    }
}

fn prefer_if_null_operators(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let conditional = ctx
        .ast
        .get(ctx.ast.cast::<ConditionalExpression>(node).unwrap());
    let Some(condition_id) = ctx
        .ast
        .cast::<BinaryExpression>(conditional.condition.raw())
    else {
        return;
    };
    let condition = ctx.ast.get(condition_id);
    let operator = token_text(ctx, condition.operator);
    if !matches!(operator, "==" | "!=") {
        return;
    }
    let compared = if ctx.ast.kind(condition.left_operand.raw()) == NodeKind::NullLiteral {
        condition.right_operand.raw()
    } else if ctx.ast.kind(condition.right_operand.raw()) == NodeKind::NullLiteral {
        condition.left_operand.raw()
    } else {
        return;
    };
    let result = if operator == "==" {
        conditional.else_expression.raw()
    } else {
        conditional.then_expression.raw()
    };
    if ctx.text(result) == ctx.text(compared) {
        ctx.report_node(out, &diag::PREFER_IF_NULL_OPERATORS, node, &[]);
    }
}

fn cascade_for_invocation(ctx: &LinterContext<'_>, node: NodeId) -> Option<Id<CascadeExpression>> {
    let invocation = ctx.ast.get(ctx.ast.cast::<MethodInvocation>(node)?);
    let operator = invocation
        .operator
        .map(|t| token_text(ctx, t))
        .unwrap_or("");
    if !operator.contains("..") {
        return None;
    }
    ctx.ast.this_or_ancestor_of_type::<CascadeExpression>(node)
}

fn prefer_inlined_adds(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let invocation_id = ctx.ast.cast::<MethodInvocation>(node).unwrap();
    let invocation = ctx.ast.get(invocation_id);
    let method_name = token_text(ctx, ctx.ast.get(invocation.method_name).token);
    let add_all = method_name == "addAll";
    if method_name != "add" && !add_all {
        return;
    }
    let Some(cascade_id) = cascade_for_invocation(ctx, node) else {
        return;
    };
    let arguments = ctx
        .ast
        .list(ctx.ast.get(invocation.argument_list).arguments);
    if arguments.len() != 1 {
        return;
    }
    let cascade = ctx.ast.get(cascade_id);
    let sections = ctx.ast.list(cascade.cascade_sections);
    if ctx.ast.kind(cascade.target.raw()) != NodeKind::ListLiteral
        || sections.first().map(|n| n.raw()) != Some(node)
    {
        return;
    }
    if add_all && ctx.ast.kind(arguments[0].raw()) != NodeKind::ListLiteral {
        return;
    }
    ctx.report_node(
        out,
        if add_all {
            &diag::PREFER_INLINED_ADDS_MULTIPLE
        } else {
            &diag::PREFER_INLINED_ADDS_SINGLE
        },
        invocation.method_name,
        &[],
    );
}

fn prefer_is_not_operator(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let is_expression = ctx.ast.get(ctx.ast.cast::<IsExpression>(node).unwrap());
    if is_expression.not_operator.is_some() {
        return;
    }
    let Some(paren) = ctx
        .ast
        .parent(node)
        .and_then(|p| ctx.ast.cast::<ParenthesizedExpression>(p))
    else {
        return;
    };
    let Some(prefix) = ctx
        .ast
        .parent(paren.raw())
        .and_then(|p| ctx.ast.cast::<PrefixExpression>(p))
    else {
        return;
    };
    if token_text(ctx, ctx.ast.get(prefix).operator) == "!" {
        ctx.report_node(out, &diag::PREFER_IS_NOT_OPERATOR, prefix, &[]);
    }
}

fn checked_non_null_target(ctx: &LinterContext<'_>, expression: NodeId, compared: NodeId) -> bool {
    let Some(invocation) = ctx.ast.cast::<FunctionExpressionInvocation>(expression) else {
        return false;
    };
    let Some(postfix) = ctx
        .ast
        .cast::<PostfixExpression>(ctx.ast.get(invocation).function.raw())
    else {
        return false;
    };
    let postfix = ctx.ast.get(postfix);
    token_text(ctx, postfix.operator) == "!" && ctx.text(postfix.operand) == ctx.text(compared)
}

fn prefer_null_aware_method_calls(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let (condition_node, invoked) = match ctx.ast.kind(node) {
        NodeKind::ConditionalExpression => {
            let n = ctx
                .ast
                .get(ctx.ast.cast::<ConditionalExpression>(node).unwrap());
            if ctx.ast.kind(n.else_expression.raw()) != NodeKind::NullLiteral {
                return;
            }
            (n.condition.raw(), n.then_expression.raw())
        }
        NodeKind::IfStatement => {
            let n = ctx.ast.get(ctx.ast.cast::<IfStatement>(node).unwrap());
            if n.else_keyword.is_some() {
                return;
            }
            let mut statement = n.then_statement.raw();
            if let Some(block) = ctx.ast.cast::<Block>(statement) {
                let statements = ctx.ast.list(ctx.ast.get(block).statements);
                if statements.len() != 1 {
                    return;
                }
                statement = statements[0].raw();
            }
            let Some(statement) = ctx.ast.cast::<ExpressionStatement>(statement) else {
                return;
            };
            (n.expression.raw(), ctx.ast.get(statement).expression.raw())
        }
        _ => return,
    };
    let Some(condition) = ctx.ast.cast::<BinaryExpression>(condition_node) else {
        return;
    };
    let condition = ctx.ast.get(condition);
    if token_text(ctx, condition.operator) != "!="
        || ctx.ast.kind(condition.right_operand.raw()) != NodeKind::NullLiteral
    {
        return;
    }
    if checked_non_null_target(ctx, invoked, condition.left_operand.raw()) {
        ctx.report_node(out, &diag::PREFER_NULL_AWARE_METHOD_CALLS, invoked, &[]);
    }
}

fn prefer_null_aware_operators(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let conditional = ctx
        .ast
        .get(ctx.ast.cast::<ConditionalExpression>(node).unwrap());
    let Some(condition) = ctx
        .ast
        .cast::<BinaryExpression>(conditional.condition.raw())
    else {
        return;
    };
    let condition = ctx.ast.get(condition);
    let compared = if ctx.ast.kind(condition.left_operand.raw()) == NodeKind::NullLiteral {
        condition.right_operand.raw()
    } else if ctx.ast.kind(condition.right_operand.raw()) == NodeKind::NullLiteral {
        condition.left_operand.raw()
    } else {
        return;
    };
    let result = match token_text(ctx, condition.operator) {
        "==" if ctx.ast.kind(conditional.then_expression.raw()) == NodeKind::NullLiteral => {
            conditional.else_expression.raw()
        }
        "!=" if ctx.ast.kind(conditional.else_expression.raw()) == NodeKind::NullLiteral => {
            conditional.then_expression.raw()
        }
        _ => return,
    };
    let mut current = Some(result);
    while let Some(id) = current {
        current = match ctx.ast.kind(id) {
            NodeKind::PrefixedIdentifier => Some(
                ctx.ast
                    .get(ctx.ast.cast::<PrefixedIdentifier>(id).unwrap())
                    .prefix
                    .raw(),
            ),
            NodeKind::MethodInvocation => ctx
                .ast
                .get(ctx.ast.cast::<MethodInvocation>(id).unwrap())
                .target
                .map(|n| n.raw()),
            NodeKind::PostfixExpression => {
                let p = ctx.ast.get(ctx.ast.cast::<PostfixExpression>(id).unwrap());
                (token_text(ctx, p.operator) == "!").then_some(p.operand.raw())
            }
            NodeKind::PropertyAccess => ctx
                .ast
                .get(ctx.ast.cast::<PropertyAccess>(id).unwrap())
                .target
                .map(|n| n.raw()),
            _ => None,
        };
        if current.is_some_and(|id| ctx.text(id) == ctx.text(compared)) {
            ctx.report_node(out, &diag::PREFER_NULL_AWARE_OPERATORS, node, &[]);
            return;
        }
    }
}

fn is_in_constant_context(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let mut current = Some(node);
    while let Some(id) = current {
        match ctx.ast.kind(id) {
            NodeKind::Annotation => return true,
            NodeKind::ListLiteral => {
                if ctx
                    .ast
                    .get(ctx.ast.cast::<ListLiteral>(id).unwrap())
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::SetOrMapLiteral => {
                if ctx
                    .ast
                    .get(ctx.ast.cast::<SetOrMapLiteral>(id).unwrap())
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::InstanceCreationExpression => {
                let n = ctx
                    .ast
                    .get(ctx.ast.cast::<InstanceCreationExpression>(id).unwrap());
                if n.keyword.is_some_and(|t| token_text(ctx, t) == "const") {
                    return true;
                }
            }
            _ => {}
        }
        current = ctx.ast.parent(id);
    }
    false
}

fn prefer_spread_collections(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let invocation_id = ctx.ast.cast::<MethodInvocation>(node).unwrap();
    let invocation = ctx.ast.get(invocation_id);
    if token_text(ctx, ctx.ast.get(invocation.method_name).token) != "addAll" {
        return;
    }
    let Some(cascade_id) = cascade_for_invocation(ctx, node) else {
        return;
    };
    let arguments = ctx
        .ast
        .list(ctx.ast.get(invocation.argument_list).arguments);
    if arguments.len() != 1 || ctx.ast.kind(arguments[0].raw()) == NodeKind::ListLiteral {
        return;
    }
    let cascade = ctx.ast.get(cascade_id);
    let sections = ctx.ast.list(cascade.cascade_sections);
    if sections.first().map(|n| n.raw()) != Some(node)
        || ctx.ast.kind(cascade.target.raw()) != NodeKind::ListLiteral
        || is_in_constant_context(ctx, cascade.target.raw())
    {
        return;
    }
    ctx.report_node(
        out,
        &diag::PREFER_SPREAD_COLLECTIONS,
        invocation.method_name,
        &[],
    );
}

fn prefer_typing_uninitialized_variables(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let list = ctx
        .ast
        .get(ctx.ast.cast::<VariableDeclarationList>(node).unwrap());
    if list.type_.is_some() {
        return;
    }
    let field = ctx
        .ast
        .parent(node)
        .is_some_and(|p| ctx.ast.kind(p) == NodeKind::FieldDeclaration);
    let code = if field {
        &diag::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_FIELD
    } else {
        &diag::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_LOCAL_VARIABLE
    };
    for &variable in ctx.ast.list(list.variables) {
        if ctx.ast.get(variable).initializer.is_none() {
            ctx.report_node(out, code, variable, &[]);
        }
    }
}
