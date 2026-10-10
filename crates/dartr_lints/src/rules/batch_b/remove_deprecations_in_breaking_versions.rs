// Dart source: pkg/linter/lib/src/rules/remove_deprecations_in_breaking_version.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_project::yaml::{NodeKind as YamlNodeKind, Scalar, YamlNode};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if has_breaking_version(c) {
        r.add(
            NodeKind::Annotation,
            "remove_deprecations_in_breaking_versions",
            check,
        );
    }
}

fn scalar_text(node: &YamlNode) -> Option<String> {
    match &node.kind {
        YamlNodeKind::Scalar(Scalar::Null) => None,
        YamlNodeKind::Scalar(s) => Some(s.to_dart_string()),
        _ => None,
    }
}

struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
    has_build: bool,
}

/// Dart `Version.parse` from `package:pub_semver`.
fn parse_version(version: &str) -> Option<ParsedVersion> {
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
    let has_build = if let Some(rest) = suffix.strip_prefix('-') {
        if let Some((pre, build)) = rest.split_once('+') {
            if !valid_identifiers(pre) || !valid_identifiers(build) || build.contains('+') {
                return None;
            }
            true
        } else {
            if !valid_identifiers(rest) {
                return None;
            }
            false
        }
    } else if let Some(build) = suffix.strip_prefix('+') {
        if !valid_identifiers(build) || build.contains('+') {
            return None;
        }
        true
    } else if suffix.is_empty() {
        false
    } else {
        return None;
    };
    let parts: Vec<_> = version[..numeric_end].split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(ParsedVersion {
        major: parts[0].parse().ok()?,
        minor: parts[1].parse().ok()?,
        patch: parts[2].parse().ok()?,
        has_build,
    })
}

/// Dart `RemoveDeprecationsInBreakingVersion.isBreakingVersion`.
fn is_breaking_version(v: &ParsedVersion) -> bool {
    !v.has_build && ((v.minor == 0 && v.patch == 0) || (v.major == 0 && v.patch == 0))
}

fn has_breaking_version(c: &LinterContext<'_>) -> bool {
    let Some(root) = c.package_root() else {
        return false;
    };
    let pubspec_path = root.join("pubspec.yaml");
    let Some(content) = pubspec_path
        .to_str()
        .and_then(dartr_project::fs::read_string_strict)
    else {
        return false;
    };
    let Ok(yaml) = dartr_project::yaml::load_yaml_node(&content) else {
        return false;
    };
    let YamlNodeKind::Map(entries) = &yaml.kind else {
        return false;
    };
    let mut version_text = None;
    for (k, v) in entries {
        if scalar_text(k).as_deref() == Some("version") {
            version_text = scalar_text(v);
        }
    }
    let Some(version_text) = version_text else {
        return false;
    };
    let Some(version) = parse_version(&version_text) else {
        return false;
    };
    is_breaking_version(&version)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<Annotation>::from_raw(node)];
    // Dart `node.elementAnnotation?.isDeprecated`.
    let deprecated = c
        .element(node)
        .is_some_and(|e| is_deprecated_annotation_element(c, base(c, e)));
    if deprecated {
        c.report_node(
            out,
            &diag::REMOVE_DEPRECATIONS_IN_BREAKING_VERSIONS,
            n.name,
            &[],
        );
    }
}
