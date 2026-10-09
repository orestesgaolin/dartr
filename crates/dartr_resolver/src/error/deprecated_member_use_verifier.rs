// Dart source: pkg/analyzer/lib/src/error/deprecated_member_use_verifier.dart

//! The deprecated elements as a usage set of the
//! [`super::element_usage_detector::ElementUsageDetector`]
//! (`DeprecatedElementUsageSet`, `DeprecatedElementUsageReporter`).

use dartr_diagnostics::diag;
use dartr_element::{Ctx, ElementId, Tag};

use super::{UnitVerifier, VerifierHost};
use crate::element_metadata::{UnitAst, element_annotations, element_has, flags};

/// Dart `normalizeDeprecationMessage(message)`.
pub fn normalize_deprecation_message(message: &str) -> Option<String> {
    let message = message.trim();
    if message.is_empty() || message == "." {
        None
    } else if message.ends_with('.') || message.ends_with('?') || message.ends_with('!') {
        Some(message.to_string())
    } else {
        Some(format!("{message}."))
    }
}

/// Dart `DeprecatedElementUsageSet.getTagInfo`: the message of the
/// `@Deprecated` annotation of [element] (the empty string for no message),
/// `None` if [element] is not deprecated for use.
///
/// The message is read from the syntax of the annotation (see
/// [`crate::element_metadata`]): a message that is not a string literal is
/// treated as the empty message.
pub fn get_tag_info(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit: Option<UnitAst<'_>>,
) -> Option<String> {
    if !element_has(ctx, element, flags::DEPRECATED, unit) {
        return None;
    }
    for annotation in element_annotations(ctx, element, unit) {
        let Some(kind) = annotation.deprecation_kind(ctx) else {
            continue;
        };
        if kind != "use" {
            continue;
        }
        if annotation.element.is_some_and(|e| e.tag() == Tag::Getter) {
            // `@deprecated` is treated as though it had no message.
            return Some(String::new());
        }
        return Some(
            annotation
                .string_argument(Some(0), Some("message"))
                .unwrap_or_default(),
        );
    }
    None
}

/// Dart `DeprecatedElementUsageReporter.report`.
pub fn report(
    v: &mut UnitVerifier<'_>,
    usage_site: (u32, u32),
    display_name: &str,
    tag_info: &str,
    is_in_same_package: bool,
) {
    if is_in_same_package {
        return;
    }
    let d = match normalize_deprecation_message(tag_info) {
        Some(message) => diag::deprecated_member_use_with_message(display_name, &message),
        None => diag::deprecated_member_use(display_name),
    };
    v.report(d.at_offset(usage_site.0 as usize, usage_site.1 as usize));
}
