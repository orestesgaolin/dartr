// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/yaml_completion_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/producer.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/analysis_options_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/fix_data_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/pubspec_generator.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/yaml.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_completion.dart (_getServerYamlItems)
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (toCompletionItem)

//! Code completion for YAML files (`pubspec.yaml`, `analysis_options.yaml`,
//! and `fix_data.yaml` / `fix_data/**.yaml`), shared between the LSP server
//! and the legacy protocol server.

use dartr_diagnostics::{Origin, all_codes};
use dartr_lints::{ALL_RULES, RuleStateType};
use dartr_project::experiments::KNOWN_FEATURES;
use dartr_syntax::LineInfo;
use dartr_yaml::{
    NodeKind, Scalar, ScalarStyle, SourceFile, YamlNode, load_yaml_node_with_options,
};
use indexmap::IndexSet;
use serde_json::{Map, Value, json};

use super::candidate::utf16_len;
use super::lsp::{ItemCapabilities, build_insert_text_for_yaml};
use super::relevance::MAXIMUM_RELEVANCE_SORT;
use crate::fuzzy::FuzzyMatcher;
use crate::mapping::to_range;

/// The kind of a YAML file supported by completion (`file_paths.dart`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YamlFileKind {
    AnalysisOptions,
    FixData,
    Pubspec,
}

impl YamlFileKind {
    /// Returns the [`YamlFileKind`] of `path` if it is `analysis_options.yaml`,
    /// `fix_data.yaml` (or `.../fix_data/**.yaml`), or `pubspec.yaml`.
    pub fn from_path(path: &str) -> Option<Self> {
        let base = dartr_project::paths::basename(path);
        if base == "analysis_options.yaml" {
            Some(Self::AnalysisOptions)
        } else if base == "fix_data.yaml"
            || (path.ends_with(".yaml") && path.split(['/', '\\']).any(|c| c == "fix_data"))
        {
            Some(Self::FixData)
        } else if base == "pubspec.yaml" {
            Some(Self::Pubspec)
        } else {
            None
        }
    }
}

/// The kind of a YAML completion suggestion (`CompletionSuggestionKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YamlSuggestionKind {
    Identifier,
    PackageName,
}

/// A YAML completion suggestion (`CompletionSuggestion` produced by `Producer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YamlSuggestion {
    pub kind: YamlSuggestionKind,
    pub relevance: i64,
    pub completion: String,
    pub display_text: Option<String>,
    pub selection_offset: usize,
    pub selection_length: usize,
    pub is_deprecated: bool,
    pub is_potential: bool,
    pub doc_summary: Option<String>,
    pub doc_complete: Option<String>,
}

impl YamlSuggestion {
    /// Dart `Producer.identifier`.
    pub fn identifier(identifier: &str, relevance: i64, doc_complete: Option<String>) -> Self {
        Self {
            kind: YamlSuggestionKind::Identifier,
            relevance,
            completion: identifier.to_string(),
            display_text: None,
            selection_offset: utf16_len(identifier),
            selection_length: 0,
            is_deprecated: false,
            is_potential: false,
            doc_summary: None,
            doc_complete,
        }
    }

    /// Dart `Producer.packageName`.
    pub fn package_name(package_name: &str, relevance: i64) -> Self {
        Self {
            kind: YamlSuggestionKind::PackageName,
            relevance,
            completion: package_name.to_string(),
            display_text: None,
            selection_offset: utf16_len(package_name),
            selection_length: 0,
            is_deprecated: false,
            is_potential: false,
            doc_summary: None,
            doc_complete: None,
        }
    }
}

/// Dart `YamlCompletionResults`.
#[derive(Clone, Debug, Default)]
pub struct YamlCompletionResults {
    pub suggestions: Vec<YamlSuggestion>,
    pub target_prefix: String,
    pub replacement_offset: usize,
    pub replacement_length: usize,
}

impl YamlCompletionResults {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Dart `YamlCompletionRequest`.
#[derive(Clone, Debug)]
struct YamlCompletionRequest<'a> {
    file_path: &'a str,
    preceding_text: String,
}

