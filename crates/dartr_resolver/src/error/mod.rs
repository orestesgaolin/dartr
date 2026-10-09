// Dart source: pkg/analyzer/lib/src/error/ (the verifiers; one module per
// Dart file, except codes*.dart and listener.dart, which are in
// dartr_diagnostics, and ignore_validator.dart, which is in dartr_cli)

//! The verifiers of `pkg/analyzer/lib/src/error/` (design
//! `docs/design/semantics.md` wave D, units D8–D12).
//!
//! They run on a resolved unit, after resolution. Two kinds of callers:
//!
//! - the library-wide steps of the library analyzer
//!   (`library_analyzer::compute_diagnostics`, Dart
//!   `LibraryAnalyzer._computeDiagnostics` and `_computeWarnings`) give them
//!   a [`UnitVerifier`];
//! - the error verifier (`generated/error_verifier.dart`) and the resolver
//!   call some verifiers on single nodes. These verifiers are free functions
//!   generic over [`VerifierHost`], which [`UnitVerifier`] and the resolver
//!   implement (the error verifier implements it too).

use dartr_ast::{Ast, CompilationUnit, Id, NodeId};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, LocatedDiagnostic};
use dartr_element::{
    Ctx, EId, ElemRef, FId, LibraryElement, LibraryFragment, ResolutionTables, TypeId,
};
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_syntax::TokenId;
use dartr_typesystem::TypeSystem;

use crate::options::AnalysisOptions;
use crate::scope::LibraryScopes;
use crate::tables::ResolverTables;

// D8: inheritance and duplicate definitions.
pub mod correct_override;
pub mod duplicate_definition_verifier;
pub mod getter_setter_types_verifier;
pub mod inheritance_override;
pub mod member_duplicate_definition_verifier;
pub mod override_verifier;
pub mod redeclare_verifier;

// D9: imports, unused elements, dead code, TODOs.
pub mod dead_code_verifier;
pub mod imports_verifier;
pub mod todo_finder;
pub mod unused_local_elements_verifier;

// D10: best practices, annotations, element usage.
pub mod annotation_verifier;
pub mod best_practices_verifier;
pub mod deprecated_functionality_verifier;
pub mod deprecated_member_use_verifier;
pub mod do_not_submit_member_use_verifier;
pub mod element_usage_detector;
pub mod element_usage_frontier_detector;
pub mod experimental_member_use_verifier;
pub mod immutable_verifier;
pub mod must_call_super_verifier;
pub mod null_safe_api_verifier;
pub mod use_result_verifier;
pub mod widget_preview_verifier;

// D11: types, literals, returns, constructors, class modifiers.
pub mod base_or_final_type_verifier;
pub mod constructor_fields_verifier;
pub mod literal_element_verifier;
pub mod required_parameters_verifier;
pub mod return_type_verifier;
pub mod super_formal_parameters_verifier;
pub mod type_arguments_verifier;

// Ported with C4/C5; moved here from assignment_expression_resolver.rs.
pub mod assignment_verifier;

// D12: the remaining verifiers.
pub mod async_return_visitor;
pub mod bool_expression_verifier;
pub mod const_argument_verifier;
pub mod doc_comment_verifier;
pub mod error_handler_verifier;
pub mod inference_error;
pub mod language_version_override_verifier;
pub mod nullable_dereference_verifier;
pub mod unicode_text_verifier;

/// What a verifier reads and where it reports (Dart: the resolved AST, the
/// `DiagnosticReporter`, the `TypeSystemImpl` and the library element that
/// the verifier classes get in their constructors).
///
/// Implemented by [`UnitVerifier`] (library-wide steps) and by the
/// resolver; the error verifier implements it too.
pub trait VerifierHost<'a> {
    /// The lookup context, with the unit's local arena.
    fn ctx(&self) -> Ctx<'a>;
    /// Dart `typeSystem`.
    fn type_system(&self) -> TypeSystem<'a>;
    /// The resolved AST of the unit.
    fn ast(&self) -> &Ast;
    /// The resolution results of the unit.
    fn tables(&self) -> &ResolutionTables;
    /// The resolver-private node data of the unit.
    fn rt(&self) -> &ResolverTables;
    /// Dart `_currentLibrary` / `definingLibrary`.
    fn library(&self) -> EId<LibraryElement>;
    /// Dart `libraryFragment` / `unit.declaredFragment`.
    fn fragment(&self) -> FId<LibraryFragment>;
    /// The analysis options.
    fn options(&self) -> AnalysisOptions;
    /// Dart `unit.featureSet`.
    fn features(&self) -> ExperimentalFeatures;
    /// Dart `diagnosticReporter.report(diagnostic)`.
    fn report(&mut self, diagnostic: LocatedDiagnostic);

    /// Dart `diagnostic.at(node)`.
    fn at(&self, diagnostic: LocatableDiagnostic, node: impl Into<NodeId>) -> LocatedDiagnostic
    where
        Self: Sized,
    {
        let node = node.into();
        let ast = self.ast();
        diagnostic.at_offset(ast.offset(node) as usize, ast.length(node) as usize)
    }

    /// Dart `diagnostic.at(token)`.
    fn at_token(&self, diagnostic: LocatableDiagnostic, token: TokenId) -> LocatedDiagnostic {
        let t = self.ast().tokens.get(token);
        diagnostic.at_offset(t.offset as usize, (t.end() - t.offset) as usize)
    }

    /// Dart `node.staticType`.
    fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId>
    where
        Self: Sized,
    {
        self.tables().static_type.get(node.into()).copied()
    }

    /// Dart `node.element`.
    fn element(&self, node: impl Into<NodeId>) -> Option<ElemRef>
    where
        Self: Sized,
    {
        self.tables().element.get(node.into()).copied()
    }
}

