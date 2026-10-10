// Dart source: pkg/analyzer_plugin/lib/protocol/protocol_common.dart (SourceChange, SourceFileEdit, SourceEdit, LinkedEditGroup, LinkedEditSuggestion, Position)
// Dart source: pkg/analyzer_plugin/lib/src/protocol/protocol_internal.dart (addEditForSource)
// Dart source: pkg/analyzer_plugin/lib/utilities/change_builder/conflicting_edit_exception.dart

//! The change model of the correction producers: a [SourceChange] is a set
//! of [SourceFileEdit]s (edits of one file, sorted by decreasing offset),
//! linked edit groups and a selection. Offsets and lengths are in UTF-16
//! code units, like in Dart.

/// The UTF-16 length of [s] (Dart `String.length`).
pub fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Dart `SourceEdit`. [id] identifies the edit (Dart compares edits by
/// identity when it reverts and finalizes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEdit {
    pub offset: u32,
    pub length: u32,
    pub replacement: String,
    pub id: u64,
}

impl SourceEdit {
    pub fn end(&self) -> u32 {
        self.offset + self.length
    }

    /// The change of the length of the text (Dart `_editDelta`).
    pub fn delta(&self) -> i64 {
        utf16_len(&self.replacement) as i64 - self.length as i64
    }

    /// Dart `SourceEdit.apply`: applies the edit to [code] (UTF-16 offsets).
    pub fn apply(&self, code: &str) -> String {
        let units: Vec<u16> = code.encode_utf16().collect();
        let start = (self.offset as usize).min(units.len());
        let end = (self.end() as usize).min(units.len());
        let mut out = String::from_utf16_lossy(&units[..start]);
        out.push_str(&self.replacement);
        out.push_str(&String::from_utf16_lossy(&units[end..]));
        out
    }
}

/// Dart `ConflictingEditException`.
#[derive(Clone, Debug)]
pub struct ConflictingEdit;

/// Dart `SourceFileEdit`: the edits of one file. `file_stamp` is `-1` for a
/// file that does not exist yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFileEdit {
    pub file: String,
    pub file_stamp: i64,
    /// Sorted by decreasing offset.
    pub edits: Vec<SourceEdit>,
}

impl SourceFileEdit {
    pub fn new(file: &str, file_stamp: i64) -> Self {
        SourceFileEdit {
            file: file.to_string(),
            file_stamp,
            edits: Vec::new(),
        }
    }

    /// Dart `addEditForSource`: inserts [edit] in offset order; overlapping
    /// edits are a [ConflictingEdit].
    pub fn add(
        &mut self,
        edit: SourceEdit,
        insert_before_existing: bool,
    ) -> Result<(), ConflictingEdit> {
        let edits = &mut self.edits;
        let length = edits.len();
        let mut index = 0;
        while index < length && edits[index].offset > edit.offset {
            index += 1;
        }
        if insert_before_existing && edit.length == 0 {
            while index < length && edits[index].offset >= edit.offset {
                index += 1;
            }
        }
        if index > 0 {
            let previous = &edits[index - 1];
            if edit.offset + edit.length > previous.offset {
                return Err(ConflictingEdit);
            }
        }
        if index < length {
            let next = &edits[index];
            if (edit.offset == next.offset && edit.length > 0 && next.length > 0)
                || next.offset + next.length > edit.offset
            {
                return Err(ConflictingEdit);
            }
        }
        edits.insert(index, edit);
        Ok(())
    }
}

/// Dart `Position`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    pub file: String,
    pub offset: u32,
}

/// Dart `LinkedEditSuggestionKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkedEditSuggestionKind {
    Method,
    Parameter,
    Type,
    Variable,
}

/// Dart `LinkedEditSuggestion`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedEditSuggestion {
    pub value: String,
    pub kind: LinkedEditSuggestionKind,
}

/// Dart `LinkedEditGroup`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkedEditGroup {
    pub positions: Vec<Position>,
    pub length: u32,
    pub suggestions: Vec<LinkedEditSuggestion>,
}

impl LinkedEditGroup {
    /// Dart `addPosition`.
    pub fn add_position(&mut self, position: Position, length: u32) {
        self.positions.push(position);
        self.length = length;
    }

    /// Dart `addSuggestion`.
    pub fn add_suggestion(&mut self, suggestion: LinkedEditSuggestion) {
        self.suggestions.push(suggestion);
    }
}

/// Dart `SourceChange`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceChange {
    pub message: String,
    pub edits: Vec<SourceFileEdit>,
    pub linked_edit_groups: Vec<LinkedEditGroup>,
    pub selection: Option<Position>,
    pub selection_length: Option<u32>,
    pub id: Option<String>,
}

impl SourceChange {
    /// Dart `getFileEdit`.
    pub fn file_edit(&self, file: &str) -> Option<&SourceFileEdit> {
        self.edits.iter().find(|e| e.file == file)
    }
}