/// Dart `Producer` hierarchy in `pkg/analysis_server/lib/src/services/completion/yaml/`.
#[derive(Clone)]
enum Producer {
    Empty,
    Boolean,
    Enum(&'static [&'static str]),
    FilePath,
    Experiment,
    LintRule,
    Error,
    Plugins,
    PubPackageName,
    PubPackageVersion,
    MapKey(Box<Producer>),
    List(Box<Producer>),
    ListOrMap {
        element: Box<Producer>,
        map_value: Box<Producer>,
    },
    Map(Vec<(&'static str, Producer)>),
}

impl Producer {
    /// Whether this producer is a `ListProducer` in Dart (`ListProducer` or
    /// `ListOrMapProducer`, which extends `ListProducer`).
    fn is_list_producer(&self) -> bool {
        matches!(self, Producer::List(_) | Producer::ListOrMap { .. })
    }

    /// Whether this producer implements `KeyValueProducer` in Dart.
    fn is_key_value_producer(&self) -> bool {
        matches!(
            self,
            Producer::Map(_)
                | Producer::ListOrMap { .. }
                | Producer::Error
                | Producer::Plugins
                | Producer::PubPackageName
        )
    }

    /// Dart `KeyValueProducer.producerForKey`.
    fn producer_for_key(&self, key: &str) -> Option<Producer> {
        match self {
            Producer::Map(children) => children
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone()),
            Producer::ListOrMap { map_value, .. } => Some((**map_value).clone()),
            Producer::Error => Some(Producer::Enum(&["ignore", "info", "warning", "error"])),
            Producer::Plugins => Some(plugins_value_producer()),
            Producer::PubPackageName => Some(Producer::PubPackageVersion),
            _ => None,
        }
    }

    /// If `self` is a `ListProducer` (or `ListOrMapProducer`), returns its
    /// `element` producer.
    fn list_element(&self) -> Option<Producer> {
        match self {
            Producer::List(element) | Producer::ListOrMap { element, .. } => {
                Some((**element).clone())
            }
            _ => None,
        }
    }

    /// Dart `Producer.suggestions`.
    fn suggestions(&self, request: &YamlCompletionRequest<'_>) -> Vec<YamlSuggestion> {
        match self {
            Producer::Empty
            | Producer::Plugins
            | Producer::PubPackageName
            | Producer::PubPackageVersion => Vec::new(),
            Producer::Boolean => vec![
                YamlSuggestion::identifier("true", 1000, None),
                YamlSuggestion::identifier("false", 1000, None),
            ],
            Producer::Enum(values) => values
                .iter()
                .map(|v| YamlSuggestion::identifier(v, 1000, None))
                .collect(),
            Producer::FilePath => file_path_suggestions(request.file_path, &request.preceding_text),
            Producer::Experiment => KNOWN_FEATURES
                .iter()
                .filter(|f| !f.is_enabled_by_default)
                .map(|f| YamlSuggestion::identifier(f.enable_string, 1000, None))
                .collect(),
            Producer::LintRule => ALL_RULES
                .iter()
                .filter(|r| include_rule_in_completions(r.state.kind))
                .map(|r| YamlSuggestion::identifier(r.name, 1000, Some(r.description.to_string())))
                .collect(),
            Producer::Error => error_suggestions(),
            Producer::MapKey(element) => element
                .suggestions(request)
                .into_iter()
                .map(|s| {
                    YamlSuggestion::identifier(
                        &format!("{}: ", s.completion),
                        s.relevance,
                        s.doc_complete,
                    )
                })
                .collect(),
            Producer::List(element) => element
                .suggestions(request)
                .into_iter()
                .map(|s| {
                    YamlSuggestion::identifier(
                        &format!("- {}", s.completion),
                        s.relevance,
                        s.doc_complete,
                    )
                })
                .collect(),
            Producer::ListOrMap { element, .. } => {
                let mut out = Producer::List(element.clone()).suggestions(request);
                out.extend(Producer::MapKey(element.clone()).suggestions(request));
                out
            }
            Producer::Map(children) => children
                .iter()
                .map(|(k, v)| {
                    if v.is_list_producer() {
                        YamlSuggestion::identifier(&format!("{k}:"), 1000, None)
                    } else {
                        YamlSuggestion::identifier(&format!("{k}: "), 1000, None)
                    }
                })
                .collect(),
        }
    }
}

/// Dart `AbstractAnalysisRule.includeInCompletions`.
fn include_rule_in_completions(kind: RuleStateType) -> bool {
    !matches!(
        kind,
        RuleStateType::Internal | RuleStateType::Removed | RuleStateType::Testing
    )
}

