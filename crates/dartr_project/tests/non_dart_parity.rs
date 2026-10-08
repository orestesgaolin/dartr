//! Differential integration tests against Dart 3.13.3 `dart analyze --format=json`.
//! Reference dispatch: `pkg/analysis_server/lib/src/context_manager.dart`.

use dartr_project::{AnalysisContextCollection, CollectionOptions, FileKind, non_dart};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

fn dart() -> String {
    std::env::var("DARTR_DART").unwrap_or_else(|_| "dart".into())
}

fn oracle(path: &Path) -> Vec<Value> {
    let version = Command::new(dart())
        .arg("--version")
        .output()
        .expect("Dart is required");
    let version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        version.status.success() && version_text.contains("3.13.3"),
        "Expected pinned Dart 3.13.3: {version_text}"
    );
    let output = Command::new(dart())
        .args(["analyze", "--format=json"])
        .arg(path)
        .output()
        .expect("run Dart analyzer");
    let parsed: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "Dart oracle failed: {error}; status={}; stdout={}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert!(
        matches!(output.status.code(), Some(0..=3)),
        "Dart exited unexpectedly: {}",
        output.status
    );
    let mut diagnostics: Vec<Value> = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| {
            matches!(
                FileKind::of(d["location"]["file"].as_str().unwrap()),
                FileKind::AnalysisOptions | FileKind::Pubspec | FileKind::AndroidManifest
            )
        })
        .map(|d| {
            let start = d["location"]["range"]["start"]["offset"].as_u64().unwrap();
            let end = d["location"]["range"]["end"]["offset"].as_u64().unwrap();
            json!({"file":d["location"]["file"], "code":d["code"], "severity":d["severity"],
                "offset":start, "length":end-start, "message":d["problemMessage"],
                "correction":d.get("correctionMessage").cloned().unwrap_or(Value::Null)})
        })
        .collect();
    non_dart::sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn compare(path: &Path) -> (usize, usize, Vec<String>) {
    let path = path.canonicalize().unwrap();
    let options = CollectionOptions {
        sdk_path: dartr_project::sdk::sdk_path_for_executable(&dart()),
        ..Default::default()
    };
    let collection =
        AnalysisContextCollection::new(&[path.to_string_lossy().into_owned()], &options);
    let actual = non_dart::collection_cli_diagnostics_json(&collection);
    let expected = oracle(&path);
    let actual = actual.as_array().unwrap();
    let files = non_dart::analyzed_files(&collection);
    let mut differing_files = Vec::new();
    let mut all_files = files.clone();
    all_files.extend(
        expected
            .iter()
            .map(|d| d["file"].as_str().unwrap().to_owned()),
    );
    all_files.sort();
    all_files.dedup();
    for file in &all_files {
        let a: Vec<_> = actual.iter().filter(|d| d["file"] == *file).collect();
        let e: Vec<_> = expected.iter().filter(|d| d["file"] == *file).collect();
        if a != e {
            differing_files.push(format!(
                "{file}\n  oracle={}\n  dartr={}",
                json!(e),
                json!(a)
            ));
        }
    }
    eprintln!(
        "parity {}: {} non-Dart files, oracle={} diagnostics, dartr={} diagnostics, {} differing files ({:.2}% file parity)",
        path.display(),
        all_files.len(),
        expected.len(),
        actual.len(),
        differing_files.len(),
        if all_files.is_empty() {
            100.0
        } else {
            100.0 * (all_files.len() - differing_files.len()) as f64 / all_files.len() as f64
        }
    );
    if let Ok(directory) = std::env::var("DARTR_PARITY_EVIDENCE") {
        std::fs::create_dir_all(&directory).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        std::fs::write(
            Path::new(&directory).join(format!("{name}-oracle.json")),
            serde_json::to_string_pretty(&expected).unwrap(),
        )
        .unwrap();
        std::fs::write(
            Path::new(&directory).join(format!("{name}-dartr.json")),
            serde_json::to_string_pretty(actual).unwrap(),
        )
        .unwrap();
        std::fs::write(
            Path::new(&directory).join(format!("{name}-differences.txt")),
            differing_files.join("\n"),
        )
        .unwrap();
    }
    (all_files.len(), expected.len(), differing_files)
}

