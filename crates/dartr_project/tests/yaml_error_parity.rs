//! Differential integration coverage for package:yaml syntax errors.
//! The oracle uses the pinned package:yaml 3.1.4 from tools/oracle.
//! These cases validate messages and UTF-16 source ranges through the
//! analyzer's byte-span adapter. Broader node/recovery and corpus parity
//! lives in dartr_yaml/tests/differential.rs.

use dartr_project::yaml::load_yaml_node;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Case {
    name: &'static str,
    input: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "hex_escape_non_bmp",
        input: "name: \"\\x😀\"\n",
    },
    Case {
        name: "yaml_major_incompatible",
        input: "%YAML 2.0\n---\nanalyzer: {}\n",
    },
    Case {
        name: "yaml_zero_minor",
        input: "%YAML 1.0\n---\nanalyzer: {}\n",
    },
    Case {
        name: "yaml_duplicate",
        input: "%YAML 1.2\n%YAML 1.2\n---\nanalyzer: {}\n",
    },
    Case {
        name: "mapping_missing_colon",
        input: "analyzer:\n  language\n  exclude: []\n",
    },
    Case {
        name: "node_content",
        input: "name: [",
    },
    Case {
        name: "double_quote_eof",
        input: "name: \"unterminated",
    },
    Case {
        name: "single_quote_eof",
        input: "name: 'unterminated",
    },
    Case {
        name: "flow_mapping_comma",
        input: "{one: 1 two: 2}",
    },
    Case {
        name: "flow_sequence_comma",
        input: "[one, {a: b c: d}]",
    },
    Case {
        name: "block_mapping_key",
        input: "name:\n  child: value\n bad: indent\n",
    },
    Case {
        name: "missing_colon",
        input: "name: value\nother\n",
    },
    Case {
        name: "tab_indent",
        input: "name:\n\tchild: value\n",
    },
    Case {
        name: "tab_plain",
        input: "name: one\t two\n",
    },
    Case {
        name: "unknown_alias",
        input: "*missing\n",
    },
    Case {
        name: "empty_alias",
        input: "*\n",
    },
    Case {
        name: "empty_anchor",
        input: "&\n",
    },
    Case {
        name: "directive_empty",
        input: "%\n---\nx\n",
    },
    Case {
        name: "yaml_no_version",
        input: "%YAML\n---\nx\n",
    },
    Case {
        name: "yaml_no_dot",
        input: "%YAML 1\n---\nx\n",
    },
    Case {
        name: "yaml_bad_minor",
        input: "%YAML 1.x\n---\nx\n",
    },
    Case {
        name: "yaml_trailing",
        input: "%YAML 1.2 junk\n---\nx\n",
    },
    Case {
        name: "tag_empty",
        input: "%TAG\n---\nx\n",
    },
    Case {
        name: "tag_no_prefix",
        input: "%TAG !foo!\n---\nx\n",
    },
    Case {
        name: "tag_trailing",
        input: "%TAG !foo! tag:foo junk\n---\nx\n",
    },
    Case {
        name: "unknown_tag",
        input: "!foo!bar value\n",
    },
    Case {
        name: "unknown_escape",
        input: "name: \"\\q\"\n",
    },
    Case {
        name: "hex_short",
        input: "name: \"\\x1\"\n",
    },
    Case {
        name: "unicode_short",
        input: "name: \"\\u12\"\n",
    },
    Case {
        name: "unicode_nonhex",
        input: "name: \"\\uZ000\"\n",
    },
    Case {
        name: "unicode_surrogate",
        input: "name: \"\\uD800\"\n",
    },
    Case {
        name: "unicode_large",
        input: "name: \"\\U00110000\"\n",
    },
    Case {
        name: "quote_doc_indicator",
        input: "name: \"a\n---\nb\"\n",
    },
    Case {
        name: "block_scalar_zero",
        input: "name: |0\n  value\n",
    },
    Case {
        name: "unexpected_at",
        input: "@value\n",
    },
    Case {
        name: "misplaced_bracket",
        input: "]\n",
    },
    Case {
        name: "wrong_flow_close",
        input: "{a: b]\n",
    },
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn dart() -> String {
    std::env::var("DARTR_DART").unwrap_or_else(|_| "dart".into())
}

fn oracle() -> Vec<Value> {
    let repo = repo_root();
    let packages = std::env::var("DARTR_ORACLE_PACKAGES").unwrap_or_else(|_| {
        repo.join("tools/oracle/.dart_tool/package_config.json")
            .to_string_lossy()
            .into_owned()
    });
    assert!(
        Path::new(&packages).is_file(),
        "Run dart pub get in tools/oracle, or set DARTR_ORACLE_PACKAGES"
    );

    let version = Command::new(dart())
        .arg("--version")
        .output()
        .expect("Dart is required for the package:yaml oracle");
    let version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        version.status.success() && version_text.contains("3.13.3"),
        "Expected pinned Dart 3.13.3: {version_text}"
    );

    let mut child = Command::new(dart())
        .arg(format!("--packages={packages}"))
        .arg(repo.join("tools/oracle/bin/yaml_errors.dart"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run package:yaml oracle");
    let inputs: Vec<_> = CASES.iter().map(|case| case.input).collect();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&inputs).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "package:yaml oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "package:yaml oracle returned invalid JSON: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn actual(case: &Case) -> Value {
    match load_yaml_node(case.input) {
        Ok(_) => Value::Null,
        Err(error) => {
            let Some(span) = error.span else {
                return json!({
                    "message": error.message,
                    "offset": null,
                    "length": null,
                    "line": null,
                    "column": null,
                });
            };
            let (offset, length) = error.utf16_range(case.input).unwrap();
            json!({
                "message": error.message,
                "offset": offset,
                "length": length,
                "line": span.line,
                "column": span.column,
            })
        }
    }
}

#[test]
fn malformed_yaml_matches_package_yaml() {
    let expected = oracle();
    assert_eq!(expected.len(), CASES.len());

    let mut differences = Vec::new();
    for (index, case) in CASES.iter().enumerate() {
        let actual = actual(case);
        if actual != expected[index] {
            differences.push(format!(
                "{}: oracle={} dartr={actual}",
                case.name, expected[index]
            ));
        }
    }

    eprintln!(
        "yaml error parity: {} cases, {} differences",
        CASES.len(),
        differences.len()
    );
    assert!(
        differences.is_empty(),
        "{} YAML error differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}