/// Dart `_ErrorProducer.suggestions`.
fn error_suggestions() -> Vec<YamlSuggestion> {
    // `diagnosticCodeValues` in `diagnostic_code_values.g.dart` contains all
    // active messages in `AnalyzerDiagnosticPackage.analyzer` (`FeAnalyzerShared`
    // and `Analyzer`), sorted by `camelCaseName`.
    let mut analyzer_codes: Vec<_> = all_codes()
        .iter()
        .copied()
        .filter(|c| matches!(c.origin, Origin::FeAnalyzerShared | Origin::Analyzer))
        .collect();
    analyzer_codes.sort_by(|a, b| a.camel_case_name.cmp(b.camel_case_name));

    let mut names: IndexSet<&'static str> = IndexSet::new();
    for diagnostic in analyzer_codes {
        names.insert(diagnostic.lower_case_name());
    }
    for rule in ALL_RULES {
        if include_rule_in_completions(rule.state.kind) {
            names.insert(rule.name);
        }
    }
    names
        .into_iter()
        .map(|name| YamlSuggestion::identifier(&format!("{name}: "), 1000, None))
        .collect()
}

/// Dart `_PluginsProducer.producerForKey`.
fn plugins_value_producer() -> Producer {
    Producer::Map(vec![
        ("diagnostics", Producer::Empty),
        (
            "git",
            Producer::Map(vec![
                ("url", Producer::Empty),
                ("ref", Producer::Empty),
                ("path", Producer::Empty),
                ("tag_pattern", Producer::Empty),
            ]),
        ),
        ("path", Producer::Empty),
        ("version", Producer::Empty),
        ("hosted", Producer::Empty),
    ])
}

/// Dart `AnalysisOptionsGenerator.analysisOptionsProducer`.
fn analysis_options_producer() -> Producer {
    Producer::Map(vec![
        (
            "analyzer",
            Producer::Map(vec![
                (
                    "enable-experiment",
                    Producer::List(Box::new(Producer::Experiment)),
                ),
                ("errors", Producer::Error),
                ("exclude", Producer::Empty),
                (
                    "language",
                    Producer::Map(vec![
                        ("strict-casts", Producer::Boolean),
                        ("strict-inference", Producer::Boolean),
                        ("strict-raw-types", Producer::Boolean),
                    ]),
                ),
                (
                    "optional-checks",
                    Producer::Map(vec![("chrome-os-manifest-checks", Producer::Empty)]),
                ),
                ("plugins", Producer::Empty),
                ("propagate-linter-exceptions", Producer::Empty),
            ]),
        ),
        (
            "code-style",
            Producer::Map(vec![("format", Producer::Boolean)]),
        ),
        (
            "formatter",
            Producer::Map(vec![
                ("page_width", Producer::Empty),
                ("trailing_commas", Producer::Enum(&["automate", "preserve"])),
            ]),
        ),
        ("include", Producer::Empty),
        (
            "linter",
            Producer::Map(vec![(
                "rules",
                Producer::ListOrMap {
                    element: Box::new(Producer::LintRule),
                    map_value: Box::new(Producer::Enum(&[
                        "true", "false", "ignore", "error", "info", "warning",
                    ])),
                },
            )]),
        ),
        ("plugins", Producer::Plugins),
    ])
}

/// Dart `FixDataGenerator.fixDataProducer`.
fn fix_data_producer() -> Producer {
    let element_producer = Producer::Map(vec![
        ("uris", Producer::Empty),
        ("class", Producer::Empty),
        ("constant", Producer::Empty),
        ("constructor", Producer::Empty),
        ("enum", Producer::Empty),
        ("extension", Producer::Empty),
        ("extensionType", Producer::Empty),
        ("field", Producer::Empty),
        ("function", Producer::Empty),
        ("getter", Producer::Empty),
        ("method", Producer::Empty),
        ("mixin", Producer::Empty),
        ("setter", Producer::Empty),
        ("typedef", Producer::Empty),
        ("variable", Producer::Empty),
        ("inClass", Producer::Empty),
        ("inEnum", Producer::Empty),
        ("inExtension", Producer::Empty),
        ("inExtensionType", Producer::Empty),
        ("inMixin", Producer::Empty),
    ]);
    let changes_producer = Producer::List(Box::new(Producer::Map(vec![
        (
            "kind",
            Producer::Enum(&[
                "addParameter",
                "addTypeParameter",
                "removeParameter",
                "rename",
                "renameParameter",
                "replacedBy",
            ]),
        ),
        ("index", Producer::Empty),
        ("name", Producer::Empty),
        ("style", Producer::Empty),
        (
            "argumentValue",
            Producer::Map(vec![
                ("expression", Producer::Empty),
                ("requiredIf", Producer::Empty),
                ("variables", Producer::Empty),
            ]),
        ),
        ("extends", Producer::Empty),
        ("oldName", Producer::Empty),
        ("newName", Producer::Empty),
        ("newElement", element_producer.clone()),
    ])));
    Producer::Map(vec![
        ("version", Producer::Empty),
        (
            "transforms",
            Producer::List(Box::new(Producer::Map(vec![
                ("title", Producer::Empty),
                ("date", Producer::Empty),
                ("bulkApply", Producer::Boolean),
                ("element", element_producer),
                ("changes", changes_producer.clone()),
                (
                    "oneOf",
                    Producer::List(Box::new(Producer::Map(vec![
                        ("if", Producer::Empty),
                        ("changes", changes_producer),
                    ]))),
                ),
                ("variables", Producer::Empty),
            ]))),
        ),
    ])
}

