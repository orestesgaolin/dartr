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
    /// The constant values computed for the library (Dart: the
    /// `evaluationResult` of the elements and annotations), by
    /// `_computeConstants`. The annotation keys use the unit index.
    pub constants: ConstantValues,
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
        constants: ConstantValues::default(),
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
            constants.extend(crate::constant::utilities::find_constants(&ctx, index as u32, unit));
            constants.extend(crate::constant::utilities::find_dependencies(&engine, index as u32));
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

/// Dart `_checkForInconsistentLanguageVersionOverride`: reports a `part`
/// directive whose part has a language version override that is different
/// from the one of the library (or has one when the library has none, or
/// the reverse).
fn check_for_inconsistent_language_version_override(verifiers: &mut [crate::error::UnitVerifier<'_>]) {
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
            let Some(DirectiveUri::Unit { library_fragment, .. }) = parts.get(index).map(|p| &p.directive.uri) else {
                continue;
            };
            let Some(part) = verifiers.iter().position(|p| p.fragment == *library_fragment) else {
                continue;
            };
            let part_override =
                language_version_token(verifiers[part].ast, verifiers[part].unit).map(|(_, major, minor)| (major, minor));
            let should_report = match (library_override, part_override) {
                (Some(library), Some(part)) => library != part,
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            if should_report {
                let uri = ast[directive].uri;
                reports.push(dartr_diagnostics::diag::inconsistent_language_version_override().at_offset(
                    ast.offset(uri) as usize,
                    ast.length(uri) as usize,
                ));
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

/// [`ExternalUnits`] that resolves the units of other libraries on demand
/// and keeps them (shared by the analyses of many libraries).
pub struct ExternalUnitCache<'w> {
    world: &'w WorldSnapshot,
    tp: &'w TypeProvider,
    options: AnalysisOptions,
    /// The parsed units by path, with their URIs.
    sources: IndexMap<Arc<str>, (Arc<str>, Arc<ParsedUnit>)>,
    scopes: Mutex<IndexMap<EId<LibraryElement>, Arc<LibraryScopes>>>,
    units: Mutex<IndexMap<FId<LibraryFragment>, Arc<OnceLock<Option<Arc<ResolvedUnit>>>>>>,
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
        ExternalUnitCache {
            world,
            tp,
            options,
            sources,
            scopes: Mutex::new(IndexMap::new()),
            units: Mutex::new(IndexMap::new()),
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
        let (uri, parsed) = self.sources.get(&path)?.clone();
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
        };
        let resolved = resolve_file(&input, &scopes, &features, &unit);
        if resolved.panic.is_some() {
            return None;
        }
        Some(Arc::new(resolved))
    }
}

static EMPTY_FEATURES: std::sync::LazyLock<dartr_element::FeatureSet> =
    std::sync::LazyLock::new(dartr_element::FeatureSet::default);

impl ExternalUnits for ExternalUnitCache<'_> {
    fn resolved_unit(&self, fragment: FId<LibraryFragment>) -> Option<Arc<ResolvedUnit>> {
        let cell = self
            .units
            .lock()
            .unwrap()
            .entry(fragment)
            .or_insert_with(|| Arc::new(OnceLock::new()))
            .clone();
        cell.get_or_init(|| self.resolve(fragment)).clone()
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
