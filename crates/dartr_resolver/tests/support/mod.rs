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

pub mod g3;

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
    let tp = Arc::new(dartr_link::types_builder::world_type_provider(
        &driver.state.world,
    ));
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
        find_node(
            &self.unit().ast,
            self.unit().unit.raw(),
            kind,
            offset as u32,
        )
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

/// The files of a mock package of the analyzer tests (Dart
/// `MockPackages.add<Name>PackageFiles` in
/// `pkg/analyzer/lib/src/test_utilities/mock_packages.dart`): (path
/// relative to `lib`, content). `function` is e.g. `addMetaPackageFiles`.
/// Returns `None` when the SDK sources are not available.
pub fn mock_package_files(function: &str) -> Option<Vec<(String, String)>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/dart-sdk/pkg/analyzer/lib/src/test_utilities/mock_packages.dart"
    );
    let text = std::fs::read_to_string(path).ok()?;
    let start = text.find(&format!("static void {function}("))?;
    let body = &text[start..];
    let end = body[1..]
        .find("static void ")
        .map(|e| e + 1)
        .unwrap_or(body.len());
    let mut body = &body[..end];
    let mut files = Vec::new();
    while let Some(i) = body.find("getFile('") {
        let rest = &body[i + "getFile('".len()..];
        let name_end = rest.find('\'')?;
        let name = rest[..name_end].to_string();
        let open = rest.find("r'''")? + 4;
        let content_rest = &rest[open..];
        let close = content_rest.find("'''")?;
        files.push((name, content_rest[..close].to_string()));
        body = &content_rest[close + 3..];
    }
    Some(files)
}

/// Like [analyze], but in a pub package `test` (`pubspec.yaml`, files in
/// `test/lib`, mapped to `package:test/`) with the other [packages]
/// (name, files relative to their `lib` folder), mapped to
/// `package:<name>/` (Dart `PubPackageResolutionTest` with
/// `writeTestPackageConfig`).
pub fn analyze_in_packages(
    files: &[(&str, &str)],
    packages: &[(&str, Vec<(String, String)>)],
    options: AnalysisOptions,
) -> Option<Analyzed> {
    let sdk_path = dartr_project::sdk::find_sdk_path()?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = format!(
        "{}/resolver_pkg_{}_{n}",
        env!("CARGO_TARGET_TMPDIR"),
        std::process::id()
    );
    let mut package_list = Vec::new();
    let mut write_package = |name: &str, files: &[(String, String)]| -> Vec<(String, String)> {
        let root = format!("{dir}/{name}");
        let lib = format!("{root}/lib");
        std::fs::create_dir_all(&lib).expect("temp dir");
        std::fs::write(format!("{root}/pubspec.yaml"), format!("name: {name}\n")).expect("write");
        let mut written = Vec::new();
        for (path, content) in files {
            let full = format!("{lib}/{path}");
            if let Some(parent) = std::path::Path::new(&full).parent() {
                std::fs::create_dir_all(parent).expect("dir");
            }
            std::fs::write(&full, content).expect("write");
            written.push((full, content.clone()));
        }
        package_list.push(dartr_project::package_config::Package {
            name: name.to_string(),
            root,
            lib,
            language_version: None,
        });
        written
    };
    let test_files: Vec<(String, String)> = files
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect();
    let sources = write_package("test", &test_files);
    for (name, files) in packages {
        write_package(name, files);
    }

    let sdk = DartSdk::new(&sdk_path);
    let version = sdk
        .language_version()
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::new(package_list), &dir),
        sdk: Some(sdk),
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    let generation = Arc::new(Generation::new(0));
    let mut driver = Driver::new(FileSystemState::new(source_factory, config_for), generation);
    let main = driver.fs.get_file_for_path(&sources[0].0);
    driver.fs.discover();
    driver.link_libraries(&[main]);
    let library = driver
        .analyze_library(main, options)
        .expect("linked library");
    let tp = Arc::new(dartr_link::types_builder::world_type_provider(
        &driver.state.world,
    ));
    Some(Analyzed {
        driver,
        library,
        tp,
        features: FeatureSet::default(),
        sources,
    })
}
