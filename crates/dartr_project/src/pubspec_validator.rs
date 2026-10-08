//! Validation of `pubspec.yaml` files.
//!
//! Port of `pkg/analyzer/lib/src/pubspec/pubspec_validator.dart` and
//! `pkg/analyzer/lib/src/pubspec/validators/*.dart`.

use crate::yaml::{NodeKind, Scalar, Span, YamlNode};
use crate::{AnalysisContext, fs, paths};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};

const DEPENDENCIES_FIELD: &str = "dependencies";
const DEV_DEPENDENCIES_FIELD: &str = "dev_dependencies";
const FLUTTER_FIELD: &str = "flutter";
const ASSETS_FIELD: &str = "assets";
const ASSET_PATH_FIELD: &str = "path";
const GIT_FIELD: &str = "git";
const NAME_FIELD: &str = "name";
const PATH_FIELD: &str = "path";
const PLATFORMS_FIELD: &str = "platforms";
const PUBLISH_TO_FIELD: &str = "publish_to";
const SCREENSHOTS_FIELD: &str = "screenshots";
const VERSION_FIELD: &str = "version";
const WORKSPACE_FIELD: &str = "workspace";

/// Validates the `pubspec.yaml` at [path].
///
/// As in `ContextManagerImpl._analyzePubspecYaml`, an unreadable file or a
/// YAML parse failure has no diagnostics. A valid non-map document is treated
/// as an empty map by the individual validators.
pub fn validate_pubspec(context: &AnalysisContext, path: &str) -> Vec<Diagnostic> {
    let Some(text) = fs::read_string(path) else {
        return Vec::new();
    };
    let Ok(contents) = crate::yaml::load_yaml_node(&text) else {
        return Vec::new();
    };
    let mut validator = Validator {
        contents: &contents,
        text: &text,
        path,
        diagnostics: Vec::new(),
    };

    // `_pubspecValidators` order is observable in the diagnostic list.
    validator.dependency_validator();
    validator.field_validator();
    validator.flutter_validator();
    validator.name_validator();
    validator.screenshots_validator();
    validator.platforms_validator();
    validator.workspace_validator();
    let options = crate::non_dart::options_for_file(context, path);
    if options.lint {
        validator.lint_validators(&options.lint_rules);
    }
    validator.filter_ignored();
    validator.diagnostics
}

/// Dependency edits associated with a `missing_dependency` diagnostic.
///
/// This is the Rust equivalent of `MissingDependencyData.byDiagnostic`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissingDependencyData {
    pub add_dependencies: Vec<String>,
    pub add_dev_dependencies: Vec<String>,
    pub remove_dev_dependencies: Vec<String>,
}

/// Result of [`validate_missing_dependencies`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissingDependencyValidation {
    pub diagnostics: Vec<Diagnostic>,
    pub data: Option<MissingDependencyData>,
}

