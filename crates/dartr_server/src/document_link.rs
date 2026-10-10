// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_document_link.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/navigation/document_links.dart

//! `textDocument/documentLink` handler and link computers for Dart files,
//! `pubspec.yaml`, and `analysis_options.yaml`.

use std::path::{Path, PathBuf};

use dartr_ast::doc_comment::{DocDirective, DocDirectiveType};
use dartr_ast::{Ast, AstVisitor, Comment, CompilationUnit, Id};
use dartr_lints::ALL_RULES;
use dartr_lints::RuleStateType;
use dartr_syntax::LineInfo;
use dartr_yaml::{NodeKind, Scalar, YamlNode, load_yaml_node};
use serde_json::{Value, json};

use crate::mapping::{ErrorOr, to_range};
use crate::server::{ParsedFile, Server, is_dart_document};
use crate::uri::path_to_uri;

/// A link to another document or URI found in a source file (Dart `DocumentLink`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentLink {
    pub offset: u32,
    pub length: u32,
    pub target_uri: String,
}

impl Server {
    /// Handles `textDocument/documentLink` (Dart `DocumentLinkHandler.handle`).
    pub(crate) fn document_link(&mut self, params: &Value) -> ErrorOr<Value> {
        let path = self.path_of_doc(params)?;
        if is_dart_document(params) {
            self.get_dart_document_links(&path)
        } else if is_pubspec_yaml(&path) {
            Ok(self.get_pubspec_document_links(&path))
        } else if is_analysis_options_yaml(&path) {
            Ok(self.get_analysis_options_document_links(&path))
        } else {
            Ok(json!([]))
        }
    }

    fn get_dart_document_links(&mut self, file_path: &str) -> ErrorOr<Value> {
        let file = self.require_parsed(file_path, false)?;
        let links = find_dart_document_links(file_path, &file);
        Ok(convert_links(&links, &file.unit.line_info))
    }

    fn get_pubspec_document_links(&self, file_path: &str) -> Value {
        let Some(content) = self.content(file_path) else {
            return json!([]);
        };
        let line_info = LineInfo::from_content(&content);
        let pub_hosted_url = valid_pub_hosted_url();
        let computer = PubspecDocumentLinkComputer::new(&pub_hosted_url);
        let links = computer.find_links(&content);
        convert_links(&links, &line_info)
    }

    fn get_analysis_options_document_links(&self, file_path: &str) -> Value {
        let Some(content) = self.content(file_path) else {
            return json!([]);
        };
        let line_info = LineInfo::from_content(&content);
        let pub_hosted_url = valid_pub_hosted_url();
        let computer = AnalysisOptionLinkComputer::new(&pub_hosted_url);
        let links = computer.find_links(&content);
        convert_links(&links, &line_info)
    }
}

fn is_pubspec_yaml(path: &str) -> bool {
    Path::new(path).file_name().and_then(|s| s.to_str()) == Some("pubspec.yaml")
}

fn is_analysis_options_yaml(path: &str) -> bool {
    Path::new(path).file_name().and_then(|s| s.to_str()) == Some("analysis_options.yaml")
}

fn convert_links(links: &[DocumentLink], line_info: &LineInfo) -> Value {
    let items: Vec<Value> = links
        .iter()
        .map(|link| {
            json!({
                "range": to_range(line_info, link.offset, link.length),
                "target": link.target_uri,
            })
        })
        .collect();
    Value::Array(items)
}

/// Returns a valid Pub base URL from `PUB_HOSTED_URL` if valid, otherwise
/// `'https://pub.dev'` (Dart `PubApi._validPubHostedUrl`).
pub fn valid_pub_hosted_url() -> String {
    let env_url = std::env::var("PUB_HOSTED_URL").ok();
    let valid_url = match env_url.as_deref() {
        Some(u) if is_valid_absolute_uri(u) => u,
        _ => "https://pub.dev",
    };
    valid_url.strip_suffix('/').unwrap_or(valid_url).to_string()
}

fn is_valid_absolute_uri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        && !rest.chars().any(|c| c.is_whitespace())
}

