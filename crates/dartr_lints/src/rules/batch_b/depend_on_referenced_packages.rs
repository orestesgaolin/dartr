// Dart source: pkg/linter/lib/src/rules/pub/depend_on_referenced_packages.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_project::yaml::{NodeKind as YamlNodeKind, Scalar, YamlNode};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if pubspec_deps(c).is_some() {
        r.add(
            NodeKind::ImportDirective,
            "depend_on_referenced_packages",
            check,
        );
        r.add(
            NodeKind::ExportDirective,
            "depend_on_referenced_packages",
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

fn process_dependencies(node: &YamlNode) -> Option<Vec<String>> {
    let YamlNodeKind::Map(entries) = &node.kind else {
        return None;
    };
    Some(entries.iter().filter_map(|(k, _)| scalar_text(k)).collect())
}

struct PubspecDeps {
    name: String,
    dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
}

fn parse_pubspec_deps(pubspec_path: &std::path::Path) -> Option<std::sync::Arc<PubspecDeps>> {
    let content = dartr_project::fs::read_string_strict(pubspec_path.to_str()?)?;
    let yaml = dartr_project::yaml::load_yaml_node(&content).ok()?;
    let YamlNodeKind::Map(entries) = &yaml.kind else {
        return None;
    };
    let mut name: Option<String> = None;
    let mut dependencies: Option<Vec<String>> = None;
    let mut dev_dependencies: Option<Vec<String>> = None;
    for (k, v) in entries {
        let Some(key) = scalar_text(k) else {
            continue;
        };
        match key.as_str() {
            "name" => name = scalar_text(v),
            "dependencies" => dependencies = process_dependencies(v),
            "dev_dependencies" => dev_dependencies = process_dependencies(v),
            _ => {}
        }
    }
    Some(std::sync::Arc::new(PubspecDeps {
        name: name?,
        dependencies: dependencies.unwrap_or_default(),
        dev_dependencies: dev_dependencies.unwrap_or_default(),
    }))
}

fn pubspec_deps(c: &LinterContext<'_>) -> Option<std::sync::Arc<PubspecDeps>> {
    static CACHE: std::sync::OnceLock<
        std::sync::RwLock<
            std::collections::HashMap<std::path::PathBuf, Option<std::sync::Arc<PubspecDeps>>>,
        >,
    > = std::sync::OnceLock::new();
    let root = c.pub_package_root()?;
    let pubspec_path = root.join("pubspec.yaml");
    if !dartr_project::fs::has_overlays() {
        let cache = CACHE.get_or_init(Default::default);
        if let Some(cached) = cache.read().unwrap().get(&pubspec_path) {
            return cached.clone();
        }
        let parsed = parse_pubspec_deps(&pubspec_path);
        cache.write().unwrap().insert(pubspec_path, parsed.clone());
        return parsed;
    }
    parse_pubspec_deps(&pubspec_path)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let uri = if let Some(import) = c.ast.cast::<ImportDirective>(node) {
        c.ast[import].uri
    } else if let Some(export) = c.ast.cast::<ExportDirective>(node) {
        c.ast[export].uri
    } else {
        return;
    };
    let Some(uri_content) = super::valid_regexps::string_value(c, uri.raw()) else {
        return;
    };
    let Some(rest) = uri_content.strip_prefix("package:") else {
        return;
    };
    let Some((package_name, _)) = rest.split_once('/') else {
        return;
    };
    if package_name == "flutter_gen" {
        return;
    }
    let Some(deps) = pubspec_deps(c) else {
        return;
    };
    if deps.name == package_name
        || deps.dependencies.iter().any(|d| d == package_name)
        || (!c.is_in_public_dir() && deps.dev_dependencies.iter().any(|d| d == package_name))
    {
        return;
    }
    c.report_node(
        out,
        &diag::DEPEND_ON_REFERENCED_PACKAGES,
        uri,
        &[package_name],
    );
}
