// Dart source: pkg/linter/test/rules/{avoid_void_async,always_put_required_named_parameters_first,avoid_bool_literals_in_conditional_expressions}_test.dart
//! Exercises the production driver, resolution, parts, ignore filtering and UTF-16 spans.
use std::{path::Path, process::Command};
#[test]
fn resolved_library_lints_match_pinned_analyzer() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap().parent().unwrap();
    let fixture = manifest.join("tests/fixtures/resolved_lints");
    let output = root.join("target/lints/driver-integration");
    let result = Command::new("python3")
        .arg(root.join("tools/lints_differential.py"))
        .arg("--input-dir")
        .arg(fixture.join("lib"))
        .arg("--rules-file")
        .arg(fixture.join("rules.txt"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_lints_resolved_dump"))
        .arg("--output")
        .arg(&output)
        .arg("--timeout")
        .arg("60")
        .env("RAYON_NUM_THREADS", "2")
        .output()
        .expect("run resolved lint differential");
    assert!(
        result.status.success(),
        "{}\n{}\nEvidence: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        output.display()
    );
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("summary.json")).unwrap())
            .unwrap();
    assert_eq!(
        summary["oracle"], 5,
        "positive and clean cases must execute"
    );
}
