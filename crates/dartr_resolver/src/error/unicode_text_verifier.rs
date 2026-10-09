// Dart source: pkg/analyzer/lib/src/error/unicode_text_verifier.dart

//! Checks for unsafe Unicode text (Dart `UnicodeTextVerifier`, see
//! CVE-2021-22567): the text direction code points U+202A–U+202E and
//! U+2066–U+2069.
//!
//! The source content is the source text of the token stream
//! (`Ast::tokens.source`); offsets are UTF-16 code units, as in Dart.

use dartr_ast::NodeKind;
use dartr_diagnostics::diag;

use super::{UnitVerifier, VerifierHost};

/// Dart `UnicodeTextVerifier(diagnosticReporter).verify(unit, content)`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let source = &*ast.tokens.source;
    // Optimization: we assume that we won't find any of these.
    if !source.chars().any(is_code_point_match) {
        return;
    }
    let mut offset = 0usize;
    for c in source.chars() {
        if is_code_point_match(c) {
            let node = ast.node_covering(v.unit, offset as u32, 0);
            // If it's not in a string literal, we assume we're in a comment.
            // This can potentially over-report on syntactically incorrect
            // sources (where Unicode is outside a string or comment).
            let code = format!("{:X}", c as u32);
            let d = match node.map(|n| ast.kind(n)) {
                Some(NodeKind::SimpleStringLiteral | NodeKind::InterpolationString) => {
                    diag::text_direction_code_point_in_literal(&code)
                }
                _ => diag::text_direction_code_point_in_comment(&code),
            };
            v.report(d.at_offset(offset, 1));
        }
        offset += c.len_utf16();
    }
}

/// Dart `_isCodeUnitMatch(codeUnit)`: U+202A, U+202B, U+202C, U+202D,
/// U+202E, U+2066, U+2067, U+2068, U+2069.
fn is_code_point_match(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}
