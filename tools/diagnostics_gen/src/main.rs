//! Generator for `crates/dartr_diagnostics/src/generated/*.rs`.
//!
//! Port of the parts of `pkg/analyzer_utilities/lib/messages.dart`,
//! `pkg/analyzer_utilities/lib/analyzer_messages.dart`,
//! `pkg/analyzer/tool/messages/generate.dart` and
//! `pkg/front_end/tool/generate_messages_lib.dart` (Dart SDK 3.13.3) that
//! decode the `messages.yaml` files and compute the diagnostic tables.
//!
//! Inputs (relative to the repository root):
//! - `third_party/dart-sdk/pkg/_fe_analyzer_shared/messages.yaml`
//! - `third_party/dart-sdk/pkg/analyzer/messages.yaml`
//! - `third_party/dart-sdk/pkg/linter/messages.yaml`
//! - `third_party/dart-sdk/pkg/analysis_server/messages.yaml`
//! - `pkg/front_end/messages.yaml`, read with `git show HEAD:...` because
//!   `pkg/front_end` is not in the sparse checkout. Only the messages with a
//!   `pseudoSharedCode` are used (they are generated into
//!   `_fe_analyzer_shared/lib/src/messages/diagnostic.g.dart`).
//!
//! Usage: `cargo run --manifest-path tools/diagnostics_gen/Cargo.toml [repo_root]`

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use yaml_rust2::{Yaml, YamlLoader};

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

/// `StringExtension.toSnakeCase` of `analyzer_testing`.
fn to_snake_case(s: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    // Word starts: `_?[A-Z]`.
    let mut starts = Vec::new();
    let mut j = 0;
    while j < chars.len() {
        if chars[j] == '_' && j + 1 < chars.len() && chars[j + 1].is_ascii_uppercase() {
            starts.push(j);
            j += 2;
        } else if chars[j].is_ascii_uppercase() {
            starts.push(j);
            j += 1;
        } else {
            j += 1;
        }
    }
    for start in starts {
        if i < start {
            parts.push(chars[i..start].iter().collect::<String>().to_lowercase());
            i = start;
        }
        if chars[i] == '_' && !parts.is_empty() {
            i += 1;
        }
    }
    if i < chars.len() {
        parts.push(chars[i..].iter().collect::<String>().to_lowercase());
    }
    parts.join("_")
}

/// `StringExtension.toCamelCase` of `analyzer_testing`.
fn to_camel_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('_').collect();
    let mut out = String::new();
    let mut i = 0;
    while i < parts.len() - 1 && parts[i].is_empty() {
        out.push('_');
        i += 1;
    }
    if i < parts.len() {
        out.push_str(parts[i]);
        i += 1;
        while i < parts.len() {
            let p = parts[i];
            if !p.is_empty() {
                out.push_str(&p[..1].to_uppercase());
                out.push_str(&p[1..]);
            }
            i += 1;
        }
    }
    out
}

/// `StringExtension.toPascalCase` of `analyzer_testing`.
fn to_pascal_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::new();
    for p in lower.split('_') {
        if !p.is_empty() {
            out.push_str(&p[..1].to_uppercase());
            out.push_str(&p[1..]);
        }
    }
    out
}

fn is_camel_case(s: &str) -> bool {
    !s.contains('_') && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// `DiagnosticCodeName` of `analyzer_utilities/messages.dart`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CodeName {
    snake: String,
    camel: String,
}

impl CodeName {
    fn from_camel(camel: &str) -> CodeName {
        let snake = match camel {
            "finalNotInitializedConstructor1" => "final_not_initialized_constructor_1".to_string(),
            "finalNotInitializedConstructor2" => "final_not_initialized_constructor_2".to_string(),
            "finalNotInitializedConstructor3Plus" => {
                "final_not_initialized_constructor_3_plus".to_string()
            }
            "linesLongerThan80Chars" => "lines_longer_than_80_chars".to_string(),
            _ => to_snake_case(camel),
        };
        assert_eq!(snake, snake.to_lowercase(), "snake case name {snake} is not lower case");
        assert_eq!(to_camel_case(&snake), camel, "round trip of {camel} failed");
        CodeName { snake, camel: camel.to_string() }
    }

