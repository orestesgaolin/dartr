//! The source of the diagnostics of `dartr analyze`.
//!
//! [DiagnosticsProvider] returns, for each analyzed file, the diagnostics
//! that the analyzer reports for it (Dart `ErrorsResult.diagnostics` /
//! `ResolvedUnitResult.diagnostics`): after `// ignore:` filtering, before
//! the `errors:` severity processing of `analysis_options.yaml` (that is
//! server side, see [crate::server]).
//!
//! [ParseOnlyProvider] gives the parse diagnostics only (kept for
//! comparison: `dartr analyze --dartr-parse-only`).
//! [crate::driver_provider::DriverProvider] gives the diagnostics of the
//! analysis driver (`dartr_driver`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use dartr_ast_builder::ParsedUnit;
use dartr_ast_builder::parse::parse_file;
use dartr_diagnostics::{Diagnostic, all_codes};
use dartr_lints::{RuleContextUnit, lint_library_unfiltered};
use dartr_parser::ExperimentalFlag;
use dartr_project::analysis_options::DiagnosticSeverity as OptionsSeverity;
use dartr_project::package_config::Packages;
use dartr_project::{AnalysisContextCollection, AnalysisOptions, paths};
use dartr_syntax::LineInfo;
use rayon::prelude::*;

use crate::ignore_info::IgnoreInfo;
use crate::ignore_validator::{filter_ignored_diagnostics, is_generated, validate_ignores};

/// A file to analyze: an analyzed `.dart` file of a context of the
/// collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzedFile {
    /// Absolute, normalized path.
    pub path: String,
    /// Index into [AnalysisContextCollection::contexts].
    pub context: usize,
}

/// The diagnostics of one file.
#[derive(Debug)]
pub struct FileDiagnostics {
    pub path: String,
    /// The line starts of the file, for line and column numbers.
    pub line_info: LineInfo,
    /// The reported diagnostics, without the ignored ones. The severity is
    /// the default severity of the code.
    pub diagnostics: Vec<Diagnostic>,
}

/// Produces the diagnostics of analyzed files.
pub trait DiagnosticsProvider {
    /// The diagnostics of one file (Dart `AnalysisDriver.getErrors`).
    /// Returns `None` if the file cannot be read.
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics>;

    /// The diagnostics of all [files], in any order. Implementations should
    /// analyze in parallel; the default analyzes one file after the other.
    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        files
            .iter()
            .filter_map(|f| self.diagnostics_for_file(collection, f))
            .collect()
    }
}

/// Parse diagnostics (scanner and parser), the AST-only lint rules and the
/// ignore-comment diagnostics (`duplicate_ignore`, `unignorable_ignore`),
/// with `// ignore:` filtering. Each file is parsed like the analysis driver
/// parses it: with the language version of its package and the experiments
/// of its analysis options.
///
/// Lint rules: the enabled rules of the options of each file that
/// `dartr_lints` implements run on the library of the file (the defining
/// unit and its parts, found with the `part` and `part of` directives).
/// Enabled rules that are not implemented need resolution and are skipped
/// without a message (set `DARTR_DEBUG_LINTS=1` to log them to stderr).
/// Diagnostic order of a file, like the analyzer: parse diagnostics, lints,
/// ignore-comment diagnostics.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOnlyProvider;

/// Reads the content of a file (for example from open documents).
/// `None` if the file cannot be read.
pub type ContentReader<'a> = &'a (dyn Fn(&str) -> Option<String> + Sync);

