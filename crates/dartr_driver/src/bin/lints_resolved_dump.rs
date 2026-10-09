// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
//! JSON-lines lint runner that links and resolves before running lint processors.
use dartr_driver::{
    driver::Driver,
    file_state::{FileConfig, FileSystemState, SourceFactory},
};
use dartr_element::Generation;
use dartr_lints::{Registry, rules::implemented_rules};
use dartr_project::{AnalysisContextCollection, CollectionOptions, paths};
use dartr_resolver::options::AnalysisOptions;
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead},
    rc::Rc,
    sync::Arc,
};

fn run() {
    if std::env::args().any(|arg| arg == "--list") {
        let registry = Registry::builtin();
        let rules: Vec<_> = dartr_lints::ALL_RULES
            .iter()
            .map(|rule| {
                let rule = registry.get_rule(rule.name).unwrap();
                json!({"name":rule.name,
                "codes":rule.diagnostic_codes().map(|code|code.name).collect::<Vec<_>>(),
                "implemented":implemented_rules().contains(&rule.name)})
            })
            .collect();
        println!("{}", json!(rules));
        return;
    }
    let requests: Vec<Value> = io::stdin()
        .lock()
        .lines()
        .map(|line| serde_json::from_str(&line.expect("read request")).expect("JSON request"))
        .collect();
    if requests.is_empty() {
        return;
    }
    let paths: Vec<String> = requests
        .iter()
        .map(|r| paths::normalize(r["path"].as_str().expect("path")))
        .collect();
    let collection = Rc::new(AnalysisContextCollection::new(
        &paths,
        &CollectionOptions {
            sdk_path: dartr_project::sdk::find_sdk_path(),
            ..Default::default()
        },
    ));
    let generation = Arc::new(Generation::new(0));
    let mut results: IndexMap<String, Value> = IndexMap::new();
    for context_index in 0..collection.contexts.len() {
        let context = &collection.contexts[context_index];
        let selected: Vec<usize> = paths
            .iter()
            .enumerate()
            .filter(|(_, p)| context.root.is_analyzed(p))
            .map(|(i, _)| i)
            .collect();
        if selected.is_empty() {
            continue;
        }
        let source_factory = SourceFactory {
            workspace: context.root.workspace.clone(),
            sdk: context.sdk.as_deref().cloned(),
        };
        let config_collection = collection.clone();
        let config_for = Box::new(move |path: &str, uri: &str| {
            let context = &config_collection.contexts[context_index];
            let version = context.language_version(path, uri);
            let options = config_collection.options_for(context, path);
            let flags = options.enable_experiment_flags.clone().unwrap_or_default();
            let enabled = dartr_project::experiments::enabled_experiments(&flags);
            FileConfig {
                package_language_version: (version.major, version.minor),
                experiments: dartr_lints::ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .filter(|flag| enabled.contains(&flag.name()))
                    .collect(),
            }
        });
        let mut driver = Driver::new(
            FileSystemState::new(source_factory, config_for),
            generation.clone(),
        );
        let files: Vec<_> = selected
            .iter()
            .map(|&i| driver.fs.get_file_for_path(&paths[i]))
            .collect();
        driver.fs.discover();
        let libraries: Vec<_> = files
            .iter()
            .copied()
            .filter(|&file| !driver.fs.file(file).kind().is_part())
            .collect();
        driver.link_libraries(&libraries);
        for (&index, &file) in selected.iter().zip(&files) {
            if driver.fs.file(file).kind().is_part() {
                continue;
            }
            let enabled: Vec<&str> = requests[index]["enabled"]
                .as_array()
                .expect("enabled rules")
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect();
            let result =
                driver.analyze_library_with_lints(file, AnalysisOptions::default(), &enabled);
            if let Some(result) = result {
                for unit in result.units {
                    let diagnostics: Vec<_> = unit
                        .diagnostics
                        .into_iter()
                        .filter(|d| d.code.diagnostic_type.name() == "LINT")
                        .map(|d| {
                            json!({"code":d.code.name,"severity":d.severity.name(),
                            "offset":d.offset,"length":d.length,"message":d.message})
                        })
                        .collect();
                    if let Some(panic) = &unit.panic {
                        eprintln!("resolution panic: {}: {}", unit.path, panic);
                    }
                    results.insert(unit.path.to_string(), json!({"path":unit.path.as_ref(),"diagnostics":diagnostics,"panic":unit.panic}));
                }
            } else {
                eprintln!("library not linked: {}", paths[index]);
            }
        }
        // Detached parts have parse-only context; attached parts are in the defining library.
        for (&index, &file) in selected.iter().zip(&files) {
            if !results.contains_key(&paths[index]) {
                let enabled: Vec<_> = requests[index]["enabled"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_str().unwrap())
                    .collect();
                let parsed = &driver.fs.file(file).c().parsed;
                let ds =
                    dartr_lints::lint(parsed, &parsed.ast.tokens.source, &paths[index], &enabled);
                let diagnostics: Vec<_> = ds.into_iter().map(|d|json!({"code":d.code.name,"severity":d.severity.name(),"offset":d.offset,"length":d.length,"message":d.message})).collect();
                results.insert(
                    paths[index].clone(),
                    json!({"path":paths[index],"diagnostics":diagnostics,"unresolvedPart":true}),
                );
            }
        }
    }
    for path in paths {
        println!(
            "{}",
            results.get(&path).cloned().unwrap_or_else(
                || json!({"path":path,"diagnostics":[],"error":"no analysis context"})
            )
        );
    }
}
fn main() {
    rayon::ThreadPoolBuilder::new()
        .stack_size(256 << 20)
        .build_global()
        .unwrap();
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
