// Dart source: pkg/dartdev/lib/src/commands/fix.dart (FixCommand)
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_bulk_fixes.dart (EditBulkFixes)
// Dart source: pkg/analysis_server/lib/src/services/correction/bulk_fix_processor.dart (BulkFixProcessor.fixErrors, _computeFixes)

//! `dartr fix`: the command line of `dart fix` (same options, output and
//! exit codes), with the bulk fix processor of the server in process.

use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

use dartr_cli::DriverSession;
use dartr_cli::provider::AnalyzedFile;
use dartr_cli::server::processed_diagnostics;
use dartr_diagnostics::Diagnostic;
use dartr_element::NoopSink;
use dartr_project::{AnalysisContextCollection, CollectionOptions};

use super::code_actions::{ServerWorkspace, options_for};
use super::{Document, ResolvedUnitRef};
use crate::correction::bulk_fix_processor::{
    BulkFixer, ChangeMap, apply_edits, is_fixable, is_generated,
};
use crate::correction::change::SourceFileEdit;
use crate::correction::change_builder::ChangeBuilder;
use crate::correction::fix_processor::FixUnit;
use crate::correction::utils::CorrectionUtils;

/// Dart `FixCommand.maxPasses`.
const MAX_PASSES: usize = 4;

const DESCRIPTION: &str = "Apply automated fixes to Dart source code.

This tool looks for and fixes analysis issues that have associated automated fixes.

To use the tool, run either 'dart fix --dry-run' for a preview of the proposed changes for a project, or 'dart fix --apply' to apply the changes.";

const USAGE: &str = "Usage: dart fix [arguments]
-h, --help                      Print this usage information.
-n, --dry-run                   Preview the proposed changes but make no changes.
    --apply                     Apply the proposed changes.
    --code=<code1,code2,...>    Apply fixes for one (or more) diagnostic codes.

Run \"dart help\" to see global options.";

struct Args {
    dry_run: bool,
    apply: bool,
    help: bool,
    codes: Vec<String>,
    rest: Vec<String>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        dry_run: false,
        apply: false,
        help: false,
        codes: Vec::new(),
        rest: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == "--" {
            parsed.rest.extend(args[i..].iter().cloned());
            break;
        }
        match a.as_str() {
            "-n" | "--dry-run" => parsed.dry_run = true,
            "--apply" => parsed.apply = true,
            "-h" | "--help" => parsed.help = true,
            "--code" => {
                let Some(v) = args.get(i) else {
                    return Err("Missing argument for \"code\".".to_string());
                };
                i += 1;
                parsed.codes.extend(v.split(',').map(str::to_string));
            }
            _ => {
                if let Some(v) = a.strip_prefix("--code=") {
                    parsed.codes.extend(v.split(',').map(str::to_string));
                } else if a.starts_with("--") {
                    let name = a[2..].split('=').next().unwrap_or("");
                    return Err(format!("Could not find an option named \"--{name}\"."));
                } else if a.starts_with('-') && a.len() > 1 {
                    // Combined short flags, for example `-nh`.
                    for ch in a[1..].chars() {
                        match ch {
                            'n' => parsed.dry_run = true,
                            'h' => parsed.help = true,
                            _ => {
                                return Err(format!("Could not find an option or flag \"-{ch}\"."));
                            }
                        }
                    }
                } else {
                    parsed.rest.push(a.clone());
                }
            }
        }
    }
    Ok(parsed)
}

fn usage_error(err: &mut dyn Write, message: &str) -> i32 {
    let _ = writeln!(err, "{message}\n\n{USAGE}");
    64
}

/// Dart `path.canonicalize(path.normalize(p))` for the target.
fn canonical(path: &str) -> String {
    let absolute = if path.starts_with('/') {
        path.to_string()
    } else {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        format!("{cwd}/{path}")
    };
    dartr_project::paths::absolute_normalized(&absolute)
}

/// Dart `path.relative(p, from: dir)`.
fn relative(p: &str, dir: &str) -> String {
    let to: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    let from: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let common = to.iter().zip(&from).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn plural_fix(n: usize) -> &'static str {
    if n == 1 { "fix" } else { "fixes" }
}

