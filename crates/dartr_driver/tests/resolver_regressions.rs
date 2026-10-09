//! Regression tests of the resolver on inputs that made it hang or crash.
//! Each test analyzes Dart source with `Driver::analyze_library`. The tests
//! need the Dart SDK (`dart` on `PATH`); without it they return early.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dartr_driver::driver::Driver;
use dartr_driver::file_state::{FileConfig, FileSystemState, SourceFactory};
use dartr_element::Generation;
use dartr_project::{DartSdk, Packages, Workspace};
use dartr_resolver::library_analyzer::ResolvedLibrary;
use dartr_resolver::options::AnalysisOptions;

/// Analyzes [source] as `<tmp>/resolver_regressions/<name>/lib.dart`.
fn analyze(name: &str, source: &str) -> Option<ResolvedLibrary> {
    let sdk_path = dartr_project::sdk::find_sdk_path()?;
    let sdk = DartSdk::new(&sdk_path);
    let version = sdk
        .language_version()
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("resolver_regressions")
        .join(name);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("lib.dart");
    std::fs::write(&path, source).expect("write source");
    let path = dartr_project::paths::normalize(path.to_str().expect("utf-8 path"));
    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::empty(), "/"),
        sdk: Some(sdk),
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    let mut driver = Driver::new(
        FileSystemState::new(source_factory, config_for),
        Arc::new(Generation::new(0)),
    );
    let file = driver.fs.get_file_for_path(&path);
    driver.fs.discover();
    driver.link_libraries(&[file]);
    driver.analyze_library(file, AnalysisOptions::default())
}

/// `X1 extends FutureOr<X1>` makes `isSubtypeOf(X1, Object)` recurse
/// without end (from `tests/language/least_upper_bound/
/// least_upper_bound_greatest_closure_2_test.dart`). The Dart analyzer
/// fails with a `StackOverflowError`; the port must stop quickly with a
/// caught panic for the unit, not run for hours.
#[test]
fn recursive_future_or_bound_stops_with_stack_overflow() {
    let source = r#"
import 'dart:async';

var condition = true;

void main() {
  void f6<X1 extends FutureOr<X1>>(X1 x1, FutureOr<Object> t2) {
    var z3 = condition ? x1 : t2;
  }
}
"#;
    let start = Instant::now();
    // The `dartr` binary runs the analysis on threads with 256 MiB stacks;
    // so does this test (the default test thread stack is too small for the
    // guarded recursion depth in a debug build).
    let library = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || analyze("recursive_future_or_bound", source))
        .expect("thread")
        .join()
        .expect("no panic outside the unit");
    let Some(library) = library else {
        return;
    };
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "took {:?}",
        start.elapsed()
    );
    let panic = library.units[0]
        .panic
        .clone()
        .expect("the unit stops with a panic");
    assert!(panic.contains("StackOverflowError"), "{panic}");
}
