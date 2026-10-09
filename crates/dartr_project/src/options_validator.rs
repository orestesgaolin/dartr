//! Diagnostics for `analysis_options.yaml`.
//!
//! Ports `pkg/analyzer/lib/src/analysis_options/analysis_options_parser.dart`,
//! `analysis_options_parse_model.dart`, `analysis_options_include_walker.dart`,
//! and `linter_rule_options_validator.dart` from Dart SDK 3.13.3.

#[path = "lint_rule_metadata.rs"]
mod lint_rule_metadata;

use crate::analysis_options::{
    FileContent, IncludeResolution, OptionsParseResult, OptionsParseSession,
};
use crate::lint_rules::LINT_RULE_NAMES;
use crate::workspace::WorkspacePackage;
use crate::yaml::{NodeKind, Scalar, Span, YamlNode};
use crate::{AnalysisContext, experiments, paths};
use dartr_diagnostics::{Diagnostic, DiagnosticMessage, LocatableDiagnostic, all_codes, diag};
use lint_rule_metadata::{RuleStateKind, incompatible_rules, rule_state};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const ANALYZER_OPTIONS: &[&str] = &[
    "cannot-ignore",
    "enable-experiment",
    "errors",
    "exclude",
    "language",
    "optional-checks",
    "plugins",
    "strong-mode",
];
const LANGUAGE_OPTIONS: &[&str] = &["strict-casts", "strict-inference", "strict-raw-types"];
const OPTIONAL_CHECKS_OPTIONS: &[&str] =
    &["chrome-os-manifest-checks", "propagate-linter-exceptions"];
const GIT_OPTIONS: &[&str] = &["url", "ref", "path", "tag_pattern"];
const PLUGIN_OPTIONS: &[&str] = &["diagnostics", "git", "path", "version", "hosted"];
// Upstream concatenates these lists without deduplicating `false`.
const ERROR_PROCESSOR_VALUES: &[&str] = &[
    "ignore", "false", "true", "false", "error", "info", "warning",
];
const LINT_VALUES: &[&str] = &["true", "false", "ignore", "info", "warning", "error"];

#[derive(Clone)]
struct RuleOccurrence {
    /// Spelling in the YAML file, used in diagnostic messages.
    name: String,
    /// Registry key; lint names are matched without case sensitivity.
    canonical: &'static str,
    node: YamlNode,
    file: String,
    enabled: bool,
}

#[derive(Default)]
struct LocalSemantics {
    diagnostics: Vec<Diagnostic>,
    rules: Vec<RuleOccurrence>,
    legacy_plugins: Vec<(String, YamlNode)>,
}

struct Validator<'a> {
    context: &'a AnalysisContext,
    session: OptionsParseSession,
    result: OptionsParseResult,
    graph: HashMap<String, Vec<IncludeResolution>>,
    known_codes: HashSet<String>,
    sdk_min: Option<MinimumVersion>,
}

/// Validate one analysis options file and return raw analyzer diagnostics.
///
/// The caller applies effective `analyzer/errors` processors after this
/// function returns, matching the analysis server converter.
pub fn validate_analysis_options(context: &AnalysisContext, path: &str) -> Vec<Diagnostic> {
    let session = OptionsParseSession::new();
    let result = session.parse(&context.root.workspace, path);
    match result.content.as_ref() {
        FileContent::Unreadable => Vec::new(),
        FileContent::Malformed { text, error } => {
            error
                .utf16_range(text)
                .map_or_else(Vec::new, |(offset, length)| {
                    vec![diag::parse_error(&error.message).to_diagnostic(offset, length)]
                })
        }
        FileContent::Parsed { .. } => {
            let graph = result.includes.iter().cloned().collect();
            // The analysis server codes (`TransformSetErrorCode`) are not in
            // the analyzer's `errorCodeByUniqueName`: the analyzer reports
            // them as unrecognized in `errors:` and `ignore:` lists.
            let known_codes = all_codes()
                .iter()
                .filter(|code| code.origin != dartr_diagnostics::Origin::AnalysisServer)
                .map(|code| code.name.to_ascii_uppercase())
                .chain(LINT_RULE_NAMES.iter().map(|name| name.to_ascii_uppercase()))
                .chain(std::iter::once("MISSING_RETURN".to_string()))
                .collect();
            let sdk_min = sdk_minimum(context, path);
            let validator = Validator {
                context,
                session,
                result,
                graph,
                known_codes,
                sdk_min,
            };
            validator.validate()
        }
    }
}

