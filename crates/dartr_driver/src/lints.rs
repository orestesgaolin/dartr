// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
use dartr_element::{Ctx, NoopSink};
use dartr_lints::{
    AnnotationRef, ElementMetadata, ResolvedLintContext, ResolvedRuleContextUnit,
    lint_resolved_library,
};
use dartr_resolver::constant::evaluation::{ConstantEvaluationEngine, NodeRef};
use dartr_resolver::library_analyzer::{LibraryAnalysisInput, ResolvedLibrary};

/// [`ElementMetadata`] with the constant evaluation engine of the library.
struct EngineMetadata<'a>(ConstantEvaluationEngine<'a>);

impl ElementMetadata for EngineMetadata<'_> {
    fn annotations(&self, element: dartr_element::ElementId) -> Vec<AnnotationRef> {
        self.0
            .metadata_annotations(element)
            .into_iter()
            .map(|a| AnnotationRef {
                unit: a.unit,
                node: a.node,
            })
            .collect()
    }

    fn annotation_element(&self, annotation: AnnotationRef) -> Option<dartr_element::ElementId> {
        let a = NodeRef::new(annotation.unit, annotation.node);
        let element = self.0.annotation_element_of(a)?;
        let ctx = self.0.unit_ctx(annotation.unit);
        Some(dartr_typesystem::member::base_element(&ctx, element))
    }

    fn annotation_value(
        &self,
        annotation: AnnotationRef,
    ) -> Option<dartr_constant::DartObjectImpl> {
        let a = NodeRef::new(annotation.unit, annotation.node);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.0.compute_annotation_constant_value(a)
        }))
        .ok()
        .flatten()
    }
}

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
    static DECLARED_VARIABLES: std::sync::LazyLock<dartr_constant::DeclaredVariables> =
        std::sync::LazyLock::new(dartr_constant::DeclaredVariables::new);
    let engine = ConstantEvaluationEngine::new(
        input.world,
        input.type_provider,
        library.library,
        &DECLARED_VARIABLES,
        &library.units,
        input.external,
    );
    *engine.values.borrow_mut() = library.constants.clone();
    let metadata = EngineMetadata(engine);
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
                potentially_mutated_in_scope: &resolved.rt.potentially_mutated_in_scope,
                library: library.library,
                metadata: Some(&metadata),
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
