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

    fn element_constant_value(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<dartr_constant::DartObjectImpl> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.0.compute_constant_value_of(element)
        }))
        .ok()
        .flatten()
    }

    fn default_value_code(&self, element: dartr_element::ElementId) -> Option<String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let node = self.0.constant_initializer(element)?;
            let unit = self.0.unit(node.unit);
            Some(dartr_ast::to_source::to_source(&unit.ast, node.node))
        }))
        .ok()
        .flatten()
    }

    fn expression_constant_value(
        &self,
        unit: u32,
        node: dartr_ast::NodeId,
    ) -> Option<dartr_constant::DartObjectImpl> {
        let node = NodeRef::new(unit, node);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.0.compute_expression_constant_value(node)
        }))
        .ok()
        .flatten()
    }

    fn has_constant_verifier_error(&self, unit: u32, node: dartr_ast::NodeId) -> bool {
        let node = NodeRef::new(unit, node);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dartr_resolver::constant::constant_verifier::has_constant_verifier_error(&self.0, node)
        }))
        .unwrap_or(false)
    }

    fn expression_has_constant_error(&self, unit: u32, node: dartr_ast::NodeId) -> bool {
        let node = NodeRef::new(unit, node);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.0.expression_has_constant_error(node)
        }))
        .unwrap_or(true)
    }

    fn can_be_const(&self, unit: u32, node: dartr_ast::NodeId) -> bool {
        let node = NodeRef::new(unit, node);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dartr_resolver::constant::constant_verifier::can_be_const(&self.0, node)
        }))
        .unwrap_or(false)
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
    let body_contexts: Vec<dartr_ast::NodeMap<dartr_lints::BodyContext>> = library
        .units
        .iter()
        .map(|unit| {
            let mut map = dartr_ast::NodeMap::new();
            for (node, context) in unit.rt.body_context.iter() {
                map.insert(
                    node,
                    dartr_lints::BodyContext {
                        imposed_type: context.imposed_type,
                        may_complete_normally: context.may_complete_normally,
                    },
                );
            }
            map
        })
        .collect();
    let exit_detectors: Vec<Box<dyn Fn(dartr_ast::NodeId) -> bool + '_>> = library
        .units
        .iter()
        .map(|unit| {
            let ctx = Ctx {
                local: Some(&unit.local),
                features,
                ..global
            };
            Box::new(move |node: dartr_ast::NodeId| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    dartr_resolver::exit_detector::exits_resolved(
                        &unit.ast,
                        &unit.tables,
                        &unit.rt,
                        ctx,
                        node,
                    )
                }))
                .unwrap_or(false)
            }) as Box<dyn Fn(dartr_ast::NodeId) -> bool + '_>
        })
        .collect();
    let units: Vec<_> = input
        .units
        .iter()
        .zip(&library.units)
        .zip(&body_contexts)
        .zip(&exit_detectors)
        .map(|(((original, resolved), body_context), exits)| ResolvedRuleContextUnit {
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
                corresponding_parameter_type: &resolved.rt.corresponding_parameter_type,
                body_context,
                this_scope_lookup: &resolved.rt.this_scope_lookup,
                exits: Some(exits.as_ref()),
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
