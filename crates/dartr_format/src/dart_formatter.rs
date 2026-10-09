// Dart source: dart_style lib/src/dart_formatter.dart

//! The entry point of the formatter: parses the source, selects the style
//! from the language version and runs the front end of that style.

use dartr_ast::{Ast, BlockFunctionBody, FunctionDeclaration, NodeId};
use dartr_ast_builder::parse::parse_file_with_sdk_version;
use dartr_diagnostics::DiagnosticType;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{LineInfo, TokenType};

use crate::dart_version_history::{DartVersionHistory, Version};
use crate::exceptions::{
    FormatError, FormatterError, FormatterException, UnexpectedOutputException,
};
use crate::source_code::SourceCode;
use crate::string_compare;

/// Configuration for how trailing commas should be handled by the formatter.
///
/// Note that this only applies when using the new formatter to format code at
/// language version 3.7 or later. On older versions, it always behaves as if
/// it were [TrailingCommas::Preserve].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TrailingCommas {
    /// The formatter will add a trailing comma if a construct is split and
    /// remove the trailing comma and collapse the construct if it decides to do
    /// so.
    #[default]
    Automate,

    /// The formatter will add a trailing comma if a construct splits. If the
    /// construct has a trailing comma, it will always be forced to split and the
    /// trailing comma is preserved.
    Preserve,
}

/// A Dart source code formatter.
///
/// This is a lightweight class that mostly bundles formatting options so that
/// you don't have to pass a long argument list to [format()] and
/// [formatStatement()]. You can efficiently create a new instance of this for
/// every format invocation.
#[derive(Clone, Debug)]
pub struct DartFormatter {
    /// The Dart language version that formatted code should be parsed as.
    ///
    /// Note that a `// @dart=` comment inside the code overrides this.
    pub language_version: Version,

    /// The string that newlines should use.
    ///
    /// If not explicitly provided, this is inferred from the source text. If the
    /// first newline is `\r\n` (Windows), it will use that. Otherwise, it uses
    /// Unix-style line endings (`\n`).
    pub line_ending: Option<String>,

    /// The number of characters allowed in a single line.
    pub page_width: usize,

    /// The number of characters of indentation to prefix the output lines with.
    pub indent: usize,

    /// How trailing commas in various constructs should affect formatting.
    ///
    /// The default is [TrailingCommas::Automate] where the formatter is free to
    /// add and remove them if it decides a constructor should be split or
    /// collapsed.
    pub trailing_commas: TrailingCommas,

    /// Flags to enable experimental language features.
    ///
    /// See dart.dev/go/experiments for details.
    pub experiment_flags: Vec<String>,
}

/// Regular expression that matches a format width comment like:
///
///     // dart format width=123
///
/// Dart `RegExp(r'^// dart format width=(\d+)$')`. Returns the width text.
fn match_width_comment(lexeme: &str) -> Option<&str> {
    let digits = lexeme.strip_prefix("// dart format width=")?;
    // Dart `$` also matches before a final newline; comment lexemes never
    // contain one.
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(digits)
    } else {
        None
    }
}

impl DartFormatter {
    /// The latest Dart language version that can be parsed and formatted by this
    /// version of the formatter.
    pub const LATEST_LANGUAGE_VERSION: Version = DartVersionHistory::LATEST;

    /// The latest Dart language version that will be formatted using the older
    /// "short" style.
    ///
    /// Any Dart code at a language version later than this will be formatted
    /// using the new "tall" style.
    pub const LATEST_SHORT_STYLE_LANGUAGE_VERSION: Version = DartVersionHistory::LATEST_SHORT_STYLE;

    /// The page width that the formatter tries to fit code inside if no other
    /// width is specified.
    pub const DEFAULT_PAGE_WIDTH: usize = 80;

    /// Creates a new formatter for Dart code at [language_version] with the
    /// default options.
    pub fn new(language_version: Version) -> DartFormatter {
        DartFormatter {
            language_version,
            line_ending: None,
            page_width: Self::DEFAULT_PAGE_WIDTH,
            indent: 0,
            trailing_commas: TrailingCommas::Automate,
            experiment_flags: Vec::new(),
        }
    }

    /// Formats the given [source] string containing an entire Dart compilation
    /// unit.
    ///
    /// If [uri] is given, it is used to identify the file being formatted in
    /// error messages.
    pub fn format(&self, source: &str, uri: Option<&str>) -> Result<String, FormatError> {
        let code = SourceCode {
            uri: uri.map(str::to_string),
            text: source.to_string(),
            is_compilation_unit: true,
            selection_start: None,
            selection_length: None,
        };
        Ok(self.format_source(&code)?.text)
    }

    /// Formats the given [source] string containing a single Dart statement.
    pub fn format_statement(&self, source: &str) -> Result<String, FormatError> {
        let code = SourceCode {
            uri: None,
            text: source.to_string(),
            is_compilation_unit: false,
            selection_start: None,
            selection_length: None,
        };
        Ok(self.format_source(&code)?.text)
    }

