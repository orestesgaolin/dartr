// Dart source: pkg/analysis_server_plugin/lib/src/correction/assist_processor.dart (computeAssists, AssistProcessor)
// Dart source: pkg/analysis_server_plugin/lib/src/correction/assist_generators.dart (registeredAssistGenerators.lintRuleMap)
// Dart source: pkg/analysis_server/lib/src/services/correction/assist_internal.dart (registerBuiltInAssistGenerators)

//! The assist processor: the assists at a selection, from the registered
//! assist producers.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::change::SourceChange;
use super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::fix_kind::{FixKind, format_list};
use super::fix_processor::FixUnit;
use super::generated::{assist_kinds, fix_registry};
use super::producer::*;
use super::producers;

/// Dart `Assist`.
#[derive(Clone, Debug)]
pub struct Assist {
    pub kind: &'static FixKind,
    pub change: SourceChange,
}

/// Dart `lintRuleMap`: the lint names for which an assist generator is
/// also a fix generator.
fn lint_rule_map() -> &'static HashMap<&'static str, Vec<&'static str>> {
    static MAP: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
        for generator in assist_kinds::ASSIST_GENERATORS {
            let lints = fix_registry::LINT_PRODUCERS
                .iter()
                .filter(|(_, producers)| producers.contains(generator))
                .map(|(lint, _)| *lint)
                .collect();
            map.insert(*generator, lints);
        }
        map
    })
}

/// Dart `AssistProcessor._generatorAppliesToAnyLintRule`.
fn generator_applies_to_any_lint_rule(c: &ProducerContext<'_>, generator: &str) -> bool {
    let Some(lints) = lint_rule_map().get(generator) else {
        return false;
    };
    if lints.is_empty() {
        return false;
    }
    let Some(node) = c
        .ast
        .node_covering(c.unit, c.selection_offset, c.selection_length)
    else {
        return false;
    };
    let file_offset = c.ast.offset(node) as usize;
    c.diagnostics.iter().any(|d| {
        file_offset >= d.offset
            && file_offset <= d.offset + d.length
            && d.code.diagnostic_type == dartr_diagnostics::DiagnosticType::Lint
            && lints.contains(&d.code.camel_case_name)
    })
}

/// Dart `AssistProcessor._addFromProducer`.
fn add_from_producer(
    c: &ProducerContext<'_>,
    mut producer: Box<dyn CorrectionProducer>,
    workspace: &mut dyn ChangeWorkspace,
    assists: &mut Vec<Assist>,
) {
    let Some(kind) = producer.assist_kind() else {
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
    change.message = format_list(kind.message, &producer.assist_arguments());
    assists.push(Assist { kind, change });
}

/// Dart `computeAssists`: the assists of the selection in [unit].
pub fn compute_assists(
    unit: &FixUnit<'_>,
    selection_offset: u32,
    selection_length: u32,
    workspace: &mut dyn ChangeWorkspace,
) -> Vec<Assist> {
    let c = ProducerContext::new(
        unit.resolved,
        unit.ctx,
        unit.utils,
        unit.path,
        unit.options,
        None,
        unit.diagnostics,
        selection_offset,
        selection_length,
        false,
    );
    let mut assists = Vec::new();
    for name in assist_kinds::ASSIST_GENERATORS {
        let Some(generator) = producers::generator(name) else {
            continue;
        };
        if generator_applies_to_any_lint_rule(&c, name) {
            continue;
        }
        add_from_producer(&c, generator(&c), workspace, &mut assists);
    }
    for name in assist_kinds::ASSIST_MULTI_GENERATORS {
        let Some(generator) = producers::multi_generator(name) else {
            continue;
        };
        for producer in generator(&c, workspace) {
            add_from_producer(&c, producer, workspace, &mut assists);
        }
    }
    assists
}
