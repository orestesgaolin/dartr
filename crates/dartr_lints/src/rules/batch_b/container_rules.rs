// Dart source: pkg/linter/lib/src/rules/use_decorated_box.dart
// Dart source: pkg/linter/lib/src/rules/sized_box_for_whitespace.dart
// Dart source: pkg/linter/lib/src/rules/use_colored_box.dart
//! The `Container` rules (shared shape: `isWidgetTypeContainer` and the
//! named arguments).

use super::flutter::*;
use super::util::*;
use crate::LinterContext;
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode};
use dartr_element::Nullability;
use dartr_typesystem::TypeExt;

/// Visits an instance creation of `Container` and reports its constructor
/// name when [should_report] accepts its arguments.
pub fn check_container(
    c: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
    code: &'static DiagnosticCode,
    should_report: fn(&LinterContext<'_>, &[NodeId]) -> bool,
) {
    if !is_container(c, c.static_type(node)) {
        return;
    }
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    if should_report(c, &arguments(c, n.argument_list)) {
        c.report_node(out, code, n.constructor_name, &[]);
    }
}

/// `use_decorated_box`.
pub fn decorated_box(c: &LinterContext<'_>, args: &[NodeId]) -> bool {
    let (mut has_child, mut has_decoration) = (false, false);
    for &argument in args {
        match named_argument_name(c, argument) {
            None => return false,
            Some("child") => has_child = true,
            Some("decoration") => has_decoration = true,
            Some("key") => {}
            Some(_) => return false,
        }
    }
    has_child && has_decoration
}

/// `sized_box_for_whitespace`.
pub fn sized_box(c: &LinterContext<'_>, args: &[NodeId]) -> bool {
    let (mut has_child, mut has_height, mut has_width) = (false, false, false);
    for &argument in args {
        match named_argument_name(c, argument) {
            None => return false,
            Some("child") => has_child = true,
            Some("height") => has_height = true,
            Some("width") => has_width = true,
            Some("key") => {}
            Some(_) => return false,
        }
    }
    has_child && (has_width || has_height) || has_width && has_height
}

/// `use_colored_box`.
pub fn colored_box(c: &LinterContext<'_>, args: &[NodeId]) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let (mut has_child, mut has_color) = (false, false);
    for &argument in args {
        match named_argument_name(c, argument) {
            None => return false,
            Some("child") => has_child = true,
            Some("color")
                if c.static_type(argument_expression(c, argument))
                    .is_none_or(|t| ctx.nullability_suffix(t) != Nullability::Question) =>
            {
                has_color = true
            }
            Some("key") => {}
            Some(_) => return false,
        }
    }
    has_child && has_color
}
