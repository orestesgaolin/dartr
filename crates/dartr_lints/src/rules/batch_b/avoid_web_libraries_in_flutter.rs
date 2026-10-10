// Dart source: pkg/linter/lib/src/rules/avoid_web_libraries_in_flutter.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_project::yaml::NodeKind as YamlNodeKind;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if has_flutter_dep(c) {
        r.add(
            NodeKind::ImportDirective,
            "avoid_web_libraries_in_flutter",
            check,
        );
    }
}

/// Dart `AvoidWebLibrariesInFlutter.hasFlutterDep`.
fn has_flutter_dep(c: &LinterContext<'_>) -> bool {
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
    let Ok(parsed_pubspec) = dartr_project::yaml::load_yaml_node(&content) else {
        return false;
    };
    if !matches!(parsed_pubspec.kind, YamlNodeKind::Map(_)) {
        return false;
    }
    let Some(deps) = parsed_pubspec.value_at("dependencies") else {
        return false;
    };
    if !matches!(deps.kind, YamlNodeKind::Map(_)) || deps.value_at("flutter").is_none() {
        return false;
    }
    let is_web_plugin = parsed_pubspec
        .value_at("flutter")
        .and_then(|n| n.value_at("plugin"))
        .and_then(|n| n.value_at("platforms"))
        .and_then(|n| n.value_at("web"))
        .is_some_and(|w| !w.is_null_scalar());
    !is_web_plugin
}

fn is_web_uri(uri: Option<&str>) -> bool {
    matches!(uri, Some("dart:html" | "dart:js" | "dart:js_util"))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let uri = c.ast[Id::<ImportDirective>::from_raw(node)].uri.raw();
    let uri_string = super::valid_regexps::string_value(c, uri);
    if is_web_uri(uri_string.as_deref()) {
        c.report_node(out, &diag::AVOID_WEB_LIBRARIES_IN_FLUTTER, node, &[]);
    }
}