    fn pascal(&self) -> String {
        to_pascal_case(&self.snake)
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
    "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

fn rust_ident(snake: &str) -> String {
    if RUST_KEYWORDS.contains(&snake) { format!("{snake}_") } else { snake.to_string() }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    FeAnalyzerShared,
    Analyzer,
    Linter,
    AnalysisServer,
    FrontEnd,
}

impl Origin {
    fn rust(self) -> &'static str {
        match self {
            Origin::FeAnalyzerShared => "Origin::FeAnalyzerShared",
            Origin::Analyzer => "Origin::Analyzer",
            Origin::Linter => "Origin::Linter",
            Origin::AnalysisServer => "Origin::AnalysisServer",
            Origin::FrontEnd => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
struct Param {
    name: String,
    /// The type name used in `messages.yaml` (`String`, `Type`, `Name`, ...).
    ty: String,
}

#[derive(Clone, Debug)]
struct NumericConversion {
    fraction_digits: Option<u32>,
    pad_width: u32,
    pad_with_zeros: bool,
}

#[derive(Clone, Debug)]
enum Part {
    Literal(String),
    Param { index: usize, conversion: Option<NumericConversion> },
}

#[derive(Clone, Debug)]
struct Message {
    origin: Origin,
    /// The key of the message in its yaml file (camelCase).
    key: String,
    /// For analyzer style files: the class key (`CompileTimeErrorCode`, ...).
    class: Option<String>,
    analyzer_code: Option<CodeName>,
    ty: Option<String>,
    has_published_docs: bool,
    params: Vec<Param>,
    problem: Vec<Part>,
    correction: Option<Vec<Part>>,
    shared_name: Option<CodeName>,
    removed_in: Option<String>,
    previous_name: Option<String>,
    alias_for: Option<String>,
    deprecated_message: Option<String>,
    is_unresolved_identifier: bool,
    cfe_severity: Option<String>,
    pseudo_shared_code: Option<String>,
}

fn get<'a>(map: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    match map {
        Yaml::Hash(h) => h.get(&Yaml::String(key.to_string())),
        _ => None,
    }
}

fn get_str(map: &Yaml, key: &str) -> Option<String> {
    get(map, key).map(|v| match v {
        Yaml::String(s) => s.clone(),
        other => panic!("key {key}: expected a string, got {other:?}"),
    })
}

fn get_bool(map: &Yaml, key: &str) -> Option<bool> {
    get(map, key).map(|v| match v {
        Yaml::Boolean(b) => *b,
        other => panic!("key {key}: expected a bool, got {other:?}"),
    })
}

fn decode_params(map: &Yaml, key: &str) -> Vec<Param> {
    match get(map, "parameters") {
        Some(Yaml::String(s)) if s == "none" => vec![],
        Some(Yaml::Hash(h)) => h
            .iter()
            .map(|(k, _)| {
                let k = k.as_str().expect("parameter key");
                let parts: Vec<&str> = k.split(' ').collect();
                assert_eq!(parts.len(), 2, "{key}: malformed parameter key {k:?}");
                Param { ty: parts[0].to_string(), name: parts[1].to_string() }
            })
            .collect(),
        other => panic!("{key}: parameters must be a map or \"none\", got {other:?}"),
    }
}

fn placeholder_regex() -> Regex {
    Regex::new(r"#([-a-zA-Z0-9_]+)(?:%([0-9]*).([0-9]+))?").unwrap()
}

/// `MessageYaml.getMessageTemplate`.
fn decode_template(text: &str, params: &[Param], key: &str) -> Vec<Part> {
    let text = text.trim_end();
    let old = Regex::new(r"\{\d+\}").unwrap();
    assert!(!old.is_match(text), "{key}: old style placeholder");
    let re = placeholder_regex();
    let mut parts = Vec::new();
    let mut i = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        if m.start() > i {
            parts.push(Part::Literal(text[i..m.start()].to_string()));
        }
        let name = &caps[1];
        let index = params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| panic!("{key}: placeholder {name:?} not declared"));
        let padding = caps.get(2).map(|m| m.as_str());
        let fraction = caps.get(3).map(|m| m.as_str().parse::<u32>().unwrap());
        let conversion = match (padding, fraction) {
            (Some(p), _) if !p.is_empty() => Some(NumericConversion {
                fraction_digits: fraction,
                pad_width: p.parse().unwrap(),
                pad_with_zeros: p.starts_with('0'),
            }),
            (_, Some(f)) => {
                Some(NumericConversion { fraction_digits: Some(f), pad_width: 0, pad_with_zeros: false })
            }
            _ => None,
        };
        parts.push(Part::Param { index, conversion });
        i = m.end();
    }
    if text.len() > i {
        parts.push(Part::Literal(text[i..].to_string()));
    }
    parts
}

