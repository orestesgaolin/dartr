// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/scanner.dart

//! Entry points of the front end scanner.

use std::sync::Arc;

use crate::abstract_scanner::{
    AbstractScanner, LanguageVersionChanged, LanguageVersionInfo, ScannerConfiguration,
};
use crate::token::{TokenId, Tokens};

/// Dart `ScannerResult`.
#[derive(Clone, Debug)]
pub struct ScannerResult {
    pub tokens: Tokens,
    /// The first token. Error tokens (if any) come first.
    pub first: TokenId,
    /// Dart `lineStarts`: UTF-16 offsets of the line starts. Like in Dart,
    /// there is an additional line start after the end of the file.
    pub line_starts: Vec<u32>,
    pub has_errors: bool,
    /// The last language version comment in the header.
    pub language_version: Option<LanguageVersionInfo>,
}

/// Removes a leading byte order mark. Dart `File.readAsStringSync` (UTF-8
/// decoding) does the same before the analyzer sees the text.
pub fn strip_bom(source: &str) -> &str {
    source.strip_prefix('\u{FEFF}').unwrap_or(source)
}

/// Dart `scanString`: scans [source] (without byte order mark).
pub fn scan_string(
    source: &str,
    configuration: Option<ScannerConfiguration>,
    include_comments: bool,
    mut language_version_changed: Option<&mut LanguageVersionChanged<'_>>,
) -> ScannerResult {
    let shared: Arc<str> = Arc::from(source);
    let (result, recovery_start) = run(
        source,
        shared.clone(),
        configuration,
        include_comments,
        language_version_changed.as_deref_mut(),
        None,
    );
    if let Some(offset) = recovery_start {
        // If there was a single missing `}` and the scanner can identify a
        // good candidate for better recovery, create a new scanner and
        // instruct it to do that recovery.
        let (result, _) = run(
            source,
            shared,
            configuration,
            include_comments,
            language_version_changed,
            Some(offset),
        );
        return result;
    }
    result
}

fn run(
    source: &str,
    shared: Arc<str>,
    configuration: Option<ScannerConfiguration>,
    include_comments: bool,
    language_version_changed: Option<&mut LanguageVersionChanged<'_>>,
    offset_for_curly_bracket_recovery_start: Option<u32>,
) -> (ScannerResult, Option<u32>) {
    // Shorten the trait object lifetime to the lifetime of the reference (the
    // match is a coercion site).
    #[allow(clippy::needless_match)]
    let callback: Option<&mut LanguageVersionChanged<'_>> = match language_version_changed {
        Some(f) => Some(f),
        None => None,
    };
    let mut scanner = AbstractScanner::new(
        source,
        Tokens::new(shared),
        configuration,
        include_comments,
        callback,
    );
    scanner.offset_for_curly_bracket_recovery_start = offset_for_curly_bracket_recovery_start;
    let first = scanner.tokenize();
    let recovery_start = if scanner.has_errors && offset_for_curly_bracket_recovery_start.is_none()
    {
        scanner.get_offset_for_curly_bracket_recovery_start()
    } else {
        None
    };
    let has_errors = scanner.has_errors;
    let language_version = scanner.language_version();
    let line_starts = std::mem::take(&mut scanner.line_starts);
    (
        ScannerResult {
            tokens: scanner.into_tokens(),
            first,
            line_starts,
            has_errors,
            language_version,
        },
        recovery_start,
    )
}
