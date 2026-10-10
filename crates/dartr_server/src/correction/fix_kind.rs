// Dart source: pkg/analyzer_plugin/lib/utilities/fixes/fixes.dart (FixKind)
// Dart source: pkg/analysis_server_plugin/lib/edit/dart/dart_fix_kind_priority.dart
// Dart source: pkg/analysis_server_plugin/lib/src/correction/ignore_diagnostic.dart (ignoreErrorAnalysisFileKind, ignoreErrorFileKind, ignoreErrorLineKind)
// Dart source: pkg/_fe_analyzer_shared/lib/src/base/errors.dart (formatList)

//! Dart `FixKind`: the id, priority and message template of a fix.

/// Dart `FixKind`.
#[derive(Debug, PartialEq, Eq)]
pub struct FixKind {
    pub id: &'static str,
    pub priority: i64,
    pub message: &'static str,
}

/// Dart `DartFixKindPriority`.
pub mod priority {
    pub const STANDARD: i64 = 50;
    pub const IN_FILE: i64 = 40;
    pub const IGNORE: i64 = 30;
}

/// Dart `ignoreErrorAnalysisFileKind`.
pub const IGNORE_ERROR_ANALYSIS_FILE: FixKind = FixKind {
    id: "dart.fix.ignore.analysis",
    priority: priority::IGNORE - 2,
    message: "Ignore '{0}' in `analysis_options.yaml`",
};

/// Dart `ignoreErrorFileKind`.
pub const IGNORE_ERROR_FILE: FixKind = FixKind {
    id: "dart.fix.ignore.file",
    priority: priority::IGNORE - 1,
    message: "Ignore '{0}' for the whole file",
};

/// Dart `ignoreErrorLineKind`.
pub const IGNORE_ERROR_LINE: FixKind = FixKind {
    id: "dart.fix.ignore.line",
    priority: priority::IGNORE,
    message: "Ignore '{0}' for this line",
};

/// Dart `formatList`: replaces `{n}` with `arguments[n]`.
pub fn format_list(pattern: &str, arguments: &[String]) -> String {
    if arguments.is_empty() {
        return pattern.to_string();
    }
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let mut j = i + 1;
            let mut number = 0usize;
            while j < chars.len() && chars[j].is_ascii_digit() {
                number = number * 10 + chars[j].to_digit(10).unwrap() as usize;
                j += 1;
            }
            if j < chars.len() && chars[j] == '}' && j > i + 1 {
                if let Some(a) = arguments.get(number) {
                    out.push_str(a);
                }
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
