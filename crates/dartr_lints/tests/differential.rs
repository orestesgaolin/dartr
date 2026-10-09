// Dart source: pkg/linter/test/rules (differential integration fixtures)
use std::{path::Path, process::Command};

#[test]
fn lint_context_variants_match_dart_analyze() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (fixture, output, options) in [
        (
            "fixtures_experiments",
            "experiments",
            vec![
                "--language-version",
                "3.12",
                "--enable-experiment",
                "primary-constructors",
            ],
        ),
        (
            "fixtures_augmentations",
            "augmentations",
            vec!["--enable-experiment", "augmentations"],
        ),
        (
            "fixtures_file_uris",
            "file-uris",
            vec!["--no-package-config"],
        ),
    ] {
        let evidence = root.join("target/lints").join(output);
        let result = Command::new("python3")
            .arg(root.join("tools/lints_differential.py"))
            .arg("--input-dir")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(fixture),
            )
            .args(options)
            .arg("--binary")
            .arg(env!("CARGO_BIN_EXE_lints_dump"))
            .arg("--output")
            .arg(&evidence)
            .output()
            .expect("run Dart context differential test");
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        println!("{stdout}");
        assert!(
            result.status.success(),
            "Dart lint context parity failed:\n{stdout}\n{stderr}\nEvidence: {}",
            evidence.display()
        );
    }
}

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
