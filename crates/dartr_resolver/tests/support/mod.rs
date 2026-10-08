//! Test support: writes Dart files into a temporary directory, links them
//! with `dartr_driver` (the SDK from `dart` on `PATH`), runs the library
//! analyzer, and finds nodes and results.

#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use dartr_ast::{Ast, NodeId, NodeKind};
use dartr_diagnostics::Diagnostic;
use dartr_driver::driver::Driver;
use dartr_driver::file_state::{FileConfig, FileSystemState, SourceFactory};
use dartr_element::{
    Ctx, DisplayOptions, ElemRef, ElementId, FeatureSet, Generation, NoopSink, TypeId, TypeProvider,
};
use dartr_project::{DartSdk, Packages, Workspace};
use dartr_resolver::library_analyzer::{ResolvedLibrary, ResolvedUnit};
use dartr_resolver::options::AnalysisOptions;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A resolved library and what is needed to read its results.
pub struct Analyzed {
    pub driver: Driver,
    pub library: ResolvedLibrary,
    pub tp: Arc<TypeProvider>,
    pub features: FeatureSet,
    pub sources: Vec<(String, String)>,
}

/// Analyzes the library `main.dart` of [files] (name, content). Returns
/// `None` when no SDK is found.
pub fn analyze(files: &[(&str, &str)]) -> Option<Analyzed> {
    let sdk_path = dartr_project::sdk::find_sdk_path()?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = format!(
        "{}/resolver_{}_{n}",
        env!("CARGO_TARGET_TMPDIR"),
        std::process::id()
    );
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut sources = Vec::new();
    for (name, content) in files {
        let path = format!("{dir}/{name}");
        std::fs::write(&path, content).expect("write");
        sources.push((path, content.to_string()));
    }

    let sdk = DartSdk::new(&sdk_path);
    let version = sdk
        .language_version()
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::empty(), &dir),
        sdk: Some(sdk),
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    let generation = Arc::new(Generation::new(0));
    let mut driver = Driver::new(FileSystemState::new(source_factory, config_for), generation);
    let main = driver.fs.get_file_for_path(&format!("{dir}/main.dart"));
    driver.fs.discover();
    driver.link_libraries(&[main]);
    let library = driver
        .analyze_library(main, AnalysisOptions::default())
        .expect("linked library");
    let tp = Arc::new(dartr_link::types_builder::world_type_provider(&driver.state.world));
    Some(Analyzed {
        driver,
        library,
        tp,
        features: FeatureSet::default(),
        sources,
    })
}

static SINK: NoopSink = NoopSink;

impl Analyzed {
    /// The defining unit.
    pub fn unit(&self) -> &ResolvedUnit {
        &self.library.units[0]
    }

    /// The source of the defining unit.
    pub fn source(&self) -> &str {
        &self.sources[0].1
    }

    /// A context that sees the local elements of [unit].
    pub fn ctx<'a>(&'a self, unit: &'a ResolvedUnit) -> Ctx<'a> {
        Ctx {
            world: &self.driver.state.world,
            current: None,
            local: Some(&unit.local),
            tp: &self.tp,
            features: &self.features,
            req: &SINK,
        }
    }

    /// The display string of [t].
    pub fn type_str(&self, t: TypeId) -> String {
        let unit = self.unit();
        dartr_element::type_display_string_with(&self.ctx(unit), t, DisplayOptions::default())
    }

    /// The display string of [e].
    pub fn element_str(&self, e: ElementId) -> String {
        let unit = self.unit();
        dartr_element::element_display_string_with(&self.ctx(unit), e, DisplayOptions::default())
    }

    /// The name of [e].
    pub fn element_name(&self, e: ElementId) -> Option<String> {
        let unit = self.unit();
        let ctx = self.ctx(unit);
        let name = ctx.element_data(e)?.name?;
        Some(ctx.name_str(name).to_string())
    }

    /// The node of [kind] in the tree of the defining unit that starts at
    /// the [n]th occurrence of [search] plus [delta] characters.
    pub fn node_at(&self, kind: NodeKind, search: &str, n: usize, delta: usize) -> NodeId {
        let source = self.source();
        let offset = source
            .match_indices(search)
            .nth(n)
            .unwrap_or_else(|| panic!("{search:?} #{n} not found"))
            .0
            + delta;
        find_node(&self.unit().ast, self.unit().unit.raw(), kind, offset as u32)
            .unwrap_or_else(|| panic!("no {kind:?} at {offset} ({search:?})"))
    }

    /// The element of [node] (`ResolutionTables.element`), as a base
    /// element.
    pub fn element_of(&self, node: NodeId) -> Option<ElementId> {
        match self.unit().tables.element.get(node)? {
            ElemRef::Base(e) => Some(*e),
            ElemRef::Member(m) => {
                let unit = self.unit();
                Some(self.ctx(unit).member(*m).base)
            }
        }
    }

    /// The element of the declared fragment of [node].
    pub fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        let unit = self.unit();
        let ctx = self.ctx(unit);
        let fragment = *unit.tables.declared_fragment.get(node)?;
        ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    /// The display string of the annotation type of [node].
    pub fn annotation_type_str(&self, node: NodeId) -> Option<String> {
        let t = *self.unit().tables.annotation_type.get(node)?;
        Some(self.type_str(t))
    }

    /// The diagnostics of the defining unit, as `name@offset` strings.
    pub fn diagnostic_names(&self) -> Vec<String> {
        diagnostic_names(&self.unit().diagnostics)
    }
}

pub fn diagnostic_names(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|d| format!("{}@{}", d.code.name, d.offset))
        .collect()
}

/// The node of [kind] that starts at [offset] and is in the tree under
/// [root]. A rewritten node keeps its parent link but its parent no longer
/// has it as a child, so it is not found.
pub fn find_node(ast: &Ast, root: NodeId, kind: NodeKind, offset: u32) -> Option<NodeId> {
    (0..ast.node_count())
        .map(NodeId::from_index)
        .find(|&n| ast.kind(n) == kind && ast.offset(n) == offset && is_attached(ast, root, n))
}

/// Whether [node] is reachable from [root] through child links.
pub fn is_attached(ast: &Ast, root: NodeId, node: NodeId) -> bool {
    let mut current = node;
    while current != root {
        let Some(parent) = ast.parent(current) else {
            return false;
        };
        if !ast.children(parent).contains(&current) {
            return false;
        }
        current = parent;
    }
    true
}
