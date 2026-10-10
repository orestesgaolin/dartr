// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (DartFileEditBuilderImpl._addLibraryImports)
// Dart source: pkg/analyzer/lib/src/analysis_options/code_style_options.dart (preferredQuoteForUris)

//! The edits that add import directives to a unit, as the change builder
//! of Dart writes them.

use dartr_ast::*;
use serde_json::{Value, json};

use super::target::TokenExt;
use crate::mapping::to_range;

/// One edit of [`import_edits`]: an insertion or a deletion.
struct SourceEdit {
    offset: u32,
    length: u32,
    text: String,
}

/// The first line ending of `content` (Dart `String.endOfLine`), or `\n`.
pub fn end_of_line(content: &str) -> &'static str {
    match content.find('\n') {
        Some(i) if i > 0 && content.as_bytes()[i - 1] == b'\r' => "\r\n",
        Some(_) => "\n",
        None => match content.find('\r') {
            Some(_) => "\r",
            None => "\n",
        },
    }
}

/// The string value of a directive URI (Dart `StringLiteral.stringValue`
/// for a simple string, otherwise the empty string).
fn directive_uri_value(ast: &Ast, uri: Id<StringLiteral>) -> String {
    ast.cast::<SimpleStringLiteral>(uri.raw())
        .map(|s| ast[s].value.to_string())
        .unwrap_or_default()
}

/// Dart `CodeStyleOptions.preferredQuoteForUris`.
fn preferred_quote_for_uris(ast: &Ast, imports: &[Id<ImportDirective>], lint_quote: Option<char>) -> char {
    if let Some(q) = lint_quote {
        return q;
    }
    let (mut single, mut double) = (0, 0);
    let mut add = |s: Id<SimpleStringLiteral>| {
        if ast.t_lexeme(ast[s].literal).starts_with('"') {
            double += 1;
        } else {
            single += 1;
        }
    };
    for i in imports {
        let uri = ast[*i].uri.raw();
        if let Some(s) = ast.cast::<SimpleStringLiteral>(uri) {
            add(s);
        } else if let Some(a) = ast.cast::<AdjacentStrings>(uri) {
            for string in ast.list_raw(ast[a].strings) {
                if let Some(s) = ast.cast::<SimpleStringLiteral>(*string) {
                    add(s);
                }
            }
        }
    }
    if double > single { '"' } else { '\'' }
}