fn pluralize(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// Dart `quotedAndCommaSeparatedWithAnd`.
fn quoted_and(items: &[String]) -> String {
    let q: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    match q.len() {
        0 => String::new(),
        1 => q[0].clone(),
        2 => format!("{} and {}", q[0], q[1]),
        n => format!("{}, and {}", q[..n - 1].join(", "), q[n - 1]),
    }
}

/// Runs `dartr fix` with the arguments after `fix`; returns the exit code.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let args = match parse_args(args) {
        Ok(a) => a,
        Err(message) => return usage_error(err, &message),
    };
    if args.help || (!args.apply && !args.dry_run) || (args.apply && args.dry_run) {
        let _ = writeln!(out, "{DESCRIPTION}\n\n{USAGE}");
        return 0;
    }
    if args.rest.len() > 1 {
        return usage_error(err, "Only one file or directory is expected.");
    }
    let base = match args.rest.first() {
        Some(p) => p.clone(),
        None => std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    let target = canonical(&base);
    let meta = std::fs::metadata(&target);
    let is_directory = meta.as_ref().is_ok_and(|m| m.is_dir());
    if meta.is_err() {
        let entity = if is_directory { "Directory" } else { "File" };
        return usage_error(err, &format!("{entity} doesn't exist: {target}"));
    }
    let args_target = args.rest.first().cloned().unwrap_or_default();
    let mode_text = if args.dry_run { " (dry run)" } else { "" };
    let target_name = target.rsplit('/').next().unwrap_or("").to_string();
    let _ = writeln!(out, "Computing fixes in {target_name}{mode_text}...");
    let _ = out.flush();

    let codes: Option<Vec<String>> = if args.codes.is_empty() {
        None
    } else {
        Some(args.codes.iter().map(|c| c.to_lowercase()).collect())
    };
    let mut fixer = Fixer::new(&target);
    if let Some(codes) = &codes {
        fixer.collection.enable_lint_rules(codes);
    }
    let result = fixer.apply_all_edits(codes.as_deref());
    let dir = if is_directory {
        target.clone()
    } else {
        dartr_project::paths::dirname(&target).to_string()
    };
    let details = match result {
        Err(message) => {
            let _ = writeln!(out, "Unable to compute fixes: {message}");
            return 3;
        }
        Ok(details) => details,
    };
    if details.library_map.is_empty() {
        let _ = writeln!(out, "Nothing to fix!");
        return 0;
    }
    let file_count = details.library_map.len();
    let fix_count: usize = details.library_map.values().flat_map(|m| m.values()).sum();
    if args.dry_run {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{fix_count} proposed {} in {file_count} {}.",
            plural_fix(fix_count),
            pluralize("file", file_count)
        );
        print_details(out, &details, &dir);
        let mut codes: Vec<&String> = details
            .library_map
            .values()
            .flat_map(|m| m.keys())
            .collect();
        codes.sort();
        codes.dedup();
        let _ = writeln!(out, "To fix an individual diagnostic, run one of:");
        for code in codes {
            let _ = writeln!(out, "  dart fix --apply --code={code} {args_target}");
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "To fix all diagnostics, run:");
        let _ = writeln!(out, "  dart fix --apply {args_target}");
    } else {
        let _ = writeln!(out, "Applying fixes...");
        for (path, content) in &fixer.content_cache {
            if let Err(e) = std::fs::write(path, content) {
                let _ = writeln!(err, "{path}: {e}");
            }
        }
        print_details(out, &details, &dir);
        let _ = writeln!(
            out,
            "{fix_count} {} made in {file_count} {}.",
            plural_fix(fix_count),
            pluralize("file", file_count)
        );
    }
    0
}

/// Dart `_printDetails`.
fn print_details(out: &mut dyn Write, details: &ChangeMap, dir: &str) {
    let _ = writeln!(out);
    let mut paths: Vec<&String> = details.library_map.keys().collect();
    paths.sort_by_key(|p| relative(p, dir));
    for path in paths {
        let _ = writeln!(out, "{}", relative(path, dir));
        let mut fixes: Vec<(&String, &usize)> = details.library_map[path].iter().collect();
        fixes.sort_by(|a, b| a.0.cmp(b.0));
        for (code, n) in fixes {
            let _ = writeln!(out, "  {code} - {n} {}", plural_fix(*n));
        }
        let _ = writeln!(out);
    }
}