impl Validator<'_> {
    fn validate(&self) -> Vec<Diagnostic> {
        let initial = self.result.file.as_str();
        let mut diagnostics = Vec::new();
        let mut chain = Vec::new();
        self.walk(initial, None, &mut chain, &mut diagnostics);
        diagnostics
    }

    fn walk(
        &self,
        file: &str,
        first_include: Option<Span>,
        chain: &mut Vec<(String, Span)>,
        output: &mut Vec<Diagnostic>,
    ) -> EffectiveSemantics {
        let content = self.session.content(file);
        let FileContent::Parsed { text, yaml } = content.as_ref() else {
            return EffectiveSemantics::default();
        };
        let is_initial = file == self.result.file;
        let local = self.validate_local(file, text, yaml, is_initial);
        for diagnostic in &local.diagnostics {
            self.emit(file, text, first_include, diagnostic.clone(), output);
        }

        let mut included = Vec::new();
        let mut included_plugin: Option<String> = None;
        for resolution in self.graph.get(file).into_iter().flatten() {
            let (uri, node) = match resolution {
                IncludeResolution::Missing { uri, node }
                | IncludeResolution::Malformed { uri, node, .. }
                | IncludeResolution::Parsed { uri, node, .. } => (uri, node),
            };
            let first = first_include.unwrap_or(node.span);
            match resolution {
                IncludeResolution::Missing { .. } => output.push(at(
                    self.result.content.text().unwrap_or_default(),
                    first,
                    diag::include_file_not_found(uri, file, &self.context.root.root),
                )),
                IncludeResolution::Malformed {
                    file: malformed, ..
                } => {
                    if let FileContent::Malformed {
                        text: malformed_text,
                        error,
                    } = self.session.content(malformed).as_ref()
                        && let Some((start, length)) = error.utf16_range(malformed_text)
                    {
                        output.push(at(
                            self.result.content.text().unwrap_or_default(),
                            first,
                            diag::included_file_parse_error(
                                malformed,
                                start as i64,
                                (start + length) as i64,
                                &error.message,
                            ),
                        ));
                    }
                }
                IncludeResolution::Parsed {
                    file: included_file,
                    ..
                } => {
                    if included_file == &self.result.file {
                        output.push(at(
                            self.result.content.text().unwrap_or_default(),
                            first,
                            diag::recursive_include_file(uri, file),
                        ));
                        continue;
                    }
                    if let Some((_, cycle_span)) =
                        chain.iter().find(|(path, _)| path == included_file)
                    {
                        let included_content = self.session.content(included_file);
                        let included_text = included_content.text().unwrap_or_default();
                        let (start, length) = utf16_span(included_text, *cycle_span);
                        output.push(at(
                            self.result.content.text().unwrap_or_default(),
                            first,
                            diag::included_file_warning(
                                included_file,
                                start as i64,
                                (start + length).saturating_sub(1) as i64,
                                "The file includes itself recursively.",
                            ),
                        ));
                        continue;
                    }
                    chain.push((file.to_string(), node.span));
                    let semantics = self.walk(included_file, Some(first), chain, output);
                    chain.pop();
                    if included_plugin.is_none() {
                        included_plugin = semantics.first_legacy_plugin.clone();
                    }
                    included.push((node.clone(), semantics));
                }
            }
        }

        self.report_plugin_conflicts(
            file,
            text,
            first_include,
            &local.legacy_plugins,
            included_plugin.as_deref(),
            output,
        );
        self.report_included_lint_conflicts(
            file,
            text,
            first_include,
            &local.rules,
            &included,
            output,
        );

        let mut rules = Vec::<RuleOccurrence>::new();
        for (_, semantics) in &included {
            apply_rules(&mut rules, &semantics.rules);
        }
        apply_local_rules(&mut rules, &local.rules);
        EffectiveSemantics {
            rules,
            first_legacy_plugin: included_plugin
                .or_else(|| local.legacy_plugins.first().map(|p| p.0.clone())),
        }
    }

    fn emit(
        &self,
        file: &str,
        file_text: &str,
        first_include: Option<Span>,
        diagnostic: Diagnostic,
        output: &mut Vec<Diagnostic>,
    ) {
        if let Some(include_span) = first_include {
            output.push(at(
                self.result.content.text().unwrap_or_default(),
                include_span,
                diag::included_file_warning(
                    file,
                    diagnostic.offset as i64,
                    diagnostic.end().saturating_sub(1) as i64,
                    &diagnostic.message,
                ),
            ));
        } else {
            let _ = file_text;
            output.push(diagnostic);
        }
    }

    fn validate_local(
        &self,
        file: &str,
        text: &str,
        yaml: &YamlNode,
        is_initial: bool,
    ) -> LocalSemantics {
        let mut semantics = LocalSemantics::default();
        let Some(entries) = yaml.as_map() else {
            return semantics;
        };

        if let Some(analyzer) = value_at(entries, "analyzer") {
            self.validate_analyzer(text, analyzer, &mut semantics);
        }
        if let Some(code_style) = value_at(entries, "code-style") {
            validate_code_style(text, code_style, &mut semantics.diagnostics);
        }
        if let Some(formatter) = value_at(entries, "formatter") {
            validate_formatter(text, formatter, &mut semantics.diagnostics);
        }
        if let Some(linter) = value_at(entries, "linter") {
            self.validate_linter(file, text, linter, is_initial, &mut semantics);
        }
        if let Some(plugins) = value_at(entries, "plugins") {
            self.validate_plugins(file, text, plugins, is_initial, &mut semantics.diagnostics);
        }
        semantics
    }

    fn validate_analyzer(&self, text: &str, analyzer: &YamlNode, semantics: &mut LocalSemantics) {
        if analyzer.is_null_scalar() {
            return;
        }
        let Some(entries) = analyzer.as_map() else {
            semantics.diagnostics.push(at(
                text,
                analyzer.span,
                diag::invalid_section_format("analyzer"),
            ));
            return;
        };
        unsupported_keys(
            text,
            entries,
            "analyzer",
            ANALYZER_OPTIONS,
            &mut semantics.diagnostics,
        );
        if let Some((key, strong_mode)) = entry_at(entries, "strong-mode") {
            let _ = key;
            validate_strong_mode(text, strong_mode, &mut semantics.diagnostics);
        }
        if let Some((key, plugins)) = entry_at(entries, "plugins") {
            semantics.diagnostics.push(at(
                text,
                key.span,
                diag::analysis_options_deprecated_plugins(),
            ));
            semantics.legacy_plugins = legacy_plugins(plugins);
        }
        if let Some(value) = value_at(entries, "cannot-ignore") {
            self.validate_code_list(
                text,
                "cannot-ignore",
                value,
                true,
                &mut semantics.diagnostics,
            );
        }
        if let Some(value) = value_at(entries, "enable-experiment") {
            validate_experiments(text, value, &mut semantics.diagnostics);
        }
        if let Some(value) = value_at(entries, "errors") {
            self.validate_errors(text, value, &mut semantics.diagnostics);
        }
        if let Some(value) = value_at(entries, "exclude") {
            validate_string_list(text, "exclude", value, &mut semantics.diagnostics);
        }
        if let Some(value) = value_at(entries, "language") {
            validate_bool_section(
                text,
                "language",
                value,
                LANGUAGE_OPTIONS,
                &mut semantics.diagnostics,
            );
        }
        if let Some(value) = value_at(entries, "optional-checks") {
            validate_optional_checks(text, value, &mut semantics.diagnostics);
        }
    }

    fn validate_code_list(
        &self,
        text: &str,
        section: &str,
        node: &YamlNode,
        allow_severity: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(nodes) = node.as_list() else {
            out.push(at(text, node.span, diag::invalid_section_format(section)));
            return;
        };
        for value in nodes {
            match value.string_value() {
                Some(name) if allow_severity && matches!(name, "error" | "info" | "warning") => {}
                Some(name) if !self.known_codes.contains(&name.to_ascii_uppercase()) => {
                    out.push(at(text, value.span, diag::unrecognized_error_code(name)));
                }
                Some(_) => {}
                None => out.push(at(text, value.span, diag::invalid_section_format(section))),
            }
        }
    }

    fn validate_errors(&self, text: &str, errors: &YamlNode, out: &mut Vec<Diagnostic>) {
        let Some(entries) = errors.as_map() else {
            out.push(at(
                text,
                errors.span,
                diag::invalid_section_format("errors"),
            ));
            return;
        };
        for (key, value) in entries {
            match key.scalar() {
                Some(Scalar::String(code)) => {
                    if !self.known_codes.contains(&code.to_ascii_uppercase()) {
                        out.push(at(text, key.span, diag::unrecognized_error_code(code)));
                    }
                }
                Some(scalar) => out.push(at(
                    text,
                    key.span,
                    diag::unrecognized_error_code(&scalar.to_dart_string()),
                )),
                None => {
                    out.push(at(text, key.span, diag::invalid_section_format("errors")));
                    continue;
                }
            }
            let Some(scalar) = value.scalar() else {
                out.push(at(text, value.span, diag::invalid_section_format("errors")));
                continue;
            };
            let action = scalar.to_dart_string().to_ascii_lowercase();
            if !ERROR_PROCESSOR_VALUES.contains(&action.as_str()) {
                out.push(at(
                    text,
                    value.span,
                    diag::unsupported_option_with_legal_values(
                        "errors",
                        &scalar.to_dart_string(),
                        &quoted(ERROR_PROCESSOR_VALUES, "and"),
                    ),
                ));
            }
        }
    }

    fn validate_linter(
        &self,
        file: &str,
        text: &str,
        linter: &YamlNode,
        is_initial: bool,
        semantics: &mut LocalSemantics,
    ) {
        if linter.is_null_scalar() {
            return;
        }
        let Some(entries) = linter.as_map() else {
            semantics.diagnostics.push(at(
                text,
                linter.span,
                diag::invalid_section_format("linter"),
            ));
            return;
        };
        unsupported_keys(
            text,
            entries,
            "linter",
            &["rules"],
            &mut semantics.diagnostics,
        );
        let Some(rules) = value_at(entries, "rules") else {
            return;
        };
        let mut active: Vec<RuleOccurrence> = Vec::new();
        let mut parse = |name_node: &YamlNode, value: Option<&YamlNode>, default_enabled: bool| {
            let Some(name) = name_node.string_value() else {
                return;
            };
            let Some(&canonical) = LINT_RULE_NAMES
                .iter()
                .find(|candidate| candidate.eq_ignore_ascii_case(name))
            else {
                semantics
                    .diagnostics
                    .push(at(text, name_node.span, diag::undefined_lint(name)));
                return;
            };
            let enabled = match value {
                None => default_enabled,
                Some(value) if value.is_null_scalar() => return,
                Some(value) => match value.scalar() {
                    Some(Scalar::Bool(enabled)) => *enabled,
                    Some(Scalar::String(action))
                        if matches!(action.as_str(), "ignore" | "info" | "warning" | "error") =>
                    {
                        action != "ignore"
                    }
                    Some(scalar) => {
                        semantics.diagnostics.push(at(
                            text,
                            value.span,
                            diag::unsupported_value(
                                name,
                                &scalar.to_dart_string(),
                                &quoted(LINT_VALUES, "or"),
                            ),
                        ));
                        false
                    }
                    None => {
                        semantics.diagnostics.push(at(
                            text,
                            value.span,
                            diag::unsupported_value(
                                name,
                                &node_value(value),
                                &quoted(LINT_VALUES, "or"),
                            ),
                        ));
                        false
                    }
                },
            };
            if is_initial {
                self.report_rule_state(
                    text,
                    name_node,
                    name,
                    canonical,
                    &mut semantics.diagnostics,
                );
            }
            let occurrence = RuleOccurrence {
                name: name.to_string(),
                canonical,
                node: name_node.clone(),
                file: file.to_string(),
                enabled,
            };
            if enabled {
                let incompatible: Vec<_> = incompatible_rules(canonical)
                    .iter()
                    .flat_map(|incompatible| {
                        active
                            .iter()
                            .filter(move |other| other.canonical == *incompatible)
                            .cloned()
                    })
                    .collect();
                if !incompatible.is_empty() {
                    semantics.diagnostics.push(incompatible_diagnostic(
                        text,
                        &occurrence,
                        &incompatible,
                        false,
                    ));
                }
                if active.iter().any(|other| other.canonical == canonical) {
                    semantics.diagnostics.push(at(
                        text,
                        name_node.span,
                        diag::duplicate_rule(name),
                    ));
                }
                active.retain(|other| other.canonical != canonical);
                active.push(occurrence.clone());
            }
            semantics.rules.push(occurrence);
        };
        match &rules.kind {
            NodeKind::List(nodes) => {
                for node in nodes {
                    parse(node, None, true);
                }
            }
            NodeKind::Map(entries) => {
                for (key, value) in entries {
                    parse(key, Some(value), false);
                }
            }
            _ => {}
        }
    }

    fn report_rule_state(
        &self,
        text: &str,
        node: &YamlNode,
        name: &str,
        canonical: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(state) = rule_state(canonical) else {
            return;
        };
        if !state.since.is_none_or(|since| {
            self.sdk_min
                .as_ref()
                .is_some_and(|minimum| minimum.includes(since))
        }) {
            return;
        }
        let located = match (state.kind, state.replacement) {
            (RuleStateKind::Deprecated, Some(replacement)) => {
                diag::deprecated_lint_with_replacement(name, replacement)
            }
            (RuleStateKind::Deprecated, None) => diag::deprecated_lint(name),
            (RuleStateKind::Removed, Some(replacement)) => {
                diag::replaced_lint(name, &version_text(state.since), replacement)
            }
            (RuleStateKind::Removed, None) => diag::removed_lint(name, &version_text(state.since)),
        };
        out.push(at(text, node.span, located));
    }

    fn validate_plugins(
        &self,
        file: &str,
        text: &str,
        plugins: &YamlNode,
        is_initial: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        if plugins.is_null_scalar() {
            return;
        }
        let Some(entries) = plugins.as_map() else {
            out.push(at(
                text,
                plugins.span,
                diag::invalid_section_format("plugins"),
            ));
            return;
        };
        if is_initial && paths::dirname(file) != self.context.root.root {
            out.push(at(
                text,
                plugins.span,
                diag::plugins_in_inner_options(&self.context.root.root),
            ));
        }
        for (name_node, plugin) in entries {
            let Some(name) = name_node.string_value() else {
                continue;
            };
            if name == "dependency_overrides" {
                let Some(overrides) = plugin.as_map() else {
                    out.push(at(
                        text,
                        plugin.span,
                        diag::invalid_section_format("plugins/dependency_overrides"),
                    ));
                    continue;
                };
                for (override_name, value) in overrides {
                    if let Some(name) = override_name.string_value() {
                        validate_plugin(
                            text,
                            &format!("plugins/dependency_overrides/{name}"),
                            value,
                            out,
                        );
                    }
                }
            } else {
                validate_plugin(text, &format!("plugins/{name}"), plugin, out);
            }
        }
    }

    fn report_plugin_conflicts(
        &self,
        file: &str,
        text: &str,
        first_include: Option<Span>,
        local: &[(String, YamlNode)],
        included: Option<&str>,
        out: &mut Vec<Diagnostic>,
    ) {
        let first = included.or_else(|| local.first().map(|entry| entry.0.as_str()));
        let Some(first) = first else { return };
        for (index, (name, node)) in local.iter().enumerate() {
            if name != first && (included.is_some() || index > 0) {
                let diagnostic = at(text, node.span, diag::multiple_plugins(first));
                self.emit(file, text, first_include, diagnostic, out);
            }
        }
    }

    fn report_included_lint_conflicts(
        &self,
        file: &str,
        text: &str,
        first_include: Option<Span>,
        local: &[RuleOccurrence],
        included: &[(YamlNode, EffectiveSemantics)],
        out: &mut Vec<Diagnostic>,
    ) {
        let disabled: HashSet<_> = local
            .iter()
            .filter(|r| !r.enabled)
            .map(|r| r.canonical)
            .collect();
        let mut seen: Vec<(YamlNode, RuleOccurrence)> = Vec::new();
        for (include_node, semantics) in included {
            for rule in &semantics.rules {
                seen.retain(|(_, old)| old.canonical != rule.canonical);
            }
            let mut conflicts = Vec::new();
            let mut conflicting_include_spans = HashSet::new();
            for rule in semantics
                .rules
                .iter()
                .filter(|r| r.enabled && !disabled.contains(r.canonical))
            {
                for incompatible in incompatible_rules(rule.canonical) {
                    for (previous_include_node, old) in seen
                        .iter()
                        .filter(|(_, old)| old.canonical == *incompatible)
                    {
                        conflicts.push(rule.clone());
                        conflicts.push(old.clone());
                        conflicting_include_spans
                            .insert((include_node.span.start, include_node.span.end));
                        conflicting_include_spans.insert((
                            previous_include_node.span.start,
                            previous_include_node.span.end,
                        ));
                    }
                }
            }
            collapse_rules_by_file(&mut conflicts);
            if !conflicts.is_empty() {
                let names = quoted(
                    &conflicts
                        .iter()
                        .map(|r| r.name.as_str())
                        .collect::<Vec<_>>(),
                    "and",
                );
                let file_count = conflicting_include_spans.len() as i64;
                let located = diag::incompatible_lint_included(
                    &node_value(include_node),
                    &names,
                    file_count,
                    if file_count == 1 { "" } else { "s" },
                )
                .with_context_messages(context_messages(&conflicts, false));
                let diagnostic = at(text, include_node.span, located);
                self.emit(file, text, first_include, diagnostic, out);
            }
            for rule in semantics
                .rules
                .iter()
                .filter(|r| r.enabled && !disabled.contains(r.canonical))
            {
                seen.retain(|(_, old)| old.canonical != rule.canonical);
                seen.push((include_node.clone(), rule.clone()));
            }
        }
        let mut effective = Vec::new();
        for (_, semantics) in included {
            apply_rules(&mut effective, &semantics.rules);
        }
        for rule in local.iter().filter(|r| r.enabled) {
            let conflicts: Vec<_> = incompatible_rules(rule.canonical)
                .iter()
                .flat_map(|incompatible| {
                    effective
                        .iter()
                        .filter(move |old| old.enabled && old.canonical == *incompatible)
                        .cloned()
                })
                .collect();
            if !conflicts.is_empty() {
                let diagnostic = incompatible_diagnostic(text, rule, &conflicts, true);
                self.emit(file, text, first_include, diagnostic, out);
            }
        }
    }
}

