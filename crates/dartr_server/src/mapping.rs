// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart
// Dart source: pkg/analysis_server/lib/src/lsp/constants.dart (ServerErrorCodes)
// Dart source: pkg/analysis_server/lib/src/lsp/error_or.dart

//! Conversion of analyzer data to LSP JSON: positions, ranges, diagnostics,
//! symbol kinds, and the LSP errors of the server.

use dartr_diagnostics::{Diagnostic, DiagnosticSeverity};
use dartr_syntax::LineInfo;
use serde_json::{Map, Value, json};

use crate::uri::path_to_uri;

/// LSP `ErrorCodes` and Dart `ServerErrorCodes`.
pub mod codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const SERVER_NOT_INITIALIZED: i64 = -32002;
    pub const REQUEST_CANCELLED: i64 = -32800;
    pub const CONTENT_MODIFIED: i64 = -32801;
    // ServerErrorCodes.
    pub const UNHANDLED_ERROR: i64 = -32001;
    pub const SERVER_ALREADY_INITIALIZED: i64 = -32002;
    pub const INVALID_FILE_PATH: i64 = -32003;
    pub const INVALID_FILE_LINE_COL: i64 = -32004;
    pub const UNKNOWN_COMMAND: i64 = -32005;
    pub const INVALID_COMMAND_ARGUMENTS: i64 = -32006;
    pub const FILE_NOT_ANALYZED: i64 = -32007;
    pub const FILE_HAS_ERRORS: i64 = -32008;
    pub const CLIENT_FAILED_TO_APPLY_EDIT: i64 = -32009;
    pub const CLIENT_SERVER_INCONSISTENT_STATE: i64 = -32099;
}

/// LSP `ResponseError`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    pub data: Option<String>,
}

impl ResponseError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        ResponseError {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i64, message: impl Into<String>, data: impl Into<String>) -> Self {
        ResponseError {
            code,
            message: message.into(),
            data: Some(data.into()),
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("code".into(), json!(self.code));
        m.insert("message".into(), json!(self.message));
        if let Some(d) = &self.data {
            m.insert("data".into(), json!(d));
        }
        Value::Object(m)
    }
}

pub type ErrorOr<T> = Result<T, ResponseError>;

/// Dart `toPosition` of `lineInfo.getLocation(offset)`.
pub fn to_position(line_info: &LineInfo, offset: u32) -> Value {
    let loc = line_info.get_location(offset);
    json!({"line": loc.line_number - 1, "character": loc.column_number - 1})
}

/// Dart `toRange`.
pub fn to_range(line_info: &LineInfo, offset: u32, length: u32) -> Value {
    json!({
        "start": to_position(line_info, offset),
        "end": to_position(line_info, offset + length),
    })
}

/// Reads an LSP `Position` (`line`, `character`).
pub fn read_position(v: &Value) -> Option<(u32, u32)> {
    Some((
        v.get("line")?.as_u64()? as u32,
        v.get("character")?.as_u64()? as u32,
    ))
}

/// Dart `toOffset`: the UTF-16 offset of an LSP position. The character is
/// not checked, like in Dart.
pub fn to_offset(line_info: &LineInfo, line: u32, character: u32, critical: bool) -> ErrorOr<u32> {
    if line as usize >= line_info.line_count() {
        return Err(ResponseError::with_data(
            if critical {
                codes::CLIENT_SERVER_INCONSISTENT_STATE
            } else {
                codes::INVALID_FILE_LINE_COL
            },
            "Invalid line number",
            line.to_string(),
        ));
    }
    Ok(line_info.line_starts[line as usize] + character)
}

/// The client capabilities that the diagnostic conversion reads.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticOptions {
    /// `textDocument.publishDiagnostics.tagSupport.valueSet`.
    pub supported_tags: Option<Vec<i64>>,
    /// `textDocument.publishDiagnostics.codeDescriptionSupport`.
    pub code_description: bool,
}

/// Dart `diagnosticTagsForErrorCode`.
fn diagnostic_tags(code: &str) -> &'static [i64] {
    const UNNECESSARY: i64 = 1;
    const DEPRECATED: i64 = 2;
    match code {
        "dead_code" => &[UNNECESSARY],
        "deprecated_member_use"
        | "deprecated_member_use_from_same_package"
        | "deprecated_member_use_from_same_package_with_message"
        | "deprecated_member_use_with_message" => &[DEPRECATED],
        _ => &[],
    }
}

/// Dart `pluginToDiagnosticSeverity`.
fn severity(s: DiagnosticSeverity) -> i64 {
    match s {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Info | DiagnosticSeverity::None => 3,
    }
}

/// Dart `toDiagnostic` / `pluginToDiagnostic` for a diagnostic of the file
/// with [line_info]. [other_line_info] returns the line info of the files
/// of context messages in other files.
pub fn to_diagnostic(
    line_info: &LineInfo,
    path: &str,
    d: &Diagnostic,
    options: &DiagnosticOptions,
    other_line_info: &dyn Fn(&str) -> Option<LineInfo>,
) -> Value {
    let mut m = Map::new();
    m.insert(
        "range".into(),
        to_range(line_info, d.offset as u32, d.length as u32),
    );
    m.insert("severity".into(), json!(severity(d.severity)));
    m.insert("code".into(), json!(d.code.name));
    m.insert("source".into(), json!("dart"));
    let message = match &d.correction {
        Some(c) => format!("{}\n{}", d.message, c),
        None => d.message.clone(),
    };
    m.insert("message".into(), json!(message));
    if let Some(supported) = &options.supported_tags {
        let tags: Vec<i64> = diagnostic_tags(d.code.name)
            .iter()
            .copied()
            .filter(|t| supported.contains(t))
            .collect();
        if !tags.is_empty() {
            m.insert("tags".into(), json!(tags));
        }
    }
    if !d.context_messages.is_empty() {
        let mut related = Vec::new();
        for c in &d.context_messages {
            let info = if c.file_path == path {
                Some(line_info.clone())
            } else {
                other_line_info(&c.file_path)
            };
            if let Some(info) = info {
                related.push(json!({
                    "location": {
                        "uri": path_to_uri(&c.file_path),
                        "range": to_range(&info, c.offset as u32, c.length as u32),
                    },
                    "message": c.message,
                }));
            }
        }
        m.insert("relatedInformation".into(), Value::Array(related));
    }
    if options.code_description {
        if let Some(url) = d.code.url() {
            m.insert("codeDescription".into(), json!({"href": url}));
        }
    }
    Value::Object(m)
}

