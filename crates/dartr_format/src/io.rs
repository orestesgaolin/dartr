// Dart source: dart_style lib/src/io.dart

//! Formats stdin or the files and directories given on the command line.
//!
//! Dart formats one file after the other. Here the files of each path
//! argument are formatted in parallel (rayon), and the results are reported
//! in the order of Dart: configuration lookups run first in file order (they
//! share a cache), then the files are read and formatted in parallel, then
//! the output, messages and summary are written in file order.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::cli::formatter_options::FormatterOptions;
use crate::config_cache::ConfigCache;
use crate::dart_formatter::{DartFormatter, TrailingCommas};
use crate::dart_version_history::Version;
use crate::exceptions::FormatError;
use crate::source_code::SourceCode;

/// The output streams of the command.
pub struct Console<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    /// Dart `stdioType(stderr) == StdioType.terminal` (syntax errors are
    /// highlighted with colors).
    pub stderr_is_terminal: bool,
}

/// Reads and formats input from stdin until closed.
pub fn format_stdin(
    options: &mut FormatterOptions,
    selection: Option<(i64, i64)>,
    path: Option<&str>,
    stdin: &mut dyn Read,
    console: &mut Console<'_>,
) {
    let mut selection_start = 0;
    let mut selection_length = 0;

    if let Some((start, length)) = selection {
        selection_start = start;
        selection_length = length;
    }

    let mut cache = ConfigCache::new();
    let mut messages = String::new();

    let mut language_version = options.language_version;
    if language_version.is_none()
        && let Some(path) = path
    {
        // We have a stdin-name, so look for a surrounding package config.
        language_version = cache.find_language_version(path, path, &mut messages);
    }

    // If they didn't specify a version or a path, or couldn't find a package
    // surrounding the path, then default to the latest version.
    let language_version = language_version.unwrap_or(DartFormatter::LATEST_LANGUAGE_VERSION);

    // Determine the page width.
    let mut page_width = options.page_width;
    let mut trailing_commas = options.trailing_commas;
    if (page_width.is_none() || trailing_commas.is_none())
        && let Some(path) = path
    {
        // We have a stdin-name, so look for a surrounding analyisis_options.yaml.
        match cache.find_formatter_options(path, &mut messages) {
            Ok(config) => {
                page_width = page_width.or(config.page_width);
                trailing_commas = trailing_commas.or(config.trailing_commas);
            }
            Err(crash) => {
                let _ = console.stderr.write_all(messages.as_bytes());
                let _ = writeln!(console.stderr, "{crash}");
                options.exit_code = 1;
                return;
            }
        }
    }
    let _ = console.stderr.write_all(messages.as_bytes());

    // Use a default page width if we don't have a specified one and couldn't
    // find a configured one.
    let page_width = page_width.unwrap_or(DartFormatter::DEFAULT_PAGE_WIDTH);

    let name = path.unwrap_or("stdin");

    let mut bytes = Vec::new();
    let _ = stdin.read_to_end(&mut bytes);
    // Dart `Utf8Decoder()` (not `allowMalformed`): invalid input is an error.
    let input = match String::from_utf8(bytes) {
        Ok(input) => input,
        Err(err) => {
            let _ = writeln!(
                console.stderr,
                "FormatException: Unexpected extension byte (at offset {})",
                err.utf8_error().valid_up_to()
            );
            options.exit_code = 1;
            return;
        }
    };

    let formatter = make_formatter(language_version, options, page_width, trailing_commas);
    let start = Instant::now();
    let result = to_usize(selection_start)
        .zip(to_usize(selection_length))
        .ok_or_else(|| "Invalid argument(s): selection must not be negative.".to_string())
        .and_then(|(start, length)| {
            SourceCode::new(
                input,
                path.map(str::to_string),
                true,
                Some(start),
                Some(length),
            )
            .map_err(|e| format!("Invalid argument(s): {e}"))
        });
    let source = match result {
        Ok(source) => source,
        Err(err) => {
            let _ = writeln!(
                console.stderr,
                "Hit a bug in the formatter when formatting stdin.\nPlease report at: github.com/dart-lang/dart_style/issues\n{err}\n"
            );
            options.exit_code = 70; // sysexits.h: EX_SOFTWARE
            return;
        }
    };
    match formatter.format_source(&source) {
        Ok(output) => {
            let changed = source.text != output.text;
            options.after_file(console, None, name, &output, changed, start.elapsed());
        }
        Err(FormatError::Formatter(err)) => {
            // Dart `err.message()`: `color` is `null`, so no colors.
            let _ = writeln!(console.stderr, "{}", err.message(false));
            options.exit_code = 65; // sysexits.h: EX_DATAERR
        }
        Err(err) => {
            let _ = writeln!(
                console.stderr,
                "Hit a bug in the formatter when formatting stdin.\nPlease report at: github.com/dart-lang/dart_style/issues\n{err}\n"
            );
            options.exit_code = 70; // sysexits.h: EX_SOFTWARE
        }
    }
}

