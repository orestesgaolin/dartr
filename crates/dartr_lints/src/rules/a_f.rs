// Dart sources: pkg/linter/lib/src/rules/{a,b,c,d,e,f}*.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_syntax::{TokenId, TokenType};

pub const RULES: &[&str] = &[
    "always_declare_return_types",
    "always_put_control_body_on_new_line",
    "always_use_package_imports",
    "avoid_annotating_with_dynamic",
    "avoid_empty_else",
    "avoid_escaping_inner_quotes",
    "avoid_final_parameters",
    "avoid_js_rounded_ints",
    "avoid_multiple_declarations_per_line",
    "avoid_relative_lib_imports",
    "avoid_return_types_on_setters",
    "avoid_private_typedef_functions",
    "avoid_shadowing_type_parameters",
    "avoid_single_cascade_in_expression_statements",
    "camel_case_extensions",
    "camel_case_types",
    "combinators_ordering",
    "constant_identifier_names",
    "curly_braces_in_flow_control_structures",
    "dangling_library_doc_comments",
    "directives_ordering",
    "document_ignores",
    "empty_catches",
    "empty_constructor_bodies",
    "empty_container_bodies",
    "empty_statements",
    "eol_at_end_of_file",
    "file_names",
    "flutter_style_todos",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    let name: &'static str = match name {
        "always_declare_return_types" => "always_declare_return_types",
        "always_put_control_body_on_new_line" => "always_put_control_body_on_new_line",
        "always_use_package_imports" => "always_use_package_imports",
        "avoid_annotating_with_dynamic" => "avoid_annotating_with_dynamic",
        "avoid_empty_else" => "avoid_empty_else",
        "avoid_escaping_inner_quotes" => "avoid_escaping_inner_quotes",
        "avoid_final_parameters" => "avoid_final_parameters",
        "avoid_js_rounded_ints" => "avoid_js_rounded_ints",
        "avoid_multiple_declarations_per_line" => "avoid_multiple_declarations_per_line",
        "avoid_relative_lib_imports" => "avoid_relative_lib_imports",
        "avoid_return_types_on_setters" => "avoid_return_types_on_setters",
        "avoid_private_typedef_functions" => "avoid_private_typedef_functions",
        "avoid_shadowing_type_parameters" => "avoid_shadowing_type_parameters",
        "avoid_single_cascade_in_expression_statements" => {
            "avoid_single_cascade_in_expression_statements"
        }
        "camel_case_extensions" => "camel_case_extensions",
        "camel_case_types" => "camel_case_types",
        "combinators_ordering" => "combinators_ordering",
        "constant_identifier_names" => "constant_identifier_names",
        "curly_braces_in_flow_control_structures" => "curly_braces_in_flow_control_structures",
        "dangling_library_doc_comments" => "dangling_library_doc_comments",
        "directives_ordering" => "directives_ordering",
        "document_ignores" => "document_ignores",
        "empty_catches" => "empty_catches",
        "empty_constructor_bodies" => "empty_constructor_bodies",
        "empty_container_bodies" => "empty_container_bodies",
        "empty_statements" => "empty_statements",
        "eol_at_end_of_file" => "eol_at_end_of_file",
        "file_names" => "file_names",
        "flutter_style_todos" => "flutter_style_todos",
        _ => return false,
    };
    match name {
        "always_declare_return_types" => {
            for kind in [
                NodeKind::FunctionDeclaration,
                NodeKind::FunctionTypeAlias,
                NodeKind::MethodDeclaration,
            ] {
                registry.add(kind, name, always_declare_return_types);
            }
        }
        "always_put_control_body_on_new_line" => {
            for kind in [
                NodeKind::DoStatement,
                NodeKind::ForStatement,
                NodeKind::IfStatement,
                NodeKind::WhileStatement,
            ] {
                registry.add(kind, name, always_put_control_body_on_new_line);
            }
        }
        "always_use_package_imports" => {
            if context.is_in_lib_dir() {
                registry.add(NodeKind::ImportDirective, name, always_use_package_imports);
            }
        }
        "avoid_annotating_with_dynamic" => {
            for kind in [
                NodeKind::FieldFormalParameter,
                NodeKind::RegularFormalParameter,
                NodeKind::SuperFormalParameter,
            ] {
                registry.add(kind, name, avoid_annotating_with_dynamic);
            }
        }
        "avoid_empty_else" => registry.add(NodeKind::IfStatement, name, avoid_empty_else),
        "avoid_escaping_inner_quotes" => {
            for kind in [NodeKind::SimpleStringLiteral, NodeKind::StringInterpolation] {
                registry.add(kind, name, avoid_escaping_inner_quotes);
            }
        }
        "avoid_final_parameters" => {
            if context.parsed.language_version.effective() < (3, 13) {
                registry.add(NodeKind::FormalParameterList, name, avoid_final_parameters);
            }
        }
        "avoid_js_rounded_ints" => {
            registry.add(NodeKind::IntegerLiteral, name, avoid_js_rounded_ints)
        }
        "avoid_multiple_declarations_per_line" => registry.add(
            NodeKind::VariableDeclarationList,
            name,
            avoid_multiple_declarations_per_line,
        ),
        "avoid_relative_lib_imports" => {
            registry.add(NodeKind::ImportDirective, name, avoid_relative_lib_imports)
        }
        "avoid_return_types_on_setters" => {
            for kind in [NodeKind::FunctionDeclaration, NodeKind::MethodDeclaration] {
                registry.add(kind, name, avoid_return_types_on_setters);
            }
        }
        "avoid_private_typedef_functions" => {
            for kind in [NodeKind::FunctionTypeAlias, NodeKind::GenericTypeAlias] {
                registry.add(kind, name, avoid_private_typedef_functions);
            }
        }
        "avoid_shadowing_type_parameters" => {
            for kind in [
                NodeKind::FunctionDeclarationStatement,
                NodeKind::FunctionExpression,
                NodeKind::RegularFormalParameter,
                NodeKind::GenericFunctionType,
                NodeKind::GenericTypeAlias,
                NodeKind::MethodDeclaration,
            ] {
                registry.add(kind, name, avoid_shadowing_type_parameters);
            }
        }
        "avoid_single_cascade_in_expression_statements" => {
            registry.add(NodeKind::CascadeExpression, name, avoid_single_cascade)
        }
        "camel_case_extensions" => {
            registry.add(NodeKind::ExtensionDeclaration, name, camel_case_extensions)
        }
        "camel_case_types" => {
            for kind in [
                NodeKind::GenericTypeAlias,
                NodeKind::ClassDeclaration,
                NodeKind::ClassTypeAlias,
                NodeKind::FunctionTypeAlias,
                NodeKind::EnumDeclaration,
                NodeKind::ExtensionTypeDeclaration,
                NodeKind::MixinDeclaration,
            ] {
                registry.add(kind, name, camel_case_types);
            }
        }
        "combinators_ordering" => {
            for kind in [NodeKind::HideCombinator, NodeKind::ShowCombinator] {
                registry.add(kind, name, combinators_ordering);
            }
        }
        "constant_identifier_names" => {
            for kind in [
                NodeKind::DeclaredVariablePattern,
                NodeKind::EnumConstantDeclaration,
                NodeKind::VariableDeclarationList,
            ] {
                registry.add(kind, name, constant_identifier_names);
            }
        }
        "curly_braces_in_flow_control_structures" => {
            for kind in [
                NodeKind::DoStatement,
                NodeKind::ForStatement,
                NodeKind::IfStatement,
                NodeKind::WhileStatement,
            ] {
                registry.add(kind, name, curly_braces);
            }
        }
        "dangling_library_doc_comments" => registry.add(
            NodeKind::CompilationUnit,
            name,
            dangling_library_doc_comments,
        ),
        "directives_ordering" => registry.add(NodeKind::CompilationUnit, name, directives_ordering),
        "document_ignores" => registry.add(NodeKind::CompilationUnit, name, document_ignores),
        "empty_catches" => registry.add(NodeKind::CatchClause, name, empty_catches),
        "empty_constructor_bodies" => {
            for kind in [
                NodeKind::ConstructorDeclaration,
                NodeKind::PrimaryConstructorBody,
            ] {
                registry.add(kind, name, empty_constructor_bodies);
            }
        }
        "empty_container_bodies" => {
            if context.parsed.language_version.effective() >= (3, 13) {
                registry.add(NodeKind::BlockClassBody, name, empty_container_bodies)
            }
        }
        "empty_statements" => registry.add(NodeKind::EmptyStatement, name, empty_statements),
        "eol_at_end_of_file" => registry.add(NodeKind::CompilationUnit, name, eol_at_end_of_file),
        "file_names" => registry.add(NodeKind::CompilationUnit, name, file_names),
        "flutter_style_todos" => registry.add(NodeKind::CompilationUnit, name, flutter_style_todos),
        _ => unreachable!(),
    }
    true
}