/// The analysis server `ElementKind` (`analyzer_plugin` protocol).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    Class,
    ClassTypeAlias,
    CompilationUnit,
    Constructor,
    ConstructorInvocation,
    Enum,
    EnumConstant,
    Extension,
    ExtensionType,
    Field,
    Function,
    FunctionTypeAlias,
    Getter,
    Method,
    Mixin,
    Setter,
    TopLevelVariable,
    TypeAlias,
    UnitTestGroup,
    UnitTestTest,
}

impl ElementKind {
    pub fn name(self) -> &'static str {
        match self {
            ElementKind::Class => "CLASS",
            ElementKind::ClassTypeAlias => "CLASS_TYPE_ALIAS",
            ElementKind::CompilationUnit => "COMPILATION_UNIT",
            ElementKind::Constructor => "CONSTRUCTOR",
            ElementKind::ConstructorInvocation => "CONSTRUCTOR_INVOCATION",
            ElementKind::Enum => "ENUM",
            ElementKind::EnumConstant => "ENUM_CONSTANT",
            ElementKind::Extension => "EXTENSION",
            ElementKind::ExtensionType => "EXTENSION_TYPE",
            ElementKind::Field => "FIELD",
            ElementKind::Function => "FUNCTION",
            ElementKind::FunctionTypeAlias => "FUNCTION_TYPE_ALIAS",
            ElementKind::Getter => "GETTER",
            ElementKind::Method => "METHOD",
            ElementKind::Mixin => "MIXIN",
            ElementKind::Setter => "SETTER",
            ElementKind::TopLevelVariable => "TOP_LEVEL_VARIABLE",
            ElementKind::TypeAlias => "TYPE_ALIAS",
            ElementKind::UnitTestGroup => "UNIT_TEST_GROUP",
            ElementKind::UnitTestTest => "UNIT_TEST_TEST",
        }
    }
}

/// LSP `SymbolKind` values.
pub mod symbol_kind {
    pub const FILE: i64 = 1;
    pub const NAMESPACE: i64 = 3;
    pub const CLASS: i64 = 5;
    pub const METHOD: i64 = 6;
    pub const PROPERTY: i64 = 7;
    pub const FIELD: i64 = 8;
    pub const CONSTRUCTOR: i64 = 9;
    pub const ENUM: i64 = 10;
    pub const FUNCTION: i64 = 12;
    pub const VARIABLE: i64 = 13;
    pub const OBJECT: i64 = 19;
    pub const ENUM_MEMBER: i64 = 22;
    pub const TYPE_PARAMETER: i64 = 26;
}

/// The default `SymbolKind`s of a client that does not list its supported
/// kinds (Dart `defaultSupportedSymbolKinds`, `File` to `Array`).
pub const DEFAULT_SYMBOL_KINDS: std::ops::RangeInclusive<i64> = 1..=18;

/// Dart `elementKindToSymbolKind`.
pub fn element_kind_to_symbol_kind(supported: &[i64], kind: ElementKind) -> i64 {
    use symbol_kind::*;
    let preferences: &[i64] = match kind {
        ElementKind::Class | ElementKind::ClassTypeAlias => &[CLASS],
        ElementKind::CompilationUnit => &[FILE],
        ElementKind::Constructor | ElementKind::ConstructorInvocation => &[CONSTRUCTOR],
        ElementKind::Enum => &[ENUM],
        ElementKind::EnumConstant => &[ENUM_MEMBER, ENUM],
        ElementKind::Extension | ElementKind::ExtensionType => &[NAMESPACE],
        ElementKind::Field => &[FIELD],
        ElementKind::Function => &[FUNCTION],
        ElementKind::FunctionTypeAlias => &[CLASS],
        ElementKind::Getter | ElementKind::Setter => &[PROPERTY],
        ElementKind::Method => &[METHOD],
        ElementKind::Mixin => &[CLASS],
        ElementKind::TopLevelVariable => &[VARIABLE],
        ElementKind::UnitTestGroup | ElementKind::UnitTestTest => &[METHOD],
        // `TYPE_ALIAS` is not in the Dart switch: it falls to the default
        // (`null` kind), which maps to `Obj`.
        ElementKind::TypeAlias => &[],
    };
    // Dart: the first supported preference, or `Obj` (which every client
    // supports, because it is in the default set).
    preferences
        .iter()
        .copied()
        .find(|k| supported.contains(k))
        .unwrap_or(OBJECT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets() {
        let info = LineInfo::from_content("ab\ncd\n");
        assert_eq!(to_offset(&info, 1, 1, false), Ok(4));
        let err = to_offset(&info, 9, 0, false).unwrap_err();
        assert_eq!(
            err.to_json(),
            json!({"code": -32004, "message": "Invalid line number", "data": "9"})
        );
        assert_eq!(
            to_range(&info, 1, 3),
            json!({"start": {"line": 0, "character": 1}, "end": {"line": 1, "character": 1}})
        );
    }
}
