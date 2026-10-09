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
/// `_computeConstantErrors` (the constant verifier) of each unit.
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
        engine.values.into_inner()
    }));
    library.units = units;
    match result {
        Ok(values) => library.constants = values,
        Err(e) => {
            let message = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string());
            if let Some(unit) = library.units.first_mut()
                && unit.panic.is_none()
            {
                unit.panic = Some(format!("constant evaluation: {message}"));
            }
        }
    }
}

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