fn lexeme<'a>(c: &'a LinterContext<'_>, token: TokenId) -> &'a str {
    c.ast.tokens.lexeme(token)
}

fn always_use_package_imports(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let directive = c.ast.get(Id::<ImportDirective>::from_raw(node));
    let Some(uri) = string_value(c, directive.uri) else {
        return;
    };
    let has_scheme = uri
        .split(['/', '\\'])
        .next()
        .is_some_and(|part| part.contains(':'));
    if !has_scheme {
        c.report_node(out, &diag::ALWAYS_USE_PACKAGE_IMPORTS, directive.uri, &[]);
    }
}

fn always_declare_return_types(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (return_type, augment, property, name, code) = match c.ast.kind(node) {
        NodeKind::FunctionDeclaration => {
            let n = c.ast.get(Id::<FunctionDeclaration>::from_raw(node));
            (
                n.return_type,
                n.augment_keyword,
                n.property_keyword,
                n.name,
                &diag::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS,
            )
        }
        NodeKind::FunctionTypeAlias => {
            let n = c.ast.get(Id::<FunctionTypeAlias>::from_raw(node));
            (
                n.return_type,
                None,
                None,
                n.name,
                &diag::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS,
            )
        }
        NodeKind::MethodDeclaration => {
            let n = c.ast.get(Id::<MethodDeclaration>::from_raw(node));
            (
                n.return_type,
                n.augment_keyword,
                n.property_keyword,
                n.name,
                &diag::ALWAYS_DECLARE_RETURN_TYPES_OF_METHODS,
            )
        }
        _ => return,
    };
    let name_text = lexeme(c, name);
    if return_type.is_some()
        || augment.is_some()
        || property.is_some_and(|t| lexeme(c, t) == "set")
        || name_text == "[]="
    {
        return;
    }
    if c.ast.kind(node) == NodeKind::MethodDeclaration
        && c.is_in_test_directory()
        && (name_text.starts_with("test_") || name_text.starts_with("solo_test_"))
    {
        return;
    }
    c.report_token(out, code, name, &[name_text]);
}

fn check_control_line(
    c: &LinterContext<'_>,
    body: Id<Statement>,
    end: u32,
    out: &mut Vec<Diagnostic>,
) {
    let offset = if let Some(block) = c.ast.cast::<Block>(body) {
        let n = c.ast.get(block);
        let Some(first) = c.ast.list_raw(n.statements).first() else {
            return;
        };
        c.ast.offset(*first)
    } else {
        c.ast.offset(body)
    };
    if c.parsed.line_info.on_same_line(end, offset) {
        c.report_token(
            out,
            &diag::ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE,
            c.ast.begin_token(body),
            &[],
        );
    }
}

fn always_put_control_body_on_new_line(
    c: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    match c.ast.kind(node) {
        NodeKind::DoStatement => {
            let n = c.ast.get(Id::<DoStatement>::from_raw(node));
            check_control_line(c, n.body, c.ast.tokens.get(n.do_keyword).end(), out);
        }
        NodeKind::ForStatement => {
            let n = c.ast.get(Id::<ForStatement>::from_raw(node));
            check_control_line(c, n.body, c.ast.tokens.get(n.right_parenthesis).end(), out);
        }
        NodeKind::WhileStatement => {
            let n = c.ast.get(Id::<WhileStatement>::from_raw(node));
            check_control_line(c, n.body, c.ast.tokens.get(n.right_parenthesis).end(), out);
        }
        NodeKind::IfStatement => {
            let n = c.ast.get(Id::<IfStatement>::from_raw(node));
            check_control_line(
                c,
                n.then_statement,
                c.ast.tokens.get(n.right_parenthesis).end(),
                out,
            );
            if let (Some(k), Some(s)) = (n.else_keyword, n.else_statement)
                && c.ast.kind(s) != NodeKind::IfStatement
            {
                check_control_line(c, s, c.ast.tokens.get(k).end(), out);
            }
        }
        _ => {}
    }
}

fn explicit_dynamic(c: &LinterContext<'_>, ty: Option<Id<TypeAnnotation>>) -> bool {
    let Some(ty) = ty.and_then(|n| c.ast.cast::<NamedType>(n)) else {
        return false;
    };
    let n = c.ast.get(ty);
    n.import_prefix.is_none()
        && n.type_arguments.is_none()
        && n.question.is_none()
        && lexeme(c, n.name) == "dynamic"
}

