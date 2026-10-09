// Dart sources: pkg/linter/lib/src/rules/{require_trailing_commas,simple_directive_paths,
// slash_for_doc_comments,sort_constructors_first,unintended_html_in_doc_comment,
// test_types_in_equals,throw_in_finally,unnecessary_breaks,
// unnecessary_brace_in_string_interps,unnecessary_const,
// unnecessary_const_in_enum_constructor,unnecessary_final,unnecessary_late,
// unnecessary_library_directive,unnecessary_library_name,unnecessary_new,
// unnecessary_null_in_if_null_operators,unnecessary_primary_constructor_body,
// unnecessary_raw_strings,unnecessary_string_escapes,
// unnecessary_type_name_in_constructor,use_function_type_syntax_for_parameters,
// use_raw_strings,use_string_in_part_of_directives,var_with_no_type_annotation}.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};

pub const RULES: &[&str] = &[
    "require_trailing_commas",
    "simple_directive_paths",
    "slash_for_doc_comments",
    "sort_constructors_first",
    "test_types_in_equals",
    "throw_in_finally",
    "unintended_html_in_doc_comment",
    "unnecessary_breaks",
    "unnecessary_brace_in_string_interps",
    "unnecessary_const",
    "unnecessary_const_in_enum_constructor",
    "unnecessary_final",
    "unnecessary_late",
    "unnecessary_library_directive",
    "unnecessary_library_name",
    "unnecessary_new",
    "unnecessary_null_in_if_null_operators",
    "unnecessary_primary_constructor_body",
    "unnecessary_raw_strings",
    "unnecessary_string_escapes",
    "unnecessary_type_name_in_constructor",
    "use_function_type_syntax_for_parameters",
    "use_raw_strings",
    "use_string_in_part_of_directives",
    "var_with_no_type_annotation",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    let primary_constructors =
        context.is_feature_enabled(crate::ExperimentalFlag::PrimaryConstructors);
    match name {
        "require_trailing_commas" => {
            if context.defining_unit().parsed.language_version.effective() < (3, 7) {
                for kind in [
                    NodeKind::ArgumentList,
                    NodeKind::AssertInitializer,
                    NodeKind::AssertStatement,
                    NodeKind::FormalParameterList,
                    NodeKind::ListLiteral,
                    NodeKind::SetOrMapLiteral,
                ] {
                    registry.add(kind, "require_trailing_commas", require_trailing_commas);
                }
            }
        }
        "simple_directive_paths" => {
            registry.add(
                NodeKind::Configuration,
                "simple_directive_paths",
                simple_directive_paths,
            );
            registry.add(
                NodeKind::ExportDirective,
                "simple_directive_paths",
                simple_directive_paths,
            );
            registry.add(
                NodeKind::ImportDirective,
                "simple_directive_paths",
                simple_directive_paths,
            );
            registry.add(
                NodeKind::PartDirective,
                "simple_directive_paths",
                simple_directive_paths,
            );
            registry.add(
                NodeKind::PartOfDirective,
                "simple_directive_paths",
                simple_directive_paths,
            );
        }
        "slash_for_doc_comments" => registry.add(
            NodeKind::Comment,
            "slash_for_doc_comments",
            slash_for_doc_comments,
        ),
        "sort_constructors_first" => {
            registry.add(
                NodeKind::ClassDeclaration,
                "sort_constructors_first",
                sort_constructors_first,
            );
            registry.add(
                NodeKind::EnumDeclaration,
                "sort_constructors_first",
                sort_constructors_first,
            );
            registry.add(
                NodeKind::ExtensionTypeDeclaration,
                "sort_constructors_first",
                sort_constructors_first,
            );
        }
        "test_types_in_equals" => registry.add(
            NodeKind::AsExpression,
            "test_types_in_equals",
            test_types_in_equals,
        ),
        "throw_in_finally" => registry.add(
            NodeKind::ThrowExpression,
            "throw_in_finally",
            throw_in_finally,
        ),
        "unintended_html_in_doc_comment" => registry.add(
            NodeKind::Comment,
            "unintended_html_in_doc_comment",
            unintended_html_in_doc_comment,
        ),
        "unnecessary_breaks" => {
            if context.is_feature_enabled(crate::ExperimentalFlag::Patterns) {
                registry.add(
                    NodeKind::BreakStatement,
                    "unnecessary_breaks",
                    unnecessary_breaks,
                );
            }
        }
        "unnecessary_brace_in_string_interps" => registry.add(
            NodeKind::StringInterpolation,
            "unnecessary_brace_in_string_interps",
            unnecessary_brace_in_string_interps,
        ),
        "unnecessary_const" => {
            registry.add(
                NodeKind::DotShorthandConstructorInvocation,
                "unnecessary_const",
                unnecessary_const,
            );
            registry.add(
                NodeKind::InstanceCreationExpression,
                "unnecessary_const",
                unnecessary_const,
            );
            registry.add(
                NodeKind::ListLiteral,
                "unnecessary_const",
                unnecessary_const,
            );
            registry.add(
                NodeKind::RecordLiteral,
                "unnecessary_const",
                unnecessary_const,
            );
            registry.add(
                NodeKind::SetOrMapLiteral,
                "unnecessary_const",
                unnecessary_const,
            );
        }
        "unnecessary_const_in_enum_constructor" => {
            if !primary_constructors {
                return true;
            }
            registry.add(
                NodeKind::ConstructorDeclaration,
                "unnecessary_const_in_enum_constructor",
                unnecessary_const_in_enum_constructor,
            );
            registry.add(
                NodeKind::PrimaryConstructorDeclaration,
                "unnecessary_const_in_enum_constructor",
                unnecessary_const_in_enum_constructor,
            );
        }
        "unnecessary_final" => {
            if !primary_constructors {
                registry.add(
                    NodeKind::FormalParameterList,
                    "unnecessary_final",
                    unnecessary_final,
                );
            }
            registry.add(
                NodeKind::VariableDeclarationStatement,
                "unnecessary_final",
                unnecessary_final,
            );
            registry.add(
                NodeKind::ForStatement,
                "unnecessary_final",
                unnecessary_final,
            );
            registry.add(
                NodeKind::DeclaredVariablePattern,
                "unnecessary_final",
                unnecessary_final,
            );
        }
        "unnecessary_late" => {
            registry.add(
                NodeKind::FieldDeclaration,
                "unnecessary_late",
                unnecessary_late,
            );
            registry.add(
                NodeKind::TopLevelVariableDeclaration,
                "unnecessary_late",
                unnecessary_late,
            );
        }
        "unnecessary_library_directive" => registry.add(
            NodeKind::LibraryDirective,
            "unnecessary_library_directive",
            unnecessary_library_directive,
        ),
        "unnecessary_library_name" => {
            if context.is_feature_enabled(crate::ExperimentalFlag::UnnamedLibraries) {
                registry.add(
                    NodeKind::LibraryDirective,
                    "unnecessary_library_name",
                    unnecessary_library_name,
                );
            }
        }
        "unnecessary_new" => registry.add(
            NodeKind::InstanceCreationExpression,
            "unnecessary_new",
            unnecessary_new,
        ),
        "unnecessary_null_in_if_null_operators" => registry.add(
            NodeKind::BinaryExpression,
            "unnecessary_null_in_if_null_operators",
            unnecessary_null_in_if_null_operators,
        ),
        "unnecessary_primary_constructor_body" => registry.add(
            NodeKind::PrimaryConstructorBody,
            "unnecessary_primary_constructor_body",
            unnecessary_primary_constructor_body,
        ),
        "unnecessary_raw_strings" => registry.add(
            NodeKind::SimpleStringLiteral,
            "unnecessary_raw_strings",
            unnecessary_raw_strings,
        ),
        "unnecessary_string_escapes" => {
            registry.add(
                NodeKind::SimpleStringLiteral,
                "unnecessary_string_escapes",
                unnecessary_string_escapes,
            );
            registry.add(
                NodeKind::StringInterpolation,
                "unnecessary_string_escapes",
                unnecessary_string_escapes,
            );
        }
        "unnecessary_type_name_in_constructor" => {
            if primary_constructors {
                registry.add(
                    NodeKind::ConstructorDeclaration,
                    "unnecessary_type_name_in_constructor",
                    unnecessary_type_name_in_constructor,
                );
            }
        }
        "use_function_type_syntax_for_parameters" => {
            registry.add(
                NodeKind::FieldFormalParameter,
                "use_function_type_syntax_for_parameters",
                use_function_type_syntax_for_parameters,
            );
            registry.add(
                NodeKind::RegularFormalParameter,
                "use_function_type_syntax_for_parameters",
                use_function_type_syntax_for_parameters,
            );
            registry.add(
                NodeKind::SuperFormalParameter,
                "use_function_type_syntax_for_parameters",
                use_function_type_syntax_for_parameters,
            );
        }
        "use_raw_strings" => registry.add(
            NodeKind::SimpleStringLiteral,
            "use_raw_strings",
            use_raw_strings,
        ),
        "use_string_in_part_of_directives" => {
            if !context.is_feature_enabled(crate::ExperimentalFlag::EnhancedParts) {
                registry.add(
                    NodeKind::PartOfDirective,
                    "use_string_in_part_of_directives",
                    use_string_in_part_of_directives,
                );
            }
        }
        "var_with_no_type_annotation" => {
            if !primary_constructors {
                registry.add(
                    NodeKind::FormalParameterList,
                    "var_with_no_type_annotation",
                    var_with_no_type_annotation,
                );
            }
        }
        _ => return false,
    }
    true
}