/// Ports `MissingDependencyValidator.validate`.
///
/// The analysis server calls this validator during bulk-fix computation after
/// resolution has supplied the packages used by normal and development files.
/// Input order is retained, matching Dart's insertion-ordered `Set`.
pub fn validate_missing_dependencies(
    _context: &AnalysisContext,
    path: &str,
    used_dependencies: &[String],
    used_dev_dependencies: &[String],
) -> MissingDependencyValidation {
    let Some(text) = fs::read_string(path) else {
        return MissingDependencyValidation::default();
    };
    let Ok(contents) = crate::yaml::load_yaml_node(&text) else {
        return MissingDependencyValidation::default();
    };
    let Some(content_entries) = contents.as_map() else {
        return MissingDependencyValidation::default();
    };
    let mut validator = Validator {
        contents: &contents,
        text: &text,
        path,
        diagnostics: Vec::new(),
    };
    let dependencies = validator.declared_dependencies(DEPENDENCIES_FIELD);
    let dev_dependencies = validator.declared_dependencies(DEV_DEPENDENCIES_FIELD);
    let available_dependencies: Vec<String> = dependencies
        .iter()
        .map(|(name, _)| node_value_string(name))
        .collect();
    let available_dev_dependencies: Vec<String> = dev_dependencies
        .iter()
        .map(|(name, _)| node_value_string(name))
        .collect();
    let package_name = map_value(content_entries, NAME_FIELD).and_then(node_optional_value_string);

    let allowed =
        |name: &String| package_name.as_ref() != Some(name) && name.as_str() != "flutter_gen";
    let mut data = MissingDependencyData::default();
    for name in used_dependencies.iter().filter(|name| allowed(name)) {
        if !available_dependencies.contains(name) {
            data.add_dependencies.push(name.clone());
            if available_dev_dependencies.contains(name) {
                data.remove_dev_dependencies.push(name.clone());
            }
        }
    }
    for name in used_dev_dependencies.iter().filter(|name| allowed(name)) {
        if !available_dev_dependencies.contains(name) && !available_dependencies.contains(name) {
            data.add_dev_dependencies.push(name.clone());
        }
    }
    if data.add_dependencies.is_empty() && data.add_dev_dependencies.is_empty() {
        return MissingDependencyValidation {
            diagnostics: validator.diagnostics,
            data: None,
        };
    }

    let mut missing_parts = Vec::new();
    let mut fix_parts = Vec::new();
    for (section, packages) in [
        (DEPENDENCIES_FIELD, &data.add_dependencies),
        (DEV_DEPENDENCIES_FIELD, &data.add_dev_dependencies),
    ] {
        if packages.is_empty() {
            continue;
        }
        let names = packages
            .iter()
            .map(|name| format!("'{name}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let noun = if packages.len() == 1 {
            "package"
        } else {
            "packages"
        };
        missing_parts.push(format!("{noun} {names} in '{section}'"));
        fix_parts.push(format!("{names} to '{section}'"));
    }
    let location = content_entries
        .first()
        .map(|(_, value)| value.span)
        .unwrap_or_default();
    validator.report_span(
        location,
        diag::missing_dependency(&missing_parts.join(", and "), &fix_parts.join(", and ")),
    );
    MissingDependencyValidation {
        diagnostics: validator.diagnostics,
        data: Some(data),
    }
}

struct Validator<'a> {
    contents: &'a YamlNode,
    text: &'a str,
    path: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl Validator<'_> {
    fn dependency_validator(&mut self) {
        let dependencies = self.declared_dependencies(DEPENDENCIES_FIELD);
        let dev_dependencies = self.declared_dependencies(DEV_DEPENDENCIES_FIELD);

        let Some(contents) = self.contents.as_map() else {
            return;
        };
        let publishable = map_value(contents, VERSION_FIELD).is_some()
            && scalar_string(map_value(contents, PUBLISH_TO_FIELD)) != Some("none");

        for (_, dependency) in &dependencies {
            self.validate_path_entries(dependency, publishable);
        }
        for (package_name, dependency) in &dev_dependencies {
            if dependencies
                .iter()
                .any(|(dependency_name, _)| dependency_name.value_equals(package_name))
            {
                self.report(
                    package_name,
                    diag::unnecessary_dev_dependency(&node_value_string(package_name)),
                );
            }
            self.validate_path_entries(dependency, false);
        }
    }

    fn declared_dependencies(&mut self, field_name: &str) -> Vec<(YamlNode, YamlNode)> {
        let Some(contents) = self.contents.as_map() else {
            return Vec::new();
        };
        let Some(field) = map_value(contents, field_name) else {
            return Vec::new();
        };
        if field.is_null_scalar() {
            return Vec::new();
        }
        if let Some(entries) = field.as_map() {
            return entries.to_vec();
        }
        self.report(field, diag::dependencies_field_not_map(field_name));
        Vec::new()
    }

    fn validate_path_entries(&mut self, dependency: &YamlNode, reject_path_and_git: bool) {
        let Some(entries) = dependency.as_map() else {
            return;
        };
        if let Some(path_value) = map_value(entries, PATH_FIELD)
            && let Some(path_entry) = scalar_string(Some(path_value))
        {
            let path_key = map_key(entries, PATH_FIELD).expect("value has a key");
            if path_entry.contains('\\') {
                self.report(path_value, diag::path_not_posix(path_entry));
                return;
            }
            let package_root = paths::dirname(self.path);
            let dependency_path =
                paths::absolute_normalized(&paths::join(package_root, path_entry));
            if !fs::folder_exists(&dependency_path) {
                self.report(path_value, diag::path_does_not_exist(path_entry));
            } else if !fs::file_exists(&paths::join(&dependency_path, "pubspec.yaml")) {
                self.report(path_value, diag::path_pubspec_does_not_exist(path_entry));
            }
            if reject_path_and_git {
                self.report(path_key, diag::invalid_dependency(PATH_FIELD));
            }
        }

        if reject_path_and_git && map_value(entries, GIT_FIELD).is_some() {
            let git_key = map_key(entries, GIT_FIELD).expect("value has a key");
            self.report(git_key, diag::invalid_dependency(GIT_FIELD));
        }
    }

    fn field_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            return;
        };
        for (field, _) in contents {
            let Some(name) = scalar_string(Some(field)) else {
                continue;
            };
            if matches!(name, "author" | "authors" | "transformers" | "web") {
                self.report(field, diag::deprecated_field(name));
            }
        }
    }

    fn flutter_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            return;
        };
        let Some(flutter) = map_value(contents, FLUTTER_FIELD) else {
            return;
        };
        let Some(flutter_entries) = flutter.as_map() else {
            if !flutter.is_null_scalar() {
                self.report(flutter, diag::flutter_field_not_map());
            }
            return;
        };
        let Some(assets) = map_value(flutter_entries, ASSETS_FIELD) else {
            return;
        };
        let Some(asset_entries) = assets.as_list() else {
            self.report(assets, diag::asset_field_not_list());
            return;
        };

        for asset in asset_entries {
            match &asset.kind {
                NodeKind::Scalar(Scalar::String(path)) => self.validate_asset_path(path, asset),
                NodeKind::Scalar(_) => {
                    self.report(asset, diag::asset_not_string_or_map());
                    return;
                }
                NodeKind::Map(entries) => {
                    let Some(path_field) = map_value(entries, ASSET_PATH_FIELD) else {
                        self.report(asset, diag::asset_missing_path());
                        continue;
                    };
                    match &path_field.kind {
                        NodeKind::Scalar(Scalar::String(path)) => {
                            self.validate_asset_path(path, path_field)
                        }
                        NodeKind::Scalar(_) => {
                            self.report(path_field, diag::asset_not_string());
                            return;
                        }
                        _ => self.report(path_field, diag::asset_path_not_string()),
                    }
                }
                NodeKind::List(_) => self.report(asset, diag::asset_not_string_or_map()),
            }
        }
    }

    fn validate_asset_path(&mut self, path_value: &str, error_field: &YamlNode) {
        if path_value.starts_with("packages/") {
            return;
        }
        let package_root = paths::dirname(self.path);
        let asset_path = paths::normalize(&paths::join(package_root, path_value));
        if !asset_exists_at_path(&asset_path) {
            if path_value.ends_with('/') {
                self.report(
                    error_field,
                    diag::asset_directory_does_not_exist(path_value),
                );
            } else {
                self.report(error_field, diag::asset_does_not_exist(path_value));
            }
        }
    }

    fn name_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            self.report_span(Span::default(), diag::missing_name());
            return;
        };
        let Some(name) = map_value(contents, NAME_FIELD) else {
            self.report_span(Span::default(), diag::missing_name());
            return;
        };
        if !matches!(name.kind, NodeKind::Scalar(Scalar::String(_))) {
            self.report(name, diag::name_not_string());
        }
    }

    fn screenshots_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            return;
        };
        let Some(screenshots) = map_value(contents, SCREENSHOTS_FIELD).and_then(YamlNode::as_list)
        else {
            return;
        };
        let package_root = paths::dirname(self.path);
        for entry in screenshots {
            let Some(entries) = entry.as_map() else {
                continue;
            };
            let Some(path_node) = map_value(entries, PATH_FIELD) else {
                continue;
            };
            let Some(path_value) = scalar_string(Some(path_node)) else {
                continue;
            };
            let full_path = paths::normalize(&paths::join(package_root, path_value));
            if !fs::file_exists(&full_path) {
                self.report(path_node, diag::path_does_not_exist(path_value));
            }
        }
    }

    fn platforms_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            return;
        };
        let Some(platforms) = map_value(contents, PLATFORMS_FIELD) else {
            return;
        };
        let Some(entries) = platforms.as_map() else {
            self.report(platforms, diag::invalid_platforms_field());
            return;
        };
        for (platform, _) in entries {
            let known = scalar_string(Some(platform)).is_some_and(|value| {
                matches!(
                    value,
                    "android" | "ios" | "linux" | "macos" | "web" | "windows"
                )
            });
            if !known {
                self.report(
                    platform,
                    diag::unknown_platform(&node_value_string(platform)),
                );
            }
        }
        for (_, value) in entries {
            if !value.is_null_scalar() {
                self.report(value, diag::platform_value_disallowed());
            }
        }
    }

    fn workspace_validator(&mut self) {
        let Some(contents) = self.contents.as_map() else {
            return;
        };
        let Some(workspace) = map_value(contents, WORKSPACE_FIELD) else {
            return;
        };
        let Some(entries) = workspace.as_list() else {
            self.report(workspace, diag::workspace_field_not_list());
            return;
        };
        for directory in entries {
            let Some(path_value) = scalar_string(Some(directory)) else {
                self.report(directory, diag::workspace_value_not_string());
                if directory.is_scalar() {
                    return;
                }
                continue;
            };
            self.validate_workspace_path(path_value, directory);
        }
    }

    fn validate_workspace_path(&mut self, path_value: &str, error_field: &YamlNode) {
        let package_root = paths::dirname(self.path);
        let directory_path = paths::normalize(&paths::join(package_root, path_value));
        if !paths::is_within(package_root, &directory_path) {
            self.report(
                error_field,
                diag::workspace_value_not_subdirectory(package_root),
            );
            return;
        }
        if is_glob_pattern(path_value) {
            if let Some(base_path) = glob_base_path(path_value) {
                let base_directory = paths::normalize(&paths::join(package_root, &base_path));
                if !fs::folder_exists(&base_directory) {
                    self.report(error_field, diag::path_does_not_exist(&base_path));
                }
            }
        } else if !fs::folder_exists(&directory_path) {
            self.report(error_field, diag::path_does_not_exist(path_value));
        }
    }

    fn lint_validators(&mut self, lint_rules: &[String]) {
        for rule in lint_rules {
            match rule.as_str() {
                "package_names" => self.package_names_lint(),
                "secure_pubspec_urls" => self.secure_pubspec_urls_lint(),
                "sort_pub_dependencies" => self.sort_pub_dependencies_lint(),
                _ => {}
            }
        }
    }

    fn package_names_lint(&mut self) {
        let Some(entries) = self.contents.as_map() else {
            return;
        };
        let Some(name) = map_value(entries, NAME_FIELD) else {
            return;
        };
        let Some(package_name) = scalar_text(name) else {
            return;
        };
        if !is_valid_package_name(&package_name) {
            self.report(name, diag::package_names(package_name));
        }
    }

    fn secure_pubspec_urls_lint(&mut self) {
        let Some(entries) = self.contents.as_map() else {
            return;
        };
        // `_Pubspec.accept` visits these entries in this order.
        for field in ["documentation", "homepage", "issue_tracker", "repository"] {
            if let Some(value) = map_value(entries, field) {
                self.check_secure_url(value);
            }
        }
        for field in [
            DEPENDENCIES_FIELD,
            DEV_DEPENDENCIES_FIELD,
            "dependency_overrides",
        ] {
            let Some(dependencies) = map_value(entries, field).and_then(YamlNode::as_map) else {
                continue;
            };
            for (name, specification) in dependencies {
                if !name.is_scalar() {
                    continue;
                }
                let Some(specification) = specification.as_map() else {
                    continue;
                };
                if let Some(git) = map_value(specification, GIT_FIELD) {
                    if git.is_scalar() {
                        self.check_secure_url(git);
                    } else if let Some(git) = git.as_map()
                        && let Some(url) = map_value(git, "url")
                    {
                        self.check_secure_url(url);
                    }
                }
                if let Some(hosted) = map_value(specification, "hosted") {
                    if hosted.is_scalar() {
                        self.check_secure_url(hosted);
                    } else if let Some(hosted) = hosted.as_map()
                        && let Some(url) = map_value(hosted, "url")
                    {
                        self.check_secure_url(url);
                    }
                }
            }
        }
    }

    fn check_secure_url(&mut self, node: &YamlNode) {
        let Some(text) = scalar_text(node) else {
            return;
        };
        let Some(colon) = text.find(':') else {
            return;
        };
        let scheme = &text[..colon];
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("git") {
            self.report(node, diag::secure_pubspec_urls(scheme.to_ascii_lowercase()));
        }
    }

    fn sort_pub_dependencies_lint(&mut self) {
        let Some(entries) = self.contents.as_map() else {
            return;
        };
        for field in [
            DEPENDENCIES_FIELD,
            DEV_DEPENDENCIES_FIELD,
            "dependency_overrides",
        ] {
            let Some(dependencies) = map_value(entries, field).and_then(YamlNode::as_map) else {
                continue;
            };
            let mut previous = String::new();
            for (name, _) in dependencies {
                let Some(text) = scalar_text(name) else {
                    continue;
                };
                if text.encode_utf16().cmp(previous.encode_utf16()).is_lt() {
                    self.report(name, diag::sort_pub_dependencies());
                    break;
                }
                previous = text;
            }
        }
    }

    fn report(&mut self, node: &YamlNode, diagnostic: LocatableDiagnostic) {
        self.report_span(node.span, diagnostic);
    }

    fn report_span(&mut self, span: Span, diagnostic: LocatableDiagnostic) {
        let offset = utf16_offset(self.text, span.start);
        let end = utf16_offset(self.text, span.end);
        self.diagnostics
            .push(diagnostic.to_diagnostic(offset, end.saturating_sub(offset)));
    }

    fn filter_ignored(&mut self) {
        let ignores = YamlIgnores::parse(self.text);
        self.diagnostics.retain(|diagnostic| {
            let line = utf16_line(self.text, diagnostic.offset);
            !ignores.ignores(line, diagnostic.code.name, diagnostic.code.unique_name)
        });
    }
}

