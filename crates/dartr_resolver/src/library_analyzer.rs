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
///
/// Units whose resolution panicked are skipped. A panic in a verifier is
/// caught and recorded as the unit's panic (the analyzer reports an
/// exception for the library).
///
/// Not here: the error verifier and the FFI verifier (D4–D7), lints
/// (`dartr_lints`), the SDK constraint verifier, `IgnoreValidator` and the
/// ignore filtering (`dartr_cli`). Dart runs the warnings only when
/// `analysisOptions.warning` is set (the default).
fn compute_diagnostics(input: &LibraryAnalysisInput<'_>, library: &mut ResolvedLibrary) {
    use crate::error::*;

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
    let mut panics: Vec<Option<String>> = library.units.iter().map(|u| u.panic.clone()).collect();
    let mut verifiers: Vec<UnitVerifier<'_>> = Vec::new();
    for (index, (unit, unit_input)) in library.units.iter_mut().zip(&input.units).enumerate() {
        let ResolvedUnit {
            path,
            uri,
            fragment,
            ast,
            unit: unit_node,
            tables,
            rt,
            local,
            diagnostics,
            ..
        } = unit;
        let ctx = Ctx {
            world: input.world,
            current: None,
            local: Some(&*local),
            tp: input.type_provider,
            features: &library_features,
            req: &sink,
        };
        verifiers.push(UnitVerifier {
            ctx,
            type_system: dartr_typesystem::TypeSystem::new(ctx),
            index,
            path,
            uri,
            parsed: &unit_input.parsed,
            ast,
            unit: *unit_node,
            tables,
            rt,
            library: input.library,
            fragment: *fragment,
            scopes: &scopes,
            options: input.options,
            features: unit_input.parsed.feature_set,
            diagnostics,
        });
    }

    // Dart `_computeVerifyErrors` (per unit): the constant verifier, the
    // inheritance override verifier, the error verifier, the FFI verifier.
    unit_step("InheritanceOverrideVerifier", &mut verifiers, &mut panics, &mut |v| {
        inheritance_override::verify_unit(v)
    });

    library_step("MemberDuplicateDefinitionVerifier", &mut verifiers, &mut panics, &mut |vs| {
        member_duplicate_definition_verifier::check_library(vs)
    });
    // Dart `_libraryVerificationContext.constructorFieldsVerifier.report()`.
    // The error verifier adds the constructors (D4–D7).
    let mut constructor_fields = constructor_fields_verifier::ConstructorFieldsVerifier::default();
    library_step("ConstructorFieldsVerifier", &mut verifiers, &mut panics, &mut |vs| {
        constructor_fields.report(vs)
    });

    // Dart `if (_analysisOptions.warning)`: the used local elements of all
    // units, then `_computeWarnings` per unit.
    let mut used_parts = Vec::new();
    for v in &verifiers {
        if panics[v.index].is_some() {
            continue;
        }
        match catch_unwind(AssertUnwindSafe(|| unused_local_elements_verifier::gather_used_local_elements(v))) {
            Ok(used) => used_parts.push(used),
            Err(e) => panics[v.index] = Some(format!("GatherUsedLocalElementsVisitor: {}", panic_message(&*e))),
        }
    }
    let used = unused_local_elements_verifier::UsedLocalElements::merge(used_parts);
    let prevents_import_warnings = imports_verifier::has_diagnostic_reported_that_prevents_import_warnings(&verifiers);
    // Dart `fileAnalysis.importsTracking` (recorded during resolution in
    // Dart; replayed from the resolved units here).
    let imports_tracking = if prevents_import_warnings {
        imports_verifier::ImportsTracking::default()
    } else {
        catch_unwind(AssertUnwindSafe(|| imports_verifier::compute_imports_tracking(&verifiers)))
            .unwrap_or_default()
    };
    // Dart `_computeWarnings`, the steps of one unit in Dart order.
    unit_step("_computeWarnings", &mut verifiers, &mut panics, &mut |v| {
        unicode_text_verifier::verify(v);
        dead_code_verifier::verify(v);
        best_practices_verifier::verify(v);
        override_verifier::verify(v);
        redeclare_verifier::verify(v);
        todo_finder::find_in(v);
        language_version_override_verifier::verify(v);
        if !prevents_import_warnings {
            imports_verifier::verify(v, &imports_tracking);
        }
        unused_local_elements_verifier::verify(v, &used);
    });

    // Dart `_checkForInconsistentLanguageVersionOverride`.
    check_for_inconsistent_language_version_override(&mut verifiers);

    drop(verifiers);
    for (unit, panic) in library.units.iter_mut().zip(panics) {
        if unit.panic.is_none() {
            unit.panic = panic;
        }
    }
}

/// Dart `_checkForInconsistentLanguageVersionOverride`. STUB (wd-errors).
fn check_for_inconsistent_language_version_override(verifiers: &mut [crate::error::UnitVerifier<'_>]) {
    let _ = verifiers;
}

/// Runs [step] on each unit that resolved without a panic; a panic of the
/// step is recorded for the unit.
fn unit_step(
    name: &str,
    verifiers: &mut [crate::error::UnitVerifier<'_>],
    panics: &mut [Option<String>],
    step: &mut dyn FnMut(&mut crate::error::UnitVerifier<'_>),
) {
    for v in verifiers.iter_mut() {
        if panics[v.index].is_some() {
            continue;
        }
        if let Err(e) = catch_unwind(AssertUnwindSafe(|| step(v))) {
            panics[v.index] = Some(format!("{name}: {}", panic_message(&*e)));
        }
    }
}

/// Runs a library-wide [step]; a panic is recorded for the first unit
/// without a panic.
fn library_step(
    name: &str,
    verifiers: &mut [crate::error::UnitVerifier<'_>],
    panics: &mut [Option<String>],
    step: &mut dyn FnMut(&mut [crate::error::UnitVerifier<'_>]),
) {
    if panics.iter().any(Option::is_some) {
        // Dart: an exception in resolution aborts the library analysis.
        return;
    }
    if let Err(e) = catch_unwind(AssertUnwindSafe(|| step(verifiers))) {
        if let Some(slot) = panics.first_mut() {
            *slot = Some(format!("{name}: {}", panic_message(&*e)));
        }
    }
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}

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
            crate::element_binding_visitor::bind_unit(&ctx, &ast, root, unit.fragment, &mut tables, &mut rt);
            crate::resolution_visitor::resolve_unit(
                &ctx,
                unit_ctx,
                &mut ast,
                root,
                &mut tables,
                &mut rt,
                &mut diagnostics,
            );
            let mut resolver = ResolverVisitor::new(ctx, &mut ast, &mut tables, &mut rt, &mut diagnostics, unit_ctx);
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