/// Reads from the file system. The analyzer decodes with
/// `allowMalformed: true`.
pub fn read_from_disk(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The per-file input of the parse step. Unlike the collection (which uses
/// `Rc`), it can be sent to other threads.
#[derive(Clone)]
struct ParseTask {
    path: String,
    package_version: (u32, u32),
    settings: Arc<FileSettings>,
    packages: Arc<Packages>,
}

/// Settings derived from one [AnalysisOptions].
pub(crate) struct FileSettings {
    pub(crate) experiments: Vec<ExperimentalFlag>,
    /// Dart `unignorableDiagnosticCodeNames`.
    pub(crate) unignorable_names: HashSet<String>,
    /// The enabled lint rules that `dartr_lints` implements.
    pub(crate) lint_rules: Vec<&'static str>,
    /// The enabled lint rules that `dartr_lints` does not implement: an
    /// ignore of one of them is not reported as unnecessary, since dartr
    /// cannot know whether the rule reports there.
    pub(crate) unimplemented_lint_rules: HashSet<String>,
}

impl FileSettings {
    pub(crate) fn new(options: &AnalysisOptions) -> FileSettings {
        let experiments = options
            .enabled_experiments()
            .iter()
            .filter_map(|name| {
                ExperimentalFlag::VALUES
                    .iter()
                    .copied()
                    .find(|f| f.name() == *name)
            })
            .collect();
        let codes: Vec<(&str, OptionsSeverity)> = all_codes()
            .iter()
            .filter_map(|c| {
                let severity = match c.severity() {
                    dartr_diagnostics::DiagnosticSeverity::Info => OptionsSeverity::Info,
                    dartr_diagnostics::DiagnosticSeverity::Warning => OptionsSeverity::Warning,
                    dartr_diagnostics::DiagnosticSeverity::Error => OptionsSeverity::Error,
                    dartr_diagnostics::DiagnosticSeverity::None => return None,
                };
                Some((c.lower_case_name(), severity))
            })
            .collect();
        let unignorable_names = options
            .unignorable_code_names(&codes)
            .into_iter()
            .map(|n| n.to_lowercase())
            .collect();
        let implemented = dartr_lints::rules::implemented_rules();
        let debug = std::env::var_os("DARTR_DEBUG_LINTS").is_some();
        let mut lint_rules = Vec::new();
        let mut unimplemented_lint_rules = HashSet::new();
        for name in &options.lint_rules {
            match implemented.iter().find(|n| **n == name.as_str()) {
                Some(rule) => lint_rules.push(*rule),
                None => {
                    if debug {
                        eprintln!("dartr: lint rule {name} is not implemented (needs resolution)");
                    }
                    unimplemented_lint_rules.insert(name.to_lowercase());
                }
            }
        }
        FileSettings {
            experiments,
            unignorable_names,
            lint_rules,
            unimplemented_lint_rules,
        }
    }
}

/// Settings and packages shared by the tasks of one options and one context.
#[derive(Default)]
struct TaskCache {
    settings: HashMap<*const AnalysisOptions, Arc<FileSettings>>,
    packages: HashMap<usize, Arc<Packages>>,
}

impl TaskCache {
    fn task(
        &mut self,
        collection: &AnalysisContextCollection,
        context_index: usize,
        path: &str,
    ) -> ParseTask {
        let context = &collection.contexts[context_index];
        let info = context.file_info(path);
        let options = collection.options_for(context, path);
        let key = std::rc::Rc::as_ptr(options);
        let settings = self
            .settings
            .entry(key)
            .or_insert_with(|| Arc::new(FileSettings::new(options)))
            .clone();
        let packages = self
            .packages
            .entry(context_index)
            .or_insert_with(|| Arc::new(context.packages.clone()))
            .clone();
        ParseTask {
            path: path.to_string(),
            package_version: (info.language_version.major, info.language_version.minor),
            settings,
            packages,
        }
    }
}

struct LoadedUnit {
    path: String,
    content: String,
    parsed: ParsedUnit,
}

fn load_unit(
    read: ContentReader<'_>,
    path: &str,
    version: (u32, u32),
    settings: &FileSettings,
) -> Option<LoadedUnit> {
    let content = read(path)?;
    let content = match content.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => content,
    };
    let parsed = parse_file(&content, path, version, &settings.experiments);
    Some(LoadedUnit {
        path: path.to_string(),
        content,
        parsed,
    })
}

/// The diagnostics of one unit: parse diagnostics, [lints], ignore-comment
/// diagnostics, filtered with the ignore comments.
fn finish_unit(
    unit: &LoadedUnit,
    lints: Vec<Diagnostic>,
    settings: &FileSettings,
) -> FileDiagnostics {
    finish_file(
        &unit.path,
        &unit.content,
        &unit.parsed,
        unit.parsed.diagnostics.clone(),
        lints,
        settings,
    )
}

/// The diagnostics of one unit: the analysis [diagnostics] (parse
/// diagnostics first), [lints], the ignore-comment diagnostics, filtered
/// with the ignore comments of the unit.
pub(crate) fn finish_file(
    path: &str,
    content: &str,
    parsed: &ParsedUnit,
    mut diagnostics: Vec<Diagnostic>,
    lints: Vec<Diagnostic>,
    settings: &FileSettings,
) -> FileDiagnostics {
    let tokens = &parsed.ast.tokens;
    let first = parsed.ast.begin_token(dartr_ast::NodeId::from(parsed.unit));
    let ignore_info = IgnoreInfo::for_dart(tokens, first, &parsed.line_info, content);
    let unignorable = &settings.unignorable_names;
    diagnostics.extend(lints);
    if !is_generated(path) {
        let validate_unnecessary_ignores = settings.lint_rules.contains(&"unnecessary_ignore");
        let ignore_diagnostics = validate_ignores(
            &ignore_info,
            &diagnostics,
            &parsed.line_info,
            unignorable,
            validate_unnecessary_ignores,
            &settings.unimplemented_lint_rules,
        );
        diagnostics.extend(ignore_diagnostics);
    }
    let diagnostics =
        filter_ignored_diagnostics(diagnostics, &ignore_info, &parsed.line_info, unignorable);
    FileDiagnostics {
        path: path.to_string(),
        line_info: parsed.line_info.clone(),
        diagnostics,
    }
}

/// `part` and `part of` directives of a unit.
struct Directives {
    /// `Some` for a `part of` unit: the resolved path of `part of 'uri'`.
    part_of: Option<Option<String>>,
    /// The resolved paths of the `part` directives, in source order.
    parts: Vec<String>,
}

fn resolve_uri(uri: &str, path: &str, packages: &Packages) -> Option<String> {
    let uri = uri.split(['?', '#']).next().unwrap_or("");
    if uri.starts_with("package:") {
        return packages.resolve_package_uri(uri);
    }
    if uri.starts_with("file:") {
        return paths::file_uri_to_path(uri);
    }
    let scheme = uri.split('/').next().is_some_and(|s| s.contains(':'));
    if scheme || uri.is_empty() {
        return None;
    }
    let decoded = paths::percent_decode(uri);
    if paths::is_absolute(&decoded) {
        return Some(paths::normalize(&decoded));
    }
    Some(paths::normalize(&paths::join(
        paths::dirname(path),
        &decoded,
    )))
}

fn directives(parsed: &ParsedUnit, path: &str, packages: &Packages) -> Directives {
    use dartr_ast::{
        Id, NodeKind, PartDirective, PartOfDirective, SimpleStringLiteral, StringLiteral,
    };
    let ast = &parsed.ast;
    let value = |literal: Id<StringLiteral>| {
        ast.cast::<SimpleStringLiteral>(literal)
            .map(|n| ast.get(n).value.to_string())
    };
    let unit = ast.get(parsed.unit);
    let mut result = Directives {
        part_of: None,
        parts: Vec::new(),
    };
    for &directive in ast.list_raw(unit.directives) {
        match ast.kind(directive) {
            NodeKind::PartDirective => {
                let d = ast.get(Id::<PartDirective>::from_raw(directive));
                if let Some(path) = value(d.uri).and_then(|u| resolve_uri(&u, path, packages)) {
                    result.parts.push(path);
                }
            }
            NodeKind::PartOfDirective => {
                let d = ast.get(Id::<PartOfDirective>::from_raw(directive));
                result.part_of = Some(
                    d.uri
                        .and_then(value)
                        .and_then(|u| resolve_uri(&u, path, packages)),
                );
            }
            _ => {}
        }
    }
    result
}

/// The files of the directory of [path] that have a `part` directive for
/// [path] (for `part of name;`, which does not name the library file).
fn libraries_of_named_part(read: ContentReader<'_>, task: &ParseTask) -> Option<String> {
    let dir = paths::dirname(&task.path);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".dart"))
        .collect();
    names.sort();
    for name in names {
        let candidate = paths::join(dir, &name);
        if candidate == task.path {
            continue;
        }
        let Some(unit) = load_unit(read, &candidate, task.package_version, &task.settings) else {
            continue;
        };
        if directives(&unit.parsed, &candidate, &task.packages)
            .parts
            .contains(&task.path)
        {
            return Some(candidate);
        }
    }
    None
}