fn require_trailing_commas(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (opening, closing, last, error) = match ctx.ast.kind(node) {
        NodeKind::ArgumentList => {
            let n = &ctx.ast[Id::<ArgumentList>::from_raw(node)];
            let Some(last) = ctx.ast.list(n.arguments).last() else {
                return;
            };
            (
                n.left_parenthesis,
                n.right_parenthesis,
                last.raw(),
                n.right_parenthesis,
            )
        }
        NodeKind::AssertInitializer => {
            let n = &ctx.ast[Id::<AssertInitializer>::from_raw(node)];
            (
                n.left_parenthesis,
                n.right_parenthesis,
                n.message.unwrap_or(n.condition).raw(),
                n.right_parenthesis,
            )
        }
        NodeKind::AssertStatement => {
            let n = &ctx.ast[Id::<AssertStatement>::from_raw(node)];
            (
                n.left_parenthesis,
                n.right_parenthesis,
                n.message.unwrap_or(n.condition).raw(),
                n.right_parenthesis,
            )
        }
        NodeKind::FormalParameterList => {
            let n = &ctx.ast[Id::<FormalParameterList>::from_raw(node)];
            let Some(last) = ctx.ast.list(n.parameters).last() else {
                return;
            };
            if ctx.ast.parent(node).is_some_and(|p| {
                ctx.ast.kind(p) == NodeKind::PrimaryConstructorDeclaration
                    && ctx
                        .ast
                        .parent(p)
                        .is_some_and(|g| ctx.ast.kind(g) == NodeKind::ExtensionTypeDeclaration)
            }) {
                return;
            }
            (
                n.left_parenthesis,
                n.right_parenthesis,
                last.raw(),
                n.right_delimiter.unwrap_or(n.right_parenthesis),
            )
        }
        NodeKind::ListLiteral => {
            let n = &ctx.ast[Id::<ListLiteral>::from_raw(node)];
            let Some(last) = ctx.ast.list(n.elements).last() else {
                return;
            };
            (n.left_bracket, n.right_bracket, last.raw(), n.right_bracket)
        }
        NodeKind::SetOrMapLiteral => {
            let n = &ctx.ast[Id::<SetOrMapLiteral>::from_raw(node)];
            let Some(last) = ctx.ast.list(n.elements).last() else {
                return;
            };
            (n.left_bracket, n.right_bracket, last.raw(), n.right_bracket)
        }
        _ => return,
    };
    let after_last = ctx.ast.tokens.next(ctx.ast.end_token(last));
    if after_last.is_some() && ctx.ast.tokens.lexeme(after_last) == "," {
        return;
    }
    if ctx.parsed.line_info.on_same_line(
        ctx.ast.tokens.offset(opening),
        ctx.ast.tokens.get(closing).end(),
    ) {
        return;
    }
    if trailing_comma_exception(ctx, last) {
        return;
    }
    ctx.report_token(out, &diag::REQUIRE_TRAILING_COMMAS, error, &[]);
}