fn is_valid_http_or_https_uri(s: &str) -> bool {
    let valid_scheme = s.starts_with("https:") || s.starts_with("http:");
    valid_scheme
        && !s.chars().any(|c| {
            c.is_whitespace() || matches!(c, '<' | '>' | '"' | '^' | '`' | '{' | '|' | '}')
        })
}

/// Finds document links in a Dart compilation unit (Dart `DartDocumentLinkVisitor`).
pub fn find_dart_document_links(file_path: &str, file: &ParsedFile) -> Vec<DocumentLink> {
    let mut visitor = DartDocumentLinkVisitor {
        content_utf16: file.content.encode_utf16().collect(),
        file_path,
        folder_with_examples_api: None,
        links: Vec::new(),
    };
    visitor.visit_compilation_unit(&file.unit.ast, file.unit.unit);
    visitor.links
}

struct DartDocumentLinkVisitor<'a> {
    content_utf16: Vec<u16>,
    file_path: &'a str,
    folder_with_examples_api: Option<Option<PathBuf>>,
    links: Vec<DocumentLink>,
}

impl<'a> DartDocumentLinkVisitor<'a> {
    fn get_folder_with_examples_api(&mut self) -> Option<PathBuf> {
        if let Some(cached) = &self.folder_with_examples_api {
            return cached.clone();
        }
        let mut found = None;
        if let Some(parent) = Path::new(self.file_path).parent() {
            for ancestor in parent.ancestors() {
                if ancestor.join("examples").join("api").is_dir() {
                    found = Some(ancestor.to_path_buf());
                    break;
                }
            }
        }
        self.folder_with_examples_api = Some(found.clone());
        found
    }

    fn visit_compilation_unit(&mut self, ast: &Ast, unit: Id<CompilationUnit>) {
        ast.accept(unit.raw(), self);
    }
}

impl AstVisitor for DartDocumentLinkVisitor<'_> {
    fn visit_node(&mut self, ast: &Ast, node: dartr_ast::NodeId) {
        ast.visit_children(node, self);
    }

    fn visit_comment(&mut self, ast: &Ast, node: Id<Comment>) {
        ast.visit_children(node.raw(), self);

        let comment = &ast[node];
        for directive in &comment.doc_directives {
            let DocDirective::Block(tool_directive) = directive else {
                continue;
            };
            if tool_directive.opening_tag.ty != DocDirectiveType::Tool {
                continue;
            }
            let contents_start = tool_directive.opening_tag.end as usize;
            let Some(closing_tag) = &tool_directive.closing_tag else {
                continue;
            };
            let contents_end = closing_tag.offset as usize;
            if contents_start >= contents_end || contents_end > self.content_utf16.len() {
                continue;
            }
            let str_units = &self.content_utf16[contents_start..contents_end];
            let str_value = String::from_utf16_lossy(str_units);
            if str_value.is_empty() {
                continue;
            }

            let see_code_in = "** See code in ";
            let needle = "** See code in examples/api/";
            if let Some(byte_start) = str_value.find(needle) {
                let Some(folder_with_examples_api) = self.get_folder_with_examples_api() else {
                    return;
                };
                let start_byte = byte_start + see_code_in.len();
                let Some(dart_byte) = str_value.find(".dart") else {
                    continue;
                };
                let end_byte = dart_byte + 5;
                if start_byte >= end_byte || end_byte > str_value.len() {
                    continue;
                }
                let path_snippet = &str_value[start_byte..end_byte];
                let mut example_path = folder_with_examples_api;
                for segment in path_snippet.split('/') {
                    example_path.push(segment);
                }
                let start_utf16 = str_value[..start_byte].encode_utf16().count();
                let length_utf16 = path_snippet.encode_utf16().count();
                let offset = (contents_start + start_utf16) as u32;
                let length = length_utf16 as u32;
                let target_uri = path_to_uri(&example_path.to_string_lossy());
                self.links.push(DocumentLink {
                    offset,
                    length,
                    target_uri,
                });
            }
        }
    }
}

