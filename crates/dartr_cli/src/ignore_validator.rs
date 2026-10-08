// Dart source: pkg/analyzer/lib/src/error/ignore_validator.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_filterIgnoredDiagnostics)

//! Validation of ignore comments (`IgnoreValidator`) and the removal of the
//! ignored diagnostics (`LibraryAnalyzer._filterIgnoredDiagnostics`).
//!
//! Not ported yet: the `unnecessary_ignore` lint (needs the lint rule
//! registry), and the `removed_lint_use` / `replaced_lint_use` reports that
//! belong to it.

use std::collections::HashSet;

use dartr_diagnostics::{Diagnostic, diag};
use dartr_syntax::LineInfo;

use crate::ignore_info::{IgnoreInfo, IgnoredElement};

/// Dart `IgnoreValidator.reportErrors` without the unnecessary-ignore part:
/// returns the `unignorable_ignore` and `duplicate_ignore` diagnostics.
/// [unignorable_names] are lower case.
pub fn validate_ignores(
    ignore_info: &IgnoreInfo,
    unignorable_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    let mut reported = Vec::new();
    if !ignore_info.has_ignores() {
        return reported;
    }
    let ignored_for_file = ignore_info.ignored_for_file();
    let mut names_for_file = HashSet::new();
    let mut types_for_file = HashSet::new();
    let mut unignorable = Vec::new();
    let mut duplicated = Vec::new();
    for element in &ignored_for_file {
        match element {
            IgnoredElement::Name { name, .. } => {
                if unignorable_names.contains(name) {
                    unignorable.push(element);
                } else if !names_for_file.insert(name.clone()) {
                    duplicated.push(element);
                }
            }
            IgnoredElement::Type { type_, .. } => {
                if !types_for_file.insert(type_.clone()) {
                    duplicated.push(element);
                }
            }
            IgnoredElement::Comment { .. } => {}
        }
    }
    report(&unignorable, &duplicated, &mut reported);
    for elements in ignore_info.ignored_on_line().values() {
        let mut names_on_line = HashSet::new();
        let mut types_on_line = HashSet::new();
        let mut unignorable = Vec::new();
        let mut duplicated = Vec::new();
        for element in elements {
            match element {
                IgnoredElement::Name { name, .. } => {
                    if unignorable_names.contains(name) {
                        unignorable.push(element);
                    } else if names_for_file.contains(name) || !names_on_line.insert(name.clone())
                    {
                        duplicated.push(element);
                    }
                }
                IgnoredElement::Type { type_, .. } => {
                    if types_for_file.contains(type_) || !types_on_line.insert(type_.clone()) {
                        duplicated.push(element);
                    }
                }
                IgnoredElement::Comment { .. } => {}
            }
        }
        report(&unignorable, &duplicated, &mut reported);
    }
    reported
}

/// Dart `_reportUnignorableAndDuplicateIgnores` (the removal from the lists
/// only matters for the unnecessary-ignore part, which is not ported).
fn report(
    unignorable: &[&IgnoredElement],
    duplicated: &[&IgnoredElement],
    reported: &mut Vec<Diagnostic>,
) {
    for element in unignorable {
        if let IgnoredElement::Name { name, offset, .. } = element {
            reported.push(
                diag::unignorable_ignore(name)
                    .at_offset(*offset as usize, utf16_len(name))
                    .into_diagnostic(),
            );
        }
    }
    for element in duplicated {
        match element {
            IgnoredElement::Name { name, offset, .. } => reported.push(
                diag::duplicate_ignore(name)
                    .at_offset(*offset as usize, utf16_len(name))
                    .into_diagnostic(),
            ),
            IgnoredElement::Type {
                type_,
                offset,
                length,
            } => reported.push(
                diag::duplicate_ignore(type_)
                    .at_offset(*offset as usize, *length as usize)
                    .into_diagnostic(),
            ),
            IgnoredElement::Comment { .. } => {}
        }
    }
}

/// Dart `file_paths.isGenerated`: the ignore comments of generated files are
/// not validated.
pub fn is_generated(path: &str) -> bool {
    const SUFFIXES: [&str; 6] = [
        ".g.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbserver.dart",
        ".pbjson.dart",
        ".template.dart",
    ];
    SUFFIXES.iter().any(|s| path.ends_with(s))
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Dart `LibraryAnalyzer._filterIgnoredDiagnostics`: removes the diagnostics
/// that an ignore comment suppresses, except the unignorable ones.
pub fn filter_ignored_diagnostics(
    diagnostics: Vec<Diagnostic>,
    ignore_info: &IgnoreInfo,
    line_info: &LineInfo,
    unignorable_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    if diagnostics.is_empty() || !ignore_info.has_ignores() {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .filter(|d| {
            unignorable_names.contains(d.code.lower_case_name()) || !ignore_info.ignored(d, line_info)
        })
        .collect()
}
