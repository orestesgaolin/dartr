// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart
// (_resolveDirectives, _resolveLibraryImportDirective,
// _resolveLibraryExportDirective, _resolveLibraryDocImportDirective,
// _reportImportDirectiveErrors, _resolvePartDirective),
// pkg/analyzer/lib/src/util/file_paths.dart (isGenerated)

//! The URI diagnostics of the directives of a library (Dart
//! `LibraryAnalyzer._resolveDirectives`): they need the file states of the
//! driver (does the file exist, is it a library or a part), so the driver
//! computes them and the library analyzer adds them to the diagnostics of
//! each unit ([`dartr_resolver::library_analyzer::UnitInput`]).

use dartr_ast::{
    Ast, CompilationUnit, ExportDirective, Id, ImportDirective, LibraryDirective, PartDirective,
    StringLiteral,
};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use dartr_element::{Ctx, EId, FId, LibraryElement, LibraryFragment};
use indexmap::{IndexMap, IndexSet};

use crate::file_state::{
    DirectiveUri, FileId, FileKind, FileSystemState, LibraryExportState, LibraryImportState,
    PartIncludeState,
};

/// Dart `file_paths.isGenerated(path)`.
pub fn is_generated(path: &str) -> bool {
    const SUFFIXES: [&str; 6] = [
        ".g.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbserver.dart",
        ".pbjson.dart",
        ".template.dart",
    ];
    SUFFIXES.iter().any(|s| path.ends_with(s))
}

/// The directive diagnostics of the library [library_file] (linked as
/// [library]), by unit file. Dart `_resolveDirectives` from the defining
/// unit, recursively into the parts.
pub fn directive_diagnostics(
    fs: &FileSystemState,
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    library_file: FileId,
) -> IndexMap<FileId, Vec<Diagnostic>> {
    let mut resolver = DirectivesResolver {
        fs,
        ctx,
        library,
        library_file,
        library_files: IndexSet::new(),
        result: IndexMap::new(),
    };
    let first = ctx.get(library).first_fragment();
    resolver.resolve_directives(library_file, library_file, first);
    resolver.result
}

struct DirectivesResolver<'a, 'c> {
    fs: &'a FileSystemState,
    ctx: &'a Ctx<'c>,
    library: EId<LibraryElement>,
    library_file: FileId,
    /// Dart `_libraryFiles`: the files of the library parsed so far.
    library_files: IndexSet<FileId>,
    result: IndexMap<FileId, Vec<Diagnostic>>,
}

impl DirectivesResolver<'_, '_> {
    fn report(&mut self, file: FileId, ast: &Ast, uri: Id<StringLiteral>, d: LocatableDiagnostic) {
        let offset = ast.offset(uri) as usize;
        let length = ast.length(uri) as usize;
        self.result
            .entry(file)
            .or_default()
            .push(d.at_offset(offset, length).into_diagnostic());
    }

    /// Dart `_resolveDirectives(enclosingFile:, fileKind:, fileFragment:)`.
    fn resolve_directives(
        &mut self,
        _enclosing_file: FileId,
        file: FileId,
        fragment: FId<LibraryFragment>,
    ) {
        // Dart `_parse`: records the file in `_libraryFiles`.
        self.library_files.insert(file);
        let content = self.fs.file(file).c();
        let parsed = content.parsed.clone();
        let ast = &parsed.ast;
        let unit: Id<CompilationUnit> = parsed.unit;

        let mut library_export_index = 0;
        let mut library_import_index = 0;
        let mut part_index = 0;
        let mut library_directive = None;
        for &directive in ast.list(ast[unit].directives) {
            let directive = directive.raw();
            if let Some(d) = ast.cast::<ExportDirective>(directive) {
                let index = library_export_index;
                library_export_index += 1;
                if let Some(state) = content.library_exports.get(index) {
                    self.resolve_library_export_directive(file, ast, ast[d].uri, state);
                }
            } else if let Some(d) = ast.cast::<ImportDirective>(directive) {
                let index = library_import_index;
                library_import_index += 1;
                if let Some(state) = content.library_imports.get(index) {
                    self.report_import_directive_errors(file, ast, ast[d].uri, state);
                }
            } else if let Some(d) = ast.cast::<LibraryDirective>(directive) {
                library_directive.get_or_insert(d);
            } else if let Some(d) = ast.cast::<PartDirective>(directive) {
                let index = part_index;
                part_index += 1;
                if let Some(state) = content.part_includes.get(index) {
                    // Dart `enclosingFile: fileAnalysis` (this file).
                    self.resolve_part_directive(
                        file, file, fragment, index, ast, ast[d].uri, state,
                    );
                }
            }
        }

        // Dart: the doc imports of the first library directive.
        let comment = library_directive.and_then(|d| ast[d].documentation_comment);
        if let Some(comment) = comment {
            for (i, doc_import) in ast[comment].doc_imports.iter().enumerate() {
                let Some(state) = content.doc_library_imports.get(i) else {
                    continue;
                };
                let doc_ast = &doc_import.ast;
                let uri = doc_ast[doc_import.import].uri;
                self.report_import_directive_errors(file, doc_ast, uri, state);
            }
        }
    }