fn trailing_comma_exception(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    if let Some(parameter) = ctx.ast.cast::<FormalParameter>(node) {
        let named = match ctx.ast.kind(parameter) {
            NodeKind::RegularFormalParameter => ctx.ast
                [Id::<RegularFormalParameter>::from_raw(node)]
            .kind
            .is_named(),
            NodeKind::FieldFormalParameter => ctx.ast[Id::<FieldFormalParameter>::from_raw(node)]
                .kind
                .is_named(),
            NodeKind::SuperFormalParameter => ctx.ast[Id::<SuperFormalParameter>::from_raw(node)]
                .kind
                .is_named(),
            _ => false,
        };
        if named {
            return false;
        }
    }
    if ctx.parsed.line_info.on_same_line(
        ctx.ast.tokens.offset(ctx.ast.begin_token(node)),
        ctx.ast.tokens.get(ctx.ast.end_token(node)).end(),
    ) {
        return false;
    }
    if let Some(function) = ctx.ast.cast::<FunctionExpression>(node)
        && ctx.ast.kind(ctx.ast[function].body) == NodeKind::BlockFunctionBody
    {
        return true;
    }
    if StringLiteral::test(ctx.ast.kind(node)) {
        return true;
    }
    if let Some(invocation) = ctx.ast.cast::<FunctionExpressionInvocation>(node) {
        let n = &ctx.ast[invocation];
        if ctx.ast.kind(n.function) == NodeKind::FunctionExpression {
            let args = &ctx.ast[n.argument_list];
            if ctx.parsed.line_info.on_same_line(
                ctx.ast.tokens.offset(args.left_parenthesis),
                ctx.ast.tokens.get(args.right_parenthesis).end(),
            ) {
                return true;
            }
        }
    }
    matches!(
        ctx.ast.kind(node),
        NodeKind::SetOrMapLiteral | NodeKind::ListLiteral
    )
}

fn simple_directive_paths(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let uri = match ctx.ast.kind(node) {
        NodeKind::Configuration => Some(ctx.ast[Id::<Configuration>::from_raw(node)].uri),
        NodeKind::ExportDirective => Some(ctx.ast[Id::<ExportDirective>::from_raw(node)].uri),
        NodeKind::ImportDirective => Some(ctx.ast[Id::<ImportDirective>::from_raw(node)].uri),
        NodeKind::PartDirective => Some(ctx.ast[Id::<PartDirective>::from_raw(node)].uri),
        NodeKind::PartOfDirective => ctx.ast[Id::<PartOfDirective>::from_raw(node)].uri,
        _ => None,
    };
    let Some(uri) = uri else {
        return;
    };
    let Some(simple) = ctx.ast.cast::<SimpleStringLiteral>(uri) else {
        return;
    };
    let value = ctx.ast[simple].value.as_ref();
    if value.is_empty() {
        return;
    }
    if !uri_is_simple(value, &ctx.source_uri()) {
        ctx.report_node(out, &diag::SIMPLE_DIRECTIVE_PATHS, uri, &[]);
    }
}

