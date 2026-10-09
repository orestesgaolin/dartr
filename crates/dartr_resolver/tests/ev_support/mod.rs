//! Support of the ErrorVerifier diagnostic tests (wave D, D4–D7): the port
//! of `PubPackageResolutionTest.assertErrorsInCode` from
//! `pkg/analyzer/test/src/diagnostics/*_test.dart`.
//!
//! Only the diagnostics whose codes are in
//! `tools/difftest/error_verifier_codes.txt` are compared (on both sides):
//! the other diagnostics come from other units (constants, error/*
//! verifiers, lints) and are tested there.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::support::analyze;

/// The codes of `tools/difftest/error_verifier_codes.txt`.
pub fn error_verifier_codes() -> &'static BTreeSet<String> {
    static CODES: OnceLock<BTreeSet<String>> = OnceLock::new();
    CODES.get_or_init(|| {
        include_str!("../../../../tools/difftest/error_verifier_codes.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect()
    })
}

/// Dart `assertErrorsInCode(code, [error(diag.x, offset, length), ...])`
/// for a library `main.dart` with [source]. [expected] is `(code, offset,
/// length)`, the code as the lower case name (`DiagnosticCode.lowerCaseName`).
/// Expected entries with a code that is not in
/// [error_verifier_codes] are ignored, like the actual ones.
pub fn assert_errors_in_code(source: &str, expected: &[(&str, usize, usize)]) {
    assert_errors_in_files(&[("main.dart", source)], expected);
}

/// Like [assert_errors_in_code], with more files (the first is
/// `main.dart`, the library that is checked; the others are imported or
/// parts).
pub fn assert_errors_in_files(files: &[(&str, &str)], expected: &[(&str, usize, usize)]) {
    let Some(a) = analyze(files) else {
        eprintln!("skipped: no Dart SDK on PATH");
        return;
    };
    let unit = a.unit();
    assert!(unit.panic.is_none(), "{:?}", unit.panic);
    let codes = error_verifier_codes();
    let mut actual: Vec<(String, usize, usize)> = unit
        .diagnostics
        .iter()
        .filter(|d| codes.contains(d.code.name))
        .map(|d| (d.code.name.to_string(), d.offset, d.length))
        .collect();
    actual.sort();
    let mut expected: Vec<(String, usize, usize)> = expected
        .iter()
        .filter(|e| codes.contains(e.0))
        .map(|&(c, o, l)| (c.to_string(), o, l))
        .collect();
    expected.sort();
    assert_eq!(actual, expected, "diagnostics of:\n{}", files[0].1);
}
