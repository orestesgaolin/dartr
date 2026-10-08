//! Focused differential coverage for analysis-options include semantics.

use dartr_project::{AnalysisContextCollection, CollectionOptions, non_dart};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn dart() -> String {
    std::env::var("DARTR_DART").unwrap_or_else(|_| "dart".into())
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

#[test]
fn sibling_include_lint_conflict_matches_dart_analyze() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dartr-options-sibling-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let root = root.canonicalize().unwrap();
    write(&root.join("pubspec.yaml"), "name: sibling_conflict\n");
    write(
        &root.join("analysis_options.yaml"),
        "include: [a.yaml, b.yaml]\n",
    );
    write(
        &root.join("a.yaml"),
        "linter:\n  rules: [prefer_single_quotes]\n",
    );
    write(
        &root.join("b.yaml"),
        "linter:\n  rules: [prefer_double_quotes]\n",
    );
    write(&root.join("lib/main.dart"), "void main() {}\n");

    let dart = dart();
    let oracle_output = Command::new(&dart)
        .args(["analyze", "--format=json"])
        .arg(&root)
        .output()
        .expect("run the pinned Dart analyzer");
    assert!(
        matches!(oracle_output.status.code(), Some(0..=3)),
        "Dart failed: {}",
        String::from_utf8_lossy(&oracle_output.stderr)
    );
    let oracle_json: Value = serde_json::from_slice(&oracle_output.stdout).unwrap();
    let options_file = root.join("analysis_options.yaml");
    let options_path = options_file.to_string_lossy();
    let oracle: Vec<_> = oracle_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| diagnostic["location"]["file"] == options_path.as_ref())
        .map(|diagnostic| {
            let start = diagnostic["location"]["range"]["start"]["offset"]
                .as_u64()
                .unwrap();
            let end = diagnostic["location"]["range"]["end"]["offset"]
                .as_u64()
                .unwrap();
            json!({
                "file": diagnostic["location"]["file"],
                "code": diagnostic["code"],
                "severity": diagnostic["severity"],
                "offset": start,
                "length": end - start,
                "message": diagnostic["problemMessage"],
                "correction": diagnostic.get("correctionMessage").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();

    let collection = AnalysisContextCollection::new(
        &[root.to_string_lossy().into_owned()],
        &CollectionOptions {
            sdk_path: dartr_project::sdk::sdk_path_for_executable(&dart),
            ..Default::default()
        },
    );
    let actual = non_dart::collection_diagnostics_json(&collection);
    let actual: Vec<_> = actual
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| diagnostic["file"] == options_path.as_ref())
        .cloned()
        .collect();

    assert_eq!(actual, oracle);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0]["code"], "incompatible_lint");
    assert!(actual[0]["message"].as_str().unwrap().contains("b.yaml"));

    std::fs::remove_dir_all(root).unwrap();
}
