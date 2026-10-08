//! Directed parity coverage for YAML document metadata and streams.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_yaml::{
    NodeKind, Scalar, YamlDocument, YamlException, YamlNode, load_yaml_document,
    load_yaml_documents, load_yaml_stream,
};
use serde_json::{Value, json};

fn location(location: &dartr_yaml::SourceLocation) -> Value {
    json!({"offset": location.offset, "line": location.line, "column": location.column})
}

fn span(span: &dartr_yaml::FileSpan) -> Value {
    json!({"start": location(&span.start), "end": location(&span.end)})
}

fn value(node: &YamlNode) -> Value {
    match &node.kind {
        NodeKind::Scalar(Scalar::Null) => Value::Null,
        NodeKind::Scalar(Scalar::Bool(value)) => json!(value),
        NodeKind::Scalar(Scalar::Int(value)) => json!(value),
        NodeKind::Scalar(Scalar::Float(value)) => json!(value),
        NodeKind::Scalar(Scalar::String(value)) => json!(value),
        NodeKind::List(items) => Value::Array(items.iter().map(value).collect()),
        NodeKind::Map(entries) => Value::Array(
            entries
                .iter()
                .map(|(key, value_node)| json!({"key": value(key), "value": value(value_node)}))
                .collect(),
        ),
    }
}

fn document(document: &YamlDocument) -> Value {
    json!({
        "contents": value(&document.contents),
        "span": span(&document.span),
        "version": document.version_directive.as_ref().map(|version| {
            json!({"major": version.major, "minor": version.minor})
        }),
        "tags": document.tag_directives.iter().map(|tag| {
            json!({"handle": tag.handle, "prefix": tag.prefix})
        }).collect::<Vec<_>>(),
        "startImplicit": document.start_implicit,
        "endImplicit": document.end_implicit,
    })
}

fn exception(error: &YamlException) -> Value {
    json!({"message": error.message, "span": span(&error.span)})
}

fn rust_result(text: &str, operation: &str) -> Value {
    let result = match operation {
        "document" => load_yaml_document(text).map(|result| document(&result)),
        "documents" => load_yaml_documents(text)
            .map(|results| Value::Array(results.iter().map(document).collect())),
        "stream" => load_yaml_stream(text)
            .map(|result| json!({"contents": value(&result), "span": span(&result.span)})),
        _ => unreachable!(),
    };
    match result {
        Ok(result) => json!({"result": result, "error": null}),
        Err(error) => json!({"result": null, "error": exception(&error)}),
    }
}

fn oracle_results(cases: &[(&str, &str)]) -> Vec<Value> {
    let dart = std::env::var_os("DARTR_DART")
        .or_else(|| std::env::var_os("DART"))
        .unwrap_or_else(|| "dart".into());
    let packages = std::env::var_os("DARTR_ORACLE_PACKAGES").unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/oracle/.dart_tool/package_config.json")
            .into_os_string()
    });
    let package_config = std::fs::read_to_string(&packages).expect("read oracle package_config");
    assert!(package_config.contains("yaml-3.1.4"));
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/yaml_documents_oracle.dart");
    let mut child = Command::new(dart)
        .arg(format!("--packages={}", PathBuf::from(packages).display()))
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start document oracle");
    let mut stdin = child.stdin.take().unwrap();
    for (text, operation) in cases {
        serde_json::to_writer(&mut stdin, &json!({"text": text, "operation": operation})).unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin);
    let results = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect::<Vec<_>>();
    let status = child.wait().unwrap();
    assert!(status.success(), "document oracle exited with {status}");
    assert_eq!(results.len(), cases.len());
    results
}

#[test]
fn document_and_stream_metadata_match_package_yaml() {
    let inputs = [
        "",
        "# comment only\n",
        "plain\n",
        "--- explicit\n...\n",
        "%YAML 1.2\n---\nvalue\n...\n",
        "%TAG !e! tag:example.com,2026:\n--- !e!thing value\n",
        "--- one\n...\n--- two\n",
        "--- emoji: 😀\r\n...\r\n",
    ];
    let mut cases = Vec::new();
    for input in inputs {
        cases.push((input, "documents"));
        cases.push((input, "stream"));
    }
    cases.push(("--- one\n--- two\n", "document"));
    cases.push(("%YAML 1.2\n---\nvalue\n...\n", "document"));

    let expected = oracle_results(&cases);
    for (((text, operation), expected), index) in cases.iter().zip(expected).zip(0..) {
        assert_eq!(
            rust_result(text, operation),
            expected,
            "document parity case {index}, operation={operation}, input={text:?}"
        );
    }
    eprintln!("document metadata parity: {0}/{0} comparisons", cases.len());
}
