// Dart source: pkg/analyzer/lib/src/dart/element/element.dart
// (ElementAnnotationImpl, MetadataImpl, ConstantInitializerImpl,
// ElementDirectiveImpl, LibraryImportImpl, LibraryExportImpl, PartIncludeImpl,
// DirectiveUri*Impl, Show/HideElementCombinatorImpl),
// pkg/analyzer/lib/dart/element/element.dart (LibraryLanguageVersion),
// pkg/analyzer/lib/src/error/inference_error.dart (TopLevelInferenceError),
// pkg/analyzer/lib/src/dart/element/field_name_non_promotability_info.dart,
// pkg/analyzer/lib/src/dart/resolver/scope.dart (Namespace, data only),
// _fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart
// (JoinedPatternVariableInconsistency)

//! Data types that elements and fragments hold: metadata, directives,
//! namespaces, inference errors.

use std::sync::Arc;

use dartr_ast::NodeId;
use indexmap::IndexMap;

use crate::ids::{EId, ElementId, FId, InterfaceElement, PropertyAccessorElement};
use crate::name::Name;
use crate::slot::OnceSlot;
use crate::{FieldElement, LibraryElement, LibraryFragment, PrefixFragment};

/// An expression that linking resolves (const initializer, default value,
/// annotation, constructor initializer, initializer of an inferred
/// variable): a node in the `ConstExprs` arena of the cycle that owns the
/// element (design §2.2, a detached copy as in `detach_nodes.dart`). For a
/// local element, a node in the AST of the unit under analysis.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ConstExprId(pub NodeId);

/// A source file (Dart `Source`: `fullName` and `uri`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceRef {
    /// `Source.fullName`.
    pub path: Arc<str>,
    /// `Source.uri`, as text.
    pub uri: Arc<str>,
}

/// A language version (`major.minor`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

/// Dart `LibraryLanguageVersion`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LibraryLanguageVersion {
    /// The version for the whole package that contains the library.
    pub package: Version,
    /// The `// @dart = x.y` override; `None` if absent or invalid.
    pub override_: Option<Version>,
}

impl LibraryLanguageVersion {
    /// Dart `effective`.
    pub fn effective(&self) -> Version {
        self.override_.unwrap_or(self.package)
    }
}

/// One annotation (`ElementAnnotationImpl`).
#[derive(Debug)]
pub struct ElementAnnotation {
    /// Dart: ElementAnnotationImpl.libraryFragment
    pub library_fragment: FId<LibraryFragment>,
    /// The `Annotation` node in the cycle's `ConstExprs`.
    /// Dart: ElementAnnotationImpl.annotationAst
    pub annotation_ast: ConstExprId,
}

/// The annotations of an element or fragment (`MetadataImpl`).
#[derive(Debug, Default)]
pub struct Metadata {
    /// Dart: MetadataImpl.annotations
    pub annotations: Vec<ElementAnnotation>,
    /// Lazily computed `has*` bits (`_metadataFlags2`); filled by unit C9 /
    /// D10 from the resolved annotations.
    /// Dart: MetadataImpl._metadataFlags2
    pub metadata_flags: OnceSlot<u32>,
}

impl Metadata {
    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }
}

/// `ConstantInitializerImpl`: the initializer of a const variable (or of a
/// final field of a class with a const constructor).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConstantInitializer {
    /// Dart: ConstantInitializerImpl.fragment
    pub fragment: crate::FragmentId,
    /// Dart: ConstantInitializerImpl.expression
    pub expression: ConstExprId,
}

/// The URI of a directive (`DirectiveUri` and its subclasses, each adds
/// data to its superclass).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DirectiveUri {
    /// Dart: DirectiveUriImpl (no URI at all).
    None,
    /// Dart: DirectiveUriWithRelativeUriStringImpl.relativeUriString
    RelativeUriString { relative_uri_string: Arc<str> },
    /// Dart: DirectiveUriWithRelativeUriImpl.relativeUri
    RelativeUri {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
    },
    /// Dart: DirectiveUriWithSourceImpl.source
    Source {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        source: SourceRef,
    },
    /// Dart: DirectiveUriWithLibraryImpl.library
    Library {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        source: SourceRef,
        library: EId<LibraryElement>,
    },
    /// Dart: DirectiveUriWithUnitImpl.libraryFragment
    Unit {
        relative_uri_string: Arc<str>,
        relative_uri: Arc<str>,
        library_fragment: FId<LibraryFragment>,
    },
}