fn avoid_annotating_with_dynamic(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut ancestor = Some(node);
    while let Some(current) = ancestor {
        let augmentation = match c.ast.kind(current) {
            NodeKind::FunctionDeclaration => c
                .ast
                .get(Id::<FunctionDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::MethodDeclaration => c
                .ast
                .get(Id::<MethodDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::ConstructorDeclaration => c
                .ast
                .get(Id::<ConstructorDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::ClassDeclaration => c
                .ast
                .get(Id::<ClassDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::EnumDeclaration => c
                .ast
                .get(Id::<EnumDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::ExtensionDeclaration => c
                .ast
                .get(Id::<ExtensionDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::ExtensionTypeDeclaration => c
                .ast
                .get(Id::<ExtensionTypeDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            NodeKind::MixinDeclaration => c
                .ast
                .get(Id::<MixinDeclaration>::from_raw(current))
                .augment_keyword
                .is_some(),
            _ => false,
        };
        if augmentation {
            return;
        }
        if current != node
            && (c.ast.kind(current) == NodeKind::Block || Declaration::test(c.ast.kind(current)))
        {
            break;
        }
        ancestor = c.ast.parent(current);
    }
    let ty = match c.ast.kind(node) {
        NodeKind::FieldFormalParameter => {
            c.ast.get(Id::<FieldFormalParameter>::from_raw(node)).type_
        }
        NodeKind::RegularFormalParameter => {
            c.ast
                .get(Id::<RegularFormalParameter>::from_raw(node))
                .type_
        }
        NodeKind::SuperFormalParameter => {
            c.ast.get(Id::<SuperFormalParameter>::from_raw(node)).type_
        }
        _ => None,
    };
    if explicit_dynamic(c, ty) {
        c.report_node(out, &diag::AVOID_ANNOTATING_WITH_DYNAMIC, node, &[]);
    }
}

fn avoid_empty_else(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(s) = c.ast.get(Id::<IfStatement>::from_raw(node)).else_statement
        && c.ast.kind(s) == NodeKind::EmptyStatement
    {
        let n = c.ast.get(Id::<EmptyStatement>::from_raw(s.raw()));
        if !c.ast.tokens.get(n.semicolon).is_synthetic() {
            c.report_node(out, &diag::AVOID_EMPTY_ELSE, s, &[]);
        }
    }
}

fn avoid_escaping_inner_quotes(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let source = c.text(node);
    if source.starts_with('r')
        || source.starts_with('R')
        || source.starts_with("'''")
        || source.starts_with("\"\"\"")
    {
        return;
    }
    let single = source.starts_with('\'');
    let text = match c.ast.kind(node) {
        NodeKind::SimpleStringLiteral => c
            .ast
            .get(Id::<SimpleStringLiteral>::from_raw(node))
            .value
            .to_string(),
        NodeKind::StringInterpolation => {
            let n = c.ast.get(Id::<StringInterpolation>::from_raw(node));
            let mut s = String::new();
            for e in c.ast.list_raw(n.elements) {
                if let Some(i) = c.ast.cast::<InterpolationString>(*e) {
                    s.push_str(&c.ast.get(i).value);
                }
            }
            s
        }
        _ => return,
    };
    let (inner, other, from, to) = if single {
        ('\'', '"', "'", "\"")
    } else {
        ('"', '\'', "\"", "'")
    };
    if text.contains(inner) && !text.contains(other) {
        c.report_node(out, &diag::AVOID_ESCAPING_INNER_QUOTES, node, &[from, to]);
    }
}

fn avoid_final_parameters(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.ast
        .parent(node)
        .is_some_and(|p| c.ast.kind(p) == NodeKind::GenericFunctionType)
    {
        return;
    }
    let n = c.ast.get(Id::<FormalParameterList>::from_raw(node));
    for p in c.ast.list_raw(n.parameters) {
        if c.ast.kind(*p) == NodeKind::RegularFormalParameter {
            let p = c.ast.get(Id::<RegularFormalParameter>::from_raw(*p));
            if p.function_typed_suffix.is_none()
                && let Some(k) = p.const_final_or_var_keyword
                && lexeme(c, k) == "final"
            {
                c.report_token(out, &diag::AVOID_FINAL_PARAMETERS, k, &[]);
            }
        }
    }
}

fn avoid_js_rounded_ints(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<IntegerLiteral>::from_raw(node));
    if integer_literal_is_rounded(lexeme(c, n.literal)) {
        c.report_node(out, &diag::AVOID_JS_ROUNDED_INTS, node, &[]);
    }
}

fn integer_literal_is_rounded(text: &str) -> bool {
    let text = text.replace('_', "");
    let (radix, digits) =
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (16, hex)
        } else {
            (10, text.as_str())
        };
    let mut words = vec![0u32];
    for byte in digits.bytes() {
        let Some(digit) = (byte as char).to_digit(radix) else {
            return false;
        };
        let mut carry = digit as u64;
        for word in &mut words {
            let value = *word as u64 * radix as u64 + carry;
            *word = value as u32;
            carry = value >> 32;
        }
        if carry != 0 {
            words.push(carry as u32);
        }
    }
    while words.last() == Some(&0) && words.len() > 1 {
        words.pop();
    }
    if words.len() == 1 && words[0] == 0 {
        return false;
    }
    let bit_length = (words.len() - 1) * 32 + 32 - words.last().unwrap().leading_zeros() as usize;
    if bit_length <= 53 {
        return false;
    }
    let zero_words = words.iter().take_while(|word| **word == 0).count();
    let trailing = zero_words * 32
        + words
            .get(zero_words)
            .map_or(0, |word| word.trailing_zeros() as usize);
    trailing < bit_length - 53
}

fn avoid_multiple_declarations_per_line(
    c: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    if c.ast
        .parent(node)
        .is_some_and(|p| c.ast.kind(p) == NodeKind::ForPartsWithDeclarations)
    {
        return;
    }
    let n = c.ast.get(Id::<VariableDeclarationList>::from_raw(node));
    if let Some(v) = c.ast.list(n.variables).get(1) {
        c.report_token(
            out,
            &diag::AVOID_MULTIPLE_DECLARATIONS_PER_LINE,
            c.ast.get(*v).name,
            &[],
        );
    }
}

fn string_value(c: &LinterContext<'_>, node: Id<StringLiteral>) -> Option<String> {
    c.ast
        .cast::<SimpleStringLiteral>(node)
        .map(|n| c.ast.get(n).value.to_string())
}
fn avoid_relative_lib_imports(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<ImportDirective>::from_raw(node));
    if let Some(uri) = string_value(c, n.uri) {
        let scheme = uri
            .split(['/', '\\'])
            .next()
            .is_some_and(|s| s.contains(':'));
        if !scheme && uri.contains("/lib/") {
            c.report_node(out, &diag::AVOID_RELATIVE_LIB_IMPORTS, n.uri, &[]);
        }
    }
}

fn avoid_return_types_on_setters(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (property, ty) = match c.ast.kind(node) {
        NodeKind::FunctionDeclaration => {
            let n = c.ast.get(Id::<FunctionDeclaration>::from_raw(node));
            (n.property_keyword, n.return_type)
        }
        NodeKind::MethodDeclaration => {
            let n = c.ast.get(Id::<MethodDeclaration>::from_raw(node));
            (n.property_keyword, n.return_type)
        }
        _ => return,
    };
    if property.is_some_and(|t| lexeme(c, t) == "set")
        && let Some(ty) = ty
    {
        c.report_node(out, &diag::AVOID_RETURN_TYPES_ON_SETTERS, ty, &[]);
    }
}

fn count_named_type(ast: &Ast, root: NodeId, name: &str) -> usize {
    let mut count = 0;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if ast.kind(node) == NodeKind::NamedType {
            count += usize::from(
                ast.tokens
                    .lexeme(ast.get(Id::<NamedType>::from_raw(node)).name)
                    == name,
            );
        }
        pending.extend(ast.children(node));
    }
    count
}

fn avoid_private_typedef_functions(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let name = match c.ast.kind(node) {
        NodeKind::FunctionTypeAlias => c.ast.get(Id::<FunctionTypeAlias>::from_raw(node)).name,
        NodeKind::GenericTypeAlias => {
            let alias = c.ast.get(Id::<GenericTypeAlias>::from_raw(node));
            if alias.type_parameters.is_some()
                || matches!(
                    c.ast.kind(alias.type_),
                    NodeKind::NamedType | NodeKind::RecordTypeAnnotation
                )
            {
                return;
            }
            alias.name
        }
        _ => return,
    };
    let name_text = lexeme(c, name);
    if !name_text.starts_with('_') {
        return;
    }
    let count: usize = c
        .all_units
        .iter()
        .map(|unit| count_named_type(&unit.parsed.ast, unit.parsed.unit.raw(), name_text))
        .sum();
    if count <= 1 {
        c.report_token(out, &diag::AVOID_PRIVATE_TYPEDEF_FUNCTIONS, name, &[]);
    }
}

fn name_part_type_parameters(
    c: &LinterContext<'_>,
    part: Id<ClassNamePart>,
) -> Option<Id<TypeParameterList>> {
    match c.ast.kind(part) {
        NodeKind::NameWithTypeParameters => {
            c.ast
                .get(Id::<NameWithTypeParameters>::from_raw(part.raw()))
                .type_parameters
        }
        NodeKind::PrimaryConstructorDeclaration => {
            c.ast
                .get(Id::<PrimaryConstructorDeclaration>::from_raw(part.raw()))
                .type_parameters
        }
        _ => None,
    }
}

fn type_parameters_for(c: &LinterContext<'_>, node: NodeId) -> Option<Id<TypeParameterList>> {
    match c.ast.kind(node) {
        NodeKind::ClassDeclaration => name_part_type_parameters(
            c,
            c.ast.get(Id::<ClassDeclaration>::from_raw(node)).name_part,
        ),
        NodeKind::EnumDeclaration => name_part_type_parameters(
            c,
            c.ast.get(Id::<EnumDeclaration>::from_raw(node)).name_part,
        ),
        NodeKind::ExtensionDeclaration => {
            c.ast
                .get(Id::<ExtensionDeclaration>::from_raw(node))
                .type_parameters
        }
        NodeKind::ExtensionTypeDeclaration => name_part_type_parameters(
            c,
            c.ast
                .get(Id::<ExtensionTypeDeclaration>::from_raw(node))
                .name_part,
        ),
        NodeKind::MethodDeclaration => {
            c.ast
                .get(Id::<MethodDeclaration>::from_raw(node))
                .type_parameters
        }
        NodeKind::MixinDeclaration => {
            c.ast
                .get(Id::<MixinDeclaration>::from_raw(node))
                .type_parameters
        }
        NodeKind::FunctionDeclaration => {
            let n = c.ast.get(Id::<FunctionDeclaration>::from_raw(node));
            c.ast.get(n.function_expression).type_parameters
        }
        NodeKind::RegularFormalParameter => c
            .ast
            .get(Id::<RegularFormalParameter>::from_raw(node))
            .function_typed_suffix
            .and_then(|suffix| c.ast.get(suffix).type_parameters),
        NodeKind::GenericFunctionType => {
            c.ast
                .get(Id::<GenericFunctionType>::from_raw(node))
                .type_parameters
        }
        NodeKind::GenericTypeAlias => {
            c.ast
                .get(Id::<GenericTypeAlias>::from_raw(node))
                .type_parameters
        }
        NodeKind::FunctionExpression => {
            c.ast
                .get(Id::<FunctionExpression>::from_raw(node))
                .type_parameters
        }
        _ => None,
    }
}

fn check_shadowing(
    c: &LinterContext<'_>,
    current: Id<TypeParameterList>,
    ancestor: Id<TypeParameterList>,
    kind: &str,
    out: &mut Vec<Diagnostic>,
) {
    let names: Vec<&str> = c
        .ast
        .list(c.ast.get(ancestor).type_parameters)
        .iter()
        .map(|p| lexeme(c, c.ast.get(*p).name))
        .collect();
    for parameter in c.ast.list(c.ast.get(current).type_parameters) {
        let name = lexeme(c, c.ast.get(*parameter).name);
        if name == "_" && c.parsed.language_version.effective() >= (3, 7) {
            continue;
        }
        if names.contains(&name) {
            c.report_node(
                out,
                &diag::AVOID_SHADOWING_TYPE_PARAMETERS,
                *parameter,
                &[name, kind],
            );
        }
    }
}

fn check_shadowing_ancestors(
    c: &LinterContext<'_>,
    current: Id<TypeParameterList>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let mut parent = c.ast.parent(node);
    while let Some(ancestor) = parent {
        let kind = match c.ast.kind(ancestor) {
            NodeKind::ClassDeclaration => Some("class"),
            NodeKind::EnumDeclaration => Some("enum"),
            NodeKind::ExtensionDeclaration => Some("extension"),
            NodeKind::ExtensionTypeDeclaration => Some("extension type"),
            NodeKind::MethodDeclaration => Some("method"),
            NodeKind::MixinDeclaration => Some("mixin"),
            NodeKind::FunctionDeclaration => Some("function"),
            NodeKind::RegularFormalParameter => Some("parameter"),
            NodeKind::GenericFunctionType | NodeKind::FunctionExpression => Some("function"),
            NodeKind::GenericTypeAlias => Some("typedef"),
            _ => None,
        };
        if let (Some(kind), Some(parameters)) = (kind, type_parameters_for(c, ancestor)) {
            check_shadowing(c, current, parameters, kind, out);
        }
        parent = c.ast.parent(ancestor);
    }
}

fn avoid_shadowing_type_parameters(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::FunctionDeclarationStatement => {
            let d = c.ast.get(
                c.ast
                    .get(Id::<FunctionDeclarationStatement>::from_raw(node))
                    .function_declaration,
            );
            if let Some(p) = c.ast.get(d.function_expression).type_parameters {
                check_shadowing_ancestors(c, p, node, out);
            }
        }
        NodeKind::FunctionExpression => {
            if c.ast
                .parent(node)
                .is_some_and(|p| c.ast.kind(p) == NodeKind::FunctionDeclaration)
            {
                return;
            }
            if let Some(p) = type_parameters_for(c, node) {
                check_shadowing_ancestors(c, p, node, out);
            }
        }
        NodeKind::GenericFunctionType | NodeKind::RegularFormalParameter => {
            if let Some(p) = type_parameters_for(c, node) {
                check_shadowing_ancestors(c, p, node, out);
            }
        }
        NodeKind::MethodDeclaration => {
            let n = c.ast.get(Id::<MethodDeclaration>::from_raw(node));
            if !n.modifier_keyword.is_some_and(|t| lexeme(c, t) == "static")
                && let Some(p) = n.type_parameters
            {
                check_shadowing_ancestors(c, p, node, out);
            }
        }
        NodeKind::GenericTypeAlias => {
            let n = c.ast.get(Id::<GenericTypeAlias>::from_raw(node));
            if let Some(function) = c.ast.cast::<GenericFunctionType>(n.type_)
                && let (Some(current), Some(ancestor)) =
                    (c.ast.get(function).type_parameters, n.type_parameters)
            {
                check_shadowing(c, current, ancestor, "typedef", out);
            }
        }
        _ => {}
    }
}

fn avoid_single_cascade(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<CascadeExpression>::from_raw(node));
    let sections = c.ast.list_raw(n.cascade_sections);
    if sections.len() != 1
        || !c
            .ast
            .parent(node)
            .is_some_and(|p| c.ast.kind(p) == NodeKind::ExpressionStatement)
    {
        return;
    }
    let op = match c.ast.kind(sections[0]) {
        NodeKind::PropertyAccess => Some(
            c.ast
                .get(Id::<PropertyAccess>::from_raw(sections[0]))
                .operator,
        ),
        NodeKind::MethodInvocation => {
            c.ast
                .get(Id::<MethodInvocation>::from_raw(sections[0]))
                .operator
        }
        _ => None,
    };
    let replacement =
        if op.is_some_and(|t| c.ast.tokens.ty(t) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION) {
            "?."
        } else {
            "."
        };
    c.report_node(
        out,
        &diag::AVOID_SINGLE_CASCADE_IN_EXPRESSION_STATEMENTS,
        node,
        &[replacement],
    );
}

fn upper_camel(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut index = bytes.iter().take_while(|byte| **byte == b'_').count();
    loop {
        let start = index;
        while bytes.get(index) == Some(&b'$') {
            index += 1;
        }
        if index == start || bytes.get(index) != Some(&b'_') {
            index = start;
            break;
        }
        while bytes.get(index) == Some(&b'_') {
            index += 1;
        }
    }
    let Some(first) = bytes.get(index) else {
        return false;
    };
    if !(*first == b'$' || *first == b'?' || first.is_ascii_uppercase()) {
        return false;
    }
    bytes[index + 1..]
        .iter()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, b'$' | b'?'))
}
fn lower_camel(name: &str) -> bool {
    if name == "_" || name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase() {
        return true;
    }
    let name = name.trim_start_matches('_');
    let Some(first) = name.bytes().next() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || matches!(first, b'$' | b'?')) {
        return false;
    }
    let bytes = name.as_bytes();
    for i in 1..bytes.len() {
        let ch = bytes[i];
        if ch == b'_' {
            if i + 1 != bytes.len() && !bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                return false;
            }
        } else if !(ch.is_ascii_alphanumeric() || matches!(ch, b'$' | b'?')) {
            return false;
        }
    }
    true
}
fn report_type_name(c: &LinterContext<'_>, token: TokenId, out: &mut Vec<Diagnostic>) {
    let name = lexeme(c, token);
    if !upper_camel(name) {
        c.report_token(out, &diag::CAMEL_CASE_TYPES, token, &[name]);
    }
}
fn class_name(c: &LinterContext<'_>, part: Id<ClassNamePart>) -> TokenId {
    match c.ast.kind(part) {
        NodeKind::NameWithTypeParameters => {
            c.ast
                .get(Id::<NameWithTypeParameters>::from_raw(part.raw()))
                .type_name
        }
        NodeKind::PrimaryConstructorDeclaration => {
            c.ast
                .get(Id::<PrimaryConstructorDeclaration>::from_raw(part.raw()))
                .type_name
        }
        _ => c.ast.begin_token(part),
    }
}

