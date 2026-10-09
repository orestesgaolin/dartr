// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
use dartr_element::{Ctx, NoopSink};
use dartr_lints::{ResolvedLintContext, ResolvedRuleContextUnit, lint_resolved_library};
use dartr_resolver::library_analyzer::{LibraryAnalysisInput, ResolvedLibrary};

/// Defining unit first, followed by parts. Each unit reads its own local arena.
pub fn compute_lints(
    input: &LibraryAnalysisInput<'_>,
    library: &ResolvedLibrary,
    enabled: &[&str],
) -> Vec<Vec<dartr_lints::LintDiagnostic>> {
    if enabled.is_empty() {
        return vec![vec![]; library.units.len()];
    }
    let sink = NoopSink;
    let global = Ctx {
        world: input.world,
        current: None,
        local: None,
        tp: input.type_provider,
        features: &dartr_element::FeatureSet::default(),
        req: &sink,
    };
    let features = &global.get(library.library).feature_set;
    let units: Vec<_> = input
        .units
        .iter()
        .zip(&library.units)
        .map(|(original, resolved)| ResolvedRuleContextUnit {
            parsed: &original.parsed,
            ast: &resolved.ast,
            unit: resolved.unit.raw(),
            source: &original.parsed.ast.tokens.source,
            path: &resolved.path,
            resolved: resolved.panic.is_none().then_some(ResolvedLintContext {
                ctx: Ctx {
                    local: Some(&resolved.local),
                    features,
                    ..global
                },
                tables: &resolved.tables,
                library: library.library,
            }),
        })
        .collect();
    let mut diagnostics = lint_resolved_library(&units, enabled);
    for (unit, output) in library.units.iter().zip(&mut diagnostics) {
        if unit.panic.is_some() {
            output.clear();
        }
    }
    diagnostics
}
