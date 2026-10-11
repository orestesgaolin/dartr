// Dart source: pkg/analyzer/lib/src/dart/analysis/driver.dart (the analysis
// of a library: `_analyzeFileImpl` → `LibraryAnalyzer.analyze`),
// pkg/analyzer/lib/src/dart/analysis/library_context.dart (the linked
// element model the analysis reads)

//! [`Driver::analyze_library`]: resolves the units of a linked library with
//! `dartr_resolver::library_analyzer` (design §2.5 step 4).

use std::sync::Arc;

use dartr_lints::LintDiagnostic as Diagnostic;
use indexmap::IndexMap;

use dartr_ast_builder::ParsedUnit;
use dartr_element::{
    Ctx, DirectiveUri, EId, FId, FeatureSet, LibraryElement, LibraryFragment, NoopSink,
};
use dartr_resolver::library_analyzer::{
    ExternalUnitCache, LibraryAnalysisInput, ResolvedLibrary, UnitInput, analyze_library,
    analyze_library_with_unignorable,
};
use dartr_resolver::options::AnalysisOptions;

use crate::driver::Driver;
use crate::file_state::FileId;

impl Driver {
    /// Analyzes the library of [file] (the defining unit). The library must
    /// be linked ([`Driver::link_libraries`]); returns `None` otherwise.
    pub fn analyze_library(
        &self,
        file: FileId,
        options: AnalysisOptions,
    ) -> Option<ResolvedLibrary> {
        self.analyze_library_with_lints(file, options, &[])
    }

    /// Resolves first, then runs enabled lint rules on the resolved units.
    pub fn analyze_library_with_lints(
        &self,
        file: FileId,
        options: AnalysisOptions,
        enabled: &[&str],
    ) -> Option<ResolvedLibrary> {
        let world = &self.state.world;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let (library, units) = self.library_units_with_tp(file, &tp)?;
        let external = ExternalUnitCache::new(world, &tp, options, self.unit_sources());
        let input = LibraryAnalysisInput {
            world,
            type_provider: &tp,
            library,
            units,
            options,
            external: Some(&external),
            doc_import_libraries: self.doc_import_libraries(file),
        };
        let pkg_info = (!enabled.is_empty())
            .then(|| dartr_lints::LintPackageInfo::for_defining_path(&self.fs.file(file).path));
        let mut library = analyze_library(&input);
        let diagnostics =
            crate::lints::compute_lints_with_package(&input, &library, enabled, pkg_info.as_ref());
        for (unit, lints) in library.units.iter_mut().zip(diagnostics) {
            unit.diagnostics.extend(lints);
        }
        Some(library)
    }

    /// Analyzes [libraries] (defining units with their options and the
    /// unignorable code names of `analyzer: cannot-ignore`, which the ignore
    /// filtering of the library analyzer keeps) in parallel on the current
    /// rayon pool (design §2.5 step 4) and maps each result with [f], in the
    /// order of [libraries]. The libraries must be linked; a library that is
    /// not linked, or whose analysis panics, gives `Err` with a message. [f]
    /// gets the unit inputs of the library (the parsed units, for steps that
    /// need the unresolved AST) and can drop the resolved library, so that
    /// not all results are in memory at once.
    pub fn analyze_libraries<T, F>(
        &self,
        libraries: &[(FileId, AnalysisOptions, &[String])],
        f: F,
    ) -> Vec<T>
    where
        T: Send,
        F: Fn(FileId, &[UnitInput], Result<ResolvedLibrary, String>) -> T + Sync,
    {
        let jobs: Vec<LintJob<'_>> = libraries
            .iter()
            .map(|&(file, options, unignorable)| (file, options, unignorable, Vec::new()))
            .collect();
        self.analyze_libraries_with_lints(&jobs, |file, units, result, _| f(file, units, result))
    }

