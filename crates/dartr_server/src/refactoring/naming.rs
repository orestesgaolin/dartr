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