fn camel_case_extensions(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<ExtensionDeclaration>::from_raw(node));
    if n.augment_keyword.is_some() {
        return;
    }
    if let Some(token) = n.name {
        let name = lexeme(c, token);
        if !upper_camel(name) {
            c.report_token(out, &diag::CAMEL_CASE_EXTENSIONS, token, &[name]);
        }
    }
}
fn camel_case_types(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let token = match c.ast.kind(node) {
        NodeKind::GenericTypeAlias => c.ast.get(Id::<GenericTypeAlias>::from_raw(node)).name,
        NodeKind::ClassTypeAlias => c.ast.get(Id::<ClassTypeAlias>::from_raw(node)).name,
        NodeKind::FunctionTypeAlias => c.ast.get(Id::<FunctionTypeAlias>::from_raw(node)).name,
        NodeKind::MixinDeclaration => {
            let n = c.ast.get(Id::<MixinDeclaration>::from_raw(node));
            if n.augment_keyword.is_some() {
                return;
            }
            n.name
        }
        NodeKind::ClassDeclaration => {
            let n = c.ast.get(Id::<ClassDeclaration>::from_raw(node));
            if n.augment_keyword.is_some() {
                return;
            }
            class_name(c, n.name_part)
        }
        NodeKind::EnumDeclaration => {
            let n = c.ast.get(Id::<EnumDeclaration>::from_raw(node));
            if n.augment_keyword.is_some() {
                return;
            }
            class_name(c, n.name_part)
        }
        NodeKind::ExtensionTypeDeclaration => {
            let n = c.ast.get(Id::<ExtensionTypeDeclaration>::from_raw(node));
            if n.augment_keyword.is_some() {
                return;
            }
            class_name(c, n.name_part)
        }
        _ => return,
    };
    report_type_name(c, token, out);
}

