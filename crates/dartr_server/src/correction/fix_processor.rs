// Dart source: pkg/analysis_server_plugin/lib/src/correction/fix_processor.dart (computeFixes, FixProcessor)
// Dart source: pkg/analysis_server_plugin/lib/src/correction/fix_in_file_processor.dart (FixInFileProcessor)
// Dart source: pkg/analysis_server_plugin/lib/src/correction/fix_generators.dart (registeredFixGenerators)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix_internal.dart (registerBuiltInFixGenerators)

//! The fix processors: the fixes of one diagnostic from the registered
//! producers, the "fix all in file" fixes, and the ignore fixes.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::OnceLock;

use dartr_diagnostics::{Diagnostic, DiagnosticType};
use dartr_element::Ctx;
use dartr_project::AnalysisOptions;

use super::change::SourceChange;
use super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::fix_kind::{FixKind, IGNORE_ERROR_ANALYSIS_FILE, format_list};
use super::generated::fix_registry;
use super::ignore::{IgnoreDiagnostic, IgnoreKind};
use super::producer::*;
use super::producers;
use super::utils::CorrectionUtils;
use crate::server::ResolvedUnitRef;

/// Dart `Fix`.
#[derive(Clone, Debug)]
pub struct Fix {
    pub kind: &'static FixKind,
    pub change: SourceChange,
}

/// The resolved unit that fixes are computed for (Dart `DartFixContext`
/// without the diagnostic).
pub struct FixUnit<'a> {
    pub resolved: &'a Rc<ResolvedUnitRef>,
    pub ctx: &'a Ctx<'a>,
    pub utils: &'a CorrectionUtils<'a>,
    pub path: &'a str,
    pub options: &'a Rc<AnalysisOptions>,
    /// Dart `unitResult.diagnostics`.
    pub diagnostics: &'a [Diagnostic],
}

impl<'a> FixUnit<'a> {
    fn context<'b>(&'b self, diagnostic: &'b Diagnostic, bulk: bool) -> ProducerContext<'b>
    where
        'a: 'b,
    {
        ProducerContext::new(
            self.resolved,
            self.ctx,
            self.utils,
            self.path,
            self.options,
            Some(diagnostic),
            self.diagnostics,
            diagnostic.offset as u32,
            diagnostic.length as u32,
            bulk,
        )
    }
}

/// The registered producer names of a diagnostic code: the single
/// producers and the multi producers (Dart `lintProducers` /
/// `warningProducers` and the multi variants).
struct Registry {
    lint: HashMap<&'static str, &'static [&'static str]>,
    lint_multi: HashMap<&'static str, &'static [&'static str]>,
    warning: HashMap<&'static str, &'static [&'static str]>,
    warning_multi: HashMap<&'static str, &'static [&'static str]>,
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let map = |entries: &'static [(&'static str, &'static [&'static str])]| {
            entries.iter().map(|(k, v)| (*k, *v)).collect()
        };
        Registry {
            lint: map(fix_registry::LINT_PRODUCERS),
            lint_multi: map(fix_registry::LINT_MULTI_PRODUCERS),
            warning: map(fix_registry::WARNING_PRODUCERS),
            warning_multi: map(fix_registry::WARNING_MULTI_PRODUCERS),
        }
    })
}

fn is_lint(d: &Diagnostic) -> bool {
    d.code.diagnostic_type == DiagnosticType::Lint
}

/// The producer names of [d] (single, multi).
fn producer_names(d: &Diagnostic) -> (&'static [&'static str], &'static [&'static str]) {
    let r = registry();
    let name = d.code.camel_case_name;
    if is_lint(d) {
        (
            r.lint.get(name).copied().unwrap_or(&[]),
            r.lint_multi.get(name).copied().unwrap_or(&[]),
        )
    } else {
        (
            r.warning.get(name).copied().unwrap_or(&[]),
            r.warning_multi.get(name).copied().unwrap_or(&[]),
        )
    }
}

/// Whether any producer is registered for [d] (including the ignore
/// producers).
pub fn has_fixes(d: &Diagnostic) -> bool {
    let (single, multi) = producer_names(d);
    !single.is_empty()
        || !multi.is_empty()
        || matches!(
            d.code.diagnostic_type,
            DiagnosticType::Lint | DiagnosticType::Hint | DiagnosticType::StaticWarning
        )
}

/// Dart `computeFixes`: the fixes of [diagnostic], then the "fix all in
/// file" fixes.
pub fn compute_fixes(
    unit: &FixUnit<'_>,
    diagnostic: &Diagnostic,
    workspace: &mut dyn ChangeWorkspace,
    mut already_calculated: Option<&mut HashSet<String>>,
) -> Vec<Fix> {
    let mut fixes = fix_processor(
        unit,
        diagnostic,
        workspace,
        already_calculated.as_deref_mut(),
    );
    fixes.extend(fix_in_file_processor(
        unit,
        diagnostic,
        workspace,
        already_calculated,
    ));
    fixes
}