fn check(path: &Path) {
    let (_, _, differences) = compare(path);
    assert!(
        differences.is_empty(),
        "{} differing files:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

#[test]
fn broken_fixture_projects_match_dart_analyze() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics");
    let mut directories: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    directories.sort();
    let mut total = (0, 0);
    let mut differences = Vec::new();
    for directory in &directories {
        let counts = compare(directory);
        differences.extend(counts.2);
        total.0 += counts.0;
        total.1 += counts.1;
    }
    assert!(
        differences.is_empty(),
        "{} differing files:\n{}",
        differences.len(),
        differences.join("\n")
    );
    eprintln!(
        "fixture parity: {} projects, {} files, {} diagnostics, 100.00%",
        directories.len(),
        total.0,
        total.1
    );
}

/// The CLI hides priority ERRORs. Compare the server/parser APIs separately
/// so malformed YAML and error severity overrides are still checked.
#[test]
fn raw_server_diagnostics_match_pinned_analyzer() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics");
    let mut paths: Vec<String> = std::fs::read_dir(fixtures)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .map(|p| p.canonicalize().unwrap().to_string_lossy().into_owned())
        .collect();
    paths.sort();
    let packages = std::env::var("DARTR_ORACLE_PACKAGES").unwrap_or_else(|_| {
        repo.join("tools/oracle/.dart_tool/package_config.json")
            .to_string_lossy()
            .into_owned()
    });
    assert!(
        Path::new(&packages).is_file(),
        "Run dart pub get in tools/oracle, or set DARTR_ORACLE_PACKAGES to the pinned oracle package config"
    );
    let output = Command::new(dart())
        .arg(format!("--packages={packages}"))
        .arg(repo.join("tools/oracle/bin/non_dart.dart"))
        .args(&paths)
        .output()
        .expect("run pinned raw analyzer oracle");
    assert!(
        output.status.success(),
        "raw oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut expected: Vec<Value> =
        serde_json::from_slice(&output.stdout).expect("raw oracle JSON array");
    non_dart::sort_diagnostics(&mut expected);
    let options = CollectionOptions {
        sdk_path: dartr_project::sdk::sdk_path_for_executable(&dart()),
        ..Default::default()
    };
    let collection = AnalysisContextCollection::new(&paths, &options);
    let actual = non_dart::collection_diagnostics_json(&collection);
    let actual = actual.as_array().unwrap();
    let mut differences = Vec::new();
    let mut files = non_dart::analyzed_files(&collection);
    files.extend(
        expected
            .iter()
            .map(|d| d["file"].as_str().unwrap().to_owned()),
    );
    files.sort();
    files.dedup();
    for file in &files {
        let a: Vec<_> = actual.iter().filter(|d| d["file"] == *file).collect();
        let e: Vec<_> = expected.iter().filter(|d| d["file"] == *file).collect();
        if a != e {
            differences.push(format!(
                "{file}\n  oracle={}\n  dartr={}",
                json!(e),
                json!(a)
            ));
        }
    }
    eprintln!(
        "raw server parity: {} projects, {} files, oracle={} diagnostics, dartr={} diagnostics, {} differing files",
        paths.len(),
        files.len(),
        expected.len(),
        actual.len(),
        differences.len()
    );
    if let Ok(directory) = std::env::var("DARTR_PARITY_EVIDENCE") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            Path::new(&directory).join("raw-oracle.json"),
            serde_json::to_string_pretty(&expected).unwrap(),
        )
        .unwrap();
        std::fs::write(
            Path::new(&directory).join("raw-dartr.json"),
            serde_json::to_string_pretty(actual).unwrap(),
        )
        .unwrap();
        std::fs::write(
            Path::new(&directory).join("raw-differences.txt"),
            differences.join("\n"),
        )
        .unwrap();
    }
    assert!(
        differences.is_empty(),
        "{} raw differing files:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

#[test]
#[ignore = "whole Flutter repository; run explicitly"]
fn flutter_repository_matches_dart_analyze() {
    let path = std::env::var("DARTR_FLUTTER_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join("fvm/default"));
    check(&path);
}

#[test]
#[ignore = "external read-only corpus; run explicitly"]
fn visible_repository_matches_dart_analyze() {
    let path = std::env::var("DARTR_VISIBLE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/Users/dominik/Projects/dartr/bench/corpus/visible-app")
        });
    check(&path);
}
