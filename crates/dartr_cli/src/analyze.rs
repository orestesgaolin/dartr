// Dart source: pkg/dartdev/lib/src/commands/analyze.dart (AnalyzeCommand.run)
// Dart source: pkg/dartdev/lib/src/experiments.dart (enabledExperiments)

//! `dartr analyze`: the command of `dart analyze`, with the analysis done in
//! the process instead of by an analysis server.

use std::cmp::Ordering;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::Instant;

use dartr_project::experiments::KNOWN_FEATURES;
use dartr_project::{AnalysisContextCollection, CollectionOptions, FileKind, paths};

use crate::args::{self, AnalyzeArgs, Format, UsageError};
use crate::output::{self, Ansi};
use crate::provider::{AnalyzedFile, DiagnosticsProvider};
use crate::server::{self, AnalysisError, LineInfoCache};

/// The exit code of a usage error (`UsageException` in dartdev, checked
/// with `dart analyze --bogus`).
pub const USAGE_EXIT_CODE: i32 = 64;

/// dartdev `_Result`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitResult {
    Success = 0,
    Infos = 1,
    Warnings = 2,
    Errors = 3,
    Crash = 4,
}

/// The terminal facts that change the output.
#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    /// `Ansi.terminalSupportsAnsi`.
    pub ansi: bool,
    /// `dartdevUsageLineLength`: the terminal width, `None` if stdout is
    /// not a terminal.
    pub columns: Option<usize>,
}

impl Terminal {
    /// No terminal: plain output, no wrapping (stdout is a pipe or a file).
    pub const NONE: Terminal = Terminal {
        ansi: false,
        columns: None,
    };

    /// Detects the terminal of the process stdout. The width is the
    /// window size of the terminal (`stdout.terminalColumns`); no wrapping
    /// if the terminal reports a width of 0.
    pub fn detect() -> Terminal {
        if !std::io::stdout().is_terminal() {
            return Terminal::NONE;
        }
        let ansi = std::env::var("TERM").map_or(true, |t| t != "dumb");
        Terminal {
            ansi,
            columns: terminal_columns().filter(|&c| c > 0),
        }
    }
}

/// The number of columns of the terminal of stdout (`TIOCGWINSZ`).
fn terminal_columns() -> Option<usize> {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `ioctl(TIOCGWINSZ)` writes a `winsize` into `size`.
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };
    (result == 0).then_some(size.ws_col as usize)
}

/// A file system entity named on the command line.
enum Target {
    Directory(String),
    File(String),
}

impl Target {
    fn path(&self) -> &str {
        match self {
            Target::Directory(p) | Target::File(p) => p,
        }
    }
}

/// Dart `path.basename` (trailing separators are ignored).
fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "" } else { "/" };
    }
    match trimmed.rfind('/') {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

/// `absolute.resolveSymbolicLinksSync()`.
fn resolve(path: &str) -> String {
    match std::fs::canonicalize(path) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => paths::absolute_normalized(path),
    }
}

fn current_dir() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_string())
}

/// The resident set size of this process in KB (`ProcessProfiler`, which
/// runs `ps`).
fn memory_kb() -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// Runs `analyze` with the arguments after the command name. Writes to
/// [stdout] and [stderr] and returns the exit code.
pub fn run(
    argv: &[String],
    provider: &dyn DiagnosticsProvider,
    terminal: Terminal,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let usage_error = |stderr: &mut dyn Write, error: UsageError| {
        let _ = stderr.write_all(args::usage_error_text(&error).as_bytes());
        USAGE_EXIT_CODE
    };
    let args = match args::parse(argv) {
        Ok(a) => a,
        Err(e) => return usage_error(stderr, e),
    };
    if args.help {
        let _ = stdout.write_all(args::help().as_bytes());
        return 0;
    }
    match run_parsed(&args, provider, terminal, stdout, stderr) {
        Ok(code) => code,
        Err(e) => usage_error(stderr, e),
    }
}

