// Dart source: tools/oracle/bin/elements.dart (dumpElements)

//! `dartr dump elements`: links the libraries of the input paths (and
//! `dart:` URIs) with `dartr_driver` and writes one line per input with
//! `dartr_link::dump::library_json`.
//!
//! Like the oracle, the input paths are grouped by analysis context
//! (`AnalysisContextCollection(includedPaths: paths)`), and `dart:` URIs
//! are linked in a context that has only the SDK.

use std::rc::Rc;
use std::sync::Arc;

use dartr_driver::driver::Driver;
use dartr_driver::file_state::{FileConfig, FileId, FileSystemState, SourceFactory};
use dartr_driver::uri::Uri;
use dartr_element::{ConstExprId, Ctx, ElementId, FeatureSet, Generation, NoopSink, StoreId};
use dartr_link::dump::{DumpSources, error_json, library_json};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_project::{
    AnalysisContextCollection, CollectionOptions, DartSdk, Packages, Workspace, paths,
};
use indexmap::IndexMap;

/// The result of looking up one input.
pub(crate) enum Input {
    Error(&'static str),
    Library { driver: usize, file: FileId },
}

/// The drivers of the analysis contexts of the inputs, with the libraries
/// of the inputs linked.
pub(crate) struct LinkedInputs {
    pub collection: Rc<AnalysisContextCollection>,
    pub drivers: Vec<Driver>,
    /// The context index of each driver; `None` for the driver of the
    /// `dart:` inputs.
    pub driver_context: Vec<Option<usize>>,
    /// One entry per input, in input order.
    pub inputs: Vec<Input>,
}

/// Groups [inputs] by analysis context (like the oracle), creates one
/// driver per context (and one for the `dart:` URIs), reads the files and
/// links the libraries of the inputs. A part file gives
/// `Input::Error("NotLibraryButPartResult")`.
pub(crate) fn link_inputs(inputs: &[String]) -> LinkedInputs {
    let generation = Arc::new(Generation::new(0));
    let sdk_path = dartr_project::sdk::find_sdk_path();

    let paths: Vec<String> = inputs
        .iter()
        .filter(|p| !p.starts_with("dart:") && **p == paths::normalize(p))
        .cloned()
        .collect();
    let collection = Rc::new(AnalysisContextCollection::new(
        &paths,
        &CollectionOptions {
            sdk_path: sdk_path.clone(),
            ..Default::default()
        },
    ));

    let mut drivers: Vec<Driver> = Vec::new();
    let mut driver_context: Vec<Option<usize>> = Vec::new();
    let mut context_driver: IndexMap<usize, usize> = IndexMap::new();
    let mut sdk_driver: Option<usize> = None;
    let mut resolved: Vec<Input> = Vec::new();

    for p in inputs {
        if p.starts_with("dart:") {
            let d = *sdk_driver.get_or_insert_with(|| {
                drivers.push(sdk_only_driver(sdk_path.as_deref(), generation.clone()));
                driver_context.push(None);
                drivers.len() - 1
            });
            let driver = &mut drivers[d];
            let Some(uri) = Uri::try_parse(p) else {
                resolved.push(Input::Error("CannotResolveUriResult"));
                continue;
            };
            match driver.fs.get_file_for_uri(&uri) {
                None => resolved.push(Input::Error("CannotResolveUriResult")),
                Some(file) => resolved.push(Input::Library { driver: d, file }),
            }
            continue;
        }
        if *p != paths::normalize(p) {
            resolved.push(Input::Error("ArgumentError"));
            continue;
        }
        let Some(context_index) = collection
            .contexts
            .iter()
            .position(|c| c.root.is_analyzed(p))
        else {
            resolved.push(Input::Error("StateError"));
            continue;
        };
        let d = *context_driver.entry(context_index).or_insert_with(|| {
            drivers.push(context_driver_for(
                &collection,
                context_index,
                generation.clone(),
            ));
            driver_context.push(Some(context_index));
            drivers.len() - 1
        });
        let file = drivers[d].fs.get_file_for_path(p);
        resolved.push(Input::Library { driver: d, file });
    }

    // Discover, check kinds, link.
    let timings = std::env::var_os("DARTR_TIMINGS").is_some();
    let start = std::time::Instant::now();
    let mut libraries: Vec<Vec<FileId>> = vec![Vec::new(); drivers.len()];
    for driver in &mut drivers {
        driver.fs.discover();
    }
    let discovered = start.elapsed();
    for input in &mut resolved {
        if let Input::Library { driver, file } = *input {
            if drivers[driver].fs.file(file).kind().is_part() {
                *input = Input::Error("NotLibraryButPartResult");
            } else {
                libraries[driver].push(file);
            }
        }
    }
    for (d, driver) in drivers.iter_mut().enumerate() {
        driver.link_libraries(&libraries[d]);
    }
    if timings {
        let files: usize = drivers.iter().map(|d| d.fs.files().len()).sum();
        let cycles: usize = drivers.iter().map(|d| d.cycles.len()).sum();
        eprintln!(
            "timings: {files} files read and parsed in {:.1} ms; {cycles} cycles linked in {:.1} ms (threads: {})",
            discovered.as_secs_f64() * 1e3,
            (start.elapsed() - discovered).as_secs_f64() * 1e3,
            rayon::current_num_threads()
        );
    }
    LinkedInputs {
        collection,
        drivers,
        driver_context,
        inputs: resolved,
    }
}

/// One line per input path, in input order. [with_const]: add the
/// `"const"` key to const variables (`dump elements --with-const`).
pub fn dump_elements_all(inputs: &[String], interface: bool, with_const: bool) -> Vec<String> {
    let linked = link_inputs(inputs);
    let drivers = &linked.drivers;
    let resolved = &linked.inputs;

    let features = FeatureSet::default();
    let sink = NoopSink;
    inputs
        .iter()
        .zip(resolved.iter())
        .map(|(p, input)| match input {
            Input::Error(e) => error_json(p, e),
            Input::Library { driver, file } => {
                let driver = &drivers[*driver];
                let uri = &driver.fs.file(*file).uri_str;
                let Some(library) = driver.state.world.libraries.get(uri).copied() else {
                    return error_json(p, "NotLinked");
                };
                let tp = dartr_link::types_builder::world_type_provider(&driver.state.world);
                let ctx = Ctx {
                    world: &driver.state.world,
                    current: None,
                    local: None,
                    tp: &tp,
                    features: &features,
                    req: &sink,
                };
                if interface {
                    return dartr_typesystem::interface_dump::interface_library_json(
                        &ctx, p, library,
                    );
                }
                let consts = if with_const {
                    Some(crate::elements_const::library_const_values(driver, *file))
                } else {
                    None
                };
                let sources = Sources { driver, consts };
                library_json(&ctx, &sources, p, library)
            }
        })
        .collect()
}

/// `DumpSources` over the linked cycles of a driver.
struct Sources<'a> {
    driver: &'a Driver,
    /// `--with-const`: the JSON text of the `"const"` value of each const
    /// variable of the library.
    consts: Option<IndexMap<ElementId, String>>,
}