/// Dart `PubspecGenerator.pubspecProducer`.
fn pubspec_producer() -> Producer {
    Producer::Map(vec![
        ("name", Producer::Empty),
        ("version", Producer::Empty),
        ("description", Producer::Empty),
        ("homepage", Producer::Empty),
        ("repository", Producer::Empty),
        ("issue_tracker", Producer::Empty),
        ("documentation", Producer::Empty),
        ("executables", Producer::Empty),
        ("publish_to", Producer::Empty),
        (
            "environment",
            Producer::Map(vec![("flutter", Producer::Empty), ("sdk", Producer::Empty)]),
        ),
        ("workspace", Producer::List(Box::new(Producer::FilePath))),
        (
            "resolution",
            Producer::Enum(&["external", "local", "workspace"]),
        ),
        ("dependencies", Producer::PubPackageName),
        ("dev_dependencies", Producer::PubPackageName),
        ("dependency_overrides", Producer::Empty),
        (
            "flutter",
            Producer::Map(vec![
                ("assets", Producer::List(Box::new(Producer::FilePath))),
                (
                    "fonts",
                    Producer::List(Box::new(Producer::Map(vec![
                        ("family", Producer::Empty),
                        (
                            "fonts",
                            Producer::List(Box::new(Producer::Map(vec![
                                ("asset", Producer::Empty),
                                ("style", Producer::Enum(&["italic", "normal"])),
                                (
                                    "weight",
                                    Producer::Enum(&[
                                        "100", "200", "300", "400", "500", "600", "700", "800",
                                        "900",
                                    ]),
                                ),
                            ]))),
                        ),
                    ]))),
                ),
                ("generate", Producer::Boolean),
                (
                    "module",
                    Producer::Map(vec![
                        ("androidX", Producer::Boolean),
                        ("androidPackage", Producer::Empty),
                        ("iosBundleIdentifier", Producer::Empty),
                    ]),
                ),
                (
                    "plugin",
                    Producer::Map(vec![(
                        "platforms",
                        Producer::Map(vec![
                            (
                                "android",
                                Producer::Map(vec![
                                    ("package", Producer::Empty),
                                    ("pluginClass", Producer::Empty),
                                ]),
                            ),
                            ("ios", Producer::Map(vec![("pluginClass", Producer::Empty)])),
                            (
                                "linux",
                                Producer::Map(vec![
                                    ("dartPluginClass", Producer::Empty),
                                    ("pluginClass", Producer::Empty),
                                ]),
                            ),
                            (
                                "macos",
                                Producer::Map(vec![
                                    ("dartPluginClass", Producer::Empty),
                                    ("pluginClass", Producer::Empty),
                                ]),
                            ),
                            (
                                "web",
                                Producer::Map(vec![
                                    ("fileName", Producer::Empty),
                                    ("pluginClass", Producer::Empty),
                                ]),
                            ),
                            (
                                "windows",
                                Producer::Map(vec![
                                    ("dartPluginClass", Producer::Empty),
                                    ("pluginClass", Producer::Empty),
                                ]),
                            ),
                        ]),
                    )]),
                ),
                ("uses-material-design", Producer::Boolean),
            ]),
        ),
        (
            "screenshots",
            Producer::List(Box::new(Producer::Map(vec![
                ("description", Producer::Empty),
                ("path", Producer::FilePath),
            ]))),
        ),
        ("topics", Producer::Empty),
    ])
}

