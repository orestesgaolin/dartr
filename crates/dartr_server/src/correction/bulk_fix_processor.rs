// Dart source: pkg/analysis_server/lib/src/services/correction/bulk_fix_processor.dart (BulkFixProcessor, ChangeMap, IterativeBulkFixProcessor)
// Dart source: pkg/analysis_server/lib/src/utilities/source_change_merger.dart (SourceChangeMerger)
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/organize_imports.dart (OrganizeImports)

//! The bulk fix processor: the fixes of all fixable diagnostics of a unit,
//! applied with one change builder (`dart fix` and the LSP "Fix All").

use indexmap::IndexMap;

use dartr_diagnostics::{Diagnostic, DiagnosticType};

use super::change::{SourceEdit, SourceFileEdit};
use super::change_builder::ChangeBuilder;
use super::fix_processor::{FixUnit, producer_names};
use super::organize_imports::{ImportOrganizer, Resolution};
use super::producer::*;
use super::producers;

/// Dart `IterativeBulkFixProcessor._maxPassCount` (`dart fix` uses the same
/// number of passes).
pub const MAX_PASS_COUNT: usize = 4;

/// Dart `BulkFixProcessor.nonLintMultiProducerMap`: the non-lint codes
/// (camel case names) that use the data-driven fixes in bulk.
const NON_LINT_MULTI_PRODUCER_CODES: &[&str] = &[
    "argumentTypeNotAssignable",
    "castToNonType",
    "dotShorthandUndefinedGetter",
    "dotShorthandUndefinedInvocation",
    "extendsNonClass",
    "implementsNonClass",
    "invalidOverride",
    "invalidOverrideSetter",
    "missingRequiredArgument",
    "mixinOfNonClass",
    "newWithUndefinedConstructorDefault",
    "nonTypeAsTypeArgument",
    "notEnoughPositionalArgumentsNamePlural",
    "notEnoughPositionalArgumentsNameSingular",
    "notEnoughPositionalArgumentsPlural",
    "notEnoughPositionalArgumentsSingular",
    "undefinedClass",
    "undefinedExtensionGetter",
    "undefinedFunction",
    "undefinedGetter",
    "undefinedIdentifier",
    "undefinedMethod",
    "undefinedNamedParameter",
    "undefinedSetter",
    "wrongNumberOfTypeArguments",
    "wrongNumberOfTypeArgumentsConstructor",
    "wrongNumberOfTypeArgumentsExtension",
    "wrongNumberOfTypeArgumentsElement",
    "deprecatedMemberUse",
    "deprecatedMemberUseWithMessage",
    "deprecatedExportUse",
    "overrideOnNonOverridingMethod",
];

/// Dart `ChangeMap`: the number of fixes by path and diagnostic code.
#[derive(Default, Debug, Clone)]
pub struct ChangeMap {
    pub library_map: IndexMap<String, IndexMap<String, usize>>,
}

impl ChangeMap {
    pub fn has_fixes(&self) -> bool {
        !self.library_map.is_empty()
    }

    pub fn add(&mut self, path: &str, code: &str) {
        *self
            .library_map
            .entry(path.to_string())
            .or_default()
            .entry(code.to_string())
            .or_insert(0) += 1;
    }
}

/// Dart `file_paths.isGenerated`.
pub fn is_generated(path: &str) -> bool {
    [
        ".g.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbserver.dart",
        ".pbjson.dart",
        ".template.dart",
    ]
    .iter()
    .any(|s| path.ends_with(s))
}

fn is_directives_ordering(d: &Diagnostic) -> bool {
    d.code.diagnostic_type == DiagnosticType::Lint && d.code.name == "directives_ordering"
}

fn is_unused_import_code(d: &Diagnostic) -> bool {
    matches!(
        d.code.camel_case_name,
        "duplicateImport" | "unnecessaryImport" | "unusedImport"
    ) && d.code.diagnostic_type != DiagnosticType::Lint
}

