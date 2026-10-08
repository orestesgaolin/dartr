//! Diagnostic codes, types and severities.
//!
//! Port of `DiagnosticCode`, `DiagnosticSeverity` and `DiagnosticType` in
//! `pkg/_fe_analyzer_shared/lib/src/base/errors.dart`, and of
//! `DiagnosticParameterType` in `pkg/analyzer_utilities/lib/messages.dart`.

use crate::generated::diag;

/// The severity of a diagnostic (`DiagnosticSeverity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    None,
    Info,
    Warning,
    Error,
}

impl DiagnosticSeverity {
    /// `DiagnosticSeverity.name`: `NONE`, `INFO`, `WARNING`, `ERROR`.
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticSeverity::None => "NONE",
            DiagnosticSeverity::Info => "INFO",
            DiagnosticSeverity::Warning => "WARNING",
            DiagnosticSeverity::Error => "ERROR",
        }
    }

    pub fn ordinal(self) -> u8 {
        self as u8
    }

    /// The name used in machine output (`" "`, `I`, `W`, `E`).
    pub fn machine_code(self) -> &'static str {
        match self {
            DiagnosticSeverity::None => " ",
            DiagnosticSeverity::Info => "I",
            DiagnosticSeverity::Warning => "W",
            DiagnosticSeverity::Error => "E",
        }
    }

    /// The name used in readable output.
    pub fn display_name(self) -> &'static str {
        match self {
            DiagnosticSeverity::None => "none",
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Error => "error",
        }
    }
}

/// The type of a diagnostic code (`DiagnosticType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticType {
    Todo,
    Hint,
    CompileTimeError,
    CheckedModeCompileTimeError,
    StaticWarning,
    SyntacticError,
    Lint,
}

impl DiagnosticType {
    /// `DiagnosticType.name`, for example `COMPILE_TIME_ERROR`.
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticType::Todo => "TODO",
            DiagnosticType::Hint => "HINT",
            DiagnosticType::CompileTimeError => "COMPILE_TIME_ERROR",
            DiagnosticType::CheckedModeCompileTimeError => "CHECKED_MODE_COMPILE_TIME_ERROR",
            DiagnosticType::StaticWarning => "STATIC_WARNING",
            DiagnosticType::SyntacticError => "SYNTACTIC_ERROR",
            DiagnosticType::Lint => "LINT",
        }
    }

    /// `DiagnosticType.ordinal` (there is no ordinal 5).
    pub fn ordinal(self) -> u8 {
        match self {
            DiagnosticType::Todo => 0,
            DiagnosticType::Hint => 1,
            DiagnosticType::CompileTimeError => 2,
            DiagnosticType::CheckedModeCompileTimeError => 3,
            DiagnosticType::StaticWarning => 4,
            DiagnosticType::SyntacticError => 6,
            DiagnosticType::Lint => 7,
        }
    }

    /// The default severity of diagnostics of this type.
    pub fn severity(self) -> DiagnosticSeverity {
        match self {
            DiagnosticType::Todo | DiagnosticType::Hint | DiagnosticType::Lint => {
                DiagnosticSeverity::Info
            }
            DiagnosticType::CompileTimeError
            | DiagnosticType::CheckedModeCompileTimeError
            | DiagnosticType::SyntacticError => DiagnosticSeverity::Error,
            DiagnosticType::StaticWarning => DiagnosticSeverity::Warning,
        }
    }

    /// `DiagnosticType.displayName`, for example `compile time error`.
    pub fn display_name(self) -> String {
        self.name().to_lowercase().replace('_', " ")
    }
}

/// The type of a diagnostic parameter as written in `messages.yaml`
/// (`DiagnosticParameterType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterType {
    Character,
    Constant,
    Element,
    Int,
    Name,
    NameOkEmpty,
    Names,
    Num,
    Object,
    String,
    StringOkEmpty,
    Token,
    Type,
    Unicode,
    Uri,
}

impl ParameterType {
    /// The name used in `messages.yaml`.
    pub fn yaml_name(self) -> &'static str {
        match self {
            ParameterType::Character => "Character",
            ParameterType::Constant => "Constant",
            ParameterType::Element => "Element",
            ParameterType::Int => "int",
            ParameterType::Name => "Name",
            ParameterType::NameOkEmpty => "NameOKEmpty",
            ParameterType::Names => "Names",
            ParameterType::Num => "num",
            ParameterType::Object => "Object",
            ParameterType::String => "String",
            ParameterType::StringOkEmpty => "StringOKEmpty",
            ParameterType::Token => "Token",
            ParameterType::Type => "Type",
            ParameterType::Unicode => "Unicode",
            ParameterType::Uri => "Uri",
        }
    }

    /// The name of the analyzer `ExpectedType` value used to check the
    /// arguments of a code (`DiagnosticCodeWithExpectedTypes.expectedTypes`).
    pub fn expected_type(self) -> &'static str {
        match self {
            ParameterType::Character => "character",
            ParameterType::Constant => "constant",
            ParameterType::Element => "element",
            ParameterType::Int => "int",
            ParameterType::Name => "name",
            ParameterType::NameOkEmpty => "nameOKEmpty",
            ParameterType::Names => "names",
            ParameterType::Num => "num",
            ParameterType::Object => "object",
            ParameterType::String => "string",
            ParameterType::StringOkEmpty => "stringOKEmpty",
            ParameterType::Token => "token",
            ParameterType::Type => "type",
            ParameterType::Unicode => "unicode",
            ParameterType::Uri => "uri",
        }
    }
}