/// Dart `FilePathProducer.suggestions`.
fn file_path_suggestions(file_path: &str, preceding_text: &str) -> Vec<YamlSuggestion> {
    let separator = '/';
    let mut parent_directory = if preceding_text.is_empty() || preceding_text.ends_with(separator) {
        preceding_text.to_string()
    } else {
        posix_dirname(preceding_text)
    };
    if parent_directory == "." {
        parent_directory.clear();
    } else if let Some(stripped) = parent_directory.strip_suffix(separator) {
        parent_directory = stripped.to_string();
    }
    let parts = posix_split(&parent_directory);
    parent_directory = if parts.first() == Some(&"/") {
        format!("/{}", parts[1..].join("/"))
    } else {
        parts.join("/")
    };
    if !parent_directory.starts_with('/') {
        let file_dir = dartr_project::paths::dirname(file_path);
        parent_directory = if parent_directory.is_empty() {
            file_dir.to_string()
        } else {
            dartr_project::paths::join(file_dir, &parent_directory)
        };
    }
    parent_directory = dartr_project::paths::normalize(&parent_directory);
    if dartr_project::fs::folder_exists(&parent_directory)
        && let Some(children) = dartr_project::fs::children(&parent_directory)
    {
        let mut list = Vec::new();
        for child in children {
            let name = dartr_project::paths::basename(&child.path);
            let relevance = if name.starts_with('.') { 500 } else { 1000 };
            list.push(YamlSuggestion::identifier(&name, relevance, None));
        }
        return list;
    }
    Vec::new()
}

fn posix_dirname(p: &str) -> String {
    let trimmed = p.trim_end_matches('/');
    if trimmed.is_empty() {
        return if p.starts_with('/') {
            "/".to_string()
        } else {
            ".".to_string()
        };
    }
    match trimmed.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(idx) => {
            let prefix = trimmed[..idx].trim_end_matches('/');
            if prefix.is_empty() {
                "/".to_string()
            } else {
                prefix.to_string()
            }
        }
    }
}

fn posix_split(p: &str) -> Vec<&str> {
    if p.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if p.starts_with('/') {
        out.push("/");
    }
    for part in p.split('/') {
        if !part.is_empty() {
            out.push(part);
        }
    }
    out
}

/// Computes YAML completion suggestions for `file_path` at `offset` (UTF-16
/// code units), reading `file_path` via `dartr_project::fs::read_string_strict`.
pub fn compute_yaml_suggestions(file_path: &str, offset: usize) -> YamlCompletionResults {
    let Some(kind) = YamlFileKind::from_path(file_path) else {
        return YamlCompletionResults::empty();
    };
    let Some(content) = dartr_project::fs::read_string_strict(file_path) else {
        return YamlCompletionResults::empty();
    };
    compute_yaml_suggestions_with_content(kind, file_path, &content, offset)
}

/// Dart `YamlCompletionGenerator.getSuggestions` on already-loaded `content`.
pub fn compute_yaml_suggestions_with_content(
    kind: YamlFileKind,
    file_path: &str,
    content: &str,
    offset: usize,
) -> YamlCompletionResults {
    let top_level_producer = match kind {
        YamlFileKind::AnalysisOptions => analysis_options_producer(),
        YamlFileKind::FixData => fix_data_producer(),
        YamlFileKind::Pubspec => pubspec_producer(),
    };
    let Some(root) = load_yaml_node_with_options(content, true).node else {
        return YamlCompletionResults::empty();
    };
    let source = SourceFile::new(content);
    let cursor_column = column_at(&source, offset);
    let node_path = path_to_offset(&root, offset, cursor_column);
    let Some(&completion_node) = node_path.last() else {
        return YamlCompletionResults::empty();
    };
    let mut preceding_text = String::new();
    if let NodeKind::Scalar(scalar) = &completion_node.kind {
        match scalar {
            Scalar::String(value) => {
                let start = completion_node.span.start.offset;
                let value_utf16: Vec<u16> = value.encode_utf16().collect();
                if completion_node.scalar_style == ScalarStyle::Plain
                    && offset >= start
                    && offset - start <= value_utf16.len()
                {
                    preceding_text = String::from_utf16_lossy(&value_utf16[..offset - start]);
                } else {
                    return YamlCompletionResults::empty();
                }
            }
            Scalar::Null => {}
            _ => {
                return YamlCompletionResults::empty();
            }
        }
    }
    let request = YamlCompletionRequest {
        file_path,
        preceding_text,
    };
    get_suggestions_for_path(&top_level_producer, &source, &request, &node_path, offset)
}