/// The defining unit of the library that [task] is a part of (`path`
/// itself if it is no part or no library is found).
fn find_root(read: ContentReader<'_>, task: &ParseTask, unit: &LoadedUnit) -> String {
    let mut current = task.path.clone();
    let mut info = directives(&unit.parsed, &current, &task.packages);
    for _ in 0..8 {
        let Some(part_of) = info.part_of.clone() else {
            return current;
        };
        let next = match part_of {
            Some(path) => Some(path),
            None => {
                let named = ParseTask {
                    path: current.clone(),
                    ..task.clone()
                };
                libraries_of_named_part(read, &named)
            }
        };
        let Some(next) = next else {
            return task.path.clone();
        };
        let Some(next_unit) = load_unit(read, &next, task.package_version, &task.settings) else {
            return task.path.clone();
        };
        info = directives(&next_unit.parsed, &next, &task.packages);
        current = next;
    }
    task.path.clone()
}

/// What the first step knows about a requested file.
enum Classified {
    Done(Box<FileDiagnostics>),
    /// The file is in the library with this defining unit.
    Library(String),
    Missing,
}

fn classify(read: ContentReader<'_>, task: &ParseTask) -> Classified {
    let Some(unit) = load_unit(read, &task.path, task.package_version, &task.settings) else {
        return Classified::Missing;
    };
    let rules = &task.settings.lint_rules;
    if rules.is_empty() {
        return Classified::Done(Box::new(finish_unit(&unit, Vec::new(), &task.settings)));
    }
    let info = directives(&unit.parsed, &task.path, &task.packages);
    if info.part_of.is_none() && info.parts.is_empty() {
        // A library of one unit.
        let lints = lint_library_unfiltered(
            &[RuleContextUnit {
                parsed: &unit.parsed,
                source: &unit.content,
                path: &unit.path,
            }],
            rules,
        )
        .remove(0);
        return Classified::Done(Box::new(finish_unit(&unit, lints, &task.settings)));
    }
    Classified::Library(find_root(read, task, &unit))
}