fn uri_is_simple(uri: &str, source_path: &str) -> bool {
    let bytes = uri.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'%' && bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit()
        {
            let hex = |b: u8| {
                if b.is_ascii_digit() {
                    b - b'0'
                } else {
                    b.to_ascii_lowercase() - b'a' + 10
                }
            };
            let decoded = hex(bytes[i + 1]) * 16 + hex(bytes[i + 2]);
            if decoded.is_ascii_alphanumeric() || matches!(decoded, b'-' | b'.' | b'_' | b'~') {
                return false;
            }
            if bytes[i + 1].is_ascii_lowercase() || bytes[i + 2].is_ascii_lowercase() {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    let has_scheme = uri.split('/').next().is_some_and(|head| head.contains(':'));
    let has_authority = !has_scheme && uri.starts_with("//");
    if !has_scheme && !has_authority && (uri.contains('?') || uri.contains('#')) {
        return false;
    }
    let path = uri.split_once(':').map_or(uri, |(_, rest)| rest);
    if has_authority {
        return true;
    }
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let absolute = path.starts_with('/');
    if !has_scheme && !absolute && !source_path.is_empty() {
        let package_uri = source_path.starts_with("package:");
        let source_path = source_path
            .strip_prefix("package:")
            .or_else(|| source_path.strip_prefix("file://"))
            .unwrap_or(source_path);
        // Dart package URI resolution never traverses above the package name.
        let directory_depth = source_path
            .split('/')
            .filter(|part| !part.is_empty())
            .count()
            .saturating_sub(if package_uri { 2 } else { 1 });
        let leading_parents = path
            .split('/')
            .take_while(|segment| *segment == "..")
            .count();
        if leading_parents > directory_depth {
            return false;
        }
    }
    let mut depth = 0usize;
    for segment in path.split('/') {
        match segment {
            "." => return false,
            ".." if absolute || depth > 0 => return false,
            ".." => {}
            "" if !absolute => {}
            "" => {}
            _ => depth += 1,
        }
    }
    !path.contains("//")
}

fn slash_for_doc_comments(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let comment = &ctx.ast[Id::<Comment>::from_raw(node)];
    if let Some(&token) = ctx.ast.token_list(comment.tokens).first()
        && ctx.ast.tokens.lexeme(token).starts_with("/**")
    {
        ctx.report_node(out, &diag::SLASH_FOR_DOC_COMMENTS, node, &[]);
    }
}

fn constructor_error_range(
    ctx: &LinterContext<'_>,
    id: Id<ConstructorDeclaration>,
    out: &mut Vec<Diagnostic>,
    code: &'static DiagnosticCode,
) {
    let n = &ctx.ast[id];
    let start = n
        .type_name
        .map(|x| ctx.ast.offset(x))
        .or_else(|| n.new_keyword.map(|x| ctx.ast.tokens.offset(x)))
        .or_else(|| n.factory_keyword.map(|x| ctx.ast.tokens.offset(x)));
    if let Some(start) = start {
        let end = n
            .name
            .map(|x| ctx.ast.tokens.get(x).end())
            .unwrap_or_else(|| {
                n.type_name
                    .map(|x| ctx.ast.end(x))
                    .or_else(|| n.new_keyword.map(|x| ctx.ast.tokens.get(x).end()))
                    .or_else(|| n.factory_keyword.map(|x| ctx.ast.tokens.get(x).end()))
                    .unwrap_or(start)
            });
        ctx.report_offset(out, code, start as usize, (end - start) as usize, &[]);
    }
}

fn check_constructor_members(
    ctx: &LinterContext<'_>,
    members: NodeList<ClassMember>,
    out: &mut Vec<Diagnostic>,
) {
    let mut saw_other = false;
    for &member in ctx.ast.list(members) {
        match ctx.ast.kind(member) {
            NodeKind::ConstructorDeclaration => {
                if saw_other {
                    constructor_error_range(
                        ctx,
                        Id::from_raw(member.raw()),
                        out,
                        &diag::SORT_CONSTRUCTORS_FIRST,
                    );
                }
            }
            NodeKind::PrimaryConstructorBody => {
                if saw_other {
                    ctx.report_token(
                        out,
                        &diag::SORT_CONSTRUCTORS_FIRST,
                        ctx.ast[Id::<PrimaryConstructorBody>::from_raw(member.raw())].this_keyword,
                        &[],
                    );
                }
            }
            _ => saw_other = true,
        }
    }
}

fn sort_constructors_first(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let body = match ctx.ast.kind(node) {
        NodeKind::ClassDeclaration => ctx.ast[Id::<ClassDeclaration>::from_raw(node)].body.raw(),
        NodeKind::EnumDeclaration => ctx.ast[Id::<EnumDeclaration>::from_raw(node)].body.raw(),
        NodeKind::ExtensionTypeDeclaration => ctx.ast
            [Id::<ExtensionTypeDeclaration>::from_raw(node)]
        .body
        .raw(),
        _ => return,
    };
    match ctx.ast.kind(body) {
        NodeKind::BlockClassBody => check_constructor_members(
            ctx,
            ctx.ast[Id::<BlockClassBody>::from_raw(body)].members,
            out,
        ),
        NodeKind::BlockEnumBody => check_constructor_members(
            ctx,
            ctx.ast[Id::<BlockEnumBody>::from_raw(body)].members,
            out,
        ),
        _ => {}
    }
}

fn test_types_in_equals(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<AsExpression>::from_raw(node)];
    let Some(expression) = ctx.ast.cast::<SimpleIdentifier>(n.expression) else {
        return;
    };
    let Some(method) = ctx.ast.this_or_ancestor_of_type::<MethodDeclaration>(node) else {
        return;
    };
    let m = &ctx.ast[method];
    if m.operator_keyword.is_none() || ctx.ast.tokens.lexeme(m.name) != "==" {
        return;
    }
    let Some(parameters) = m.parameters else {
        return;
    };
    let params = ctx.ast.list(ctx.ast[parameters].parameters);
    if params.len() != 1 {
        return;
    }
    let parameter_name = formal_parameter_name(ctx.ast, params[0]);
    if parameter_name.map(|t| ctx.ast.tokens.lexeme(t))
        != Some(ctx.ast.tokens.lexeme(ctx.ast[expression].token))
    {
        return;
    }
    ctx.report_node(out, &diag::TEST_TYPES_IN_EQUALS, node, &["unknown"]);
}

fn formal_parameter_name(ast: &Ast, node: Id<FormalParameter>) -> Option<dartr_syntax::TokenId> {
    match ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            ast[Id::<RegularFormalParameter>::from_raw(node.raw())].name
        }
        NodeKind::FieldFormalParameter => {
            Some(ast[Id::<FieldFormalParameter>::from_raw(node.raw())].name)
        }
        NodeKind::SuperFormalParameter => {
            Some(ast[Id::<SuperFormalParameter>::from_raw(node.raw())].name)
        }
        _ => None,
    }
}

// Dart `ControlFlowInFinallyBlockReporter.reportIfFinallyAncestorExists`:
// only the nearest `TryStatement` is checked.
fn throw_in_finally(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut current = Some(node);
    let mut try_statement = None;
    while let Some(n) = current {
        if let Some(t) = ctx.ast.cast::<TryStatement>(n) {
            try_statement = Some(t);
            break;
        }
        current = ctx.ast.parent(n);
    }
    let Some(finally_block) = try_statement.and_then(|t| ctx.ast[t].finally_block) else {
        return;
    };
    let finally_raw = finally_block.raw();
    let in_finally = |n: NodeId| {
        ctx.ast
            .this_or_ancestor_matching(n, |_, m| m == finally_raw)
            .is_some()
    };
    if !in_finally(node) {
        return;
    }
    // A function body inside the `finally` block that contains the node
    // enables the `throw`.
    let mut current = Some(node);
    while let Some(n) = current {
        if FunctionBody::test(ctx.ast.kind(n)) && in_finally(n) {
            return;
        }
        current = ctx.ast.parent(n);
    }
    ctx.report_node(out, &diag::THROW_IN_FINALLY, node, &["throw"]);
}

const VALID_HTML_TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "bdi",
    "bdo",
    "blockquote",
    "br",
    "button",
    "canvas",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "keygen",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "meta",
    "meter",
    "nav",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "param",
    "pre",
    "progress",
    "q",
    "s",
    "samp",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strong",
    "style",
    "sub",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];

fn unintended_html_in_doc_comment(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let comment = &ctx.ast[Id::<Comment>::from_raw(node)];
    for &token in ctx.ast.token_list(comment.tokens) {
        let offset_after_slash = ctx.ast.tokens.offset(token) + 3;
        if comment
            .code_blocks
            .iter()
            .flat_map(|b| b.lines.iter())
            .any(|line| {
                line.offset <= offset_after_slash && offset_after_slash <= line.offset + line.length
            })
        {
            continue;
        }
        let text = ctx.ast.tokens.lexeme(token);
        for (start, end) in unintended_tags(text) {
            let utf16_start = text[..start].encode_utf16().count();
            let utf16_len = text[start..end].encode_utf16().count();
            ctx.report_offset(
                out,
                &diag::UNINTENDED_HTML_IN_DOC_COMMENT,
                ctx.ast.tokens.offset(token) as usize + utf16_start,
                utf16_len,
                &[],
            );
        }
    }
}

