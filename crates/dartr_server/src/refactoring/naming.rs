// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/naming_conventions.dart

//! The naming conventions of the refactorings (Dart `validate*Name`).

use super::status::RefactoringStatus;

/// Dart `Keyword.keywords[identifier]`: whether [identifier] is a keyword,
/// and whether it is a built-in identifier or a pseudo keyword.
fn keyword(identifier: &str) -> Option<bool> {
    for i in 0..=255u8 {
        let ty = dartr_syntax::TokenType(i);
        if ty.is_keyword() && ty.lexeme() == identifier {
            return Some(ty.is_built_in() || ty.is_pseudo());
        }
    }
    None
}

pub fn validate_class_name(name: &str) -> RefactoringStatus {
    validate_upper_camel_case(name, "Class")
}

pub fn validate_constructor_name(name: &str) -> RefactoringStatus {
    if name.is_empty() {
        return RefactoringStatus::new();
    }
    validate_lower_camel_case(name, "Constructor", true)
}

pub fn validate_extension_type_name(name: &str) -> RefactoringStatus {
    validate_upper_camel_case(name, "Extension type")
}

pub fn validate_field_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Field", true)
}

pub fn validate_function_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Function", true)
}

pub fn validate_import_prefix_name(name: &str) -> RefactoringStatus {
    if name.is_empty() {
        return RefactoringStatus::new();
    }
    validate_lower_camel_case(name, "Import prefix", false)
}

pub fn validate_label_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Label", true)
}

pub fn validate_library_name(name: &str) -> RefactoringStatus {
    if name.trim().is_empty() {
        return RefactoringStatus::fatal("Library name must not be blank.");
    }
    let identifiers: Vec<&str> = name.split('.').collect();
    for identifier in &identifiers {
        let status = validate_identifier(
            identifier,
            "Library name identifier",
            "a lowercase letter or underscore",
            false,
        );
        if !status.is_ok() {
            return status;
        }
    }
    for identifier in &identifiers {
        if identifier.chars().any(|c| c.is_ascii_uppercase()) {
            return RefactoringStatus::warning(
                "Library name should consist of lowercase identifier separated by dots.",
            );
        }
    }
    RefactoringStatus::new()
}

pub fn validate_method_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Method", true)
}

pub fn validate_parameter_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Parameter", true)
}

pub fn validate_type_alias_name(name: &str) -> RefactoringStatus {
    validate_upper_camel_case(name, "Type alias")
}

pub fn validate_type_parameter(name: &str) -> RefactoringStatus {
    validate_upper_camel_case(name, "Type parameter")
}

pub fn validate_variable_name(name: &str) -> RefactoringStatus {
    validate_lower_camel_case(name, "Variable", true)
}

/// Dart `isLetter` of a UTF-16 code unit.
fn is_letter(c: u16) -> bool {
    (b'a' as u16..=b'z' as u16).contains(&c) || (b'A' as u16..=b'Z' as u16).contains(&c)
}

fn is_digit(c: u16) -> bool {
    (b'0' as u16..=b'9' as u16).contains(&c)
}

fn validate_identifier(
    identifier: &str,
    desc: &str,
    begin_desc: &str,
    allow_built_in: bool,
) -> RefactoringStatus {
    // Dart `String.trim` removes whitespace (`isWhitespace` is close to
    // `char::is_whitespace`).
    if identifier != identifier.trim() {
        return RefactoringStatus::fatal(format!("{desc} must not start or end with a blank."));
    }
    let units: Vec<u16> = identifier.encode_utf16().collect();
    if units.is_empty() {
        return RefactoringStatus::fatal(format!("{desc} must not be empty."));
    }
    if let Some(built_in_or_pseudo) = keyword(identifier) {
        if built_in_or_pseudo && allow_built_in {
            return RefactoringStatus::warning("Avoid using built-in identifiers as names.");
        }
        return RefactoringStatus::fatal(format!("{desc} must not be a keyword."));
    }
    for &c in &units {
        if !(is_letter(c) || is_digit(c) || c == b'_' as u16) && c != b'$' as u16 {
            let char_str = String::from_utf16_lossy(&[c]);
            return RefactoringStatus::fatal(format!("{desc} must not contain '{char_str}'."));
        }
    }
    let first = units[0];
    if !is_letter(first) && first != b'_' as u16 && first != b'$' as u16 {
        return RefactoringStatus::fatal(format!("{desc} must begin with {begin_desc}."));
    }
    RefactoringStatus::new()
}

