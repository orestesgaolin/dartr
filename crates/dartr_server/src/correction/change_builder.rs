// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_core.dart (ChangeBuilderImpl, EditBuilderImpl, FileEditBuilderImpl, LinkedEditBuilderImpl)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (DartFileEditBuilderImpl: file edits, imports, finalize)

//! [ChangeBuilder]: builds a [SourceChange] from edits of files. The file
//! builders and the edit builders are views on the state of the change
//! builder ([ChangeData]), so that an edit builder can add imports or
//! linked edit groups while it writes.
//!
//! `commit` and `revert` (used by "fix all in file") save and restore a copy
//! of the state.

use std::rc::Rc;

use dartr_project::AnalysisOptions;
use indexmap::IndexMap;

use super::change::*;
use super::imports::LibraryImport;
use crate::server::ResolvedUnitRef;

/// Dart `ChangeWorkspace`: the resolved units and options of the files that
/// a change can edit.
pub trait ChangeWorkspace {
    /// The resolved unit of the Dart file [path], `None` when the file is
    /// not analyzed.
    fn resolved_unit(&mut self, path: &str) -> Option<ResolvedUnitRef>;

    /// The analysis options of [path].
    fn analysis_options(&self, path: &str) -> Rc<AnalysisOptions>;

    /// The content of a file (the overlay or the file on disk).
    fn content(&self, path: &str) -> Option<String>;
}

/// A position of a linked edit group that gets its offset when the change
/// is built (Dart `_PendingPosition`).
#[derive(Clone, Debug)]
struct PendingPosition {
    group: String,
    index: usize,
    /// The edit that contains the position, and the offset in it.
    edit: Option<u64>,
    offset: u32,
}

/// The Dart part of a file builder (Dart `DartFileEditBuilderImpl`).
#[derive(Clone)]
pub struct DartFileData {
    pub resolved: Rc<ResolvedUnitRef>,
    /// The path of the library file, when this file is a part.
    pub library_path: Option<String>,
    pub create_edits_for_imports: bool,
    /// Dart `_librariesToImport`, by URI.
    pub libraries_to_import: IndexMap<String, LibraryImport>,
    /// Dart `_elementLibrariesToImport`, by element.
    pub element_libraries_to_import: IndexMap<dartr_element::ElementId, String>,
    pub file_header: Option<String>,
}

/// The state of one file builder (Dart `FileEditBuilderImpl`).
#[derive(Clone)]
pub struct FileData {
    pub file_edit: SourceFileEdit,
    pub eol: String,
    pending_positions: Vec<PendingPosition>,
    pub dart: Option<DartFileData>,
    /// The content of the file (for generic edits).
    pub content: Option<String>,
}

impl FileData {
    pub fn has_edits(&self) -> bool {
        !self.file_edit.edits.is_empty()
            || self
                .dart
                .as_ref()
                .is_some_and(|d| !d.libraries_to_import.is_empty() || d.file_header.is_some())
    }

    /// Dart `_deltaToEdit`: the length change of the edits before
    /// (at lower offsets than) the edit [id].
    fn delta_to_edit(&self, id: u64) -> i64 {
        let edits = &self.file_edit.edits;
        let Some(index) = edits.iter().position(|e| e.id == id) else {
            return 0;
        };
        edits[index + 1..].iter().map(SourceEdit::delta).sum()
    }

    /// Dart `_deltaToOffset`.
    fn delta_to_offset(&self, offset: u32) -> i64 {
        self.file_edit
            .edits
            .iter()
            .filter(|e| e.offset <= offset)
            .map(SourceEdit::delta)
            .sum()
    }
}

/// The state of a change builder that `commit`/`revert` save.
#[derive(Clone, Default)]
pub struct ChangeData {
    pub linked_edit_groups: IndexMap<String, LinkedEditGroup>,
    pub selection: Option<Position>,
    /// Dart `_selectionRange` (offset, length).
    pub selection_range: Option<(u32, u32)>,
    pub files: IndexMap<String, FileData>,
    pub modification_count: usize,
}

/// Dart `ChangeBuilderImpl`.
pub struct ChangeBuilder<'w> {
    pub workspace: &'w mut dyn ChangeWorkspace,
    pub data: ChangeData,
    committed: ChangeData,
    pub default_eol: String,
    next_edit_id: u64,
    /// Set when an edit overlaps an existing edit (Dart throws a
    /// `ConflictingEditException`); the edit is dropped.
    pub conflict: bool,
}

