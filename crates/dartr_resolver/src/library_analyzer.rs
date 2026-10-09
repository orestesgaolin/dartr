// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart
// (LibraryAnalyzer.analyze, _parseAndResolve, _resolveFile)

//! `LibraryAnalyzer`: analysis of one library (design §2.5, step 4).
//!
//! 1. Build the library scopes ([`LibraryScopes`]) once.
//! 2. Resolve the units in parallel (rayon). Each unit gets a clone of its
//!    parsed AST (resolution rewrites nodes) and its own [`LocalArena`] for
//!    local elements and types; the passes are element binding, the
//!    resolution visitor and the resolver visitor (Dart `_resolveFile`).
//! 3. The library-wide steps (Dart `_computeConstants`,
//!    `_computeDiagnostics`: verifiers, unused elements, imports, ignore
//!    comments). STUB: wave D.
//!
//! A panic while a unit is resolved is caught; the unit result then has
//! [`ResolvedUnit::panic`] set (the analyzer reports an exception
//! diagnostic for the library in that case).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use dartr_ast::{Ast, CompilationUnit, Id};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::Diagnostic;
use dartr_element::{
    Ctx, EId, FId, LibraryElement, LibraryFragment, LocalArena, NoopSink, ResolutionTables,
    TypeProvider, WorldSnapshot,
};
use rayon::prelude::*;

use crate::options::AnalysisOptions;
use crate::resolver::{ResolverVisitor, UnitContext};
use crate::scope::LibraryScopes;
use crate::tables::ResolverTables;

/// One unit of the library to analyze.
#[derive(Clone)]
pub struct UnitInput {
    /// The file path of the unit (Dart `Source.fullName`).
    pub path: Arc<str>,
    /// The URI of the unit.
    pub uri: Arc<str>,
    /// The parsed unit (shared with the file state; resolution clones the
    /// AST).
    pub parsed: Arc<ParsedUnit>,
    /// The library fragment of the unit (linked).
    pub fragment: FId<LibraryFragment>,
}

/// The inputs of [`analyze_library`].
pub struct LibraryAnalysisInput<'a> {
    /// A snapshot with the cycle of the library and all its dependencies.
    pub world: &'a WorldSnapshot,
    pub type_provider: &'a TypeProvider,
    pub library: EId<LibraryElement>,
    /// The units: the defining unit first, then the parts in the order of
    /// the `part` directives (depth first).
    pub units: Vec<UnitInput>,
    pub options: AnalysisOptions,
}

/// The resolution of one unit.
pub struct ResolvedUnit {
    pub path: Arc<str>,
    pub uri: Arc<str>,
    pub fragment: FId<LibraryFragment>,
    /// The resolved AST (with the rewrites of resolution).
    pub ast: Ast,
    pub unit: Id<CompilationUnit>,
    pub tables: ResolutionTables,
    pub rt: ResolverTables,
    /// The local elements and types of the unit (to read the results, use a
    /// [`Ctx`] with `local: Some(&unit.local)`).
    pub local: LocalArena,
    /// The parse diagnostics followed by the resolution diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The panic message, when resolution of the unit panicked.
    pub panic: Option<String>,
}

/// The analysis result of a library.
pub struct ResolvedLibrary {
    pub library: EId<LibraryElement>,
    pub units: Vec<ResolvedUnit>,
}

/// Dart `LibraryAnalyzer.analyze()`.
pub fn analyze_library(input: &LibraryAnalysisInput<'_>) -> ResolvedLibrary {
    let sink = NoopSink;
    let library_features = {
        let ctx = global_ctx(input, &sink);
        ctx.get(input.library).feature_set.clone()
    };
    let scopes = {
        let ctx = Ctx {
            world: input.world,
            current: None,
            local: None,
            tp: input.type_provider,
            features: &library_features,
            req: &sink,
        };
        LibraryScopes::build(&ctx, input.library)
    };

    // Dart `_parseAndResolve`: the units in parallel.
    let units: Vec<ResolvedUnit> = input
        .units
        .par_iter()
        .map(|unit| resolve_file(input, &scopes, &library_features, unit))
        .collect();

    let mut library = ResolvedLibrary {
        library: input.library,
        units,
    };
    compute_constants(input, &mut library);
    compute_diagnostics(input, &mut library);
    library
}

