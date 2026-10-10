// Dart source: pkg/linter/lib/src/rules/pub/depend_on_referenced_packages.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if c.pubspec_deps().is_some() {
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
    let Some(deps) = c.pubspec_deps() else {
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
