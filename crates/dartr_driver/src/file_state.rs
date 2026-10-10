// Dart source: pkg/analyzer/lib/src/dart/analysis/file_state.dart
// (FileState, FileSystemState, FileKind, LibraryFileKind, PartOfNameFileKind,
// PartOfUriKnownFileKind, PartOfUriUnknownFileKind, DirectiveUri*,
// LibraryImportState, LibraryExportState, PartIncludeState,
// FileUriProperties), pkg/analyzer/lib/src/util/uri.dart
// (rewriteToCanonicalUri)

//! [`FileSystemState`]: the known files, their content, parsed AST,
//! unlinked data and kind, and the resolved directives.
//!
//! Differences to the Dart code:
//! - Files are stored in a vector and referenced by [`FileId`]; the Dart
//!   object graph (`referencingFiles`, directive states) uses ids.
//! - Directive states are computed when a file is added, not on first
//!   access; [`FileSystemState::discover`] then reads and parses all
//!   referenced files in parallel waves (rayon). The set of files is the
//!   same as in Dart after `discoverReferencedFiles`.
//! - The parsed unit is kept (Dart parses the file again for linking).
//! - There is no byte store. [`FileSystemState::change_file`] is Dart
//!   `changeFile` + `refresh` for one file (design §4.2 v1).

use std::sync::Arc;

use dartr_ast_builder::{ParsedUnit, parse_file};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_project::{DartSdk, Workspace, paths};
use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;

use crate::api_signature::{ApiSignature, hex};
use crate::unlinked_data::{
    ConfigurableUriDirective, UnlinkedLibraryExportDirective, UnlinkedLibraryImportDirective,
    UnlinkedPartDirective, UnlinkedUnit, serialize_ast_unlinked2,
};
use crate::uri::Uri;

/// Index of a file in [`FileSystemState`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct FileId(pub u32);

/// Dart `SourceFactory` of a context: `dart:` URIs through the SDK,
/// `package:` and `file:` URIs through the workspace.
#[derive(Clone, Debug)]
pub struct SourceFactory {
    pub workspace: Workspace,
    pub sdk: Option<DartSdk>,
}

impl SourceFactory {
    /// Dart `forUri2(uri)?.fullName`.
    pub fn for_uri(&self, uri: &Uri) -> Option<String> {
        let path = self
            .workspace
            .resolve_uri(&uri.to_string(), self.sdk.as_ref())?;
        Some(path)
    }

    /// Dart `pathToUri(path)`.
    pub fn path_to_uri(&self, path: &str) -> String {
        self.workspace.path_to_uri(path, self.sdk.as_ref())
    }
}

/// The language version and experiments of a file (Dart
/// `FileState.packageLanguageVersion` and `featureSet`).
#[derive(Clone, Debug)]
pub struct FileConfig {
    pub package_language_version: (u32, u32),
    pub experiments: Vec<ExperimentalFlag>,
}

/// The language version and experiments of a file, by path and URI.
/// `Send + Sync`, so that a driver can move to the threads of a pool.
pub type ConfigFor = Box<dyn Fn(&str, &str) -> FileConfig + Send + Sync>;

/// A file to read and parse: id, path, whether it is `dart:core`, config,
/// salt.
type ParseJob = (FileId, Arc<str>, bool, FileConfig, Vec<u32>);

/// Dart `DirectiveUri` (no `DirectiveUriWithInSummarySource`: there are no
/// summary inputs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectiveUri {
    /// `DirectiveUriWithoutString`.
    WithoutString,
    /// `DirectiveUriWithString`.
    WithString { relative_uri_str: Arc<str> },
    /// `DirectiveUriWithUri`.
    WithUri {
        relative_uri_str: Arc<str>,
        relative_uri: Arc<str>,
    },
    /// `DirectiveUriWithFile`.
    WithFile {
        relative_uri_str: Arc<str>,
        relative_uri: Arc<str>,
        file: FileId,
    },
}