fn validate_lower_camel_case(
    identifier: &str,
    desc: &str,
    allow_built_in: bool,
) -> RefactoringStatus {
    let desc = format!("{desc} name");
    let status = validate_identifier(
        identifier,
        &desc,
        "a lowercase letter or underscore",
        allow_built_in,
    );
    if !status.is_ok() {
        return status;
    }
    let first = identifier.encode_utf16().next().unwrap();
    if first == b'_' as u16 || first == b'$' as u16 {
        return RefactoringStatus::new();
    }
    if !(b'a' as u16..=b'z' as u16).contains(&first) {
        return RefactoringStatus::warning(format!("{desc} should start with a lowercase letter."));
    }
    RefactoringStatus::new()
}

fn validate_upper_camel_case(identifier: &str, desc: &str) -> RefactoringStatus {
    let desc = format!("{desc} name");
    let status = validate_identifier(
        identifier,
        &desc,
        "an uppercase letter or underscore",
        false,
    );
    if !status.is_ok() {
        return status;
    }
    let first = identifier.encode_utf16().next().unwrap();
    if first == b'_' as u16 || first == b'$' as u16 {
        return RefactoringStatus::new();
    }
    if !(b'A' as u16..=b'Z' as u16).contains(&first) {
        return RefactoringStatus::warning(format!(
            "{desc} should start with an uppercase letter."
        ));
    }
    RefactoringStatus::new()
}

/// Dart `StringExtension.toFileName` (`pkg/analysis_server/lib/src/utilities/extensions/string.dart`).
pub fn to_file_name(symbol_name: &str) -> String {
    let mut out = String::with_capacity(symbol_name.len() + 8);
    for (i, c) in symbol_name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out.push_str(".dart");
    out
}

const KNOWN_METHOD_NAME_PREFIXES: &[&str] = &["get", "is", "to"];

/// Dart `getCamelWords` (`pkg/analysis_server/lib/src/services/correction/name_suggestion.dart`).
pub fn get_camel_words(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut word_start = 0usize;
    for i in 1..chars.len() {
        let c = chars[i];
        if c.is_ascii_uppercase() {
            let word: String = chars[word_start..i].iter().collect();
            parts.push(word);
            word_start = i;
        }
    }
    let last_word: String = chars[word_start..].iter().collect();
    parts.push(last_word);
    parts
}

/// Dart `getCamelWordCombinations`: for `fooBarBaz` returns `["fooBarBaz", "barBaz", "baz"]`.
pub fn get_camel_word_combinations(name: &str) -> Vec<String> {
    let words = get_camel_words(name);
    let mut result = Vec::with_capacity(words.len());
    for i in 0..words.len() {
        let mut s = String::new();
        for (j, w) in words[i..].iter().enumerate() {
            if j == 0 {
                let mut ch = w.chars();
                if let Some(first) = ch.next() {
                    s.push(first.to_ascii_lowercase());
                    s.push_str(ch.as_str());
                }
            } else {
                s.push_str(w);
            }
        }
        if !s.is_empty() && !result.contains(&s) {
            result.push(s);
        }
    }
    result
}

fn capitalize(s: &str) -> String {
    let mut ch = s.chars();
    let Some(first) = ch.next() else {
        return String::new();
    };
    let mut out = String::with_capacity(s.len());
    out.push(first.to_ascii_uppercase());
    out.push_str(ch.as_str());
    out
}

