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
