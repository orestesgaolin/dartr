// Dart source: pkg/analyzer/lib/src/analysis_rule/analysis_rule.dart
// Dart source: pkg/analyzer/lib/src/analysis_rule/rule_context.dart
// Dart source: pkg/analyzer/lib/src/lint/linter_visitor.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)

use dartr_ast::{Ast, NodeId, NodeKind};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::{Diagnostic, DiagnosticCode};
use dartr_syntax::TokenId;
use indexmap::{IndexMap, IndexSet};

mod analysis_rule_timers;
mod ignore_info;
pub mod rules;
pub use analysis_rule_timers::{AnalysisRuleTimers, RuleTimer};
pub use ignore_info::IgnoreInfo;
mod registry;
mod rule_metadata;
mod subscriptions;
pub use dartr_parser::experimental_flags::ExperimentalFlag;
pub use registry::*;
pub use rule_metadata::ALL_RULES;
#[derive(Clone, Copy)]
pub struct RuleContextUnit<'a> {
    pub parsed: &'a ParsedUnit,
    pub source: &'a str,
    pub path: &'a str,
}
pub struct LinterContext<'a> {
    pub parsed: &'a ParsedUnit,
    pub ast: &'a Ast,
    pub source: &'a str,
    pub path: &'a str,
    pub all_units: &'a [RuleContextUnit<'a>],
    pub current_unit: usize,
}
pub type RuleContext<'a> = LinterContext<'a>;
impl<'a> LinterContext<'a> {
    /// The library's defining unit, including when a processor visits a part.
    pub fn defining_unit(&self) -> &RuleContextUnit<'a> {
        &self.all_units[0]
    }
    /// Workspace package root for the defining unit, if a pubspec exists.
    pub fn package_root(&self) -> Option<std::path::PathBuf> {
        let path = self.all_units.first()?.path;
        let parent = std::path::Path::new(path).parent()?;
        parent
            .ancestors()
            .find(|root| root.join("pubspec.yaml").is_file())
            .map(std::path::Path::to_path_buf)
    }
    pub fn is_in_lib_dir(&self) -> bool {
        self.package_root().is_some_and(|root| {
            std::path::Path::new(self.all_units[0].path).starts_with(root.join("lib"))
        })
    }
    pub fn is_in_test_directory(&self) -> bool {
        self.package_root().is_some_and(|root| {
            std::path::Path::new(self.all_units[0].path).starts_with(root.join("test"))
        })
    }
    pub fn is_feature_enabled(&self, feature: ExperimentalFlag) -> bool {
        self.defining_unit()
            .parsed
            .feature_set
            .is_experiment_enabled(feature)
    }
    /// The visited unit's source URI. The analyzer uses a
    /// package URI for files mapped by package_config, and a file URI otherwise.
    pub fn source_uri(&self) -> String {
        let file = std::path::Path::new(self.path);
        for root in file.ancestors().skip(1) {
            let config = root.join(".dart_tool/package_config.json");
            let Ok(content) = std::fs::read_to_string(&config) else {
                continue;
            };
            let Ok(config_value) = serde_json::from_str::<serde_json::Value>(&content) else {
                break;
            };
            if let Some(packages) = config_value["packages"].as_array() {
                for package in packages {
                    let (Some(name), Some(root_uri)) =
                        (package["name"].as_str(), package["rootUri"].as_str())
                    else {
                        continue;
                    };
                    let package_root = if let Some(path) = root_uri.strip_prefix("file://") {
                        std::path::PathBuf::from(path)
                    } else if root_uri.contains(':') {
                        continue;
                    } else {
                        config.parent().unwrap().join(root_uri)
                    };
                    let package_dir =
                        package_root.join(package["packageUri"].as_str().unwrap_or(""));
                    if let Ok(package_dir) = package_dir.canonicalize()
                        && let Ok(relative) = file.strip_prefix(&package_dir)
                    {
                        return format!(
                            "package:{name}/{}",
                            relative.to_string_lossy().replace('\\', "/")
                        );
                    }
                }
            }
            break;
        }
        if file.is_absolute() {
            format!("file://{}", self.path.replace('\\', "/"))
        } else {
            self.path.replace('\\', "/")
        }
    }
    pub fn report_node(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        node: impl Into<NodeId>,
        args: &[&str],
    ) {
        let node = node.into();
        if !self.is_synthetic(node) {
            self.report_offset(
                out,
                code,
                self.ast.offset(node) as usize,
                self.ast.length(node) as usize,
                args,
            );
        }
    }
    pub fn report_token(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        token: TokenId,
        args: &[&str],
    ) {
        let token = self.ast.tokens.get(token);
        if !token.is_synthetic() {
            self.report_offset(
                out,
                code,
                token.offset as usize,
                token.length as usize,
                args,
            );
        }
    }
    pub fn report_offset(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        offset: usize,
        length: usize,
        args: &[&str],
    ) {
        out.push(Diagnostic::new(code, offset, length, args, vec![]));
    }
    pub fn is_synthetic(&self, node: impl Into<NodeId>) -> bool {
        use dartr_ast::{ExpressionStatement, Id, NamedType};
        let node = node.into();
        match self.ast.kind(node) {
            NodeKind::BooleanLiteral
            | NodeKind::SimpleIdentifier
            | NodeKind::SimpleStringLiteral
            | NodeKind::EmptyStatement => self
                .ast
                .tokens
                .get(self.ast.begin_token(node))
                .is_synthetic(),
            NodeKind::NamedType => {
                let n = &self.ast[Id::<NamedType>::from_raw(node)];
                self.ast.tokens.get(n.name).is_synthetic() && n.type_arguments.is_none()
            }
            NodeKind::ExpressionStatement => {
                let n = &self.ast[Id::<ExpressionStatement>::from_raw(node)];
                self.is_synthetic(n.expression)
                    && n.semicolon
                        .is_none_or(|token| self.ast.tokens.get(token).is_synthetic())
            }
            _ => false,
        }
    }
    pub fn text(&self, node: impl Into<NodeId>) -> String {
        let node = node.into();
        if self.ast.kind(node) == NodeKind::CompilationUnit {
            return self.source.to_owned();
        }
        let start = self.ast.tokens.byte_range(self.ast.begin_token(node)).start;
        let end = self.ast.tokens.byte_range(self.ast.end_token(node)).end;
        if let Some(text) = self.source.get(start..end) {
            return text.to_owned();
        }
        String::from_utf16_lossy(
            &self
                .source
                .encode_utf16()
                .skip(self.ast.offset(node) as usize)
                .take(self.ast.length(node) as usize)
                .collect::<Vec<_>>(),
        )
    }
}
pub type NodeProcessor = fn(&LinterContext<'_>, NodeId, &mut Vec<Diagnostic>);
pub type AfterLibraryProcessor = fn(&LinterContext<'_>, &mut Vec<Diagnostic>);
#[derive(Default)]
pub struct RuleVisitorRegistry {
    pub enable_timing: bool,
    pub timers: std::cell::RefCell<AnalysisRuleTimers>,
    subscriptions: IndexMap<NodeKind, Vec<(&'static str, NodeProcessor)>>,
    after_library: Vec<(&'static str, AfterLibraryProcessor)>,
}
pub type NodeLintRegistry = RuleVisitorRegistry;
impl RuleVisitorRegistry {
    pub fn with_timing(enable_timing: bool) -> Self {
        Self {
            enable_timing,
            ..Self::default()
        }
    }
    pub fn add(&mut self, kind: NodeKind, rule: &'static str, processor: NodeProcessor) {
        self.subscriptions
            .entry(kind)
            .or_default()
            .push((rule, processor));
    }
    pub fn add_after_library(&mut self, rule: &'static str, processor: AfterLibraryProcessor) {
        self.after_library.push((rule, processor));
    }
}
pub fn lint(parsed: &ParsedUnit, source: &str, path: &str, enabled: &[&str]) -> Vec<Diagnostic> {
    lint_library(
        &[RuleContextUnit {
            parsed,
            source,
            path,
        }],
        enabled,
    )
    .remove(0)
}

/// Register once per library, visit each unit in source order, then run callbacks.
/// The first unit is the defining unit. All units must exist and be parsed.
pub fn lint_library(units: &[RuleContextUnit<'_>], enabled: &[&str]) -> Vec<Vec<Diagnostic>> {
    let mut out = lint_library_unfiltered(units, enabled);
    for (index, diagnostics) in out.iter_mut().enumerate() {
        let ctx = LinterContext {
            parsed: units[index].parsed,
            ast: &units[index].parsed.ast,
            source: units[index].source,
            path: units[index].path,
            all_units: units,
            current_unit: index,
        };
        let ignores = IgnoreInfo::for_dart(&ctx);
        diagnostics.retain(|diagnostic| !ignores.ignored(&ctx, diagnostic));
    }
    out
}

/// Like [lint_library], without the ignore-comment filtering. The analyzer
/// filters all diagnostics of a unit together, with the `cannot-ignore`
/// codes of the analysis options: callers that do that use this function.
pub fn lint_library_unfiltered(
    units: &[RuleContextUnit<'_>],
    enabled: &[&str],
) -> Vec<Vec<Diagnostic>> {
    let Some(_) = units.first() else {
        return vec![];
    };
    let context = |index: usize| {
        let unit = &units[index];
        LinterContext {
            parsed: unit.parsed,
            ast: &unit.parsed.ast,
            source: unit.source,
            path: unit.path,
            all_units: units,
            current_unit: index,
        }
    };
    let mut registry = RuleVisitorRegistry::default();
    let enabled: IndexSet<_> = enabled.iter().map(|s| s.to_ascii_lowercase()).collect();
    let defining_context = context(0);
    for rule in ALL_RULES {
        if enabled.contains(rule.name) {
            rules::register(rule.name, &mut registry, &defining_context);
        }
    }
    let visitor = AnalysisRuleVisitor::new(&registry);
    let mut out = Vec::with_capacity(units.len());
    for (index, unit) in units.iter().enumerate() {
        let ctx = context(index);
        let mut diagnostics = vec![];
        visitor.visit(&ctx, unit.parsed.unit.raw(), &mut diagnostics);
        out.push(diagnostics);
    }
    let last_index = units.len() - 1;
    visitor.after_library(&context(last_index), &mut out[last_index]);
    out
}

/// The generated Dart visitor runs each node's subscriptions before children.
/// Panics propagate, so a failed rule cannot silently produce incomplete output.
pub struct AnalysisRuleVisitor<'a> {
    registry: &'a RuleVisitorRegistry,
}
impl<'a> AnalysisRuleVisitor<'a> {
    pub fn new(registry: &'a RuleVisitorRegistry) -> Self {
        Self { registry }
    }
    pub fn visit(&self, ctx: &LinterContext<'_>, root: NodeId, out: &mut Vec<Diagnostic>) {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if let Some(subscriptions) = self.registry.subscriptions.get(&ctx.ast.kind(node)) {
                for (rule, processor) in subscriptions {
                    if self.registry.enable_timing {
                        self.registry.timers.borrow_mut().get_timer(rule).start();
                    }
                    processor(ctx, node, out);
                    if self.registry.enable_timing {
                        self.registry.timers.borrow_mut().get_timer(rule).stop();
                    }
                }
            }
            pending.extend(ctx.ast.children(node).into_iter().rev());
        }
    }
    pub fn after_library(&self, ctx: &LinterContext<'_>, out: &mut Vec<Diagnostic>) {
        for (rule, callback) in &self.registry.after_library {
            if self.registry.enable_timing {
                self.registry.timers.borrow_mut().get_timer(rule).start();
            }
            callback(ctx, out);
            if self.registry.enable_timing {
                self.registry.timers.borrow_mut().get_timer(rule).stop();
            }
        }
    }
}

/// Run enabled AST rules using the parsed unit's source text.
/// File-sensitive rules should use `lint` with the actual file path.
pub fn run_lints(parsed: &ParsedUnit, enabled: &[&str]) -> Vec<Diagnostic> {
    lint(parsed, &parsed.ast.tokens.source, "", enabled)
}

/// Apply explicitly enabled configurations and severity overrides.
pub fn lint_with_config(
    parsed: &ParsedUnit,
    source: &str,
    path: &str,
    configs: &IndexMap<String, RuleConfig>,
) -> Vec<Diagnostic> {
    let registry = Registry::builtin();
    let enabled: Vec<_> = registry.enabled(configs).map(|rule| rule.name).collect();
    let mut diagnostics = lint(parsed, source, path, &enabled);
    for diagnostic in &mut diagnostics {
        if let Some(config) = configs.get(diagnostic.code.name)
            && let Some(severity) = config.diagnostic_severity()
        {
            diagnostic.severity = severity;
        }
    }
    diagnostics
}