#[derive(Default)]
struct EffectiveSemantics {
    rules: Vec<RuleOccurrence>,
    first_legacy_plugin: Option<String>,
}

fn apply_rules(target: &mut Vec<RuleOccurrence>, additions: &[RuleOccurrence]) {
    for rule in additions {
        target.retain(|old| old.canonical != rule.canonical);
        target.push(rule.clone());
    }
}

fn apply_local_rules(target: &mut Vec<RuleOccurrence>, additions: &[RuleOccurrence]) {
    // Upstream stores local disabled and enabled declarations separately, then
    // applies all disabled declarations before all enabled declarations.
    for enabled in [false, true] {
        apply_rules(
            target,
            &additions
                .iter()
                .filter(|rule| rule.enabled == enabled)
                .cloned()
                .collect::<Vec<_>>(),
        );
    }
}

fn collapse_rules_by_file(rules: &mut Vec<RuleOccurrence>) {
    let mut collapsed: Vec<RuleOccurrence> = Vec::new();
    for rule in rules.drain(..) {
        if let Some(existing) = collapsed.iter_mut().find(|old| old.file == rule.file) {
            *existing = rule;
        } else {
            collapsed.push(rule);
        }
    }
    *rules = collapsed;
}

fn incompatible_diagnostic(
    text: &str,
    rule: &RuleOccurrence,
    conflicts: &[RuleOccurrence],
    across_files: bool,
) -> Diagnostic {
    // Upstream constructs a map keyed by source path. Later rules from the
    // same file replace earlier ones while the path's insertion order stays.
    let mut conflicts = conflicts.to_vec();
    collapse_rules_by_file(&mut conflicts);
    let names = quoted(
        &conflicts
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        "and",
    );
    let located = if across_files {
        diag::incompatible_lint_files(&rule.name, &names)
            .with_context_messages(context_messages(&conflicts, true))
    } else {
        diag::incompatible_lint(&rule.name, &names)
            .with_context_messages(context_messages(&conflicts, false))
    };
    at(text, rule.node.span, located)
}