fn combinators_ordering(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let names = match c.ast.kind(node) {
        NodeKind::HideCombinator => c
            .ast
            .list_raw(c.ast.get(Id::<HideCombinator>::from_raw(node)).hidden_names),
        NodeKind::ShowCombinator => c
            .ast
            .list_raw(c.ast.get(Id::<ShowCombinator>::from_raw(node)).shown_names),
        _ => return,
    };
    let sorted = names.windows(2).all(|pair| {
        let a = c.ast.get(Id::<SimpleIdentifier>::from_raw(pair[0])).token;
        let b = c.ast.get(Id::<SimpleIdentifier>::from_raw(pair[1])).token;
        lexeme(c, a) <= lexeme(c, b)
    });
    if !sorted {
        c.report_node(out, &diag::COMBINATORS_ORDERING, node, &[]);
    }
}

fn report_constant(c: &LinterContext<'_>, token: TokenId, out: &mut Vec<Diagnostic>) {
    let name = lexeme(c, token);
    if !lower_camel(name) {
        c.report_token(out, &diag::CONSTANT_IDENTIFIER_NAMES, token, &[name]);
    }
}
fn constant_identifier_names(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::DeclaredVariablePattern => {
            let n = c.ast.get(Id::<DeclaredVariablePattern>::from_raw(node));
            let shortcut = c.ast.parent(node).is_some_and(|p| {
                if c.ast.kind(p) != NodeKind::PatternField {
                    return false;
                }
                let f = c.ast.get(Id::<PatternField>::from_raw(p));
                f.name.is_some_and(|name| c.ast.get(name).name.is_none())
            });
            if !shortcut {
                report_constant(c, n.name, out);
            }
        }
        NodeKind::EnumConstantDeclaration => {
            let n = c.ast.get(Id::<EnumConstantDeclaration>::from_raw(node));
            if n.augment_keyword.is_none() {
                report_constant(c, n.name, out);
            }
        }
        NodeKind::VariableDeclarationList => {
            let n = c.ast.get(Id::<VariableDeclarationList>::from_raw(node));
            if c.ast
                .parent(node)
                .is_some_and(|parent| match c.ast.kind(parent) {
                    NodeKind::TopLevelVariableDeclaration => c
                        .ast
                        .get(Id::<TopLevelVariableDeclaration>::from_raw(parent))
                        .augment_keyword
                        .is_some(),
                    NodeKind::FieldDeclaration => c
                        .ast
                        .get(Id::<FieldDeclaration>::from_raw(parent))
                        .augment_keyword
                        .is_some(),
                    _ => false,
                })
            {
                return;
            }
            if n.keyword.is_some_and(|t| lexeme(c, t) == "const") {
                for v in c.ast.list(n.variables) {
                    report_constant(c, c.ast.get(*v).name, out);
                }
            }
        }
        _ => {}
    }
}