/// Dart `DirectiveUris`.
#[derive(Clone, Debug)]
pub struct DirectiveUris {
    pub primary: DirectiveUri,
    pub configurations: Vec<DirectiveUri>,
    pub selected: DirectiveUri,
}

/// Dart `LibraryImportState`.
#[derive(Clone, Debug)]
pub struct LibraryImportState {
    pub unlinked: UnlinkedLibraryImportDirective,
    pub uris: DirectiveUris,
}

/// Dart `LibraryExportState`.
#[derive(Clone, Debug)]
pub struct LibraryExportState {
    pub unlinked: UnlinkedLibraryExportDirective,
    pub uris: DirectiveUris,
}

/// Dart `PartIncludeState`.
#[derive(Clone, Debug)]
pub struct PartIncludeState {
    pub unlinked: UnlinkedPartDirective,
    pub uris: DirectiveUris,
}

/// The kind of a file (Dart `FileKind` subclasses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// `LibraryFileKind`.
    Library { name: Option<String> },
    /// `PartOfNameFileKind`.
    PartOfName { name: String },
    /// `PartOfUriKnownFileKind`.
    PartOfUriKnown { uri_file: FileId },
    /// `PartOfUriUnknownFileKind`.
    PartOfUriUnknown,
}

impl FileKind {
    pub fn is_library(&self) -> bool {
        matches!(self, FileKind::Library { .. })
    }

    pub fn is_part(&self) -> bool {
        !self.is_library()
    }
}

/// Dart `FileUriProperties`.
#[derive(Clone, Debug, Default)]
pub struct FileUriProperties {
    pub is_dart: bool,
    pub is_dart_internal: bool,
    pub is_src: bool,
    pub package_name: Option<String>,
}

impl FileUriProperties {
    pub fn new(uri: &Uri) -> FileUriProperties {
        if uri.scheme == "dart" {
            let name = uri.path.split('/').next().unwrap_or("");
            return FileUriProperties {
                is_dart: true,
                is_dart_internal: name.starts_with('_'),
                ..Default::default()
            };
        }
        if uri.scheme == "package" {
            let segments: Vec<&str> = uri.path.split('/').collect();
            if segments.len() >= 2 {
                return FileUriProperties {
                    package_name: Some(segments[0].to_string()),
                    is_src: segments[1] == "src",
                    ..Default::default()
                };
            }
        }
        FileUriProperties::default()
    }
}

/// One file (Dart `FileState`).
#[derive(Debug)]
pub struct FileState {
    pub id: FileId,
    pub path: Arc<str>,
    pub uri: Uri,
    pub uri_str: Arc<str>,
    pub uri_properties: FileUriProperties,
    pub config: FileConfig,
    /// Set by [`FileSystemState::discover`].
    pub content: Option<FileContent>,
    /// Dart `FileState.referencingFiles`.
    pub referencing_files: IndexSet<FileId>,
}

/// The data computed from the content of a file.
#[derive(Debug)]
pub struct FileContent {
    pub exists: bool,
    /// Dart `contentHash`: the MD5 of the content, hex.
    pub content_hash: String,
    /// Dart `unlinkedKey` (without the `.unlinked2` suffix).
    pub unlinked_signature: [u8; 16],
    pub parsed: Arc<ParsedUnit>,
    pub unlinked: UnlinkedUnit,
    pub kind: FileKind,
    /// Dart `FileKind.libraryImports`.
    pub library_imports: Vec<LibraryImportState>,
    /// Dart `FileKind.libraryExports`.
    pub library_exports: Vec<LibraryExportState>,
    /// Dart `FileKind.partIncludes`.
    pub part_includes: Vec<PartIncludeState>,
    /// Dart `FileKind.docLibraryImports`.
    pub doc_library_imports: Vec<LibraryImportState>,
}

impl FileState {
    /// The content data; panics before [`FileSystemState::discover`].
    pub fn c(&self) -> &FileContent {
        self.content.as_ref().expect("file is not discovered")
    }