fn context_messages(
    rules: &[RuleOccurrence],
    include_file_in_message: bool,
) -> Vec<DiagnosticMessage> {
    rules
        .iter()
        .map(|rule| {
            let (offset, length) = crate::fs::read_string(&rule.file)
                .map(|text| utf16_span(&text, rule.node.span))
                .unwrap_or((rule.node.span.start, rule.node.span.length()));
            DiagnosticMessage {
                file_path: rule.file.clone(),
                offset: offset as i64,
                length: length as i64,
                message: if include_file_in_message {
                    format!(
                        "The rule '{}' is enabled here in the file '{}'.",
                        rule.name, rule.file
                    )
                } else {
                    format!("The rule '{}' is enabled here.", rule.name)
                },
                url: Some(rule.file.clone()),
            }
        })
        .collect()
}

fn validate_code_style(text: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    if node.is_null_scalar() {
        return;
    }
    let Some(entries) = node.as_map() else {
        out.push(at(
            text,
            node.span,
            diag::invalid_section_format("code-style"),
        ));
        return;
    };
    for (key, value) in entries {
        if key.string_value() == Some("format") {
            if value.scalar().is_none() {
                out.push(at(text, value.span, diag::invalid_section_format("format")));
            } else if value.to_bool().is_none() {
                out.push(at(
                    text,
                    value.span,
                    diag::unsupported_value("format", &node_value(value), "'true' and 'false'"),
                ));
            }
        } else {
            out.push(at(
                text,
                key.span,
                diag::unsupported_option_without_values("code-style", &node_value(key)),
            ));
        }
    }
}

