// Dart source: pkg/analyzer/lib/src/error/doc_comment_verifier.dart

//! Verifies the data parsed in doc comments: doc directives
//! (`{@animation ...}`, `{@youtube ...}`, ...) and doc imports
//! (`@docImport`) (Dart `DocCommentVerifier`).
//!
//! The best practices verifier calls [`doc_import`] and [`doc_directive`]
//! for each `Comment` (Dart `BestPracticesVerifier.visitComment`).

use dartr_ast::doc_comment::{
    DocDirective, DocDirectiveParameterFormat, DocDirectiveTag, DocImport,
};
use dartr_diagnostics::diag;

use super::VerifierHost;

/// Dart `DocCommentVerifier.docDirective(docDirective)`.
pub fn doc_directive<'a, H: VerifierHost<'a>>(host: &mut H, doc_directive: &DocDirective) {
    match doc_directive {
        DocDirective::Simple(d) => doc_directive_tag(host, &d.tag),
        DocDirective::Block(d) => {
            doc_directive_tag(host, &d.opening_tag);
            if let Some(closing_tag) = &d.closing_tag {
                doc_directive_tag(host, closing_tag);
            }
        }
    }
}

/// Dart `DocCommentVerifier.docDirectiveTag(tag)`.
pub fn doc_directive_tag<'a, H: VerifierHost<'a>>(host: &mut H, tag: &DocDirectiveTag) {
    validate_argument_count(host, tag);
    validate_argument_format(host, tag);
}

/// Dart `DocCommentVerifier.docImport(docImport)`: verifies a doc import,
/// written as `@docImport`.
pub fn doc_import<'a, H: VerifierHost<'a>>(host: &mut H, doc_import: &DocImport) {
    // The import directive is in its own AST, with offsets in the unit.
    let ast = &*doc_import.ast;
    let import = &ast[doc_import.import];
    if let Some(deferred_keyword) = import.deferred_keyword {
        let t = ast.tokens.get(deferred_keyword);
        host.report(
            diag::doc_import_cannot_be_deferred()
                .at_offset(t.offset as usize, (t.end() - t.offset) as usize),
        );
    }
    let configurations = ast.list_raw(import.configurations);
    if let (Some(&first), Some(&last)) = (configurations.first(), configurations.last()) {
        let offset = ast.offset(first);
        host.report(
            diag::doc_import_cannot_have_configurations()
                .at_offset(offset as usize, (ast.end(last) - offset) as usize),
        );
    }

    // TODO(srawlins): Support combinators.
    let combinators = ast.list_raw(import.combinators);
    if let (Some(&first), Some(&last)) = (combinators.first(), combinators.last()) {
        let offset = ast.offset(first);
        host.report(
            diag::doc_import_cannot_have_combinators()
                .at_offset(offset as usize, (ast.end(last) - offset) as usize),
        );
    }

    // TODO(srawlins): Support prefixes.
    if let Some(prefix) = import.prefix {
        let offset = ast.offset(prefix);
        host.report(
            diag::doc_import_cannot_have_prefix()
                .at_offset(offset as usize, (ast.end(prefix) - offset) as usize),
        );
    }
}

/// Dart `DocCommentVerifier.validateArgumentCount(tag)`.
pub fn validate_argument_count<'a, H: VerifierHost<'a>>(host: &mut H, tag: &DocDirectiveTag) {
    let positional_argument_count = tag.positional_arguments.len();
    let required = tag.ty.positional_parameters();
    let required_count = required.len();
    let directive = tag.ty.name();
    let tag_length = (tag.end - tag.offset) as usize;

    if positional_argument_count < required_count {
        let gap = required_count - positional_argument_count;
        let n = required.len();
        let d = match gap {
            1 => Some(diag::doc_directive_missing_one_argument(
                directive,
                required[n - 1].name,
            )),
            2 => Some(diag::doc_directive_missing_two_arguments(
                directive,
                required[n - 2].name,
                required[n - 1].name,
            )),
            3 => Some(diag::doc_directive_missing_three_arguments(
                directive,
                required[n - 3].name,
                required[n - 2].name,
                required[n - 1].name,
            )),
            _ => None,
        };
        if let Some(d) = d {
            host.report(d.at_offset(tag.offset as usize, tag_length));
        }
    }

    if tag.ty.rest_parameters_allowed() {
        // TODO(srawlins): We probably want to enforce that at least one
        // argument is given, particularly for 'category' and 'subCategory'.
        return;
    }

    if positional_argument_count > required_count {
        let error_offset = tag.positional_arguments[required_count].offset();
        let error_length =
            tag.positional_arguments[positional_argument_count - 1].end() - error_offset;
        host.report(
            diag::doc_directive_has_extra_arguments(
                directive,
                positional_argument_count as i64,
                required_count as i64,
            )
            .at_offset(error_offset as usize, error_length as usize),
        );
    }

    for named_argument in &tag.named_arguments {
        if !tag
            .ty
            .named_parameters()
            .iter()
            .any(|p| p.name == &*named_argument.name)
        {
            host.report(
                diag::doc_directive_has_unexpected_named_argument(directive, &named_argument.name)
                    .at_offset(
                        named_argument.offset as usize,
                        (named_argument.end - named_argument.offset) as usize,
                    ),
            );
        }
    }
}

