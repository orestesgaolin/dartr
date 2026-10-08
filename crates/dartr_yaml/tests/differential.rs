//! Differential coverage against package:yaml 3.1.4.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dartr_yaml::{
    CollectionStyle, NodeKind, Scalar, ScalarStyle, YamlException, YamlNode, load_yaml_node,
    load_yaml_node_with_options,
};
use serde_json::{Value, json};

fn location(location: &dartr_yaml::SourceLocation) -> Value {
    json!({
        "offset": location.offset,
        "line": location.line,
        "column": location.column,
    })
}

fn span(span: &dartr_yaml::FileSpan) -> Value {
    json!({"start": location(&span.start), "end": location(&span.end)})
}

fn scalar_style(style: ScalarStyle) -> &'static str {
    match style {
        ScalarStyle::Any => "ANY",
        ScalarStyle::Plain => "PLAIN",
        ScalarStyle::Literal => "LITERAL",
        ScalarStyle::Folded => "FOLDED",
        ScalarStyle::SingleQuoted => "SINGLE_QUOTED",
        ScalarStyle::DoubleQuoted => "DOUBLE_QUOTED",
    }
}

fn collection_style(style: CollectionStyle) -> &'static str {
    match style {
        CollectionStyle::Any => "ANY",
        CollectionStyle::Block => "BLOCK",
        CollectionStyle::Flow => "FLOW",
    }
}

fn node(yaml_node: &YamlNode) -> Value {
    match &yaml_node.kind {
        NodeKind::Scalar(value) => {
            let (scalar_type, value) = match value {
                Scalar::Null => ("null", Value::Null),
                Scalar::Bool(value) => ("bool", json!(value)),
                Scalar::Int(value) => ("int", json!(value)),
                Scalar::Float(value) if value.is_nan() => ("float", json!("nan")),
                Scalar::Float(value) if *value == f64::INFINITY => ("float", json!("+infinity")),
                Scalar::Float(value) if *value == f64::NEG_INFINITY => {
                    ("float", json!("-infinity"))
                }
                Scalar::Float(value) => ("float", json!(value)),
                Scalar::String(value) => ("string", json!(value)),
            };
            json!({
                "span": span(&yaml_node.span),
                "kind": "scalar",
                "scalarType": scalar_type,
                "value": value,
                "scalarStyle": scalar_style(yaml_node.scalar_style),
            })
        }
        NodeKind::List(items) => json!({
            "span": span(&yaml_node.span),
            "kind": "list",
            "collectionStyle": collection_style(yaml_node.collection_style),
            "items": items.iter().map(node).collect::<Vec<_>>(),
        }),
        NodeKind::Map(entries) => json!({
            "span": span(&yaml_node.span),
            "kind": "map",
            "collectionStyle": collection_style(yaml_node.collection_style),
            "entries": entries.iter().map(|(key, value)| {
                json!({"key": node(key), "value": node(value)})
            }).collect::<Vec<_>>(),
        }),
    }
}

fn error(error: &YamlException) -> Value {
    json!({"message": error.message, "span": span(&error.span)})
}

fn rust_dump(text: &str, recover: bool) -> Value {
    if recover {
        let result = load_yaml_node_with_options(text, true);
        json!({
            "node": result.node.as_ref().map(node),
            "errors": result.errors.iter().map(error).collect::<Vec<_>>(),
        })
    } else {
        match load_yaml_node(text) {
            Ok(parsed) => json!({"node": node(&parsed), "errors": []}),
            Err(exception) => json!({"node": null, "errors": [error(&exception)]}),
        }
    }
}