fn unintended_tags(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i] == b'`' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            let fence = &text[start..i];
            if let Some(end) = text[i..].find(fence) {
                i += end + fence.len();
            }
            continue;
        }
        if bytes[i] == b'['
            && let Some(end) = text[i + 1..].find(']')
        {
            i += end + 2;
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if text[i..].starts_with("<!--") {
            i = text[i + 4..]
                .find("-->")
                .map_or(bytes.len(), |n| i + 4 + n + 3);
            continue;
        }
        if text[i..].starts_with("<?") {
            i = text[i + 2..]
                .find("?>")
                .map_or(bytes.len(), |n| i + 2 + n + 2);
            continue;
        }
        if text[i..].starts_with("<!") || text[i..].starts_with("<[CDATA") {
            i = text[i + 2..]
                .find('>')
                .map_or(bytes.len(), |n| i + 2 + n + 1);
            continue;
        }
        let Some(rel_end) = text[i + 1..].find('>') else {
            break;
        };
        let end = i + 1 + rel_end + 1;
        let inside = &text[i + 1..end - 1];
        if !inside.bytes().any(|b| b.is_ascii_whitespace()) && inside.contains(':') {
            i = end;
            continue;
        }
        let name_start = usize::from(inside.starts_with('/'));
        let name_end = inside[name_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric()))
            .map_or(inside.len(), |n| name_start + n);
        if name_end == name_start || !inside.as_bytes()[name_start].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let name = inside[name_start..name_end].to_ascii_lowercase();
        let suffix = &inside[name_end..];
        let valid_ending = suffix.is_empty()
            || suffix.starts_with(|c: char| c.is_ascii_whitespace())
            || (name_start == 0 && suffix == "/");
        if !VALID_HTML_TAGS.contains(&name.as_str()) || !valid_ending {
            out.push((i, end));
        }
        i = end;
    }
    out
}

fn unnecessary_breaks(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<BreakStatement>::from_raw(node)];
    if n.label.is_some() {
        return;
    }
    let Some(parent) = ctx.ast.parent(node) else {
        return;
    };
    let statements = match ctx.ast.kind(parent) {
        NodeKind::SwitchCase => ctx.ast[Id::<SwitchCase>::from_raw(parent)].statements,
        NodeKind::SwitchDefault => ctx.ast[Id::<SwitchDefault>::from_raw(parent)].statements,
        NodeKind::SwitchPatternCase => {
            ctx.ast[Id::<SwitchPatternCase>::from_raw(parent)].statements
        }
        _ => return,
    };
    let statements = ctx.ast.list(statements);
    if statements.len() > 1 && statements.last().is_some_and(|x| x.raw() == node) {
        ctx.report_node(out, &diag::UNNECESSARY_BREAKS, node, &[]);
    }
}

fn unnecessary_brace_in_string_interps(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    for &element in ctx
        .ast
        .list(ctx.ast[Id::<StringInterpolation>::from_raw(node)].elements)
    {
        let Some(interpolation) = ctx.ast.cast::<InterpolationExpression>(element) else {
            continue;
        };
        let n = &ctx.ast[interpolation];
        let eligible = match ctx.ast.kind(n.expression) {
            NodeKind::SimpleIdentifier => !ctx
                .ast
                .tokens
                .lexeme(ctx.ast[Id::<SimpleIdentifier>::from_raw(n.expression.raw())].token)
                .contains('$'),
            NodeKind::ThisExpression => true,
            NodeKind::TypeLiteral => {
                let ty = &ctx.ast[Id::<TypeLiteral>::from_raw(n.expression.raw())];
                let named = &ctx.ast[ty.type_];
                named.type_arguments.is_none()
                    && named.import_prefix.is_none()
                    && named.question.is_none()
                    && !ctx.ast.tokens.lexeme(named.name).contains('$')
            }
            _ => false,
        };
        if !eligible {
            continue;
        }
        let Some(right) = n.right_bracket else {
            continue;
        };
        let next = ctx.ast.tokens.next(right);
        let next_lexeme = if next.is_some() {
            ctx.ast.tokens.lexeme(next)
        } else {
            ""
        };
        if !next_lexeme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            ctx.report_node(
                out,
                &diag::UNNECESSARY_BRACE_IN_STRING_INTERPS,
                interpolation,
                &[],
            );
        }
    }
}