/// Dart `BulkFixProcessor._canBulkFix` (the producers are made with the
/// context of [d]: the applicability does not depend on it).
fn can_bulk_fix(unit: &FixUnit<'_>, d: &Diagnostic) -> bool {
    let (single, multi) = producer_names(d);
    let c = unit.context(d, true);
    if single.iter().any(|name| {
        producers::generator(name)
            .is_some_and(|g| can_be_applied_across_files(g(&c).applicability()))
    }) {
        return true;
    }
    if d.code.diagnostic_type == DiagnosticType::Lint {
        return !multi.is_empty();
    }
    !multi.is_empty() || NON_LINT_MULTI_PRODUCER_CODES.contains(&d.code.camel_case_name)
}

/// Dart `Diagnostic.isFixable` of the bulk fix processor.
pub fn is_fixable(unit: &FixUnit<'_>, d: &Diagnostic) -> bool {
    is_unused_import_code(d) || is_directives_ordering(d) || can_bulk_fix(unit, d)
}

/// Dart `canBeAppliedAcrossFiles`.
pub fn can_be_applied_across_files(a: Applicability) -> bool {
    matches!(
        a,
        Applicability::AcrossFiles
            | Applicability::Automatically
            | Applicability::AutomaticallyButOncePerFile
    )
}

/// Dart `canBeAppliedAutomatically`.
pub fn can_be_applied_automatically(a: Applicability) -> bool {
    matches!(
        a,
        Applicability::Automatically | Applicability::AutomaticallyButOncePerFile
    )
}

