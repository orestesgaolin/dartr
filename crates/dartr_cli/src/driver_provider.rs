// Dart source: pkg/analyzer/lib/src/dart/analysis/driver.dart
// (AnalysisDriver.getErrors, _computeErrors, _computeAnalysisResult: one
// driver per analysis context, the library of a file, the diagnostics of the
// units of the library), pkg/analysis_server/lib/src/context_manager.dart
// (one driver per context root)

//! [DriverProvider]: the full diagnostics of `dartr analyze`, from the
//! analysis driver (`dartr_driver`): parse, resolution, constant and (when
//! `dartr_resolver` reports them) verifier diagnostics, then the same steps
//! as [crate::provider::ParseOnlyProvider]: the AST-only lint rules, the
//! ignore-comment diagnostics and the `// ignore:` filtering.
//!
//! [DriverSession] keeps one [Driver] per analysis context of the
//! collection. `dartr analyze` uses a new session for each run; the language
//! server keeps the session and reports file changes with
//! [DriverSession::change_file] (design `docs/design/semantics.md` §4.2).
//!
//! Per context (design §2.5): read and parse the requested files and every
//! file they reference in parallel, find the library of each file, link the
//! library cycles with the DAG scheduler (a cycle starts when its
//! dependencies are linked), then analyze all libraries in parallel.
//!
//! Fallbacks to the parse-only pipeline (the file still gets its parse
//! diagnostics, lints and ignore handling): a part file without a library
//! (Dart analyzes it as a library of its own), a context whose linking
//! panics, a library whose analysis panics. `DARTR_DEBUG_DRIVER=1` writes
//! these cases and the resolver panics of units to stderr.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use dartr_diagnostics::Diagnostic;
use dartr_driver::driver::Driver;
use dartr_driver::file_state::FileId;
use dartr_element::Generation;
use dartr_lints::{RuleContextUnit, lint_library_unfiltered};
use dartr_project::AnalysisContextCollection;
use dartr_project::paths;
use dartr_resolver::library_analyzer::{ResolvedLibrary, UnitInput};
use dartr_resolver::options::AnalysisOptions as ResolverOptions;
use indexmap::IndexMap;

use crate::provider::{
    AnalyzedFile, DiagnosticsProvider, FileDiagnostics, FileSettings, diagnostics_with_reader,
    finish_file, pool,
};

/// The diagnostics of the analysis driver. See the module documentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriverProvider;

impl DiagnosticsProvider for DriverProvider {
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics> {
        self.diagnostics_for_files(collection, std::slice::from_ref(file))
            .pop()
    }

    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        DriverSession::default().diagnostics(collection, files)
    }
}

fn debug() -> bool {
    std::env::var_os("DARTR_DEBUG_DRIVER").is_some()
}

/// The settings of the files of one context, by analysis options folder
/// (Dart `AnalysisOptionsMap.getOptions`), owned so that the analysis
/// threads can read them.
struct ContextOptions {
    /// The options folders, deepest first, with their settings.
    folders: Vec<(String, Arc<FileSettings>, ResolverOptions)>,
    default: (Arc<FileSettings>, ResolverOptions),
}

impl ContextOptions {
    fn new(collection: &AnalysisContextCollection, context: usize) -> ContextOptions {
        let map = collection.options_map(&collection.contexts[context]);
        let settings = |o: &dartr_project::AnalysisOptions| {
            (
                Arc::new(FileSettings::new(o)),
                ResolverOptions {
                    strict_casts: o.strict_casts,
                    strict_inference: o.strict_inference,
                    strict_raw_types: o.strict_raw_types,
                },
            )
        };
        ContextOptions {
            folders: map
                .entries()
                .iter()
                .map(|(folder, o)| {
                    let (s, r) = settings(o);
                    (folder.clone(), s, r)
                })
                .collect(),
            default: settings(map.default_options()),
        }
    }

    fn for_path(&self, path: &str) -> (&Arc<FileSettings>, ResolverOptions) {
        for (folder, settings, options) in &self.folders {
            if paths::is_within(folder, path) {
                return (settings, *options);
            }
        }
        (&self.default.0, self.default.1)
    }
}

/// One driver per analysis context, kept between requests.
#[derive(Default)]
pub struct DriverSession {
    generation: Option<Arc<Generation>>,
    /// By context index of the collection.
    drivers: IndexMap<usize, Driver>,
}

/// What the analysis of one context gives for one requested file.
enum Outcome {
    Done(FileDiagnostics),
    /// Use the parse-only pipeline.
    Fallback(AnalyzedFile),
}

