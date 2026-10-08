//! `analysis_options.yaml`: parsing, `include:` resolution, merging, and the
//! effective analysis options.
//!
//! Ports `pkg/analyzer/lib/src/analysis_options/analysis_options_parser.dart`
//! (`AnalysisOptionsParseSession`, `_ParseRequest`, `_ApplyState`),
//! `analysis_options_parse_model.dart` (the `_Parsed*Data` classes, without
//! the diagnostics), `analysis_options.dart` (`AnalysisOptionsBuilder`),
//! `analysis_options_file.dart` (keys), `pkg/analyzer/lib/src/lint/config.dart`
//! (`parseLinterSection`, `parseDiagnosticsSection`) and `ErrorProcessor` of
//! `pkg/analyzer/lib/source/error_processor.dart`.
//!
//! The YAML of each file is kept with its spans in [FileContent], so that
//! the diagnostics of options files can be reported later.

use crate::yaml::{self, NodeKind, Scalar, YamlError, YamlNode};
use crate::{experiments, fs, lint_rules, paths};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Keys of an analysis options file (`AnalysisOptionsFileKeys`).
pub mod keys {
    pub const ANALYZER: &str = "analyzer";
    pub const CODE_STYLE: &str = "code-style";
    pub const FORMATTER: &str = "formatter";
    pub const LINTER: &str = "linter";
    pub const PLUGINS: &str = "plugins";
    pub const CANNOT_IGNORE: &str = "cannot-ignore";
    pub const ENABLE_EXPERIMENT: &str = "enable-experiment";
    pub const ERRORS: &str = "errors";
    pub const EXCLUDE: &str = "exclude";
    pub const INCLUDE: &str = "include";
    pub const LANGUAGE: &str = "language";
    pub const OPTIONAL_CHECKS: &str = "optional-checks";
    pub const STRONG_MODE: &str = "strong-mode";
    pub const CHROME_OS_MANIFEST_CHECKS: &str = "chrome-os-manifest-checks";
    pub const PROPAGATE_LINTER_EXCEPTIONS: &str = "propagate-linter-exceptions";
    pub const STRICT_CASTS: &str = "strict-casts";
    pub const STRICT_INFERENCE: &str = "strict-inference";
    pub const STRICT_RAW_TYPES: &str = "strict-raw-types";
    pub const FORMAT: &str = "format";
    pub const PAGE_WIDTH: &str = "page_width";
    pub const TRAILING_COMMAS: &str = "trailing_commas";
    pub const RULES: &str = "rules";
    pub const DIAGNOSTICS: &str = "diagnostics";
    pub const GIT: &str = "git";
    pub const PATH: &str = "path";
    pub const REF: &str = "ref";
    pub const TAG_PATTERN: &str = "tag_pattern";
    pub const URL: &str = "url";
    pub const VERSION: &str = "version";
    pub const HOSTED: &str = "hosted";
    pub const DEPENDENCY_OVERRIDES: &str = "dependency_overrides";
    /// Ways to say `ignore`.
    pub const IGNORE_SYNONYMS: [&str; 2] = ["ignore", "false"];
    /// Valid severities (`severityMap.keys`).
    pub const SEVERITIES: [&str; 3] = ["error", "info", "warning"];
}

/// The severity of a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl DiagnosticSeverity {
    /// `severityMap` of `error_processor.dart`.
    pub fn from_option(name: &str) -> Option<DiagnosticSeverity> {
        match name {
            "error" => Some(DiagnosticSeverity::Error),
            "info" => Some(DiagnosticSeverity::Info),
            "warning" => Some(DiagnosticSeverity::Warning),
            _ => None,
        }
    }

    /// The name of the Dart constant (`DiagnosticSeverity.name`).
    pub fn name(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Info => "INFO",
            DiagnosticSeverity::Warning => "WARNING",
            DiagnosticSeverity::Error => "ERROR",
        }
    }
}

/// Changes the severity of a diagnostic code, or filters it (`ErrorProcessor`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorProcessor {
    /// The lower case code name.
    pub code: String,
    /// The new severity, or `None` to ignore the diagnostic.
    pub severity: Option<DiagnosticSeverity>,
}

/// The configured severity of an analysis rule (`ConfiguredSeverity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfiguredSeverity {
    Disable,
    Enable,
    Info,
    Warning,
    Error,
}

