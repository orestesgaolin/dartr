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