fn in_constant_context(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let mut current = ctx.ast.parent(node);
    while let Some(n) = current {
        match ctx.ast.kind(n) {
            NodeKind::Annotation | NodeKind::EnumConstantArguments | NodeKind::SwitchCase => {
                return true;
            }
            NodeKind::ConstantPattern => {
                return ctx.ast[Id::<ConstantPattern>::from_raw(n)]
                    .const_keyword
                    .is_some();
            }
            NodeKind::DotShorthandConstructorInvocation => {
                if ctx.ast[Id::<DotShorthandConstructorInvocation>::from_raw(n)]
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::InstanceCreationExpression => {
                if ctx.ast[Id::<InstanceCreationExpression>::from_raw(n)]
                    .keyword
                    .is_some_and(|t| ctx.ast.tokens.lexeme(t) == "const")
                {
                    return true;
                }
            }
            NodeKind::RecordLiteral => {
                if ctx.ast[Id::<RecordLiteral>::from_raw(n)]
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::ListLiteral => {
                if ctx.ast[Id::<ListLiteral>::from_raw(n)]
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::SetOrMapLiteral => {
                if ctx.ast[Id::<SetOrMapLiteral>::from_raw(n)]
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::VariableDeclarationList => {
                return ctx.ast[Id::<VariableDeclarationList>::from_raw(n)]
                    .keyword
                    .is_some_and(|t| ctx.ast.tokens.lexeme(t) == "const");
            }
            NodeKind::ArgumentList
            | NodeKind::ForElement
            | NodeKind::IfElement
            | NodeKind::InterpolationExpression
            | NodeKind::MapLiteralEntry
            | NodeKind::NamedArgument
            | NodeKind::RecordLiteralNamedField
            | NodeKind::NullAwareElement
            | NodeKind::SpreadElement
            | NodeKind::VariableDeclaration => {}
            kind if Expression::test(kind) => {
                if kind == NodeKind::ThrowExpression || kind == NodeKind::FunctionExpression {
                    return false;
                }
            }
            _ => return false,
        }
        current = ctx.ast.parent(n);
    }
    false
}

fn unnecessary_const(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let token = match ctx.ast.kind(node) {
        NodeKind::DotShorthandConstructorInvocation => {
            ctx.ast[Id::<DotShorthandConstructorInvocation>::from_raw(node)].const_keyword
        }
        NodeKind::InstanceCreationExpression => ctx.ast
            [Id::<InstanceCreationExpression>::from_raw(node)]
        .keyword
        .filter(|t| ctx.ast.tokens.lexeme(*t) == "const"),
        NodeKind::RecordLiteral => ctx.ast[Id::<RecordLiteral>::from_raw(node)].const_keyword,
        NodeKind::ListLiteral => {
            if ctx
                .ast
                .parent(node)
                .is_some_and(|p| ctx.ast.kind(p) == NodeKind::ConstantPattern)
            {
                return;
            }
            ctx.ast[Id::<ListLiteral>::from_raw(node)].const_keyword
        }
        NodeKind::SetOrMapLiteral => {
            if ctx
                .ast
                .parent(node)
                .is_some_and(|p| ctx.ast.kind(p) == NodeKind::ConstantPattern)
            {
                return;
            }
            ctx.ast[Id::<SetOrMapLiteral>::from_raw(node)].const_keyword
        }
        _ => None,
    };
    if let Some(token) = token
        && in_constant_context(ctx, node)
    {
        ctx.report_token(out, &diag::UNNECESSARY_CONST, token, &[]);
    }
}

fn unnecessary_const_in_enum_constructor(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    match ctx.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if ctx
                .ast
                .parent(node)
                .is_some_and(|p| ctx.ast.kind(p) == NodeKind::BlockEnumBody)
                && let Some(t) = n.const_keyword
            {
                ctx.report_token(out, &diag::UNNECESSARY_CONST_IN_ENUM_CONSTRUCTOR, t, &[]);
            }
        }
        NodeKind::PrimaryConstructorDeclaration => {
            let n = &ctx.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            if ctx
                .ast
                .parent(node)
                .is_some_and(|p| ctx.ast.kind(p) == NodeKind::EnumDeclaration)
                && let Some(t) = n.const_keyword
            {
                ctx.report_token(out, &diag::UNNECESSARY_CONST_IN_ENUM_CONSTRUCTOR, t, &[]);
            }
        }
        _ => {}
    }
}

fn report_final(
    ctx: &LinterContext<'_>,
    out: &mut Vec<Diagnostic>,
    token: dartr_syntax::TokenId,
    has_type: bool,
) {
    if ctx.ast.tokens.lexeme(token) == "final" {
        ctx.report_token(
            out,
            if has_type {
                &diag::UNNECESSARY_FINAL_WITH_TYPE
            } else {
                &diag::UNNECESSARY_FINAL_WITHOUT_TYPE
            },
            token,
            &[],
        );
    }
}

fn unnecessary_final(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::FormalParameterList => {
            for &p in ctx
                .ast
                .list(ctx.ast[Id::<FormalParameterList>::from_raw(node)].parameters)
            {
                match ctx.ast.kind(p) {
                    NodeKind::RegularFormalParameter => {
                        let p = &ctx.ast[Id::<RegularFormalParameter>::from_raw(p.raw())];
                        if let Some(t) = p.const_final_or_var_keyword {
                            report_final(ctx, out, t, p.type_.is_some());
                        }
                    }
                    NodeKind::FieldFormalParameter => {
                        let p = &ctx.ast[Id::<FieldFormalParameter>::from_raw(p.raw())];
                        if let Some(t) = p.const_final_or_var_keyword {
                            report_final(ctx, out, t, p.type_.is_some());
                        }
                    }
                    NodeKind::SuperFormalParameter => {
                        let p = &ctx.ast[Id::<SuperFormalParameter>::from_raw(p.raw())];
                        if let Some(t) = p.const_final_or_var_keyword {
                            report_final(ctx, out, t, p.type_.is_some());
                        }
                    }
                    _ => {}
                }
            }
        }
        NodeKind::VariableDeclarationStatement => {
            let v = &ctx.ast[ctx.ast[Id::<VariableDeclarationStatement>::from_raw(node)].variables];
            if let Some(t) = v.keyword {
                report_final(ctx, out, t, v.type_.is_some());
            }
        }
        NodeKind::ForStatement => {
            let parts = ctx.ast[Id::<ForStatement>::from_raw(node)].for_loop_parts;
            match ctx.ast.kind(parts.raw()) {
                NodeKind::ForEachPartsWithDeclaration => {
                    let n = &ctx.ast[Id::<ForEachPartsWithDeclaration>::from_raw(parts.raw())];
                    let v = &ctx.ast[n.loop_variable];
                    if let Some(t) = v.keyword {
                        report_final(ctx, out, t, v.type_.is_some());
                    }
                }
                NodeKind::ForEachPartsWithPattern => {
                    let n = &ctx.ast[Id::<ForEachPartsWithPattern>::from_raw(parts.raw())];
                    report_final(ctx, out, n.keyword, false);
                }
                _ => {}
            }
        }
        NodeKind::DeclaredVariablePattern => {
            let n = &ctx.ast[Id::<DeclaredVariablePattern>::from_raw(node)];
            let keyword = n.keyword.or_else(|| {
                ctx.ast
                    .this_or_ancestor_of_type::<PatternVariableDeclaration>(node)
                    .map(|d| ctx.ast[d].keyword)
            });
            // Dart uses `node.matchedValueType`, which the resolver always sets
            // for a declared variable pattern, so the code is the "with type" one.
            if let Some(t) = keyword {
                report_final(ctx, out, t, true);
            }
        }
        _ => {}
    }
}

fn unnecessary_late(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let variables = match ctx.ast.kind(node) {
        NodeKind::FieldDeclaration => {
            let n = &ctx.ast[Id::<FieldDeclaration>::from_raw(node)];
            if n.static_keyword.is_none() {
                return;
            }
            n.fields
        }
        NodeKind::TopLevelVariableDeclaration => {
            ctx.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables
        }
        _ => return,
    };
    let list = &ctx.ast[variables];
    let Some(late) = list.late_keyword else {
        return;
    };
    if ctx
        .ast
        .list(list.variables)
        .iter()
        .all(|v| ctx.ast[*v].initializer.is_some())
    {
        ctx.report_token(out, &diag::UNNECESSARY_LATE, late, &[]);
    }
}

fn unnecessary_library_directive(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<LibraryDirective>::from_raw(node)];
    let Some(unit) = ctx
        .ast
        .parent(node)
        .and_then(|p| ctx.ast.cast::<CompilationUnit>(p))
    else {
        return;
    };
    if ctx
        .ast
        .list(ctx.ast[unit].directives)
        .iter()
        .any(|d| ctx.ast.kind(*d) == NodeKind::PartDirective)
    {
        return;
    }
    if n.documentation_comment.is_none() && n.metadata.is_empty() {
        ctx.report_node(out, &diag::UNNECESSARY_LIBRARY_DIRECTIVE, node, &[]);
    }
}