/// Dart `YamlCompletionGenerator.getSuggestionsForPath`.
fn get_suggestions_for_path(
    top_level_producer: &Producer,
    source: &SourceFile,
    request: &YamlCompletionRequest<'_>,
    node_path: &[&YamlNode],
    offset: usize,
) -> YamlCompletionResults {
    let Some(mut producer) = producer_for_path(top_level_producer, node_path) else {
        return YamlCompletionResults::empty();
    };
    if let Producer::ListOrMap { element, .. } = &producer {
        producer = resolve_list_or_map_producer(&producer, element, node_path);
    }
    let invalid_suggestions = siblings_on_path(node_path);
    let mut suggestions = Vec::new();
    for suggestion in producer.suggestions(request) {
        if !invalid_suggestions.contains(&suggestion.completion) {
            suggestions.push(suggestion);
        }
    }
    let node = node_path.last().copied();
    let (target_prefix, replacement_offset, replacement_length) = match node {
        Some(n) if matches!(n.kind, NodeKind::Scalar(_)) && contains_offset(n, offset) => {
            let start = n.span.start.offset;
            let prefix = source.get_text(start, offset.min(n.span.end.offset));
            (prefix, start, n.span.length())
        }
        _ => (String::new(), offset, 0),
    };
    YamlCompletionResults {
        suggestions,
        target_prefix,
        replacement_offset,
        replacement_length,
    }
}

/// Dart `YamlCompletionGenerator._columnAt`.
fn column_at(source: &SourceFile, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let text = source.get_text(0, offset.min(source.length()));
    let units: Vec<u16> = text.encode_utf16().collect();
    let limit = offset.min(units.len());
    for i in (0..limit).rev() {
        if units[i] == b'\n' as u16 {
            return limit - i - 1;
        }
    }
    limit
}

/// Dart `_columnAllowed`.
fn column_allowed(cursor_column: usize, node: &YamlNode) -> bool {
    cursor_column >= node.span.start.column
}

/// Dart `YamlNodeExtensions.containsOffset`.
fn contains_offset(node: &YamlNode, offset: usize) -> bool {
    let node_offset = node.span.start.offset;
    let node_end = node_offset + node.span.length();
    node_offset <= offset && offset <= node_end
}

/// Dart `YamlNodeExtensions.childContainingOffset`.
fn child_containing_offset(
    node: &YamlNode,
    offset: usize,
    cursor_column: usize,
) -> Option<&YamlNode> {
    match &node.kind {
        NodeKind::List(elements) => {
            for element in elements {
                if contains_offset(element, offset) && column_allowed(cursor_column, element) {
                    return Some(element);
                }
            }
            for element in elements {
                if matches!(element.kind, NodeKind::Scalar(Scalar::Null)) {
                    return Some(element);
                }
            }
            None
        }
        NodeKind::Map(entries) => {
            for (i, (key, value)) in entries.iter().enumerate() {
                let next_entry_offset = entries.get(i + 1).map(|(nk, _)| nk.span.start.offset);
                if contains_offset(key, offset) {
                    return Some(key);
                }
                let cursor_in_gap = key.span.end.offset <= offset
                    && offset < value.span.start.offset
                    && next_entry_offset.is_none_or(|ne| offset < ne);
                let value_non_empty_collection = match &value.kind {
                    NodeKind::List(items) => !items.is_empty(),
                    NodeKind::Map(map_entries) => !map_entries.is_empty(),
                    _ => false,
                };
                if (contains_offset(value, offset) && column_allowed(cursor_column, value))
                    || (matches!(value.kind, NodeKind::Scalar(Scalar::Null))
                        && next_entry_offset.is_none_or(|ne| offset < ne))
                    || (cursor_in_gap
                        && column_allowed(cursor_column, value)
                        && value_non_empty_collection)
                {
                    return Some(value);
                }
            }
            None
        }
        NodeKind::Scalar(_) => None,
    }
}

/// Dart `YamlCompletionGenerator._pathToOffset`.
fn path_to_offset(root: &YamlNode, offset: usize, cursor_column: usize) -> Vec<&YamlNode> {
    let mut path = Vec::new();
    let mut cur = Some(root);
    while let Some(node) = cur {
        path.push(node);
        cur = child_containing_offset(node, offset, cursor_column);
    }
    path
}