fn validate_formatter(text: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    if node.is_null_scalar() {
        return;
    }
    let Some(entries) = node.as_map() else {
        out.push(at(
            text,
            node.span,
            diag::invalid_section_format("formatter"),
        ));
        return;
    };
    for (key, value) in entries {
        match key.string_value() {
            Some("page_width") => {
                if !matches!(value.scalar(), Some(Scalar::Int(width)) if *width > 0) {
                    out.push(at(
                        text,
                        value.span,
                        diag::invalid_option(
                            "page_width",
                            "\"page_width\" must be a positive integer.",
                        ),
                    ));
                }
            }
            Some("trailing_commas") => {
                if !matches!(value.string_value(), Some("automate" | "preserve")) {
                    out.push(at(
                        text,
                        value.span,
                        diag::invalid_option(
                            "trailing_commas",
                            "\"trailing_commas\" must be \"automate\" or \"preserve\".",
                        ),
                    ));
                }
            }
            _ => out.push(at(
                text,
                key.span,
                diag::unsupported_option_without_values("formatter", &node_value(key)),
            )),
        }
    }
}

fn validate_strong_mode(text: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    let Some(entries) = node.as_map() else {
        out.push(at(
            text,
            node.span,
            diag::invalid_section_format("strong-mode"),
        ));
        return;
    };
    for (key, value) in entries {
        match key.string_value() {
            Some(name @ ("implicit-casts" | "implicit-dynamic")) => {
                validate_bool(text, name, value, out)
            }
            _ => out.push(at(
                text,
                key.span,
                diag::unsupported_option_with_legal_values(
                    "strong-mode",
                    &node_value(key),
                    "'implicit-casts' and 'implicit-dynamic'",
                ),
            )),
        }
    }
}