fn unnecessary_library_name(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(name) = ctx.ast[Id::<LibraryDirective>::from_raw(node)].name {
        ctx.report_node(out, &diag::UNNECESSARY_LIBRARY_NAME, name, &[]);
    }
}

fn unnecessary_new(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(t) = ctx.ast[Id::<InstanceCreationExpression>::from_raw(node)].keyword
        && ctx.ast.tokens.lexeme(t) == "new"
    {
        ctx.report_token(out, &diag::UNNECESSARY_NEW, t, &[]);
    }
}

fn unnecessary_null_in_if_null_operators(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let n = &ctx.ast[Id::<BinaryExpression>::from_raw(node)];
    if ctx.ast.tokens.lexeme(n.operator) != "??" {
        return;
    }
    if ctx.ast.kind(n.right_operand) == NodeKind::NullLiteral {
        ctx.report_node(
            out,
            &diag::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS,
            n.right_operand,
            &[],
        );
    } else if ctx.ast.kind(n.left_operand) == NodeKind::NullLiteral {
        ctx.report_node(
            out,
            &diag::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS,
            n.left_operand,
            &[],
        );
    }
}

fn unnecessary_primary_constructor_body(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let n = &ctx.ast[Id::<PrimaryConstructorBody>::from_raw(node)];
    if n.documentation_comment.is_some() || !n.metadata.is_empty() || !n.initializers.is_empty() {
        return;
    }
    let empty = match ctx.ast.kind(n.body) {
        NodeKind::EmptyFunctionBody => true,
        NodeKind::BlockFunctionBody => ctx.ast
            [ctx.ast[Id::<BlockFunctionBody>::from_raw(n.body.raw())].block]
            .statements
            .is_empty(),
        _ => false,
    };
    if empty {
        ctx.report_token(
            out,
            &diag::UNNECESSARY_PRIMARY_CONSTRUCTOR_BODY,
            n.this_keyword,
            &[],
        );
    }
}

fn unnecessary_raw_strings(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let lexeme = ctx
        .ast
        .tokens
        .lexeme(ctx.ast[Id::<SimpleStringLiteral>::from_raw(node)].literal);
    if (lexeme.starts_with('r') || lexeme.starts_with('R'))
        && !lexeme.contains('\\')
        && !lexeme.contains('$')
    {
        ctx.report_node(out, &diag::UNNECESSARY_RAW_STRINGS, node, &[]);
    }
}

fn string_content(lexeme: &str) -> Option<(usize, usize, bool, bool)> {
    let raw = lexeme.starts_with('r') || lexeme.starts_with('R');
    let start = usize::from(raw);
    let quote = lexeme.as_bytes().get(start).copied()?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let multiline = lexeme
        .as_bytes()
        .get(start..start + 3)
        .is_some_and(|x| x.iter().all(|q| *q == quote));
    let width = if multiline { 3 } else { 1 };
    Some((
        start + width,
        lexeme.len().saturating_sub(width),
        quote == b'\'',
        multiline,
    ))
}

fn check_string_escapes(
    ctx: &LinterContext<'_>,
    out: &mut Vec<Diagnostic>,
    token: dartr_syntax::TokenId,
    content_start: usize,
    content_end: usize,
    single: bool,
    multiline: bool,
) {
    let lexeme = ctx.ast.tokens.lexeme(token);
    let bytes = lexeme.as_bytes();
    let mut i = content_start;
    let mut pending_quotes: Vec<(usize, bool)> = Vec::new();
    let flush_quotes = |pending: &mut Vec<(usize, bool)>, out: &mut Vec<Diagnostic>| {
        if multiline && pending.len() < 3 {
            for &(index, escaped) in pending.iter() {
                if escaped {
                    if content_end != lexeme.len() && index + 2 == content_end {
                        continue;
                    }
                    let utf16 = lexeme[..index].encode_utf16().count();
                    ctx.report_offset(
                        out,
                        &diag::UNNECESSARY_STRING_ESCAPES,
                        ctx.ast.tokens.offset(token) as usize + utf16,
                        1,
                        &[],
                    );
                }
            }
        }
        pending.clear();
    };
    while i + 1 < content_end && i + 1 < bytes.len() {
        let escape_index = i;
        let mut escaped_flag = false;
        if bytes[i] == b'\\' {
            escaped_flag = true;
            let escaped = bytes[i + 1] as char;
            let allowed = matches!(
                escaped,
                '"' | '\'' | '$' | '\\' | 'n' | 'r' | 'f' | 'b' | 't' | 'v' | 'x' | 'u'
            );
            if (single && escaped == '"') || (!single && escaped == '\'') || !allowed {
                let utf16 = lexeme[..i].encode_utf16().count();
                ctx.report_offset(
                    out,
                    &diag::UNNECESSARY_STRING_ESCAPES,
                    ctx.ast.tokens.offset(token) as usize + utf16,
                    1,
                    &[],
                );
            }
            i += 2;
        } else {
            i += 1;
        }
        let current = bytes[i - 1] as char;
        if current == if single { '\'' } else { '"' } {
            pending_quotes.push((escape_index, escaped_flag));
        } else {
            flush_quotes(&mut pending_quotes, out);
        }
    }
    flush_quotes(&mut pending_quotes, out);
}

