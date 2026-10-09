// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_formatting.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_format_range.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_format_on_type.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/formatter.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/extensions/formatter_options.dart

//! `textDocument/formatting`, `textDocument/rangeFormatting` and
//! `textDocument/onTypeFormatting`: the formatter of a parsed unit (Dart
//! `createFormatter`, `formatSafely`) and the trigger check of on-type
//! formatting. The server checks the document, the configuration and the
//! parse diagnostics (see `Server::format_request`); the edits are computed
//! in [`crate::source_edits`].

use dartr_ast::*;
use dartr_format::{DartFormatter, SourceCode, TrailingCommas, Version};
use dartr_parser::ExperimentalFlag;
use dartr_project::analysis_options::TrailingCommas as OptionsTrailingCommas;
use serde_json::Value;

use crate::mapping::{ErrorOr, read_position};
use crate::server::ParsedFile;
use crate::source_edits::generate_edits_for_formatting;

/// Dart `FormatterOptions` of the analysis options of a file
/// (`formatter: page_width, trailing_commas`).
#[derive(Clone, Copy, Debug, Default)]
pub struct FormatterOptions {
    pub page_width: Option<i64>,
    pub trailing_commas: Option<OptionsTrailingCommas>,
}

/// Dart `FormatterOptionsExtension.dartStyleTrailingCommas`.
fn dart_style_trailing_commas(t: OptionsTrailingCommas) -> TrailingCommas {
    match t {
        OptionsTrailingCommas::Automate => TrailingCommas::Automate,
        OptionsTrailingCommas::Preserve => TrailingCommas::Preserve,
    }
}

/// Dart `_getExperiments`: the experiments of the features that are enabled
/// in the unit and are not released yet (Dart `FeatureStatus.future`).
fn get_experiments(file: &ParsedFile) -> Vec<String> {
    ExperimentalFlag::VALUES
        .iter()
        .filter(|f| !f.is_enabled_by_default() && !f.is_expired())
        .filter(|f| file.unit.feature_set.is_experiment_enabled(**f))
        .map(|f| f.name().to_string())
        .collect()
}

/// Dart `createFormatter`: a formatter with the settings of [file]. The
/// page width of the analysis options wins over [default_page_width] (the
/// `dart.lineLength` setting of the client).
pub fn create_formatter(
    file: &ParsedFile,
    options: &FormatterOptions,
    default_page_width: Option<i64>,
) -> DartFormatter {
    let (major, minor) = file.unit.language_version.effective();
    let mut formatter = DartFormatter::new(Version::new(major, minor));
    if let Some(width) = options.page_width.or(default_page_width) {
        formatter.page_width = width.max(0) as usize;
    }
    if let Some(t) = options.trailing_commas {
        formatter.trailing_commas = dart_style_trailing_commas(t);
    }
    formatter.experiment_flags = get_experiments(file);
    formatter
}

/// Dart `DartFormatterExtension.formatSafely`: the formatted content, or
/// the content if formatting failed for any reason (for example a parse
/// error).
pub fn format_safely(formatter: &DartFormatter, content: &str) -> String {
    match formatter.format_source(&SourceCode::unit(content)) {
        Ok(result) => result.text,
        Err(_) => content.to_string(),
    }
}

/// Dart `generateEditsForFormatting(result, defaultPageWidth:, range:)`:
/// the edits that format [file], `null` if nothing changes.
pub fn format_file(
    file: &ParsedFile,
    options: &FormatterOptions,
    default_page_width: Option<i64>,
    range: Option<&Value>,
) -> ErrorOr<Value> {
    let formatter = create_formatter(file, options, default_page_width);
    let formatted = format_safely(&formatter, &file.content);
    generate_edits_for_formatting(
        &file.content,
        &file.unit.line_info,
        &formatted,
        file.unit.feature_set.build_scanner_configuration(),
        range,
    )
}

/// Dart `dartTypeFormattingCharacters`: the trigger characters of on-type
/// formatting.
pub const DART_TYPE_FORMATTING_CHARACTERS: [&str; 2] = ["}", ";"];