    pub fn exists(&self) -> bool {
        self.c().exists
    }

    /// Dart `FileState.apiSignature`.
    pub fn api_signature(&self) -> &[u8; 16] {
        &self.c().unlinked.api_signature
    }

    pub fn kind(&self) -> &FileKind {
        &self.c().kind
    }
}

/// Dart `FileSystemState` of one analysis context.
pub struct FileSystemState {
    pub source_factory: SourceFactory,
    /// Dart `DeclaredVariables` (`-D` values) for configurable imports.
    pub declared_variables: IndexMap<String, String>,
    /// Dart `_saltForUnlinked`.
    pub salt_for_unlinked: Vec<u32>,
    /// Dart `_saltForElements`.
    pub salt_for_elements: Vec<u32>,
    files: Vec<FileState>,
    path_to_file: IndexMap<Arc<str>, FileId>,
    uri_to_file: IndexMap<Arc<str>, FileId>,
    /// Dart `_libraryNameToFiles`.
    library_name_to_files: IndexMap<String, Vec<FileId>>,
    /// The language version and experiments of a file (path, URI).
    config_for: ConfigFor,
    /// Files that are added but not read and parsed yet.
    pending: Vec<FileId>,
}

impl FileSystemState {
    pub fn new(source_factory: SourceFactory, config_for: ConfigFor) -> FileSystemState {
        FileSystemState {
            source_factory,
            declared_variables: IndexMap::new(),
            salt_for_unlinked: Vec::new(),
            salt_for_elements: Vec::new(),
            files: Vec::new(),
            path_to_file: IndexMap::new(),
            uri_to_file: IndexMap::new(),
            library_name_to_files: IndexMap::new(),
            config_for,
            pending: Vec::new(),
        }
    }

    pub fn file(&self, id: FileId) -> &FileState {
        &self.files[id.0 as usize]
    }

    pub fn files(&self) -> &[FileState] {
        &self.files
    }

    /// Dart `getExistingFromPath`.
    pub fn get_existing_from_path(&self, path: &str) -> Option<FileId> {
        self.path_to_file.get(path).copied()
    }

    /// Dart `getFileForPath`. The file is read with the next
    /// [`Self::discover`].
    pub fn get_file_for_path(&mut self, path: &str) -> FileId {
        if let Some(&id) = self.path_to_file.get(path) {
            return id;
        }
        let uri_str = self.source_factory.path_to_uri(path);
        let uri = Uri::try_parse(&uri_str).unwrap_or_default();
        self.new_file(path, uri)
    }

    /// Dart `getFileForUri`: `None` when the URI cannot be resolved to a
    /// path.
    pub fn get_file_for_uri(&mut self, uri: &Uri) -> Option<FileId> {
        let uri_str = uri.to_string();
        if let Some(&id) = self.uri_to_file.get(uri_str.as_str()) {
            return Some(id);
        }
        let path = self.source_factory.for_uri(uri)?;
        if let Some(&id) = self.path_to_file.get(path.as_str()) {
            return Some(id);
        }
        // Dart `rewriteToCanonicalUri`.
        let rewritten = self.source_factory.path_to_uri(&path);
        let rewritten = Uri::try_parse(&rewritten)?;
        Some(self.new_file(&path, rewritten))
    }

    /// Dart `_newFile` (without the refresh, which [`Self::discover`] does).
    fn new_file(&mut self, path: &str, uri: Uri) -> FileId {
        let id = FileId(self.files.len() as u32);
        let uri_str: Arc<str> = uri.to_string().into();
        let path: Arc<str> = path.into();
        let config = (self.config_for)(&path, &uri_str);
        self.files.push(FileState {
            id,
            path: path.clone(),
            uri_properties: FileUriProperties::new(&uri),
            uri,
            uri_str: uri_str.clone(),
            config,
            content: None,
            referencing_files: IndexSet::new(),
        });
        self.path_to_file.insert(path, id);
        self.uri_to_file.insert(uri_str, id);
        self.pending.push(id);
        id
    }