fn run_parsed(
    args: &AnalyzeArgs,
    provider: &dyn DiagnosticsProvider,
    terminal: Terminal,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32, UsageError> {
    // Find targets from the 'rest' params.
    let mut targets = Vec::new();
    if args.rest.is_empty() {
        targets.push(Target::Directory(current_dir()));
    } else {
        for target in &args.rest {
            let path = Path::new(target);
            if path.is_dir() {
                targets.push(Target::Directory(target.clone()));
            } else if path.is_file() {
                targets.push(Target::File(target.clone()));
            } else {
                return Err(UsageError(format!(
                    "Directory or file doesn't exist: {target}"
                )));
            }
        }
    }

    let machine_format = args.format == Format::Machine;
    let json_format = args.format == Format::Json;
    let print_memory = args.memory && json_format;

    if let Some(sdk_path) = &args.sdk_path {
        if !Path::new(sdk_path).is_dir() {
            return Err(UsageError(format!("Invalid Dart SDK path: {sdk_path}")));
        }
        let snapshot_name = if args.use_aot_snapshot {
            "analysis_server_aot.dart.snapshot"
        } else {
            "analysis_server.dart.snapshot"
        };
        let snapshot = Path::new(sdk_path)
            .join("bin")
            .join("snapshots")
            .join(snapshot_name);
        if !snapshot.is_file() {
            return Err(UsageError(format!(
                "Invalid Dart SDK path has no '{snapshot_name}' file: {sdk_path}"
            )));
        }
    }

    let enabled_experiments = enabled_experiments(args, stderr);
    let mut experiment_names: Vec<&str> = Vec::new();
    for experiment in &enabled_experiments {
        let name = experiment.strip_prefix("no-").unwrap_or(experiment);
        if !experiment_names.contains(&name) {
            experiment_names.push(name);
        }
    }
    let unknown: Vec<String> = experiment_names
        .iter()
        .filter(|n| !KNOWN_FEATURES.iter().any(|f| f.enable_string == **n))
        .map(|n| format!("'{n}'"))
        .collect();
    if !unknown.is_empty() {
        return Err(UsageError(format!(
            "Unknown experiment(s): {}",
            unknown.join(", ")
        )));
    }

    let targets_names: Vec<&str> = targets.iter().map(|t| basename(t.path())).collect();
    let start = Instant::now();
    let progress = !(machine_format || json_format);
    if progress {
        // cli_logging `SimpleProgress` (no terminal) or `AnsiProgress`.
        let message = format!("Analyzing {}...", targets_names.join(", "));
        if terminal.ansi {
            let _ = write!(stdout, "{:<40}", format!("{message}  "));
        } else {
            let _ = writeln!(stdout, "{message}");
        }
        let _ = stdout.flush();
    }

    // `AnalysisServer(_packagesFile(), ...)`.
    let packages = match &args.packages {
        Some(path) => {
            if !Path::new(path).is_file() {
                return Err(UsageError(format!("The file doesn't exist: {path}")));
            }
            Some(paths::absolute_normalized(path))
        }
        None => None,
    };
    // Also called by the server start (the warning is printed again).
    enabled_experiments_warnings(args, stderr);

    // `analysis.setAnalysisRoots`: canonical paths without a trailing slash.
    let roots: Vec<String> = targets
        .iter()
        .map(|t| {
            let resolved = resolve(t.path());
            match resolved.strip_suffix('/') {
                Some(r) if !r.is_empty() => r.to_string(),
                _ => resolved,
            }
        })
        .collect();
    let collection = AnalysisContextCollection::new(
        &roots,
        &CollectionOptions {
            options_file: None,
            package_config_file: packages,
            sdk_path: args.sdk_path.clone(),
            enabled_experiments: enabled_experiments.clone(),
        },
    );
    let t_contexts = start.elapsed();
    let files = analyzed_dart_files(&collection);
    let t_files = start.elapsed();
    let results = provider.diagnostics_for_files(&collection, &files);
    let t_diagnostics = start.elapsed();
    if std::env::var_os("DARTR_TIMINGS").is_some() {
        let _ = writeln!(
            stderr,
            "[timings] contexts {:?}, file list {:?} ({} files), diagnostics {:?}",
            t_contexts,
            t_files - t_contexts,
            files.len(),
            t_diagnostics - t_files
        );
    }

    let memory = if print_memory { memory_kb() } else { None };

    if progress && terminal.ansi {
        let _ = writeln!(stdout, "{:.1}s", start.elapsed().as_secs_f64());
    }

    // Errors in analysis_options.yaml and pubspec.yaml are reported first.
    let mut line_infos = LineInfoCache::default();
    let mut priority_errors = Vec::new();
    let mut non_priority_errors = Vec::new();
    for file in &results {
        let is_priority_file = matches!(
            paths::basename(&file.path),
            "analysis_options.yaml" | "pubspec.yaml"
        );
        for error in server::analysis_errors(&collection, file, &mut line_infos) {
            // dartdev drops TODO infos.
            if error.type_ == "TODO"
                && error.severity == dartr_diagnostics::DiagnosticSeverity::Info
            {
                continue;
            }
            if is_priority_file && error.severity == dartr_diagnostics::DiagnosticSeverity::Error {
                priority_errors.push(error);
            } else {
                non_priority_errors.push(error);
            }
        }
    }

    let mut out = String::new();
    if priority_errors.is_empty() && non_priority_errors.is_empty() {
        if json_format {
            output::emit_json_format(&mut out, &[], memory);
        } else if !machine_format {
            out.push_str("No issues found!\n");
        }
        let _ = stdout.write_all(out.as_bytes());
        return Ok(ExitResult::Success as i32);
    }

    let sort = |errors: &mut Vec<AnalysisError>| errors.sort_by(|a, b| a.compare(b));
    sort(&mut priority_errors);
    sort(&mut non_priority_errors);

    if machine_format {
        output::emit_machine_format(&mut out, &non_priority_errors);
    } else if json_format {
        output::emit_json_format(&mut out, &non_priority_errors, memory);
    } else {
        let relative_to = if targets.len() == 1 {
            match &targets[0] {
                Target::File(f) => {
                    let resolved = resolve(f);
                    paths::dirname(&resolved).to_string()
                }
                Target::Directory(d) => resolve(d),
            }
        } else {
            current_dir()
        };
        let ansi = Ansi {
            use_ansi: terminal.ansi,
        };
        let emit = |out: &mut String, errors: &[AnalysisError]| {
            output::emit_default_format(
                out,
                errors,
                &relative_to,
                args.verbose,
                ansi,
                terminal.columns,
            )
        };
        if !priority_errors.is_empty() {
            out.push('\n');
            out.push_str(
                "Errors were found in 'pubspec.yaml' and/or 'analysis_options.yaml' which might \
                 result in either invalid diagnostics being produced or valid diagnostics being \
                 missed.\n",
            );
            emit(&mut out, &priority_errors);
            if !non_priority_errors.is_empty() {
                out.push_str("Errors in remaining files.\n");
            }
        }
        if !non_priority_errors.is_empty() {
            emit(&mut out, &non_priority_errors);
        }
        let count = priority_errors.len() + non_priority_errors.len();
        let issues = if count == 1 { "issue" } else { "issues" };
        out.push_str(&format!("{count} {issues} found.\n"));
    }
    let _ = stdout.write_all(out.as_bytes());

    let all = priority_errors.iter().chain(&non_priority_errors);
    let (mut has_errors, mut has_warnings, mut has_infos) = (false, false, false);
    for error in all {
        match error.severity {
            dartr_diagnostics::DiagnosticSeverity::Error => has_errors = true,
            dartr_diagnostics::DiagnosticSeverity::Warning => has_warnings = true,
            dartr_diagnostics::DiagnosticSeverity::Info => has_infos = true,
            dartr_diagnostics::DiagnosticSeverity::None => {}
        }
    }
    let result = if has_errors {
        ExitResult::Errors
    } else if args.fatal_warnings && has_warnings {
        ExitResult::Warnings
    } else if args.fatal_infos && has_infos {
        ExitResult::Infos
    } else {
        ExitResult::Success
    };
    Ok(result as i32)
}

/// The analyzed `.dart` files of all contexts, without duplicates, in
/// path order.
pub fn analyzed_dart_files(collection: &AnalysisContextCollection) -> Vec<AnalyzedFile> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (index, context) in collection.contexts.iter().enumerate() {
        for path in context.root.analyzed_files() {
            // TODO(phase 9+): analysis_options.yaml, pubspec.yaml,
            // AndroidManifest.xml and fix_data.yaml diagnostics.
            if FileKind::of(&path) == FileKind::Dart && seen.insert(path.clone()) {
                files.push(AnalyzedFile {
                    path,
                    context: index,
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path).then(Ordering::Equal));
    files
}

/// dartdev `ArgResults.enabledExperiments`: the `--enable-experiment`
/// values, with a warning for each experiment that is enabled by default.
fn enabled_experiments(args: &AnalyzeArgs, stderr: &mut dyn Write) -> Vec<String> {
    enabled_experiments_warnings(args, stderr);
    args.enable_experiment.clone().unwrap_or_default()
}

fn enabled_experiments_warnings(args: &AnalyzeArgs, stderr: &mut dyn Write) {
    let Some(experiments) = &args.enable_experiment else {
        return;
    };
    let mut features: Vec<_> = KNOWN_FEATURES.iter().filter(|f| !f.is_expired).collect();
    features.sort_by(|a, b| a.enable_string.cmp(b.enable_string));
    for feature in features {
        if feature.is_enabled_by_default && experiments.iter().any(|e| e == feature.enable_string) {
            let _ = writeln!(
                stderr,
                "'{}' is now enabled by default; this flag is no longer required.",
                feature.enable_string
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_like_dart() {
        assert_eq!(basename("/a/b/"), "b");
        assert_eq!(basename("."), ".");
        assert_eq!(basename("lib/a.dart"), "a.dart");
    }
}