fn decode_message(origin: Origin, key: &str, class: Option<&str>, map: &Yaml) -> Message {
    let params = decode_params(map, key);
    let template = |k: &str| get_str(map, k).map(|t| decode_template(&t, &params, key));
    let analyzer_code = match origin {
        Origin::FeAnalyzerShared => {
            let c = get_str(map, "analyzerCode").expect("analyzerCode");
            assert!(is_camel_case(&c), "{key}: analyzer codes must be camelCase");
            Some(CodeName::from_camel(&c))
        }
        Origin::FrontEnd => None,
        _ => {
            assert!(is_camel_case(key), "{key}: names must be camelCase");
            Some(CodeName::from_camel(key))
        }
    };
    let shared_name = get_str(map, "sharedName").map(|s| {
        assert!(is_camel_case(&s), "{key}: shared names must be camelCase");
        CodeName::from_camel(&s)
    });
    let problem = template("problemMessage");
    if matches!(origin, Origin::FeAnalyzerShared | Origin::FrontEnd) {
        assert!(problem.is_some(), "{key}: problemMessage is required");
    }
    Message {
        origin,
        key: key.to_string(),
        class: class.map(str::to_string),
        analyzer_code,
        ty: get_str(map, "type"),
        has_published_docs: get_bool(map, "hasPublishedDocs").unwrap_or(false),
        problem: problem.unwrap_or_default(),
        correction: template("correctionMessage"),
        shared_name,
        removed_in: get_str(map, "removedIn"),
        previous_name: get_str(map, "previousName"),
        alias_for: get_str(map, "aliasFor"),
        deprecated_message: get_str(map, "deprecatedMessage"),
        is_unresolved_identifier: get_bool(map, "isUnresolvedIdentifier").unwrap_or(false),
        cfe_severity: get_str(map, "severity"),
        pseudo_shared_code: get_str(map, "pseudoSharedCode"),
        params,
    }
}

fn load_yaml(text: &str, what: &str) -> Yaml {
    let mut docs = YamlLoader::load_from_str(text).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(docs.len(), 1, "{what}: expected one yaml document");
    docs.remove(0)
}

/// CFE style file: a flat map of messages.
fn load_cfe_style(origin: Origin, text: &str, what: &str) -> Vec<Message> {
    let yaml = load_yaml(text, what);
    let Yaml::Hash(h) = &yaml else { panic!("{what}: root is not a map") };
    h.iter()
        .map(|(k, v)| decode_message(origin, k.as_str().expect("key"), None, v))
        .collect()
}

/// Analyzer style file: class -> message.
fn load_analyzer_style(origin: Origin, text: &str, what: &str) -> Vec<Message> {
    let yaml = load_yaml(text, what);
    let Yaml::Hash(h) = &yaml else { panic!("{what}: root is not a map") };
    let mut out = Vec::new();
    for (class, messages) in h {
        let class = class.as_str().expect("class key");
        let Yaml::Hash(mh) = messages else { panic!("{what}: {class} is not a map") };
        for (k, v) in mh {
            out.push(decode_message(origin, k.as_str().expect("key"), Some(class), v));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Code generation helpers
// ---------------------------------------------------------------------------

fn lit(s: &str) -> String {
    format!("{s:?}")
}

fn opt_lit(s: &Option<String>) -> String {
    match s {
        Some(s) => format!("Some({})", lit(s)),
        None => "None".to_string(),
    }
}

/// `convertTemplate`: a template with `{0}` style placeholders.
fn convert_template(parts: &[Part]) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Literal(t) => t.clone(),
            Part::Param { index, .. } => format!("{{{index}}}"),
        })
        .collect()
}

fn diagnostic_type(ty: &str) -> &'static str {
    match ty {
        "compileTimeError" => "DiagnosticType::CompileTimeError",
        "hint" => "DiagnosticType::Hint",
        "lint" => "DiagnosticType::Lint",
        "staticWarning" => "DiagnosticType::StaticWarning",
        "syntacticError" => "DiagnosticType::SyntacticError",
        "todo" => "DiagnosticType::Todo",
        other => panic!("unknown diagnostic type {other}"),
    }
}

