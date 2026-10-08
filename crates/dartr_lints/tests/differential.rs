// Dart source: pkg/linter/test/rules (differential integration fixtures)
use std::{path::Path, process::Command};

#[test]
fn lint_diagnostics_match_dart_analyze() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = root.join("target/lints/fixtures");
    let result = Command::new("python3")
        .arg(root.join("tools/lints_differential.py"))
        .arg("--fixtures")
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_lints_dump"))
        .arg("--output")
        .arg(&output)
        .output()
        .expect("run Dart differential test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("{stdout}");
    assert!(
        result.status.success(),
        "Dart lint parity failed:\n{stdout}\n{stderr}\nEvidence: {}",
        output.display()
    );
}

#[test]
fn lint_registry_matches_pinned_analyzer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = root.join("target/lints/metadata");
    let result = Command::new("python3")
        .arg(root.join("tools/lints_metadata_differential.py"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_lints_dump"))
        .arg("--output")
        .arg(&output)
        .output()
        .expect("run Dart registry comparison");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("{stdout}");
    assert!(
        result.status.success(),
        "Dart lint registry parity failed:\n{stdout}\n{stderr}\nEvidence: {}",
        output.display()
    );
}