impl ConfiguredSeverity {
    pub fn name(&self) -> &'static str {
        match self {
            ConfiguredSeverity::Disable => "disable",
            ConfiguredSeverity::Enable => "enable",
            ConfiguredSeverity::Info => "info",
            ConfiguredSeverity::Warning => "warning",
            ConfiguredSeverity::Error => "error",
        }
    }

    fn from_name(name: &str) -> Option<ConfiguredSeverity> {
        Some(match name {
            "disable" => ConfiguredSeverity::Disable,
            "enable" => ConfiguredSeverity::Enable,
            "info" => ConfiguredSeverity::Info,
            "warning" => ConfiguredSeverity::Warning,
            "error" => ConfiguredSeverity::Error,
            _ => return None,
        })
    }
}

/// The configuration of one rule (`RuleConfig`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleConfig {
    pub name: String,
    pub group: Option<String>,
    pub severity: ConfiguredSeverity,
}

impl RuleConfig {
    pub fn is_enabled(&self) -> bool {
        self.severity != ConfiguredSeverity::Disable
    }
}

/// Rule configurations by name, in insertion order.
pub type RuleConfigs = Vec<(String, RuleConfig)>;

fn put<V>(map: &mut Vec<(String, V)>, key: String, value: V) {
    match map.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => map.push((key, value)),
    }
}

/// Where a plugin comes from (`PluginSource`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginSource {
    Versioned {
        constraint: String,
        hosted_url: Option<String>,
    },
    Git {
        url: String,
        path: Option<String>,
        git_ref: Option<String>,
        tag_pattern: Option<String>,
    },
    Path {
        path: String,
    },
}

/// A plugin of the top-level `plugins:` section (`PluginConfiguration`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginConfiguration {
    pub name: String,
    pub source: PluginSource,
    pub diagnostic_configs: RuleConfigs,
}

/// `PluginsOptions`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginsOptions {
    pub configurations: Vec<PluginConfiguration>,
    pub dependency_overrides: Option<Vec<(String, PluginSource)>>,
}

/// `TrailingCommas` of the formatter options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas {
    Automate,
    Preserve,
}

impl TrailingCommas {
    pub fn name(&self) -> &'static str {
        match self {
            TrailingCommas::Automate => "automate",
            TrailingCommas::Preserve => "preserve",
        }
    }
}

/// A `cannot-ignore` entry that names a severity: all diagnostics with this
/// severity (after the error processors known at that point) cannot be
/// ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnignorableSeverity {
    pub severity: DiagnosticSeverity,
    pub error_processors: Vec<ErrorProcessor>,
}

/// Effective analysis options (`AnalysisOptionsImpl`).
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisOptions {
    /// The options file, if any.
    pub file: Option<String>,
    /// The `enable-experiment` flags of the file that set them last, or
    /// `None` for the default feature set.
    pub enable_experiment_flags: Option<Vec<String>>,
    pub enabled_legacy_plugin_names: Vec<String>,
    pub error_processors: Vec<ErrorProcessor>,
    pub exclude_patterns: Vec<String>,
    /// `true` if at least one lint rule is enabled.
    pub lint: bool,
    /// The enabled lint rules, sorted by name.
    pub lint_rules: Vec<String>,
    pub propagate_linter_exceptions: bool,
    pub strict_casts: bool,
    pub strict_inference: bool,
    pub strict_raw_types: bool,
    pub chrome_os_manifest_checks: bool,
    pub code_style_use_formatter: bool,
    pub formatter_page_width: Option<i64>,
    pub formatter_trailing_commas: Option<TrailingCommas>,
    /// Lower case names of `cannot-ignore` codes.
    pub unignorable_names: Vec<String>,
    /// `cannot-ignore` entries that name a severity.
    pub unignorable_severities: Vec<UnignorableSeverity>,
    pub plugins: PluginsOptions,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        AnalysisOptions::with_file(None)
    }
}