/// Dart `String.endOfLine`: the first line terminator of [content].
pub fn end_of_line(content: &str) -> Option<&'static str> {
    let index = content.find('\n')?;
    if index > 0 && content.as_bytes()[index - 1] == b'\r' {
        Some("\r\n")
    } else {
        Some("\n")
    }
}

impl<'w> ChangeBuilder<'w> {
    pub fn new(workspace: &'w mut dyn ChangeWorkspace, default_eol: Option<&str>) -> Self {
        ChangeBuilder {
            workspace,
            data: ChangeData::default(),
            committed: ChangeData::default(),
            default_eol: default_eol.unwrap_or("\n").to_string(),
            next_edit_id: 1,
            conflict: false,
        }
    }

    /// Dart `hasEdits`: whether a file builder was created.
    pub fn has_edits(&self) -> bool {
        !self.data.files.is_empty()
    }

    pub fn has_edits_for(&self, path: &str) -> bool {
        self.data.files.contains_key(path)
    }

    /// Dart `commit`.
    pub fn commit(&mut self) {
        self.committed = self.data.clone();
    }

    /// Dart `revert`.
    pub fn revert(&mut self) {
        self.data = self.committed.clone();
    }

    /// Dart `addDartFileEdit`. Returns `false` when the file cannot be
    /// edited (it is not analyzed); [build] is not called then.
    pub fn add_dart_file_edit(
        &mut self,
        path: &str,
        build: impl FnOnce(&mut FileEditBuilder<'_, 'w>),
    ) -> bool {
        self.add_dart_file_edit_with(path, true, build)
    }

    /// [Self::add_dart_file_edit] with Dart `createEditsForImports`.
    pub fn add_dart_file_edit_with(
        &mut self,
        path: &str,
        create_edits_for_imports: bool,
        build: impl FnOnce(&mut FileEditBuilder<'_, 'w>),
    ) -> bool {
        if !self.ensure_dart_file(path, create_edits_for_imports) {
            return false;
        }
        let mut builder = FileEditBuilder {
            change: self,
            path: path.to_string(),
        };
        build(&mut builder);
        true
    }

    /// Dart `_createDartFileEditBuilder`: creates the builder of [path]
    /// (and of its library, for a part). Returns `false` when the file is
    /// not analyzed.
    fn ensure_dart_file(&mut self, path: &str, create_edits_for_imports: bool) -> bool {
        if self.data.files.contains_key(path) {
            return true;
        }
        let Some(resolved) = self.workspace.resolved_unit(path) else {
            return false;
        };
        let library_path = {
            let first = resolved
                .library
                .library
                .units
                .first()
                .map(|u| u.path.to_string());
            first.filter(|p| p != path)
        };
        if let Some(library_path) = &library_path {
            self.ensure_dart_file(library_path, create_edits_for_imports);
        }
        let content = resolved.unit().ast.tokens.source.to_string();
        let eol = end_of_line(&content)
            .map(str::to_string)
            .unwrap_or_else(|| self.default_eol.clone());
        self.data.files.insert(
            path.to_string(),
            FileData {
                file_edit: SourceFileEdit::new(path, 0),
                eol,
                pending_positions: Vec::new(),
                dart: Some(DartFileData {
                    resolved: Rc::new(resolved),
                    library_path,
                    create_edits_for_imports,
                    libraries_to_import: IndexMap::new(),
                    element_libraries_to_import: IndexMap::new(),
                    file_header: None,
                }),
                content: Some(content),
            },
        );
        true
    }

    /// Dart `addGenericFileEdit` (and `addYamlFileEdit`): edits of a file
    /// that is not a Dart file.
    pub fn add_generic_file_edit(
        &mut self,
        path: &str,
        build: impl FnOnce(&mut FileEditBuilder<'_, 'w>),
    ) {
        if !self.data.files.contains_key(path) {
            let content = self.workspace.content(path);
            let eol = content
                .as_deref()
                .and_then(end_of_line)
                .map(str::to_string)
                .unwrap_or_else(|| self.default_eol.clone());
            self.data.files.insert(
                path.to_string(),
                FileData {
                    file_edit: SourceFileEdit::new(path, 0),
                    eol,
                    pending_positions: Vec::new(),
                    dart: None,
                    content,
                },
            );
        }
        let mut builder = FileEditBuilder {
            change: self,
            path: path.to_string(),
        };
        build(&mut builder);
    }