/// Loads the units of the library of [root]: the defining unit, then the
/// parts, depth first.
fn collect_units(
    read: ContentReader<'_>,
    root: &ParseTask,
    requested: &HashMap<String, ParseTask>,
) -> Vec<LoadedUnit> {
    let mut units: Vec<LoadedUnit> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending = vec![root.path.clone()];
    // Depth first in source order: process the stack with reversed parts.
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let task = requested.get(&path).unwrap_or(root);
        let Some(unit) = load_unit(read, &path, task.package_version, &root.settings) else {
            continue;
        };
        let info = directives(&unit.parsed, &path, &root.packages);
        pending.extend(info.parts.into_iter().rev());
        units.push(unit);
    }
    units
}

/// Lints the library of [root] and returns the diagnostics of the requested
/// units of it. Requested files that the defining unit does not include as
/// parts are analyzed as libraries of one unit.
fn run_library(
    read: ContentReader<'_>,
    root: &ParseTask,
    requested: &HashMap<String, ParseTask>,
    wanted: &[String],
) -> Vec<FileDiagnostics> {
    let units = collect_units(read, root, requested);
    let context_units: Vec<RuleContextUnit<'_>> = units
        .iter()
        .map(|u| RuleContextUnit {
            parsed: &u.parsed,
            source: &u.content,
            path: &u.path,
        })
        .collect();
    let mut lints = lint_library_unfiltered(&context_units, &root.settings.lint_rules);
    let mut result = Vec::new();
    for path in wanted {
        match units.iter().position(|u| &u.path == path) {
            Some(index) => {
                let settings = &requested[path].settings;
                result.push(finish_unit(
                    &units[index],
                    std::mem::take(&mut lints[index]),
                    settings,
                ));
            }
            None => {
                // Not included by its library (or no library found).
                let task = &requested[path];
                if let Some(unit) = load_unit(read, path, task.package_version, &task.settings) {
                    let lints = lint_library_unfiltered(
                        &[RuleContextUnit {
                            parsed: &unit.parsed,
                            source: &unit.content,
                            path: &unit.path,
                        }],
                        &task.settings.lint_rules,
                    )
                    .remove(0);
                    result.push(finish_unit(&unit, lints, &task.settings));
                }
            }
        }
    }
    result
}

