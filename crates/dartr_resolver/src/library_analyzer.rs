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
use std::sync::{Arc, Mutex, OnceLock};

use indexmap::IndexMap;

use dartr_ast::{Ast, CompilationUnit, Id};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::Diagnostic;
use dartr_element::{
    Ctx, EId, FId, LibraryElement, LibraryFragment, LocalArena, NoopSink, ResolutionTables,
    TypeProvider, WorldSnapshot,
};
use rayon::prelude::*;

use dartr_constant::DeclaredVariables;

use crate::constant::evaluation::{ConstantEvaluationEngine, ConstantValues, ExternalUnits};
use crate::constant::exhaustiveness::ExhaustivenessCache;
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
    /// The URI diagnostics of the directives of the unit (Dart
    /// `_resolveDirectives`); the driver computes them from its file states
    /// (`dartr_driver::directives`).
    pub directive_diagnostics: Vec<Diagnostic>,
    /// `Configuration.resolvedUri` of the configuration nodes of the unit
    /// (Dart `_resolveUriConfigurations`, computed in
    /// `dartr_driver::directives`).
    pub resolved_uris: dartr_ast::NodeMap<dartr_element::DirectiveUri>,
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
    /// The resolved units of other libraries, for the constants that the
    /// library reads from them (see [`crate::constant`]). `None`: the
    /// constants of other libraries have no value.
    pub external: Option<&'a dyn ExternalUnits>,
    /// Dart `_library.docLibraryImports` as library elements (the
    /// `@docImport` libraries of the defining unit, linked by the caller).
    pub doc_import_libraries: Vec<EId<LibraryElement>>,
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
    /// The diagnostics that an ignore comment removed from [Self::diagnostics]
    /// (Dart `IgnoreValidator` runs on the diagnostics before the filtering,
    /// in `dartr_cli`).
    pub ignored_diagnostics: Vec<Diagnostic>,
    /// The panic message, when resolution of the unit panicked.
    pub panic: Option<String>,
    /// Cached declaration node of each declared fragment in [`Self::tables`].
    pub declarations: std::sync::OnceLock<IndexMap<dartr_element::FragmentId, dartr_ast::NodeId>>,
}

impl ResolvedUnit {
    /// The first declaration node of `fragment` in [`Self::tables`].
    pub fn declaration_of(&self, fragment: dartr_element::FragmentId) -> Option<dartr_ast::NodeId> {
        self.declarations
            .get_or_init(|| {
                let mut map = IndexMap::new();
                for (node, &f) in self.tables.declared_fragment.iter() {
                    map.entry(f).or_insert(node);
                }
                map
            })
            .get(&fragment)
            .copied()
    }
}

/// The analysis result of a library.
pub struct ResolvedLibrary {
    pub library: EId<LibraryElement>,
    pub units: Vec<ResolvedUnit>,
    /// The constant values computed for the library (Dart: the
    /// `evaluationResult` of the elements and annotations), by
    /// `_computeConstants`. The annotation keys use the unit index.
    pub constants: ConstantValues,
}

/// Dart `LibraryAnalyzer.analyze()` without unignorable codes: every
/// diagnostic that an ignore comment names is removed. Callers that have
/// analysis options use [`analyze_library_with_unignorable`].
pub fn analyze_library(input: &LibraryAnalysisInput<'_>) -> ResolvedLibrary {
    analyze_library_with_unignorable(input, &[])
}

/// Dart `LibraryAnalyzer.analyze()`. [unignorable_names] is Dart
/// `_analysisOptions.unignorableDiagnosticCodeNames` (the lower-case code
/// names of `analyzer: cannot-ignore`, as
/// `dartr_project::AnalysisOptions::unignorable_code_names` gives them):
/// `_filterIgnoredDiagnostics` keeps these diagnostics. The result is
/// filtered: a caller must not filter it again with other names.
pub fn analyze_library_with_unignorable(
    input: &LibraryAnalysisInput<'_>,
    unignorable_names: &[String],
) -> ResolvedLibrary {
    analyze_library_with_unignorable_mode(input, unignorable_names, true)
}

/// Like [`analyze_library_with_unignorable`], with [parallel_units] controlling
/// whether units within the library use `par_iter` (disabled when multiple
/// libraries are already being analyzed in parallel to avoid nested Rayon
/// work-stealing stack growth).
pub fn analyze_library_with_unignorable_mode(
    input: &LibraryAnalysisInput<'_>,
    unignorable_names: &[String],
    parallel_units: bool,
) -> ResolvedLibrary {
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
        let mut scopes = LibraryScopes::build(&ctx, input.library);
        scopes.set_doc_import_libraries(&ctx, &input.doc_import_libraries);
        scopes
    };

    // Dart `_parseAndResolve`: the units in parallel when analyzing a single library,
    // or sequentially when outer library analysis is already bounded in parallel.
    let units: Vec<ResolvedUnit> = if parallel_units && input.units.len() > 1 {
        input
            .units
            .par_iter()
            .map(|unit| resolve_file(input, &scopes, &library_features, unit))
            .collect()
    } else {
        input
            .units
            .iter()
            .map(|unit| resolve_file(input, &scopes, &library_features, unit))
            .collect()
    };

    let mut library = ResolvedLibrary {
        library: input.library,
        units,
        constants: ConstantValues::default(),
    };
    compute_constants(input, &mut library);
    compute_diagnostics(input, &scopes, &library_features, &mut library);
    for (unit, unit_input) in library.units.iter_mut().zip(&input.units) {
        // Dart `RecordingDiagnosticListener` is a set: a diagnostic equal
        // to one already recorded (from any pass) is dropped.
        let diagnostics = std::mem::take(&mut unit.diagnostics);
        append_unique(&mut unit.diagnostics, diagnostics);
        filter_ignored_diagnostics(unit, &unit_input.parsed, unignorable_names);
    }
    library
}

/// Dart `_filterIgnoredDiagnostics`: removes the diagnostics that an
/// `// ignore:` or `// ignore_for_file:` comment of the unit ignores, except
/// the diagnostics with an unignorable code ([unignorable_names], Dart
/// `unignorableDiagnosticCodeNames`).
fn filter_ignored_diagnostics(
    unit: &mut ResolvedUnit,
    parsed: &ParsedUnit,
    unignorable_names: &[String],
) {
    if unit.diagnostics.is_empty() {
        return;
    }
    let tokens = &parsed.ast.tokens;
    let begin = parsed.ast[parsed.unit].begin_token;
    let ignore_info = dartr_ast_builder::ignore_info::IgnoreInfo::for_dart(
        tokens,
        begin,
        &parsed.line_info,
        &tokens.source,
    );
    if !ignore_info.has_ignores() {
        return;
    }
    let (kept, ignored) = std::mem::take(&mut unit.diagnostics)
        .into_iter()
        .partition(|d| {
            unignorable_names
                .iter()
                .any(|n| n == d.code.lower_case_name())
                || !ignore_info.ignored(d, &parsed.line_info)
        });
    unit.diagnostics = kept;
    unit.ignored_diagnostics = ignored;
}