/// Dart `DocCommentVerifier.validateArgumentFormat(tag)`.
pub fn validate_argument_format<'a, H: VerifierHost<'a>>(host: &mut H, tag: &DocDirectiveTag) {
    let required = tag.ty.positional_parameters();
    let positional_argument_count = tag.positional_arguments.len().min(required.len());
    for i in 0..positional_argument_count {
        let parameter = required[i];
        let argument = &tag.positional_arguments[i];
        let value = argument.value();
        let wrong_format = match parameter.expected_format {
            DocDirectiveParameterFormat::Any => false,
            DocDirectiveParameterFormat::Integer => int_try_parse(value).is_none(),
            DocDirectiveParameterFormat::Uri => !uri_parses(value),
            DocDirectiveParameterFormat::YoutubeUrl => {
                !uri_parses(value)
                    || !value.starts_with(DocDirectiveParameterFormat::YOUTUBE_URL_PREFIX)
            }
        };
        if wrong_format {
            host.report(
                diag::doc_directive_argument_wrong_format(
                    parameter.name,
                    parameter.expected_format.display_string(),
                )
                .at_offset(
                    argument.offset() as usize,
                    (argument.end() - argument.offset()) as usize,
                ),
            );
        }
    }
}

/// Dart `int.tryParse(source)` (radix 10, or 16 with a `0x` prefix):
/// leading and trailing whitespace is ignored, an optional sign, and the
/// value must fit in 64 bits.
fn int_try_parse(source: &str) -> Option<i64> {
    let s = source.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}');
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let (radix, digits) = match digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        Some(hex) => (16, hex),
        None => (10, digits),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let value = i128::from_str_radix(digits, radix).ok()?;
    let value = if negative { -value } else { value };
    if radix == 16 {
        // The VM accepts hexadecimal literals up to 2^64 - 1 (wrapping).
        if value.unsigned_abs() > u64::MAX as u128 {
            return None;
        }
        return Some(value as i64);
    }
    i64::try_from(value).ok()
}

/// Whether Dart `Uri.tryParse(value)` returns a URI. An approximation of
/// the cases in which `Uri.parse` throws a `FormatException`: an invalid
/// scheme, a port that is not a number, and an invalid bracketed host (as
/// `dartr_project::fix_data_validator::uri_parses`).
fn uri_parses(value: &str) -> bool {
    let bytes = value.as_bytes();
    let end_of_scheme = value.find([':', '/', '?', '#']);
    let mut rest = value;
    if let Some(colon) = end_of_scheme.filter(|&i| bytes[i] == b':') {
        let scheme = &value[..colon];
        let mut chars = scheme.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if !valid {
            return false;
        }
        rest = &value[colon + 1..];
    }
    let Some(after_slashes) = rest.strip_prefix("//") else {
        return true;
    };
    let authority_end = after_slashes
        .find(['/', '?', '#'])
        .unwrap_or(after_slashes.len());
    let authority = &after_slashes[..authority_end];
    let host_and_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let (host, port) = if let Some(inner) = host_and_port.strip_prefix('[') {
        let Some(close) = inner.find(']') else {
            return false;
        };
        let address = &inner[..close];
        let after = &inner[close + 1..];
        let address_ok = address.starts_with('v')
            || address.starts_with('V')
            || address
                .split_once('%')
                .map_or(address, |(a, _)| a)
                .parse::<std::net::Ipv6Addr>()
                .is_ok();
        if !address_ok {
            return false;
        }
        match after {
            "" => ("", None),
            _ => match after.strip_prefix(':') {
                Some(port) => ("", Some(port)),
                None => return false,
            },
        }
    } else {
        match host_and_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_and_port, None),
        }
    };
    if host.contains(['[', ']']) {
        return false;
    }
    match port {
        Some(port) => {
            let digits = port.strip_prefix(['+', '-']).unwrap_or(port);
            digits.chars().all(|c| c.is_ascii_digit())
        }
        None => true,
    }
}
