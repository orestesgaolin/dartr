//! Differential test of the project model: for each case, the JSON of
//! `tools/oracle/bin/contexts.dart` (the pinned `package:analyzer`) must be
//! equal to the JSON of `dartr_project::dump::collection_json`.
//!
//! The test needs `dart` on `PATH` (the SDK version of the oracle). Cases with
//! projects outside of the repository (Flutter, the SDK sources) are skipped
//! when the folders do not exist.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .canonicalize()
        .unwrap()
}

fn oracle_json(paths: &[String]) -> Value {
    let output = Command::new("dart")
        .current_dir(repo_root().join("tools/oracle"))
        .args(["run", "bin/contexts.dart"])
        .args(paths)
        .output()
        .expect("`dart` must be on PATH");
    assert!(
        output.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle writes JSON")
}

fn dartr_json(paths: &[String]) -> Value {
    let collection = dartr_project::AnalysisContextCollection::new(paths, &Default::default());
    dartr_project::dump::collection_json(&collection)
}

/// Collects the JSON paths where [a] and [b] differ.
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                match (x.get(key), y.get(key)) {
                    (Some(p), Some(q)) => differences(p, q, &format!("{path}.{key}"), out),
                    (p, q) => out.push(format!("{path}.{key}: oracle={p:?} dartr={q:?}")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!(
                    "{path}: oracle has {} items, dartr has {}",
                    x.len(),
                    y.len()
                ));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                differences(p, q, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path}: oracle={a} dartr={b}"));
            }
        }
    }
}

/// Compares the two outputs for [paths] and prints a parity line.
fn check(name: &str, paths: &[PathBuf]) {
    let paths: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let expected = oracle_json(&paths);
    let actual = dartr_json(&paths);
    let mut diffs = Vec::new();
    differences(&expected, &actual, "", &mut diffs);
    let contexts = expected["contexts"].as_array().map_or(0, Vec::len);
    let files: usize = expected["contexts"]
        .as_array()
        .map(|c| {
            c.iter()
                .map(|c| c["files"].as_array().map_or(0, Vec::len))
                .sum()
        })
        .unwrap_or(0);
    eprintln!(
        "parity {name}: {contexts} contexts, {files} files, {} differences",
        diffs.len()
    );
    assert!(
        diffs.is_empty(),
        "{name}: {} differences:\n{}",
        diffs.len(),
        diffs
            .iter()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The Flutter SDK root, if the Dart SDK on `PATH` is the one of a Flutter SDK.
fn flutter_root() -> Option<PathBuf> {
    let sdk = dartr_project::sdk::find_sdk_path()?;
    let root = Path::new(&sdk).parent()?.parent()?.parent()?.to_path_buf();
    root.join("packages/flutter").is_dir().then_some(root)
}

#[test]
fn fixture_nested_packages_excludes_and_options() {
    check("nested", &[fixtures().join("nested")]);
}

#[test]
fn fixture_nested_with_overlapping_included_paths() {
    let f = fixtures();
    check(
        "nested-overlap",
        &[
            f.join("nested/test"),
            f.join("nested/lib/a.dart"),
            f.join("nested"),
        ],
    );
}

#[test]
fn fixture_pub_workspace_root() {
    check("pub_workspace", &[fixtures().join("pub_workspace")]);
}

#[test]
fn fixture_pub_workspace_members() {
    let f = fixtures();
    check(
        "pub_workspace-members",
        &[
            f.join("pub_workspace/pkgs/a"),
            f.join("pub_workspace/pkgs/b"),
            f.join("pub_workspace/pkgs/c"),
        ],
    );
}

#[test]
fn fixture_includes_lists_cycles_missing_and_malformed() {
    check("includes", &[fixtures().join("includes")]);
}

#[test]
fn fixture_basic_files_and_missing_paths() {
    let f = fixtures();
    check(
        "basic",
        &[
            f.join("basic/bin/main.dart"),
            f.join("basic/sub"),
            f.join("basic/nope.dart"),
        ],
    );
}

#[test]
fn sdk_package_analyzer_without_workspace_root_package_config() {
    let path = repo_root().join("third_party/dart-sdk/pkg/analyzer");
    if !path.is_dir() {
        eprintln!("skip: {} does not exist", path.display());
        return;
    }
    check("pkg/analyzer", &[path]);
}

#[test]
fn flutter_package() {
    let Some(root) = flutter_root() else {
        eprintln!("skip: no Flutter SDK on PATH");
        return;
    };
    check("flutter/packages/flutter", &[root.join("packages/flutter")]);
}

#[test]
fn flutter_tools_package() {
    let Some(root) = flutter_root() else {
        eprintln!("skip: no Flutter SDK on PATH");
        return;
    };
    check(
        "flutter/packages/flutter_tools",
        &[root.join("packages/flutter_tools")],
    );
}