/// Dart `FormatOnTypeHandler._shouldTriggerFormatting`: whether
/// [character] at [position] should trigger formatting. Generally only a
/// `;` at the end of a statement or declaration and a `}` at the end of a
/// block, not inside something like a string or comment. The error is the
/// message of the Dart `ArgumentError` for a line after the end.
pub fn should_trigger_formatting(
    file: &ParsedFile,
    position: &Value,
    character: &str,
) -> Result<bool, String> {
    let unit = &file.unit;
    let ast = &unit.ast;
    let (line, column) = read_position(position).unwrap_or((0, 0));
    let offset = unit.line_info.get_offset_of_line(line as usize)? + column;
    let Some(node) = ast.node_covering(unit.unit, offset, 0) else {
        return Ok(false);
    };

    // Check both offset and end because the LSP spec says
    // >> This is not necessarily the exact position where the character
    // >> denoted by the property `ch` got typed.
    // and in testing with VS Code it's the end.
    let is_at_offset = |token: dartr_syntax::TokenId| {
        let t = ast.tokens.get(token);
        t.offset == offset || t.end() == offset
    };

    macro_rules! token_of {
        ($node:expr, $field:ident: $($ty:ty),+) => {{
            let mut found = None;
            $(
                if found.is_none() {
                    if let Some(n) = ast.cast::<$ty>($node) {
                        found = Some(ast[n].$field);
                    }
                }
            )+
            found
        }};
    }

    let token = match character {
        // Only consider semicolons that are the end of statements and
        // declarations.
        ";" => token_of!(node, semicolon:
            // Statements
            AssertStatement, BreakStatement, ContinueStatement, DoStatement,
            EmptyStatement, PatternVariableDeclarationStatement, ReturnStatement,
            VariableDeclarationStatement, YieldStatement,
            // Bodies
            EmptyClassBody, EmptyFunctionBody, NativeFunctionBody,
            // Declarations
            FieldDeclaration, TopLevelVariableDeclaration, FunctionTypeAlias,
            GenericTypeAlias,
            // Directives
            LibraryDirective, ImportDirective, ExportDirective, PartDirective,
            PartOfDirective)
        .or_else(|| {
            token_of!(node, semicolon: ExpressionStatement, ExpressionFunctionBody).flatten()
        }),
        // Only consider closing braces that are the end of "blocks" but not
        // things like patterns that might usually be inline.
        "}" => token_of!(node, right_bracket:
            Block, BlockClassBody, BlockEnumBody, ListLiteral, SetOrMapLiteral,
            SwitchExpression, SwitchStatement),
        _ => None,
    };
    Ok(token.is_some_and(is_at_offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The positions where `dart language-server` 3.13.3 formats on type
    /// (the response of `textDocument/onTypeFormatting` is not `null`) for
    /// each `;` and `}` of [SOURCE], at the character and after it.
    const SOURCE: &str = "import 'dart:math';
enum E { a, b }
class A{
int x=1  ;
  void f( int a,int b ){ if(a>b){print( 'a;}' );} else {print('b');}
  for (;;) { break; }
  switch (a) { case 1: return; }
  var s = {1, 2};
  var r = switch (a) { _ => 1 };
  // comment; }
  }
  List<int> get items => [1,2,3,];
  void g();
  typedef_() => max(1,2);
}
typedef T = int;
void main(){var a=A();a.f(1,2);}
";

    const DART_TRIGGERS: &[(u32, u32, &str)] = &[
        (0, 19, ";"),
        (1, 14, "}"),
        (1, 15, "}"),
        (3, 9, ";"),
        (3, 10, ";"),
        (4, 48, ";"),
        (4, 49, "}"),
        (4, 67, ";"),
        (4, 68, "}"),
        (5, 18, ";"),
        (5, 19, ";"),
        (5, 20, "}"),
        (5, 21, "}"),
        (6, 29, ";"),
        (6, 30, ";"),
        (6, 31, "}"),
        (6, 32, "}"),
        (7, 16, "}"),
        (7, 17, ";"),
        (8, 30, "}"),
        (8, 31, "}"),
        (8, 32, ";"),
        (10, 2, "}"),
        (10, 3, "}"),
        (11, 34, ";"),
        (12, 11, ";"),
        (13, 25, ";"),
        (14, 0, "}"),
        (14, 1, "}"),
        (15, 16, ";"),
        (16, 31, ";"),
        (16, 32, "}"),
    ];

    #[test]
    fn on_type_triggers_match_dart() {
        let file = ParsedFile {
            content: SOURCE.to_string(),
            unit: dartr_ast_builder::parse_file(SOURCE, "/p/lib/ontype.dart", (3, 13), &[]),
        };
        let mut triggers = Vec::new();
        for (l, line) in SOURCE.lines().enumerate() {
            for (c, ch) in line.char_indices() {
                if ch == ';' || ch == '}' {
                    let ch = ch.to_string();
                    for character in [c, c + 1] {
                        let position = json!({"line": l, "character": character});
                        if should_trigger_formatting(&file, &position, &ch).unwrap() {
                            triggers.push((l as u32, character as u32, ch.clone()));
                        }
                    }
                }
            }
        }
        let expected: Vec<(u32, u32, String)> = DART_TRIGGERS
            .iter()
            .map(|&(l, c, ch)| (l, c, ch.to_string()))
            .collect();
        assert_eq!(triggers, expected);
        // Other characters never trigger; a line after the end is an error.
        assert!(
            !should_trigger_formatting(&file, &json!({"line": 3, "character": 9}), "x").unwrap()
        );
        assert!(
            should_trigger_formatting(&file, &json!({"line": 900, "character": 0}), ";").is_err()
        );
    }
}
