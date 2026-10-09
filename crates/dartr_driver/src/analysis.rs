// Dart source: pkg/analyzer/lib/src/dart/analysis/driver.dart (the analysis
// of a library: `_analyzeFileImpl` → `LibraryAnalyzer.analyze`),
// pkg/analyzer/lib/src/dart/analysis/library_context.dart (the linked
// element model the analysis reads)

//! [`Driver::analyze_library`]: resolves the units of a linked library with
//! `dartr_resolver::library_analyzer` (design §2.5 step 4).

use std::sync::Arc;

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
        let world = &self.state.world;
        let (library, units) = self.library_units(file)?;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let external = ExternalUnitCache::new(world, &tp, options, self.unit_sources());
        let input = LibraryAnalysisInput {
            world,
            type_provider: &tp,
            library,
            units,
            options,
            external: Some(&external),
        };
        Some(analyze_library(&input))
    }

    /// Analyzes [libraries] (defining units with their options and the
    /// unignorable code names of `analyzer: cannot-ignore`, which the ignore
    /// filtering of the library analyzer keeps) in parallel on the current rayon pool (design §2.5 step 4) and maps each
    /// result with [f], in the order of [libraries]. The libraries must be
    /// linked; a library that is not linked, or whose analysis panics, gives
    /// `Err` with a message. [f] gets the unit inputs of the library (the
    /// parsed units, for steps that need the unresolved AST) and can drop
    /// the resolved library, so that not all results are in memory at once.
    pub fn analyze_libraries<T, F>(
        &self,
        libraries: &[(FileId, AnalysisOptions, &[String])],
        f: F,
    ) -> Vec<T>
    where
        T: Send,
        F: Fn(FileId, &[UnitInput], Result<ResolvedLibrary, String>) -> T + Sync,
    {
        use rayon::prelude::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let world = &self.state.world;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let external =
            ExternalUnitCache::new(world, &tp, AnalysisOptions::default(), self.unit_sources());
        type Job<'u> = (
            FileId,
            AnalysisOptions,
            &'u [String],
            Option<(EId<LibraryElement>, Vec<UnitInput>)>,
        );
        let jobs: Vec<Job<'_>> = libraries
            .iter()
            .map(|&(file, options, unignorable)| {
                (file, options, unignorable, self.library_units(file))
            })
            .collect();
        jobs.par_iter()
            .map(|(file, options, unignorable, job)| {
                let Some((library, units)) = job else {
                    return f(*file, &[], Err("library is not linked".to_string()));
                };
                let input = LibraryAnalysisInput {
                    world,
                    type_provider: &tp,
                    library: *library,
                    units: units.clone(),
                    options: *options,
                    external: Some(&external),
                };
                let result =
                    catch_unwind(AssertUnwindSafe(|| {
                        analyze_library_with_unignorable(&input, unignorable)
                    }))
                    .map_err(|e| {
                        e.downcast_ref::<String>()
                            .cloned()
                            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "panic".to_string())
                    });
                f(*file, units, result)
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
    pub fn library_units(&self, file: FileId) -> Option<(EId<LibraryElement>, Vec<UnitInput>)> {
        let world = &self.state.world;
        let uri = &self.fs.file(file).uri_str;
        let library = *world.libraries.get(uri)?;
        let tp = dartr_link::types_builder::world_type_provider(world);
        let features = FeatureSet::default();
        let sink = NoopSink;
        let ctx = Ctx {
            world,
            current: None,
            local: None,
            tp: &tp,
            features: &features,
            req: &sink,
        };
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
            });
        }
        Some((library, units))
    }
}

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
