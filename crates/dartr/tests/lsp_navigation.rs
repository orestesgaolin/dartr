//! Differential tests of the LSP navigation features (`textDocument/
//! {definition,typeDefinition,hover,references,documentHighlight,
//! implementation}`): the same requests at identifier positions against
//! `dart language-server` and `dartr language-server`, on a fixture
//! project with two packages, and (with `DARTR_LSP_NAV_CORPUS=<folder>`)
//! on a sample of real files. Prints per method how many responses are
//! identical.

mod lsp_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lsp_support::*;
use serde_json::{Value, json};

/// The methods compared, with the extra parameters of each.
const METHODS: &[&str] = &[
    "textDocument/definition",
    "textDocument/typeDefinition",
    "textDocument/hover",
    "textDocument/references",
    "textDocument/documentHighlight",
    "textDocument/implementation",
];

/// The `dartr` binary of the build, or `DARTR_LSP_NAV_BIN` (for example a
/// release build: a search in the SDK libraries is slow in a debug build).
fn dartr_bin() -> &'static str {
    match std::env::var("DARTR_LSP_NAV_BIN") {
        Ok(bin) => Box::leak(bin.into_boxed_str()),
        Err(_) => env!("CARGO_BIN_EXE_dartr"),
    }
}

fn session_args() -> Vec<&'static str> {
    vec![
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ]
}

/// The positions (line, UTF-16 column) of the identifiers of [text]
/// outside of comments and strings: one position inside each identifier
/// (its middle). Every [every]-th identifier is kept.
fn identifier_positions(text: &str, every: usize) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut count = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        let mut in_string: Option<char> = None;
        let mut col16 = 0u32;
        let mut cols = Vec::with_capacity(chars.len() + 1);
        for c in &chars {
            cols.push(col16);
            col16 += c.len_utf16() as u32;
        }
        cols.push(col16);
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = in_string {
                if c == '\\' {
                    i += 2;
                    continue;
                }
                if c == q {
                    in_string = None;
                }
                i += 1;
                continue;
            }
            if c == '/' && chars.get(i + 1) == Some(&'/') {
                break;
            }
            if c == '\'' || c == '"' {
                in_string = Some(c);
                i += 1;
                continue;
            }
            if c.is_ascii_alphabetic() || c == '_' || c == '$' {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$')
                {
                    i += 1;
                }
                count += 1;
                if count % every == 0 {
                    let middle = start + (i - start) / 2;
                    out.push((line_no as u32, cols[middle]));
                }
                continue;
            }
            i += 1;
        }
    }
    out
}

fn params_for(method: &str, uri: &str, (line, character): (u32, u32)) -> Value {
    let mut p =
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}});
    if method == "textDocument/references" {
        p["context"] = json!({"includeDeclaration": true});
    }
    p
}

/// The responses of one server: by method, the response of each
/// (file, position) request.
type Responses = BTreeMap<&'static str, Vec<(String, (u32, u32), Value)>>;

fn run_requests(program: &str, root: &Path, files: &[PathBuf], every: usize) -> Responses {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    let mut out: Responses = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(file).unwrap();
        let uri = file_uri(file);
        // As in an editor: the file is open (Dart: a priority file, whose
        // resolved unit the server keeps, so a local search finds the
        // elements of the request).
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
        );
        for position in identifier_positions(&text, every) {
            for &method in METHODS {
                let mut response = c.request(method, params_for(method, &uri, position));
                if let Some(o) = response.as_object_mut() {
                    o.remove("jsonrpc");
                }
                out.entry(method)
                    .or_default()
                    .push((uri.clone(), position, response));
            }
        }
        c.notify("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
    }
    let _ = c.shutdown_and_exit();
    out
}

/// `DARTR_LSP_NAV_DUMP=<folder>`: writes the responses of each server to
/// `<folder>/<label>.<server>.json`. With `DARTR_LSP_NAV_REUSE_DART=1`, the
/// `dart` responses are read from that file when it exists.
fn run_or_reuse(program: &str, label: &str, root: &Path, files: &[PathBuf], every: usize) -> Responses {
    let dump = std::env::var_os("DARTR_LSP_NAV_DUMP").map(PathBuf::from);
    let server = if program == "dart" { "dart" } else { "dartr" };
    let path = dump.as_ref().map(|d| d.join(format!("{label}.{server}.json")));
    if server == "dart" && std::env::var_os("DARTR_LSP_NAV_REUSE_DART").is_some() {
        if let Some(text) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            let value: Value = serde_json::from_str(&text).unwrap();
            let mut out: Responses = BTreeMap::new();
            for &method in METHODS {
                for item in value[method].as_array().into_iter().flatten() {
                    out.entry(method).or_default().push((
                        item[0].as_str().unwrap().to_string(),
                        (item[1].as_u64().unwrap() as u32, item[2].as_u64().unwrap() as u32),
                        item[3].clone(),
                    ));
                }
            }
            return out;
        }
    }
    let out = run_requests(program, root, files, every);
    if let Some(path) = path {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let value: serde_json::Map<String, Value> = out
            .iter()
            .map(|(m, items)| {
                let list = items.iter().map(|(u, p, r)| json!([u, p.0, p.1, r])).collect();
                (m.to_string(), Value::Array(list))
            })
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&Value::Object(value)).unwrap()).unwrap();
    }
    out
}