    /// Dart `getLinkedEditGroup`.
    fn linked_edit_group(&mut self, name: &str) -> &mut LinkedEditGroup {
        self.data
            .linked_edit_groups
            .entry(name.to_string())
            .or_default()
    }

    /// Dart `setSelection`.
    pub fn set_selection(&mut self, position: Position) {
        self.data.selection = Some(position);
        self.data.selection_range = None;
    }

    /// Dart `_setSelectionRange`.
    fn set_selection_range(&mut self, offset: u32, length: u32) {
        self.data.selection_range = Some((offset, length));
        if let Some(selection) = &mut self.data.selection {
            selection.offset = offset;
        }
    }

    /// Dart `_updatePositions`.
    fn update_positions(&mut self, file: &str, offset: u32, delta: i64) {
        if let Some(selection) = &mut self.data.selection {
            if selection.file == file && selection.offset >= offset {
                selection.offset = (selection.offset as i64 + delta) as u32;
            }
        }
        if let Some((o, l)) = self.data.selection_range {
            if o >= offset {
                self.data.selection_range = Some(((o as i64 + delta) as u32, l));
            }
        }
    }

    /// Dart `sourceChange` (finalizes the file builders: imports and
    /// linked positions).
    pub fn source_change(&mut self) -> SourceChange {
        let paths: Vec<String> = self.data.files.keys().cloned().collect();
        let mut change = SourceChange::default();
        for path in &paths {
            if !self.data.files[path].has_edits() {
                continue;
            }
            self.finalize_file(path);
            let file = &self.data.files[path];
            change.edits.push(file.file_edit.clone());
        }
        change.linked_edit_groups = self.data.linked_edit_groups.values().cloned().collect();
        if let Some(selection) = &self.data.selection {
            change.selection = Some(selection.clone());
            if let Some((_, length)) = self.data.selection_range {
                change.selection_length = Some(length);
            }
        }
        change
    }

    /// Dart `DartFileEditBuilderImpl.finalize` and
    /// `FileEditBuilderImpl.finalize`.
    fn finalize_file(&mut self, path: &str) {
        let dart = self.data.files[path].dart.clone();
        if let Some(dart) = dart {
            if dart.create_edits_for_imports && !dart.libraries_to_import.is_empty() {
                let imports: Vec<LibraryImport> =
                    dart.libraries_to_import.values().cloned().collect();
                let mut builder = FileEditBuilder {
                    change: self,
                    path: path.to_string(),
                };
                super::imports::add_library_imports(&mut builder, imports);
            }
            if let Some(header) = &dart.file_header {
                let mut builder = FileEditBuilder {
                    change: self,
                    path: path.to_string(),
                };
                let header = header.clone();
                builder.add_insertion_with(0, true, |b| b.writeln(&header));
            }
        }
        let file = self.data.files.get_mut(path).unwrap();
        let pending = std::mem::take(&mut file.pending_positions);
        for p in &pending {
            let offset = match p.edit {
                Some(id) => match file.file_edit.edits.iter().find(|e| e.id == id) {
                    Some(edit) => {
                        Some((edit.offset as i64 + file.delta_to_edit(id) + p.offset as i64) as u32)
                    }
                    None => None,
                },
                None => Some((p.offset as i64 + file.delta_to_offset(p.offset)) as u32),
            };
            if let Some(offset) = offset {
                if let Some(group) = self.data.linked_edit_groups.get_mut(&p.group) {
                    if let Some(position) = group.positions.get_mut(p.index) {
                        position.offset = offset;
                    }
                }
            }
        }
        // Dart keeps the pending positions; a second `sourceChange` would
        // finalize again from the same pending data.
        self.data.files.get_mut(path).unwrap().pending_positions = pending;
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_edit_id;
        self.next_edit_id += 1;
        id
    }
}

/// A view on the builder of one file (Dart `FileEditBuilderImpl` and
/// `DartFileEditBuilderImpl`).
pub struct FileEditBuilder<'c, 'w> {
    pub change: &'c mut ChangeBuilder<'w>,
    pub path: String,
}

/// A pending linked edit of an edit builder: group name, start and length
/// in the edit, suggestions.
#[derive(Clone, Debug)]
struct PendingLinkedEdit {
    group: String,
    start: u32,
    length: u32,
    suggestions: Vec<LinkedEditSuggestion>,
}

