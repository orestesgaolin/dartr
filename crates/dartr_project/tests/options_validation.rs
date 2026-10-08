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

fn options_diagnostics(root: &Path) -> (Vec<Value>, Vec<Value>) {
    let dart = dart();
    let oracle_output = Command::new(&dart)
        .args(["analyze", "--format=json"])
        .arg(root)
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
    let oracle = oracle_json["diagnostics"]
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
    let actual = actual
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| diagnostic["file"] == options_path.as_ref())
        .cloned()
        .collect();
    (oracle, actual)
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

    let (oracle, actual) = options_diagnostics(&root);

    assert_eq!(actual, oracle);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0]["code"], "incompatible_lint");
    assert!(actual[0]["message"].as_str().unwrap().contains("b.yaml"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn incompatible_rule_names_follow_rule_metadata_order() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dartr-options-rule-order-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let root = root.canonicalize().unwrap();
    write(&root.join("pubspec.yaml"), "name: rule_order\n");
    write(
        &root.join("analysis_options.yaml"),
        "linter:\n  rules: [omit_local_variable_types, avoid_types_on_closure_parameters, always_specify_types]\n",
    );
    write(&root.join("lib/main.dart"), "void main() {}\n");

    let (oracle, actual) = options_diagnostics(&root);
    assert_eq!(actual, oracle);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0]["code"], "incompatible_lint");
    assert!(
        actual[0]["message"]
            .as_str()
            .unwrap()
            .contains("''omit_local_variable_types''")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_enabled_rules_override_disabled_canonical_duplicates() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dartr-options-local-override-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let root = root.canonicalize().unwrap();
    write(&root.join("pubspec.yaml"), "name: local_override\n");
    write(
        &root.join("analysis_options.yaml"),
        "include: child.yaml\nlinter:\n  rules: [prefer_double_quotes]\n",
    );
    write(
        &root.join("child.yaml"),
        "linter:\n  rules:\n    PREFER_SINGLE_QUOTES: true\n    prefer_single_quotes: false\n",
    );
    write(&root.join("lib/main.dart"), "void main() {}\n");

    let (oracle, actual) = options_diagnostics(&root);
    assert_eq!(actual, oracle);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0]["code"], "incompatible_lint");
    assert!(
        actual[0]["message"]
            .as_str()
            .unwrap()
            .contains("'PREFER_SINGLE_QUOTES'")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_constraint_lower_bound_shapes_match_dart_analyze() {
    for (index, (constraint, expected_count)) in [
        ("3.13.0", 1),
        (">= 3.13.0", 1),
        ("<4.0.0 >=3.13.0", 1),
        (">=3.12.0 >=3.13.0 <4.0.0", 1),
        ("^3.13.0 junk", 0),
        ("^3.13.0-foo+", 0),
        (">=3.13.0 <=3.12.0", 0),
        ("any", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dartr-options-constraint-{index}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let root = root.canonicalize().unwrap();
        write(
            &root.join("pubspec.yaml"),
            &format!("name: constraint_fixture\nenvironment:\n  sdk: '{constraint}'\n"),
        );
        write(
            &root.join(".dart_tool/package_config.json"),
            r#"{
  "configVersion": 2,
  "packages": [{
    "name": "constraint_fixture",
    "rootUri": "../",
    "packageUri": "lib/",
    "languageVersion": "3.13"
  }]
}
"#,
        );
        write(
            &root.join("analysis_options.yaml"),
            "linter:\n  rules: [always_require_non_null_named_parameters]\n",
        );
        write(&root.join("lib/main.dart"), "void main() {}\n");

        let (oracle, actual) = options_diagnostics(&root);
        assert_eq!(actual, oracle, "constraint: {constraint}");
        assert_eq!(actual.len(), expected_count, "constraint: {constraint}");
        if expected_count != 0 {
            assert_eq!(actual[0]["code"], "removed_lint");
        }

        std::fs::remove_dir_all(root).unwrap();
    }
}