/// `NamespaceCombinator`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NamespaceCombinator {
    Hide {
        /// Dart: HideElementCombinatorImpl.hiddenNames
        hidden_names: Vec<Name>,
        /// Dart: HideElementCombinatorImpl.offset
        offset: u32,
        /// Dart: HideElementCombinatorImpl.end
        end: i32,
    },
    Show {
        /// Dart: ShowElementCombinatorImpl.shownNames
        shown_names: Vec<Name>,
        /// Dart: ShowElementCombinatorImpl.offset
        offset: u32,
        /// Dart: ShowElementCombinatorImpl.end
        end: i32,
    },
}

/// The data of every directive (`ElementDirectiveImpl`).
#[derive(Debug)]
pub struct ElementDirective {
    /// Dart: ElementDirectiveImpl.libraryFragment
    pub library_fragment: FId<LibraryFragment>,
    /// Dart: ElementDirectiveImpl.uri
    pub uri: DirectiveUri,
    /// Dart: ElementDirectiveImpl.metadata
    pub metadata: Metadata,
}

/// `LibraryImportImpl`.
#[derive(Debug)]
pub struct LibraryImport {
    pub directive: ElementDirective,
    /// Dart: LibraryImportImpl.isSynthetic
    pub is_synthetic: bool,
    /// Dart: LibraryImportImpl.combinators
    pub combinators: Vec<NamespaceCombinator>,
    /// Dart: LibraryImportImpl.importKeywordOffset
    pub import_keyword_offset: i32,
    /// Dart: LibraryImportImpl.prefix
    pub prefix: Option<FId<PrefixFragment>>,
    /// The import namespace, computed on first use (unit A8).
    /// Dart: LibraryImportImpl._namespace
    pub namespace: OnceSlot<Arc<Namespace>>,
}

/// `LibraryExportImpl`.
#[derive(Debug)]
pub struct LibraryExport {
    pub directive: ElementDirective,
    /// Dart: LibraryExportImpl.combinators
    pub combinators: Vec<NamespaceCombinator>,
    /// Dart: LibraryExportImpl.exportKeywordOffset
    pub export_keyword_offset: i32,
}

/// `PartIncludeImpl`.
#[derive(Debug)]
pub struct PartInclude {
    pub directive: ElementDirective,
    /// Dart: PartIncludeImpl.partKeywordOffset
    pub part_keyword_offset: i32,
}

/// A namespace (`Namespace` of resolver/scope.dart, data only; unit A8 ports
/// the builders): name to element, in Dart map insertion order. Setter names
/// end with `=`.
#[derive(Debug, Default, Clone)]
pub struct Namespace {
    pub defined_names: IndexMap<Name, ElementId>,
}

/// `TopLevelInferenceError` (error/inference_error.dart).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TopLevelInferenceError {
    /// `TopLevelInferenceErrorDependencyCycle`.
    DependencyCycle { cycle: Vec<Name> },
    /// `TopLevelInferenceErrorNoCombinedSuperSignature`.
    OverrideNoCombinedSuperSignature { candidate_signatures: Arc<str> },
}

/// `FieldNameNonPromotabilityInfo`.
#[derive(Clone, Debug, Default)]
pub struct FieldNameNonPromotabilityInfo {
    pub conflicting_fields: Vec<EId<FieldElement>>,
    pub conflicting_getters: Vec<EId<PropertyAccessorElement>>,
    pub conflicting_nsm_classes: Vec<EId<InterfaceElement>>,
}

/// `JoinedPatternVariableInconsistency` (type_analyzer.dart).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum JoinedPatternVariableInconsistency {
    #[default]
    None,
    LogicalOr,
    SharedCaseAbsent,
    SharedCaseHasLabel,
    DifferentFinalityOrType,
}
