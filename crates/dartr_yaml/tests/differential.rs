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
        if let Some(runtime_error) = result
            .errors
            .iter()
            .find_map(|error| error.runtime_error.as_ref())
        {
            return json!({
                "node": null,
                "errors": result.errors.iter()
                    .filter(|error| error.runtime_error.is_none())
                    .map(error).collect::<Vec<_>>(),
                "runtimeError": runtime_error,
            });
        }
        json!({
            "node": result.node.as_ref().map(node),
            "errors": result.errors.iter().map(error).collect::<Vec<_>>(),
        })
    } else {
        match load_yaml_node(text) {
            Ok(parsed) => json!({"node": node(&parsed), "errors": []}),
            Err(exception) => match &exception.runtime_error {
                Some(runtime_error) => {
                    json!({"node": null, "errors": [], "runtimeError": runtime_error})
                }
                None => json!({"node": null, "errors": [error(&exception)]}),
            },
        }
    }
}

fn oracle_dumps(cases: &[(&str, bool)]) -> Vec<Value> {
    let dart = std::env::var_os("DARTR_DART")
        .or_else(|| std::env::var_os("DART"))
        .unwrap_or_else(|| "dart".into());
    let packages = std::env::var_os("DARTR_ORACLE_PACKAGES").unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/oracle/.dart_tool/package_config.json")
            .into_os_string()
    });
    let package_config = std::fs::read_to_string(&packages).unwrap_or_else(|error| {
        panic!(
            "read oracle package_config {}: {error}; run dart pub get in tools/oracle or set DARTR_ORACLE_PACKAGES",
            PathBuf::from(&packages).display()
        )
    });
    assert!(
        package_config.contains("yaml-3.1.4"),
        "YAML oracle must resolve package:yaml 3.1.4"
    );
    let version = Command::new(&dart)
        .arg("--version")
        .output()
        .expect("run Dart YAML oracle --version");
    let version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        version.status.success() && version_text.contains("3.13.3"),
        "YAML oracle requires Dart 3.13.3, got: {version_text}"
    );
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
        .map(|line| {
            let line = line.unwrap();
            serde_json::from_str(&line).unwrap_or_else(|error| {
                panic!("YAML oracle returned non-JSON output {line:?}: {error}")
            })
        })
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

fn package_yaml_test_inputs() -> Vec<String> {
    let dart = std::env::var_os("DARTR_DART")
        .or_else(|| std::env::var_os("DART"))
        .unwrap_or_else(|| "dart".into());
    let packages = std::env::var_os("DARTR_ORACLE_PACKAGES").unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/oracle/.dart_tool/package_config.json")
            .into_os_string()
    });
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(&packages).expect("read oracle package_config"),
    )
    .unwrap();
    let root_uri = config["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "yaml")
        .and_then(|package| package["rootUri"].as_str())
        .expect("package:yaml rootUri");
    let package_root = PathBuf::from(
        root_uri
            .strip_prefix("file://")
            .expect("package:yaml must use a file rootUri"),
    );
    let test_files = [
        package_root.join("test/yaml_test.dart"),
        package_root.join("test/span_test.dart"),
    ];
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/yaml_oracle.dart");
    let mut child = Command::new(dart)
        .arg(format!("--packages={}", PathBuf::from(packages).display()))
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start YAML test extractor");
    serde_json::to_writer(
        child.stdin.as_mut().unwrap(),
        &json!({"testFiles": test_files}),
    )
    .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "YAML test extractor failed");
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    response["testInputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| input.as_str().unwrap().to_owned())
        .collect()
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
        ("!!int ' 42 '", false),
        ("!!float ' 1.5 '", false),
        ("!!int '42 '", false),
        ("!!float '1.25 '", false),
        ("0x7fffffffffffffff", false),
        ("0x8000000000000000", false),
        ("0xffffffffffffffff", false),
        ("9223372036854775808", false),
        ("0o7777777777777777777777", false),
        ("!!int '7'", false),
        ("!!int '0o+10'", false),
        ("!!int ' 0o10 '", false),
        ("!!int '0x+10'", false),
        ("!!int '0x-10'", false),
        ("!!int '0o-10'", false),
        ("!!int '0o 10'", false),
        ("{.nan: first, .nan: second}", false),
        ("{1: int, 1.0: float}", false),
        ("{9007199254740992: a, 9007199254740992.0: b}", false),
        ("{9007199254740993: a, 9007199254740992.0: b}", false),
        ("{9223372036854775807: a, 9223372036854775808: b}", false),
        ("%YAML 1.0\n---\nx\n", false),
        ("%YAML 1.2\n%YAML 1.2\n---\nx\n", false),
        ("{one: 1 two: 2}", false),
        ("[one, {a: b c: d}]", false),
        ("*\n", false),
        ("&\n", false),
        ("%\n---\nx\n", false),
        ("%YAML\n---\nx\n", false),
        ("%YAML 1\n---\nx\n", false),
        ("%YAML 1.x\n---\nx\n", false),
        ("%YAML 1.2 junk\n---\nx\n", false),
        ("%TAG\n---\nx\n", false),
        ("%TAG !foo!\n---\nx\n", false),
        ("%TAG !foo! tag:foo junk\n---\nx\n", false),
        ("!foo!bar value\n", false),
        ("!foo value\n", false),
        ("!foo%2Fbar value\n", false),
        ("!<foo%2Fbar> value\n", false),
        ("!<foo%F0%9F%98%80> value\n", false),
        ("!<foo%> value\n", false),
        ("!<foo%ZZ> value\n", false),
        ("!<foo%C3%28> value\n", false),
        ("!<foo%80> value\n", false),
        ("!<foo%E0%80%80> value\n", false),
        ("!<foo%ED%A0%80> value\n", false),
        ("!<foo%F4%90%80%80> value\n", false),
        ("%YAML 999999999999999999999999999999.2\n---\nx\n", false),
        ("%UNKNOWN value\rone: two\r", false),
        ("one: two\rone: three\r", false),
        ("name: \"\\x1\"\n", false),
        ("name: \"\\u12\"\n", false),
        ("name: \"\\uZ000\"\n", false),
        ("name: \"\\uD800\"\n", false),
        ("name: \"\\U00110000\"\n", false),
        ("name: |0\n  value\n", false),
        ("@value\n", false),
        ("]\n", false),
        ("{a: b]\n", false),
        ("dependencies:\n  one: any\n  two\n  three:\n  four\n", true),
        (
            "linter:\n  rules:\n    - annotate_overrides\n    alway\n",
            true,
        ),
        ("a: [", true),
    ];
    eprintln!("fixed YAML parity: {} inputs", cases.len());
    assert_parity(&cases);
}