fn param_type(ty: &str) -> &'static str {
    match ty {
        "Character" => "ParameterType::Character",
        "Constant" => "ParameterType::Constant",
        "Element" => "ParameterType::Element",
        "int" => "ParameterType::Int",
        "Name" => "ParameterType::Name",
        "NameOKEmpty" => "ParameterType::NameOkEmpty",
        "Names" => "ParameterType::Names",
        "num" => "ParameterType::Num",
        "Object" => "ParameterType::Object",
        "String" => "ParameterType::String",
        "StringOKEmpty" => "ParameterType::StringOkEmpty",
        "Token" => "ParameterType::Token",
        "Type" => "ParameterType::Type",
        "Unicode" => "ParameterType::Unicode",
        "Uri" => "ParameterType::Uri",
        other => panic!("unknown parameter type {other}"),
    }
}

fn params_lit(params: &[Param]) -> String {
    let items: Vec<String> = params
        .iter()
        .map(|p| format!("Parameter {{ name: {}, ty: {} }}", lit(&p.name), param_type(&p.ty)))
        .collect();
    format!("&[{}]", items.join(", "))
}

/// Rust parameter type and the expression that converts it to a
/// `DiagnosticArg`, for the typed constructor of an analyzer code.
fn analyzer_arg(p: &Param, ident: &str) -> (String, String) {
    match p.ty.as_str() {
        "String" | "StringOKEmpty" | "Name" | "NameOKEmpty" | "Character" => {
            ("&str".into(), format!("DiagnosticArg::String({ident}.to_string())"))
        }
        "Token" => ("&str".into(), format!("DiagnosticArg::String({ident}.to_string())")),
        "int" => ("i64".into(), format!("DiagnosticArg::Int({ident})")),
        "Uri" => ("&str".into(), format!("DiagnosticArg::Uri({ident}.to_string())")),
        "Object" => ("impl std::fmt::Display".into(), format!("DiagnosticArg::Object({ident}.to_string())")),
        "Type" => ("TypeArg".into(), format!("DiagnosticArg::Type({ident})")),
        "Element" => ("ElementArg".into(), format!("DiagnosticArg::Element({ident})")),
        other => panic!("parameter type {other} is not supported for analyzer codes"),
    }
}

/// Rust parameter type and the expression that converts it to a `CfeArg`,
/// for the typed constructor of a CFE code.
fn cfe_arg(p: &Param, ident: &str) -> (String, String) {
    match p.ty.as_str() {
        "String" | "StringOKEmpty" | "Name" | "NameOKEmpty" | "Character" => {
            ("&str".into(), format!("CfeArg::String({ident}.to_string())"))
        }
        "Token" => ("&str".into(), format!("CfeArg::Token({ident}.to_string())")),
        "int" | "Unicode" => ("i64".into(), format!("CfeArg::Int({ident})")),
        "num" => ("f64".into(), format!("CfeArg::Num({ident})")),
        "Names" => ("&[&str]".into(), format!("CfeArg::Names({ident}.iter().map(|s| s.to_string()).collect())")),
        "Uri" => ("&str".into(), format!("CfeArg::Uri({ident}.to_string())")),
        other => panic!("parameter type {other} is not supported for CFE codes"),
    }
}

fn cfe_template_lit(parts: &[Part]) -> String {
    let items: Vec<String> = parts
        .iter()
        .map(|p| match p {
            Part::Literal(t) => format!("TemplatePart::Literal({})", lit(t)),
            Part::Param { index, conversion: None } => {
                format!("TemplatePart::Argument {{ index: {index}, conversion: None }}")
            }
            Part::Param { index, conversion: Some(c) } => format!(
                "TemplatePart::Argument {{ index: {index}, conversion: Some(NumericConversion {{ fraction_digits: {:?}, pad_width: {}, pad_with_zeros: {} }}) }}",
                c.fraction_digits, c.pad_width, c.pad_with_zeros
            ),
        })
        .collect();
    format!("&[{}]", items.join(", "))
}

