// Dart source: pkg/analyzer/lib/src/dart/analysis/file_state.dart (the
// parts of LibraryFileKind, LibraryImportState, LibraryExportState and
// PartIncludeState that summary2/library_builder.dart reads)

//! The input of linking one library cycle: for each library, its units
//! (defining unit and parts, also nested parts) with the parsed AST and the
//! resolved directives. The driver (`dartr_driver`) builds it from its file
//! state, so that the linker does not depend on the driver.

use std::sync::Arc;

use dartr_ast_builder::ParsedUnit;

/// A library to link (Dart `LibraryFileKind`).
#[derive(Clone, Debug)]
pub struct LinkLibraryInput {
    /// The defining unit; its `uri` is the library URI.
    pub unit: LinkUnitInput,
}

/// One unit of a library (Dart `FileKind` + `FileState`).
#[derive(Clone, Debug)]
pub struct LinkUnitInput {
    /// Dart `FileState.path` (`Source.fullName`).
    pub path: Arc<str>,
    /// Dart `FileState.uri`, as text.
    pub uri: Arc<str>,
    /// Dart `FileState.exists`.
    pub exists: bool,
    pub parsed: Arc<ParsedUnit>,
    /// Dart `FileKind.libraryImports` (with the synthetic `dart:core`
    /// import of a library).
    pub imports: Vec<LinkImport>,
    /// Dart `FileKind.libraryExports`.
    pub exports: Vec<LinkExport>,
    /// Dart `FileKind.partIncludes`.
    pub parts: Vec<LinkPart>,
}

/// The selected URI of a directive, as `library_builder.dart` turns it into
/// a `DirectiveUri*Impl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkDirectiveUri {
    /// `DirectiveUriWithoutString`.
    None,
    /// `DirectiveUriWithString`: the URI text does not parse.
    RelativeUriString { relative_uri_string: Arc<str> },
    /// `DirectiveUriWithUri`: the URI parses but is not resolved to a file.
    RelativeUri {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
    },
    /// `DirectiveUriWithFile` of a file that is not a library (imports and
    /// exports) or not a part of this container (parts).
    Source {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        /// `Source.fullName`.
        path: Arc<str>,
        /// `Source.uri`.
        uri: Arc<str>,
    },
    /// `DirectiveUriWithFile` of a library (imports and exports only).
    Library {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        /// The URI of the library; the linker finds its element in this
        /// cycle or in a linked cycle.
        library_uri: Arc<str>,
    },
}

/// A combinator of an import or export (Dart `UnlinkedCombinator`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkCombinator {
    pub is_show: bool,
    pub names: Vec<Arc<str>>,
    pub keyword_offset: u32,
    pub end_offset: u32,
}

/// The prefix of an import (Dart `UnlinkedLibraryImportPrefix`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkImportPrefix {
    /// `None` when the prefix name is synthetic.
    pub name: Option<Arc<str>>,
    pub name_offset: u32,
    pub is_deferred: bool,
}

/// Dart `LibraryImportState`.
#[derive(Clone, Debug)]
pub struct LinkImport {
    pub uri: LinkDirectiveUri,
    /// Dart `isSyntheticDartCore`.
    pub is_synthetic: bool,
    pub combinators: Vec<LinkCombinator>,
    /// `-1` for the synthetic import.
    pub import_keyword_offset: i32,
    pub prefix: Option<LinkImportPrefix>,
}

/// Dart `LibraryExportState`.
#[derive(Clone, Debug)]
pub struct LinkExport {
    pub uri: LinkDirectiveUri,
    pub combinators: Vec<LinkCombinator>,
    pub export_keyword_offset: u32,
}

/// Dart `PartIncludeState`.
#[derive(Clone, Debug)]
pub struct LinkPart {
    pub uri: LinkPartUri,
    pub part_keyword_offset: u32,
}

/// The selected URI of a `part` directive.
#[derive(Clone, Debug)]
pub enum LinkPartUri {
    /// `PartIncludeWithFile` with an `includedPart`: the part unit.
    Unit {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        unit: Box<LinkUnitInput>,
    },
    /// Every other case (a file that is not a part of this container, an
    /// unresolved URI, no URI).
    Other(LinkDirectiveUri),
}