fn validate_bool_section(
    text: &str,
    section: &str,
    node: &YamlNode,
    legal: &[&str],
    out: &mut Vec<Diagnostic>,
) {
    if node.is_null_scalar() {
        return;
    }
    let Some(entries) = node.as_map() else {
        out.push(at(text, node.span, diag::invalid_section_format(section)));
        return;
    };
    for (key, value) in entries {
        let Some(name) = key.string_value() else {
            continue;
        };
        if legal.contains(&name) {
            validate_bool(text, name, value, out);
        } else {
            unsupported_key(text, key, section, legal, out);
        }
    }
}

fn validate_bool(text: &str, name: &str, value: &YamlNode, out: &mut Vec<Diagnostic>) {
    if value.scalar().is_none() {
        out.push(at(text, value.span, diag::invalid_section_format(name)));
    } else if value.to_bool().is_none() {
        out.push(at(
            text,
            value.span,
            diag::unsupported_value(name, &node_value(value), "'true' and 'false'"),
        ));
    }
}

fn validate_optional_checks(text: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    match &node.kind {
        NodeKind::Scalar(Scalar::String(name))
            if OPTIONAL_CHECKS_OPTIONS.contains(&name.as_str()) => {}
        NodeKind::Scalar(_) => unsupported_key(
            text,
            node,
            "'chrome-os-manifest-checks' or 'propagate-linter-exceptions'",
            OPTIONAL_CHECKS_OPTIONS,
            out,
        ),
        NodeKind::Map(entries) => {
            for (key, value) in entries {
                let Some(name) = key.string_value() else {
                    continue;
                };
                if OPTIONAL_CHECKS_OPTIONS.contains(&name) {
                    validate_bool(text, name, value, out);
                } else {
                    unsupported_key(
                        text,
                        key,
                        "'chrome-os-manifest-checks' or 'propagate-linter-exceptions'",
                        OPTIONAL_CHECKS_OPTIONS,
                        out,
                    );
                }
            }
        }
        _ => out.push(at(
            text,
            node.span,
            diag::invalid_section_format("optional-checks"),
        )),
    }
}

fn validate_experiments(text: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    let Some(nodes) = node.as_list() else {
        out.push(at(
            text,
            node.span,
            diag::invalid_section_format("enable-experiment"),
        ));
        return;
    };
    let mut prior: HashMap<&str, bool> = HashMap::new();
    for item in nodes {
        let Some(flag) = item.string_value() else {
            out.push(at(
                text,
                item.span,
                diag::invalid_section_format("enable-experiment"),
            ));
            continue;
        };
        let (name, requested) = flag
            .strip_prefix("no-")
            .map_or((flag, true), |name| (name, false));
        let Some(feature) = experiments::feature(name) else {
            out.push(at(
                text,
                item.span,
                diag::unsupported_option_without_values("enable-experiment", flag),
            ));
            continue;
        };
        let message = if feature.is_expired {
            if requested == feature.is_enabled_by_default {
                Some(format!("Flag \"{flag}\" is no longer required."))
            } else {
                let state = if feature.is_enabled_by_default {
                    "enabled"
                } else {
                    "disabled"
                };
                Some(format!(
                    "Flag \"{flag}\" was supplied, but the feature is already unconditionally {state}."
                ))
            }
        } else if prior.get(name).is_some_and(|old| *old != requested) {
            let previous = if requested {
                format!("no-{name}")
            } else {
                name.to_string()
            };
            Some(format!(
                "Flag \"{flag}\" conflicts with previous flag \"{previous}\""
            ))
        } else {
            None
        };
        if let Some(message) = message {
            out.push(at(
                text,
                item.span,
                diag::invalid_option("enable-experiment", &message),
            ));
        } else {
            prior.insert(feature.enable_string, requested);
        }
    }
}

fn validate_string_list(text: &str, section: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    let Some(nodes) = node.as_list() else {
        out.push(at(text, node.span, diag::invalid_section_format(section)));
        return;
    };
    for value in nodes {
        if value.string_value().is_none() {
            out.push(at(text, value.span, diag::invalid_section_format(section)));
        }
    }
}

fn validate_plugin(text: &str, path: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    if node.string_value().is_some() {
        return;
    }
    let Some(entries) = node.as_map() else {
        out.push(at(text, node.span, diag::invalid_section_format(path)));
        return;
    };
    for (key, value) in entries {
        let Some(name) = key.string_value() else {
            continue;
        };
        match name {
            "diagnostics" => {
                validate_plugin_diagnostics(text, &format!("{path}/diagnostics"), value, out)
            }
            "git" => validate_git(text, &format!("{path}/git"), value, out),
            _ if !PLUGIN_OPTIONS.contains(&name) => {
                unsupported_key(text, key, path, PLUGIN_OPTIONS, out)
            }
            _ => {}
        }
    }
}

fn validate_plugin_diagnostics(text: &str, path: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    let Some(entries) = node.as_map() else {
        out.push(at(text, node.span, diag::invalid_section_format(path)));
        return;
    };
    for (_, value) in entries {
        let Some(scalar) = value.scalar() else {
            out.push(at(text, value.span, diag::invalid_section_format(path)));
            return;
        };
        let severity = scalar.to_dart_string().to_ascii_lowercase();
        if !ERROR_PROCESSOR_VALUES.contains(&severity.as_str()) {
            out.push(at(
                text,
                value.span,
                diag::unsupported_option_with_legal_values(
                    path,
                    &scalar.to_dart_string(),
                    &quoted(ERROR_PROCESSOR_VALUES, "and"),
                ),
            ));
        }
    }
}