/// The analysis state of `dartr fix`: the session and the new contents.
struct Fixer {
    target: String,
    session: DriverSession,
    collection: AnalysisContextCollection,
    /// Dart `fileContentCache`.
    content_cache: indexmap::IndexMap<String, String>,
    overlays: HashMap<String, Document>,
}

/// Dart `BulkFixProcessor._computeFixes` code check: the message for the
/// codes that the analyzer does not define.
pub fn undefined_codes_message(codes: &[String]) -> Option<String> {
    let undefined: Vec<String> = codes
        .iter()
        .filter(|c| {
            !dartr_diagnostics::all_codes()
                .iter()
                .any(|d| d.name == c.as_str())
        })
        .cloned()
        .collect();
    if undefined.is_empty() {
        return None;
    }
    let count = undefined.len();
    Some(format!(
        "The {} {} {} not defined by the analyzer.",
        pluralize("diagnostic", count),
        quoted_and(&undefined),
        if count == 1 { "is" } else { "are" }
    ))
}

/// Dart `BulkFixProcessor.fixErrors` as the legacy `edit.bulkFixes`
/// request runs it: one pass over the files of a new collection of
/// [included] (open files are read through the overlays of
/// `dartr_project::fs`). Returns the edits and the fixes by path and code,
/// or the error message for undefined [codes] (lower case names).
pub fn bulk_fixes(
    included: &[String],
    options: &CollectionOptions,
    codes: Option<&[String]>,
) -> Result<(Vec<SourceFileEdit>, ChangeMap), String> {
    if let Some(message) = codes.and_then(undefined_codes_message) {
        return Err(message);
    }
    let mut collection = AnalysisContextCollection::new(included, options);
    if let Some(codes) = codes {
        collection.enable_lint_rules(codes);
    }
    let mut fixer = Fixer::with_collection(collection);
    Ok(fixer.fix_errors(codes))
}

/// Dart `BulkFixProcessor.fixPubspec` (the legacy `edit.bulkFixes` with
/// `updatePubspec`): the edits of the `pubspec.yaml` files of [included]
/// and the fixes by path (`missing_dependency`).
pub fn bulk_fixes_pubspec(
    included: &[String],
    options: &CollectionOptions,
) -> (Vec<SourceFileEdit>, ChangeMap) {
    let collection = AnalysisContextCollection::new(included, options);
    let (edits, pubspecs) = crate::correction::pubspec_fix::pubspec_fixes(&collection);
    let mut details = ChangeMap::default();
    for pubspec in pubspecs {
        details.add(&pubspec, "missing_dependency");
    }
    (edits, details)
}

impl Fixer {
    fn with_collection(collection: AnalysisContextCollection) -> Fixer {
        Fixer {
            target: String::new(),
            session: DriverSession::default(),
            collection,
            content_cache: indexmap::IndexMap::new(),
            overlays: HashMap::new(),
        }
    }

    fn new(target: &str) -> Fixer {
        let collection =
            AnalysisContextCollection::new(&[target.to_string()], &CollectionOptions::default());
        Fixer {
            target: target.to_string(),
            session: DriverSession::default(),
            collection,
            content_cache: indexmap::IndexMap::new(),
            overlays: HashMap::new(),
        }
    }

    /// Dart `applyAllEdits` (without the pubspec pass).
    fn apply_all_edits(&mut self, codes: Option<&[String]>) -> Result<ChangeMap, String> {
        if let Some(message) = codes.and_then(undefined_codes_message) {
            return Err(message);
        }
        let mut details = ChangeMap::default();
        let mut last_empty = false;
        for _ in 0..MAX_PASSES {
            let (edits, map) = self.fix_errors(codes);
            for (path, fixes) in map.library_map {
                let entry = details.library_map.entry(path).or_default();
                for (code, n) in fixes {
                    *entry.entry(code).or_insert(0) += n;
                }
            }
            last_empty = edits.is_empty();
            if edits.is_empty() {
                break;
            }
            self.apply_edits(&edits);
        }
        // Dart: the pubspec pass when the last pass had no edits.
        if last_empty && !details.library_map.is_empty() {
            let (edits, pubspecs) = crate::correction::pubspec_fix::pubspec_fixes(&self.collection);
            for pubspec in pubspecs {
                details
                    .library_map
                    .entry(pubspec)
                    .or_default()
                    .insert("missing_dependency".to_string(), 1);
            }
            self.apply_edits(&edits);
        }
        let _ = &self.target;
        Ok(details)
    }

