//! The harness of the wave D group g3 tests (`wd_g3_*.rs`): each case is
//! a ported analyzer diagnostic test, analyzed as `package:test/main.dart`
//! with the mock `meta` and `angular_meta` packages of the analyzer tests;
//! the diagnostics of the codes of `tools/difftest/error_codes_g3.txt` are
//! compared as (code, offset, length).

use dartr_resolver::options::AnalysisOptions;

use super::{analyze_in_packages, mock_package_files};

const CODES: &str = include_str!("../../../../tools/difftest/error_codes_g3.txt");

/// One ported test: the files of `package:test` (the first is analyzed),
/// other packages, and the expected diagnostics.
pub struct Case {
    pub name: &'static str,
    pub files: &'static [(&'static str, &'static str)],
    pub packages: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    pub strict_inference: bool,
    /// (code, text of the range, occurrence of the text, starting at 0).
    pub expected: &'static [(&'static str, &'static str, usize)],
}

pub const fn case(
    name: &'static str,
    expected: &'static [(&'static str, &'static str, usize)],
) -> Case {
    Case {
        name,
        files: &[],
        packages: &[],
        strict_inference: false,
        expected,
    }
}

/// Runs [cases]; returns the failures.
fn run(cases: &[(Case, &'static str)]) -> Vec<String> {
    let codes: Vec<&str> = CODES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut failures = Vec::new();
    for (case, main) in cases {
        let mut packages: Vec<(&str, Vec<(String, String)>)> = Vec::new();
        let Some(meta) = mock_package_files("addMetaPackageFiles") else {
            eprintln!("skipped: no SDK sources");
            return failures;
        };
        packages.push(("meta", meta));
        if let Some(angular) = mock_package_files("addAngularMetaPackageFiles") {
            packages.push(("angular_meta", angular));
        }
        for (name, files) in case.packages {
            packages.push((
                name,
                files
                    .iter()
                    .map(|(p, c)| (p.to_string(), c.to_string()))
                    .collect(),
            ));
        }
        let mut files: Vec<(&str, &str)> = vec![("main.dart", main)];
        files.extend(case.files.iter().copied());
        let options = AnalysisOptions {
            strict_inference: case.strict_inference,
            ..AnalysisOptions::default()
        };
        let Some(a) = analyze_in_packages(&files, &packages, options) else {
            eprintln!("skipped: no Dart SDK on PATH");
            return failures;
        };
        if let Some(panic) = &a.unit().panic {
            failures.push(format!("{}: panic {panic}", case.name));
            continue;
        }
        let mut actual: Vec<(String, usize, usize)> = a
            .unit()
            .diagnostics
            .iter()
            .filter(|d| codes.contains(&d.code.name))
            .map(|d| (d.code.name.to_string(), d.offset, d.length))
            .collect();
        let mut expected: Vec<(String, usize, usize)> = Vec::new();
        for &(code, text, occurrence) in case.expected {
            let Some((offset, _)) = main.match_indices(text).nth(occurrence) else {
                failures.push(format!("{}: text {text:?} not found", case.name));
                continue;
            };
            let offset = main[..offset].encode_utf16().count();
            expected.push((code.to_string(), offset, text.encode_utf16().count()));
        }
        actual.sort();
        expected.sort();
        if actual != expected {
            failures.push(format!(
                "{}:\n  expected {expected:?}\n  actual   {actual:?}",
                case.name
            ));
        }
    }
    failures
}

pub fn check(cases: Vec<(Case, &'static str)>) {
    let total = cases.len();
    let failures = run(&cases);
    assert!(
        failures.is_empty(),
        "{} of {total} cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A case whose main file is [source].
pub fn c(
    name: &'static str,
    source: &'static str,
    expected: &'static [(&'static str, &'static str, usize)],
) -> (Case, &'static str) {
    (case(name, expected), source)
}

/// A case with other packages.
pub fn cp(
    name: &'static str,
    packages: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    source: &'static str,
    expected: &'static [(&'static str, &'static str, usize)],
) -> (Case, &'static str) {
    let mut case = case(name, expected);
    case.packages = packages;
    (case, source)
}

/// A test ported mechanically from an analyzer diagnostic test file (see
/// `wd_g3_ported.rs`).
pub struct Ported {
    pub name: &'static str,
    pub strict_inference: bool,
    /// Packages (name, files relative to `lib`); `test` adds files next to
    /// the test file.
    pub packages: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    /// The test code; with inline diagnostic markers when [expected] is
    /// `None` (Dart `resolveTestCodeWithDiagnostics`).
    pub source: &'static str,
    /// Dart `assertErrorsInCode`: (camelCase name, offset, length).
    pub expected: Option<&'static [(&'static str, usize, usize)]>,
}

/// The code name of the diagnostic with the Dart camelCase name.
fn code_name_of(camel: &str) -> Option<&'static str> {
    dartr_diagnostics::diag::ALL_CODES
        .iter()
        .find(|c| c.camel_case_name == camel)
        .map(|c| c.name)
}

/// Dart `removeDiagnosticExpectations` and the expected diagnostics of
/// the markers: (code name or camel name if unknown, offset, length) in
/// UTF-16 units.
pub fn parse_markers(source: &str) -> (String, Vec<(String, usize, usize)>) {
    let caret = |l: &str| {
        let t = l.trim_start();
        t.starts_with("//") && {
            let r = t[2..].trim();
            !r.is_empty() && r.chars().all(|c| c == '^')
        }
    };
    let expectation = |l: &str| {
        let t = l.trim_start();
        t.starts_with("//") && {
            let r = t[2..].trim_start();
            r.starts_with("[diag.") || r.starts_with("[context")
        }
    };
    let mut code = String::new();
    let mut expected = Vec::new();
    let mut last_line_start = 0usize;
    let mut current_caret: Option<(usize, usize)> = None;
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut retained: Vec<&str> = Vec::new();
    for line in &lines {
        let text = line.trim_end_matches(['\n', '\r']);
        if caret(text) {
            let col = text.find('^').unwrap();
            let len = text.matches('^').count();
            current_caret = Some((col, len));
            continue;
        }
        if expectation(text) {
            let r = text.trim_start()[2..].trim_start();
            if let Some(rest) = r.strip_prefix("[diag.") {
                let name_end = rest.find(']').unwrap_or(rest.len());
                let camel = &rest[..name_end];
                let after = &rest[name_end + 1..];
                let mut range = current_caret;
                if let Some(c) = after.strip_prefix("[column ") {
                    let c_end = c.find(']').unwrap();
                    let column: usize = c[..c_end].trim().parse().unwrap();
                    let l = &c[c_end + 1..];
                    let length: usize = l
                        .strip_prefix("[length ")
                        .and_then(|l| l.split(']').next())
                        .and_then(|n| n.trim().parse().ok())
                        .unwrap_or(0);
                    range = Some((column - 1, length));
                }
                if let Some((col, len)) = range {
                    let name = code_name_of(camel)
                        .map(str::to_string)
                        .unwrap_or_else(|| camel.to_string());
                    expected.push((name, last_line_start + col, len));
                }
            }
            continue;
        }
        current_caret = None;
        retained.push(line);
        last_line_start = code.encode_utf16().count();
        code.push_str(line);
    }
    // A terminator before removed marker lines is not a trailing one.
    if lines.last().is_some_and(|l| {
        let t = l.trim_end_matches(['\n', '\r']);
        caret(t) || expectation(t)
    }) {
        while code.ends_with('\n') || code.ends_with('\r') {
            code.pop();
        }
    }
    let _ = retained;
    // Columns are UTF-16 offsets within ASCII test lines.
    (code, expected)
}

/// Runs the [cases] whose names pass [filter]; returns (failures, run).
pub fn run_ported(cases: &[Ported], filter: impl Fn(&str) -> bool) -> (Vec<String>, usize) {
    run_ported_with_codes(cases, CODES, filter)
}

/// [run_ported] comparing the diagnostics of the codes listed in
/// [codes_text] (one code per line, `#` comments).
pub fn run_ported_with_codes(
    cases: &[Ported],
    codes_text: &str,
    filter: impl Fn(&str) -> bool,
) -> (Vec<String>, usize) {
    let codes: Vec<&str> = codes_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut failures = Vec::new();
    let mut count = 0;
    let Some(meta) = mock_package_files("addMetaPackageFiles") else {
        eprintln!("skipped: no SDK sources");
        return (failures, 0);
    };
    let angular = mock_package_files("addAngularMetaPackageFiles").unwrap_or_default();
    for case in cases.iter().filter(|c| filter(c.name)) {
        count += 1;
        let (code, mut expected) = match case.expected {
            None => parse_markers(case.source),
            Some(list) => (
                case.source.to_string(),
                list.iter()
                    .map(|&(camel, o, l)| {
                        (
                            code_name_of(camel)
                                .map(str::to_string)
                                .unwrap_or_else(|| camel.to_string()),
                            o,
                            l,
                        )
                    })
                    .collect(),
            ),
        };
        expected.retain(|(c, _, _)| codes.contains(&c.as_str()));
        let mut packages: Vec<(&str, Vec<(String, String)>)> =
            vec![("meta", meta.clone()), ("angular_meta", angular.clone())];
        let mut test_files: Vec<(&str, &str)> = vec![("test.dart", code.as_str())];
        for (name, files) in case.packages {
            if *name == "test" {
                test_files.extend(files.iter().copied());
            } else {
                packages.push((
                    name,
                    files
                        .iter()
                        .map(|(p, c)| (p.to_string(), c.to_string()))
                        .collect(),
                ));
            }
        }
        let options = AnalysisOptions {
            strict_inference: case.strict_inference,
            ..AnalysisOptions::default()
        };
        let Some(a) = analyze_in_packages(&test_files, &packages, options) else {
            eprintln!("skipped: no Dart SDK on PATH");
            return (failures, 0);
        };
        if let Some(panic) = &a.unit().panic {
            failures.push(format!("{}: panic {panic}", case.name));
            continue;
        }
        let mut actual: Vec<(String, usize, usize)> = a
            .unit()
            .diagnostics
            .iter()
            .filter(|d| codes.contains(&d.code.name))
            .map(|d| (d.code.name.to_string(), d.offset, d.length))
            .collect();
        actual.sort();
        expected.sort();
        if actual != expected {
            failures.push(format!(
                "{}:\n  expected {expected:?}\n  actual   {actual:?}",
                case.name
            ));
        }
    }
    (failures, count)
}