/// Dart `YamlCompletionGenerator._producerForPath`.
fn producer_for_path(top_level_producer: &Producer, path: &[&YamlNode]) -> Option<Producer> {
    let mut producer = top_level_producer.clone();
    for i in 0..path.len().saturating_sub(1) {
        let node = path[i];
        let next = path[i + 1];
        if let NodeKind::Map(entries) = &node.kind
            && producer.is_key_value_producer()
        {
            if let Some((key, _)) = entries.iter().find(|(_, v)| std::ptr::eq(v, next))
                && matches!(key.kind, NodeKind::Scalar(_))
            {
                let NodeKind::Scalar(Scalar::String(key_str)) = &key.kind else {
                    return None;
                };
                producer = producer.producer_for_key(key_str)?;
            } else if entries
                .iter()
                .any(|(k, _)| std::ptr::eq(k, next) || k.key_equals(next))
            {
                return Some(producer);
            } else {
                return None;
            }
        } else if matches!(node.kind, NodeKind::List(_)) && producer.is_list_producer() {
            producer = producer.list_element()?;
        } else {
            return Some(producer);
        }
    }
    Some(producer)
}

/// Dart `YamlCompletionGenerator._resolveListOrMapProducer`.
fn resolve_list_or_map_producer(
    producer: &Producer,
    element: &Producer,
    path: &[&YamlNode],
) -> Producer {
    let Some(&node) = path.last() else {
        return producer.clone();
    };
    if matches!(node.kind, NodeKind::Map(_)) {
        return Producer::MapKey(Box::new(element.clone()));
    }
    if matches!(node.kind, NodeKind::List(_)) {
        return Producer::List(Box::new(element.clone()));
    }
    if path.len() >= 2 {
        let parent = path[path.len() - 2];
        if let NodeKind::Map(entries) = &parent.kind
            && entries
                .iter()
                .any(|(k, _)| std::ptr::eq(k, node) || k.key_equals(node))
        {
            return Producer::MapKey(Box::new(element.clone()));
        }
    }
    producer.clone()
}

/// Dart `YamlCompletionGenerator._siblingsOnPath`.
fn siblings_on_path(path: &[&YamlNode]) -> Vec<String> {
    fn siblings_in_list(items: &[YamlNode], current: Option<&YamlNode>) -> Vec<String> {
        let mut siblings = Vec::new();
        for element in items {
            if !current.is_some_and(|c| std::ptr::eq(element, c))
                && let NodeKind::Scalar(Scalar::String(value)) = &element.kind
            {
                siblings.push(value.clone());
                siblings.push(format!("- {value}"));
            }
        }
        siblings
    }

    fn siblings_in_map(
        entries: &[(YamlNode, YamlNode)],
        current: Option<&YamlNode>,
    ) -> Vec<String> {
        let mut siblings = Vec::new();
        for (key, value) in entries {
            if !current.is_some_and(|c| std::ptr::eq(key, c))
                && let NodeKind::Scalar(Scalar::String(key_str)) = &key.kind
            {
                let suffix = if matches!(value.kind, NodeKind::List(_)) {
                    ":"
                } else {
                    ": "
                };
                siblings.push(format!("{key_str}{suffix}"));
            }
        }
        siblings
    }

    let length = path.len();
    if length < 2 {
        if length == 1
            && let NodeKind::Map(entries) = &path[0].kind
        {
            return siblings_in_map(entries, None);
        }
        return Vec::new();
    }
    let node = path[length - 1];
    match &node.kind {
        NodeKind::List(items) => return siblings_in_list(items, None),
        NodeKind::Map(entries) => return siblings_in_map(entries, None),
        NodeKind::Scalar(_) => {}
    }
    let parent = path[length - 2];
    match &parent.kind {
        NodeKind::List(items) => siblings_in_list(items, Some(node)),
        NodeKind::Map(entries) => siblings_in_map(entries, Some(node)),
        NodeKind::Scalar(_) => Vec::new(),
    }
}

/// Dart `CompletionHandler._getServerYamlItems`.
pub fn yaml_completion_items(
    kind: YamlFileKind,
    file_path: &str,
    content: &str,
    line_info: &LineInfo,
    offset: usize,
    caps: &ItemCapabilities,
    preferred_documentation: &str,
) -> Vec<Value> {
    let suggestions = compute_yaml_suggestions_with_content(kind, file_path, content, offset);
    let insert_length = offset
        .saturating_sub(suggestions.replacement_offset)
        .min(suggestions.replacement_length);
    let replacement_range = to_range(
        line_info,
        suggestions.replacement_offset as u32,
        suggestions.replacement_length as u32,
    );
    let insertion_range = to_range(
        line_info,
        suggestions.replacement_offset as u32,
        insert_length as u32,
    );

    let mut fuzzy_matcher = FuzzyMatcher::new(&suggestions.target_prefix);
    suggestions
        .suggestions
        .into_iter()
        .filter(|item| {
            let text = item.display_text.as_deref().unwrap_or(&item.completion);
            fuzzy_matcher.score(text) > 0.0
        })
        .filter_map(|item| {
            yaml_suggestion_to_lsp_item(
                caps,
                &item,
                &replacement_range,
                &insertion_range,
                preferred_documentation,
            )
        })
        .collect()
}

