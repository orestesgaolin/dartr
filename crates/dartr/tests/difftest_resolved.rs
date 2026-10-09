//! Runs the differential tests of the resolver (`difftest resolved-el` and
//! `difftest resolved`) end to end on the fixtures and on `dart:core`.
//! Needs `dart` on PATH (the oracle is compiled on first use).
//!
//! The resolver is not complete yet, so the tests do not require parity.
//! They check that the tooling works: the oracle and `dartr dump` write one
//! line per input (a resolver panic is reported in the line, it does not
//! stop the process), the oracle lists the nodes of every unit (also parts),
//! and the `dartr` output does not depend on the number of threads.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_difftest::{Options, collect_dart_files, run};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resolved")
}

fn inputs() -> Vec<PathBuf> {
    vec![fixtures(), PathBuf::from("dart:core")]
}

fn options(mode: &str) -> Options {
    Options {
        mode: mode.to_string(),
        inputs: inputs(),
        jobs: 2,
        dartr: PathBuf::from(env!("CARGO_BIN_EXE_dartr")),
        no_diagnostics: true,
        ..Default::default()
    }
}

/// Runs the difftest of [mode] and checks that both programs wrote a line
/// for every input and that the part file is reported as a part by both.
fn assert_runs(mode: &str, expected_kinds: &[&str]) -> BTreeMap<String, dartr_difftest::KindStats> {
    let report = run(&options(mode)).unwrap();
    let files = collect_dart_files(&inputs()).unwrap();
    assert_eq!(report.files, files.len());
    for d in &report.differences {
        assert!(
            !d.oracle.starts_with("<no output>") && !d.dartr.starts_with("<no output>"),
            "missing output for {}: {d:?}",
            d.file
        );
        assert!(
            !d.file.ends_with("generics_part.dart"),
            "the part file must give the same error line: {d:?}"
        );
    }
    for kind in expected_kinds {
        let k = report.kinds.get(*kind).copied().unwrap_or_default();
        assert!(
            k.oracle > 0,
            "no oracle entries of kind {kind}:\n{}",
            report.kind_report()
        );
    }
    println!("{}{}", report.kind_report(), report.summary());
    report.kinds
}

#[test]
fn resolved_el_runs_on_fixtures_and_dart_core() {
    let kinds = assert_runs(
        "resolved-el",
        &["SimpleIdentifier", "NamedType", "ConstructorName"],
    );
    // dart:core alone has more than 2000 named types (in its 37 units).
    assert!(kinds["NamedType"].oracle > 2000, "{kinds:?}");
}

#[test]
fn resolved_runs_on_fixtures_and_dart_core() {
    assert_runs(
        "resolved",
        &["IntegerLiteral", "MethodInvocation", "SimpleIdentifier"],
    );
}

/// The output of `dartr dump <mode>` with [threads] rayon threads.
fn dump(mode: &str, threads: &str) -> Vec<u8> {
    let files = collect_dart_files(&inputs()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_dartr"))
        .args(["dump", mode])
        .env("RAYON_NUM_THREADS", threads)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let input: String = files.iter().map(|f| format!("{f}\n")).collect();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout.iter().filter(|b| **b == b'\n').count(),
        files.len(),
        "one line per input"
    );
    output.stdout
}

/// Same output with 1 and 8 threads (units and libraries are resolved in
/// parallel).
#[test]
fn resolved_dumps_do_not_depend_on_threads() {
    for mode in ["resolved-el", "resolved"] {
        let one = dump(mode, "1");
        let many = dump(mode, "8");
        assert!(
            one == many,
            "mode {mode}: output differs between 1 and 8 threads"
        );
    }
}
