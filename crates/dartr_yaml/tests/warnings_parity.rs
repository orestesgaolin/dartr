//! Warning parity against package:yaml 3.1.4.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_yaml::{YamlWarning, load_yaml_node_with_options};
use serde_json::{Value, json};

fn location(location: &dartr_yaml::SourceLocation) -> Value {
    json!({"offset": location.offset, "line": location.line, "column": location.column})
}

fn warning(warning: &YamlWarning) -> Value {
    json!({
        "message": warning.message,
        "span": warning.span.as_ref().map(|span| {
            json!({"start": location(&span.start), "end": location(&span.end)})
        }),
    })
}

fn rust_result(text: &str, recover: bool) -> Value {
    let result = load_yaml_node_with_options(text, recover);
    let fatal = result
        .node
        .is_none()
        .then(|| result.errors.last().map(|error| error.message.clone()))
        .flatten();
    let recovered_errors = result.errors.len() - usize::from(fatal.is_some());
    json!({
        "warnings": result.warnings.iter().map(warning).collect::<Vec<_>>(),
        "recoveredErrors": recovered_errors,
        "fatal": fatal,
    })
}

fn oracle_results(cases: &[(&str, bool)]) -> Vec<Value> {
    let dart: OsString = std::env::var_os("DARTR_DART")
        .or_else(|| std::env::var_os("DART"))
        .unwrap_or_else(|| "dart".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packages = std::env::var_os("DARTR_ORACLE_PACKAGES")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("tools/oracle/.dart_tool/package_config.json"));
    let package_config = std::fs::read_to_string(&packages).expect("read oracle package_config");
    assert!(
        package_config.contains("yaml-3.1.4"),
        "warning oracle must resolve package:yaml 3.1.4"
    );
    let mut child = Command::new(dart)
        .arg(format!("--packages={}", packages.display()))
        .arg(root.join("tools/yaml_warnings_oracle.dart"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start warning oracle; set DARTR_DART if dart is not on PATH");
    let mut stdin = child.stdin.take().unwrap();
    for (text, recover) in cases {
        serde_json::to_writer(&mut stdin, &json!({"text": text, "recover": recover})).unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin);
    let results = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect::<Vec<_>>();
    let status = child.wait().unwrap();
    assert!(status.success(), "warning oracle exited with {status}");
    assert_eq!(results.len(), cases.len());
    results
}

#[test]
fn warnings_match_package_yaml() {
    let cases = [
        ("%FOO option\n--- value\n", false),
        ("%Y@ option\n--- value\n", false),
        ("%YAML1.3 option\n--- value\n", false),
        ("%YAML 1.3\n--- value\n", false),
        ("%FOO first\n--- one\n...\n%YAML 1.3\n--- two\n", false),
        ("%FOO option\n--- [unterminated\n", false),
        ("%FOO option\n--- [unterminated\n", true),
    ];
    let expected = oracle_results(&cases);
    for (((text, recover), expected), index) in cases.iter().zip(expected).zip(0..) {
        assert_eq!(
            rust_result(text, *recover),
            expected,
            "warning parity case {index}, recover={recover}, input={text:?}"
        );
    }
}
