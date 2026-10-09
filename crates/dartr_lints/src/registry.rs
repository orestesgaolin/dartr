// Dart source: pkg/analyzer/lib/src/lint/registry.dart
// Dart source: pkg/analyzer/lib/analysis_rule/rule_state.dart
// Dart source: pkg/analyzer/lib/src/lint/config.dart
use dartr_diagnostics::{DiagnosticCode, DiagnosticSeverity, Origin, all_codes};
use indexmap::IndexMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleStateType {
    Stable,
    Experimental,
    Internal,
    Testing,
    Deprecated,
    Removed,
}
#[derive(Clone, Copy, Debug)]
pub struct RuleState {
    pub kind: RuleStateType,
    pub since: Option<(u32, u32, u32)>,
    pub replaced_by: Option<&'static str>,
}
#[derive(Clone, Copy, Debug)]
pub struct AnalysisRule {
    pub name: &'static str,
    pub description: &'static str,
    pub state: RuleState,
    /// Dart constant names, including multiple codes for MultiAnalysisRule.
    pub diagnostic_names: &'static [&'static str],
    pub can_use_parsed_result: bool,
    pub incompatible_rules: &'static [&'static str],
}
pub type LintRule = AnalysisRule;
pub type AbstractAnalysisRule = AnalysisRule;
pub type MultiAnalysisRule = AnalysisRule;
impl AnalysisRule {
    pub fn can_use_parsed_result(&self) -> bool {
        self.can_use_parsed_result
    }
    pub fn is_implemented(&self) -> bool {
        crate::rules::implemented_rules().contains(&self.name)
    }
    pub fn diagnostic_codes(&self) -> impl Iterator<Item = &'static DiagnosticCode> + '_ {
        self.diagnostic_names.iter().filter_map(|name| {
            all_codes()
                .iter()
                .copied()
                .find(|code| code.origin == Origin::Linter && code.camel_case_name == *name)
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfiguredSeverity {
    Disable,
    Enable,
    Info,
    Warning,
    Error,
}
#[derive(Clone, Debug)]
pub struct RuleConfig {
    pub name: String,
    pub group: Option<String>,
    pub severity: ConfiguredSeverity,
}
impl RuleConfig {
    pub fn is_enabled(&self) -> bool {
        self.severity != ConfiguredSeverity::Disable
    }
    pub fn diagnostic_severity(&self) -> Option<DiagnosticSeverity> {
        match self.severity {
            ConfiguredSeverity::Disable => Some(DiagnosticSeverity::None),
            ConfiguredSeverity::Enable => None,
            ConfiguredSeverity::Info => Some(DiagnosticSeverity::Info),
            ConfiguredSeverity::Warning => Some(DiagnosticSeverity::Warning),
            ConfiguredSeverity::Error => Some(DiagnosticSeverity::Error),
        }
    }
}
#[derive(Default)]
pub struct Registry {
    pub lint_rules: IndexMap<String, &'static AnalysisRule>,
    pub warning_rules: IndexMap<String, &'static AnalysisRule>,
    pub code_map: IndexMap<String, &'static DiagnosticCode>,
}
impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        for rule in crate::rule_metadata::ALL_RULES {
            registry.register_lint_rule(rule);
        }
        registry
    }
    pub fn register_lint_rule(&mut self, rule: &'static AnalysisRule) {
        self.lint_rules.insert(rule.name.to_ascii_lowercase(), rule);
        for code in rule.diagnostic_codes() {
            self.code_map
                .insert(code.unique_name.to_ascii_lowercase(), code);
        }
    }
    pub fn register_warning_rule(&mut self, rule: &'static AnalysisRule) {
        self.warning_rules
            .insert(rule.name.to_ascii_lowercase(), rule);
        for code in rule.diagnostic_codes() {
            self.code_map
                .insert(code.unique_name.to_ascii_lowercase(), code);
        }
    }
    pub fn get_rule(&self, name: &str) -> Option<&'static AnalysisRule> {
        let key = name.to_ascii_lowercase();
        self.warning_rules
            .get(&key)
            .or_else(|| self.lint_rules.get(&key))
            .copied()
    }
    pub fn code_for_unique_name(&self, name: &str) -> Option<&'static DiagnosticCode> {
        self.code_map.get(&name.to_ascii_lowercase()).copied()
    }
    pub fn unregister_lint_rule(&mut self, name: &str) {
        if let Some(rule) = self.lint_rules.shift_remove(&name.to_ascii_lowercase()) {
            for code in rule.diagnostic_codes() {
                self.code_map.shift_remove(code.unique_name);
            }
        }
    }
    pub fn unregister_warning_rule(&mut self, name: &str) {
        if let Some(rule) = self.warning_rules.shift_remove(&name.to_ascii_lowercase()) {
            for code in rule.diagnostic_codes() {
                self.code_map.shift_remove(code.unique_name);
            }
        }
    }
    pub fn rules(&self) -> impl Iterator<Item = &'static AnalysisRule> + '_ {
        let mut rules = IndexMap::new();
        for rule in self.lint_rules.values().chain(self.warning_rules.values()) {
            rules.insert(rule.name, *rule);
        }
        rules.into_values()
    }
    pub fn enabled<'a>(
        &'a self,
        configs: &'a IndexMap<String, RuleConfig>,
    ) -> impl Iterator<Item = &'static AnalysisRule> + 'a {
        self.warning_rules
            .iter()
            .filter(move |(name, _)| configs.get(*name).is_none_or(RuleConfig::is_enabled))
            .map(|(_, rule)| *rule)
            .chain(
                self.lint_rules
                    .iter()
                    .filter(move |(name, _)| configs.get(*name).is_some_and(RuleConfig::is_enabled))
                    .map(|(_, rule)| *rule),
            )
    }
}