fn add_single_character_name(
    excluded: &std::collections::HashSet<String>,
    res: &mut Vec<String>,
    c: char,
) {
    let mut ch = c;
    while ch < 'z' {
        let name = ch.to_string();
        if !excluded.contains(&name) {
            if !res.contains(&name) {
                res.push(name);
            }
            break;
        }
        ch = char::from_u32(ch as u32 + 1).unwrap_or('{');
    }
}

fn add_all_suggestions(
    excluded: &std::collections::HashSet<String>,
    res: &mut Vec<String>,
    to_add: Vec<String>,
    prefix: Option<&str>,
) {
    for item in to_add {
        if item.is_empty() {
            continue;
        }
        let mut suffix = 1usize;
        loop {
            let name = if suffix > 1 {
                format!("{item}{suffix}")
            } else {
                item.clone()
            };
            if !excluded.contains(&name) {
                let candidate = match prefix {
                    Some(p) => format!("{p}{}", capitalize(&name)),
                    None => name,
                };
                if !res.contains(&candidate) {
                    res.push(candidate);
                }
                break;
            }
            suffix += 1;
        }
    }
}

/// Dart `getVariableNameSuggestionsForText`.
pub fn get_variable_name_suggestions_for_text(
    text: &str,
    excluded: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut clean = String::new();
    for c in text.chars() {
        if c.is_ascii_alphabetic() || c.is_whitespace() {
            clean.push(c);
        }
    }
    let words: Vec<&str> = clean.split(' ').collect();
    let mut base_name = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            base_name.push_str(w);
        } else {
            base_name.push_str(&capitalize(w));
        }
    }
    let mut res = Vec::new();
    add_all_suggestions(
        excluded,
        &mut res,
        get_camel_word_combinations(&base_name),
        None,
    );
    res
}