/// Dart `EditBuilderImpl` / `DartEditBuilderImpl`: writes the replacement
/// text of one edit.
pub struct EditBuilder<'f, 'c, 'w> {
    pub file: &'f mut FileEditBuilder<'c, 'w>,
    pub offset: u32,
    pub length: u32,
    buffer: String,
    buffer_len16: u32,
    selection_range: Option<(u32, u32)>,
    linked_edits: Vec<PendingLinkedEdit>,
    is_writing_edit_group: bool,
    /// Suggestions of the linked edit that is being written.
    current_suggestions: Vec<LinkedEditSuggestion>,
}

impl<'c, 'w> FileEditBuilder<'c, 'w> {
    pub fn data(&self) -> &FileData {
        &self.change.data.files[&self.path]
    }

    pub fn data_mut(&mut self) -> &mut FileData {
        self.change.data.files.get_mut(&self.path).unwrap()
    }

    pub fn eol(&self) -> String {
        self.data().eol.clone()
    }

    /// The resolved unit of a Dart file builder.
    pub fn resolved(&self) -> Rc<ResolvedUnitRef> {
        self.data()
            .dart
            .as_ref()
            .expect("a Dart file")
            .resolved
            .clone()
    }

    pub fn has_edits(&self) -> bool {
        self.data().has_edits()
    }

