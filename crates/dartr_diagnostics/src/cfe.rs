//! CFE ("fasta") messages that the shared scanner and parser report.
//!
//! Port of `Code`, `Message`, `Template` (`pkg/_fe_analyzer_shared/lib/src/messages/codes.dart`),
//! `CfeSeverity` (`severity.dart`) and the argument conversions
//! (`conversions.dart`).
//!
//! The parser reports a [`CfeMessage`]. The analyzer then maps it to an
//! analyzer [`DiagnosticCode`]:
//!
//! - if [`CfeCode::shared_code`] is set, the analyzer code is
//!   [`SharedCode::analyzer_code`], formatted with the raw arguments of the
//!   message ([`CfeMessage::analyzer_arguments`]), see
//!   `FastaErrorReporter.reportMessage`;
//! - else [`CfeCode::pseudo_shared_code`] selects hand-written translation
//!   logic (`error_converter.dart`, `translate_error_token.dart`), which the
//!   parser crate ports.

use crate::args::DiagnosticArg;
use crate::code::{DiagnosticCode, Parameter, ParameterType};
use crate::diagnostic::Diagnostic;
pub use crate::generated::cfe_codes::{PseudoSharedCode, SharedCode};

/// `CfeSeverity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CfeSeverity {
    Context,
    Error,
    Ignored,
    InternalProblem,
    Warning,
    Info,
}

impl CfeSeverity {
    /// The Dart enum value name (`internalProblem`, ...).
    pub fn name(self) -> &'static str {
        match self {
            CfeSeverity::Context => "context",
            CfeSeverity::Error => "error",
            CfeSeverity::Ignored => "ignored",
            CfeSeverity::InternalProblem => "internalProblem",
            CfeSeverity::Warning => "warning",
            CfeSeverity::Info => "info",
        }
    }
}

/// Formatting of a `num` placeholder such as `#count%3.1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericConversion {
    pub fraction_digits: Option<u32>,
    pub pad_width: u32,
    pub pad_with_zeros: bool,
}

/// A part of a CFE message template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplatePart {
    Literal(&'static str),
    Argument {
        index: u8,
        conversion: Option<NumericConversion>,
    },
}

/// A CFE code (`Code` with its `MessageCode`/`Template`).
#[derive(Debug)]
pub struct CfeCode {
    /// PascalCase name, for example `AsciiControlCharacter`.
    pub name: &'static str,
    pub problem_message: &'static [TemplatePart],
    pub correction_message: Option<&'static [TemplatePart]>,
    pub severity: CfeSeverity,
    pub shared_code: Option<SharedCode>,
    pub pseudo_shared_code: Option<PseudoSharedCode>,
    pub parameters: &'static [Parameter],
}

impl PartialEq for CfeCode {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || self.name == other.name
    }
}

impl Eq for CfeCode {}

/// An argument of a CFE message, before conversion.
#[derive(Clone, Debug, PartialEq)]
pub enum CfeArg {
    String(String),
    /// A token, as its lexeme (`Token.lexeme`, also `Token.toString()`).
    Token(String),
    Int(i64),
    Num(f64),
    Names(Vec<String>),
    Uri(String),
}

/// A CFE message (`Message`): a code with its formatted texts and its
/// arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct CfeMessage {
    pub code: &'static CfeCode,
    pub problem_message: String,
    pub correction_message: Option<String>,
    /// The arguments in parameter order (`Message.arguments.values`).
    pub arguments: Vec<CfeArg>,
}

impl CfeMessage {
    /// Formats the messages of `code` with `arguments` (the generated
    /// `_withArguments<Name>` functions).
    pub fn new(code: &'static CfeCode, arguments: Vec<CfeArg>) -> CfeMessage {
        let problem_message = interpolate(code, code.problem_message, &arguments);
        let correction_message = code
            .correction_message
            .map(|t| interpolate(code, t, &arguments));
        CfeMessage {
            code,
            problem_message,
            correction_message,
            arguments,
        }
    }

    /// The analyzer code for a shared CFE code.
    pub fn shared_analyzer_code(&self) -> Option<&'static DiagnosticCode> {
        self.code.shared_code.map(SharedCode::analyzer_code)
    }

    /// The raw arguments as analyzer arguments
    /// (`message.arguments.values.toList()`, converted with `toString()`).
    pub fn analyzer_arguments(&self) -> Vec<DiagnosticArg> {
        self.arguments
            .iter()
            .map(|a| match a {
                CfeArg::String(s) | CfeArg::Token(s) => DiagnosticArg::String(s.clone()),
                CfeArg::Int(i) => DiagnosticArg::Int(*i),
                CfeArg::Num(n) => DiagnosticArg::String(dart_num_to_string(*n)),
                CfeArg::Names(names) => DiagnosticArg::String(format!("[{}]", names.join(", "))),
                CfeArg::Uri(u) => DiagnosticArg::Uri(u.clone()),
            })
            .collect()
    }

    /// The analyzer diagnostic for a shared CFE code
    /// (`FastaErrorReporter.reportMessage`). Returns `None` for codes that
    /// are not shared.
    pub fn to_shared_diagnostic(&self, offset: usize, length: usize) -> Option<Diagnostic> {
        let code = self.shared_analyzer_code()?;
        Some(Diagnostic::with_arguments(
            code,
            offset,
            length,
            &self.analyzer_arguments(),
            Vec::new(),
        ))
    }
}