/// Dart `_resolveDirectives`: the directive elements are in the linked
/// fragments; the URI diagnostics come with the unit input
/// ([`UnitInput::directive_diagnostics`]).
fn resolve_directives(_input: &LibraryAnalysisInput<'_>, _unit: &UnitInput) {}

/// Dart `_computeConstants`: evaluates the constants of all units
/// (`computeConstants` over `_findConstants`), then
/// `_computeConstantErrors` (the constant verifier) of each unit; the
/// diagnostics of the verifier are appended to the unit diagnostics (Dart
/// runs the verifier first in `_computeVerifyErrors`, after resolution).
fn compute_constants(input: &LibraryAnalysisInput<'_>, library: &mut ResolvedLibrary) {
    static DECLARED_VARIABLES: std::sync::LazyLock<DeclaredVariables> =
        std::sync::LazyLock::new(DeclaredVariables::new);
    let units = std::mem::take(&mut library.units);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let engine = ConstantEvaluationEngine::new(
            input.world,
            input.type_provider,
            input.library,
            &DECLARED_VARIABLES,
            &units,
            input.external,
        );
        let mut constants = Vec::new();
        for (index, unit) in units.iter().enumerate() {
            if unit.panic.is_some() {
                continue;
            }
            let ctx = engine.ctx(unit);
            constants.extend(crate::constant::utilities::find_constants(
                &ctx,
                index as u32,
                unit,
            ));
            constants.extend(crate::constant::utilities::find_dependencies(
                &engine,
                index as u32,
            ));
        }
        crate::constant::compute::compute_constants(&engine, &constants);
        // Dart `_computeConstantErrors` of each unit (in
        // `_computeVerifyErrors`), with the evaluation results of
        // `_computeConstants`.
        let mut cache = ExhaustivenessCache::default();
        let mut verifier_diagnostics = Vec::with_capacity(units.len());
        for (index, unit) in units.iter().enumerate() {
            if unit.panic.is_some() {
                verifier_diagnostics.push(Ok(Vec::new()));
                continue;
            }
            let diagnostics = catch_unwind(AssertUnwindSafe(|| {
                crate::constant::constant_verifier::verify_unit(&engine, &mut cache, index as u32)
            }))
            .map_err(|e| panic_message(&*e));
            verifier_diagnostics.push(diagnostics);
        }
        (engine.values.into_inner(), verifier_diagnostics)
    }));
    library.units = units;
    match result {
        Ok((values, verifier_diagnostics)) => {
            library.constants = values;
            for (unit, diagnostics) in library.units.iter_mut().zip(verifier_diagnostics) {
                match diagnostics {
                    Ok(diagnostics) => append_unique(&mut unit.diagnostics, diagnostics),
                    Err(message) => {
                        if unit.panic.is_none() {
                            unit.panic = Some(format!("constant verifier: {message}"));
                        }
                    }
                }
            }
        }
        Err(e) => {
            let message = panic_message(&*e);
            if let Some(unit) = library.units.first_mut()
                && unit.panic.is_none()
            {
                unit.panic = Some(format!("constant evaluation: {message}"));
            }
        }
    }
}

/// Appends [diagnostics] to [target] like Dart's
/// `RecordingDiagnosticListener` (a `Set<Diagnostic>`): a diagnostic equal
/// to one already recorded (same code, offset, length and message) is
/// dropped.
fn append_unique(target: &mut Vec<Diagnostic>, diagnostics: Vec<Diagnostic>) {
    if diagnostics.is_empty() {
        return;
    }
    let key = |d: &Diagnostic| {
        (
            d.code as *const dartr_diagnostics::DiagnosticCode as usize,
            d.offset,
            d.length,
            d.message.clone(),
        )
    };
    let mut seen: indexmap::IndexSet<_> = target.iter().map(key).collect();
    for d in diagnostics {
        if seen.insert(key(&d)) {
            target.push(d);
        }
    }
}