/// Dart `FixProcessor._addFromProducer`.
fn add_from_producer(
    c: &ProducerContext<'_>,
    mut producer: Box<dyn CorrectionProducer>,
    workspace: &mut dyn ChangeWorkspace,
    fixes: &mut Vec<Fix>,
) {
    let Some(kind) = producer.fix_kind() else {
        return;
    };
    let eol = c.utils.end_of_line.clone();
    let mut builder = ChangeBuilder::new(workspace, Some(&eol));
    producer.compute(c, &mut builder);
    if builder.conflict {
        return;
    }
    let mut change = builder.source_change();
    if change.edits.is_empty() {
        return;
    }
    change.id = Some(kind.id.to_string());
    change.message = format_list(kind.message, &producer.fix_arguments());
    fixes.push(Fix { kind, change });
}

/// Dart `FixProcessor.compute`.
fn fix_processor(
    unit: &FixUnit<'_>,
    diagnostic: &Diagnostic,
    workspace: &mut dyn ChangeWorkspace,
    already_calculated: Option<&mut HashSet<String>>,
) -> Vec<Fix> {
    let mut fixes = Vec::new();
    let c = unit.context(diagnostic, false);
    let (single, multi) = producer_names(diagnostic);
    for name in single {
        if let Some(generator) = producers::generator(name) {
            add_from_producer(&c, generator(&c), workspace, &mut fixes);
        }
    }
    for name in multi {
        if let Some(generator) = producers::multi_generator(name) {
            for producer in generator(&c, workspace) {
                add_from_producer(&c, producer, workspace, &mut fixes);
            }
        }
    }
    if matches!(
        diagnostic.code.diagnostic_type,
        DiagnosticType::Lint | DiagnosticType::Hint | DiagnosticType::StaticWarning
    ) {
        let mut already_calculated = already_calculated;
        for kind in [
            IgnoreKind::Line,
            IgnoreKind::File,
            IgnoreKind::AnalysisOptions,
        ] {
            if kind == IgnoreKind::AnalysisOptions {
                if let Some(set) = already_calculated.as_deref_mut() {
                    let key = format!(
                        "IgnoreDiagnosticInAnalysisOptionsFile|{}|{}",
                        IGNORE_ERROR_ANALYSIS_FILE.id,
                        diagnostic.code.lower_case_name()
                    );
                    if !set.insert(key) {
                        continue;
                    }
                }
            }
            let producer = Box::new(IgnoreDiagnostic::new(kind, &c));
            add_from_producer(&c, producer, workspace, &mut fixes);
        }
    }
    fixes
}

/// Dart `FixInFileProcessor.compute`.
fn fix_in_file_processor(
    unit: &FixUnit<'_>,
    diagnostic: &Diagnostic,
    workspace: &mut dyn ChangeWorkspace,
    mut already_calculated: Option<&mut HashSet<String>>,
) -> Vec<Fix> {
    let code = diagnostic.code.lower_case_name();
    let (single, _) = producer_names(diagnostic);
    let key = |name: &str| format!("{name}|{code}");
    let generators: Vec<&str> = single
        .iter()
        .copied()
        .filter(|name| {
            already_calculated
                .as_deref()
                .is_none_or(|set| !set.contains(&key(name)))
        })
        .filter(|name| producers::generator(name).is_some())
        .collect();
    if generators.is_empty() {
        return Vec::new();
    }
    let same: Vec<&Diagnostic> = unit
        .diagnostics
        .iter()
        .filter(|d| d.code.lower_case_name() == code)
        .collect();
    if same.len() < 2 {
        return Vec::new();
    }
    let mut fixes = Vec::new();
    for name in generators {
        let generator = producers::generator(name).unwrap();
        let c = unit.context(diagnostic, false);
        if !generator(&c).can_be_applied_across_single_file() {
            continue;
        }
        let eol = unit.utils.end_of_line.clone();
        let mut builder = ChangeBuilder::new(workspace, Some(&eol));
        let mut fix_kind: Option<&'static FixKind> = None;
        let mut fix_count = 0;
        // First, try to fix the specific diagnostic.
        fix_diagnostic(
            unit,
            diagnostic,
            generator,
            &mut builder,
            &mut fix_kind,
            &mut fix_count,
        );
        if !builder.has_edits() {
            continue;
        }
        for d in &same {
            if std::ptr::eq(*d, diagnostic) || **d == *diagnostic {
                continue;
            }
            fix_diagnostic(
                unit,
                d,
                generator,
                &mut builder,
                &mut fix_kind,
                &mut fix_count,
            );
        }
        if let Some(kind) = fix_kind {
            let mut change = builder.source_change();
            if !change.edits.is_empty() && fix_count > 1 {
                change.id = Some(kind.id.to_string());
                change.message = kind.message.to_string();
                fixes.push(Fix { kind, change });
            }
        }
        if let Some(set) = already_calculated.as_deref_mut() {
            set.insert(key(name));
        }
    }
    fixes
}

/// Dart `FixInFileProcessor._fixDiagnostic`.
fn fix_diagnostic(
    unit: &FixUnit<'_>,
    diagnostic: &Diagnostic,
    generator: ProducerGenerator,
    builder: &mut ChangeBuilder<'_>,
    fix_kind: &mut Option<&'static FixKind>,
    fix_count: &mut usize,
) {
    let c = unit.context(diagnostic, true);
    let mut producer = generator(&c);
    producer.compute(&c, builder);
    if builder.conflict {
        builder.conflict = false;
        builder.revert();
        return;
    }
    let Some(multi) = producer.multi_fix_kind() else {
        builder.revert();
        return;
    };
    builder.commit();
    *fix_kind = Some(multi);
    *fix_count += 1;
}