/// Prints per method the number of identical responses and the first
/// differences. Returns the counts (identical, total) by method.
fn compare(
    label: &str,
    dart: &Responses,
    dartr: &Responses,
) -> BTreeMap<&'static str, (usize, usize)> {
    println!("== {label}");
    let mut counts = BTreeMap::new();
    for &method in METHODS {
        let (Some(a), Some(b)) = (dart.get(method), dartr.get(method)) else {
            continue;
        };
        let mut same = 0;
        let mut same_set = 0;
        let mut shown = 0;
        for ((uri, pos, ra), (_, _, rb)) in a.iter().zip(b) {
            if ra == rb {
                same += 1;
            } else if sorted_result(ra) == sorted_result(rb) {
                // The same locations in another order (Dart: the order of
                // the files of a search depends on the earlier requests).
                same_set += 1;
            } else if shown < 5 {
                shown += 1;
                println!(
                    "  DIFF {method} {uri}:{}:{}\n    dart:  {}\n    dartr: {}",
                    pos.0 + 1,
                    pos.1 + 1,
                    truncate(&ra.to_string()),
                    truncate(&rb.to_string())
                );
            }
        }
        println!(
            "  {method}: {same} of {} identical ({same_set} more with the same locations in another order)",
            a.len()
        );
        counts.insert(method, (same, a.len()));
    }
    counts
}

/// The result array of a response, sorted (by the JSON text of each item).
fn sorted_result(response: &Value) -> Option<Vec<String>> {
    let items = response.get("result")?.as_array()?;
    let mut v: Vec<String> = items.iter().map(Value::to_string).collect();
    v.sort();
    Some(v)
}

fn truncate(s: &str) -> String {
    s.chars().take(600).collect()
}

