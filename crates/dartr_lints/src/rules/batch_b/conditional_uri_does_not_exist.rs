// Dart source: pkg/linter/lib/src/rules/conditional_uri_does_not_exist.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::DirectiveUri;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::Configuration,
        "conditional_uri_does_not_exist",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = c.resolved else {
        return;
    };
    let Some(uri) = resolved.tables.resolved_uri.get(node) else {
        return;
    };
    let (relative_uri_string, exists) = match uri {
        DirectiveUri::Source {
            relative_uri_string,
            source,
            ..
        }
        | DirectiveUri::Library {
            relative_uri_string,
            source,
            ..
        } => (
            relative_uri_string.as_ref(),
            dartr_project::fs::file_exists(&source.path),
        ),
        DirectiveUri::RelativeUri {
            relative_uri_string,
            ..
        }
        | DirectiveUri::RelativeUriString {
            relative_uri_string,
        } => (relative_uri_string.as_ref(), false),
        DirectiveUri::None | DirectiveUri::Unit { .. } => return,
    };
    if !exists {
        let config_uri = c.ast[Id::<Configuration>::from_raw(node)].uri;
        c.report_node(
            out,
            &diag::CONDITIONAL_URI_DOES_NOT_EXIST,
            config_uri,
            &[relative_uri_string],
        );
    }
}