    /// Reads, parses and computes the unlinked data of every pending file in
    /// parallel, resolves their directives (which can add files), and
    /// repeats until no file is pending. Dart does the same lazily
    /// (`refresh`, `discoverReferencedFiles`).
    pub fn discover(&mut self) {
        while !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            let jobs: Vec<ParseJob> = pending
                .iter()
                .map(|&id| {
                    let f = self.file(id);
                    (
                        id,
                        f.path.clone(),
                        &*f.uri_str == "dart:core",
                        f.config.clone(),
                        self.salt_for_unlinked.clone(),
                    )
                })
                .collect();
            let results: Vec<(FileId, RefreshData)> = jobs
                .into_par_iter()
                .map(|(id, path, is_dart_core, config, salt)| {
                    (id, refresh_data(&path, is_dart_core, &config, &salt))
                })
                .collect();
            for (id, data) in results {
                self.apply_refresh(id, data);
            }
        }
    }

    /// Dart `changeFile` + `FileState.refresh` for a known file: reads and
    /// parses [file] again and updates its kind and directive states (new
    /// referenced files are discovered). Returns whether the API of the file
    /// changed (Dart: `apiSignature` differs, or the file was created or
    /// deleted); `false` means only function bodies changed.
    pub fn change_file(&mut self, file: FileId) -> bool {
        if self.file(file).content.is_none() {
            // Not read yet (pending): read it now.
            self.discover();
            return true;
        }
        let f = self.file(file);
        let is_dart_core = &*f.uri_str == "dart:core";
        let data = refresh_data(&f.path, is_dart_core, &f.config, &self.salt_for_unlinked);
        let old = self.files[file.0 as usize]
            .content
            .take()
            .expect("checked above");
        let api_changed =
            old.exists != data.exists || old.unlinked.api_signature != data.unlinked.api_signature;
        // Undo the registrations of the old content (`_updateKind`,
        // `referencingFiles`).
        if let FileKind::Library { name: Some(name) } = &old.kind
            && let Some(files) = self.library_name_to_files.get_mut(name)
        {
            files.retain(|&f| f != file);
        }
        let referenced: Vec<FileId> = old
            .library_imports
            .iter()
            .map(|s| &s.uris.selected)
            .chain(old.library_exports.iter().map(|s| &s.uris.selected))
            .chain(old.part_includes.iter().map(|s| &s.uris.selected))
            .chain(old.doc_library_imports.iter().map(|s| &s.uris.selected))
            .filter_map(|u| match u {
                DirectiveUri::WithFile { file, .. } => Some(*file),
                _ => None,
            })
            .collect();
        for target in referenced {
            self.files[target.0 as usize]
                .referencing_files
                .shift_remove(&file);
        }
        self.apply_refresh(file, data);
        self.discover();
        api_changed
    }

    /// The second half of Dart `refresh`: `_updateKind` and the directive
    /// states (with `referencingFiles`).
    fn apply_refresh(&mut self, id: FileId, data: RefreshData) {
        let unlinked = data.unlinked;
        let kind = if let Some(library) = &unlinked.library_directive {
            FileKind::Library {
                name: library.name.clone(),
            }
        } else if let Some(part_of_name) = &unlinked.part_of_name_directive {
            FileKind::PartOfName {
                name: part_of_name.name.clone(),
            }
        } else if let Some(part_of_uri) = &unlinked.part_of_uri_directive {
            match self.file_for_relative_uri(id, part_of_uri.uri.as_deref()) {
                Some(uri_file) => FileKind::PartOfUriKnown { uri_file },
                None => FileKind::PartOfUriUnknown,
            }
        } else {
            FileKind::Library { name: None }
        };
        if let FileKind::Library { name: Some(name) } = &kind {
            self.library_name_to_files
                .entry(name.clone())
                .or_default()
                .push(id);
        }

        let mut library_imports: Vec<LibraryImportState> = unlinked
            .imports
            .iter()
            .map(|u| LibraryImportState {
                uris: self.build_configurable_directive_uris(id, u),
                unlinked: u.clone(),
            })
            .collect();
        if kind.is_library() && !unlinked.is_dart_core && !unlinked.has_dart_core_import {
            let u = UnlinkedLibraryImportDirective {
                combinators: Vec::new(),
                configurations: Vec::new(),
                import_keyword_offset: -1,
                is_doc_import: false,
                is_synthetic_dart_core: true,
                prefix: None,
                uri: Some("dart:core".to_string()),
            };
            library_imports.push(LibraryImportState {
                uris: self.build_configurable_directive_uris(id, &u),
                unlinked: u,
            });
        }
        let library_exports: Vec<LibraryExportState> = unlinked
            .exports
            .iter()
            .map(|u| LibraryExportState {
                uris: self.build_configurable_directive_uris(id, u),
                unlinked: u.clone(),
            })
            .collect();
        let part_includes: Vec<PartIncludeState> = unlinked
            .parts
            .iter()
            .map(|u| PartIncludeState {
                uris: self.build_configurable_directive_uris(id, u),
                unlinked: u.clone(),
            })
            .collect();
        let doc_imports = match (&kind, &unlinked) {
            (FileKind::Library { .. }, u) => u
                .library_directive
                .as_ref()
                .map(|d| d.doc_imports.clone())
                .unwrap_or_default(),
            (FileKind::PartOfName { .. }, u) => u
                .part_of_name_directive
                .as_ref()
                .map(|d| d.doc_imports.clone())
                .unwrap_or_default(),
            (_, u) => u
                .part_of_uri_directive
                .as_ref()
                .map(|d| d.doc_imports.clone())
                .unwrap_or_default(),
        };
        let doc_library_imports: Vec<LibraryImportState> = doc_imports
            .iter()
            .map(|u| LibraryImportState {
                uris: self.build_configurable_directive_uris(id, u),
                unlinked: u.clone(),
            })
            .collect();

        // `referencingFiles` of the directive states with a file.
        let referenced: Vec<FileId> = library_imports
            .iter()
            .map(|s| &s.uris.selected)
            .chain(library_exports.iter().map(|s| &s.uris.selected))
            .chain(part_includes.iter().map(|s| &s.uris.selected))
            .chain(doc_library_imports.iter().map(|s| &s.uris.selected))
            .filter_map(|u| match u {
                DirectiveUri::WithFile { file, .. } => Some(*file),
                _ => None,
            })
            .collect();
        for target in referenced {
            self.files[target.0 as usize].referencing_files.insert(id);
        }

        self.files[id.0 as usize].content = Some(FileContent {
            exists: data.exists,
            content_hash: data.content_hash,
            unlinked_signature: data.unlinked_signature,
            parsed: data.parsed,
            unlinked,
            kind,
            library_imports,
            library_exports,
            part_includes,
            doc_library_imports,
        });
    }

    /// Dart `_buildConfigurableDirectiveUris`.
    fn build_configurable_directive_uris(
        &mut self,
        container: FileId,
        directive: &dyn ConfigurableUriDirective,
    ) -> DirectiveUris {
        let primary = self.build_directive_uri(container, directive.uri());
        let mut selected = None;
        let mut configurations = Vec::new();
        for configuration in directive.configurations() {
            let uri = self.build_directive_uri(container, configuration.uri.as_deref());
            let value = configuration.value_or_true();
            if selected.is_none()
                && self
                    .declared_variables
                    .get(&configuration.name)
                    .map(String::as_str)
                    == Some(value)
            {
                selected = Some(uri.clone());
            }
            configurations.push(uri);
        }
        DirectiveUris {
            selected: selected.unwrap_or_else(|| primary.clone()),
            primary,
            configurations,
        }
    }

    /// Dart `_buildDirectiveUri`.
    fn build_directive_uri(
        &mut self,
        container: FileId,
        relative_uri_str: Option<&str>,
    ) -> DirectiveUri {
        let Some(relative_uri_str) = relative_uri_str else {
            return DirectiveUri::WithoutString;
        };
        let Some(relative_uri) = Uri::try_parse(relative_uri_str) else {
            return DirectiveUri::WithString {
                relative_uri_str: relative_uri_str.into(),
            };
        };
        let absolute = Uri::resolve_relative(&self.file(container).uri, &relative_uri);
        match self.get_file_for_uri(&absolute) {
            None => DirectiveUri::WithUri {
                relative_uri_str: relative_uri_str.into(),
                relative_uri: relative_uri.to_string().into(),
            },
            Some(file) => DirectiveUri::WithFile {
                relative_uri_str: relative_uri_str.into(),
                relative_uri: relative_uri.to_string().into(),
                file,
            },
        }
    }

    /// Dart `_fileForRelativeUri`.
    fn file_for_relative_uri(
        &mut self,
        container: FileId,
        relative_uri_str: Option<&str>,
    ) -> Option<FileId> {
        let relative = Uri::try_parse(relative_uri_str?)?;
        let absolute = Uri::resolve_relative(&self.file(container).uri, &relative);
        self.get_file_for_uri(&absolute)
    }

    // ---- kinds ----

    /// Dart `LibraryImportWithFile.importedLibrary` /
    /// `LibraryExportWithFile.exportedLibrary`: the library file of a
    /// directive URI.
    pub fn library_of_uri(&self, uri: &DirectiveUri) -> Option<FileId> {
        match uri {
            DirectiveUri::WithFile { file, .. } if self.file(*file).kind().is_library() => {
                Some(*file)
            }
            _ => None,
        }
    }

    /// Dart `PartFileKind.isPartOf(container)`.
    pub fn is_part_of(&self, part: FileId, container: FileId) -> bool {
        match self.file(part).kind() {
            FileKind::PartOfName { name } => match self.file(container).kind() {
                FileKind::Library { name: Some(n) } => n == name,
                _ => false,
            },
            FileKind::PartOfUriKnown { uri_file } => *uri_file == container,
            _ => false,
        }
    }

    /// Dart `PartIncludeWithFile.includedPart`.
    pub fn included_part(&self, container: FileId, state: &PartIncludeState) -> Option<FileId> {
        match &state.uris.selected {
            DirectiveUri::WithFile { file, .. }
                if self.file(*file).kind().is_part() && self.is_part_of(*file, container) =>
            {
                Some(*file)
            }
            _ => None,
        }
    }

    /// Dart `FileKind.hasPart`.
    pub fn has_part(&self, container: FileId, part: FileId) -> bool {
        self.file(container).c().part_includes.iter().any(
            |d| matches!(&d.uris.selected, DirectiveUri::WithFile { file, .. } if *file == part),
        )
    }

    /// Dart `FileKind.library`: the library of a library or part file.
    /// For a `part of name` file, the sibling files are read first when no
    /// library with that name is known (`discoverLibraries`).
    pub fn library_of(&mut self, file: FileId) -> Option<FileId> {
        match self.file(file).kind().clone() {
            FileKind::Library { .. } => Some(file),
            FileKind::PartOfName { name } => {
                if !self.library_name_to_files.contains_key(&name) {
                    self.discover_libraries(file);
                }
                let mut result: Option<FileId> = None;
                for &library in self.library_name_to_files.get(&name).into_iter().flatten() {
                    if self.has_part(library, file) {
                        match result {
                            None => result = Some(library),
                            Some(r) if self.file(library).path < self.file(r).path => {
                                result = Some(library)
                            }
                            _ => {}
                        }
                    }
                }
                result
            }
            FileKind::PartOfUriKnown { .. } => {
                let mut visited = IndexSet::new();
                let mut current = self.including_container(file);
                while let Some(c) = current {
                    if !visited.insert(c) {
                        break;
                    }
                    match self.file(c).kind() {
                        FileKind::Library { .. } => return Some(c),
                        FileKind::PartOfUriKnown { .. } => current = self.including_container(c),
                        _ => return None,
                    }
                }
                None
            }
            FileKind::PartOfUriUnknown => None,
        }
    }

    /// Dart `kind.library ?? kind.asLibrary` (`AnalysisDriver._analyzeFile`):
    /// a part file without a library is analyzed as a library of its own.
    /// Returns the library file: [file] itself or its library. Dart creates a
    /// separate `LibraryFileKind(recoveredFrom: kind)`; this port changes the
    /// kind of the file in place (with the synthetic `dart:core` import that
    /// `FileKind.libraryImports` adds for a library). Call [`Self::discover`]
    /// after it.
    pub fn library_or_as_library(&mut self, file: FileId) -> FileId {
        if let Some(library) = self.library_of(file) {
            return library;
        }
        let content = self.files[file.0 as usize].c();
        let add_dart_core =
            !content.unlinked.is_dart_core && !content.unlinked.has_dart_core_import;
        let core = add_dart_core.then(|| {
            let u = UnlinkedLibraryImportDirective {
                combinators: Vec::new(),
                configurations: Vec::new(),
                import_keyword_offset: -1,
                is_doc_import: false,
                is_synthetic_dart_core: true,
                prefix: None,
                uri: Some("dart:core".to_string()),
            };
            LibraryImportState {
                uris: self.build_configurable_directive_uris(file, &u),
                unlinked: u,
            }
        });
        if let Some(DirectiveUri::WithFile { file: target, .. }) =
            core.as_ref().map(|s| &s.uris.selected)
        {
            self.files[target.0 as usize].referencing_files.insert(file);
        }
        let content = self.files[file.0 as usize].content.as_mut().unwrap();
        content.kind = FileKind::Library { name: None };
        // `LibraryFileKind` reads doc imports from the library directive only.
        content.doc_library_imports.clear();
        content.library_imports.extend(core);
        file
    }

    /// Dart `PartOfUriKnownFileKind.includingContainer`.
    fn including_container(&self, file: FileId) -> Option<FileId> {
        let FileKind::PartOfUriKnown { uri_file } = *self.file(file).kind() else {
            return None;
        };
        if self.has_part(uri_file, file) {
            Some(uri_file)
        } else {
            None
        }
    }

    /// Dart `PartOfNameFileKind.discoverLibraries`.
    fn discover_libraries(&mut self, file: FileId) {
        let folder = paths::dirname(&self.file(file).path).to_string();
        let mut siblings: Vec<String> = dartr_project::fs::children(&folder)
            .unwrap_or_default()
            .into_iter()
            .filter(|c| paths::extension(&c.path) == ".dart")
            .map(|c| c.path)
            .collect();
        siblings.sort();
        for sibling in siblings {
            self.get_file_for_path(&sibling);
        }
        self.discover();
    }

    /// Dart `LibraryFileKind.fileKinds`: the library file and its parts
    /// (also nested), depth first. A part that was visited before is not
    /// visited again (the Dart code would not end on such a cycle).
    pub fn library_file_kinds(&self, library: FileId) -> Vec<FileId> {
        let mut result = Vec::new();
        let mut visited = IndexSet::new();
        self.visit_parts(library, &mut result, &mut visited);
        result
    }

    fn visit_parts(&self, kind: FileId, result: &mut Vec<FileId>, visited: &mut IndexSet<FileId>) {
        if !visited.insert(kind) {
            return;
        }
        result.push(kind);
        for directive in &self.file(kind).c().part_includes {
            if let Some(part) = self.included_part(kind, directive) {
                self.visit_parts(part, result, visited);
            }
        }
    }

    /// Dart `LibraryFileKind.files`.
    pub fn library_files(&self, library: FileId) -> Vec<FileId> {
        let mut seen = IndexSet::new();
        for f in self.library_file_kinds(library) {
            seen.insert(f);
        }
        seen.into_iter().collect()
    }

    /// Dart `LibraryFileKind.apiSignature`.
    pub fn library_api_signature(&self, library: FileId) -> [u8; 16] {
        let mut builder = ApiSignature::new();
        let mut files = self.library_files(library);
        files.sort_by(|a, b| self.file(*a).path.cmp(&self.file(*b).path));
        for f in files {
            builder.add_bytes(self.file(f).api_signature());
        }
        builder.to_byte_list()
    }

    /// Dart `FileKind.addDirectivesSignature`.
    pub fn add_directives_signature(&self, kind: FileId, signature: &mut ApiSignature) {
        let append = |signature: &mut ApiSignature, uri: &DirectiveUri| match uri {
            DirectiveUri::WithFile { file, .. } => {
                let f = self.file(*file);
                signature.add_int(0);
                signature.add_bool(f.exists());
                signature.add_bool(f.kind().is_part());
                signature.add_string(&f.uri_str);
            }
            DirectiveUri::WithUri { relative_uri, .. } => {
                signature.add_int(2);
                signature.add_string(relative_uri);
            }
            DirectiveUri::WithString { relative_uri_str } => {
                signature.add_int(3);
                signature.add_string(relative_uri_str);
            }
            DirectiveUri::WithoutString => signature.add_int(4),
        };
        let c = self.file(kind).c();
        signature.add_int(c.library_exports.len() as u32);
        for d in &c.library_exports {
            append(signature, &d.uris.selected);
        }
        signature.add_int(c.library_imports.len() as u32);
        for d in &c.library_imports {
            append(signature, &d.uris.selected);
        }
        signature.add_int(c.doc_library_imports.len() as u32);
        for d in &c.doc_library_imports {
            append(signature, &d.uris.selected);
        }
    }
}

