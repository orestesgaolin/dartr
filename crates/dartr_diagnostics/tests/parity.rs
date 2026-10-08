//! Message formatting parity with the pinned Dart analyzer.
//!
//! `tests/fixtures/dart_diagnostics.json` is written by
//! `tools/diagnostics_oracle/bin/list_diagnostics.dart` (run
//! `tools/regen_diagnostics.sh`). It lists every diagnostic code of
//! package:analyzer, package:linter and package:analysis_server, and every
//! CFE code of the shared scanner and parser, with messages formatted with
//! fixed sample arguments. This test produces the same JSON from the
//! generated Rust tables and compares each record.

use std::collections::BTreeMap;

use dartr_diagnostics::cfe::{CfeArg, CfeMessage, all_cfe_codes};
use dartr_diagnostics::{
    Diagnostic, DiagnosticArg, DiagnosticCode, ElementArg, Origin, ParameterType, TypeArg,
    all_codes, code_by_unique_name, codes_by_name,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    let text = include_str!("fixtures/dart_diagnostics.json");
    serde_json::from_str(text).expect("fixture is valid JSON")
}

/// Sample argument for an analyzer code; same rules as list_diagnostics.dart.
fn analyzer_sample(ty: ParameterType, i: usize) -> DiagnosticArg {
    match ty {
        ParameterType::String
        | ParameterType::StringOkEmpty
        | ParameterType::Name
        | ParameterType::NameOkEmpty
        | ParameterType::Character => DiagnosticArg::String(format!("s{i}")),
        ParameterType::Int => DiagnosticArg::Int(100 + i as i64),
        ParameterType::Uri => DiagnosticArg::Uri(format!("package:p{i}/l{i}.dart")),
        ParameterType::Object => DiagnosticArg::Object(format!("o{i}")),
        // Display strings computed by the Dart element model for
        // `class T<i> {}` and `class E<i> {}`.
        ParameterType::Type => DiagnosticArg::Type(TypeArg::new(format!("T{i}"))),
        ParameterType::Element => DiagnosticArg::Element(ElementArg::new(format!("class E{i}"))),
        ParameterType::Token => DiagnosticArg::String(format!("tok{i}")),
        other => panic!("no sample for {other:?}"),
    }
}

fn cfe_sample(ty: ParameterType, i: usize) -> CfeArg {
    match ty {
        ParameterType::String
        | ParameterType::StringOkEmpty
        | ParameterType::Name
        | ParameterType::NameOkEmpty
        | ParameterType::Character => {
            CfeArg::String(char::from_u32(0x41 + i as u32).unwrap().to_string())
        }
        ParameterType::Int | ParameterType::Unicode => CfeArg::Int(31 + i as i64),
        ParameterType::Token => CfeArg::Token(format!("tok{i}")),
        other => panic!("no CFE sample for {other:?}"),
    }
}

fn code_json(code: &'static DiagnosticCode) -> Value {
    let arguments: Vec<DiagnosticArg> = code
        .parameters
        .iter()
        .enumerate()
        .map(|(i, p)| analyzer_sample(p.ty, i))
        .collect();
    let d = Diagnostic::with_arguments(code, 0, 0, &arguments, Vec::new());
    json!({
        "origin": match code.origin {
            Origin::FeAnalyzerShared | Origin::Analyzer => "analyzer",
            Origin::Linter => "linter",
            Origin::AnalysisServer => "analysis_server",
        },
        "name": code.name,
        "uniqueName": code.unique_name,
        "lowerCaseName": code.lower_case_name(),
        "lowerCaseUniqueName": code.lower_case_unique_name(),
        "type": code.diagnostic_type.name(),
        "severity": code.severity().name(),
        "isIgnorable": code.is_ignorable(),
        "hasPublishedDocs": code.has_published_docs,
        "isUnresolvedIdentifier": code.is_unresolved_identifier,
        "url": code.url(),
        "numParameters": code.num_parameters(),
        "expectedTypes": code.parameters.iter().map(|p| p.ty.expected_type()).collect::<Vec<_>>(),
        "problemMessageTemplate": code.problem_message,
        "correctionMessageTemplate": code.correction_message,
        "problemMessage": d.message,
        "correctionMessage": d.correction,
        "diagnosticSeverity": d.severity.display_name(),
        "contextMessages": d.context_messages.len(),
    })
}

