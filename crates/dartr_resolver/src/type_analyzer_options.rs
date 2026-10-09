// Dart source: pkg/analyzer/lib/src/dart/resolver/type_analyzer_options.dart

//! [`compute_type_analyzer_options`].

use dartr_flow::type_analyzer::TypeAnalyzerOptions;
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::experimental_flags::ExperimentalFlag;

/// Dart `computeTypeAnalyzerOptions(featureSet)`.
pub fn compute_type_analyzer_options(features: ExperimentalFeatures) -> TypeAnalyzerOptions {
    let on = |flag| features.is_experiment_enabled(flag);
    TypeAnalyzerOptions {
        patterns_enabled: on(ExperimentalFlag::Patterns),
        inference_update3_enabled: on(ExperimentalFlag::InferenceUpdate3),
        respect_implicitly_typed_var_initializers: on(ExperimentalFlag::ConstructorTearoffs),
        field_promotion_enabled: on(ExperimentalFlag::InferenceUpdate2),
        inference_update4_enabled: on(ExperimentalFlag::InferenceUpdate4),
        this_promotion_enabled: on(ExperimentalFlag::ThisPromotion),
        sound_flow_analysis_enabled: on(ExperimentalFlag::SoundFlowAnalysis),
    }
}