/// The edits that import the libraries `uris` (without a prefix or
/// combinators) into `unit` (Dart `DartFileEditBuilderImpl._addLibraryImports`).
pub fn import_edits(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    line_info: &dartr_syntax::LineInfo,
    content: &str,
    lint_quote: Option<char>,
    uris: &[String],
) -> Vec<Value> {
    let eol = end_of_line(content);
    let mut library_directive = None;
    let mut imports: Vec<Id<ImportDirective>> = Vec::new();
    let mut first_export = None;
    let mut first_part = None;
    for d in ast.list_raw(ast[unit].directives) {
        if ast.is::<LibraryDirective>(*d) {
            library_directive = Some(*d);
        } else if let Some(i) = ast.cast::<ImportDirective>(*d) {
            imports.push(i);
        } else if ast.is::<ExportDirective>(*d) {
            first_export.get_or_insert(*d);
        } else if ast.is::<PartDirective>(*d) {
            first_part.get_or_insert(*d);
        }
    }
    // Dart `_LibraryImport.compareTo` (by the directive sort priority, then
    // by the URI).
    let priority = |uri: &str| -> u8 {
        if uri.starts_with("dart:") {
            0
        } else if uri.starts_with("package:") {
            1
        } else {
            2
        }
    };
    let mut import_list: Vec<&String> = uris.iter().collect();
    import_list.sort_by(|a, b| (priority(a), a.as_str()).cmp(&(priority(b), b.as_str())));
    let quote = preferred_quote_for_uris(ast, &imports, lint_quote);
    let write_import = |uri: &str| format!("import {quote}{uri}{quote};");
    let mut edits: Vec<SourceEdit> = Vec::new();
    fn insert(edits: &mut Vec<SourceEdit>, offset: u32, text: String) {
        edits.push(SourceEdit { offset, length: 0, text });
    }

    if !imports.is_empty() {
        for uri in &import_list {
            let is_dart = uri.starts_with("dart:");
            let is_package = uri.starts_with("package:");
            let mut inserted = false;
            let mut last_existing: Option<Id<ImportDirective>> = None;
            let mut last_existing_dart: Option<Id<ImportDirective>> = None;
            let mut last_existing_package: Option<Id<ImportDirective>> = None;
            let mut is_last_existing_dart = false;
            let mut is_last_existing_package = false;
            // Dart `insert(prev:, next:, trailingNewLine:)`.
            let insert_at = |edits: &mut Vec<SourceEdit>, prev: Option<Id<ImportDirective>>, next: Id<ImportDirective>, trailing: bool| {
                if let Some(prev) = prev {
                    let mut offset = ast.end(prev);
                    let next_token = ast.t_next(ast.end_token(prev));
                    for comment in ast.tokens.comments(next_token) {
                        if line_info.on_same_line(ast.t_offset(comment), offset) {
                            offset = ast.t_end(comment);
                        }
                    }
                    insert(edits, offset, format!("{eol}{}", write_import(uri)));
                } else {
                    let is_first = ast.list_raw(ast[unit].directives).first() == Some(&next.raw());
                    let offset = if is_first {
                        crate::computer::outline::first_token_after_comment_and_metadata(ast, next.raw())
                            .map(|t| ast.t_offset(t))
                            .unwrap_or_else(|| ast.offset(next))
                    } else {
                        ast.offset(next)
                    };
                    let mut text = format!("{}{eol}", write_import(uri));
                    if trailing {
                        text.push_str(eol);
                    }
                    insert(edits, offset, text);
                }
            };
            for existing in &imports {
                let existing = *existing;
                let existing_uri = directive_uri_value(ast, ast[existing].uri);
                let is_existing_dart = existing_uri.starts_with("dart:");
                let is_existing_package = existing_uri.starts_with("package:");
                let is_existing_relative = !existing_uri.contains(':');
                let is_replacement = **uri == existing_uri && ast[existing].prefix.is_none();
                let is_new_before_existing = uri.as_str() < existing_uri.as_str();
                if is_replacement {
                    // Dart `updateHideCombinators`: the new import hides no
                    // names, so each hide combinator is removed.
                    for c in ast.list_raw(ast[existing].combinators) {
                        if ast.is::<HideCombinator>(*c) {
                            let begin = ast.begin_token(*c);
                            let previous_end = ast.t_prev(begin).map(|t| ast.t_end(t)).unwrap_or(0);
                            edits.push(SourceEdit { offset: previous_end, length: ast.end(*c) - previous_end, text: String::new() });
                        }
                    }
                    inserted = true;
                    break;
                } else if is_dart {
                    if !is_existing_dart || is_new_before_existing {
                        insert_at(&mut edits, last_existing_dart, existing, !is_existing_dart);
                        inserted = true;
                        break;
                    }
                } else if is_package {
                    if is_existing_relative || is_new_before_existing {
                        insert_at(&mut edits, last_existing_package, existing, is_existing_relative);
                        inserted = true;
                        break;
                    }
                } else if !is_existing_dart && !is_existing_package && is_new_before_existing {
                    insert_at(&mut edits, None, existing, false);
                    inserted = true;
                    break;
                }
                last_existing = Some(existing);
                if is_existing_dart {
                    last_existing_dart = Some(existing);
                } else if is_existing_package {
                    last_existing_package = Some(existing);
                }
                is_last_existing_dart = is_existing_dart;
                is_last_existing_package = is_existing_package;
            }
            if !inserted {
                let mut text = String::new();
                if is_package {
                    if is_last_existing_dart {
                        text.push_str(eol);
                    }
                } else if !is_dart && (is_last_existing_dart || is_last_existing_package) {
                    text.push_str(eol);
                }
                text.push_str(eol);
                text.push_str(&write_import(uri));
                insert(&mut edits, ast.end(last_existing.unwrap()), text);
            }
        }
    } else if let Some(library) = library_directive {
        let mut text = format!("{eol}{eol}");
        for (i, uri) in import_list.iter().enumerate() {
            text.push_str(&write_import(uri));
            if i != import_list.len() - 1 {
                text.push_str(eol);
            }
        }
        insert(&mut edits, ast.end(library), text);
    } else if let Some(first) = first_export.or(first_part) {
        let mut text = String::new();
        for uri in &import_list {
            text.push_str(&write_import(uri));
            text.push_str(eol);
        }
        text.push_str(eol);
        insert(&mut edits, ast.offset(first), text);
    } else {
        let declarations = ast.list_raw(ast[unit].declarations);
        let (offset, empty_line_after) = match declarations.first() {
            Some(d) => (ast.offset(*d), true),
            None => (ast.end(unit), false),
        };
        let mut text = String::new();
        for (i, uri) in import_list.iter().enumerate() {
            text.push_str(&write_import(uri));
            text.push_str(eol);
            if i == import_list.len() - 1 && empty_line_after {
                text.push_str(eol);
            }
        }
        insert(&mut edits, offset, text);
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.offset));
    edits
        .into_iter()
        .map(|e| json!({"range": to_range(line_info, e.offset, e.length), "newText": e.text}))
        .collect()
}