fn validate_git(text: &str, path: &str, node: &YamlNode, out: &mut Vec<Diagnostic>) {
    if let Some(scalar) = node.scalar() {
        if !matches!(scalar, Scalar::String(_)) {
            out.push(at(text, node.span, diag::invalid_section_format(path)));
        }
        return;
    }
    let Some(entries) = node.as_map() else {
        out.push(at(text, node.span, diag::invalid_section_format(path)));
        return;
    };
    for (key, value) in entries {
        let Some(name) = key.string_value() else {
            continue;
        };
        if !GIT_OPTIONS.contains(&name) {
            unsupported_key(text, key, path, GIT_OPTIONS, out);
        } else if !matches!(value.scalar(), Some(Scalar::String(_))) {
            out.push(at(
                text,
                value.span,
                diag::invalid_section_format(&format!("{path}/{name}")),
            ));
        }
    }
}

fn legacy_plugins(node: &YamlNode) -> Vec<(String, YamlNode)> {
    let nodes: Vec<&YamlNode> = match &node.kind {
        NodeKind::List(nodes) => nodes.iter().collect(),
        NodeKind::Map(entries) => entries.iter().map(|(key, _)| key).collect(),
        _ => vec![node],
    };
    nodes
        .into_iter()
        .filter_map(|node| {
            node.string_value()
                .map(|name| (name.to_string(), node.clone()))
        })
        .collect()
}

fn unsupported_keys(
    text: &str,
    entries: &[(YamlNode, YamlNode)],
    section: &str,
    legal: &[&str],
    out: &mut Vec<Diagnostic>,
) {
    for (key, _) in entries {
        if key
            .string_value()
            .is_some_and(|name| !legal.contains(&name))
        {
            unsupported_key(text, key, section, legal, out);
        }
    }
}

fn unsupported_key(
    text: &str,
    key: &YamlNode,
    section: &str,
    legal: &[&str],
    out: &mut Vec<Diagnostic>,
) {
    let key_text = node_value(key);
    let located = match legal.len() {
        0 => diag::unsupported_option_without_values(section, &key_text),
        1 => diag::unsupported_option_with_legal_value(section, &key_text, legal[0]),
        _ => diag::unsupported_option_with_legal_values(section, &key_text, &quoted(legal, "and")),
    };
    out.push(at(text, key.span, located));
}

fn entry_at<'a>(
    entries: &'a [(YamlNode, YamlNode)],
    name: &str,
) -> Option<(&'a YamlNode, &'a YamlNode)> {
    entries
        .iter()
        .find(|(key, _)| key.string_value() == Some(name))
        .map(|(key, value)| (key, value))
}

fn value_at<'a>(entries: &'a [(YamlNode, YamlNode)], name: &str) -> Option<&'a YamlNode> {
    entry_at(entries, name).map(|(_, value)| value)
}