fn cfe_json(code: &'static dartr_diagnostics::cfe::CfeCode) -> Value {
    let arguments: Vec<CfeArg> = code
        .parameters
        .iter()
        .enumerate()
        .map(|(i, p)| cfe_sample(p.ty, i))
        .collect();
    let message = CfeMessage::new(code, arguments);
    let analyzer = message.to_shared_diagnostic(0, 0).map(|d| {
        json!({
            "uniqueName": d.code.unique_name,
            "problemMessage": d.message,
            "correctionMessage": d.correction,
        })
    });
    json!({
        "name": code.name,
        "severity": code.severity.name(),
        "sharedCode": code.shared_code.map(|c| c.name()),
        "pseudoSharedCode": code.pseudo_shared_code.map(|c| c.name()),
        "problemMessage": message.problem_message,
        "correctionMessage": message.correction_message,
        "arguments": message.arguments.len(),
        "analyzer": analyzer,
    })
}

/// Compares two keyed record lists. Returns the number of equal records and
/// prints the first differences.
fn compare(what: &str, dart: &[Value], rust: Vec<Value>, key: &str) -> usize {
    let index = |v: &[Value]| -> BTreeMap<String, Value> {
        v.iter()
            .map(|r| (r[key].as_str().unwrap().to_string(), r.clone()))
            .collect()
    };
    let dart = index(dart);
    let rust = index(&rust);
    let mut equal = 0;
    let mut shown = 0;
    for name in dart.keys().filter(|k| !rust.contains_key(*k)) {
        eprintln!("{what}: only in Dart: {name}");
    }
    for name in rust.keys().filter(|k| !dart.contains_key(*k)) {
        eprintln!("{what}: only in Rust: {name}");
    }
    for (name, d) in &dart {
        let Some(r) = rust.get(name) else { continue };
        if d == r {
            equal += 1;
        } else if shown < 10 {
            shown += 1;
            eprintln!("{what}: {name} differs:");
            eprintln!("{}", pretty_assertions::Comparison::new(d, r));
        }
    }
    eprintln!(
        "{what}: {equal}/{} Dart records equal ({} Rust records)",
        dart.len(),
        rust.len()
    );
    equal
}

#[test]
fn diagnostic_codes_match_dart() {
    let fixture = fixture();
    assert_eq!(fixture["sdkVersion"], "3.13.3");
    let dart = fixture["codes"].as_array().unwrap();
    let rust: Vec<Value> = all_codes().iter().map(|c| code_json(c)).collect();
    let mut per_origin: BTreeMap<String, usize> = BTreeMap::new();
    for c in all_codes() {
        *per_origin.entry(format!("{:?}", c.origin)).or_default() += 1;
    }
    eprintln!("Rust codes per origin: {per_origin:?}");
    let equal = compare("codes", dart, rust, "uniqueName");
    assert_eq!(equal, dart.len());
    assert_eq!(all_codes().len(), dart.len());
}

#[test]
fn cfe_codes_match_dart() {
    let fixture = fixture();
    let dart = fixture["cfeCodes"].as_array().unwrap();
    let rust: Vec<Value> = all_cfe_codes().iter().map(|c| cfe_json(c)).collect();
    let equal = compare("cfeCodes", dart, rust, "name");
    assert_eq!(equal, dart.len());
    assert_eq!(all_cfe_codes().len(), dart.len());
}

#[test]
fn lookup_by_name_and_unique_name() {
    for code in all_codes() {
        assert!(std::ptr::eq(
            code_by_unique_name(code.unique_name).unwrap(),
            *code
        ));
        assert!(
            codes_by_name(code.name)
                .iter()
                .any(|c| std::ptr::eq(*c, *code))
        );
    }
    assert_eq!(
        code_by_unique_name("UNDEFINED_CLASS").unwrap().unique_name,
        "undefined_class"
    );
    // `abstract_field_initializer` is shared by two codes.
    let shared = codes_by_name("abstract_field_initializer");
    let mut names: Vec<_> = shared.iter().map(|c| c.unique_name).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "abstract_field_constructor_initializer",
            "abstract_field_initializer"
        ]
    );
    assert!(code_by_unique_name("no_such_code").is_none());
}