/// Dart `toCompletionItem` (`pkg/analysis_server/lib/src/lsp/mapping.dart`)
/// for a `YamlSuggestion`.
fn yaml_suggestion_to_lsp_item(
    caps: &ItemCapabilities,
    item: &YamlSuggestion,
    replacement_range: &Value,
    insertion_range: &Value,
    preferred_documentation: &str,
) -> Option<Value> {
    let resolution_data = if item.kind == YamlSuggestionKind::PackageName {
        let pkg = item.completion.split(':').next().unwrap_or("");
        Some(json!({"packageName": pkg}))
    } else {
        None
    };
    let include_documentation = if resolution_data.is_some() {
        "none"
    } else {
        preferred_documentation
    };

    let mut label = item
        .display_text
        .clone()
        .unwrap_or_else(|| item.completion.clone());
    if label.is_empty() {
        return None;
    }

    // `completionFilterTextSplitPattern` = `=>|[\(]`.
    let filter_text = if !(label.starts_with("=>") || label.starts_with('(')) {
        let cut = match (label.find("=>"), label.find('(')) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => label.len(),
        };
        label[..cut].trim().to_string()
    } else {
        label.clone()
    };

    if caps.label_details {
        label = filter_text.clone();
    }
    if label.ends_with(',') {
        label.pop();
    }

    let kind_id = match item.kind {
        YamlSuggestionKind::Identifier => 6,
        YamlSuggestionKind::PackageName => 9,
    };
    let completion_kind = caps.item_kinds.contains(&kind_id).then_some(kind_id);

    let (insert_text, is_snippet) = build_insert_text_for_yaml(
        caps.snippets,
        &item.completion,
        item.selection_offset,
        item.selection_length,
    );
    let is_multiline = insert_text.contains('\n');

    let raw_doc = match include_documentation {
        "full" => item.doc_complete.as_deref(),
        "summary" => item.doc_summary.as_deref(),
        _ => None,
    };
    let mut cleaned_doc = raw_doc.map(crate::hover::clean_dartdoc);
    let mut detail: Option<String> = None;
    if let Some(doc) = &cleaned_doc
        && let Some(inner) = doc.strip_prefix('_').and_then(|d| d.strip_suffix('_'))
        && inner.chars().count() <= 20
        && inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ' ')
    {
        detail = Some(inner.to_string());
        cleaned_doc = None;
    }

    let mut out = Map::new();
    out.insert("label".into(), json!(label));
    if let Some(k) = completion_kind {
        out.insert("kind".into(), json!(k));
    }
    if caps.deprecated_tag && item.is_deprecated {
        out.insert("tags".into(), json!([1]));
    }
    if let Some(r) = resolution_data {
        out.insert("data".into(), r);
    }
    if let Some(d) = detail.filter(|s| !s.is_empty()) {
        out.insert("detail".into(), json!(d));
    }
    if let Some(doc) = cleaned_doc {
        out.insert(
            "documentation".into(),
            crate::server::Server::markup_content_or_string(&caps.documentation_formats, doc),
        );
    }
    if caps.deprecated_flag && item.is_deprecated {
        out.insert("deprecated".into(), json!(true));
    }
    out.insert(
        "sortText".into(),
        json!((MAXIMUM_RELEVANCE_SORT - item.relevance).to_string()),
    );
    if filter_text != label {
        out.insert("filterText".into(), json!(filter_text));
    }
    if is_snippet {
        out.insert("insertTextFormat".into(), json!(2));
    }
    if caps.as_is_insert_mode && is_multiline {
        out.insert("insertTextMode".into(), json!(1));
    }
    if caps.insert_replace && insertion_range != replacement_range {
        out.insert(
            "textEdit".into(),
            json!({
                "insert": insertion_range,
                "replace": replacement_range,
                "newText": insert_text,
            }),
        );
    } else {
        out.insert(
            "textEdit".into(),
            json!({
                "range": replacement_range,
                "newText": insert_text,
            }),
        );
    }
    Some(Value::Object(out))
}