/// Dart `_getBaseNameFromExpression`.
fn get_base_name_from_expression(
    ctx: &dartr_element::Ctx<'_>,
    ast: &dartr_ast::Ast,
    tables: &dartr_element::ResolutionTables,
    mut expr: dartr_ast::NodeId,
) -> Option<String> {
    use dartr_ast::*;
    loop {
        if let Some(as_expr) = ast.cast::<AsExpression>(expr) {
            expr = ast[as_expr].expression.raw();
        } else if let Some(paren) = ast.cast::<ParenthesizedExpression>(expr) {
            expr = ast[paren].expression.raw();
        } else {
            break;
        }
    }
    if let Some(sid) = ast.cast::<SimpleIdentifier>(expr) {
        let s = ast.tokens.lexeme(ast[sid].token);
        return if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
    if let Some(pi) = ast.cast::<PrefixedIdentifier>(expr) {
        let s = ast.tokens.lexeme(ast[ast[pi].identifier].token);
        return if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
    if let Some(pa) = ast.cast::<PropertyAccess>(expr) {
        let s = ast.tokens.lexeme(ast[ast[pa].property_name].token);
        return if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
    if let Some(ice) = ast.cast::<InstanceCreationExpression>(expr) {
        let ctor_name = ast[ice].constructor_name;
        let ty = ast[ctor_name].type_;
        if let Some(imp) = ast[ty].import_prefix {
            let is_prefix = dartr_resolver::error::support::element_of(ctx, tables, imp.raw())
                .is_some_and(|el| el.tag() == dartr_element::Tag::Prefix);
            if is_prefix {
                let s = ast.tokens.lexeme(ast[ty].name);
                return if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                };
            }
            let s = ast.tokens.lexeme(ast[imp].name);
            return if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            };
        }
        let s = ast.tokens.lexeme(ast[ty].name);
        return if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
    let mut name: Option<String> = None;
    if let Some(mi) = ast.cast::<MethodInvocation>(expr) {
        let s = ast.tokens.lexeme(ast[ast[mi].method_name].token);
        if !s.is_empty() {
            name = Some(s.to_string());
        }
    } else if let Some(idx) = ast.cast::<IndexExpression>(expr) {
        if let Some(target) = ast[idx].target {
            if let Some(base) = get_base_name_from_expression(ctx, ast, tables, target.raw()) {
                if let Some(stripped) = base.strip_suffix('s') {
                    name = Some(stripped.to_string());
                } else {
                    name = Some(base);
                }
            }
        }
    }
    let mut name = name?;
    for &prefix in KNOWN_METHOD_NAME_PREFIXES {
        if name.starts_with(prefix) {
            if name == prefix {
                return None;
            } else if name[prefix.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
            {
                name = name[prefix.len()..].to_string();
                break;
            }
        }
    }
    if name.is_empty() { None } else { Some(name) }
}

/// Dart `_getBaseNameFromLocationInParent`.
fn get_base_name_from_location_in_parent(
    ctx: &dartr_element::Ctx<'_>,
    ast: &dartr_ast::Ast,
    tables: &dartr_element::ResolutionTables,
    expr: dartr_ast::NodeId,
) -> Option<String> {
    use dartr_ast::*;
    use dartr_typesystem::TypeExt;
    let parent = ast.parent(expr)?;
    if let Some(named) = ast.cast::<NamedArgument>(parent) {
        if ast[named].argument_expression.raw() == expr {
            return Some(ast.tokens.lexeme(ast[named].name).to_string());
        }
    }
    if let Some(param) =
        dartr_resolver::error::support::corresponding_parameter(ctx, ast, tables, expr)
    {
        if let Some(s) = ctx.element_name(param)
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Dart `getVariableNameSuggestionsForExpression`.
pub fn get_variable_name_suggestions_for_expression(
    ctx: &dartr_element::Ctx<'_>,
    ast: &dartr_ast::Ast,
    tables: &dartr_element::ResolutionTables,
    expected_type: Option<dartr_element::TypeId>,
    assigned_expression: Option<dartr_ast::NodeId>,
    excluded: &std::collections::HashSet<String>,
    is_method: bool,
) -> Vec<String> {
    use dartr_ast::*;
    use dartr_element::TypeKind;
    let mut prefix: Option<&str> = None;
    if is_method && let Some(mut cur) = assigned_expression {
        loop {
            if let Some(md) = ast.cast::<MethodDeclaration>(cur) {
                let enclosing_name = ast.tokens.lexeme(ast[md].name);
                if enclosing_name.starts_with("build") {
                    prefix = Some("build");
                }
                break;
            }
            let Some(p) = ast.parent(cur) else {
                break;
            };
            cur = p;
        }
    }
    let mut res = Vec::new();
    if let Some(expr) = assigned_expression {
        if let Some(mut name_from_expr) = get_base_name_from_expression(ctx, ast, tables, expr) {
            if let Some(stripped) = name_from_expr.strip_prefix('_') {
                name_from_expr = stripped.to_string();
            }
            add_all_suggestions(
                excluded,
                &mut res,
                get_camel_word_combinations(&name_from_expr),
                prefix,
            );
        }
        if let Some(name_from_parent) =
            get_base_name_from_location_in_parent(ctx, ast, tables, expr)
        {
            add_all_suggestions(
                excluded,
                &mut res,
                get_camel_word_combinations(&name_from_parent),
                None,
            );
        }
    }
    if let Some(ty) = expected_type
        && !matches!(ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid)
    {
        if let TypeKind::Interface { element, .. } = *ctx.ty(ty)
            && let Some(n) = ctx.interface(element).name
        {
            let type_name = ctx.name_str(n);
            if type_name == "int" {
                add_single_character_name(excluded, &mut res, 'i');
            } else if type_name == "double" {
                add_single_character_name(excluded, &mut res, 'd');
            } else if type_name == "String" {
                add_single_character_name(excluded, &mut res, 's');
            } else {
                add_all_suggestions(
                    excluded,
                    &mut res,
                    get_camel_word_combinations(type_name),
                    None,
                );
            }
        }
    }
    res
}