fn unnecessary_string_escapes(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::SimpleStringLiteral => {
            let t = ctx.ast[Id::<SimpleStringLiteral>::from_raw(node)].literal;
            let lexeme = ctx.ast.tokens.lexeme(t);
            if lexeme.starts_with('r') || lexeme.starts_with('R') {
                return;
            }
            if let Some((start, end, single, multiline)) = string_content(lexeme) {
                check_string_escapes(ctx, out, t, start, end, single, multiline);
            }
        }
        NodeKind::StringInterpolation => {
            let elements = ctx
                .ast
                .list(ctx.ast[Id::<StringInterpolation>::from_raw(node)].elements);
            let parts: Vec<_> = elements
                .iter()
                .filter_map(|e| ctx.ast.cast::<InterpolationString>(*e))
                .collect();
            let Some(first) = parts.first() else {
                return;
            };
            let first_token = ctx.ast[*first].contents;
            let first_lexeme = ctx.ast.tokens.lexeme(first_token);
            if first_lexeme.starts_with('r') || first_lexeme.starts_with('R') {
                return;
            }
            let prefix = first_lexeme.trim_start_matches(['r', 'R']);
            let single = prefix.starts_with('\'');
            let multiline = prefix.starts_with("'''") || prefix.starts_with("\"\"\"");
            let first_start =
                usize::from(first_lexeme.starts_with('r') || first_lexeme.starts_with('R'))
                    + if multiline { 3 } else { 1 };
            for (index, &part) in parts.iter().enumerate() {
                let token = ctx.ast[part].contents;
                let lexeme = ctx.ast.tokens.lexeme(token);
                let start = if index == 0 {
                    first_start.min(lexeme.len())
                } else {
                    0
                };
                let end = if index + 1 == parts.len() {
                    strip_closing_quote(lexeme, multiline)
                } else {
                    lexeme.len()
                };
                check_string_escapes(ctx, out, token, start.min(end), end, single, multiline);
            }
        }
        _ => {}
    }
}

fn strip_closing_quote(lexeme: &str, multiline: bool) -> usize {
    if multiline {
        lexeme.len().saturating_sub(3)
    } else {
        lexeme.len().saturating_sub(1)
    }
}

fn unnecessary_type_name_in_constructor(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(type_name) = ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)].type_name {
        ctx.report_node(
            out,
            &diag::UNNECESSARY_TYPE_NAME_IN_CONSTRUCTOR,
            type_name,
            &[],
        );
    }
}

fn use_function_type_syntax_for_parameters(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let (suffix, name) = match ctx.ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            let n = &ctx.ast[Id::<RegularFormalParameter>::from_raw(node)];
            (n.function_typed_suffix, n.name)
        }
        NodeKind::FieldFormalParameter => {
            let n = &ctx.ast[Id::<FieldFormalParameter>::from_raw(node)];
            (n.function_typed_suffix, Some(n.name))
        }
        NodeKind::SuperFormalParameter => {
            let n = &ctx.ast[Id::<SuperFormalParameter>::from_raw(node)];
            (n.function_typed_suffix, Some(n.name))
        }
        _ => return,
    };
    if suffix.is_some() {
        let name = name.map(|t| ctx.ast.tokens.lexeme(t)).unwrap_or("");
        ctx.report_node(
            out,
            &diag::USE_FUNCTION_TYPE_SYNTAX_FOR_PARAMETERS,
            node,
            &[name],
        );
    }
}

fn use_raw_strings(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let token = ctx.ast[Id::<SimpleStringLiteral>::from_raw(node)].literal;
    let lexeme = ctx.ast.tokens.lexeme(token);
    if lexeme.starts_with('r') || lexeme.starts_with('R') {
        return;
    }
    let Some((start, end, _, _)) = string_content(lexeme) else {
        return;
    };
    let bytes = lexeme.as_bytes();
    let mut has_escape = false;
    let mut i = start;
    while i + 1 < end {
        if bytes[i] == b'\\' {
            has_escape = true;
            if !matches!(bytes[i + 1], b'\\' | b'$') {
                return;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    if has_escape {
        ctx.report_node(out, &diag::USE_RAW_STRINGS, node, &[]);
    }
}

fn use_string_in_part_of_directives(
    ctx: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    if ctx.ast[Id::<PartOfDirective>::from_raw(node)]
        .library_name
        .is_some()
    {
        ctx.report_node(out, &diag::USE_STRING_IN_PART_OF_DIRECTIVES, node, &[]);
    }
}

fn var_with_no_type_annotation(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    for &p in ctx
        .ast
        .list(ctx.ast[Id::<FormalParameterList>::from_raw(node)].parameters)
    {
        let pair = match ctx.ast.kind(p) {
            NodeKind::RegularFormalParameter => {
                let n = &ctx.ast[Id::<RegularFormalParameter>::from_raw(p.raw())];
                if n.function_typed_suffix.is_some() {
                    continue;
                }
                (n.const_final_or_var_keyword, n.type_)
            }
            NodeKind::FieldFormalParameter => {
                let n = &ctx.ast[Id::<FieldFormalParameter>::from_raw(p.raw())];
                (n.const_final_or_var_keyword, n.type_)
            }
            _ => continue,
        };
        if pair.1.is_none()
            && let Some(t) = pair.0
            && ctx.ast.tokens.lexeme(t) == "var"
        {
            ctx.report_token(out, &diag::VAR_WITH_NO_TYPE_ANNOTATION, t, &[]);
        }
    }
}