/// The message of a caught panic.
fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}

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
/// Not here: the FFI verifier, lints
/// (`dartr_lints`), the SDK constraint verifier, `IgnoreValidator` and the
/// ignore filtering (`dartr_cli`). Dart runs the warnings only when
/// `analysisOptions.warning` is set (the default).
fn compute_diagnostics(
    input: &LibraryAnalysisInput<'_>,
    scopes: &LibraryScopes,
    library_features: &dartr_element::FeatureSet,
    library: &mut ResolvedLibrary,
) {
    use crate::error::*;

    let sink = NoopSink;
    // Dart `_computeVerifyErrors` (per unit): the error verifier (D4–D7).
    let mut library_context = crate::error_verifier::LibraryVerificationContext::default();
    for (index, unit) in library.units.iter_mut().enumerate() {
        if unit.panic.is_some() {
            continue;
        }
        library_context.unit_index = index;
        if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
            crate::error_verifier::compute_verify_errors(
                input,
                scopes,
                library_features,
                unit,
                &mut library_context,
            )
        })) {
            unit.panic = Some(format!("ErrorVerifier: {}", panic_message(&*e)));
        }
    }
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
    unit_step(
        "InheritanceOverrideVerifier",
        &mut verifiers,
        &mut panics,
        &mut |v| inheritance_override::verify_unit(v),
    );

    library_step(
        "MemberDuplicateDefinitionVerifier",
        &mut verifiers,
        &mut panics,
        &mut |vs| member_duplicate_definition_verifier::check_library(vs),
    );
    // Dart `_libraryVerificationContext.constructorFieldsVerifier.report()`.
    // The error verifier adds the constructors.
    let mut constructor_fields = std::mem::take(&mut library_context.constructor_fields_verifier);
    library_step(
        "ConstructorFieldsVerifier",
        &mut verifiers,
        &mut panics,
        &mut |vs| constructor_fields.report(vs),
    );

    // Dart `if (_analysisOptions.warning)`: the used local elements of all
    // units, then `_computeWarnings` per unit.
    let mut used_parts = Vec::new();
    for v in &verifiers {
        if panics[v.index].is_some() {
            continue;
        }
        match catch_unwind(AssertUnwindSafe(|| {
            unused_local_elements_verifier::gather_used_local_elements(v)
        })) {
            Ok(used) => used_parts.push(used),
            Err(e) => {
                panics[v.index] = Some(format!(
                    "GatherUsedLocalElementsVisitor: {}",
                    panic_message(&*e)
                ))
            }
        }
    }
    let used = unused_local_elements_verifier::UsedLocalElements::merge(used_parts);
    let prevents_import_warnings =
        imports_verifier::has_diagnostic_reported_that_prevents_import_warnings(&verifiers);
    // Dart `fileAnalysis.importsTracking` (recorded during resolution in
    // Dart; replayed from the resolved units here).
    let imports_tracking = if prevents_import_warnings {
        imports_verifier::ImportsTracking::default()
    } else {
        catch_unwind(AssertUnwindSafe(|| {
            imports_verifier::compute_imports_tracking(&verifiers)
        }))
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
        sdk_constraint_verifier::verify(v);
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

/// Dart `_checkForInconsistentLanguageVersionOverride`: reports a `part`
/// directive whose part has a language version override that is different
/// from the one of the library (or has one when the library has none, or
/// the reverse).
fn check_for_inconsistent_language_version_override(
    verifiers: &mut [crate::error::UnitVerifier<'_>],
) {
    use crate::error::VerifierHost;
    use crate::error::language_version_override_verifier::language_version_token;
    use dartr_element::DirectiveUri;

    let Some(library_analysis) = verifiers.first() else {
        return;
    };
    let library_override = language_version_token(library_analysis.ast, library_analysis.unit)
        .map(|(_, major, minor)| (major, minor));

    // Dart `visitPartDirectives(container)`; [visited] guards against
    // cycles, which Dart does not have.
    fn visit_part_directives(
        verifiers: &mut [crate::error::UnitVerifier<'_>],
        container: usize,
        library_override: Option<(u64, u64)>,
        visited: &mut Vec<usize>,
    ) {
        if visited.contains(&container) {
            return;
        }
        visited.push(container);
        let v = &verifiers[container];
        let ast = v.ast;
        let parts = &v.ctx.fragment(v.fragment).parts;
        let mut reports = Vec::new();
        let mut nested = Vec::new();
        let mut part_index = 0;
        for &directive in ast.list_raw(ast[v.unit].directives) {
            let Some(directive) = ast.cast::<dartr_ast::PartDirective>(directive) else {
                continue;
            };
            let index = part_index;
            part_index += 1;
            let Some(DirectiveUri::Unit {
                library_fragment, ..
            }) = parts.get(index).map(|p| &p.directive.uri)
            else {
                continue;
            };
            let Some(part) = verifiers
                .iter()
                .position(|p| p.fragment == *library_fragment)
            else {
                continue;
            };
            let part_override = language_version_token(verifiers[part].ast, verifiers[part].unit)
                .map(|(_, major, minor)| (major, minor));
            let should_report = match (library_override, part_override) {
                (Some(library), Some(part)) => library != part,
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            if should_report {
                let uri = ast[directive].uri;
                reports.push(
                    dartr_diagnostics::diag::inconsistent_language_version_override()
                        .at_offset(ast.offset(uri) as usize, ast.length(uri) as usize),
                );
            } else {
                nested.push(part);
            }
        }
        // Each unit is visited once, so reporting before the recursion
        // keeps the Dart order of the diagnostics of each unit.
        for d in reports {
            verifiers[container].report(d);
        }
        for part in nested {
            visit_part_directives(verifiers, part, library_override, visited);
        }
    }

    visit_part_directives(verifiers, 0, library_override, &mut Vec::new());
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
    tables.resolved_uri = unit.resolved_uris.clone();
    let mut rt = ResolverTables::new();
    let mut diagnostics: Vec<Diagnostic> = unit.parsed.diagnostics.clone();
    // Dart `_resolveDirectives` runs before `_resolveFile`.
    diagnostics.extend(unit.directive_diagnostics.iter().cloned());
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
        ignored_diagnostics: Vec::new(),
        panic,
        declarations: std::sync::OnceLock::new(),
    }
}

/// Source info for a unit in [`ExternalUnitCache`], either pre-parsed or lazy.
#[derive(Clone)]
pub struct ExternalUnitSource {
    pub uri: Arc<str>,
    pub source: Arc<str>,
    pub package_language_version: (u32, u32),
    pub experiments: Vec<dartr_parser::experimental_flags::ExperimentalFlag>,
    pub parsed: Option<Arc<ParsedUnit>>,
}

impl ExternalUnitSource {
    fn parsed(&self, path: &str) -> Arc<ParsedUnit> {
        if let Some(p) = &self.parsed {
            return p.clone();
        }
        Arc::new(dartr_ast_builder::parse_file(
            &self.source,
            path,
            self.package_language_version,
            &self.experiments,
        ))
    }
}

/// [`ExternalUnits`] that resolves constant expressions of other libraries on
/// demand from their linked `ElementStore::const_ast` and caches evaluated
/// constant values (shared by the analyses of many libraries).
pub struct ExternalUnitCache<'w> {
    world: &'w WorldSnapshot,
    tp: &'w TypeProvider,
    options: AnalysisOptions,
    /// The unit sources by path.
    sources: IndexMap<Arc<str>, ExternalUnitSource>,
    scopes: Mutex<IndexMap<EId<LibraryElement>, Arc<LibraryScopes>>>,
    units: Mutex<IndexMap<FId<LibraryFragment>, Option<Arc<ResolvedUnit>>>>,
    const_initializers: Mutex<
        IndexMap<
            dartr_element::ElementId,
            Option<(Arc<ResolvedUnit>, EId<LibraryElement>, dartr_ast::NodeId)>,
        >,
    >,
    ctor_initializers: Mutex<
        IndexMap<
            dartr_element::ElementId,
            Option<(
                Arc<ResolvedUnit>,
                EId<LibraryElement>,
                Vec<dartr_ast::NodeId>,
            )>,
        >,
    >,
    metadata_annotations: Mutex<
        IndexMap<
            dartr_element::ElementId,
            Option<(
                Arc<ResolvedUnit>,
                EId<LibraryElement>,
                Vec<dartr_ast::NodeId>,
            )>,
        >,
    >,
    evaluated_elements: Mutex<IndexMap<dartr_element::ElementId, dartr_constant::Constant>>,
    evaluated_constructors: Mutex<indexmap::IndexSet<dartr_element::ElementId>>,
}

impl<'w> ExternalUnitCache<'w> {
    /// A cache over [world]; [sources] maps the path of every unit that may
    /// be resolved to its URI and parsed unit.
    pub fn new(
        world: &'w WorldSnapshot,
        tp: &'w TypeProvider,
        options: AnalysisOptions,
        sources: IndexMap<Arc<str>, (Arc<str>, Arc<ParsedUnit>)>,
    ) -> ExternalUnitCache<'w> {
        let sources = sources
            .into_iter()
            .map(|(path, (uri, parsed))| {
                let source = parsed.ast.tokens.source.clone();
                (
                    path,
                    ExternalUnitSource {
                        uri,
                        source,
                        package_language_version: (3, 7),
                        experiments: Vec::new(),
                        parsed: Some(parsed),
                    },
                )
            })
            .collect();
        Self::new_lazy(world, tp, options, sources)
    }

    /// A cache over [world] that parses external units on demand from their source.
    pub fn new_lazy(
        world: &'w WorldSnapshot,
        tp: &'w TypeProvider,
        options: AnalysisOptions,
        sources: IndexMap<Arc<str>, ExternalUnitSource>,
    ) -> ExternalUnitCache<'w> {
        ExternalUnitCache {
            world,
            tp,
            options,
            sources,
            scopes: Mutex::new(IndexMap::new()),
            units: Mutex::new(IndexMap::new()),
            const_initializers: Mutex::new(IndexMap::new()),
            ctor_initializers: Mutex::new(IndexMap::new()),
            metadata_annotations: Mutex::new(IndexMap::new()),
            evaluated_elements: Mutex::new(IndexMap::new()),
            evaluated_constructors: Mutex::new(indexmap::IndexSet::new()),
        }
    }

    fn library_scopes(&self, library: EId<LibraryElement>) -> Arc<LibraryScopes> {
        if let Some(s) = self.scopes.lock().unwrap().get(&library) {
            return s.clone();
        }
        let sink = NoopSink;
        let features = {
            let input = self.input(library);
            let ctx = global_ctx(&input, &sink);
            ctx.get(library).feature_set.clone()
        };
        let ctx = Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: &features,
            req: &sink,
        };
        let scopes = Arc::new(LibraryScopes::build(&ctx, library));
        self.scopes
            .lock()
            .unwrap()
            .entry(library)
            .or_insert(scopes)
            .clone()
    }

    fn input(&self, library: EId<LibraryElement>) -> LibraryAnalysisInput<'w> {
        LibraryAnalysisInput {
            world: self.world,
            type_provider: self.tp,
            library,
            units: Vec::new(),
            options: self.options,
            external: None,
            doc_import_libraries: Vec::new(),
        }
    }

    fn resolve(&self, fragment: FId<LibraryFragment>) -> Option<Arc<ResolvedUnit>> {
        let sink = NoopSink;
        let (library, path) = {
            let ctx = Ctx {
                world: self.world,
                current: None,
                local: None,
                tp: self.tp,
                features: &EMPTY_FEATURES,
                req: &sink,
            };
            let f = ctx.fragment(fragment);
            (f.library, f.source.path.clone())
        };
        let entry = self.sources.get(&path)?;
        let uri = entry.uri.clone();
        let parsed = entry.parsed(&path);
        let scopes = self.library_scopes(library);
        let input = self.input(library);
        let features = {
            let ctx = global_ctx(&input, &sink);
            ctx.get(library).feature_set.clone()
        };
        let unit = UnitInput {
            path,
            uri,
            parsed,
            fragment,
            // Only the constants of this unit are read.
            directive_diagnostics: Vec::new(),
            resolved_uris: dartr_ast::NodeMap::new(),
        };
        let mut resolved = resolve_file(&input, &scopes, &features, &unit);
        if resolved.panic.is_some() {
            return None;
        }
        resolved.ast.shrink_to_fit();
        resolved.tables.compact();
        resolved.rt = ResolverTables::new();
        resolved.local.store.shrink_to_fit();
        resolved.diagnostics = Vec::new();
        Some(Arc::new(resolved))
    }

    pub fn resolved_units(&self) -> Vec<Arc<ResolvedUnit>> {
        self.units
            .lock()
            .unwrap()
            .values()
            .filter_map(|u| u.clone())
            .collect()
    }

    pub fn mini_units(&self) -> Vec<Arc<ResolvedUnit>> {
        let mut out = Vec::new();
        out.extend(
            self.const_initializers
                .lock()
                .unwrap()
                .values()
                .filter_map(|v| v.as_ref().map(|(u, _, _)| u.clone())),
        );
        out.extend(
            self.ctor_initializers
                .lock()
                .unwrap()
                .values()
                .filter_map(|v| v.as_ref().map(|(u, _, _)| u.clone())),
        );
        out.extend(
            self.metadata_annotations
                .lock()
                .unwrap()
                .values()
                .filter_map(|v| v.as_ref().map(|(u, _, _)| u.clone())),
        );
        out
    }

    pub fn evaluated_count(&self) -> usize {
        self.evaluated_elements.lock().unwrap().len()
    }

    fn resolve_const_initializer(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<(Arc<ResolvedUnit>, EId<LibraryElement>, dartr_ast::NodeId)> {
        use dartr_ast::{Expression, NodeId, VariableDeclaration, VariableDeclarationList};
        use dartr_element::{
            ElementFlags, ExtensionElement, FormalParameterElement, FragmentFlags, InstanceElement,
            InterfaceElement, Tag, TypeId,
        };

        let sink = NoopSink;
        let gctx = Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: &EMPTY_FEATURES,
            req: &sink,
        };
        let data = gctx.element_data(element)?;
        let library = data.library?;
        let mut fragment_iter = Some(data.first_fragment);
        let mut init_info = None;
        while let Some(f) = fragment_iter {
            if let Some(vf) = f.cast::<dartr_element::VariableFragment>()
                && let Some(expr_id) = gctx
                    .store(f.store())
                    .variable_fragment(vf)
                    .constant_initializer
                && let Some(lf) = dartr_element::diagnostics::library_fragment_of(&gctx, f)
            {
                init_info = Some((f.store(), lf, expr_id));
            }
            fragment_iter = gctx.fragment_data(f).and_then(|d| d.next_fragment);
        }
        let (store_id, lf, expr_id) = init_info?;
        let const_ast = gctx.store(store_id).const_ast.try_get()?.clone();
        let frag_source = gctx.fragment(lf).source.clone();

        let library_features = gctx.get(library).feature_set.clone();
        let unit_features = experimental_features(&library_features);
        let scopes = self.library_scopes(library);

        let local = self.world.generation.new_local_arena();
        let mut ast = Ast::new(dartr_syntax::Tokens::new(Arc::from("")));
        let mut tables = ResolutionTables::new();
        let mut rt = ResolverTables::new();

        let copied_expr = copy_from_const_ast(&const_ast, expr_id.0, &mut ast, &mut rt);
        let is_const = element.tag() == Tag::FormalParameter
            || crate::element_ext::first_fragment_flags(&gctx, element)
                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
            || crate::element_ext::is_enum_constant(&gctx, element);
        let const_tok = is_const.then(|| {
            ast.tokens.push_synthetic_string(
                dartr_syntax::TokenType::IDENTIFIER,
                "const",
                0,
                5,
                Some(0),
            )
        });
        let name_tok = ast.tokens.push_synthetic_string(
            dartr_syntax::TokenType::IDENTIFIER,
            "",
            0,
            0,
            Some(0),
        );
        let empty_meta = ast.new_list(std::iter::empty());
        let var_decl = ast.add(VariableDeclaration {
            documentation_comment: None,
            metadata: empty_meta,
            name: name_tok,
            equals: None,
            initializer: Some(Id::<Expression>::from_raw(copied_expr)),
        });
        let vars_list = ast.new_list([var_decl]);
        let empty_meta2 = ast.new_list(std::iter::empty());
        let var_list = ast.add(VariableDeclarationList {
            documentation_comment: None,
            metadata: empty_meta2,
            late_keyword: None,
            keyword: const_tok,
            type_: None,
            variables: vars_list,
        });
        let dummy_unit = make_dummy_unit(&mut ast);

        let ancestors = enclosing_ancestors(&gctx, element);
        let mut enclosing_instance: Option<EId<InstanceElement>> = None;
        let mut enclosing_class: Option<EId<InterfaceElement>> = None;
        let mut enclosing_extension: Option<EId<ExtensionElement>> = None;
        for &anc in &ancestors {
            if let Some(inst) = anc.cast::<InstanceElement>() {
                enclosing_instance = Some(inst);
                if anc.tag() == Tag::Extension {
                    enclosing_extension = anc.cast::<ExtensionElement>();
                } else {
                    enclosing_class = anc.cast::<InterfaceElement>();
                }
            }
        }
        let is_field = element.tag() == Tag::Field;
        let is_static_field = is_field
            && crate::element_ext::first_fragment_flags(&gctx, element)
                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC);
        let is_late_field = is_field
            && crate::element_ext::first_fragment_flags(&gctx, element)
                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE);
        let primary_ctor = if is_field && !is_static_field && !is_late_field {
            enclosing_instance
                .and_then(|inst| crate::scope_context::primary_constructor_of(&gctx, inst))
        } else {
            None
        };
        let primary_params: Option<Vec<EId<FormalParameterElement>>> =
            primary_ctor.map(|c| gctx.executable(c.upcast()).formal_params.clone());

        let is_inferred = element
            .cast::<dartr_element::PropertyInducingElement>()
            .is_some_and(|p| {
                gctx.property_inducing(p)
                    .flags
                    .has(ElementFlags::PROPERTY_INDUCING_ELEMENT_IS_TYPE_INFERRED_FROM_INITIALIZER)
            })
            || (is_field && crate::element_ext::is_enum_constant(&gctx, element));
        let context_type = if is_inferred {
            TypeId::UNKNOWN
        } else {
            crate::element_ext::variable_type(&gctx, element)
        };

        let ctx = Ctx {
            world: self.world,
            current: None,
            local: Some(&local),
            tp: self.tp,
            features: &library_features,
            req: &sink,
        };
        let unit_ctx = UnitContext {
            library,
            fragment: lf,
            scopes: &scopes,
            options: self.options,
            features: unit_features,
        };
        let mut diagnostics = Vec::new();

        let resolved_node: NodeId = catch_unwind(AssertUnwindSafe(|| {
            crate::element_binding_visitor::bind_subtree(
                &ctx,
                &ast,
                lf,
                lf.raw(),
                copied_expr,
                &mut tables,
                &mut rt,
            );
            {
                let mut visitor = crate::resolution_visitor::ResolutionVisitor::new(
                    ctx,
                    unit_ctx,
                    &mut tables,
                    &mut rt,
                    &mut diagnostics,
                );
                push_ancestor_scopes(&ctx, &mut visitor, &ancestors, false);
                if is_field {
                    visitor.scope_context.set_in_static_member(is_static_field);
                    if let Some(c) = primary_ctor {
                        visitor.scope_context.push_constructor_initializer_scope(c);
                    }
                }
                let expr_id = ast[var_decl].initializer?;
                visitor.visit(&mut ast, expr_id);
                for (node, result) in visitor.scope_context.finish_recorded_lookups() {
                    visitor
                        .rt
                        .this_scope_lookup
                        .insert(node, (result.getter, result.setter));
                }
            }
            let expr_id = ast[var_decl].initializer?;
            let mut resolver = ResolverVisitor::new(
                ctx,
                &mut ast,
                &mut tables,
                &mut rt,
                &mut diagnostics,
                unit_ctx,
            );
            resolver.enclosing_extension = enclosing_extension;
            resolver.prepare_enclosing_declarations(enclosing_class, None);
            {
                let ast_ref = &*resolver.ast;
                let tables_ref = &*resolver.tables;
                resolver.flow_analysis.body_or_initializer_enter(
                    ast_ref,
                    tables_ref,
                    var_list.raw(),
                    primary_params.as_deref(),
                    None,
                );
            }
            resolver.analyze_expression_node(expr_id, context_type);
            let rewritten = resolver.pop_rewrite().unwrap_or(expr_id);
            resolver.flow_analysis.body_or_initializer_exit();
            resolver.flush_type_analyzer_errors();
            Some(rewritten.raw())
        }))
        .ok()
        .flatten()?;

        crate::ast_resolver::Globalizer::new(&ctx).globalize_tables(&mut tables);
        ast.shrink_to_fit();
        tables.compact();
        let mut local = local;
        local.store.shrink_to_fit();

        let unit = Arc::new(ResolvedUnit {
            path: frag_source.path,
            uri: frag_source.uri,
            fragment: lf,
            ast,
            unit: dummy_unit,
            tables,
            rt: ResolverTables::new(),
            local,
            diagnostics: Vec::new(),
            ignored_diagnostics: Vec::new(),
            panic: None,
            declarations: OnceLock::new(),
        });
        Some((unit, library, resolved_node))
    }

    fn resolve_ctor_initializers(
        &self,
        constructor: dartr_element::ElementId,
    ) -> Option<(
        Arc<ResolvedUnit>,
        EId<LibraryElement>,
        Vec<dartr_ast::NodeId>,
    )> {
        use dartr_ast::{
            ConstructorInitializer, EmptyFunctionBody, NodeId, PrimaryConstructorBody,
        };
        use dartr_element::{
            ConstructorElement, ExecutableElement, FragmentFlags, InterfaceElement,
        };

        let sink = NoopSink;
        let gctx = Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: &EMPTY_FEATURES,
            req: &sink,
        };
        let data = gctx.element_data(constructor)?;
        let library = data.library?;
        let mut fragment_iter = Some(data.first_fragment);
        let mut inits = Vec::new();
        let mut store_and_lf = None;
        while let Some(f) = fragment_iter {
            let Some(fd) = gctx.fragment_data(f) else {
                break;
            };
            if fd
                .flags
                .get()
                .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
                && let Some(cf) = f.cast::<dartr_element::ConstructorFragment>()
                && let Some(list) = gctx
                    .store(f.store())
                    .fragment(cf)
                    .constant_initializers
                    .try_get()
                && !list.is_empty()
                && let Some(lf) = dartr_element::diagnostics::library_fragment_of(&gctx, f)
            {
                store_and_lf = Some((f.store(), lf));
                inits.extend(list.iter().copied());
            }
            fragment_iter = fd.next_fragment;
        }
        let (store_id, lf) = store_and_lf?;
        if inits.is_empty() {
            return None;
        }
        let const_ast = gctx.store(store_id).const_ast.try_get()?.clone();
        let frag_source = gctx.fragment(lf).source.clone();

        let library_features = gctx.get(library).feature_set.clone();
        let unit_features = experimental_features(&library_features);
        let scopes = self.library_scopes(library);

        let local = self.world.generation.new_local_arena();
        let mut ast = Ast::new(dartr_syntax::Tokens::new(Arc::from("")));
        let mut tables = ResolutionTables::new();
        let mut rt = ResolverTables::new();

        let copied_inits: Vec<Id<ConstructorInitializer>> = inits
            .iter()
            .map(|id| {
                let raw = copy_from_const_ast(&const_ast, id.0, &mut ast, &mut rt);
                Id::<ConstructorInitializer>::from_raw(raw)
            })
            .collect();
        let init_list = ast.new_list(copied_inits.iter().copied());
        let empty_meta = ast.new_list(std::iter::empty());
        let this_tok = ast.tokens.push_synthetic_string(
            dartr_syntax::TokenType::IDENTIFIER,
            "this",
            0,
            4,
            Some(0),
        );
        let semi_tok = ast.tokens.push_synthetic_string(
            dartr_syntax::TokenType::SEMICOLON,
            ";",
            0,
            1,
            Some(0),
        );
        let empty_body = ast.add(EmptyFunctionBody {
            semicolon: semi_tok,
        });
        let parent_body = ast.add(PrimaryConstructorBody {
            documentation_comment: None,
            metadata: empty_meta,
            this_keyword: this_tok,
            colon: None,
            initializers: init_list,
            body: empty_body.upcast(),
        });
        let dummy_unit = make_dummy_unit(&mut ast);

        let ancestors = enclosing_ancestors(&gctx, constructor);
        let enclosing_class = ancestors
            .iter()
            .rev()
            .find_map(|a| a.cast::<InterfaceElement>());
        let ctor_eid = constructor.cast::<ConstructorElement>()?;
        let exec_eid = constructor.cast::<ExecutableElement>()?;

        let ctx = Ctx {
            world: self.world,
            current: None,
            local: Some(&local),
            tp: self.tp,
            features: &library_features,
            req: &sink,
        };
        let unit_ctx = UnitContext {
            library,
            fragment: lf,
            scopes: &scopes,
            options: self.options,
            features: unit_features,
        };
        let mut diagnostics = Vec::new();

        let resolved_nodes: Vec<NodeId> = catch_unwind(AssertUnwindSafe(|| {
            for &init in &copied_inits {
                crate::element_binding_visitor::bind_subtree(
                    &ctx,
                    &ast,
                    lf,
                    lf.raw(),
                    init.raw(),
                    &mut tables,
                    &mut rt,
                );
            }
            {
                let mut visitor = crate::resolution_visitor::ResolutionVisitor::new(
                    ctx,
                    unit_ctx,
                    &mut tables,
                    &mut rt,
                    &mut diagnostics,
                );
                push_ancestor_scopes(&ctx, &mut visitor, &ancestors, false);
                let ctor_tps = crate::scope_context::element_type_parameters(&ctx, constructor);
                if !ctor_tps.is_empty() {
                    visitor.scope_context.push_type_parameter_scope(&ctor_tps);
                }
                visitor
                    .scope_context
                    .push_constructor_initializer_scope(ctor_eid);
                let init_nodes: Vec<NodeId> = ast
                    .list(ast[parent_body].initializers)
                    .iter()
                    .map(|i| i.raw())
                    .collect();
                for init in init_nodes {
                    visitor.visit(&mut ast, init);
                }
                for (node, result) in visitor.scope_context.finish_recorded_lookups() {
                    visitor
                        .rt
                        .this_scope_lookup
                        .insert(node, (result.getter, result.setter));
                }
            }
            let init_nodes: Vec<NodeId> = ast
                .list(ast[parent_body].initializers)
                .iter()
                .map(|i| i.raw())
                .collect();
            let mut resolver = ResolverVisitor::new(
                ctx,
                &mut ast,
                &mut tables,
                &mut rt,
                &mut diagnostics,
                unit_ctx,
            );
            resolver.prepare_enclosing_declarations(enclosing_class, Some(exec_eid));
            let formal_parameters = resolver.formal_parameters_of(exec_eid);
            {
                let ast_ref = &*resolver.ast;
                let tables_ref = &*resolver.tables;
                resolver.flow_analysis.body_or_initializer_enter(
                    ast_ref,
                    tables_ref,
                    parent_body.raw(),
                    Some(&formal_parameters),
                    None,
                );
            }
            resolver.flow_analysis.executable_declaration_enter(
                parent_body.raw(),
                Some(&formal_parameters),
                false,
            );
            for init in &init_nodes {
                resolver.visit_node(*init);
            }
            resolver.flow_analysis.body_or_initializer_exit();
            resolver.flush_type_analyzer_errors();
            let final_inits: Vec<NodeId> = resolver
                .ast
                .list(resolver.ast[parent_body].initializers)
                .iter()
                .map(|i| i.raw())
                .collect();
            Some(final_inits)
        }))
        .ok()
        .flatten()?;

        crate::ast_resolver::Globalizer::new(&ctx).globalize_tables(&mut tables);
        ast.shrink_to_fit();
        tables.compact();
        let mut local = local;
        local.store.shrink_to_fit();

        let unit = Arc::new(ResolvedUnit {
            path: frag_source.path,
            uri: frag_source.uri,
            fragment: lf,
            ast,
            unit: dummy_unit,
            tables,
            rt: ResolverTables::new(),
            local,
            diagnostics: Vec::new(),
            ignored_diagnostics: Vec::new(),
            panic: None,
            declarations: OnceLock::new(),
        });
        Some((unit, library, resolved_nodes))
    }

    fn resolve_metadata_annotations(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<(
        Arc<ResolvedUnit>,
        EId<LibraryElement>,
        Vec<dartr_ast::NodeId>,
    )> {
        use dartr_ast::{Annotation, NodeId, VariableDeclarationList};
        use dartr_element::{ExtensionElement, InterfaceElement, Tag};

        let sink = NoopSink;
        let gctx = Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: &EMPTY_FEATURES,
            req: &sink,
        };
        let (library, store_id, lf, anns) = if element.tag() == Tag::Library {
            let lib = element.cast::<LibraryElement>()?;
            let lib_data = gctx.get(lib);
            if lib_data.metadata.annotations.is_empty() {
                return None;
            }
            let first = lib_data.first_fragment();
            let anns: Vec<dartr_element::ConstExprId> = lib_data
                .metadata
                .annotations
                .iter()
                .map(|a| a.annotation_ast)
                .collect();
            (lib, element.store(), first, anns)
        } else {
            let data = gctx.element_data(element)?;
            let lib = data.library?;
            let mut fragment_iter = Some(data.first_fragment);
            let mut anns = Vec::new();
            let mut store_and_lf = None;
            while let Some(f) = fragment_iter {
                let Some(fd) = gctx.fragment_data(f) else {
                    break;
                };
                if !fd.metadata.annotations.is_empty()
                    && let Some(lf) = dartr_element::diagnostics::library_fragment_of(&gctx, f)
                {
                    if store_and_lf.is_none() {
                        store_and_lf = Some((f.store(), lf));
                    }
                    anns.extend(fd.metadata.annotations.iter().map(|a| a.annotation_ast));
                }
                fragment_iter = fd.next_fragment;
            }
            let (store_id, lf) = store_and_lf?;
            if anns.is_empty() {
                return None;
            }
            (lib, store_id, lf, anns)
        };

        let const_ast = gctx.store(store_id).const_ast.try_get()?.clone();
        let frag_source = gctx.fragment(lf).source.clone();

        let library_features = gctx.get(library).feature_set.clone();
        let unit_features = experimental_features(&library_features);
        let scopes = self.library_scopes(library);

        let local = self.world.generation.new_local_arena();
        let mut ast = Ast::new(dartr_syntax::Tokens::new(Arc::from("")));
        let mut tables = ResolutionTables::new();
        let mut rt = ResolverTables::new();

        let copied_anns: Vec<Id<Annotation>> = anns
            .iter()
            .map(|id| {
                let raw = copy_from_const_ast(&const_ast, id.0, &mut ast, &mut rt);
                Id::<Annotation>::from_raw(raw)
            })
            .collect();
        let meta_list = ast.new_list(copied_anns.iter().copied());
        let empty_vars = ast.new_list(std::iter::empty());
        let parent_list = ast.add(VariableDeclarationList {
            documentation_comment: None,
            metadata: meta_list,
            late_keyword: None,
            keyword: None,
            type_: None,
            variables: empty_vars,
        });
        let dummy_unit = make_dummy_unit(&mut ast);

        let ancestors = enclosing_ancestors(&gctx, element);
        let is_type_param = element.tag() == Tag::TypeParameter;
        let mut enclosing_class: Option<EId<InterfaceElement>> = None;
        let mut enclosing_extension: Option<EId<ExtensionElement>> = None;
        if !is_type_param {
            for &anc in &ancestors {
                if anc.tag() == Tag::Extension {
                    enclosing_extension = anc.cast::<ExtensionElement>();
                } else if let Some(cls) = anc.cast::<InterfaceElement>() {
                    enclosing_class = Some(cls);
                }
            }
        }

        let ctx = Ctx {
            world: self.world,
            current: None,
            local: Some(&local),
            tp: self.tp,
            features: &library_features,
            req: &sink,
        };
        let unit_ctx = UnitContext {
            library,
            fragment: lf,
            scopes: &scopes,
            options: self.options,
            features: unit_features,
        };
        let mut diagnostics = Vec::new();

        let resolved_nodes: Vec<NodeId> = catch_unwind(AssertUnwindSafe(|| {
            for &ann in &copied_anns {
                crate::element_binding_visitor::bind_subtree(
                    &ctx,
                    &ast,
                    lf,
                    lf.raw(),
                    ann.raw(),
                    &mut tables,
                    &mut rt,
                );
            }
            {
                let mut visitor = crate::resolution_visitor::ResolutionVisitor::new(
                    ctx,
                    unit_ctx,
                    &mut tables,
                    &mut rt,
                    &mut diagnostics,
                );
                push_ancestor_scopes(&ctx, &mut visitor, &ancestors, is_type_param);
                let ann_nodes: Vec<Id<Annotation>> = ast.list(ast[parent_list].metadata).to_vec();
                for ann in ann_nodes {
                    visitor.visit(&mut ast, ann);
                }
                for (node, result) in visitor.scope_context.finish_recorded_lookups() {
                    visitor
                        .rt
                        .this_scope_lookup
                        .insert(node, (result.getter, result.setter));
                }
            }
            let ann_nodes: Vec<Id<Annotation>> = ast.list(ast[parent_list].metadata).to_vec();
            let mut resolver = ResolverVisitor::new(
                ctx,
                &mut ast,
                &mut tables,
                &mut rt,
                &mut diagnostics,
                unit_ctx,
            );
            resolver.enclosing_extension = enclosing_extension;
            resolver.prepare_enclosing_declarations(enclosing_class, None);
            for ann in &ann_nodes {
                resolver.visit_annotation(*ann);
            }
            resolver.flush_type_analyzer_errors();
            let final_anns: Vec<NodeId> = resolver
                .ast
                .list(resolver.ast[parent_list].metadata)
                .iter()
                .map(|a| a.raw())
                .collect();
            Some(final_anns)
        }))
        .ok()
        .flatten()?;

        crate::ast_resolver::Globalizer::new(&ctx).globalize_tables(&mut tables);
        ast.shrink_to_fit();
        tables.compact();
        let mut local = local;
        local.store.shrink_to_fit();

        let unit = Arc::new(ResolvedUnit {
            path: frag_source.path,
            uri: frag_source.uri,
            fragment: lf,
            ast,
            unit: dummy_unit,
            tables,
            rt: ResolverTables::new(),
            local,
            diagnostics: Vec::new(),
            ignored_diagnostics: Vec::new(),
            panic: None,
            declarations: OnceLock::new(),
        });
        Some((unit, library, resolved_nodes))
    }
}