/// Dart `_resolveDirectives` (directive elements, URI diagnostics of
/// imports, exports and parts). STUB (wave D): the directive elements are
/// in the linked fragments; the URI diagnostics are not reported yet.
fn resolve_directives(_input: &LibraryAnalysisInput<'_>, _unit: &UnitInput) {}

/// Dart `_computeConstants`: evaluates the constants of all units
/// (`computeConstants` over `_findConstants`). STUB (wave D, D1–D2).
fn compute_constants(_input: &LibraryAnalysisInput<'_>, _library: &mut ResolvedLibrary) {}

/// Dart `_computeDiagnostics`: `InheritanceOverrideVerifier`,
/// `_computeVerifyErrors` per unit (error verifier, constant verifier,
/// ...), `MemberDuplicateDefinitionVerifier.checkLibrary`, the constructor
/// fields verifier, the warnings with the used local elements, lints,
/// `_checkForInconsistentLanguageVersionOverride`, `IgnoreValidator`, and
/// the filtering of ignored diagnostics (`_filterIgnoredDiagnostics`).
/// STUB (wave D).
fn compute_diagnostics(_input: &LibraryAnalysisInput<'_>, _library: &mut ResolvedLibrary) {}

fn global_ctx<'a>(input: &LibraryAnalysisInput<'a>, sink: &'a NoopSink) -> Ctx<'a> {
    static EMPTY: std::sync::OnceLock<dartr_element::FeatureSet> = std::sync::OnceLock::new();
    Ctx {
        world: input.world,
        current: None,
        local: None,
        tp: input.type_provider,
        features: EMPTY.get_or_init(dartr_element::FeatureSet::default),
        req: sink,
    }
}

/// Dart `_resolveFile`.
fn resolve_file(
    input: &LibraryAnalysisInput<'_>,
    scopes: &LibraryScopes,
    library_features: &dartr_element::FeatureSet,
    unit: &UnitInput,
) -> ResolvedUnit {
    let sink = NoopSink;
    resolve_directives(input, unit);
    let local = input.world.generation.new_local_arena();
    let mut ast = unit.parsed.ast.clone();
    let root = unit.parsed.unit;
    let mut tables = ResolutionTables::new();
    let mut rt = ResolverTables::new();
    let mut diagnostics: Vec<Diagnostic> = unit.parsed.diagnostics.clone();
    for &node in &unit.parsed.dot_shorthands {
        rt.dot_shorthand.insert(node, ());
    }

    let panic = {
        let ctx = Ctx {
            world: input.world,
            current: None,
            local: Some(&local),
            tp: input.type_provider,
            features: library_features,
            req: &sink,
        };
        let unit_ctx = UnitContext {
            library: input.library,
            fragment: unit.fragment,
            scopes,
            options: input.options,
            features: unit.parsed.feature_set,
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            crate::element_binding_visitor::bind_unit(
                &ctx,
                &ast,
                root,
                unit.fragment,
                &mut tables,
                &mut rt,
            );
            crate::resolution_visitor::resolve_unit(
                &ctx,
                unit_ctx,
                &mut ast,
                root,
                &mut tables,
                &mut rt,
                &mut diagnostics,
            );
            let mut resolver = ResolverVisitor::new(
                ctx,
                &mut ast,
                &mut tables,
                &mut rt,
                &mut diagnostics,
                unit_ctx,
            );
            resolver.visit_node(root.raw());
            resolver.flush_type_analyzer_errors();
        }));
        result.err().map(|e| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string())
        })
    };

    ResolvedUnit {
        path: unit.path.clone(),
        uri: unit.uri.clone(),
        fragment: unit.fragment,
        ast,
        unit: root,
        tables,
        rt,
        local,
        diagnostics,
        panic,
    }
}
