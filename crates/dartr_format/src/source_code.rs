// Dart source: dart_style lib/src/source_code.dart

//! Source code to format with an optional selection.

use crate::text::substring_utf16;

/// Describes a chunk of source code that is to be formatted or has been
/// formatted.
///
/// Offsets ([selection_start], [selection_length]) are in UTF-16 code units,
/// like Dart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCode {
    /// The [uri] where the source code is from.
    ///
    /// Used in error messages if the code cannot be parsed.
    pub uri: Option<String>,

    /// The Dart source code text.
    pub text: String,

    /// Whether the source is a compilation unit or a bare statement.
    pub is_compilation_unit: bool,

    /// The offset in [text] where the selection begins, or `None` if there is
    /// no selection.
    pub selection_start: Option<usize>,

    /// The number of selected characters or `None` if there is no selection.
    pub selection_length: Option<usize>,
}

impl SourceCode {
    /// Dart `SourceCode(text, uri:, isCompilationUnit:, selectionStart:,
    /// selectionLength:)`. Returns an error (Dart `ArgumentError`) if the
    /// selection is invalid.
    pub fn new(
        text: impl Into<String>,
        uri: Option<String>,
        is_compilation_unit: bool,
        selection_start: Option<usize>,
        selection_length: Option<usize>,
    ) -> Result<SourceCode, String> {
        let text = text.into();
        // Must either provide both selection bounds or neither.
        if selection_start.is_none() != selection_length.is_none() {
            return Err("If selectionStart is provided, selectionLength must be too.".into());
        }
        let len = crate::text::utf16_len(&text);
        if let Some(start) = selection_start {
            if start > len {
                return Err("selectionStart must be within text.".into());
            }
        }
        if let Some(length) = selection_length {
            if selection_start.unwrap() + length > len {
                return Err("selectionLength must end within text.".into());
            }
        }
        Ok(SourceCode {
            uri,
            text,
            is_compilation_unit,
            selection_start,
            selection_length,
        })
    }

    /// A compilation unit without a selection.
    pub fn unit(text: impl Into<String>) -> SourceCode {
        SourceCode {
            uri: None,
            text: text.into(),
            is_compilation_unit: true,
            selection_start: None,
            selection_length: None,
        }
    }

    /// Gets the source code before the beginning of the selection.
    ///
    /// If there is no selection, returns [text].
    pub fn text_before_selection(&self) -> &str {
        match self.selection_start {
            None => &self.text,
            Some(start) => substring_utf16(&self.text, 0, start),
        }
    }

    /// Gets the selected source code, if any.
    ///
    /// If there is no selection, returns an empty string.
    pub fn selected_text(&self) -> &str {
        match self.selection_start {
            None => "",
            Some(start) => {
                substring_utf16(&self.text, start, start + self.selection_length.unwrap())
            }
        }
    }

    /// Gets the source code following the selection.
    ///
    /// If there is no selection, returns an empty string.
    pub fn text_after_selection(&self) -> &str {
        match self.selection_start {
            None => "",
            Some(start) => {
                let end = start + self.selection_length.unwrap();
                substring_utf16(&self.text, end, usize::MAX)
            }
        }
    }

    /// Dart `textWithSelectionMarkers` (`test_file.dart`): the text with `‹`
    /// and `›` at the selection bounds.
    pub fn text_with_selection_markers(&self) -> String {
        if self.selection_start.is_none() {
            return self.text.clone();
        }
        format!(
            "{}‹{}›{}",
            self.text_before_selection(),
            self.selected_text(),
            self.text_after_selection()
        )
    }
}
