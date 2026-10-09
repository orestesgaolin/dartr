// Dart source: dart_style lib/src/testing/benchmark.dart (Benchmark.read)
// Dart source: dart_style benchmark/run.dart (_runTrials with --short)

//! Formats the benchmark cases of dart_style
//! (`third_party/dart_style/benchmark/case/*.unit`) in the short style and
//! compares the result with the `.expect_short` files.
//!
//! - `FORMAT_BENCHMARK_FILTER=<substring>`: only cases whose name contains it.
//! - `FORMAT_BENCHMARK_TIMES=<n>`: format each case n times and print the
//!   median time (use with `--release`).

use std::path::PathBuf;
use std::time::Instant;

use dartr_format::{DartFormatter, SourceCode};

struct Benchmark {
    name: String,
    input: String,
    page_width: usize,
    short_output: String,
}

/// Dart `File.readAsLinesSync()`: splits at `\n` and `\r\n`; a final line
/// terminator does not start a new line.
fn read_as_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

/// Dart `Benchmark.read`.
fn read(path: &std::path::Path) -> Benchmark {
    let mut input_lines = read_as_lines(&std::fs::read_to_string(path).unwrap());

    // The first line may have a "|" to indicate the page width.
    let mut page_width = 80;
    if input_lines[0].ends_with('|') {
        page_width = input_lines[0].find('|').unwrap();
        input_lines.remove(0);
    }

    let input = input_lines.join("\n");
    let short_output = std::fs::read_to_string(path.with_extension("expect_short")).unwrap();

    Benchmark {
        name: path.file_stem().unwrap().to_string_lossy().into_owned(),
        input,
        page_width,
        short_output,
    }
}

#[test]
fn short_benchmarks() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/dart_style/benchmark/case");
    if !dir.exists() {
        eprintln!("skipped: {} not found (run tools/fetch_sdk.sh)", dir.display());
        return;
    }
    let filter = std::env::var("FORMAT_BENCHMARK_FILTER").ok();
    let times: usize = std::env::var("FORMAT_BENCHMARK_TIMES")
        .ok()
        .and_then(|times| times.parse().ok())
        .unwrap_or(1);

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("unit") | Some("stmt")
            )
        })
        .collect();
    paths.sort();

    let mut count = 0;
    let mut failures = Vec::new();
    for path in paths {
        let benchmark = read(&path);
        if filter
            .as_ref()
            .is_some_and(|filter| !benchmark.name.contains(filter.as_str()))
        {
            continue;
        }
        count += 1;

        let mut formatter = DartFormatter::new(DartFormatter::LATEST_SHORT_STYLE_LANGUAGE_VERSION);
        formatter.page_width = benchmark.page_width;
        formatter.line_ending = Some("\n".to_string());

        // Dart `benchmark/run.dart` formats the parsed unit as a compilation
        // unit.
        let source = SourceCode::unit(benchmark.input.clone());
        let mut durations = Vec::new();
        let mut result = None;
        for _ in 0..times {
            let start = Instant::now();
            result = Some(formatter.format_source(&source));
            durations.push(start.elapsed());
        }
        durations.sort();
        if times > 1 {
            eprintln!(
                "{}: median {:.3} ms min {:.3} ms",
                benchmark.name,
                durations[durations.len() / 2].as_secs_f64() * 1000.0,
                durations[0].as_secs_f64() * 1000.0
            );
        }

        match result.unwrap() {
            Ok(output) if output.text == benchmark.short_output => {}
            Ok(output) => failures.push(format!(
                "{}: output differs\n--- expected\n{}\n--- actual\n{}",
                benchmark.name, benchmark.short_output, output.text
            )),
            Err(error) => failures.push(format!("{}: error: {error}", benchmark.name)),
        }
    }

    for failure in &failures {
        eprintln!("FAIL {failure}\n");
    }
    eprintln!(
        "short benchmarks: {} of {} passed",
        count - failures.len(),
        count
    );
    assert!(failures.is_empty(), "{} failures", failures.len());
}