fn oracle_dumps(cases: &[(&str, bool)]) -> Vec<Value> {
    let dart = std::env::var_os("DARTR_DART")
        .or_else(|| std::env::var_os("DART"))
        .unwrap_or_else(|| "/Users/dominik/fvm/default/bin/cache/dart-sdk/bin/dart".into());
    let packages = std::env::var_os("DARTR_ORACLE_PACKAGES").unwrap_or_else(|| {
        "/Users/dominik/Projects/dartr/tools/oracle/.dart_tool/package_config.json".into()
    });
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/yaml_oracle.dart");
    let mut child = Command::new(dart)
        .arg(format!("--packages={}", PathBuf::from(packages).display()))
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start package:yaml oracle");
    let requests: Vec<Vec<u8>> = cases
        .iter()
        .map(|(text, recover)| {
            let mut line = serde_json::to_vec(&json!({"text": text, "recover": recover})).unwrap();
            line.push(b'\n');
            line
        })
        .collect();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        for request in requests {
            stdin.write_all(&request).unwrap();
        }
    });
    let lines: Vec<Value> = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect();
    let status = child.wait().unwrap();
    writer.join().unwrap();
    assert!(status.success(), "YAML oracle exited with {status}");
    assert_eq!(lines.len(), cases.len(), "oracle response count");
    lines
}

fn assert_parity(cases: &[(&str, bool)]) {
    let expected = oracle_dumps(cases);
    let mut failures = Vec::new();
    for ((text, recover), expected) in cases.iter().zip(expected) {
        let actual = rust_dump(text, *recover);
        if actual != expected {
            failures.push(format!(
                "YAML parity failed (recover={recover}) for:\n{text}\nactual: {actual}\nexpected: {expected}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn package_yaml_3_1_4_parity() {
    let cases = [
        ("", false),
        ("null", false),
        ("true", false),
        ("-42", false),
        ("[0.0, -0.0, 6.02e23, .nan, .inf, -.inf]", false),
        ("plain", false),
        ("'single '' quote'", false),
        (r#"\"double \\u263a\""#, false),
        ("|\n  literal\n  text\n", false),
        (">-\n  folded\n  text\n", false),
        ("[one, {two: 2}]", false),
        ("one:\n  two: [three, null]\nemoji: 😀\n", false),
        ("\u{feff}bom: yes\r\nnext: no\r\n", false),
        ("%YAML 1.2\n---\nvalue\n", false),
        ("--- one\n--- two\n", false),
        ("a:\n\tb: c\n", false),
        ("[one, two", false),
        ("{one: two", false),
        ("\"bad \\q escape\"", false),
        ("*missing", false),
        ("&anchor", false),
        ("!<unterminated value", false),
        ("%YAML 2.0\n--- value\n", false),
        (
            "%TAG !e! tag:one\n%TAG !e! tag:two\n--- !e!x value\n",
            false,
        ),
        ("dependencies:\n  one: any\n  two\n  three:\n  four\n", true),
        (
            "linter:\n  rules:\n    - annotate_overrides\n    alway\n",
            true,
        ),
        ("a: [", true),
    ];
    assert_parity(&cases);
}

fn collect_yaml_files(root: &Path, recursive: bool, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_yaml_files(&path, true, output);
        } else if path.is_file()
            && matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("yaml" | "yml")
            )
        {
            output.push(path);
        } else if path.is_dir() && !recursive {
            let pubspec = path.join("pubspec.yaml");
            if pubspec.is_file() {
                output.push(pubspec);
            }
        }
    }
}

#[test]
#[ignore = "real YAML corpus; run explicitly for release parity evidence"]
fn fvm_and_pub_cache_corpus_parity() {
    let home = std::env::var_os("HOME").expect("HOME");
    let mut paths = Vec::new();
    collect_yaml_files(
        Path::new(&home).join("fvm/default").as_path(),
        true,
        &mut paths,
    );
    collect_yaml_files(
        Path::new(&home).join(".pub-cache/hosted/pub.dev").as_path(),
        false,
        &mut paths,
    );
    paths.sort();
    paths.dedup();

    let owned: Vec<String> = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect();
    let cases: Vec<(&str, bool)> = owned.iter().map(|text| (text.as_str(), false)).collect();
    assert_parity(&cases);
    eprintln!("YAML corpus parity: {}/{} files", cases.len(), cases.len());
}