    /// Dart `_reportImportDirectiveErrors`.
    fn report_import_directive_errors(
        &mut self,
        file: FileId,
        ast: &Ast,
        uri: Id<StringLiteral>,
        state: &LibraryImportState,
    ) {
        let is_doc_import = state.unlinked.is_doc_import;
        let selected = &state.uris.selected;
        let d = match selected {
            DirectiveUri::WithoutString => diag::uri_with_interpolation(),
            DirectiveUri::WithString { relative_uri_str } => diag::invalid_uri(relative_uri_str),
            DirectiveUri::WithUri {
                relative_uri_str, ..
            } => {
                if relative_uri_str.starts_with("dart-ext:") {
                    diag::use_of_native_extension()
                } else if is_doc_import {
                    diag::uri_does_not_exist_in_doc_import(relative_uri_str)
                } else {
                    diag::uri_does_not_exist(relative_uri_str)
                }
            }
            DirectiveUri::WithFile {
                relative_uri_str,
                file: imported,
                ..
            } => {
                let imported = self.fs.file(*imported);
                if relative_uri_str.starts_with("dart-ext:") {
                    diag::use_of_native_extension()
                } else if !imported.exists() {
                    if is_doc_import {
                        diag::uri_does_not_exist_in_doc_import(relative_uri_str)
                    } else if is_generated(&imported.path) {
                        diag::uri_has_not_been_generated(relative_uri_str)
                    } else {
                        diag::uri_does_not_exist(relative_uri_str)
                    }
                } else if !imported.kind().is_library() {
                    diag::import_of_non_library(relative_uri_str)
                } else {
                    return;
                }
            }
        };
        self.report(file, ast, uri, d);
    }

    /// Dart `_resolveLibraryExportDirective`.
    fn resolve_library_export_directive(
        &mut self,
        file: FileId,
        ast: &Ast,
        uri: Id<StringLiteral>,
        state: &LibraryExportState,
    ) {
        let d = match &state.uris.selected {
            DirectiveUri::WithoutString => diag::uri_with_interpolation(),
            DirectiveUri::WithString { relative_uri_str } => diag::invalid_uri(relative_uri_str),
            DirectiveUri::WithUri {
                relative_uri_str, ..
            } => {
                if relative_uri_str.starts_with("dart-ext:") {
                    diag::use_of_native_extension()
                } else {
                    diag::uri_does_not_exist(relative_uri_str)
                }
            }
            DirectiveUri::WithFile {
                relative_uri_str,
                file: exported,
                ..
            } => {
                let exported = self.fs.file(*exported);
                if relative_uri_str.starts_with("dart-ext:") {
                    diag::use_of_native_extension()
                } else if !exported.exists() {
                    if is_generated(&exported.path) {
                        diag::uri_has_not_been_generated(relative_uri_str)
                    } else {
                        diag::uri_does_not_exist(relative_uri_str)
                    }
                } else if !exported.kind().is_library() {
                    diag::export_of_non_library(relative_uri_str)
                } else {
                    return;
                }
            }
        };
        self.report(file, ast, uri, d);
    }

    /// Dart `_resolvePartDirective`.
    #[allow(clippy::too_many_arguments)]
    fn resolve_part_directive(
        &mut self,
        enclosing_file: FileId,
        file: FileId,
        fragment: FId<LibraryFragment>,
        index: usize,
        ast: &Ast,
        uri: Id<StringLiteral>,
        state: &PartIncludeState,
    ) {
        let included = match &state.uris.selected {
            DirectiveUri::WithoutString => {
                self.report(file, ast, uri, diag::uri_with_interpolation());
                return;
            }
            DirectiveUri::WithString { relative_uri_str } => {
                self.report(file, ast, uri, diag::invalid_uri(relative_uri_str));
                return;
            }
            DirectiveUri::WithUri {
                relative_uri_str, ..
            } => {
                self.report(file, ast, uri, diag::uri_does_not_exist(relative_uri_str));
                return;
            }
            DirectiveUri::WithFile { file, .. } => *file,
        };
        let included_file = self.fs.file(included);
        let included_kind = included_file.kind().clone();

        if included_kind.is_library() {
            let uri_str = &included_file.uri_str;
            let d = if included_file.exists() {
                diag::part_of_non_part(uri_str)
            } else if is_generated(&included_file.path) {
                diag::uri_has_not_been_generated(uri_str)
            } else {
                diag::uri_does_not_exist(uri_str)
            };
            self.report(file, ast, uri, d);
            return;
        }

        // Validate that the part source is unique in the library.
        if self.library_files.contains(&included) {
            let d = diag::duplicate_part(&included_file.uri_str);
            self.report(file, ast, uri, d);
            return;
        }

        // Dart `partElement.uri is! DirectiveUriWithUnitImpl`.
        let part_fragment =
            self.ctx
                .fragment(fragment)
                .parts
                .get(index)
                .and_then(|p| match &p.directive.uri {
                    dartr_element::DirectiveUri::Unit {
                        library_fragment, ..
                    } => Some(*library_fragment),
                    _ => None,
                });
        let Some(part_fragment) = part_fragment else {
            match &included_kind {
                FileKind::PartOfName { name } => {
                    let enhanced_parts = self
                        .ctx
                        .get(self.library)
                        .feature_set
                        .is_enabled("enhanced-parts");
                    if !enhanced_parts {
                        let library_name = match self.fs.file(self.library_file).kind() {
                            FileKind::Library { name: Some(n) } => n.clone(),
                            _ => String::new(),
                        };
                        let d = if library_name.is_empty() {
                            diag::part_of_unnamed_library(name)
                        } else {
                            diag::part_of_different_library(&library_name, name)
                        };
                        self.report(file, ast, uri, d);
                    }
                }
                FileKind::PartOfUriKnown { .. } | FileKind::PartOfUriUnknown => {
                    let expected = self.fs.file(enclosing_file).uri_str.clone();
                    let d = diag::part_of_different_library(&expected, &included_file.uri_str);
                    self.report(file, ast, uri, d);
                }
                FileKind::Library { .. } => {}
            }
            return;
        };

        self.resolve_directives(enclosing_file, included, part_fragment);
    }
}