/// The result of reading and parsing one file (the first half of Dart
/// `FileState.refresh`), computed in parallel.
struct RefreshData {
    exists: bool,
    content_hash: String,
    unlinked_signature: [u8; 16],
    parsed: Arc<ParsedUnit>,
    unlinked: UnlinkedUnit,
}

/// Reads [path] like Dart `FileContentCache` (a missing or unreadable file
/// has empty content and `exists = false`), parses it (`parseCode`) and
/// computes the unlinked unit (`serializeAstUnlinked2`).
fn refresh_data(path: &str, is_dart_core: bool, config: &FileConfig, salt: &[u32]) -> RefreshData {
    // Through the overlays of the language server (open documents).
    let (content, exists) = match dartr_project::fs::read_string_strict(path) {
        Some(text) => (text, true),
        None => (String::new(), false),
    };
    let content = match content.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => content,
    };
    let content_hash = {
        use md5::{Digest, Md5};
        hex(&Md5::digest(content.as_bytes()))
    };
    let unlinked_signature = {
        let mut signature = ApiSignature::new();
        signature.add_uint32_list(salt);
        let experiments = &config.experiments;
        let (major, minor) = config.package_language_version;
        let features =
            dartr_parser::experimental_features::ExperimentalFeatures::for_language_version(
                major,
                minor,
                experiments,
            );
        signature.add_feature_set(|name| crate::unlinked_data::feature_enabled(features, name));
        signature.add_language_version(major, minor);
        signature.add_string(&content_hash);
        signature.add_bool(exists);
        signature.to_byte_list()
    };
    let parsed = parse_file(
        &content,
        path,
        config.package_language_version,
        &config.experiments,
    );
    let unlinked = serialize_ast_unlinked2(&parsed, exists, is_dart_core);
    RefreshData {
        exists,
        content_hash,
        unlinked_signature,
        parsed: Arc::new(parsed),
        unlinked,
    }
}