impl DumpSources for Sources<'_> {
    fn const_expr_source(&self, store: StoreId, expr: ConstExprId) -> String {
        for cycle in self.driver.cycles.values() {
            if cycle.store.id == store {
                return dartr_ast::to_source::to_source(&cycle.const_exprs.ast, expr.0);
            }
        }
        String::new()
    }

    fn const_value_json(&self, _ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
        self.consts.as_ref()?.get(&element).cloned()
    }
}

fn experiment_flags(names: &[&str]) -> Vec<ExperimentalFlag> {
    ExperimentalFlag::VALUES
        .iter()
        .copied()
        .filter(|f| names.contains(&f.name()))
        .collect()
}

/// A driver for one analysis context of the collection.
fn context_driver_for(
    collection: &Rc<AnalysisContextCollection>,
    context_index: usize,
    generation: Arc<Generation>,
) -> Driver {
    let context = &collection.contexts[context_index];
    let source_factory = SourceFactory {
        workspace: context.root.workspace.clone(),
        sdk: context.sdk.as_deref().cloned(),
    };
    let collection = collection.clone();
    let config_for = Box::new(move |path: &str, uri: &str| {
        let context = &collection.contexts[context_index];
        let version = context.language_version(path, uri);
        let options = collection.options_for(context, path);
        let flags = options.enable_experiment_flags.clone().unwrap_or_default();
        let enabled = dartr_project::experiments::enabled_experiments(&flags);
        FileConfig {
            package_language_version: (version.major, version.minor),
            experiments: experiment_flags(&enabled),
        }
    });
    Driver::new(FileSystemState::new(source_factory, config_for), generation)
}

/// The driver of the `dart:` inputs: a context with only the SDK.
fn sdk_only_driver(sdk_path: Option<&str>, generation: Arc<Generation>) -> Driver {
    let sdk = sdk_path.map(DartSdk::new);
    let version = sdk
        .as_ref()
        .and_then(|s| s.language_version())
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::empty(), "/"),
        sdk,
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    Driver::new(FileSystemState::new(source_factory, config_for), generation)
}