fn to_usize(value: i64) -> Option<usize> {
    usize::try_from(value).ok()
}

fn make_formatter(
    language_version: Version,
    options: &FormatterOptions,
    page_width: usize,
    trailing_commas: Option<TrailingCommas>,
) -> DartFormatter {
    let mut formatter = DartFormatter::new(language_version);
    formatter.indent = options.indent;
    formatter.page_width = page_width;
    formatter.trailing_commas = trailing_commas.unwrap_or_default();
    formatter.experiment_flags = options.experiment_flags.clone();
    formatter
}

/// Formats all of the files and directories given by [paths].
///
/// Returns an error (the message of an uncaught Dart exception) if the
/// command crashed.
pub fn format_paths(
    options: &mut FormatterOptions,
    paths: &[String],
    console: &mut Console<'_>,
) -> Result<(), String> {
    // If the user didn't specify a language version, then look for surrounding
    // package configs so we know what language versions to use for the files.
    let mut cache = ConfigCache::new();

    for path in paths {
        if std::fs::metadata(path).is_ok_and(|m| m.is_dir()) {
            if !process_directory(&mut cache, options, path, console)? {
                options.exit_code = 65;
            }
            continue;
        }

        if std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
            let jobs = vec![(path.clone(), path.clone())];
            if !process_files(&mut cache, options, &jobs, console)? {
                options.exit_code = 65;
            }
        } else {
            let _ = writeln!(console.stderr, "No file or directory found at \"{path}\".");
        }
    }
    Ok(())
}

/// An entry of a recursive directory listing (Dart `FileSystemEntity`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    File,
    Directory,
    Link,
}

/// Dart `Directory(path).listSync(recursive: true, followLinks: ...)`:
/// every entry below [directory], with paths built like Dart (the directory
/// path, a `/` unless it already ends with one, the name).
fn list_recursive(directory: &str, follow_links: bool) -> Vec<(String, EntryKind)> {
    let mut entries = Vec::new();
    // Canonical paths of the directories being listed, to stop at link
    // cycles when following links.
    let mut active: Vec<std::path::PathBuf> = Vec::new();
    list_into(directory, follow_links, &mut active, &mut entries);
    entries
}

fn list_into(
    directory: &str,
    follow_links: bool,
    active: &mut Vec<std::path::PathBuf>,
    entries: &mut Vec<(String, EntryKind)>,
) {
    let Ok(read_dir) = std::fs::read_dir(directory) else {
        return;
    };
    if follow_links {
        match std::fs::canonicalize(directory) {
            Ok(canonical) => {
                if active.contains(&canonical) {
                    return;
                }
                active.push(canonical);
            }
            Err(_) => return,
        }
    }
    let mut children: Vec<(String, EntryKind)> = Vec::new();
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if directory.ends_with('/') {
            format!("{directory}{name}")
        } else {
            format!("{directory}/{name}")
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let kind = if file_type.is_symlink() {
            if follow_links {
                match std::fs::metadata(&path) {
                    Ok(m) if m.is_dir() => EntryKind::Directory,
                    Ok(_) => EntryKind::File,
                    // A broken link.
                    Err(_) => EntryKind::Link,
                }
            } else {
                EntryKind::Link
            }
        } else if file_type.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        };
        children.push((path, kind));
    }
    for (path, kind) in children {
        entries.push((path.clone(), kind));
        if kind == EntryKind::Directory {
            list_into(&path, follow_links, active, entries);
        }
    }
    if follow_links {
        active.pop();
    }
}