/// One resolved unit for the library-wide steps (Dart `FileAnalysis` with
/// its `diagnosticReporter`).
pub struct UnitVerifier<'a> {
    /// The lookup context with the unit's local arena.
    pub ctx: Ctx<'a>,
    pub type_system: TypeSystem<'a>,
    /// The index of the unit in `ResolvedLibrary::units` (0: the defining
    /// unit).
    pub index: usize,
    pub path: &'a str,
    pub uri: &'a str,
    /// The parsed unit (line info, language version, the parse
    /// diagnostics); the resolved AST is [`UnitVerifier::ast`].
    pub parsed: &'a ParsedUnit,
    pub ast: &'a Ast,
    /// The `CompilationUnit` node.
    pub unit: Id<CompilationUnit>,
    pub tables: &'a ResolutionTables,
    pub rt: &'a ResolverTables,
    pub library: EId<LibraryElement>,
    pub fragment: FId<LibraryFragment>,
    pub scopes: &'a LibraryScopes,
    pub options: AnalysisOptions,
    pub features: ExperimentalFeatures,
    /// Dart `fileAnalysis.diagnosticListener.diagnostics`: every diagnostic
    /// of the unit so far (parse, resolution, the previous steps).
    pub diagnostics: &'a mut Vec<Diagnostic>,
}

impl<'a> VerifierHost<'a> for UnitVerifier<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }
    fn type_system(&self) -> TypeSystem<'a> {
        self.type_system
    }
    fn ast(&self) -> &Ast {
        self.ast
    }
    fn tables(&self) -> &ResolutionTables {
        self.tables
    }
    fn rt(&self) -> &ResolverTables {
        self.rt
    }
    fn library(&self) -> EId<LibraryElement> {
        self.library
    }
    fn fragment(&self) -> FId<LibraryFragment> {
        self.fragment
    }
    fn options(&self) -> AnalysisOptions {
        self.options
    }
    fn features(&self) -> ExperimentalFeatures {
        self.features
    }
    fn report(&mut self, diagnostic: LocatedDiagnostic) {
        let diagnostic = diagnostic.into_diagnostic();
        // Dart `RecordingDiagnosticListener` keeps a set: a diagnostic equal
        // to a recorded one (code, offset, length, message) is dropped.
        let duplicate = self.diagnostics.iter().any(|d| {
            std::ptr::eq(d.code, diagnostic.code)
                && d.offset == diagnostic.offset
                && d.length == diagnostic.length
                && d.message == diagnostic.message
        });
        if !duplicate {
            self.diagnostics.push(diagnostic);
        }
    }
}

impl<'a> VerifierHost<'a> for crate::resolver::ResolverVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }
    fn type_system(&self) -> TypeSystem<'a> {
        self.type_system
    }
    fn ast(&self) -> &Ast {
        self.ast
    }
    fn tables(&self) -> &ResolutionTables {
        self.tables
    }
    fn rt(&self) -> &ResolverTables {
        self.rt
    }
    fn library(&self) -> EId<LibraryElement> {
        self.unit.library
    }
    fn fragment(&self) -> FId<LibraryFragment> {
        self.unit.fragment
    }
    fn options(&self) -> AnalysisOptions {
        self.unit.options
    }
    fn features(&self) -> ExperimentalFeatures {
        self.unit.features
    }
    fn report(&mut self, diagnostic: LocatedDiagnostic) {
        crate::resolver::ResolverVisitor::report(self, diagnostic);
    }
}