impl AnalysisOptions {
    /// Default options, for the given options [file].
    pub fn with_file(file: Option<String>) -> Self {
        AnalysisOptions {
            file,
            enable_experiment_flags: None,
            enabled_legacy_plugin_names: Vec::new(),
            error_processors: Vec::new(),
            exclude_patterns: Vec::new(),
            lint: false,
            lint_rules: Vec::new(),
            propagate_linter_exceptions: false,
            strict_casts: false,
            strict_inference: false,
            strict_raw_types: false,
            chrome_os_manifest_checks: false,
            code_style_use_formatter: false,
            formatter_page_width: None,
            formatter_trailing_commas: None,
            unignorable_names: Vec::new(),
            unignorable_severities: Vec::new(),
            plugins: PluginsOptions::default(),
        }
    }

    /// The enabled experimental features (not enabled by default), sorted.
    pub fn enabled_experiments(&self) -> Vec<&'static str> {
        self.enable_experiment_flags
            .as_deref()
            .map(experiments::enabled_experiments)
            .unwrap_or_default()
    }

    /// The error processor for the lower case [code], if any
    /// (`ErrorProcessor.getProcessor`).
    pub fn processor_for(&self, code: &str) -> Option<&ErrorProcessor> {
        self.error_processors.iter().find(|p| p.code == code)
    }

    /// The lower case names of the codes that cannot be ignored
    /// (`unignorableDiagnosticCodeNames`). [codes] lists every diagnostic code
    /// (lower case name and default severity); it is needed for the
    /// `cannot-ignore` entries that name a severity.
    pub fn unignorable_code_names(&self, codes: &[(&str, DiagnosticSeverity)]) -> Vec<String> {
        let mut result: Vec<String> = self.unignorable_names.clone();
        for entry in &self.unignorable_severities {
            for (code, default_severity) in codes {
                let severity = entry
                    .error_processors
                    .iter()
                    .find(|p| p.code == *code)
                    .map(|p| p.severity)
                    .unwrap_or(Some(*default_severity));
                if severity == Some(entry.severity) && !result.iter().any(|n| n == code) {
                    result.push(code.to_string());
                }
            }
        }
        result.sort();
        result.dedup();
        result
    }
}

/// The content of an options file.
#[derive(Debug)]
pub enum FileContent {
    /// The file cannot be read.
    Unreadable,
    /// The file is not valid YAML.
    Malformed { text: String, error: YamlError },
    /// The parsed YAML, with spans into [text].
    Parsed { text: String, yaml: YamlNode },
}

impl FileContent {
    pub fn text(&self) -> Option<&str> {
        match self {
            FileContent::Unreadable => None,
            FileContent::Malformed { text, .. } | FileContent::Parsed { text, .. } => Some(text),
        }
    }

    pub fn yaml(&self) -> Option<&YamlNode> {
        match self {
            FileContent::Parsed { yaml, .. } => Some(yaml),
            _ => None,
        }
    }
}

/// Resolves the URI of an `include:` directive.
pub trait IncludeResolver {
    /// Resolves [uri] relative to the options file [containing_file], and
    /// returns the path of the file, or `None` if the URI cannot be resolved
    /// to a file (`SourceFactory.resolveUri` returning a `FileSource`).
    fn resolve_include(&self, containing_file: &str, uri: &str) -> Option<String>;
}

/// How one `include:` directive resolved.
#[derive(Clone, Debug)]
pub enum IncludeResolution {
    /// The URI did not resolve, or the file cannot be read.
    Missing { uri: String, node: YamlNode },
    /// The file is not valid YAML.
    Malformed {
        uri: String,
        node: YamlNode,
        file: String,
    },
    /// The file was parsed.
    Parsed {
        uri: String,
        node: YamlNode,
        file: String,
    },
}

/// The result of parsing an options file with its includes.
#[derive(Debug)]
pub struct OptionsParseResult {
    pub file: String,
    pub content: Rc<FileContent>,
    pub options: AnalysisOptions,
    /// The include graph: for each parsed file, its resolved includes.
    pub includes: Vec<(String, Vec<IncludeResolution>)>,
}

/// Parses options files and caches their content
/// (`AnalysisOptionsParseSession`).
#[derive(Default, Debug)]
pub struct OptionsParseSession {
    contents: RefCell<HashMap<String, Rc<FileContent>>>,
}