/// A parameter of a diagnostic code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// The name in `messages.yaml` (camelCase).
    pub name: &'static str,
    pub ty: ParameterType,
}

/// The `messages.yaml` file that defines a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    /// `pkg/_fe_analyzer_shared/messages.yaml` (codes shared with the CFE).
    FeAnalyzerShared,
    /// `pkg/analyzer/messages.yaml`.
    Analyzer,
    /// `pkg/linter/messages.yaml`.
    Linter,
    /// `pkg/analysis_server/messages.yaml`.
    AnalysisServer,
}

/// A diagnostic code (`DiagnosticCode`). One static value per code is
/// generated in [`crate::diag`].
#[derive(Debug)]
pub struct DiagnosticCode {
    /// The name shown to users and used in `// ignore:` comments. Codes with
    /// a `sharedName` share this name. Always lower snake case, so this is
    /// also Dart's `lowerCaseName`.
    pub name: &'static str,
    /// The unique name of the code (lower snake case), Dart's `uniqueName`
    /// and `lowerCaseUniqueName`.
    pub unique_name: &'static str,
    /// The name of the constant in Dart (`diag.<camelCaseName>`).
    pub camel_case_name: &'static str,
    /// The problem message template, with `{0}` style placeholders.
    pub problem_message: &'static str,
    /// The correction message template, with `{0}` style placeholders.
    pub correction_message: Option<&'static str>,
    pub diagnostic_type: DiagnosticType,
    pub has_published_docs: bool,
    pub is_unresolved_identifier: bool,
    /// The parameters, in placeholder index order.
    pub parameters: &'static [Parameter],
    pub origin: Origin,
    pub deprecated_message: Option<&'static str>,
}

impl DiagnosticCode {
    /// Dart's `lowerCaseName`.
    pub fn lower_case_name(&self) -> &'static str {
        self.name
    }

    /// Dart's `lowerCaseUniqueName`.
    pub fn lower_case_unique_name(&self) -> &'static str {
        self.unique_name
    }

    /// The default severity. For all generated codes this is the severity of
    /// the type (`DiagnosticCodeImpl.severity`).
    pub fn severity(&self) -> DiagnosticSeverity {
        self.diagnostic_type.severity()
    }

    /// Whether `// ignore:` comments can suppress the diagnostic.
    pub fn is_ignorable(&self) -> bool {
        self.severity() != DiagnosticSeverity::Error
    }

    /// `numParameters`: one more than the largest `{n}` placeholder index.
    pub fn num_parameters(&self) -> usize {
        let mut result = 0;
        for s in std::iter::once(self.problem_message).chain(self.correction_message) {
            let b = s.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'{' {
                    let mut j = i + 1;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > i + 1 && j < b.len() && b[j] == b'}' {
                        let n: usize = s[i + 1..j].parse().unwrap_or(0);
                        result = result.max(n + 1);
                        i = j;
                    }
                }
                i += 1;
            }
        }
        result
    }

    /// The URL of the documentation of the code, or `None`.
    ///
    /// Lint codes (`LinterLintCode.url`) always have a URL.
    pub fn url(&self) -> Option<String> {
        if self.has_published_docs {
            Some(format!("https://dart.dev/diagnostics/{}", self.name))
        } else if self.diagnostic_type == DiagnosticType::Lint && self.origin == Origin::Linter {
            Some(format!("https://dart.dev/lints/{}", self.name))
        } else {
            None
        }
    }
}

impl PartialEq for DiagnosticCode {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || self.unique_name == other.unique_name
    }
}

impl Eq for DiagnosticCode {}

impl std::hash::Hash for DiagnosticCode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.unique_name.hash(state);
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.unique_name)
    }
}

/// A code that has a `removedIn` entry in `messages.yaml`. The analyzer
/// does not report it and does not define a constant for it.
#[derive(Debug)]
pub struct RemovedDiagnosticCode {
    pub name: &'static str,
    pub unique_name: &'static str,
    pub removed_in: &'static str,
    pub previous_name: Option<&'static str>,
}

/// All active codes, sorted by unique name.
pub fn all_codes() -> &'static [&'static DiagnosticCode] {
    &diag::ALL_CODES
}

/// Finds a code by its unique name. The match ignores case (Dart compares
/// `lowerCaseUniqueName`).
pub fn code_by_unique_name(unique_name: &str) -> Option<&'static DiagnosticCode> {
    let key = unique_name.to_ascii_lowercase();
    diag::ALL_CODES
        .binary_search_by(|c| c.unique_name.cmp(key.as_str()))
        .ok()
        .map(|i| diag::ALL_CODES[i])
}

/// Finds the codes with the given name (`lowerCaseName`). More than one code
/// can share a name. The match ignores case.
pub fn codes_by_name(name: &str) -> &'static [&'static DiagnosticCode] {
    let key = name.to_ascii_lowercase();
    match diag::CODES_BY_NAME.binary_search_by(|(n, _)| (*n).cmp(key.as_str())) {
        Ok(i) => diag::CODES_BY_NAME[i].1,
        Err(_) => &[],
    }
}

/// Codes with a `removedIn` entry, sorted by unique name.
pub fn removed_codes() -> &'static [RemovedDiagnosticCode] {
    &diag::REMOVED_CODES
}