/// Computes [`DocumentLink`]s for package names in a `'pubspec.yaml'`
/// (Dart `PubspecDocumentLinkComputer`).
pub struct PubspecDocumentLinkComputer<'a> {
    pub pub_hosted_url: &'a str,
}

impl<'a> PubspecDocumentLinkComputer<'a> {
    pub fn new(pub_hosted_url: &'a str) -> Self {
        Self { pub_hosted_url }
    }

    pub fn find_links(&self, content: &str) -> Vec<DocumentLink> {
        let Ok(node) = load_yaml_node(content) else {
            return Vec::new();
        };
        let NodeKind::Map(_) = &node.kind else {
            return Vec::new();
        };

        let mut all_dependencies: Vec<(&YamlNode, &YamlNode)> = Vec::new();
        for section in ["dependencies", "dev_dependencies", "dependency_overrides"] {
            if let Some(dep_map) = node.value_at(section)
                && let NodeKind::Map(entries) = &dep_map.kind
            {
                for (k, v) in entries {
                    all_dependencies.push((k, v));
                }
            }
        }

        let mut links = Vec::new();
        for (package_name, package_source) in all_dependencies {
            if let Some(target_uri) = self.compute_link(package_name, package_source) {
                let offset = package_name.span.start.offset as u32;
                let length = package_name.span.length() as u32;
                links.push(DocumentLink {
                    offset,
                    length,
                    target_uri,
                });
            }
        }
        links
    }

    fn compute_git(&self, git_url: &str) -> Option<String> {
        let replaced = if let Some(rest) = git_url.strip_prefix("git@github.com:") {
            format!("https://github.com/{rest}")
        } else {
            git_url.to_string()
        };
        if is_valid_http_or_https_uri(&replaced) {
            Some(replaced)
        } else {
            None
        }
    }

    fn compute_hosted(&self, hosted_base: &str, package_name: &str) -> Option<String> {
        let separator = if hosted_base.ends_with('/') { "" } else { "/" };
        let candidate = format!("{hosted_base}{separator}packages/{package_name}");
        if is_valid_http_or_https_uri(&candidate) || is_valid_absolute_uri(&candidate) {
            Some(candidate)
        } else {
            None
        }
    }