impl OptionsParseSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// The content of [file], read and parsed once.
    pub fn content(&self, file: &str) -> Rc<FileContent> {
        if let Some(content) = self.contents.borrow().get(file) {
            return content.clone();
        }
        let content = Rc::new(match fs::read_string(file) {
            None => FileContent::Unreadable,
            Some(text) => match yaml::load_yaml_node(&text) {
                Ok(yaml) => FileContent::Parsed { text, yaml },
                Err(error) => FileContent::Malformed { text, error },
            },
        });
        self.contents
            .borrow_mut()
            .insert(file.to_string(), content.clone());
        content
    }

    /// Parses [file] and its includes, resolved with [resolver]
    /// (`AnalysisOptionsParseSession.parse`).
    pub fn parse(&self, resolver: &dyn IncludeResolver, file: &str) -> OptionsParseResult {
        let mut request = ParseRequest {
            session: self,
            resolver,
            nodes: HashMap::new(),
            order: Vec::new(),
        };
        let content = request.node(file);
        let options = match content.as_ref() {
            FileContent::Parsed { .. } => {
                let mut state = ApplyState::new(file);
                let mut handled = HashSet::new();
                request.apply_parsed_files(&mut state, file, &mut handled);
                state.build()
            }
            _ => AnalysisOptions::with_file(Some(file.to_string())),
        };
        let includes = request
            .order
            .iter()
            .map(|path| (path.clone(), request.nodes[path].1.clone()))
            .collect();
        OptionsParseResult {
            file: file.to_string(),
            content,
            options,
            includes,
        }
    }
}

struct ParseRequest<'a> {
    session: &'a OptionsParseSession,
    resolver: &'a dyn IncludeResolver,
    /// Content and resolved includes of each file node.
    nodes: HashMap<String, (Rc<FileContent>, Vec<IncludeResolution>)>,
    order: Vec<String>,
}

impl ParseRequest<'_> {
    /// `_fileNodeFor`: reads the file and resolves its includes. The node is
    /// registered before its includes are resolved, so that include cycles
    /// terminate.
    fn node(&mut self, file: &str) -> Rc<FileContent> {
        if let Some((content, _)) = self.nodes.get(file) {
            return content.clone();
        }
        let content = self.session.content(file);
        self.nodes
            .insert(file.to_string(), (content.clone(), Vec::new()));
        self.order.push(file.to_string());
        if let FileContent::Parsed { yaml, .. } = content.as_ref() {
            let mut resolutions = Vec::new();
            for (node, uri) in include_directives(yaml) {
                let resolved = self.resolver.resolve_include(file, &uri);
                let resolution = match resolved {
                    None => IncludeResolution::Missing { uri, node },
                    Some(path) => match self.node(&path).as_ref() {
                        FileContent::Parsed { .. } => IncludeResolution::Parsed {
                            uri,
                            node,
                            file: path,
                        },
                        FileContent::Malformed { .. } => IncludeResolution::Malformed {
                            uri,
                            node,
                            file: path,
                        },
                        FileContent::Unreadable => IncludeResolution::Missing { uri, node },
                    },
                };
                resolutions.push(resolution);
            }
            self.nodes.get_mut(file).unwrap().1 = resolutions;
        }
        content
    }

    /// `_applyParsedFiles`: includes first (each file once), then the file.
    fn apply_parsed_files(
        &self,
        state: &mut ApplyState,
        file: &str,
        handled: &mut HashSet<String>,
    ) {
        let (content, includes) = &self.nodes[file];
        for include in includes {
            if let IncludeResolution::Parsed { file: included, .. } = include
                && handled.insert(included.clone())
            {
                self.apply_parsed_files(state, included, handled);
            }
        }
        if let Some(yaml) = content.yaml() {
            state.apply(&ParsedFileData::parse(yaml, file));
        }
    }
}

