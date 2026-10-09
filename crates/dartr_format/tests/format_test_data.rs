// Dart source: dart_style test/tall_format_test.dart, test/short_format_test.dart

//! Runs the formatting tests of dart_style (`third_party/dart_style/test`)
//! against the port.
//!
//! - `FORMAT_TEST_FILTER=<substring>`: only files whose path contains it.
//! - `FORMAT_TEST_VERBOSE=1`: print every failure (default: the first 20).
//! - `FORMAT_TEST_ALLOW_FAILURES=1`: print the summary but do not fail.

use std::path::PathBuf;

use dartr_format::testing::test_file::{TestFile, run_test_files};

fn test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/dart_style/test")
}

fn run(dirs: &[&str]) {
    let test_dir = test_dir();
    if !test_dir.exists() {
        eprintln!(
            "skipped: {} not found (run tools/fetch_sdk.sh)",
            test_dir.display()
        );
        return;
    }
    let filter = std::env::var("FORMAT_TEST_FILTER").ok();
    let verbose = std::env::var("FORMAT_TEST_VERBOSE").is_ok();
    let allow = std::env::var("FORMAT_TEST_ALLOW_FAILURES").is_ok();
    let mut files = Vec::new();
    for dir in dirs {
        files.extend(TestFile::list_directory(&test_dir, dir).unwrap());
    }
    if let Some(filter) = &filter {
        files.retain(|f| f.path.contains(filter.as_str()));
    }
    let (count, failures) = run_test_files(&files);
    for (i, failure) in failures.iter().enumerate() {
        if verbose || i < 20 {
            eprintln!(
                "FAIL {} {} at {}:\n{}\n",
                failure.file,
                failure.label,
                failure.version.major_minor(),
                failure.message
            );
        }
    }
    eprintln!(
        "{}: {} of {} tests passed ({} failed)",
        dirs.join(", "),
        count - failures.len(),
        count,
        failures.len()
    );
    if !allow {
        assert!(failures.is_empty(), "{} failures", failures.len());
    }
}

#[test]
fn tall() {
    run(&[
        "tall/declaration",
        "tall/expression",
        "tall/function",
        "tall/invocation",
        "tall/other",
        "tall/pattern",
        "tall/preserve_trailing_commas",
        "tall/statement",
        "tall/top_level",
        "tall/type",
        "tall/variable",
        "tall/regression",
    ]);
}

#[test]
fn short() {
    run(&[
        "short/comments",
        "short/regression",
        "short/selections",
        "short/splitting",
        "short/whitespace",
    ]);
}

// Dart source: dart_style lib/src/testing/benchmark.dart (Benchmark.read)

/// Formats the benchmark cases of dart_style (`benchmark/case/*.unit` and
/// `*.stmt`) in the tall style and compares with the `.expect` files.
#[test]
fn tall_benchmarks() {
    use dartr_format::{DartFormatter, SourceCode};

    let dir = test_dir().join("../benchmark/case");
    if !dir.exists() {
        eprintln!(
            "skipped: {} not found (run tools/fetch_sdk.sh)",
            dir.display()
        );
        return;
    }
    let filter = std::env::var("FORMAT_TEST_FILTER").ok();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("unit" | "stmt")
            )
        })
        .collect();
    paths.sort();
    let mut failures = Vec::new();
    let mut count = 0;
    for path in &paths {
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        if let Some(filter) = &filter {
            if !name.contains(filter.as_str()) {
                continue;
            }
        }
        count += 1;
        let content = std::fs::read_to_string(path).unwrap();
        // Dart `readAsLinesSync()` then `join('\n')`: drops the final line
        // terminator.
        let mut lines: Vec<&str> = content.lines().collect();
        // The first line may have a "|" to indicate the page width.
        let mut page_width = 80;
        if lines[0].ends_with('|') {
            page_width = lines[0].find('|').unwrap();
            lines.remove(0);
        }
        let input = lines.join("\n");
        let expected = std::fs::read_to_string(path.with_extension("expect")).unwrap();
        let mut formatter = DartFormatter::new(DartFormatter::LATEST_LANGUAGE_VERSION);
        formatter.page_width = page_width;
        let is_unit = path.extension().unwrap() == "unit";
        let source = SourceCode::new(input, None, is_unit, None, None).unwrap();
        match formatter.format_source(&source) {
            Ok(result) if result.text == expected => {}
            Ok(result) => failures.push(format!(
                "{name}: output differs\n--- expected\n{expected}\n--- actual\n{}",
                result.text
            )),
            Err(error) => failures.push(format!("{name}: error: {error}")),
        }
    }
    for failure in &failures {
        eprintln!("FAIL {failure}\n");
    }
    eprintln!(
        "tall benchmarks: {} of {} passed ({} failed)",
        count - failures.len(),
        count,
        failures.len()
    );
    if std::env::var("FORMAT_TEST_ALLOW_FAILURES").is_err() {
        assert!(failures.is_empty(), "{} failures", failures.len());
    }
}