fn copy_from_const_ast(
    const_ast: &dartr_element::ConstAst,
    src_node: dartr_ast::NodeId,
    dst: &mut Ast,
    rt: &mut ResolverTables,
) -> dartr_ast::NodeId {
    let src = const_ast.ast();
    let dst_node = dartr_ast::copy_subtree(src, src_node, dst);
    let mut stack = vec![(src_node, dst_node)];
    while let Some((s, d)) = stack.pop() {
        if const_ast.is_dot_shorthand(s) {
            rt.dot_shorthand.insert(d, ());
        }
        for (sc, dc) in src.children(s).into_iter().zip(dst.children(d)) {
            stack.push((sc, dc));
        }
    }
    dst_node
}

fn make_dummy_unit(ast: &mut Ast) -> Id<CompilationUnit> {
    let eof = ast
        .tokens
        .push_synthetic_string(dartr_syntax::TokenType::EOF, "", 0, 0, Some(0));
    let directives = ast.new_list(std::iter::empty());
    let declarations = ast.new_list(std::iter::empty());
    ast.add(CompilationUnit {
        begin_token: eof,
        script_tag: None,
        directives,
        declarations,
        end_token: eof,
    })
}

fn experimental_features(
    feature_set: &dartr_element::FeatureSet,
) -> dartr_parser::experimental_features::ExperimentalFeatures {
    use dartr_parser::experimental_features::{ExperimentalFeatures, ExperimentalFlag};
    let mut features = ExperimentalFeatures::none();
    for &flag in ExperimentalFlag::VALUES {
        if feature_set.is_enabled(flag.name()) {
            features = features.with(flag, true);
        }
    }
    features
}