fn report_braces(
    c: &LinterContext<'_>,
    statement: Id<Statement>,
    where_: &str,
    out: &mut Vec<Diagnostic>,
) {
    if c.ast.kind(statement) != NodeKind::Block {
        c.report_node(
            out,
            &diag::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES,
            statement,
            &[where_],
        );
    }
}
fn curly_braces(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::DoStatement => report_braces(
            c,
            c.ast.get(Id::<DoStatement>::from_raw(node)).body,
            "a do",
            out,
        ),
        NodeKind::ForStatement => report_braces(
            c,
            c.ast.get(Id::<ForStatement>::from_raw(node)).body,
            "a for",
            out,
        ),
        NodeKind::WhileStatement => report_braces(
            c,
            c.ast.get(Id::<WhileStatement>::from_raw(node)).body,
            "a while",
            out,
        ),
        NodeKind::IfStatement => {
            let n = c.ast.get(Id::<IfStatement>::from_raw(node));
            if let Some(other) = n.else_statement {
                report_braces(c, n.then_statement, "an if", out);
                if c.ast.kind(other) != NodeKind::IfStatement {
                    report_braces(c, other, "an if", out);
                }
                return;
            }
            if let Some(parent) = c.ast.parent(node)
                && c.ast.kind(parent) == NodeKind::IfStatement
            {
                let p = c.ast.get(Id::<IfStatement>::from_raw(parent));
                if p.else_statement.map(Id::raw) == Some(node) {
                    report_braces(c, n.then_statement, "an if", out);
                    return;
                }
            }
            if c.ast.kind(n.then_statement) != NodeKind::Block
                && !c.parsed.line_info.on_same_line(
                    c.ast.tokens.offset(n.if_keyword),
                    c.ast.end(n.then_statement),
                )
            {
                c.report_node(
                    out,
                    &diag::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES,
                    n.then_statement,
                    &["an if"],
                );
            }
        }
        _ => {}
    }
}

fn empty_catches(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<CatchClause>::from_raw(node));
    if let Some(parameter) = n.exception_parameter {
        let name = lexeme(c, c.ast.get(parameter).name);
        if !name.is_empty() && name.chars().all(|ch| ch == '_') {
            return;
        }
    }
    let body = c.ast.get(n.body);
    if body.statements.is_empty()
        && body.right_bracket.get().is_some()
        && c.ast
            .tokens
            .get(body.right_bracket)
            .preceding_comments
            .is_none()
    {
        c.report_node(out, &diag::EMPTY_CATCHES, n.body, &[]);
    }
}

fn check_empty_body(c: &LinterContext<'_>, body: Id<FunctionBody>, out: &mut Vec<Diagnostic>) {
    let Some(body) = c.ast.cast::<BlockFunctionBody>(body) else {
        return;
    };
    let block_id = c.ast.get(body).block;
    let block = c.ast.get(block_id);
    if block.statements.is_empty()
        && block.right_bracket.get().is_some()
        && c.ast
            .tokens
            .get(block.right_bracket)
            .preceding_comments
            .is_none()
    {
        c.report_node(out, &diag::EMPTY_CONSTRUCTOR_BODIES, block_id, &[]);
    }
}
fn empty_constructor_bodies(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = c.ast.get(Id::<ConstructorDeclaration>::from_raw(node));
            if n.factory_keyword.is_none() {
                check_empty_body(c, n.body, out);
            }
        }
        NodeKind::PrimaryConstructorBody => check_empty_body(
            c,
            c.ast.get(Id::<PrimaryConstructorBody>::from_raw(node)).body,
            out,
        ),
        _ => {}
    }
}

fn empty_container_bodies(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = c.ast.get(Id::<BlockClassBody>::from_raw(node));
    if !n.members.is_empty()
        || c.ast
            .tokens
            .get(n.right_bracket)
            .preceding_comments
            .is_some()
    {
        return;
    }
    let kind = match c.ast.parent(node).map(|p| c.ast.kind(p)) {
        Some(NodeKind::ClassDeclaration) => "class",
        Some(NodeKind::MixinDeclaration) => "mixin",
        Some(NodeKind::ExtensionDeclaration) => "extension",
        Some(NodeKind::ExtensionTypeDeclaration) => "extension type",
        _ => "container",
    };
    let offset = c.ast.tokens.offset(n.left_bracket);
    let length = c.ast.tokens.get(n.right_bracket).end() - offset;
    c.report_offset(
        out,
        &diag::EMPTY_CONTAINER_BODIES,
        offset as usize,
        length as usize,
        &[kind],
    );
}

fn empty_statements(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if let Some(parent) = c.ast.parent(node)
        && c.ast.kind(parent) == NodeKind::SwitchPatternCase
    {
        let n = c.ast.get(Id::<SwitchPatternCase>::from_raw(parent));
        let statements = c.ast.list_raw(n.statements);
        if statements.last().copied() == Some(node)
            && statements
                .iter()
                .all(|s| c.ast.kind(*s) == NodeKind::EmptyStatement)
        {
            return;
        }
    }
    c.report_node(out, &diag::EMPTY_STATEMENTS, node, &[]);
}

fn first_comment_child(ast: &Ast, node: NodeId) -> Option<Id<Comment>> {
    ast.children(node)
        .into_iter()
        .find_map(|child| ast.cast::<Comment>(child))
}