    pub(crate) fn compute_link(
        &self,
        package_name_node: &YamlNode,
        package_source: &YamlNode,
    ) -> Option<String> {
        let NodeKind::Scalar(Scalar::String(name)) = &package_name_node.kind else {
            return None;
        };

        match &package_source.kind {
            // Standard Pub packages: `foo:` or `foo: ^123`
            NodeKind::Scalar(_) => self.compute_hosted(self.pub_hosted_url, name),
            NodeKind::Map(_) => {
                // Hosted: `hosted: http://foo/` or `hosted: { url: http://foo }`
                if let Some(hosted) = package_source.value_at("hosted") {
                    match &hosted.kind {
                        NodeKind::Scalar(Scalar::String(base)) => {
                            return self.compute_hosted(base, name);
                        }
                        NodeKind::Map(_) => {
                            if let Some(url_node) = hosted.value_at("url")
                                && let NodeKind::Scalar(Scalar::String(base)) = &url_node.kind
                            {
                                return self.compute_hosted(base, name);
                            }
                        }
                        _ => {}
                    }
                }
                // Git: `git: git@github.com:...` or `git: { url: http://foo }`
                if let Some(git) = package_source.value_at("git") {
                    match &git.kind {
                        NodeKind::Scalar(Scalar::String(url)) => {
                            return self.compute_git(url);
                        }
                        NodeKind::Map(_) => {
                            if let Some(url_node) = git.value_at("url")
                                && let NodeKind::Scalar(Scalar::String(url)) = &url_node.kind
                            {
                                return self.compute_git(url);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// Computes [`DocumentLink`]s for lint names and plugins in an `'analysis_options.yaml'`
/// (Dart `AnalysisOptionLinkComputer`).
pub struct AnalysisOptionLinkComputer<'a> {
    pub pub_hosted_url: &'a str,
}

impl<'a> AnalysisOptionLinkComputer<'a> {
    const LINT_STATES_BASE: &'static str =
        "https://github.com/dart-lang/sdk/blob/main/pkg/linter/doc/lint-lifecycle.md";

    pub fn new(pub_hosted_url: &'a str) -> Self {
        Self { pub_hosted_url }
    }

    pub fn find_links(&self, content: &str) -> Vec<DocumentLink> {
        let Ok(node) = load_yaml_node(content) else {
            return Vec::new();
        };
        let NodeKind::Map(_) = &node.kind else {
            return Vec::new();
        };

        let mut all_rules: Vec<&YamlNode> = Vec::new();
        if let Some(linter) = node.value_at("linter")
            && let NodeKind::Map(_) = &linter.kind
            && let Some(rules) = linter.value_at("rules")
        {
            match &rules.kind {
                NodeKind::Map(entries) => {
                    for (k, _) in entries {
                        all_rules.push(k);
                    }
                }
                NodeKind::List(items) => {
                    for item in items {
                        all_rules.push(item);
                    }
                }
                _ => {}
            }
        }

        let mut links = Vec::new();
        for rule in all_rules {
            if let Some(package_link) = self.compute_rule_link(rule) {
                let offset = rule.span.start.offset as u32;
                let length = rule.span.length() as u32;
                links.push(DocumentLink {
                    offset,
                    length,
                    target_uri: package_link,
                });
            }
        }

        let mut all_plugins: Vec<(&YamlNode, &YamlNode)> = Vec::new();
        if let Some(analyzer) = node.value_at("analyzer")
            && let NodeKind::Map(_) = &analyzer.kind
            && let Some(plugins) = analyzer.value_at("plugins")
        {
            collect_plugins(plugins, &mut all_plugins);
        }
        if let Some(plugins) = node.value_at("plugins") {
            collect_plugins(plugins, &mut all_plugins);
        }

        for (plugin_name, plugin_source) in all_plugins {
            if let Some(plugin_link) = self.compute_plugin_link(plugin_name, plugin_source) {
                let offset = plugin_name.span.start.offset as u32;
                let length = plugin_name.span.length() as u32;
                links.push(DocumentLink {
                    offset,
                    length,
                    target_uri: plugin_link,
                });
            }
        }

        links.sort_by_key(|l| l.offset);
        links
    }

    fn compute_rule_link(&self, rule: &YamlNode) -> Option<String> {
        let NodeKind::Scalar(Scalar::String(name)) = &rule.kind else {
            return None;
        };
        let name_lower = name.to_lowercase();
        let lint = ALL_RULES
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(&name_lower))?;

        match lint.state.kind {
            RuleStateType::Internal => Some(format!("{}#internal", Self::LINT_STATES_BASE)),
            RuleStateType::Removed => Some(format!("{}#removed", Self::LINT_STATES_BASE)),
            RuleStateType::Testing => Some(format!("{}#testing", Self::LINT_STATES_BASE)),
            _ => Some(format!("https://dart.dev/lints/{name_lower}")),
        }
    }

    fn compute_plugin_link(
        &self,
        plugin_name: &YamlNode,
        plugin_source: &YamlNode,
    ) -> Option<String> {
        let computer = PubspecDocumentLinkComputer::new(self.pub_hosted_url);
        if let Some(link) = computer.compute_link(plugin_name, plugin_source) {
            return Some(link);
        }
        // Fallback to pub.dev if it was not a git or hosted link but we have a name.
        if let NodeKind::Scalar(Scalar::String(name)) = &plugin_name.kind
            && !name.is_empty()
        {
            let separator = if self.pub_hosted_url.ends_with('/') {
                ""
            } else {
                "/"
            };
            return Some(format!("{}{separator}packages/{name}", self.pub_hosted_url));
        }
        None
    }
}

fn collect_plugins<'a>(plugins: &'a YamlNode, out: &mut Vec<(&'a YamlNode, &'a YamlNode)>) {
    match &plugins.kind {
        NodeKind::Map(entries) => {
            for (k, v) in entries {
                out.push((k, v));
            }
        }
        NodeKind::List(items) => {
            for item in items {
                out.push((item, item));
            }
        }
        _ => {}
    }
}