fn enclosing_ancestors(
    ctx: &Ctx<'_>,
    element: dartr_element::ElementId,
) -> Vec<dartr_element::ElementId> {
    let mut ancestors = Vec::new();
    let mut cur = ctx.element_data(element).and_then(|d| d.enclosing);
    while let Some(enc) = cur {
        if enc.tag() == dartr_element::Tag::Library {
            break;
        }
        ancestors.push(enc);
        cur = ctx.element_data(enc).and_then(|d| d.enclosing);
    }
    ancestors.reverse();
    ancestors
}

fn push_ancestor_scopes<'a>(
    ctx: &Ctx<'a>,
    visitor: &mut crate::resolution_visitor::ResolutionVisitor<'_, 'a>,
    ancestors: &[dartr_element::ElementId],
    skip_last_instance_scope: bool,
) {
    use dartr_element::{FragmentFlags, InstanceElement, InterfaceElement, Tag};
    for (idx, &anc) in ancestors.iter().enumerate() {
        let tps = crate::scope_context::element_type_parameters(ctx, anc);
        if !tps.is_empty() {
            visitor.scope_context.push_type_parameter_scope(&tps);
        }
        if let Some(inst) = anc.cast::<InstanceElement>() {
            if skip_last_instance_scope && idx + 1 == ancestors.len() {
                continue;
            }
            if anc.tag() == Tag::Extension {
                visitor
                    .scope_context
                    .push_extension_scope(EId::from_raw(anc));
            } else {
                visitor.named_type_resolver.enclosing_class = anc.cast::<InterfaceElement>();
                visitor.scope_context.push_instance_scope(inst);
            }
        } else if matches!(anc.tag(), Tag::Method | Tag::Getter | Tag::Setter) {
            let is_static = crate::element_ext::first_fragment_flags(ctx, anc)
                .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC);
            visitor.scope_context.set_in_static_member(is_static);
        }
    }
}