impl DriverSession {
    /// The diagnostics of [files], in any order. Files that cannot be read
    /// give no result.
    pub fn diagnostics(
        &mut self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        let generation = self
            .generation
            .get_or_insert_with(|| Arc::new(Generation::new(0)))
            .clone();
        let mut by_context: IndexMap<usize, Vec<AnalyzedFile>> = IndexMap::new();
        for f in files {
            by_context.entry(f.context).or_default().push(f.clone());
        }
        let mut result = Vec::new();
        let mut fallback = Vec::new();
        for (context, files) in by_context {
            let options = ContextOptions::new(collection, context);
            let mut driver = self.drivers.shift_remove(&context).unwrap_or_else(|| {
                dartr_driver::project::context_driver(collection, context, generation.clone())
            });
            let outcome = pool().install(|| {
                catch_unwind(AssertUnwindSafe(|| {
                    analyze_context(&mut driver, &options, &files)
                }))
            });
            match outcome {
                Ok(outcomes) => {
                    for o in outcomes {
                        match o {
                            Outcome::Done(d) => result.push(d),
                            Outcome::Fallback(f) => fallback.push(f),
                        }
                    }
                    self.drivers.insert(context, driver);
                }
                Err(e) => {
                    // The driver state is not consistent after a panic:
                    // drop it.
                    if debug() {
                        eprintln!(
                            "dartr: driver panic in context {context}: {}",
                            panic_message(&*e)
                        );
                    }
                    fallback.extend(files);
                }
            }
        }
        if !fallback.is_empty() {
            if debug() {
                eprintln!(
                    "dartr: {} files use the parse-only pipeline",
                    fallback.len()
                );
            }
            result.extend(diagnostics_with_reader(
                collection,
                &fallback,
                &dartr_project::fs::read_string,
            ));
        }
        result
    }

    /// Dart `AnalysisDriver.changeFile` for every driver: reads [path] again
    /// and invalidates what depends on it (design §4.2). Returns the paths
    /// of the units of the libraries to analyze again.
    pub fn change_file(&mut self, path: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for driver in self.drivers.values_mut() {
            if driver.fs.get_existing_from_path(path).is_none() {
                continue;
            }
            let change = driver.change_file(path);
            for library in change.libraries {
                for unit in driver.fs.library_file_kinds(library) {
                    let p = driver.fs.file(unit).path.to_string();
                    if !result.contains(&p) {
                        result.push(p);
                    }
                }
            }
        }
        result
    }
}

/// Analyzes the requested [files] of one context with its [driver].
fn analyze_context(
    driver: &mut Driver,
    options: &ContextOptions,
    files: &[AnalyzedFile],
) -> Vec<Outcome> {
    let ids: Vec<FileId> = files
        .iter()
        .map(|f| driver.fs.get_file_for_path(&f.path))
        .collect();
    driver.fs.discover();

    let mut outcomes = Vec::new();
    // The requested files of each library (defining unit).
    let mut libraries: IndexMap<FileId, Vec<FileId>> = IndexMap::new();
    let mut requested: HashMap<FileId, &AnalyzedFile> = HashMap::new();
    for (file, &id) in files.iter().zip(&ids) {
        if !driver.fs.file(id).exists() {
            // Like the parse-only provider: no result for a file that
            // cannot be read.
            continue;
        }
        match driver.fs.library_of(id) {
            Some(library) => {
                libraries.entry(library).or_default().push(id);
                requested.insert(id, file);
            }
            None => outcomes.push(Outcome::Fallback(file.clone())),
        }
    }
    let library_ids: Vec<FileId> = libraries.keys().copied().collect();
    driver.link_libraries(&library_ids);

    let jobs: Vec<(FileId, ResolverOptions)> = library_ids
        .iter()
        .map(|&l| (l, options.for_path(&driver.fs.file(l).path).1))
        .collect();
    let requested = &requested;
    let libraries = &libraries;
    let analyzed = driver.analyze_libraries(&jobs, |library, units, result| {
        library_outcomes(options, &libraries[&library], requested, units, result)
    });
    outcomes.extend(analyzed.into_iter().flatten());
    outcomes
}

/// The diagnostics of the [wanted] units of one analyzed library.
fn library_outcomes(
    options: &ContextOptions,
    wanted: &[FileId],
    requested: &HashMap<FileId, &AnalyzedFile>,
    units: &[UnitInput],
    result: Result<ResolvedLibrary, String>,
) -> Vec<Outcome> {
    let fallback = || {
        wanted
            .iter()
            .map(|id| Outcome::Fallback(requested[id].clone()))
            .collect()
    };
    let Some(first) = units.first() else {
        return fallback();
    };
    let mut resolved = match result {
        Ok(resolved) => resolved,
        Err(message) => {
            if debug() {
                eprintln!("dartr: analysis panic in {}: {message}", first.path);
            }
            return fallback();
        }
    };
    let (library_settings, _) = options.for_path(&first.path);
    let lints: Vec<Vec<Diagnostic>> = if library_settings.lint_rules.is_empty() {
        vec![Vec::new(); units.len()]
    } else {
        let context_units: Vec<RuleContextUnit<'_>> = units
            .iter()
            .map(|u| RuleContextUnit {
                parsed: &u.parsed,
                source: &u.parsed.ast.tokens.source,
                path: &u.path,
            })
            .collect();
        lint_library_unfiltered(&context_units, &library_settings.lint_rules)
    };
    let mut outcomes = Vec::new();
    for &id in wanted {
        let file = requested[&id];
        let Some(index) = units.iter().position(|u| *u.path == *file.path) else {
            // Not a unit of its library (for example a part that the
            // library includes twice).
            outcomes.push(Outcome::Fallback(file.clone()));
            continue;
        };
        let unit = &mut resolved.units[index];
        if let Some(panic) = &unit.panic
            && debug()
        {
            eprintln!("dartr: resolver panic in {}: {panic}", unit.path);
        }
        let parsed = &units[index].parsed;
        let (settings, _) = options.for_path(&file.path);
        outcomes.push(Outcome::Done(finish_file(
            &file.path,
            &parsed.ast.tokens.source,
            parsed,
            std::mem::take(&mut unit.diagnostics),
            lints.get(index).cloned().unwrap_or_default(),
            settings,
        )));
    }
    outcomes
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}