    /// [`Driver::analyze_libraries`] that also runs the enabled lint rules
    /// of each library on its resolved units (Dart `_computeLints`, after
    /// resolution). [f] gets the lint diagnostics of each unit, in the order
    /// of the units, not filtered by the `ignore` comments: the caller
    /// validates the ignore comments with all diagnostics of the file (Dart
    /// `IgnoreValidator`) and then filters them.
    pub fn analyze_libraries_with_lints<T, F>(&self, libraries: &[LintJob<'_>], f: F) -> Vec<T>
    where
        T: Send,
        F: Fn(FileId, &[UnitInput], Result<ResolvedLibrary, String>, Vec<Vec<Diagnostic>>) -> T
            + Sync,
    {
        use rayon::prelude::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let world = &self.state.world;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let external =
            ExternalUnitCache::new(world, &tp, AnalysisOptions::default(), self.unit_sources());
        let mut pkg_cache = dartr_lints::LintPackageCache::default();
        let jobs: Vec<_> = libraries
            .iter()
            .map(|job| {
                let pkg_info = (!job.3.is_empty())
                    .then(|| pkg_cache.for_defining_path(&self.fs.file(job.0).path));
                (
                    job,
                    self.library_units_with_tp(job.0, &tp),
                    self.doc_import_libraries(job.0),
                    pkg_info,
                )
            })
            .collect();
        let panic_message = |e: Box<dyn std::any::Any + Send>| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string())
        };
        let mem_stats = std::env::var_os("DARTR_MEM_STATS").is_some();
        use std::sync::atomic::{AtomicUsize, Ordering};
        let stat_units = AtomicUsize::new(0);
        let stat_ast_used = AtomicUsize::new(0);
        let stat_ast_cap = AtomicUsize::new(0);
        let stat_res_entries = AtomicUsize::new(0);
        let stat_res_used = AtomicUsize::new(0);
        let stat_res_cap = AtomicUsize::new(0);
        let stat_rt_entries = AtomicUsize::new(0);
        let stat_rt_used = AtomicUsize::new(0);
        let stat_rt_cap = AtomicUsize::new(0);
        let stat_local_used = AtomicUsize::new(0);
        let stat_local_cap = AtomicUsize::new(0);
        let stat_peak_lib = AtomicUsize::new(0);

        let out: Vec<T> = jobs
            .par_iter()
            .map(
                |((file, options, unignorable, rules), job, doc_imports, pkg_info)| {
                    let Some((library, units)) = job else {
                        return f(
                            *file,
                            &[],
                            Err("library is not linked".to_string()),
                            Vec::new(),
                        );
                    };
                    let input = LibraryAnalysisInput {
                        world,
                        type_provider: &tp,
                        library: *library,
                        units: units.clone(),
                        options: *options,
                        external: Some(&external),
                        doc_import_libraries: doc_imports.clone(),
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        analyze_library_with_unignorable(&input, unignorable)
                    }))
                    .map_err(panic_message);
                    if mem_stats && let Ok(resolved) = &result {
                        let mut lib_cap = 0usize;
                        for u in &resolved.units {
                            let mut ast_m = u.ast.mem_stats();
                            if u.ast.tokens.is_shared() {
                                ast_m.tokens_used = 0;
                                ast_m.tokens_cap = 0;
                            }
                            let (re, ru, rc) = u.tables.mem_bytes();
                            let (rte, rtu, rtc) = u.rt.mem_bytes();
                            let (_, eu, ec) = u.local.store.elements.mem_bytes();
                            let (_, fu, fc) = u.local.store.fragments.mem_bytes();
                            let (_, ou, oc) = u.local.types.mem_bytes();
                            let lu = eu + fu + ou;
                            let lc =
                                std::mem::size_of::<dartr_element::LocalArena>() + ec + fc + oc;
                            stat_units.fetch_add(1, Ordering::Relaxed);
                            stat_ast_used.fetch_add(ast_m.total_used(), Ordering::Relaxed);
                            stat_ast_cap.fetch_add(ast_m.total_cap(), Ordering::Relaxed);
                            stat_res_entries.fetch_add(re, Ordering::Relaxed);
                            stat_res_used.fetch_add(ru, Ordering::Relaxed);
                            stat_res_cap.fetch_add(rc, Ordering::Relaxed);
                            stat_rt_entries.fetch_add(rte, Ordering::Relaxed);
                            stat_rt_used.fetch_add(rtu, Ordering::Relaxed);
                            stat_rt_cap.fetch_add(rtc, Ordering::Relaxed);
                            stat_local_used.fetch_add(lu, Ordering::Relaxed);
                            stat_local_cap.fetch_add(lc, Ordering::Relaxed);
                            lib_cap += ast_m.total_cap() + rc + rtc + lc;
                        }
                        stat_peak_lib.fetch_max(lib_cap, Ordering::Relaxed);
                    }
                    let lints = match &result {
                        Ok(resolved) if !rules.is_empty() => {
                            let enabled: Vec<&str> = rules.iter().map(String::as_str).collect();
                            catch_unwind(AssertUnwindSafe(|| {
                                crate::lints::compute_lints_unfiltered_with_package(
                                    &input,
                                    resolved,
                                    &enabled,
                                    pkg_info.as_deref(),
                                )
                            }))
                            .unwrap_or_default()
                        }
                        _ => Vec::new(),
                    };
                    f(*file, units, result, lints)
                },
            )
            .collect();

        if mem_stats && !libraries.is_empty() {
            let ext_units = external.resolved_units();
            let mut ext_ast_used = 0usize;
            let mut ext_ast_cap = 0usize;
            let mut ext_res_entries = 0usize;
            let mut ext_res_used = 0usize;
            let mut ext_res_cap = 0usize;
            let mut ext_rt_entries = 0usize;
            let mut ext_rt_used = 0usize;
            let mut ext_rt_cap = 0usize;
            let mut ext_local_used = 0usize;
            let mut ext_local_cap = 0usize;
            for u in &ext_units {
                let mut ast_m = u.ast.mem_stats();
                if u.ast.tokens.is_shared() {
                    ast_m.tokens_used = 0;
                    ast_m.tokens_cap = 0;
                }
                let (re, ru, rc) = u.tables.mem_bytes();
                let (rte, rtu, rtc) = u.rt.mem_bytes();
                let (_, eu, ec) = u.local.store.elements.mem_bytes();
                let (_, fu, fc) = u.local.store.fragments.mem_bytes();
                let (_, ou, oc) = u.local.types.mem_bytes();
                ext_ast_used += ast_m.total_used();
                ext_ast_cap += ast_m.total_cap();
                ext_res_entries += re;
                ext_res_used += ru;
                ext_res_cap += rc;
                ext_rt_entries += rte;
                ext_rt_used += rtu;
                ext_rt_cap += rtc;
                ext_local_used += eu + fu + ou;
                ext_local_cap += std::mem::size_of::<dartr_element::LocalArena>() + ec + fc + oc;
            }
            let mb = |b: usize| b as f64 / (1024.0 * 1024.0);
            let msg = format!(
                "[mem:analyze_pass] analyzed_libs={} analyzed_units={} peak_single_lib={:.2} MiB | transient_cumulative(ast={:.2}/{:.2} MiB, res_tables={} entries {:.2}/{:.2} MiB, rt_tables={} entries {:.2}/{:.2} MiB, local_arena={:.2}/{:.2} MiB) | external_cache_retained(units={}, total={:.2} MiB: ast={:.2}/{:.2} MiB, res_tables={} entries {:.2}/{:.2} MiB, rt_tables={} entries {:.2}/{:.2} MiB, local_arena={:.2}/{:.2} MiB)\n",
                libraries.len(),
                stat_units.load(Ordering::Relaxed),
                mb(stat_peak_lib.load(Ordering::Relaxed)),
                mb(stat_ast_used.load(Ordering::Relaxed)),
                mb(stat_ast_cap.load(Ordering::Relaxed)),
                stat_res_entries.load(Ordering::Relaxed),
                mb(stat_res_used.load(Ordering::Relaxed)),
                mb(stat_res_cap.load(Ordering::Relaxed)),
                stat_rt_entries.load(Ordering::Relaxed),
                mb(stat_rt_used.load(Ordering::Relaxed)),
                mb(stat_rt_cap.load(Ordering::Relaxed)),
                mb(stat_local_used.load(Ordering::Relaxed)),
                mb(stat_local_cap.load(Ordering::Relaxed)),
                ext_units.len(),
                mb(ext_ast_cap + ext_res_cap + ext_rt_cap + ext_local_cap),
                mb(ext_ast_used),
                mb(ext_ast_cap),
                ext_res_entries,
                mb(ext_res_used),
                mb(ext_res_cap),
                ext_rt_entries,
                mb(ext_rt_used),
                mb(ext_rt_cap),
                mb(ext_local_used),
                mb(ext_local_cap),
            );
            if let Ok(mut err) = std::fs::OpenOptions::new().write(true).open("/dev/stderr") {
                let _ = std::io::Write::write_all(&mut err, msg.as_bytes());
            }
        }
        out
    }

    /// Dart `library.docLibraryImports` with a library file
    /// (`LibraryImportWithFile.importedLibrary`): the files that
    /// [`Driver::link_libraries`] links with [library].
    pub fn doc_import_files(&self, library: FileId) -> Vec<FileId> {
        self.fs
            .file(library)
            .c()
            .doc_library_imports
            .iter()
            .filter_map(|import| match import.uris.selected {
                crate::file_state::DirectiveUri::WithFile { file, .. } => Some(file),
                _ => None,
            })
            .filter(|&file| {
                self.fs.file(file).content.is_some() && self.fs.file(file).kind().is_library()
            })
            .collect()
    }

    /// The linked library elements of [`Driver::doc_import_files`] (Dart
    /// `elementFactory.libraryOfUri2(import.importedFile.uri)`).
    pub fn doc_import_libraries(&self, library: FileId) -> Vec<EId<LibraryElement>> {
        self.doc_import_files(library)
            .into_iter()
            .filter_map(|file| {
                self.state
                    .world
                    .libraries
                    .get(&self.fs.file(file).uri_str)
                    .copied()
            })
            .collect()
    }

    /// The parsed units of all discovered files by path, with their URIs
    /// (the sources of an [`ExternalUnitCache`]).
    pub fn unit_sources(&self) -> IndexMap<Arc<str>, (Arc<str>, Arc<ParsedUnit>)> {
        self.fs
            .files()
            .iter()
            .filter_map(|f| {
                let content = f.content.as_ref()?;
                Some((f.path.clone(), (f.uri_str.clone(), content.parsed.clone())))
            })
            .collect()
    }

    /// The library element of [file] (the defining unit) and the inputs of
    /// its units, for [`analyze_library`]. Returns `None` when the library is
    /// not linked. Use it to analyze many libraries in parallel: the driver
    /// is not `Sync`, but the world snapshot and the unit inputs are.
    pub fn library_units(&self, file: FileId) -> Option<LibraryUnits> {
        let tp = dartr_link::types_builder::world_type_provider(&self.state.world);
        self.library_units_with_tp(file, &tp)
    }

    fn library_units_with_tp(
        &self,
        file: FileId,
        tp: &dartr_element::TypeProvider,
    ) -> Option<LibraryUnits> {
        let world = &self.state.world;
        let uri = &self.fs.file(file).uri_str;
        let library = *world.libraries.get(uri)?;
        let features = FeatureSet::default();
        let sink = NoopSink;
        let ctx = Ctx {
            world,
            current: None,
            local: None,
            tp,
            features: &features,
            req: &sink,
        };
        let (mut directive_diagnostics, mut resolved_uris) =
            crate::directives::resolve_library_directives(&self.fs, &ctx, library, file);
        let mut units = Vec::new();
        for fragment in library_fragments(&ctx, library) {
            let source = &ctx.fragment(fragment).source;
            let Some(unit_file) = self.fs.get_existing_from_path(&source.path) else {
                continue;
            };
            let f = self.fs.file(unit_file);
            units.push(UnitInput {
                path: f.path.clone(),
                uri: f.uri_str.clone(),
                parsed: f.c().parsed.clone(),
                fragment,
                directive_diagnostics: directive_diagnostics
                    .shift_remove(&unit_file)
                    .unwrap_or_default(),
                resolved_uris: resolved_uris.shift_remove(&unit_file).unwrap_or_default(),
            });
        }
        Some((library, units))
    }
}

/// A library to analyze: the defining unit, its options, the unignorable
/// code names and the enabled lint rules.
pub type LintJob<'u> = (FileId, AnalysisOptions, &'u [String], Vec<String>);

/// The library element and the unit inputs of a linked library.
pub type LibraryUnits = (EId<LibraryElement>, Vec<UnitInput>);

/// The fragments of [library]: the defining unit, then the parts, depth
/// first in `part` directive order.
pub fn library_fragments(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> Vec<FId<LibraryFragment>> {
    fn visit_parts(
        ctx: &Ctx<'_>,
        unit: FId<LibraryFragment>,
        result: &mut Vec<FId<LibraryFragment>>,
    ) {
        for part in &ctx.fragment(unit).parts {
            if let DirectiveUri::Unit {
                library_fragment, ..
            } = &part.directive.uri
            {
                result.push(*library_fragment);
                visit_parts(ctx, *library_fragment, result);
            }
        }
    }
    let first = ctx.get(library).first_fragment();
    let mut result = vec![first];
    visit_parts(ctx, first, &mut result);
    result
}

/// Keeps `Arc` in scope for the public types.
#[allow(dead_code)]
fn _arc(_: Arc<str>) {}