fn doc_comment(out: &mut String, text: &str) {
    for line in text.lines() {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            // Avoid rustdoc interpreting brackets and code fences.
            let line = line.replace('`', "'").replace('[', "\\[").replace(']', "\\]");
            let _ = writeln!(out, "/// {line}");
        }
    }
}

const HEADER: &str = "// THIS FILE IS GENERATED. DO NOT EDIT.\n//\n// Generated by `tools/diagnostics_gen` from the `messages.yaml` files of the\n// Dart SDK (tag 3.13.3). Regenerate with `tools/regen_diagnostics.sh`.\n";

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let root: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let sdk = root.join("third_party/dart-sdk");
    let read = |rel: &str| {
        std::fs::read_to_string(sdk.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    };
    let front_end_yaml = {
        let out = Command::new("git")
            .arg("-C")
            .arg(&sdk)
            .args(["show", "HEAD:pkg/front_end/messages.yaml"])
            .output()
            .expect("git show pkg/front_end/messages.yaml");
        assert!(out.status.success(), "git show failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };

    let shared = load_cfe_style(
        Origin::FeAnalyzerShared,
        &read("pkg/_fe_analyzer_shared/messages.yaml"),
        "_fe_analyzer_shared",
    );
    let analyzer =
        load_analyzer_style(Origin::Analyzer, &read("pkg/analyzer/messages.yaml"), "analyzer");
    let linter = load_analyzer_style(Origin::Linter, &read("pkg/linter/messages.yaml"), "linter");
    let server = load_analyzer_style(
        Origin::AnalysisServer,
        &read("pkg/analysis_server/messages.yaml"),
        "analysis_server",
    );
    let front_end = load_cfe_style(Origin::FrontEnd, &front_end_yaml, "front_end");

    // ---- analyzer codes --------------------------------------------------
    let all_analyzer: Vec<&Message> =
        shared.iter().chain(&analyzer).chain(&server).chain(&linter).collect();
    let mut seen = BTreeSet::new();
    let mut active: Vec<&Message> = Vec::new();
    let mut removed: Vec<&Message> = Vec::new();
    let mut shared_name_counts: HashMap<String, Vec<&Message>> = HashMap::new();
    for m in &all_analyzer {
        let code = m.analyzer_code.as_ref().unwrap();
        assert!(seen.insert(code.snake.clone()), "duplicate analyzer code {}", code.snake);
        let name = m.shared_name.as_ref().unwrap_or(code).snake.clone();
        shared_name_counts.entry(name).or_default().push(m);
        let permitted: &[&str] = match m.origin {
            Origin::FeAnalyzerShared | Origin::Analyzer => {
                &["compileTimeError", "hint", "staticWarning", "syntacticError", "todo"]
            }
            Origin::AnalysisServer => &["compileTimeError"],
            Origin::Linter => &["lint"],
            Origin::FrontEnd => unreachable!(),
        };
        let ty = m.ty.as_deref().unwrap_or_else(|| panic!("{}: missing type", m.key));
        assert!(permitted.contains(&ty), "{}: type {ty} not permitted", m.key);
        let uses_params = m
            .problem
            .iter()
            .chain(m.correction.iter().flatten())
            .any(|p| matches!(p, Part::Param { .. }));
        assert!(m.params.is_empty() || uses_params, "{}: parameters declared but not used", m.key);
        if m.removed_in.is_some() {
            removed.push(m);
        } else if m.alias_for.is_some() {
            // Aliases are not generated as separate codes (there are none in 3.13.3).
        } else {
            active.push(m);
        }
    }
    for (name, ms) in &shared_name_counts {
        if ms.len() == 1 {
            assert_eq!(
                name,
                &ms[0].analyzer_code.as_ref().unwrap().snake,
                "only message using shared name {name}"
            );
        }
    }
    active.sort_by(|a, b| a.analyzer_code.as_ref().unwrap().snake.cmp(&b.analyzer_code.as_ref().unwrap().snake));
    removed.sort_by(|a, b| a.analyzer_code.as_ref().unwrap().snake.cmp(&b.analyzer_code.as_ref().unwrap().snake));

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("\n//! Analyzer diagnostic codes: one static [`DiagnosticCode`] and one typed\n//! constructor function per code. Port of `pkg/analyzer/lib/src/diagnostic/diagnostic.g.dart`,\n//! `pkg/linter/lib/src/diagnostic.g.dart` and `pkg/analysis_server/lib/src/diagnostic.g.dart`.\n\n");
    out.push_str("#![allow(clippy::all)]\n\n");
    out.push_str("use crate::args::{DiagnosticArg, ElementArg, TypeArg};\nuse crate::code::{DiagnosticCode, DiagnosticType, Origin, Parameter, ParameterType, RemovedDiagnosticCode};\nuse crate::diagnostic::LocatableDiagnostic;\n\n");

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for m in &active {
        let code = m.analyzer_code.as_ref().unwrap();
        let origin_name = match m.origin {
            Origin::FeAnalyzerShared => "_fe_analyzer_shared",
            Origin::Analyzer => "analyzer",
            Origin::Linter => "linter",
            Origin::AnalysisServer => "analysis_server",
            Origin::FrontEnd => unreachable!(),
        };
        *counts.entry(origin_name).or_default() += 1;
        let name = m.shared_name.as_ref().unwrap_or(code).snake.clone();
        let const_name = code.snake.to_uppercase();
        let problem = convert_template(&m.problem);
        let correction = m.correction.as_ref().map(|c| convert_template(c));
        let class = m.class.clone().unwrap_or_default();
        let _ = writeln!(out, "/// `{}` ({}{}).", code.camel, origin_name, if class.is_empty() { String::new() } else { format!(", {class}") });
        out.push_str("///\n");
        doc_comment(&mut out, &format!("Problem: {problem}"));
        if let Some(d) = &m.deprecated_message {
            let _ = writeln!(out, "#[doc = {}]", lit(&format!("Deprecated: {d}")));
        }
        let _ = writeln!(out, "pub static {const_name}: DiagnosticCode = DiagnosticCode {{");
        let _ = writeln!(out, "    name: {},", lit(&name));
        let _ = writeln!(out, "    unique_name: {},", lit(&code.snake));
        let _ = writeln!(out, "    camel_case_name: {},", lit(&code.camel));
        let _ = writeln!(out, "    problem_message: {},", lit(&problem));
        let _ = writeln!(out, "    correction_message: {},", opt_lit(&correction));
        let _ = writeln!(out, "    diagnostic_type: {},", diagnostic_type(m.ty.as_ref().unwrap()));
        let _ = writeln!(out, "    has_published_docs: {},", m.has_published_docs);
        let _ = writeln!(out, "    is_unresolved_identifier: {},", m.is_unresolved_identifier);
        let _ = writeln!(out, "    parameters: {},", params_lit(&m.params));
        let _ = writeln!(out, "    origin: {},", m.origin.rust());
        let _ = writeln!(out, "    deprecated_message: {},", opt_lit(&m.deprecated_message));
        out.push_str("};\n\n");

        // Typed constructor.
        let fn_name = rust_ident(&code.snake);
        let mut sig = Vec::new();
        let mut args = Vec::new();
        for p in &m.params {
            let ident = rust_ident(&to_snake_case(&p.name));
            let (ty, expr) = analyzer_arg(p, &ident);
            sig.push(format!("{ident}: {ty}"));
            args.push(expr);
        }
        let _ = writeln!(out, "/// Creates a [`LocatableDiagnostic`] for [`{const_name}`].");
        if m.deprecated_message.is_some() {
            out.push_str("#[allow(dead_code)]\n");
        }
        let _ = writeln!(
            out,
            "pub fn {fn_name}({}) -> LocatableDiagnostic {{\n    LocatableDiagnostic::new(&{const_name}, vec![{}])\n}}\n",
            sig.join(", "),
            args.join(", ")
        );
    }

    // All codes sorted by unique name.
    let _ = writeln!(out, "/// All active diagnostic codes, sorted by [`DiagnosticCode::unique_name`].");
    let _ = writeln!(out, "pub static ALL_CODES: [&DiagnosticCode; {}] = [", active.len());
    for m in &active {
        let _ = writeln!(out, "    &{},", m.analyzer_code.as_ref().unwrap().snake.to_uppercase());
    }
    out.push_str("];\n\n");

    // By name.
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in &active {
        let code = m.analyzer_code.as_ref().unwrap();
        let name = m.shared_name.as_ref().unwrap_or(code).snake.clone();
        by_name.entry(name).or_default().push(code.snake.to_uppercase());
    }
    let _ = writeln!(out, "/// Codes grouped by [`DiagnosticCode::name`], sorted by name.");
    let _ = writeln!(out, "pub static CODES_BY_NAME: [(&str, &[&DiagnosticCode]); {}] = [", by_name.len());
    for (name, codes) in &by_name {
        let refs: Vec<String> = codes.iter().map(|c| format!("&{c}")).collect();
        let _ = writeln!(out, "    ({}, &[{}]),", lit(name), refs.join(", "));
    }
    out.push_str("];\n\n");

    // Removed codes.
    let _ = writeln!(out, "/// Codes that have a `removedIn` entry. The analyzer does not report them.");
    let _ = writeln!(out, "pub static REMOVED_CODES: [RemovedDiagnosticCode; {}] = [", removed.len());
    for m in &removed {
        let code = m.analyzer_code.as_ref().unwrap();
        let name = m.shared_name.as_ref().unwrap_or(code).snake.clone();
        let _ = writeln!(
            out,
            "    RemovedDiagnosticCode {{ name: {}, unique_name: {}, removed_in: {}, previous_name: {} }},",
            lit(&name),
            lit(&code.snake),
            lit(m.removed_in.as_ref().unwrap()),
            opt_lit(&m.previous_name)
        );
    }
    out.push_str("];\n");

    // ---- CFE codes ---------------------------------------------------------
    // `diagnosticTables.sortedSharedDiagnostics`: shared messages sorted by
    // analyzer camelCase name. The index is the `SharedCode` index.
    let mut sorted_shared: Vec<&Message> = shared.iter().collect();
    sorted_shared.sort_by(|a, b| a.analyzer_code.as_ref().unwrap().camel.cmp(&b.analyzer_code.as_ref().unwrap().camel));

    let mut cfe: Vec<&Message> = shared
        .iter()
        .chain(front_end.iter().filter(|m| m.pseudo_shared_code.is_some()))
        .collect();
    let mut fe_names = BTreeSet::new();
    for m in shared.iter().chain(&front_end) {
        assert!(is_camel_case(&m.key), "{}: front end codes must be camelCase", m.key);
        assert!(fe_names.insert(CodeName::from_camel(&m.key).snake), "duplicate front end code {}", m.key);
    }
    cfe.sort_by_key(|m| CodeName::from_camel(&m.key).snake);
    let mut pseudo_values = BTreeSet::new();
    for m in &cfe {
        if let Some(p) = &m.pseudo_shared_code {
            pseudo_values.insert(to_camel_case(p));
        }
    }

    let mut c = String::new();
    c.push_str(HEADER);
    c.push_str("\n//! CFE codes that the shared scanner and parser report. Port of\n//! `pkg/_fe_analyzer_shared/lib/src/messages/diagnostic.g.dart`.\n\n");
    c.push_str("#![allow(clippy::all)]\n\n");
    c.push_str("use crate::cfe::{CfeArg, CfeCode, CfeMessage, CfeSeverity, NumericConversion, TemplatePart};\nuse crate::code::{DiagnosticCode, Parameter, ParameterType};\nuse crate::generated::diag;\n\n");

    c.push_str("/// Analyzer codes referenced by [`CfeCode::pseudo_shared_code`]. The analyzer\n/// translates these by hand (`error_converter.dart`, `translate_error_token.dart`).\n");
    c.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum PseudoSharedCode {\n");
    for v in &pseudo_values {
        let _ = writeln!(c, "    {},", to_pascal_case(&to_snake_case(v)));
    }
    c.push_str("}\n\nimpl PseudoSharedCode {\n    /// The camelCase name used in Dart.\n    pub fn name(self) -> &'static str {\n        match self {\n");
    for v in &pseudo_values {
        let _ = writeln!(c, "            PseudoSharedCode::{} => {},", to_pascal_case(&to_snake_case(v)), lit(v));
    }
    c.push_str("        }\n    }\n}\n\n");

    c.push_str("/// Analyzer codes that are shared with the CFE ([`CfeCode::shared_code`]).\n");
    c.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum SharedCode {\n");
    for m in &sorted_shared {
        let _ = writeln!(c, "    {},", m.analyzer_code.as_ref().unwrap().pascal());
    }
    c.push_str("}\n\nimpl SharedCode {\n    /// The analyzer code (`sharedAnalyzerCodes[index]`).\n    pub fn analyzer_code(self) -> &'static DiagnosticCode {\n        match self {\n");
    for m in &sorted_shared {
        let code = m.analyzer_code.as_ref().unwrap();
        let _ = writeln!(c, "            SharedCode::{} => &diag::{},", code.pascal(), code.snake.to_uppercase());
    }
    c.push_str("        }\n    }\n\n    /// The camelCase name used in Dart.\n    pub fn name(self) -> &'static str {\n        match self {\n");
    for m in &sorted_shared {
        let code = m.analyzer_code.as_ref().unwrap();
        let _ = writeln!(c, "            SharedCode::{} => {},", code.pascal(), lit(&code.camel));
    }
    c.push_str("        }\n    }\n}\n\n");

    for m in &cfe {
        let name = CodeName::from_camel(&m.key);
        let const_name = name.snake.to_uppercase();
        let severity = match m.cfe_severity.as_deref() {
            None => "CfeSeverity::Error",
            Some("CONTEXT") => "CfeSeverity::Context",
            Some("IGNORED") => "CfeSeverity::Ignored",
            Some("INTERNAL_PROBLEM") => "CfeSeverity::InternalProblem",
            Some("WARNING") => "CfeSeverity::Warning",
            Some("INFO") => "CfeSeverity::Info",
            Some(o) => panic!("unknown severity {o}"),
        };
        let shared_code = match m.origin {
            Origin::FeAnalyzerShared => format!("Some(SharedCode::{})", m.analyzer_code.as_ref().unwrap().pascal()),
            _ => "None".into(),
        };
        let pseudo = match &m.pseudo_shared_code {
            Some(p) => format!("Some(PseudoSharedCode::{})", to_pascal_case(&to_snake_case(&to_camel_case(p)))),
            None => "None".into(),
        };
        let _ = writeln!(c, "/// `{}`.", name.pascal());
        let _ = writeln!(c, "pub static {const_name}: CfeCode = CfeCode {{");
        let _ = writeln!(c, "    name: {},", lit(&name.pascal()));
        let _ = writeln!(c, "    problem_message: {},", cfe_template_lit(&m.problem));
        let _ = writeln!(
            c,
            "    correction_message: {},",
            m.correction.as_ref().map(|t| format!("Some({})", cfe_template_lit(t))).unwrap_or("None".into())
        );
        let _ = writeln!(c, "    severity: {severity},");
        let _ = writeln!(c, "    shared_code: {shared_code},");
        let _ = writeln!(c, "    pseudo_shared_code: {pseudo},");
        let _ = writeln!(c, "    parameters: {},", params_lit(&m.params));
        c.push_str("};\n\n");
        let fn_name = rust_ident(&name.snake);
        let mut sig = Vec::new();
        let mut args = Vec::new();
        for p in &m.params {
            let ident = rust_ident(&to_snake_case(&p.name));
            let (ty, expr) = cfe_arg(p, &ident);
            sig.push(format!("{ident}: {ty}"));
            args.push(expr);
        }
        let _ = writeln!(
            c,
            "/// Creates a [`CfeMessage`] for [`{const_name}`].\npub fn {fn_name}({}) -> CfeMessage {{\n    CfeMessage::new(&{const_name}, vec![{}])\n}}\n",
            sig.join(", "),
            args.join(", ")
        );
    }
    let _ = writeln!(c, "/// All CFE codes of the shared scanner and parser, sorted by name.");
    let _ = writeln!(c, "pub static ALL_CFE_CODES: [&CfeCode; {}] = [", cfe.len());
    for m in &cfe {
        let _ = writeln!(c, "    &{},", CodeName::from_camel(&m.key).snake.to_uppercase());
    }
    c.push_str("];\n");

    let gen_dir = root.join("crates/dartr_diagnostics/src/generated");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(gen_dir.join("diag.rs"), out).unwrap();
    std::fs::write(gen_dir.join("cfe_codes.rs"), c).unwrap();

    eprintln!("active analyzer codes per source:");
    for (k, v) in &counts {
        eprintln!("  {k}: {v}");
    }
    eprintln!("  total: {}", active.len());
    eprintln!("removed codes: {}", removed.len());
    eprintln!("CFE codes of the shared scanner/parser: {} ({} shared, {} pseudo-shared)", cfe.len(), shared.len(), cfe.len() - shared.len());
}