    fn create_edit_builder<'f>(&'f mut self, offset: u32, length: u32) -> EditBuilder<'f, 'c, 'w> {
        EditBuilder {
            file: self,
            offset,
            length,
            buffer: String::new(),
            buffer_len16: 0,
            selection_range: None,
            linked_edits: Vec::new(),
            is_writing_edit_group: false,
            current_suggestions: Vec::new(),
        }
    }

    /// Dart `addDeletion`.
    pub fn add_deletion(&mut self, offset: u32, length: u32) {
        if length > 0 {
            let builder = self.create_edit_builder(offset, length);
            let parts = builder.into_parts();
            self.add_edit_builder(parts, false);
        }
    }

    /// Dart `addInsertion`.
    pub fn add_insertion(&mut self, offset: u32, build: impl FnOnce(&mut EditBuilder<'_, 'c, 'w>)) {
        self.add_insertion_with(offset, false, build)
    }

    /// Dart `addInsertion` with `insertBeforeExisting`.
    pub fn add_insertion_with(
        &mut self,
        offset: u32,
        insert_before_existing: bool,
        build: impl FnOnce(&mut EditBuilder<'_, 'c, 'w>),
    ) {
        let mut builder = self.create_edit_builder(offset, 0);
        build(&mut builder);
        let parts = builder.into_parts();
        self.add_edit_builder(parts, insert_before_existing);
    }

    /// Dart `addReplacement`.
    pub fn add_replacement(
        &mut self,
        offset: u32,
        length: u32,
        build: impl FnOnce(&mut EditBuilder<'_, 'c, 'w>),
    ) {
        let mut builder = self.create_edit_builder(offset, length);
        build(&mut builder);
        let parts = builder.into_parts();
        self.add_edit_builder(parts, false);
    }

    /// Dart `addSimpleInsertion`.
    pub fn add_simple_insertion(&mut self, offset: u32, text: &str) {
        self.add_replacement(offset, 0, |b| b.write(text));
    }

    /// Dart `addSimpleReplacement`.
    pub fn add_simple_replacement(&mut self, offset: u32, length: u32, text: &str) {
        self.add_replacement(offset, length, |b| b.write(text));
    }

    /// Dart `addLinkedPosition`.
    pub fn add_linked_position(&mut self, offset: u32, length: u32, group_name: &str) {
        let file = self.path.clone();
        let group = self.change.linked_edit_group(group_name);
        group.add_position(Position { file, offset }, length);
        let index = group.positions.len() - 1;
        self.data_mut().pending_positions.push(PendingPosition {
            group: group_name.to_string(),
            index,
            edit: None,
            offset,
        });
    }

    /// Dart `_addEditBuilder` and `_addEdit`. A conflicting edit is
    /// recorded as [ConflictingEdit] in the change (Dart throws).
    fn add_edit_builder(&mut self, parts: EditParts, insert_before_existing: bool) {
        let id = self.change.next_id();
        let edit = SourceEdit {
            offset: parts.offset,
            length: parts.length,
            replacement: parts.buffer,
            id,
        };
        let delta = edit.delta();
        let edit_offset = edit.offset;
        if self
            .data_mut()
            .file_edit
            .add(edit, insert_before_existing)
            .is_err()
        {
            self.change.conflict = true;
            return;
        }
        let file = self.path.clone();
        self.change.update_positions(&file, edit_offset, delta);
        self.change.data.modification_count += 1;
        // Linked edits of the builder (Dart `addLinkedEdit` finally block).
        for linked in parts.linked_edits {
            let position_offset = parts.offset + linked.start;
            let group = self.change.linked_edit_group(&linked.group);
            group.add_position(
                Position {
                    file: file.clone(),
                    offset: position_offset,
                },
                linked.length,
            );
            let index = group.positions.len() - 1;
            for s in linked.suggestions {
                group.add_suggestion(s);
            }
            self.data_mut().pending_positions.push(PendingPosition {
                group: linked.group,
                index,
                edit: Some(id),
                offset: linked.start,
            });
        }
        // Dart `_captureSelection`.
        if let Some((offset, length)) = parts.selection_range {
            let delta = self.data().delta_to_edit(id);
            let offset = (offset as i64 + delta) as u32;
            self.change.set_selection(Position { file, offset });
            self.change.set_selection_range(offset, length);
        }
    }
}

/// The data of a finished edit builder.
struct EditParts {
    offset: u32,
    length: u32,
    buffer: String,
    selection_range: Option<(u32, u32)>,
    linked_edits: Vec<PendingLinkedEdit>,
}

impl<'f, 'c, 'w> EditBuilder<'f, 'c, 'w> {
    fn into_parts(self) -> EditParts {
        EditParts {
            offset: self.offset,
            length: self.length,
            buffer: self.buffer,
            selection_range: self.selection_range,
            linked_edits: self.linked_edits,
        }
    }

    pub fn eol(&self) -> String {
        self.file.eol()
    }

    /// The UTF-16 length of the text written so far.
    pub fn written_length(&self) -> u32 {
        self.buffer_len16
    }

    /// Dart `write`.
    pub fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
        self.buffer_len16 += utf16_len(s);
    }

    /// Dart `writeln`.
    pub fn writeln(&mut self, s: &str) {
        self.write(s);
        let eol = self.eol();
        self.write(&eol);
    }

    /// Dart `writeln()` without text.
    pub fn newline(&mut self) {
        let eol = self.eol();
        self.write(&eol);
    }

    /// Dart `selectHere`.
    pub fn select_here(&mut self) {
        self.selection_range = Some((self.offset + self.buffer_len16, 0));
    }

    /// Dart `selectAll`.
    pub fn select_all(&mut self, writer: impl FnOnce(&mut Self)) {
        let start = self.buffer_len16;
        writer(self);
        let length = self.buffer_len16 - start;
        self.selection_range = Some((self.offset + start, length));
    }

    /// Dart `addLinkedEdit`: [build] writes the text of the group and adds
    /// suggestions with [Self::add_suggestion].
    pub fn add_linked_edit(&mut self, group_name: &str, build: impl FnOnce(&mut Self)) {
        let start = self.buffer_len16;
        if self.is_writing_edit_group {
            build(self);
            return;
        }
        self.is_writing_edit_group = true;
        let outer = std::mem::take(&mut self.current_suggestions);
        build(self);
        self.is_writing_edit_group = false;
        let suggestions = std::mem::replace(&mut self.current_suggestions, outer);
        let length = self.buffer_len16 - start;
        if length != 0 {
            self.linked_edits.push(PendingLinkedEdit {
                group: group_name.to_string(),
                start,
                length,
                suggestions,
            });
        }
    }

    /// Dart `LinkedEditBuilder.addSuggestion`.
    pub fn add_suggestion(&mut self, kind: LinkedEditSuggestionKind, value: &str) {
        self.current_suggestions.push(LinkedEditSuggestion {
            value: value.to_string(),
            kind,
        });
    }

    /// Dart `addSimpleLinkedEdit`.
    pub fn add_simple_linked_edit(
        &mut self,
        group_name: &str,
        text: &str,
        suggestions: Option<(LinkedEditSuggestionKind, &[String])>,
    ) {
        self.add_linked_edit(group_name, |b| {
            b.write(text);
            if let Some((kind, values)) = suggestions {
                for v in values {
                    b.add_suggestion(kind, v);
                }
            }
        });
    }

    /// Dart `getIndent`.
    pub fn get_indent(level: usize) -> String {
        "  ".repeat(level)
    }

    /// Dart `writeIndent`.
    pub fn write_indent(&mut self, level: usize) {
        self.write(&Self::get_indent(level));
    }
}