/// Dart `_fixOrder`: by offset, `annotate_overrides` first at an offset.
fn fix_order(a: &Diagnostic, b: &Diagnostic) -> std::cmp::Ordering {
    a.offset.cmp(&b.offset).then_with(|| {
        let ao = a.code.camel_case_name == "annotateOverrides";
        let bo = b.code.camel_case_name == "annotateOverrides";
        match (ao, bo) {
            (true, _) => std::cmp::Ordering::Less,
            (_, true) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    })
}

fn dart_insertion_sort<T: Copy, F: Fn(&T, &T) -> std::cmp::Ordering>(
    a: &mut [T],
    compare: &F,
    left: isize,
    right: isize,
) {
    let mut i = left + 1;
    while i <= right {
        let el = a[i as usize];
        let mut j = i;
        while j > left && compare(&a[(j - 1) as usize], &el) == std::cmp::Ordering::Greater {
            a[j as usize] = a[(j - 1) as usize];
            j -= 1;
        }
        a[j as usize] = el;
        i += 1;
    }
}

fn dart_do_sort<T: Copy, F: Fn(&T, &T) -> std::cmp::Ordering>(
    a: &mut [T],
    compare: &F,
    left: isize,
    right: isize,
) {
    if right - left <= 32 {
        dart_insertion_sort(a, compare, left, right);
    } else {
        dart_dual_pivot_quicksort(a, compare, left, right);
    }
}

fn dart_dual_pivot_quicksort<T: Copy, F: Fn(&T, &T) -> std::cmp::Ordering>(
    a: &mut [T],
    compare: &F,
    left: isize,
    right: isize,
) {
    let sixth = (right - left + 1) / 6;
    let index1 = left + sixth;
    let index5 = right - sixth;
    let index3 = (left + right) / 2;
    let index2 = index3 - sixth;
    let index4 = index3 + sixth;

    let mut el1 = a[index1 as usize];
    let mut el2 = a[index2 as usize];
    let mut el3 = a[index3 as usize];
    let mut el4 = a[index4 as usize];
    let mut el5 = a[index5 as usize];

    if compare(&el1, &el2) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el1, &mut el2);
    }
    if compare(&el4, &el5) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el4, &mut el5);
    }
    if compare(&el1, &el3) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el1, &mut el3);
    }
    if compare(&el2, &el3) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el1, &el4) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el1, &mut el4);
    }
    if compare(&el3, &el4) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el3, &mut el4);
    }
    if compare(&el2, &el5) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el2, &mut el5);
    }
    if compare(&el2, &el3) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el4, &el5) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut el4, &mut el5);
    }

    let pivot1 = el2;
    let pivot2 = el4;

    a[index1 as usize] = el1;
    a[index3 as usize] = el3;
    a[index5 as usize] = el5;

    a[index2 as usize] = a[left as usize];
    a[index4 as usize] = a[right as usize];

    let mut less = left + 1;
    let mut great = right - 1;
    let pivots_are_equal = compare(&pivot1, &pivot2) == std::cmp::Ordering::Equal;

    if pivots_are_equal {
        let pivot = pivot1;
        let mut k = less;
        while k <= great {
            let ak = a[k as usize];
            let comp = compare(&ak, &pivot);
            if comp == std::cmp::Ordering::Equal {
                k += 1;
                continue;
            }
            if comp == std::cmp::Ordering::Less {
                if k != less {
                    a[k as usize] = a[less as usize];
                    a[less as usize] = ak;
                }
                less += 1;
            } else {
                loop {
                    let comp = compare(&a[great as usize], &pivot);
                    if comp == std::cmp::Ordering::Greater {
                        great -= 1;
                        continue;
                    } else if comp == std::cmp::Ordering::Less {
                        a[k as usize] = a[less as usize];
                        a[less as usize] = a[great as usize];
                        less += 1;
                        a[great as usize] = ak;
                        great -= 1;
                        break;
                    } else {
                        a[k as usize] = a[great as usize];
                        a[great as usize] = ak;
                        great -= 1;
                        break;
                    }
                }
            }
            k += 1;
        }
    } else {
        let mut k = less;
        while k <= great {
            let ak = a[k as usize];
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 == std::cmp::Ordering::Less {
                if k != less {
                    a[k as usize] = a[less as usize];
                    a[less as usize] = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 == std::cmp::Ordering::Greater {
                    loop {
                        let comp = compare(&a[great as usize], &pivot2);
                        if comp == std::cmp::Ordering::Greater {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            let comp = compare(&a[great as usize], &pivot1);
                            if comp == std::cmp::Ordering::Less {
                                a[k as usize] = a[less as usize];
                                a[less as usize] = a[great as usize];
                                less += 1;
                                a[great as usize] = ak;
                                great -= 1;
                            } else {
                                a[k as usize] = a[great as usize];
                                a[great as usize] = ak;
                                great -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
    }

    a[left as usize] = a[(less - 1) as usize];
    a[(less - 1) as usize] = pivot1;
    a[right as usize] = a[(great + 1) as usize];
    a[(great + 1) as usize] = pivot2;

    dart_do_sort(a, compare, left, less - 2);
    dart_do_sort(a, compare, great + 2, right);

    if pivots_are_equal {
        return;
    }

    if less < index1 && great > index5 {
        while compare(&a[less as usize], &pivot1) == std::cmp::Ordering::Equal {
            less += 1;
        }
        while compare(&a[great as usize], &pivot2) == std::cmp::Ordering::Equal {
            great -= 1;
        }

        let mut k = less;
        while k <= great {
            let ak = a[k as usize];
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 == std::cmp::Ordering::Equal {
                if k != less {
                    a[k as usize] = a[less as usize];
                    a[less as usize] = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 == std::cmp::Ordering::Equal {
                    loop {
                        let comp = compare(&a[great as usize], &pivot2);
                        if comp == std::cmp::Ordering::Equal {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            if compare(&a[great as usize], &pivot1) == std::cmp::Ordering::Equal {
                                a[k as usize] = a[less as usize];
                                a[less as usize] = a[great as usize];
                                less += 1;
                            } else {
                                a[k as usize] = a[great as usize];
                            }
                            a[great as usize] = ak;
                            great -= 1;
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
        dart_do_sort(a, compare, less, great);
    } else {
        dart_do_sort(a, compare, less, great);
    }
}

/// Dart `_filterDiagnostics`: sorted, only [codes] (lower case names) when
/// given.
pub fn filter_diagnostics<'d>(
    diagnostics: &'d [Diagnostic],
    codes: Option<&[String]>,
) -> Vec<&'d Diagnostic> {
    let mut out: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| codes.is_none_or(|c| c.iter().any(|x| x == d.code.lower_case_name())))
        .collect();
    if !out.is_empty() {
        let right = (out.len() - 1) as isize;
        dart_do_sort(&mut out, &|a, b| fix_order(a, b), 0, right);
    }
    out
}

/// Dart `BulkFixProcessor` for one unit.
pub struct BulkFixer<'b, 'w> {
    pub builder: &'b mut ChangeBuilder<'w>,
    pub change_map: ChangeMap,
    pub codes: Option<Vec<String>>,
    pub auto_triggered: bool,
}

impl BulkFixer<'_, '_> {
    /// Dart `_applyProducer`.
    fn apply_producer(&mut self, c: &ProducerContext<'_>, producer: &mut dyn CorrectionProducer) {
        self.builder.current_change_description =
            producer.fix_kind().map(|k| k.message.to_string());
        producer.compute(c, self.builder);
        self.builder.current_change_description = None;
        if self.builder.conflict {
            self.builder.conflict = false;
            self.builder.revert();
        } else {
            self.builder.commit();
        }
    }

    /// Dart `_generateFix`.
    fn generate_fix(
        &mut self,
        c: &ProducerContext<'_>,
        producer: &mut dyn CorrectionProducer,
        code: &str,
    ) {
        let old = self.builder.data.modification_count;
        self.apply_producer(c, producer);
        if self.builder.data.modification_count != old {
            self.change_map.add(c.path, code);
        }
    }

    /// Dart `_bulkApply`.
    fn bulk_apply(&mut self, c: &ProducerContext<'_>, names: &[&str], code: &str) {
        for name in names {
            let Some(generator) = producers::generator(name) else {
                continue;
            };
            let mut producer = generator(c);
            let applicability = producer.applicability();
            let should_fix = if self.auto_triggered {
                can_be_applied_automatically(applicability)
            } else {
                can_be_applied_across_files(applicability)
            };
            if should_fix {
                self.generate_fix(c, producer.as_mut(), code);
            }
        }
    }

    /// Dart `_fixSingleDiagnostic`.
    fn fix_single_diagnostic(&mut self, unit: &FixUnit<'_>, d: &Diagnostic) {
        let c = unit.context(d, true);
        let code = d.code.lower_case_name();
        let (single, multi) = producer_names(d);
        self.bulk_apply(&c, single, code);
        let multi: &[&str] = if d.code.diagnostic_type == DiagnosticType::Lint {
            multi
        } else if NON_LINT_MULTI_PRODUCER_CODES.contains(&d.code.camel_case_name) {
            &["DataDriven.new"]
        } else {
            &[]
        };
        for name in multi {
            let Some(generator) = producers::multi_generator(name) else {
                continue;
            };
            let list = generator(&c, &mut *self.builder.workspace);
            for mut producer in list {
                self.generate_fix(&c, producer.as_mut(), code);
            }
        }
    }

    /// Dart `_fixErrorsInLibraryUnit`.
    pub fn fix_errors_in_unit(&mut self, unit: &FixUnit<'_>, is_defining_unit: bool) {
        let codes = self.codes.clone();
        let diagnostics = filter_diagnostics(unit.diagnostics, codes.as_deref());
        for d in &diagnostics {
            self.fix_single_diagnostic(unit, d);
        }
        if !is_defining_unit || self.auto_triggered || self.builder.has_edits_for(unit.path) {
            return;
        }
        let mut ordering: Option<&Diagnostic> = None;
        let mut unused: Vec<&Diagnostic> = Vec::new();
        for d in &diagnostics {
            if d.code.diagnostic_type == DiagnosticType::Lint {
                if is_directives_ordering(d) {
                    ordering = Some(d);
                    break;
                }
            } else if is_unused_import_code(d) {
                unused.push(d);
            }
        }
        if let Some(d) = ordering {
            let c = unit.context(d, true);
            let mut producer = OrganizeImports;
            self.generate_fix(&c, &mut producer, d.code.lower_case_name());
        } else {
            for d in unused {
                let c = unit.context(d, true);
                let mut producer = producers::simple::RemoveUnusedImport;
                self.generate_fix(&c, &mut producer, d.code.lower_case_name());
            }
        }
    }
}

/// Dart `OrganizeImports` (the producer).
pub struct OrganizeImports;

impl CorrectionProducer for OrganizeImports {
    fn fix_kind(&self) -> Option<&'static super::fix_kind::FixKind> {
        Some(&super::generated::fix_kinds::ORGANIZE_IMPORTS)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let organizer = ImportOrganizer {
            resolution: Some(Resolution {
                ctx: c.ctx,
                tables: c.tables,
                fragment: c.resolved.unit().fragment,
            }),
            ast: c.ast,
            unit: c.unit,
            line_info: c.line_info,
            diagnostics: c.diagnostics,
            remove_unused: true,
        };
        for edit in organizer.organize() {
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(edit.offset, edit.length, &edit.replacement)
            });
        }
    }
}

/// Dart `SourceChangeMerger.merge`.
pub fn merge_changes(edits: Vec<SourceFileEdit>) -> Vec<SourceFileEdit> {
    let mut grouped: IndexMap<String, Vec<SourceFileEdit>> = IndexMap::new();
    for e in edits {
        grouped.entry(e.file.clone()).or_default().push(e);
    }
    let mut results = Vec::new();
    for (_, lists) in grouped {
        if lists.len() == 1 {
            results.extend(lists);
            continue;
        }
        let mut first = lists[0].clone();
        let mut all: Vec<SourceEdit> = lists.into_iter().flat_map(|l| l.edits).collect();
        reorder(&mut all);
        merge(&mut all);
        first.edits = all;
        results.push(first);
    }
    results
}

fn utf16_len(s: &str) -> i64 {
    s.encode_utf16().count() as i64
}

fn delta(e: &SourceEdit) -> i64 {
    utf16_len(&e.replacement) - e.length as i64
}

fn end(e: &SourceEdit) -> i64 {
    e.offset as i64 + e.length as i64
}

/// Dart `SourceChangeMerger._reorder`.
fn reorder(edits: &mut [SourceEdit]) {
    for i in 1..edits.len() {
        let mut current = edits[i].clone();
        let mut j = i as i64 - 1;
        while j >= 0 && current.offset as i64 >= end(&edits[j as usize]) + delta(&edits[j as usize])
        {
            let d = delta(&edits[j as usize]);
            current = SourceEdit {
                description: None,
                offset: (current.offset as i64 - d) as u32,
                length: current.length,
                replacement: current.replacement.clone(),
                id: current.id,
            };
            edits[j as usize + 1] = edits[j as usize].clone();
            j -= 1;
        }
        edits[(j + 1) as usize] = current;
    }
}

/// Dart `SourceChangeMerger._merge`.
fn merge(edits: &mut Vec<SourceEdit>) {
    let mut i = 0;
    while i + 1 < edits.len() {
        if end(&edits[i + 1]) < edits[i].offset as i64 {
            i += 1;
            continue;
        }
        // Process the merged edit again in case it also overlaps the next.
        edits[i] = merge_edits(&edits[i], &edits[i + 1]);
        edits.remove(i + 1);
    }
}

/// Dart `SourceChangeMerger._mergeEdits`.
fn merge_edits(first: &SourceEdit, second: &SourceEdit) -> SourceEdit {
    let first_units: Vec<u16> = first.replacement.encode_utf16().collect();
    let actual_start = first.offset.min(second.offset) as i64;
    let actual_end = end(first).max(end(second) - delta(first));
    let length = actual_end - actual_start;
    let prefix: Vec<u16> = if second.offset > first.offset {
        first_units[..(second.offset - first.offset) as usize].to_vec()
    } else {
        Vec::new()
    };
    let suffix: Vec<u16> = if end(second) < first.offset as i64 + first_units.len() as i64 {
        first_units[(end(second) - first.offset as i64) as usize..].to_vec()
    } else {
        Vec::new()
    };
    let mut text = String::from_utf16_lossy(&prefix);
    text.push_str(&second.replacement);
    text.push_str(&String::from_utf16_lossy(&suffix));
    SourceEdit {
        description: None,
        offset: actual_start as u32,
        length: length as u32,
        replacement: text,
        id: 0,
    }
}

/// Applies [edits] (from the last to the first, Dart `SourceEdit.applySequence`)
/// to [content].
pub fn apply_edits(content: &str, edits: &[SourceEdit]) -> String {
    let mut units: Vec<u16> = content.encode_utf16().collect();
    for e in edits {
        let start = (e.offset as usize).min(units.len());
        let stop = (start + e.length as usize).min(units.len());
        let replacement: Vec<u16> = e.replacement.encode_utf16().collect();
        units.splice(start..stop, replacement);
    }
    String::from_utf16_lossy(&units)
}