fn map_value<'a>(entries: &'a [(YamlNode, YamlNode)], name: &str) -> Option<&'a YamlNode> {
    entries
        .iter()
        .find(|(key, _)| scalar_string(Some(key)) == Some(name))
        .map(|(_, value)| value)
}

fn map_key<'a>(entries: &'a [(YamlNode, YamlNode)], name: &str) -> Option<&'a YamlNode> {
    entries
        .iter()
        .find(|(key, _)| scalar_string(Some(key)) == Some(name))
        .map(|(key, _)| key)
}

fn scalar_string(node: Option<&YamlNode>) -> Option<&str> {
    match node.map(|node| &node.kind) {
        Some(NodeKind::Scalar(Scalar::String(value))) => Some(value),
        _ => None,
    }
}

fn scalar_text(node: &YamlNode) -> Option<String> {
    match &node.kind {
        NodeKind::Scalar(Scalar::Null) => None,
        NodeKind::Scalar(value) => Some(value.to_dart_string()),
        _ => None,
    }
}

fn node_optional_value_string(node: &YamlNode) -> Option<String> {
    (!node.is_null_scalar()).then(|| node_value_string(node))
}

fn is_valid_package_name(name: &str) -> bool {
    const RESERVED_WORDS: &[&str] = &[
        "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
        "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
        "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void",
        "while", "with",
    ];
    let bytes = name.as_bytes();
    if bytes.is_empty() || RESERVED_WORDS.contains(&name) {
        return false;
    }
    let mut index = bytes.iter().take_while(|byte| **byte == b'_').count();
    if index == bytes.len() || !bytes[index].is_ascii_lowercase() {
        return false;
    }
    index += 1;
    let mut previous_underscore = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'_' {
            if previous_underscore || index + 1 == bytes.len() {
                return false;
            }
            previous_underscore = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_underscore = false;
        } else {
            return false;
        }
        index += 1;
    }
    true
}

