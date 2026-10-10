// Dart source: pkg/analyzer/lib/src/error/ignore_validator.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_filterIgnoredDiagnostics)

//! Validation of ignore comments (`IgnoreValidator`) and the removal of the
//! ignored diagnostics (`LibraryAnalyzer._filterIgnoredDiagnostics`).
//!
//! The `unnecessary_ignore` lint and the `removed_lint_use` /
//! `replaced_lint_use` reports run after all other diagnostics of the file
//! are known ([validate_ignores] with `validate_unnecessary_ignores`).

use std::collections::HashSet;

use dartr_diagnostics::{Diagnostic, diag};
use dartr_syntax::LineInfo;

use crate::ignore_info::{IgnoreInfo, IgnoredElement};

/// Dart `IgnoreValidator.reportErrors`: returns the `unignorable_ignore`
/// and `duplicate_ignore` diagnostics, and, when
/// [validate_unnecessary_ignores] (the `unnecessary_ignore` lint is
/// enabled), the unnecessary ignores of the names that no diagnostic of
/// [reported_diagnostics] uses and the ignores of removed or replaced lints.
/// [unignorable_names] are lower case. The names of [unknown_names]
/// (enabled lint rules that dartr does not implement) are never reported as
/// unnecessary.
pub fn validate_ignores(
    ignore_info: &IgnoreInfo,
    reported_diagnostics: &[Diagnostic],
    line_info: &LineInfo,
    unignorable_names: &HashSet<String>,
    validate_unnecessary_ignores: bool,
    unknown_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    let mut reported = Vec::new();
    if !ignore_info.has_ignores() {
        return reported;
    }
    let mut ignored_on_line_map = ignore_info.ignored_on_line();
    let mut ignored_for_file = ignore_info.ignored_for_file();
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
    let removed_for_file: Vec<IgnoredElement> = unignorable
        .iter()
        .chain(&duplicated)
        .map(|e| (*e).clone())
        .collect();
    remove_elements(&mut ignored_for_file, &removed_for_file);
    for elements in ignored_on_line_map.values_mut() {
        let mut names_on_line = HashSet::new();
        let mut types_on_line = HashSet::new();
        let mut unignorable = Vec::new();
        let mut duplicated = Vec::new();
        for element in elements.iter() {
            match element {
                IgnoredElement::Name { name, .. } => {
                    if unignorable_names.contains(name) {
                        unignorable.push(element);
                    } else if names_for_file.contains(name) || !names_on_line.insert(name.clone()) {
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
        let removed: Vec<IgnoredElement> = unignorable
            .iter()
            .chain(&duplicated)
            .map(|e| (*e).clone())
            .collect();
        remove_elements(elements, &removed);
    }

    // Remove all of the names that a diagnostic uses.
    for diagnostic in reported_diagnostics {
        let line_number = line_info.get_location(diagnostic.offset as u32).line_number;
        let name = diagnostic.code.lower_case_name();
        remove_by_name(&mut ignored_for_file, name);
        if let Some(on_line) = ignored_on_line_map.get_mut(&line_number) {
            remove_by_name(on_line, name);
        }
    }
    if validate_unnecessary_ignores {
        remove_unknown_names(&mut ignored_for_file, unknown_names);
        for elements in ignored_on_line_map.values_mut() {
            remove_unknown_names(elements, unknown_names);
        }
        report_unnecessary_or_removed_ignores(
            ignore_info,
            line_info,
            &ignored_for_file,
            true,
            &mut reported,
        );
        for elements in ignored_on_line_map.values() {
            report_unnecessary_or_removed_ignores(
                ignore_info,
                line_info,
                elements,
                false,
                &mut reported,
            );
        }
    }
    reported
}

/// Dart `List.remove` of each of [removed] (the first equal element).
fn remove_elements(list: &mut Vec<IgnoredElement>, removed: &[IgnoredElement]) {
    for element in removed {
        if let Some(index) = list.iter().position(|e| e == element) {
            list.remove(index);
        }
    }
}

/// Removes the names of the lint rules that dartr does not implement (not
/// in Dart: the analyzer knows every diagnostic of the file).
fn remove_unknown_names(list: &mut Vec<IgnoredElement>, unknown_names: &HashSet<String>) {
    list.retain(
        |e| !matches!(e, IgnoredElement::Name { name, .. } if unknown_names.contains(name)),
    );
}

/// Dart `List<IgnoredElement>.removeByName`.
fn remove_by_name(list: &mut Vec<IgnoredElement>, name: &str) {
    list.retain(|e| !matches!(e, IgnoredElement::Name { name: n, .. } if n == name));
}

/// Dart `IgnoreValidator._reportUnnecessaryOrRemovedOrDeprecatedIgnores`.
fn report_unnecessary_or_removed_ignores(
    ignore_info: &IgnoreInfo,
    line_info: &LineInfo,
    ignored_names: &[IgnoredElement],
    for_file: bool,
    reported: &mut Vec<Diagnostic>,
) {
    use dartr_lints::RuleStateType;
    // Dart `_validDiagnosticCodeNames`.
    let valid_names = || {
        dartr_diagnostics::all_codes()
            .iter()
            .map(|c| c.lower_case_name())
    };
    for element in ignored_names {
        let IgnoredElement::Name { name, offset, .. } = element else {
            continue;
        };
        let length = utf16_len(name);
        match dartr_lints::ALL_RULES
            .iter()
            .find(|r| r.name == name.as_str())
        {
            None => {
                let lower = name.to_lowercase();
                if !valid_names().any(|n| n == lower) {
                    continue;
                }
            }
            Some(rule) => {
                let since = rule
                    .state
                    .since
                    .map(|(a, b, c)| format!("{a}.{b}.{c}"))
                    .unwrap_or_default();
                if matches!(rule.state.kind, RuleStateType::Removed) {
                    let diagnostic = match rule.state.replaced_by {
                        Some(replacement) => diag::replaced_lint_use(name, &since, replacement),
                        None => diag::removed_lint_use(name, &since),
                    };
                    reported.push(
                        diagnostic
                            .at_offset(*offset as usize, length)
                            .into_diagnostic(),
                    );
                    continue;
                }
            }
        }
        let current_line = line_info.get_location(*offset).line_number;
        let ignored_elements: Vec<IgnoredElement> = if for_file {
            ignore_info.ignored_for_file()
        } else {
            // Dart `{...?ignoredOnLine[currentLine], ...?ignoredOnLine[currentLine + 1]}`.
            let map = ignore_info.ignored_on_line();
            let mut set: Vec<IgnoredElement> = Vec::new();
            for line in [current_line, current_line + 1] {
                for e in map.get(&line).into_iter().flatten() {
                    if !set.contains(e) {
                        set.push(e.clone());
                    }
                }
            }
            set
        };
        let diagnostics_on_line = ignored_elements
            .iter()
            .filter(|e| match e {
                IgnoredElement::Name { offset, .. } => {
                    line_info.get_location(*offset).line_number == current_line
                }
                _ => false,
            })
            .count();
        let diagnostic = match (for_file, diagnostics_on_line > 1) {
            (true, true) => diag::unnecessary_ignore_name_file(name),
            (true, false) => diag::unnecessary_ignore_file(name),
            (false, true) => diag::unnecessary_ignore_name(name),
            (false, false) => diag::unnecessary_ignore(name),
        };
        reported.push(
            diagnostic
                .at_offset(*offset as usize, length)
                .into_diagnostic(),
        );
    }
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
            unignorable_names.contains(d.code.lower_case_name())
                || !ignore_info.ignored(d, line_info)
        })
        .collect()
}