/// Dart `String.compareTo`: compares UTF-16 code units.
fn compare_utf16(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Runs the formatter on every .dart file in [directory] (and its
/// subdirectories), and replaces them with their formatted output.
///
/// Returns `true` if successful or `false` if an error occurred in any of the
/// files.
fn process_directory(
    cache: &mut ConfigCache,
    options: &mut FormatterOptions,
    directory: &str,
    console: &mut Console<'_>,
) -> Result<bool, String> {
    let mut entries = list_recursive(directory, options.follow_links);
    entries.sort_by(|a, b| compare_utf16(&a.0, &b.0));

    let mut jobs: Vec<(String, String)> = Vec::new();
    for (path, kind) in entries {
        if kind != EntryKind::File || !path.ends_with(".dart") {
            continue;
        }

        // If the path is in a subdirectory starting with ".", ignore it.
        let relative =
            dartr_project::paths::normalize(path[directory.len()..].trim_start_matches('/'));
        if relative.split('/').any(|part| part.starts_with('.')) {
            continue;
        }

        let display_path = dartr_project::paths::normalize(&path);
        jobs.push((path, display_path));
    }

    process_files(cache, options, &jobs, console)
}

/// The configuration of one file and the messages that its lookup wrote.
struct Prepared {
    formatter: DartFormatter,
    messages: String,
}

/// The result of formatting one file.
enum Outcome {
    Formatted {
        source: SourceCode,
        output: SourceCode,
        elapsed: Duration,
    },
    Failed(FormatError),
    /// The file could not be read (Dart `FileSystemException`).
    ReadError(String),
}

/// Runs the formatter on each `(file, display path)` of [jobs] (Dart
/// `_processFile` for each), formatting in parallel and reporting in order.
///
/// Returns `true` if successful or `false` if an error occurred in any file.
fn process_files(
    cache: &mut ConfigCache,
    options: &mut FormatterOptions,
    jobs: &[(String, String)],
    console: &mut Console<'_>,
) -> Result<bool, String> {
    // Determine the configuration of each file, in order.
    let mut prepared: Vec<Prepared> = Vec::with_capacity(jobs.len());
    let mut crash: Option<String> = None;
    for (file, display_path) in jobs {
        let mut messages = String::new();

        // Determine what language version to use.
        let language_version = match options.language_version {
            Some(version) => Some(version),
            None => cache.find_language_version(file, display_path, &mut messages),
        };

        // If they didn't specify a version and we couldn't find a surrounding
        // package, then default to the latest version.
        let language_version = language_version.unwrap_or(DartFormatter::LATEST_LANGUAGE_VERSION);

        // Determine the configuration options.
        let mut page_width = options.page_width;
        let mut trailing_commas = options.trailing_commas;
        if page_width.is_none() || trailing_commas.is_none() {
            match cache.find_formatter_options(file, &mut messages) {
                Ok(config) => {
                    page_width = page_width.or(config.page_width);
                    trailing_commas = trailing_commas.or(config.trailing_commas);
                }
                Err(text) => {
                    messages.push_str(&text);
                    messages.push('\n');
                    crash = Some(messages);
                    break;
                }
            }
        }

        // Use a default page width if we don't have a specified one and
        // couldn't find a configured one.
        let page_width = page_width.unwrap_or(DartFormatter::DEFAULT_PAGE_WIDTH);

        prepared.push(Prepared {
            formatter: make_formatter(language_version, options, page_width, trailing_commas),
            messages,
        });
    }

    // Format the files in parallel.
    let outcomes: Vec<Outcome> = jobs[..prepared.len()]
        .par_iter()
        .zip(prepared.par_iter())
        .map(|((file, _), prepared)| format_file(file, &prepared.formatter))
        .collect();

    // Report in order.
    let mut success = true;
    for (((file, display_path), prepared), outcome) in jobs.iter().zip(&prepared).zip(outcomes) {
        let _ = console.stderr.write_all(prepared.messages.as_bytes());
        match outcome {
            Outcome::Formatted {
                source,
                output,
                elapsed,
            } => {
                let changed = source.text != output.text;
                options.after_file(console, Some(file), display_path, &output, changed, elapsed);
            }
            Outcome::Failed(FormatError::Formatter(err)) => {
                let color = console.stderr_is_terminal;
                let _ = writeln!(console.stderr, "{}", err.message(color));
                success = false;
            }
            Outcome::Failed(FormatError::UnexpectedOutput(err)) => {
                let _ = writeln!(
                    console.stderr,
                    "Hit a bug in the formatter when formatting {display_path}.\n{err}\nPlease report at github.com/dart-lang/dart_style/issues."
                );
                success = false;
            }
            Outcome::Failed(FormatError::Other(err)) | Outcome::ReadError(err) => {
                let _ = writeln!(
                    console.stderr,
                    "Hit a bug in the formatter when formatting {display_path}.\nPlease report at github.com/dart-lang/dart_style/issues.\n{err}\n"
                );
                success = false;
            }
        }
    }

    if let Some(messages) = crash {
        // The remaining files are not formatted: Dart's exception ends the
        // command.
        let _ = console.stderr.flush();
        return Err(messages.trim_end_matches('\n').to_string());
    }
    Ok(success)
}

/// Reads and formats [file] (the part of Dart `_processFile` in the `try`).
fn format_file(file: &str, formatter: &DartFormatter) -> Outcome {
    let text = match std::fs::read(file) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                return Outcome::ReadError(format!(
                    "FileSystemException: Failed to decode data using encoding 'utf-8', path = '{file}'"
                ));
            }
        },
        Err(err) => {
            return Outcome::ReadError(format!(
                "PathNotFoundException: Cannot open file, path = '{file}' ({err})"
            ));
        }
    };
    let source = SourceCode {
        uri: Some(file.to_string()),
        text,
        is_compilation_unit: true,
        selection_start: None,
        selection_length: None,
    };
    let start = Instant::now();
    match formatter.format_source(&source) {
        Ok(output) => Outcome::Formatted {
            source,
            output,
            elapsed: start.elapsed(),
        },
        Err(err) => Outcome::Failed(err),
    }
}