static EMPTY_FEATURES: std::sync::LazyLock<dartr_element::FeatureSet> =
    std::sync::LazyLock::new(dartr_element::FeatureSet::default);

impl ExternalUnits for ExternalUnitCache<'_> {
    fn resolved_unit(&self, fragment: FId<LibraryFragment>) -> Option<Arc<ResolvedUnit>> {
        if let Some(cached) = self.units.lock().unwrap().get(&fragment) {
            return cached.clone();
        }
        let resolved = self.resolve(fragment);
        self.units
            .lock()
            .unwrap()
            .entry(fragment)
            .or_insert(resolved)
            .clone()
    }

    fn resolved_constant_initializer(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<(Arc<ResolvedUnit>, EId<LibraryElement>, dartr_ast::NodeId)> {
        if let Some(cached) = self.const_initializers.lock().unwrap().get(&element) {
            return cached.clone();
        }
        let resolved = self.resolve_const_initializer(element);
        self.const_initializers
            .lock()
            .unwrap()
            .entry(element)
            .or_insert(resolved)
            .clone()
    }

    fn resolved_constructor_initializers(
        &self,
        constructor: dartr_element::ElementId,
    ) -> Option<(
        Arc<ResolvedUnit>,
        EId<LibraryElement>,
        Vec<dartr_ast::NodeId>,
    )> {
        if let Some(cached) = self.ctor_initializers.lock().unwrap().get(&constructor) {
            return cached.clone();
        }
        let resolved = self.resolve_ctor_initializers(constructor);
        self.ctor_initializers
            .lock()
            .unwrap()
            .entry(constructor)
            .or_insert(resolved)
            .clone()
    }

    fn resolved_metadata_annotations(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<(
        Arc<ResolvedUnit>,
        EId<LibraryElement>,
        Vec<dartr_ast::NodeId>,
    )> {
        if let Some(cached) = self.metadata_annotations.lock().unwrap().get(&element) {
            return cached.clone();
        }
        let resolved = self.resolve_metadata_annotations(element);
        self.metadata_annotations
            .lock()
            .unwrap()
            .entry(element)
            .or_insert(resolved)
            .clone()
    }

    fn evaluated_element(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<dartr_constant::Constant> {
        self.evaluated_elements
            .lock()
            .unwrap()
            .get(&element)
            .cloned()
    }

    fn set_evaluated_element(
        &self,
        element: dartr_element::ElementId,
        value: dartr_constant::Constant,
    ) {
        self.evaluated_elements
            .lock()
            .unwrap()
            .entry(element)
            .or_insert(value);
    }

    fn is_constructor_evaluated(&self, constructor: dartr_element::ElementId) -> bool {
        self.evaluated_constructors
            .lock()
            .unwrap()
            .contains(&constructor)
    }

    fn set_constructor_evaluated(&self, constructor: dartr_element::ElementId) {
        self.evaluated_constructors
            .lock()
            .unwrap()
            .insert(constructor);
    }
}

/// Dart `computeConstantValue()` of [elements] (variables of the library
/// of [library] or of other libraries) after the analysis of [library]:
/// the value, or `None` for an invalid constant (Dart `null`).
pub fn compute_constant_values(
    input: &LibraryAnalysisInput<'_>,
    library: &ResolvedLibrary,
    elements: &[dartr_element::ElementId],
) -> IndexMap<dartr_element::ElementId, Option<String>> {
    static DECLARED_VARIABLES: std::sync::LazyLock<DeclaredVariables> =
        std::sync::LazyLock::new(DeclaredVariables::new);
    let engine = ConstantEvaluationEngine::new(
        input.world,
        input.type_provider,
        input.library,
        &DECLARED_VARIABLES,
        &library.units,
        input.external,
    );
    *engine.values.borrow_mut() = library.constants.clone();
    let mut result = IndexMap::new();
    for &e in elements {
        let value = catch_unwind(AssertUnwindSafe(|| {
            engine.compute_constant_value_of(e).map(|v| {
                let ctx = engine.global_ctx();
                let ts = dartr_typesystem::TypeSystem::new(ctx);
                v.display(&ts)
            })
        }))
        .unwrap_or(None);
        result.insert(e, value);
    }
    result
}