    /// Dart `_applyEdits`: the new contents become overlays.
    fn apply_edits(&mut self, edits: &[SourceFileEdit]) {
        for edit in edits {
            let content = match self.content_cache.get(&edit.file) {
                Some(c) => c.clone(),
                None => super::read_file(&edit.file).unwrap_or_default(),
            };
            let new_content = apply_edits(&content, &edit.edits);
            self.content_cache
                .insert(edit.file.clone(), new_content.clone());
            dartr_project::fs::set_overlay(&edit.file, Some(new_content.clone()));
            self.overlays.insert(
                edit.file.clone(),
                Document {
                    content: new_content,
                    version: None,
                },
            );
            self.session.change_file(&edit.file);
        }
    }

    fn fix_errors(&mut self, codes: Option<&[String]>) -> (Vec<SourceFileEdit>, ChangeMap) {
        fix_errors(
            &mut self.session,
            &self.collection,
            &self.overlays,
            codes,
            &|_| true,
        )
    }
}

/// The diagnostics of [path] with their default severities (Dart
/// `unitResult.diagnostics`), the lints last.
fn unit_diagnostics(
    session: &mut DriverSession,
    collection: &AnalysisContextCollection,
    path: &str,
    context: usize,
) -> Vec<Diagnostic> {
    let files = [AnalyzedFile {
        path: path.to_string(),
        context,
    }];
    let results = session.diagnostics(collection, &files);
    let mut out = Vec::new();
    let mut lints = Vec::new();
    for file in &results {
        if file.path != path {
            continue;
        }
        for (d, _) in processed_diagnostics(collection, file) {
            let mut d = d.clone();
            d.severity = d.code.severity();
            if d.code.diagnostic_type == dartr_diagnostics::DiagnosticType::Lint {
                lints.push(d);
            } else {
                out.push(d);
            }
        }
    }
    lints.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.code.name.cmp(b.code.name)));
    out.extend(lints);
    out
}

/// Dart `BulkFixProcessor.fixErrors`: one pass over the analyzed files of
/// [collection] that [include] accepts, with one change builder.
pub(crate) fn fix_errors(
    session: &mut DriverSession,
    collection: &AnalysisContextCollection,
    overlays: &HashMap<String, Document>,
    codes: Option<&[String]>,
    include: &dyn Fn(&str) -> bool,
) -> (Vec<SourceFileEdit>, ChangeMap) {
    let mut files: Vec<(String, Vec<Diagnostic>)> = Vec::new();
    let contexts: Vec<(usize, Vec<String>)> = collection
        .contexts
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.root.analyzed_files()))
        .collect();
    for (index, paths) in contexts {
        for path in paths {
            if !path.ends_with(".dart") || is_generated(&path) || !include(&path) {
                continue;
            }
            let diagnostics = unit_diagnostics(session, collection, &path, index);
            files.push((path, diagnostics));
        }
    }
    let mut workspace = ServerWorkspace {
        session,
        collection,
        overlays,
    };
    let mut builder = ChangeBuilder::new(&mut workspace, None);
    let mut change_map = ChangeMap::default();
    for (path, diagnostics) in &files {
        let filtered =
            crate::correction::bulk_fix_processor::filter_diagnostics(diagnostics, codes);
        if filtered.is_empty() {
            continue;
        }
        let Some(resolved) = builder.workspace.resolved_unit(path) else {
            continue;
        };
        let resolved = Rc::new(ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        });
        let options = options_for(collection, path);
        let line_info = resolved.line_info().clone();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let utils = CorrectionUtils::new(&unit.ast, &line_info);
        let fix_unit = FixUnit {
            resolved: &resolved,
            ctx: &ctx,
            utils: &utils,
            path,
            options: &options,
            diagnostics,
        };
        if !filtered.iter().any(|d| is_fixable(&fix_unit, d)) {
            continue;
        }
        builder.default_eol = utils.end_of_line.clone();
        let mut fixer = BulkFixer {
            builder: &mut builder,
            change_map: std::mem::take(&mut change_map),
            codes: codes.map(<[String]>::to_vec),
            auto_triggered: false,
        };
        fixer.fix_errors_in_unit(&fix_unit, resolved.index == 0);
        change_map = std::mem::take(&mut fixer.change_map);
    }
    let change = builder.source_change();
    (change.edits, change_map)
}