fn node_value_string(node: &YamlNode) -> String {
    match &node.kind {
        NodeKind::Scalar(value) => value.to_dart_string(),
        NodeKind::List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(node_value_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NodeKind::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    node_value_string(key),
                    node_value_string(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn asset_exists_at_path(asset_path: &str) -> bool {
    if fs::folder_exists(asset_path) || fs::file_exists(asset_path) {
        return true;
    }
    let parent = paths::dirname(asset_path);
    if !fs::folder_exists(parent) {
        return false;
    }
    let file_name = paths::basename(asset_path);
    fs::children(parent).is_some_and(|children| {
        children.into_iter().any(|child| {
            child.kind == fs::ResourceKind::Folder
                && fs::file_exists(&paths::join(&child.path, file_name))
        })
    })
}

fn is_glob_pattern(value: &str) -> bool {
    // `Glob.quote(value) != value`; keep this list in sync with glob's
    // `_quoteRegExp`.
    value.chars().any(|character| {
        matches!(
            character,
            '*' | '{' | '[' | '?' | '\\' | '}' | ']' | ',' | '-' | '(' | ')'
        )
    })
}

fn glob_base_path(value: &str) -> Option<String> {
    let parts: Vec<&str> = value
        .split('/')
        .take_while(|part| !is_glob_pattern(part))
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn utf16_offset(text: &str, byte_offset: usize) -> usize {
    text.get(..byte_offset.min(text.len()))
        .unwrap_or(text)
        .encode_utf16()
        .count()
}

fn utf16_line(text: &str, offset: usize) -> usize {
    let mut line = 1;
    let mut consumed = 0;
    for character in text.chars() {
        if consumed >= offset {
            break;
        }
        consumed += character.len_utf16();
        if character == '\n' {
            line += 1;
        }
    }
    line
}

#[derive(Default)]
struct YamlIgnores {
    for_file: Vec<String>,
    on_line: Vec<(usize, Vec<String>)>,
}

impl YamlIgnores {
    fn parse(text: &str) -> Self {
        let mut result = Self::default();
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            if let Some((_, names)) = ignored_names(line, "ignore_for_file:") {
                result.for_file.extend(names);
            }
            if let Some((before, names)) = ignored_names(line, "ignore:") {
                let target_line = if before.trim().is_empty() {
                    line_number + 1
                } else {
                    line_number
                };
                result.on_line.push((target_line, names));
            }
        }
        result
    }

    fn ignores(&self, line: usize, name: &str, unique_name: &str) -> bool {
        let matches = |ignored: &str| {
            ignored.eq_ignore_ascii_case(name)
                || ignored
                    .eq_ignore_ascii_case(unique_name.split_once('.').map_or(unique_name, |x| x.1))
        };
        self.for_file.iter().any(|name| matches(name))
            || self
                .on_line
                .iter()
                .filter(|(target, _)| *target == line)
                .flat_map(|(_, names)| names)
                .any(|name| matches(name))
    }
}

fn ignored_names<'a>(line: &'a str, marker: &str) -> Option<(&'a str, Vec<String>)> {
    for (marker_offset, _) in line.rmatch_indices(marker) {
        let mut hash_end = marker_offset;
        while hash_end > 0 && line.as_bytes()[hash_end - 1] == b' ' {
            hash_end -= 1;
        }
        if hash_end == 0 || line.as_bytes()[hash_end - 1] != b'#' {
            continue;
        }
        let mut hash_start = hash_end - 1;
        while hash_start > 0 && line.as_bytes()[hash_start - 1] == b'#' {
            hash_start -= 1;
        }
        let ignored = &line[marker_offset + marker.len()..];
        return Some((
            &line[..hash_start],
            ignored
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
        ));
    }
    None
}