fn dangling_library_doc_comments(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let unit = c.ast.get(Id::<CompilationUnit>::from_raw(node));
    let directives = c.ast.list_raw(unit.directives);
    if let Some(first) = directives.first() {
        if matches!(
            c.ast.kind(*first),
            NodeKind::LibraryDirective | NodeKind::PartOfDirective
        ) {
            return;
        }
        if let Some(comment) = first_comment_child(c.ast, *first) {
            c.report_token(
                out,
                &diag::DANGLING_LIBRARY_DOC_COMMENTS,
                c.ast.get(comment).begin_token(c.ast),
                &[],
            );
        }
        return;
    }
    let declarations = c.ast.list_raw(unit.declarations);
    if declarations.is_empty() {
        let mut comment = c.ast.tokens.get(unit.end_token).preceding_comments;
        while let Some(token) = comment.get() {
            if c.ast.tokens.get(token).is_doc_comment() {
                c.report_token(out, &diag::DANGLING_LIBRARY_DOC_COMMENTS, token, &[]);
            }
            comment = c.ast.tokens.get(token).next;
        }
        return;
    }
    let first = declarations[0];
    let Some(comment_id) = first_comment_child(c.ast, first) else {
        return;
    };
    let comment = c.ast.get(comment_id);
    let tokens = c.ast.token_list(comment.tokens);
    for pair in tokens.windows(2) {
        let end_line = c
            .parsed
            .line_info
            .get_location(c.ast.tokens.get(pair[0]).end())
            .line_number;
        let next_line = c
            .parsed
            .line_info
            .get_location(c.ast.tokens.offset(pair[1]))
            .line_number;
        if next_line > end_line + 1 {
            c.report_token(out, &diag::DANGLING_LIBRARY_DOC_COMMENTS, pair[0], &[]);
            return;
        }
    }
    let mut last = comment.end_token(c.ast);
    let mut following = c.ast.tokens.get(last).next;
    while let Some(token) = following.get() {
        let end_line = c
            .parsed
            .line_info
            .get_location(c.ast.tokens.get(last).end())
            .line_number;
        let next_line = c
            .parsed
            .line_info
            .get_location(c.ast.tokens.offset(token))
            .line_number;
        if next_line > end_line + 1 {
            c.report_node(out, &diag::DANGLING_LIBRARY_DOC_COMMENTS, comment_id, &[]);
            return;
        }
        last = token;
        following = c.ast.tokens.get(token).next;
    }
    let comment_line = c
        .parsed
        .line_info
        .get_location(c.ast.tokens.get(last).end())
        .line_number;
    let declaration_line = c
        .parsed
        .line_info
        .get_location(c.ast.tokens.offset(c.ast.begin_token(first)))
        .line_number;
    if declaration_line > comment_line + 1 {
        c.report_node(out, &diag::DANGLING_LIBRARY_DOC_COMMENTS, comment_id, &[]);
    }
}

fn literal_value_ast(ast: &Ast, node: Id<StringLiteral>) -> Option<String> {
    ast.cast::<SimpleStringLiteral>(node)
        .map(|literal| ast.get(literal).value.to_string())
}
fn compare_directives(a: &str, b: &str) -> std::cmp::Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:"))
        && !a.starts_with('/')
        && !b.starts_with('/')
    {
        return a.cmp(b);
    }
    let (Some(ai), Some(bi)) = (a.find('/'), b.find('/')) else {
        return a.cmp(b);
    };
    a[..ai]
        .cmp(&b[..bi])
        .then_with(|| a[ai + 1..].cmp(&b[bi + 1..]))
}
fn report_directive(
    c: &LinterContext<'_>,
    out: &mut Vec<Diagnostic>,
    code: &'static dartr_diagnostics::DiagnosticCode,
    node: NodeId,
    args: &[&str],
    linted: &mut Vec<NodeId>,
) {
    if !linted.contains(&node) {
        linted.push(node);
        c.report_node(out, code, node, args);
    }
}
fn check_directive_group(
    c: &LinterContext<'_>,
    out: &mut Vec<Diagnostic>,
    items: &[(NodeId, String)],
    kind: &str,
    linted: &mut Vec<NodeId>,
) {
    let mut left_prefix = true;
    for (node, uri) in items {
        if left_prefix && uri.starts_with("dart:") {
            continue;
        }
        left_prefix = false;
        if uri.starts_with("dart:") {
            let plural = format!("{kind}s");
            report_directive(
                c,
                out,
                &diag::DIRECTIVES_ORDERING_DART,
                *node,
                &[&plural],
                linted,
            );
        }
    }
    let non_dart: Vec<_> = items
        .iter()
        .filter(|(_, uri)| !uri.starts_with("dart:"))
        .collect();
    let mut after_absolute = false;
    for (node, uri) in non_dart {
        if !after_absolute && uri.contains(':') {
            continue;
        }
        after_absolute = true;
        if uri.starts_with("package:") {
            let plural = format!("{kind}s");
            report_directive(
                c,
                out,
                &diag::DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE,
                *node,
                &[&plural],
                linted,
            );
        }
    }
    for category in 0..3 {
        let filtered: Vec<_> = items
            .iter()
            .filter(|(_, uri)| match category {
                0 => uri.starts_with("dart:"),
                1 => !uri.contains(':'),
                _ => uri.starts_with("package:"),
            })
            .collect();
        for pair in filtered.windows(2) {
            if compare_directives(&pair[0].1, &pair[1].1).is_gt() {
                report_directive(
                    c,
                    out,
                    &diag::DIRECTIVES_ORDERING_ALPHABETICAL,
                    pair[1].0,
                    &[],
                    linted,
                );
            }
        }
    }
}
fn directives_ordering(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let unit = c.ast.get(Id::<CompilationUnit>::from_raw(node));
    let directives = c.ast.list_raw(unit.directives);
    let mut imports = vec![];
    let mut exports = vec![];
    for directive in directives {
        match c.ast.kind(*directive) {
            NodeKind::ImportDirective => {
                let n = c.ast.get(Id::<ImportDirective>::from_raw(*directive));
                if let Some(uri) = literal_value_ast(c.ast, n.uri) {
                    imports.push((*directive, uri));
                }
            }
            NodeKind::ExportDirective => {
                let n = c.ast.get(Id::<ExportDirective>::from_raw(*directive));
                if let Some(uri) = literal_value_ast(c.ast, n.uri) {
                    exports.push((*directive, uri));
                }
            }
            _ => {}
        }
    }
    let mut linted = vec![];
    check_directive_group(c, out, &imports, "import", &mut linted);
    check_directive_group(c, out, &exports, "export", &mut linted);
    let mut index = directives.len();
    while index > 0 && c.ast.kind(directives[index - 1]) == NodeKind::PartDirective {
        index -= 1
    }
    while index > 0 && c.ast.kind(directives[index - 1]) == NodeKind::ExportDirective {
        index -= 1
    }
    for directive in directives[..index].iter().rev() {
        if c.ast.kind(*directive) == NodeKind::ExportDirective {
            report_directive(
                c,
                out,
                &diag::DIRECTIVES_ORDERING_EXPORTS,
                *directive,
                &[],
                &mut linted,
            );
        }
    }
    // Doc imports have their own AST arena, so perform the same ordering and report their absolute source ranges directly.
    for directive in directives {
        if c.ast.kind(*directive) != NodeKind::LibraryDirective {
            continue;
        }
        let library = c.ast.get(Id::<LibraryDirective>::from_raw(*directive));
        let Some(comment) = library.documentation_comment else {
            continue;
        };
        let imports = &c.ast.get(comment).doc_imports;
        let mut values = vec![];
        for import in imports {
            let n = import.ast.get(import.import);
            if let Some(uri) = literal_value_ast(&import.ast, n.uri) {
                values.push((import, uri));
            }
        }
        let mut reported = vec![];
        let mut report =
            |idx: usize, code: &'static dartr_diagnostics::DiagnosticCode, args: &[&str]| {
                if !reported.contains(&idx) {
                    reported.push(idx);
                    let import = values[idx].0;
                    c.report_offset(
                        out,
                        code,
                        import.ast.offset(import.import) as usize,
                        import.ast.length(import.import) as usize,
                        args,
                    );
                }
            };
        let mut prefix = true;
        for (i, (_, uri)) in values.iter().enumerate() {
            if prefix && uri.starts_with("dart:") {
                continue;
            }
            prefix = false;
            if uri.starts_with("dart:") {
                report(i, &diag::DIRECTIVES_ORDERING_DART, &["@docImports"]);
            }
        }
        let mut after_absolute = false;
        for (i, (_, uri)) in values.iter().enumerate() {
            if uri.starts_with("dart:") {
                continue;
            }
            if !after_absolute && uri.contains(':') {
                continue;
            }
            after_absolute = true;
            if uri.starts_with("package:") {
                report(
                    i,
                    &diag::DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE,
                    &["@docImports"],
                );
            }
        }
        for category in 0..3 {
            let filtered: Vec<_> = (0..values.len())
                .filter(|i| match category {
                    0 => values[*i].1.starts_with("dart:"),
                    1 => !values[*i].1.contains(':'),
                    _ => values[*i].1.starts_with("package:"),
                })
                .collect();
            for pair in filtered.windows(2) {
                if compare_directives(&values[pair[0]].1, &values[pair[1]].1).is_gt() {
                    report(pair[1], &diag::DIRECTIVES_ORDERING_ALPHABETICAL, &[]);
                }
            }
        }
    }
}