fn node_value(node: &YamlNode) -> String {
    match &node.kind {
        NodeKind::Scalar(value) => value.to_dart_string(),
        NodeKind::List(values) => format!(
            "[{}]",
            values.iter().map(node_value).collect::<Vec<_>>().join(", ")
        ),
        NodeKind::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("{}: {}", node_value(key), node_value(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn quoted(values: &[&str], conjunction: &str) -> String {
    match values {
        [] => String::new(),
        [only] => format!("'{only}'"),
        [first, second] => format!("'{first}' {conjunction} '{second}'"),
        _ => {
            let mut result = values[..values.len() - 1]
                .iter()
                .map(|v| format!("'{v}'"))
                .collect::<Vec<_>>()
                .join(", ");
            result.push_str(&format!(", {conjunction} '{}'", values.last().unwrap()));
            result
        }
    }
}

fn at(text: &str, span: Span, diagnostic: LocatableDiagnostic) -> Diagnostic {
    let (offset, length) = utf16_span(text, span);
    diagnostic.to_diagnostic(offset, length)
}

fn utf16_span(text: &str, span: Span) -> (usize, usize) {
    let start = span.start.min(text.len());
    let end = span.end.min(text.len()).max(start);
    (
        text[..start].encode_utf16().count(),
        text[start..end].encode_utf16().count(),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MinimumVersion {
    version: (u64, u64, u64),
    pre_release: Vec<VersionIdentifier>,
    build: Vec<VersionIdentifier>,
}

impl MinimumVersion {
    fn includes(&self, since: (u16, u16, u16)) -> bool {
        let since = MinimumVersion {
            version: (since.0.into(), since.1.into(), since.2.into()),
            pre_release: Vec::new(),
            build: Vec::new(),
        };
        self.compare_to(&since).is_ge()
    }

    /// `Version.compareTo` from `package:pub_semver`.
    fn compare_to(&self, other: &MinimumVersion) -> Ordering {
        let ordering = self.version.cmp(&other.version);
        if ordering != Ordering::Equal {
            return ordering;
        }
        if self.pre_release.is_empty() && !other.pre_release.is_empty() {
            return Ordering::Greater;
        }
        if other.pre_release.is_empty() && !self.pre_release.is_empty() {
            return Ordering::Less;
        }
        let ordering = compare_version_identifiers(&self.pre_release, &other.pre_release);
        if ordering != Ordering::Equal {
            return ordering;
        }
        compare_version_identifiers(&self.build, &other.build)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VersionIdentifier {
    Number(u64),
    Text(String),
}

fn compare_version_identifiers(
    first: &[VersionIdentifier],
    second: &[VersionIdentifier],
) -> Ordering {
    for index in 0..first.len().max(second.len()) {
        let ordering = match (first.get(index), second.get(index)) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(VersionIdentifier::Number(first)), Some(VersionIdentifier::Number(second))) => {
                first.cmp(second)
            }
            (Some(VersionIdentifier::Number(_)), Some(VersionIdentifier::Text(_))) => {
                Ordering::Less
            }
            (Some(VersionIdentifier::Text(_)), Some(VersionIdentifier::Number(_))) => {
                Ordering::Greater
            }
            (Some(VersionIdentifier::Text(first)), Some(VersionIdentifier::Text(second))) => {
                first.cmp(second)
            }
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn sdk_minimum(context: &AnalysisContext, path: &str) -> Option<MinimumVersion> {
    let WorkspacePackage::Pub {
        sdk_constraint: Some(constraint),
        ..
    } = context.root.workspace.find_package_for(path)?
    else {
        return None;
    };
    parse_minimum_version(&constraint)
}

fn parse_minimum_version(constraint: &str) -> Option<MinimumVersion> {
    let constraint = constraint.trim();
    // `pub_semver` represents unions separately from `VersionRange`; upstream
    // intentionally does not infer a lifecycle lower bound from a union.
    if constraint.contains("||") {
        return None;
    }
    let tokens: Vec<_> = constraint.split_whitespace().collect();
    if tokens.is_empty() || tokens == ["any"] {
        return None;
    }
    let mut terms = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        let (operator, version) = if matches!(token, ">=" | ">" | "<=" | "<" | "^") {
            index += 1;
            (token, *tokens.get(index)?)
        } else if let Some(version) = token.strip_prefix(">=") {
            (">=", version)
        } else if let Some(version) = token.strip_prefix("<=") {
            ("<=", version)
        } else if let Some(version) = token.strip_prefix('>') {
            (">", version)
        } else if let Some(version) = token.strip_prefix('<') {
            ("<", version)
        } else if let Some(version) = token.strip_prefix('^') {
            ("^", version)
        } else {
            ("=", token)
        };
        if version.is_empty() {
            return None;
        }
        terms.push((operator, parse_version(version)?));
        index += 1;
    }
    if terms.iter().any(|(operator, _)| *operator == "^") {
        return (terms.len() == 1).then(|| terms[0].1.clone());
    }

    let mut minimum: Option<(MinimumVersion, bool)> = None;
    let mut maximum: Option<(MinimumVersion, bool)> = None;
    for (operator, version) in terms {
        match operator {
            ">=" => update_minimum(&mut minimum, version, true),
            ">" => update_minimum(&mut minimum, version, false),
            "<=" => update_maximum(&mut maximum, version, true),
            "<" => update_maximum(&mut maximum, version, false),
            "=" => {
                update_minimum(&mut minimum, version.clone(), true);
                update_maximum(&mut maximum, version, true);
            }
            _ => unreachable!(),
        }
    }
    let (minimum, include_minimum) = minimum?;
    if let Some((maximum, include_maximum)) = maximum {
        let ordering = minimum.compare_to(&maximum);
        if ordering.is_gt() || (ordering.is_eq() && (!include_minimum || !include_maximum)) {
            return None;
        }
    }
    Some(minimum)
}

fn parse_version(version: &str) -> Option<MinimumVersion> {
    let numeric_end = version
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(version.len());
    let suffix = &version[numeric_end..];
    let valid_identifiers = |text: &str| {
        !text.is_empty()
            && text.split('.').all(|identifier| {
                !identifier.is_empty()
                    && identifier
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    };
    let (pre_release, build) = if let Some(rest) = suffix.strip_prefix('-') {
        if let Some((pre, build)) = rest.split_once('+') {
            if !valid_identifiers(pre) || !valid_identifiers(build) || build.contains('+') {
                return None;
            }
            (pre, build)
        } else {
            if !valid_identifiers(rest) {
                return None;
            }
            (rest, "")
        }
    } else if let Some(build) = suffix.strip_prefix('+') {
        if !valid_identifiers(build) || build.contains('+') {
            return None;
        }
        ("", build)
    } else if suffix.is_empty() {
        ("", "")
    } else {
        return None;
    };
    let parts: Vec<_> = version[..numeric_end].split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(MinimumVersion {
        version: (
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ),
        pre_release: parse_version_identifiers(pre_release),
        build: parse_version_identifiers(build),
    })
}

fn parse_version_identifiers(text: &str) -> Vec<VersionIdentifier> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('.')
        .map(|identifier| {
            identifier.parse().map_or_else(
                |_| VersionIdentifier::Text(identifier.to_string()),
                VersionIdentifier::Number,
            )
        })
        .collect()
}

fn update_minimum(
    current: &mut Option<(MinimumVersion, bool)>,
    candidate: MinimumVersion,
    inclusive: bool,
) {
    if current.as_ref().is_none_or(|(version, current_inclusive)| {
        candidate.compare_to(version).is_gt()
            || (candidate.compare_to(version).is_eq() && !inclusive && *current_inclusive)
    }) {
        *current = Some((candidate, inclusive));
    }
}

fn update_maximum(
    current: &mut Option<(MinimumVersion, bool)>,
    candidate: MinimumVersion,
    inclusive: bool,
) {
    if current.as_ref().is_none_or(|(version, current_inclusive)| {
        candidate.compare_to(version).is_lt()
            || (candidate.compare_to(version).is_eq() && !inclusive && *current_inclusive)
    }) {
        *current = Some((candidate, inclusive));
    }
}

fn version_text(version: Option<(u16, u16, u16)>) -> String {
    version.map_or_else(String::new, |(major, minor, patch)| {
        format!("{major}.{minor}.{patch}")
    })
}