    /// Formats the given [source].
    ///
    /// Returns a new [SourceCode] containing the formatted code and the resulting
    /// selection, if any.
    pub fn format_source(&self, source: &SourceCode) -> Result<SourceCode, FormatError> {
        let mut input_offset = 0;
        let mut text = source.text.clone();
        let mut unit_source_code = source.clone();

        // If we're parsing a single statement, wrap the source in a fake function.
        if !source.is_compilation_unit {
            let prefix = "void foo() { ";
            input_offset = prefix.len();
            text = format!("{prefix}{text}\n }}");
            unit_source_code = SourceCode {
                uri: source.uri.clone(),
                text: text.clone(),
                is_compilation_unit: false,
                selection_start: source.selection_start.map(|start| start + input_offset),
                selection_length: source.selection_length,
            };
        }

        let experiments = experiment_flags(&self.experiment_flags);

        // Parse it.
        let path = source.uri.clone().unwrap_or_default();
        // Dart `FeatureSet.fromEnableFlags2(sdkLanguageVersion:
        // languageVersion, flags: experimentFlags)`.
        let version = (self.language_version.major, self.language_version.minor);
        let parsed = parse_file_with_sdk_version(&text, &path, version, &experiments, version);

        // Infer the line ending if not given one. Do it here since now we know
        // where the lines start.
        let inferred_line_ending = match &self.line_ending {
            Some(ending) => ending.clone(),
            None => infer_line_ending(&parsed.line_info, &text).to_string(),
        };

        // Throw if there are syntactic errors.
        let syntactic_errors: Vec<FormatterError> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.code.diagnostic_type == DiagnosticType::SyntacticError)
            .map(|d| FormatterError {
                unique_name: d.code.unique_name.to_string(),
                offset: d.offset,
                length: d.length,
                message: d.message.clone(),
                source: text.clone(),
                path: path.clone(),
            })
            .collect();
        if !syntactic_errors.is_empty() {
            return Err(FormatError::Formatter(FormatterException {
                errors: syntactic_errors,
            }));
        }

        let ast: &Ast = &parsed.ast;
        let node: NodeId = if source.is_compilation_unit {
            parsed.unit.raw()
        } else {
            let unit = &ast[parsed.unit];
            let declaration = ast.list_raw(unit.declarations)[0];
            let function = ast
                .cast::<FunctionDeclaration>(declaration)
                .expect("wrapper is a function declaration");
            let expression = ast[function].function_expression;
            let body = ast
                .cast::<BlockFunctionBody>(ast[expression].body)
                .expect("wrapper has a block body");
            let block = ast[body].block;
            let statement = ast.list_raw(ast[block].statements)[0];

            // Make sure we consumed all of the source.
            let token = ast.tokens.next(ast.end_token(statement));
            if ast.tokens.ty(token) != TokenType::CLOSE_CURLY_BRACKET {
                let lexeme = ast.tokens.lexeme(token);
                let length = ast.tokens.get(token).length as usize;
                let message = format!("Unexpected token '{lexeme}'.");
                return Err(FormatError::Formatter(FormatterException {
                    errors: vec![FormatterError {
                        unique_name: "ParserErrorCode.UNEXPECTED_TOKEN".to_string(),
                        offset: (ast.tokens.offset(token) as usize).saturating_sub(input_offset),
                        length: length.max(1),
                        message,
                        source: text.clone(),
                        path: source.uri.clone().unwrap_or_default(),
                    }],
                }));
            }
            statement
        };

        // Format it.
        let line_info = &parsed.line_info;

        // If the code has an `@dart=` comment, use that to determine the style.
        let mut source_language_version = self.language_version;
        if let Some((major, minor)) = parsed.language_version.override_ {
            source_language_version = Version::new(major, minor);
        }

        // Use language version to determine what formatting style to apply.
        let output = if source_language_version > Self::LATEST_SHORT_STYLE_LANGUAGE_VERSION {
            // Look for a page width comment before the code.
            let mut page_width_from_comment = None;
            for comment in ast.tokens.comments(ast.begin_token(node)) {
                if let Some(width) = match_width_comment(ast.tokens.lexeme(comment)) {
                    // If integer parsing fails for some reason, the returned `None`
                    // means we correctly ignore the comment.
                    page_width_from_comment = width.parse::<usize>().ok();
                    break;
                }
            }

            crate::front_end::format_tall(
                self,
                &inferred_line_ending,
                source_language_version,
                page_width_from_comment,
                ast,
                line_info,
                &unit_source_code,
                node,
            )?
        } else {
            // Use the old style.
            crate::short::format_short(
                self,
                ast,
                line_info,
                &unit_source_code,
                node,
                &inferred_line_ending,
            )?
        };

        // Sanity check that only whitespace was changed if that's all we expect.
        if !string_compare::equal_ignoring_whitespace(&source.text, &output.text) {
            return Err(FormatError::UnexpectedOutput(UnexpectedOutputException {
                input: source.text.clone(),
                output: output.text,
            }));
        }

        Ok(output)
    }
}

/// Converts `--enable-experiment` names to flags. Unknown names are ignored
/// (Dart `FeatureSet.fromEnableFlags2` ignores them too).
pub fn experiment_flags(names: &[String]) -> Vec<ExperimentalFlag> {
    names
        .iter()
        .filter_map(|name| {
            ExperimentalFlag::VALUES
                .iter()
                .copied()
                .find(|flag| flag.name() == name)
        })
        .collect()
}

/// Infers the line endings in [source] from the ending of the first newline.
pub fn infer_line_ending(line_info: &LineInfo, source: &str) -> &'static str {
    // If the first newline is "\r\n", use that. Otherwise, use "\n".
    let line_starts = &line_info.line_starts;
    if line_starts.len() > 1 && line_starts[1] >= 2 {
        // Line starts are UTF-16 offsets; the character before the "\n" is at
        // `lineStarts[1] - 2`.
        let index = line_starts[1] as usize - 2;
        let byte = crate::text::byte_offset_of_utf16(source, index);
        if source.as_bytes().get(byte) == Some(&b'\r') {
            return "\r\n";
        }
    }
    "\n"
}