/// The fixture: package `pkg` (classes, mixins, extensions, generics,
/// enums, typedefs, a part) and package `app` that imports it.
fn write_project() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lsp_navigation");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let write = |rel: &str, content: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    };
    write(
        "pkg/pubspec.yaml",
        "name: pkg\nenvironment:\n  sdk: ^3.9.0\n",
    );
    write(
        "pkg/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"pkg","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"}]}"#,
    );
    write(
        "pkg/lib/pkg.dart",
        "/// The shapes library.\nlibrary;\n\nexport 'src/shapes.dart';\nexport 'src/util.dart' show Pair, swap;\n",
    );
    write(
        "pkg/lib/src/shapes.dart",
        r#"import 'dart:math' as math;

part 'shapes_part.dart';

/// A shape with an [area].
///
/// Use [Circle] or [Square].
abstract class Shape {
  /// The area of this shape.
  double get area;

  /// Describes the shape.
  String describe() => '$runtimeType with area $area';

  static Shape unit() => Square(1);
}

/// Something that has a name.
mixin Named on Shape {
  String get name;

  @override
  String describe() => '$name: ${super.describe()}';
}

/// A circle.
class Circle extends Shape with Named {
  final double radius;

  @override
  final String name;

  Circle(this.radius, {this.name = 'circle'});

  /// A circle with diameter [d].
  Circle.diameter(double d) : this(d / 2);

  @override
  double get area => math.pi * radius * radius;
}

class Square extends Shape {
  final double side;

  const Square(this.side);

  @override
  double get area => side * side;

  Square operator +(Square other) => Square(side + other.side);
}

enum Color {
  red,
  green;

  bool get isRed => this == Color.red;
}

typedef ShapeFactory<T extends Shape> = T Function(double size);

/// Extensions on lists of shapes.
extension ShapeList on List<Shape> {
  double get totalArea => fold(0.0, (sum, s) => sum + s.area);
}

/// A box of [T].
class Box<T> {
  T value;
  Box(this.value);

  R map<R>(R Function(T) f) => f(value);
}
"#,
    );
    write(
        "pkg/lib/src/shapes_part.dart",
        "part of 'shapes.dart';\n\nclass Triangle extends Shape {\n  final double base, height;\n  Triangle(this.base, this.height);\n\n  @override\n  double get area => base * height / 2;\n}\n",
    );
    write(
        "pkg/lib/src/util.dart",
        "typedef Pair<A, B> = (A, B);\n\nPair<B, A> swap<A, B>(Pair<A, B> p) => (p.$2, p.$1);\n\nint counter = 0;\n",
    );
    write(
        "app/pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.9.0\ndependencies:\n  pkg:\n    path: ../pkg\n",
    );
    write(
        "app/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"},{"name":"pkg","rootUri":"../../pkg/","packageUri":"lib/","languageVersion":"3.9"}]}"#,
    );
    write(
        "app/lib/main.dart",
        r#"import 'package:pkg/pkg.dart';
import 'package:pkg/pkg.dart' as p;

part 'helpers.dart';

/// Entry point.
void main(List<String> args) {
  var circle = Circle(2, name: 'c');
  final square = const Square(3);
  Shape shape = Shape.unit();
  var shapes = <Shape>[circle, square, shape];
  print(shapes.totalArea);
  print(circle.describe());
  var sum = square + const Square(1);
  var box = Box<Shape>(sum);
  var areaText = box.map((s) => s.area.toStringAsFixed(2));
  var pair = swap((1, 'one'));
  print(pair.$1.length + areaText.length);
  for (var color in Color.values) {
    if (color.isRed) continue;
    print(color.name);
  }
  outer:
  for (var i = 0; i < args.length; i++) {
    if (i > 2) break outer;
  }
  switch (shape) {
    case Circle(:var radius) when radius > 1:
      print(radius);
    case Square s:
      print(s.side);
    default:
  }
  p.Circle.diameter(4);
  helper(circle);
  int local(int x) => x * 2;
  print(local(counter));
}

class Labeled extends Circle {
  Labeled(super.radius, {super.name});

  @override
  double get area => super.area + 1;
}

ShapeFactory<Square> squares = Square.new;
"#,
    );
    write(
        "app/lib/helpers.dart",
        "part of 'main.dart';\n\nvoid helper(Named n) {\n  print(n.name);\n  var total = [n].totalArea;\n  print(total);\n}\n",
    );
    root
}

fn dart_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                if !name.starts_with('.') && name != "build" {
                    stack.push(p);
                }
            } else if name.ends_with(".dart") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn lsp_navigation_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_project();
    let files = dart_files(&root);
    let dart = run_or_reuse("dart", "fixture", &root, &files, 1);
    let dartr = run_or_reuse(dartr_bin(), "fixture", &root, &files, 1);
    let counts = compare("fixture project", &dart, &dartr);
    // The methods that must be at parity on the fixture.
    let required: Vec<&str> = std::env::var("DARTR_LSP_NAV_REQUIRED")
        .map(|s| {
            s.split(',')
                .map(|m| -> &'static str { Box::leak(m.to_string().into_boxed_str()) })
                .collect()
        })
        .unwrap_or_default();
    for m in required {
        let (same, total) = counts.get(m).copied().unwrap_or((0, 0));
        assert_eq!(same, total, "{m}: {same} of {total} identical");
    }
}

/// `DARTR_LSP_NAV_CORPUS=<folder>[:<every>[:<max files>]]`: a scripted run
/// over the Dart files of a real project folder.
#[test]
fn lsp_navigation_corpus() {
    let Some(spec) = std::env::var_os("DARTR_LSP_NAV_CORPUS") else {
        eprintln!("skipped: set DARTR_LSP_NAV_CORPUS=<folder>[:<every>[:<max files>]]");
        return;
    };
    let spec = spec.to_string_lossy().to_string();
    let mut parts = spec.split(':');
    let root = PathBuf::from(parts.next().unwrap()).canonicalize().unwrap();
    let every: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(50);
    let max_files: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let mut files = dart_files(&root);
    // A deterministic sample spread over the folder.
    if files.len() > max_files {
        let step = files.len() / max_files;
        files = files
            .into_iter()
            .step_by(step.max(1))
            .take(max_files)
            .collect();
    }
    let label = root.file_name().unwrap().to_string_lossy().to_string();
    let dart = run_or_reuse("dart", &label, &root, &files, every);
    let dartr = run_or_reuse(dartr_bin(), &label, &root, &files, every);
    compare(&format!("corpus {}", root.display()), &dart, &dartr);
}