#[test]
fn package_yaml_runtime_error_parity() {
    assert_parity(&[
        ("!!int ''", false),
        ("!!int ''", true),
        ("!!float ''", false),
        ("!!float ''", true),
        ("!!float '1'", false),
        ("!!float '1'", true),
    ]);
}

#[test]
fn package_yaml_source_test_literal_parity() {
    let inputs = package_yaml_test_inputs();
    assert!(
        inputs.len() >= 150,
        "only {} YAML test inputs",
        inputs.len()
    );
    let cases: Vec<(&str, bool)> = inputs
        .iter()
        .flat_map(|input| [(input.as_str(), false), (input.as_str(), true)])
        .collect();
    eprintln!(
        "package:yaml source parity: {} inputs, {} strict/recover comparisons",
        inputs.len(),
        cases.len()
    );
    assert_parity(&cases);
}

#[test]
fn malformed_mutation_parity() {
    let bases = [
        "name: package\ndependencies:\n  one: any\n  two: ^2.0.0\n",
        "analyzer:\n  exclude:\n    - generated/**\nlinter:\n  rules: [one, two]\n",
        "top: {nested: [one, two, {three: four}]}\n",
    ];
    let mut inputs = Vec::new();
    for base in bases {
        for (index, _) in base.match_indices('\n').skip(1) {
            inputs.push(base[..index].to_owned());
        }
        inputs.push(base.replacen("  ", "\t", 1));
        inputs.push(base.replacen("  ", " ", 1));
    }
    inputs.extend(
        [
            "[one, two",
            "{one: two",
            "a: \"unterminated",
            "a: 'unterminated",
            "a: \"bad \\q\"",
            "a: *unknown",
            "a: &",
            "a: !<bad tag",
            "%YAML 1.x\n---\na",
            "%TAG !x! tag:one\n%TAG !x! tag:two\n---\n!x!a b",
            "a: 1\na: 2\n",
            "\u{feff}a:\r\n\tb: c\r\n",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    inputs.push(format!("{}: value\n", "a".repeat(1025)));
    inputs.push(format!("{}: value\n", "😀".repeat(513)));
    let cases: Vec<(&str, bool)> = inputs
        .iter()
        .flat_map(|input| [(input.as_str(), false), (input.as_str(), true)])
        .collect();
    eprintln!(
        "malformed mutation parity: {} inputs, {} strict/recover comparisons",
        inputs.len(),
        cases.len()
    );
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
    let cases: Vec<(&str, bool)> = owned
        .iter()
        .flat_map(|text| [(text.as_str(), false), (text.as_str(), true)])
        .collect();
    assert_parity(&cases);
    eprintln!(
        "YAML corpus parity: {0}/{0} files in strict and recover modes ({1}/{1} comparisons)",
        paths.len(),
        cases.len()
    );
}