fn ignore_comment_state(text: &str) -> Option<bool> {
    let rest = text
        .strip_prefix("//")?
        .trim_start_matches('/')
        .trim_start();
    let rest = rest
        .strip_prefix("ignore:")
        .or_else(|| rest.strip_prefix("ignore_for_file:"))?;
    let mut any = false;
    let mut cursor = rest;
    loop {
        cursor = cursor.trim_start();
        let len = cursor
            .bytes()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
            .count();
        if len == 0 {
            break;
        }
        any = true;
        let word = &cursor[..len];
        cursor = &cursor[len..];
        if word == "type" {
            let rest = cursor.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                let value_len = rest
                    .bytes()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
                    .count();
                if value_len == 0 {
                    break;
                }
                cursor = &rest[value_len..];
            }
        } else if let Some(rest) = cursor.strip_prefix('/') {
            let plugin_len = rest
                .bytes()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
                .count();
            if plugin_len == 0 {
                break;
            }
            cursor = &rest[plugin_len..];
        }
        cursor = cursor.trim_start();
        if let Some(next) = cursor.strip_prefix(',') {
            cursor = next;
            continue;
        }
        break;
    }
    if !any {
        return None;
    }
    Some(!cursor.trim().is_empty())
}
fn document_ignores(c: &LinterContext<'_>, _: NodeId, out: &mut Vec<Diagnostic>) {
    for i in 0..c.ast.tokens.len() {
        let token = TokenId(i as u32);
        let t = c.ast.tokens.get(token);
        if !t.is_comment() {
            continue;
        }
        if ignore_comment_state(lexeme(c, token)) != Some(false) {
            continue;
        }
        let line = c.parsed.line_info.get_location(t.offset).line_number;
        if line > 1 {
            let start = c.parsed.line_info.line_starts[(line - 2) as usize] as usize;
            let utf16: Vec<_> = c.source.encode_utf16().collect();
            let end = c.parsed.line_info.line_starts[(line - 1) as usize] as usize;
            let previous = String::from_utf16_lossy(&utf16[start..end]);
            if previous.trim_start().starts_with("//") {
                continue;
            }
        }
        c.report_token(out, &diag::DOCUMENT_IGNORES, token, &[]);
    }
}

fn valid_dart_file_name(name: &str) -> bool {
    if name.len() < 6 || !name.ends_with(".dart") {
        return true;
    }
    let stem = &name[..name.len() - 5];
    if stem.len() > 2 && stem[1..stem.len() - 1].contains('.') {
        return true;
    }
    stem.bytes()
        .enumerate()
        .all(|(i, ch)| ch.is_ascii_lowercase() || ch == b'_' || (i > 0 && ch.is_ascii_digit()))
}
fn file_names(c: &LinterContext<'_>, _: NodeId, out: &mut Vec<Diagnostic>) {
    let name = c.path.replace('\\', "/");
    let name = name.rsplit('/').next().unwrap_or(&name);
    if !valid_dart_file_name(name) {
        c.report_offset(out, &diag::FILE_NAMES, 0, 0, &[name]);
    }
}

fn eol_at_end_of_file(c: &LinterContext<'_>, _: NodeId, out: &mut Vec<Diagnostic>) {
    if c.source.is_empty() {
        return;
    }
    let multiple =
        c.source.ends_with("\n\n") || c.source.ends_with("\r\r") || c.source.ends_with("\r\n\r\n");
    let code = if multiple {
        Some(&diag::EOL_AT_END_OF_FILE_TOO_MANY)
    } else if c.source.ends_with('\n') || c.source.ends_with('\r') {
        None
    } else {
        Some(&diag::EOL_AT_END_OF_FILE_MISSING)
    };
    if let Some(code) = code {
        c.report_offset(
            out,
            code,
            c.source.trim_end().encode_utf16().count(),
            1,
            &[],
        );
    }
}

fn invalid_todo(comment: &str) -> bool {
    let b = comment.as_bytes();
    if b.len() < 2 || b[0] != b'/' || b[1] != b'/' {
        return false;
    }
    let mut i = 2;
    while b.get(i) == Some(&b'/') {
        i += 1
    }
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1
    }
    if b.get(i..i + 4)
        .is_none_or(|w| !w.eq_ignore_ascii_case(b"TODO"))
    {
        return false;
    }
    if b.get(i + 4)
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
    {
        return false;
    }
    let mut e = 2;
    while b.get(e).is_some_and(u8::is_ascii_whitespace) {
        e += 1
    }
    if b.get(e..e + 5) != Some(b"TODO(".as_slice()) {
        return true;
    }
    e += 5;
    let start = e;
    if !b.get(e).is_some_and(u8::is_ascii_alphanumeric) {
        return true;
    }
    e += 1;
    while b
        .get(e)
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, b'-' | b'.'))
    {
        e += 1
    }
    e == start
        || b.get(e) != Some(&b')')
        || b.get(e + 1) != Some(&b':')
        || b.get(e + 2) != Some(&b' ')
}
fn flutter_style_todos(c: &LinterContext<'_>, _: NodeId, out: &mut Vec<Diagnostic>) {
    for i in 0..c.ast.tokens.len() {
        let token = TokenId(i as u32);
        if c.ast.tokens.get(token).is_comment() && invalid_todo(lexeme(c, token)) {
            c.report_token(out, &diag::FLUTTER_STYLE_TODOS, token, &[]);
        }
    }
}
