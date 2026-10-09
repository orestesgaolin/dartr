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
