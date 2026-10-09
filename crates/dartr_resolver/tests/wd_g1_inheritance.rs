//! The analyzer diagnostic tests of the g1 codes (inheritance, overrides,
//! duplicate definitions; units D8), ported from
//! `pkg/analyzer/test/src/diagnostics/*_test.dart` (single-file cases) into
//! `wd_g1_fixtures/cases.txt`:
//!
//! ```text
//! === <test file>::<test name>
//! --- <diagnostic camelCase unique name> <offset> <length>   (zero or more)
//! ---
//! <source>
//! ```
//!
//! Only the diagnostics with the codes of `tools/difftest/error_codes_g1.txt`
//! are compared, on both sides (the other wave D codes are not all
//! reported yet).

mod support;

use dartr_diagnostics::DiagnosticCode;

const CASES: &str = include_str!("wd_g1_fixtures/cases.txt");
const G1_CODES: &str = include_str!("../../../tools/difftest/error_codes_g1.txt");

/// Cases that fail for reasons outside the g1 verifiers (name, reason).
const KNOWN_FAILURES: &[(&str, &str)] = &[];

/// Groups of cases that are run but not asserted (name prefix, reason).
const NOT_ASSERTED: &[(&str, &str)] = &[(
    "duplicate_definition::",
    "the error verifier (D4-D7, not merged yet) calls DuplicateDefinitionVerifier \
     (checkUnit, checkParameters, checkStatements, ...); only the members of \
     classes are checked by the library-wide MemberDuplicateDefinitionVerifier",
)];

struct Case {
    name: String,
    expected: Vec<(String, usize, usize)>,
    source: String,
}

fn parse_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for chunk in CASES.split("\n=== ").skip(1) {
        let (name, rest) = chunk.split_once('\n').unwrap();
        let mut expected = Vec::new();
        let mut rest = rest;
        loop {
            let (line, tail) = rest.split_once('\n').unwrap_or((rest, ""));
            rest = tail;
            if line == "---" {
                break;
            }
            let mut parts = line.trim_start_matches("--- ").split(' ');
            let code = parts.next().unwrap().to_string();
            let offset = parts.next().unwrap().parse().unwrap();
            let length = parts.next().unwrap().parse().unwrap();
            expected.push((code, offset, length));
        }
        // The source (with the leading newline of the Dart `r\'\'\'` string)
        // runs to the next case.
        cases.push(Case {
            name: name.to_string(),
            expected,
            source: rest.to_string(),
        });
    }
    cases
}

/// `concreteClassHasEnumSuperinterface` -> the code with that unique name.
fn code_of(camel: &str) -> Option<&'static DiagnosticCode> {
    let mut snake = String::new();
    let mut prev_digit = false;
    for c in camel.chars() {
        if c.is_ascii_uppercase() {
            snake.push('_');
            snake.push(c.to_ascii_lowercase());
        } else if c.is_ascii_digit() && !prev_digit {
            snake.push('_');
            snake.push(c);
        } else {
            snake.push(c);
        }
        prev_digit = c.is_ascii_digit();
    }
    dartr_diagnostics::code_by_unique_name(&snake)
}

#[test]
fn g1_verifiers_report_the_diagnostics_of_the_analyzer_tests() {
    let g1: Vec<&str> = G1_CODES
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let cases = parse_cases();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    let mut unexpected_passes = Vec::new();
    let mut passed = 0;
    for case in &cases {
        let Some(a) = support::analyze(&[("main.dart", &case.source)]) else {
            eprintln!("skipped: no Dart SDK on PATH");
            return;
        };
        let unit = a.unit();
        let mut actual: Vec<(String, usize, usize)> = unit
            .diagnostics
            .iter()
            .filter(|d| g1.contains(&d.code.name))
            .map(|d| (d.code.unique_name.to_string(), d.offset, d.length))
            .collect();
        actual.sort();
        let mut expected: Vec<(String, usize, usize)> = case
            .expected
            .iter()
            .filter_map(|(c, o, l)| {
                let code = code_of(c).unwrap_or_else(|| panic!("unknown code {c}"));
                g1.contains(&code.name)
                    .then(|| (code.unique_name.to_string(), *o, *l))
            })
            .collect();
        expected.sort();
        let known = KNOWN_FAILURES.iter().any(|(n, _)| *n == case.name);
        let ok = actual == expected && unit.panic.is_none();
        if NOT_ASSERTED.iter().any(|(p, _)| case.name.starts_with(p)) {
            passed += usize::from(ok);
            continue;
        }
        if ok {
            passed += 1;
            if known {
                unexpected_passes.push(case.name.clone());
            }
        } else if !known {
            failures.push(format!(
                "{}:\n  expected {:?}\n  actual   {:?}\n  panic    {:?}",
                case.name, expected, actual, unit.panic
            ));
        }
    }
    eprintln!("g1 analyzer cases: {passed}/{} pass", cases.len());
    assert!(
        failures.is_empty() && unexpected_passes.is_empty(),
        "{} failures:\n{}\nunexpected passes: {:?}",
        failures.len(),
        failures.join("\n"),
        unexpected_passes
    );
}