fn interpolate(code: &CfeCode, template: &[TemplatePart], arguments: &[CfeArg]) -> String {
    let mut out = String::new();
    for part in template {
        match part {
            TemplatePart::Literal(s) => out.push_str(s),
            TemplatePart::Argument { index, conversion } => {
                let index = *index as usize;
                let ty = code.parameters[index].ty;
                out.push_str(&convert(ty, *conversion, &arguments[index]));
            }
        }
    }
    out
}

/// Applies the conversion of the parameter type (`DiagnosticParameterType.cfeConversion`).
fn convert(ty: ParameterType, conversion: Option<NumericConversion>, arg: &CfeArg) -> String {
    if let (Some(c), CfeArg::Num(n)) = (conversion, arg) {
        return format_number(*n, c.fraction_digits, c.pad_width, c.pad_with_zeros);
    }
    match (ty, arg) {
        (ParameterType::Character, CfeArg::String(s)) => {
            debug_assert_eq!(s.chars().count(), 1, "Not a character '{s}'");
            s.clone()
        }
        (ParameterType::Name, CfeArg::String(s)) => {
            debug_assert!(!s.is_empty(), "No name provided");
            demangle_mixin_application_name(s)
        }
        (ParameterType::NameOkEmpty, CfeArg::String(s)) => {
            if s.is_empty() {
                "(unnamed)".to_string()
            } else {
                s.clone()
            }
        }
        (ParameterType::String, CfeArg::String(s)) => {
            debug_assert!(!s.is_empty(), "No string provided");
            s.clone()
        }
        (ParameterType::StringOkEmpty, CfeArg::String(s)) => {
            if s.is_empty() {
                "(empty)".to_string()
            } else {
                s.clone()
            }
        }
        (ParameterType::Token, CfeArg::Token(lexeme)) => lexeme.clone(),
        (ParameterType::Unicode, CfeArg::Int(c)) => code_point_to_unicode(*c),
        (ParameterType::Int, CfeArg::Int(i)) => i.to_string(),
        (ParameterType::Num, CfeArg::Num(n)) => format_number(*n, None, 0, false),
        (ParameterType::Names, CfeArg::Names(names)) => {
            debug_assert!(!names.is_empty(), "No names provided");
            itemize_names(names)
        }
        // `relativizeUri(Uri.base, uri)`: package and dart URIs are not
        // changed. File URIs are shown as given.
        (ParameterType::Uri, CfeArg::Uri(u)) => u.clone(),
        (ty, arg) => panic!("argument {arg:?} does not match parameter type {ty:?}"),
    }
}

/// `codePointToUnicode`: `U+` and at least four upper case hex digits.
pub fn code_point_to_unicode(code_point: i64) -> String {
    format!("U+{:04X}", code_point)
}

/// `demangleMixinApplicationName`.
pub fn demangle_mixin_application_name(name: &str) -> String {
    let parts: Vec<&str> = name.split('&').collect();
    if parts.len() < 2 || name == "&" {
        return name.to_string();
    }
    let mut out = parts[1].to_string();
    for (i, p) in parts.iter().enumerate().skip(2) {
        out.push_str(if i == 2 { " with " } else { ", " });
        out.push_str(p);
    }
    out
}

/// `itemizeNames`.
pub fn itemize_names(names: &[String]) -> String {
    let mut out = String::new();
    for n in &names[..names.len() - 1] {
        out.push_str(" - ");
        out.push_str(n);
        out.push('\n');
    }
    out.push_str(" - ");
    out.push_str(&names[names.len() - 1]);
    out
}

/// `formatNumber`.
pub fn format_number(
    n: f64,
    fraction_digits: Option<u32>,
    pad_width: u32,
    pad_with_zeros: bool,
) -> String {
    let s = match fraction_digits {
        None => dart_num_to_string(n),
        Some(d) => format!("{:.*}", d as usize, n),
    };
    let width = pad_width as usize;
    if s.len() >= width {
        s
    } else {
        let pad = if pad_with_zeros { "0" } else { " " }.repeat(width - s.len());
        pad + &s
    }
}

/// Dart's `double.toString()` for common values (integral doubles end with
/// `.0`).
fn dart_num_to_string(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{n:.1}")
    } else {
        n.to_string()
    }
}

/// All CFE codes of the shared scanner and parser, sorted by name.
pub fn all_cfe_codes() -> &'static [&'static CfeCode] {
    &crate::generated::cfe_codes::ALL_CFE_CODES
}