/// The `include:` directives of a file: a string, or a list of strings.
fn include_directives(yaml: &YamlNode) -> Vec<(YamlNode, String)> {
    let Some(include) = yaml.value_at(keys::INCLUDE) else {
        return Vec::new();
    };
    match &include.kind {
        NodeKind::Scalar(Scalar::String(uri)) => vec![(include.clone(), uri.clone())],
        NodeKind::List(nodes) => nodes
            .iter()
            .filter_map(|node| {
                node.string_value()
                    .map(|uri| (node.clone(), uri.to_string()))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The local values of one options file (`_ParsedFileData`).
#[derive(Default)]
struct ParsedFileData {
    cannot_ignore: Option<Vec<String>>,
    enable_experiments: Option<Vec<String>>,
    error_processors: Option<Vec<ErrorProcessor>>,
    excludes: Option<Vec<String>>,
    strict_casts: Option<bool>,
    strict_inference: Option<bool>,
    strict_raw_types: Option<bool>,
    legacy_plugins: Option<YamlNode>,
    chrome_os_manifest_checks: Option<bool>,
    propagate_linter_exceptions: Option<bool>,
    use_formatter: Option<bool>,
    page_width: Option<i64>,
    trailing_commas: Option<TrailingCommas>,
    linter: LinterData,
    plugins: PluginsData,
}

#[derive(Default)]
enum LinterData {
    #[default]
    Absent,
    Invalid,
    Valid(RuleConfigs),
}

#[derive(Default)]
struct PluginsData {
    clears_existing: bool,
    configurations: Vec<PluginConfiguration>,
    dependency_overrides: Option<Vec<(String, PluginSource)>>,
}

fn key_is(node: &YamlNode, key: &str) -> bool {
    matches!(&node.kind, NodeKind::Scalar(Scalar::String(s)) if s == key)
}

impl ParsedFileData {
    fn parse(yaml: &YamlNode, file: &str) -> ParsedFileData {
        let mut data = ParsedFileData::default();
        let map = yaml.as_map().map(|_| yaml);

        if let Some(analyzer) = map.and_then(|m| m.value_at(keys::ANALYZER))
            && analyzer.as_map().is_some()
        {
            data.parse_analyzer(analyzer);
        }

        if let Some(code_style) = map.and_then(|m| m.value_at(keys::CODE_STYLE)) {
            for (key, value) in code_style.as_map().unwrap_or_default() {
                if key_is(key, keys::FORMAT) && value.is_scalar() {
                    data.use_formatter = value.to_bool();
                }
            }
        }

        if let Some(formatter) = map.and_then(|m| m.value_at(keys::FORMATTER)) {
            for (key, value) in formatter.as_map().unwrap_or_default() {
                if key_is(key, keys::PAGE_WIDTH) {
                    if let Some(Scalar::Int(width)) = value.scalar()
                        && *width > 0
                    {
                        data.page_width = Some(*width);
                    }
                } else if key_is(key, keys::TRAILING_COMMAS) {
                    data.trailing_commas = match value.string_value() {
                        Some("automate") => Some(TrailingCommas::Automate),
                        Some("preserve") => Some(TrailingCommas::Preserve),
                        _ => None,
                    };
                }
            }
        }

        data.linter = match map {
            None => LinterData::Absent,
            Some(map) => match map.value_at(keys::LINTER) {
                None => LinterData::Absent,
                Some(linter) if linter.is_null_scalar() => LinterData::Absent,
                Some(linter) if linter.as_map().is_none() => LinterData::Invalid,
                Some(linter) => {
                    let mut configs = RuleConfigs::new();
                    if let Some(rules) = linter.value_at(keys::RULES) {
                        for (name, config) in parse_diagnostics_section(rules) {
                            put(&mut configs, name.to_lowercase(), config);
                        }
                    }
                    LinterData::Valid(configs)
                }
            },
        };

        if let Some(plugins) = map.and_then(|m| m.value_at(keys::PLUGINS)) {
            data.plugins = parse_plugins(plugins, file);
        }
        data
    }

    fn parse_analyzer(&mut self, analyzer: &YamlNode) {
        if let Some(node) = analyzer.value_at(keys::CANNOT_IGNORE)
            && let Some(nodes) = node.as_list()
        {
            self.cannot_ignore = Some(
                nodes
                    .iter()
                    .filter_map(|n| n.string_value().map(str::to_string))
                    .collect(),
            );
        }
        if let Some(node) = analyzer.value_at(keys::ENABLE_EXPERIMENT)
            && let Some(nodes) = node.as_list()
        {
            self.enable_experiments = Some(
                nodes
                    .iter()
                    .filter_map(|n| n.string_value().map(str::to_string))
                    .collect(),
            );
        }
        if let Some(errors) = analyzer.value_at(keys::ERRORS)
            && let Some(entries) = errors.as_map()
        {
            let mut processors = Vec::new();
            for (key, value) in entries {
                let Some(code) = key.string_value() else {
                    continue;
                };
                let Some(value) = value.scalar() else {
                    continue;
                };
                let action = value.to_dart_string().to_lowercase();
                if keys::IGNORE_SYNONYMS.contains(&action.as_str()) {
                    processors.push(ErrorProcessor {
                        code: code.to_lowercase(),
                        severity: None,
                    });
                } else if let Some(severity) = DiagnosticSeverity::from_option(&action) {
                    processors.push(ErrorProcessor {
                        code: code.to_lowercase(),
                        severity: Some(severity),
                    });
                }
            }
            self.error_processors = Some(processors);
        }
        if let Some(exclude) = analyzer.value_at(keys::EXCLUDE)
            && let Some(nodes) = exclude.as_list()
        {
            self.excludes = Some(
                nodes
                    .iter()
                    .filter_map(|n| n.string_value().map(str::to_string))
                    .collect(),
            );
        }
        if let Some(language) = analyzer.value_at(keys::LANGUAGE) {
            for (key, value) in language.as_map().unwrap_or_default() {
                let parse_bool = |value: &YamlNode| {
                    if value.is_scalar() {
                        value.to_bool()
                    } else {
                        None
                    }
                };
                if key_is(key, keys::STRICT_CASTS) {
                    self.strict_casts = parse_bool(value);
                } else if key_is(key, keys::STRICT_INFERENCE) {
                    self.strict_inference = parse_bool(value);
                } else if key_is(key, keys::STRICT_RAW_TYPES) {
                    self.strict_raw_types = parse_bool(value);
                }
            }
        }
        self.legacy_plugins = analyzer.value_at(keys::PLUGINS).cloned();
        if let Some(checks) = analyzer.value_at(keys::OPTIONAL_CHECKS) {
            match &checks.kind {
                NodeKind::Scalar(Scalar::String(value)) => {
                    if value == keys::CHROME_OS_MANIFEST_CHECKS {
                        self.chrome_os_manifest_checks = Some(true);
                    } else if value == keys::PROPAGATE_LINTER_EXCEPTIONS {
                        self.propagate_linter_exceptions = Some(true);
                    }
                }
                NodeKind::Map(entries) => {
                    for (key, value) in entries {
                        let parsed = if value.is_scalar() {
                            value.to_bool()
                        } else {
                            None
                        };
                        if key_is(key, keys::CHROME_OS_MANIFEST_CHECKS) {
                            self.chrome_os_manifest_checks = parsed;
                        } else if key_is(key, keys::PROPAGATE_LINTER_EXCEPTIONS) {
                            self.propagate_linter_exceptions = parsed;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// `parseDiagnosticsSection` of `lint/config.dart`: a list of names, or a map
/// from names to `true`/`false`/severity, or a map of groups.
pub fn parse_diagnostics_section(value: &YamlNode) -> RuleConfigs {
    let mut result = RuleConfigs::new();
    match &value.kind {
        NodeKind::List(nodes) => {
            for node in nodes {
                if let Some(name) = node.string_value() {
                    put(
                        &mut result,
                        name.to_string(),
                        RuleConfig {
                            name: name.to_string(),
                            group: None,
                            severity: ConfiguredSeverity::Enable,
                        },
                    );
                }
            }
        }
        NodeKind::Map(entries) => {
            for (key, config_value) in entries {
                let Some(config_name) = key.string_value() else {
                    continue;
                };
                if let Some(config) = parse_rule_config(key, config_value, None) {
                    put(&mut result, config.name.clone(), config);
                    continue;
                }
                if let Some(group_entries) = config_value.as_map() {
                    for (rule_name, rule_value) in group_entries {
                        if let Some(config) =
                            parse_rule_config(rule_name, rule_value, Some(config_name))
                        {
                            put(&mut result, config.name.clone(), config);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    result
}

fn parse_rule_config(key: &YamlNode, value: &YamlNode, group: Option<&str>) -> Option<RuleConfig> {
    let name = key.string_value()?;
    let severity = match value.scalar()? {
        Scalar::Bool(true) => ConfiguredSeverity::Enable,
        Scalar::Bool(false) => ConfiguredSeverity::Disable,
        Scalar::String(severity) => {
            ConfiguredSeverity::from_name(severity).unwrap_or(ConfiguredSeverity::Enable)
        }
        _ => return None,
    };
    Some(RuleConfig {
        name: name.to_string(),
        group: group.map(str::to_string),
        severity,
    })
}

/// `_ParsedPluginsData.parse`.
fn parse_plugins(plugins: &YamlNode, file: &str) -> PluginsData {
    let Some(entries) = plugins.as_map() else {
        return PluginsData {
            clears_existing: true,
            ..Default::default()
        };
    };
    let mut data = PluginsData::default();
    for (name_node, plugin_node) in entries {
        let Some(name) = name_node.scalar() else {
            continue;
        };
        let plugin_name = name.to_dart_string();
        if plugin_name == keys::DEPENDENCY_OVERRIDES {
            if let Some(overrides) = plugin_node.as_map() {
                for (override_name, override_node) in overrides {
                    let Some(override_name) = override_name.scalar() else {
                        continue;
                    };
                    if let Some(source) = parse_plugin_source(override_node, file) {
                        let map = data.dependency_overrides.get_or_insert_with(Vec::new);
                        put(map, override_name.to_dart_string(), source);
                    }
                }
            }
            continue;
        }
        if let Some(source) = parse_plugin_source(plugin_node, file) {
            let diagnostic_configs = plugin_node
                .as_map()
                .and(plugin_node.value_at(keys::DIAGNOSTICS))
                .map(parse_diagnostics_section)
                .unwrap_or_default();
            data.configurations.push(PluginConfiguration {
                name: plugin_name,
                source,
                diagnostic_configs,
            });
        }
    }
    data
}

/// `_ParsedPluginsData._parsePluginSource`.
fn parse_plugin_source(node: &YamlNode, file: &str) -> Option<PluginSource> {
    if let Some(constraint) = node.string_value() {
        return Some(PluginSource::Versioned {
            constraint: constraint.to_string(),
            hosted_url: None,
        });
    }
    node.as_map()?;
    let version = node
        .value_at(keys::VERSION)
        .and_then(YamlNode::string_value);
    let hosted = node.value_at(keys::HOSTED).and_then(YamlNode::string_value);
    if let Some(version) = version {
        return Some(PluginSource::Versioned {
            constraint: version.to_string(),
            hosted_url: hosted.map(str::to_string),
        });
    }
    if let Some(git) = node.value_at(keys::GIT) {
        if let Some(url) = git.string_value() {
            return Some(PluginSource::Git {
                url: url.to_string(),
                path: None,
                git_ref: None,
                tag_pattern: None,
            });
        }
        if git.as_map().is_some()
            && let Some(url) = git.value_at(keys::URL).and_then(YamlNode::string_value)
        {
            let text = |key: &str| {
                git.value_at(key)
                    .and_then(YamlNode::string_value)
                    .map(str::to_string)
            };
            return Some(PluginSource::Git {
                url: url.to_string(),
                path: text(keys::PATH),
                git_ref: text(keys::REF),
                tag_pattern: text(keys::TAG_PATTERN),
            });
        }
    }
    if let Some(path) = node.value_at(keys::PATH).and_then(YamlNode::string_value) {
        let path = if paths::is_absolute(path) {
            path.to_string()
        } else {
            paths::normalize(&paths::join(paths::dirname(file), path))
        };
        return Some(PluginSource::Path { path });
    }
    None
}

/// The first string in an `analyzer: plugins:` node
/// (`_ParsedLegacyPluginsData.firstPluginName`).
fn first_legacy_plugin_name(node: &YamlNode) -> Option<String> {
    match &node.kind {
        NodeKind::Scalar(_) => node.string_value().map(str::to_string),
        NodeKind::List(nodes) => nodes
            .iter()
            .find_map(|n| n.string_value().map(str::to_string)),
        NodeKind::Map(entries) => entries
            .iter()
            .find_map(|(k, _)| k.string_value().map(str::to_string)),
    }
}

/// Applies the parsed files of an include graph (`_ApplyState`).
struct ApplyState {
    options: AnalysisOptions,
    linter_rule_configs: RuleConfigs,
    has_linter_section: bool,
    legacy_plugins_node: Option<YamlNode>,
}

impl ApplyState {
    fn new(file: &str) -> Self {
        ApplyState {
            options: AnalysisOptions::with_file(Some(file.to_string())),
            linter_rule_configs: RuleConfigs::new(),
            has_linter_section: false,
            legacy_plugins_node: None,
        }
    }

    fn apply(&mut self, data: &ParsedFileData) {
        let options = &mut self.options;
        // _applyAnalyzer
        if let Some(flags) = &data.enable_experiments {
            options.enable_experiment_flags = Some(flags.clone());
        }
        if let Some(processors) = &data.error_processors {
            for processor in processors {
                match options
                    .error_processors
                    .iter_mut()
                    .find(|p| p.code == processor.code)
                {
                    Some(existing) => *existing = processor.clone(),
                    None => options.error_processors.push(processor.clone()),
                }
            }
        }
        if let Some(names) = &data.cannot_ignore {
            for name in names {
                if let Some(severity) = DiagnosticSeverity::from_option(name) {
                    options.unignorable_severities.push(UnignorableSeverity {
                        severity,
                        error_processors: options.error_processors.clone(),
                    });
                } else {
                    let name = name.to_lowercase();
                    if !options.unignorable_names.contains(&name) {
                        options.unignorable_names.push(name);
                    }
                }
            }
        }
        if let Some(patterns) = &data.excludes {
            for pattern in patterns {
                if !options.exclude_patterns.contains(pattern) {
                    options.exclude_patterns.push(pattern.clone());
                }
            }
        }
        if let Some(value) = data.strict_casts {
            options.strict_casts = value;
        }
        if let Some(value) = data.strict_inference {
            options.strict_inference = value;
        }
        if let Some(value) = data.strict_raw_types {
            options.strict_raw_types = value;
        }
        if let Some(value) = data.chrome_os_manifest_checks {
            options.chrome_os_manifest_checks = value;
        }
        if let Some(value) = data.propagate_linter_exceptions {
            options.propagate_linter_exceptions = value;
        }
        // _applyCodeStyle
        if let Some(value) = data.use_formatter {
            options.code_style_use_formatter = value;
        }
        // _applyFormatter
        if data.page_width.is_some() {
            options.formatter_page_width = data.page_width;
        }
        if data.trailing_commas.is_some() {
            options.formatter_trailing_commas = data.trailing_commas;
        }
        // _applyPlugins
        if data.plugins.clears_existing {
            options.plugins = PluginsOptions::default();
        } else {
            for configuration in &data.plugins.configurations {
                match options
                    .plugins
                    .configurations
                    .iter_mut()
                    .find(|c| c.name == configuration.name)
                {
                    Some(existing) => *existing = configuration.clone(),
                    None => options.plugins.configurations.push(configuration.clone()),
                }
            }
            if let Some(overrides) = &data.plugins.dependency_overrides {
                let map = options
                    .plugins
                    .dependency_overrides
                    .get_or_insert_with(Vec::new);
                for (name, source) in overrides {
                    put(map, name.clone(), source.clone());
                }
            }
        }
        // _applyLegacyPlugins
        if let Some(local) = &data.legacy_plugins {
            let merged = match &self.legacy_plugins_node {
                None => local.clone(),
                Some(existing) => yaml::merge(existing, local),
            };
            options.enabled_legacy_plugin_names =
                first_legacy_plugin_name(&merged).into_iter().collect();
            self.legacy_plugins_node = Some(merged);
        }
        // _applyLinterSection
        match &data.linter {
            LinterData::Absent => {}
            LinterData::Invalid => {
                self.has_linter_section = false;
                self.linter_rule_configs.clear();
            }
            LinterData::Valid(configs) => {
                self.has_linter_section = true;
                for (name, config) in configs {
                    put(&mut self.linter_rule_configs, name.clone(), config.clone());
                }
            }
        }
    }

    fn build(mut self) -> AnalysisOptions {
        if self.has_linter_section {
            // `Registry.ruleRegistry.enabled`: lint rules that are explicitly
            // enabled. The linter has no warning rules.
            let rules: Vec<String> = lint_rules::LINT_RULE_NAMES
                .iter()
                .filter(|name| {
                    self.linter_rule_configs
                        .iter()
                        .any(|(n, c)| n == *name && c.is_enabled())
                })
                .map(|name| name.to_string())
                .collect();
            if !rules.is_empty() {
                self.options.lint = true;
                self.options.lint_rules = rules;
            }
        }
        self.options
    }
}