pub(crate) fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .stack_size(256 << 20)
            .build()
            .expect("thread pool")
    })
}

/// The diagnostics of [files], in parallel, with the content from [read].
/// The order of the result is not defined.
pub fn diagnostics_with_reader(
    collection: &AnalysisContextCollection,
    files: &[AnalyzedFile],
    read: ContentReader<'_>,
) -> Vec<FileDiagnostics> {
    let mut cache = TaskCache::default();
    let tasks: Vec<ParseTask> = files
        .iter()
        .map(|f| cache.task(collection, f.context, &f.path))
        .collect();
    let classified: Vec<Classified> =
        pool().install(|| tasks.par_iter().map(|t| classify(read, t)).collect());
    let mut result = Vec::new();
    // Libraries by defining unit; the requested files of each.
    let mut libraries: Vec<(String, Vec<String>)> = Vec::new();
    let mut library_index: HashMap<String, usize> = HashMap::new();
    let mut requested: HashMap<String, ParseTask> = HashMap::new();
    let mut context_of: HashMap<String, usize> = HashMap::new();
    for ((task, file), class) in tasks.iter().zip(files).zip(classified) {
        match class {
            Classified::Done(d) => result.push(*d),
            Classified::Missing => {}
            Classified::Library(root) => {
                let index = *library_index.entry(root.clone()).or_insert_with(|| {
                    libraries.push((root.clone(), Vec::new()));
                    libraries.len() - 1
                });
                libraries[index].1.push(task.path.clone());
                requested.insert(task.path.clone(), task.clone());
                context_of.entry(root).or_insert(file.context);
            }
        }
    }
    // A task for each defining unit that was not requested.
    let mut roots: Vec<(ParseTask, Vec<String>)> = Vec::new();
    for (root, wanted) in libraries {
        let task = match requested.get(&root) {
            Some(task) => task.clone(),
            None => {
                let context = collection
                    .context_for(&root)
                    .and_then(|c| collection.contexts.iter().position(|x| std::ptr::eq(x, c)))
                    .unwrap_or(context_of[&root]);
                cache.task(collection, context, &root)
            }
        };
        roots.push((task, wanted));
    }
    let requested = &requested;
    let libs: Vec<FileDiagnostics> = pool().install(|| {
        roots
            .par_iter()
            .flat_map_iter(|(root, wanted)| run_library(read, root, requested, wanted))
            .collect()
    });
    result.extend(libs);
    result
}

impl DiagnosticsProvider for ParseOnlyProvider {
    fn diagnostics_for_file(
        &self,
        collection: &AnalysisContextCollection,
        file: &AnalyzedFile,
    ) -> Option<FileDiagnostics> {
        diagnostics_with_reader(collection, std::slice::from_ref(file), &read_from_disk)
            .into_iter()
            .next()
    }

    fn diagnostics_for_files(
        &self,
        collection: &AnalysisContextCollection,
        files: &[AnalyzedFile],
    ) -> Vec<FileDiagnostics> {
        diagnostics_with_reader(collection, files, &read_from_disk)
    }
}

/// The diagnostics of the analyzed non-Dart files: `analysis_options.yaml`,
/// `pubspec.yaml`, `AndroidManifest.xml` and the `fix_data.yaml` files of the
/// context roots (Dart `ContextManager` non-Dart file handling). The
/// `errors:` severity processing is already applied (see
/// [dartr_project::non_dart]); [crate::server::analysis_errors] does not
/// process these files again.
pub fn non_dart_diagnostics(collection: &AnalysisContextCollection) -> Vec<FileDiagnostics> {
    let mut result = Vec::new();
    for path in dartr_project::non_dart::analyzed_files(collection) {
        let Some(context) = dartr_project::non_dart::context_for_file(collection, &path) else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
        result.push(FileDiagnostics {
            line_info: LineInfo::from_content(content),
            diagnostics: dartr_project::non_dart::diagnostics_for_file(context, &path),
            path,
        });
    }
    result
}
