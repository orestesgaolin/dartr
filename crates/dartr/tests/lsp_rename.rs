//! Differential tests of `textDocument/prepareRename` and
//! `textDocument/rename`: the same requests against `dart language-server`
//! and `dartr language-server`, on a fixture project (scripted probes) and
//! (with `DARTR_LSP_RENAME_CORPUS=<folder>[:<every>[:<max files>]]`) on the
//! identifiers of a real project. Prints how many responses are identical:
//! the prepare result, the rename result (workspace edit or error) and the
//! prompts of `window/showMessageRequest`.
//!
//! `DARTR_LSP_RENAME_DUMP=<folder>` writes the responses of each server;
//! with `DARTR_LSP_RENAME_REUSE_DART=1` the `dart` responses are read from
//! that folder when they exist.

mod lsp_support;

use std::path::{Path, PathBuf};

use lsp_support::*;
use serde_json::{Value, json};

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

/// A rename probe: the file, the position (line, UTF-16 column) and the
/// new name.
#[derive(Clone, Debug)]
struct Probe {
    file: PathBuf,
    line: u32,
    column: u32,
    new_name: String,
    label: String,
}

/// (line, UTF-16 column) of the UTF-16 offset [offset] in [text].
fn position_of(text: &str, offset: usize) -> (u32, u32) {
    let (mut line, mut col, mut units) = (0u32, 0u32, 0usize);
    for c in text.chars() {
        if units >= offset {
            break;
        }
        units += c.len_utf16();
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += c.len_utf16() as u32;
        }
    }
    (line, col)
}

/// The probe at the [occurrence]-th match of [marker] in [file], [delta]
/// UTF-16 units into the marker.
fn probe(
    root: &Path,
    file: &str,
    marker: &str,
    occurrence: usize,
    delta: usize,
    new_name: &str,
) -> Probe {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let byte = text
        .match_indices(marker)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("no {marker:?} #{occurrence} in {file}"))
        .0;
    let offset = text[..byte].encode_utf16().count() + delta;
    let (line, column) = position_of(&text, offset);
    Probe {
        file: path,
        line,
        column,
        new_name: new_name.to_string(),
        label: format!("{file}:{marker}#{occurrence}+{delta} -> {new_name:?}"),
    }
}

fn write_project(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let write = |rel: &str, content: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    };
    write(
        "app/pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.9.0\n",
    );
    write(
        "app/lib/shapes.dart",
        r#"/// Shapes.
library shapes;

import 'dart:math' as math;

abstract class Shape {
  double get area;
  String describe({bool verbose = false}) => 'shape';
}

class Circle extends Shape {
  final double radius;
  Circle(this.radius);
  Circle.unit() : this(1);

  @override
  double get area => math.pi * radius * radius;

  Circle scale(double factor, [int times = 1]) => Circle(radius * factor);

  Circle operator +(Circle other) => Circle(radius + other.radius);
}

class Square extends Shape {
  final double side;
  Square(this.side);

  @override
  double get area => side * side;

  @override
  String describe({bool verbose = false}) => verbose ? 'square $side' : 'square';
}

enum Color { red, green, blue }

extension ShapeList on List<Shape> {
  double get totalArea => fold(0, (a, b) => a + b.area);
  Shape? largest() => isEmpty ? null : reduce((a, b) => a.area > b.area ? a : b);
}

typedef Measure = double Function(Shape shape);

int _counter = 0;
int get counter => _counter;
set counter(int value) => _counter = value;

T first<T>(List<T> items) => items.first;

mixin Named {
  String get name;
}

extension type Meters(double value) {
  Meters twice() => Meters(value * 2);
}
"#,
    );
    write(
        "app/lib/main.dart",
        r#"import 'dart:async';
import 'shapes.dart';
import 'shapes.dart' as s;

class Canvas with Named {
  final List<Shape> shapes = [];
  int count = 0;

  @override
  String get name => 'canvas';

  void add(Shape shape) {
    shapes.add(shape);
    count++;
  }

  void clear() {
    var count = shapes.length;
    shapes.clear();
    this.count = count;
  }
}

double total(List<Shape> shapes, {double scale = 1}) {
  var sum = 0.0;
  outer:
  for (final shape in shapes) {
    if (sum > 100) break outer;
    sum += shape.area * scale;
  }
  return sum;
}

void main() {
  final canvas = Canvas();
  canvas.add(Circle(1));
  canvas.add(Circle.unit());
  canvas.add(s.Square(2));
  final value = total(canvas.shapes, scale: 2);
  print(value);
  print(Color.red);
  print(first([1, 2]));
  counter = counter + 1;
  Timer(const Duration(seconds: 1), () {});
  final a = 1;
  final b = 2;
  print('$a ${b + 1}');
  print(Meters(2).twice());
  print([Circle(1)].totalArea);
}
"#,
    );
    root
}

fn fixture_probes(root: &Path) -> Vec<Probe> {
    let m = "app/lib/main.dart";
    let sh = "app/lib/shapes.dart";
    vec![
        // Locals and parameters.
        probe(root, m, "shapes.add(shape)", 0, 11, "item"),
        probe(root, m, "var count", 0, 4, "n"),
        probe(root, m, "var count", 0, 4, "shapes"),
        probe(root, m, "final a", 0, 6, "b"),
        probe(root, m, "final a", 0, 6, "class"),
        probe(root, m, "final a", 0, 6, "Alpha"),
        probe(root, m, "final a", 0, 6, "1x"),
        probe(root, m, "final a", 0, 6, "a"),
        probe(root, m, "final canvas", 0, 6, "board"),
        probe(root, m, "{double scale", 0, 8, "factor"),
        probe(root, m, "scale: 2", 0, 0, "zoom"),
        probe(root, sh, "double factor", 0, 7, "f"),
        probe(root, sh, "{bool verbose", 0, 6, "detailed"),
        probe(root, sh, "{bool verbose", 0, 6, "_detailed"),
        probe(root, m, "outer:", 0, 0, "loop"),
        probe(root, m, "break outer", 0, 6, "loop"),
        // Class members.
        probe(root, m, "int count", 0, 4, "total"),
        probe(root, m, "this.count", 0, 5, "size"),
        probe(root, m, "void add", 0, 5, "append"),
        probe(root, m, "void add", 0, 5, "clear"),
        probe(root, m, "canvas.add(", 0, 7, "insert"),
        probe(root, sh, "final double radius", 0, 13, "r"),
        probe(root, sh, "this.radius", 0, 5, "size"),
        probe(root, sh, "double get area;", 0, 11, "size"),
        probe(root, m, "shape.area", 0, 6, "extent"),
        probe(root, sh, "operator +", 0, 9, "plus"),
        probe(root, m, "String get name", 0, 11, "title"),
        probe(root, sh, "Meters twice", 0, 7, "doubled"),
        probe(root, sh, "double get totalArea", 0, 11, "sum"),
        // Constructors.
        probe(root, sh, "Circle.unit()", 0, 7, "one"),
        probe(root, m, "Circle.unit()", 0, 7, "one"),
        probe(root, sh, "Circle(this.radius)", 0, 0, "create"),
        probe(root, m, "Circle(1)", 0, 0, "Disc"),
        // Unit members.
        probe(root, sh, "abstract class Shape", 0, 15, "Figure"),
        probe(root, m, "List<Shape>", 0, 5, "Figure"),
        probe(root, m, "class Canvas", 0, 6, "Square"),
        probe(root, m, "class Canvas", 0, 6, "Board"),
        probe(root, sh, "enum Color", 0, 5, "Hue"),
        probe(root, m, "Color.red", 0, 6, "crimson"),
        probe(root, sh, "extension ShapeList", 0, 10, "Shapes"),
        probe(root, sh, "typedef Measure", 0, 8, "Metric"),
        probe(root, sh, "int _counter", 0, 4, "_count"),
        probe(root, sh, "int _counter", 0, 4, "count"),
        probe(root, m, "counter = counter", 0, 0, "tally"),
        probe(root, m, "double total(", 0, 7, "sumAll"),
        probe(root, sh, "T first<T>", 0, 2, "head"),
        probe(root, sh, "first<T>", 0, 6, "E"),
        probe(root, sh, "mixin Named", 0, 6, "Titled"),
        probe(root, sh, "extension type Meters", 0, 15, "Distance"),
        // Imports and libraries.
        probe(root, m, "as s;", 0, 3, "shapesLib"),
        probe(root, m, "as s;", 0, 3, ""),
        probe(root, m, "s.Square", 0, 0, "geo"),
        probe(root, m, "import 'shapes.dart';", 0, 0, "geo"),
        probe(root, sh, "math.pi", 0, 0, "m"),
        probe(root, sh, "library shapes", 0, 8, "geometry"),
        // Not renamable.
        probe(root, m, "print(value)", 0, 0, "show"),
        probe(root, m, "Timer(", 0, 0, "Clock"),
        probe(root, m, "void main", 0, 0, "x"),
        probe(root, m, "'canvas'", 0, 2, "x"),
    ]
}

/// The responses of one probe: prepare, rename and the prompts.
fn run(program: &str, root: &Path, probes: &[Probe]) -> Vec<Value> {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    if std::env::var_os("DARTR_LSP_STEP_TIMEOUT").is_none() {
        c.step_timeout = std::time::Duration::from_secs(900);
    }
    // Choose "Rename Anyway" for warnings.
    c.show_message_choice = Some(0);
    let init = dart_code_initialize_params(root);
    let response = c.request("initialize", init);
    assert!(response["result"]["capabilities"].is_object(), "{response}");
    c.notify("initialized", json!({}));
    c.settle(true);
    // The editor has the files open.
    let mut opened: Vec<PathBuf> = Vec::new();
    for p in probes {
        if !opened.contains(&p.file) {
            opened.push(p.file.clone());
            let text = std::fs::read_to_string(&p.file).unwrap();
            c.notify(
                "textDocument/didOpen",
                json!({"textDocument": {"uri": file_uri(&p.file), "languageId": "dart", "version": 1, "text": text}}),
            );
        }
    }
    c.settle(false);
    let mut out = Vec::new();
    for p in probes {
        let uri = file_uri(&p.file);
        let position = json!({"line": p.line, "character": p.column});
        let prepare = c.request(
            "textDocument/prepareRename",
            json!({"textDocument": {"uri": uri}, "position": position}),
        );
        let before = c.server_requests.len();
        let rename = c.request(
            "textDocument/rename",
            json!({"textDocument": {"uri": uri}, "position": position, "newName": p.new_name}),
        );
        let prompts: Vec<Value> = c.server_requests[before..]
            .iter()
            .filter(|(m, _)| m == "window/showMessageRequest")
            .map(|(_, params)| params.clone())
            .collect();
        out.push(json!({
            "label": p.label,
            "prepare": strip(prepare),
            "rename": strip(rename),
            "prompts": prompts,
        }));
    }
    let _ = c.shutdown_and_exit();
    out
}

fn strip(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("jsonrpc");
        o.remove("id");
    }
    v
}

fn run_or_reuse(program: &str, label: &str, root: &Path, probes: &[Probe]) -> Vec<Value> {
    let dump = std::env::var_os("DARTR_LSP_RENAME_DUMP").map(PathBuf::from);
    let server = if program == "dart" { "dart" } else { "dartr" };
    let path = dump
        .as_ref()
        .map(|d| d.join(format!("{label}.{server}.json")));
    if server == "dart" && std::env::var_os("DARTR_LSP_RENAME_REUSE_DART").is_some() {
        if let Some(text) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            let value: Value = serde_json::from_str(&text).unwrap();
            return value.as_array().unwrap().clone();
        }
    }
    let out = run(program, root, probes);
    if let Some(path) = path {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        std::fs::write(
            path,
            serde_json::to_string_pretty(&Value::Array(out.clone())).unwrap(),
        )
        .unwrap();
    }
    out
}

/// The document changes of a workspace edit, sorted by URI (Dart orders
/// the files of a change by a hash map).
fn sorted_edit(edit: &Value) -> Value {
    let mut edit = edit.clone();
    if let Some(changes) = edit
        .pointer_mut("/result/documentChanges")
        .and_then(Value::as_array_mut)
    {
        changes.sort_by_key(|c| {
            c.pointer("/textDocument/uri")
                .or(c.get("uri"))
                .or(c.get("oldUri"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        });
    }
    edit
}

fn compare(title: &str, dart: &[Value], dartr: &[Value]) {
    assert_eq!(dart.len(), dartr.len());
    let (mut prepare, mut rename, mut rename_unordered, mut prompts) = (0, 0, 0, 0);
    let mut shown = 0;
    for (d, r) in dart.iter().zip(dartr) {
        let p_ok = d["prepare"] == r["prepare"];
        let r_ok = d["rename"] == r["rename"];
        let ru_ok = sorted_edit(&d["rename"]) == sorted_edit(&r["rename"]);
        let m_ok = d["prompts"] == r["prompts"];
        prepare += p_ok as usize;
        rename += r_ok as usize;
        rename_unordered += ru_ok as usize;
        prompts += m_ok as usize;
        if (!p_ok || !ru_ok || !m_ok) && shown < 40 {
            shown += 1;
            eprintln!("DIFF {}", d["label"]);
            for key in ["prepare", "rename", "prompts"] {
                if d[key] != r[key] && !(key == "rename" && ru_ok) {
                    let ds = d[key].to_string();
                    let rs = r[key].to_string();
                    eprintln!(
                        "  {key}\n    dart:  {}\n    dartr: {}",
                        &ds[..ds.len().min(900)],
                        &rs[..rs.len().min(900)]
                    );
                }
            }
        }
    }
    let n = dart.len();
    eprintln!(
        "{title}: {n} probes; prepare identical {prepare} | rename identical {rename} | rename identical up to file order {rename_unordered} | prompts identical {prompts}"
    );
}

#[test]
fn lsp_rename_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_project("lsp_rename");
    let probes = fixture_probes(&root);
    let dart = run_or_reuse("dart", "fixture", &root, &probes);
    let dartr = run_or_reuse(dartr_bin(), "fixture", &root, &probes);
    compare("fixture project", &dart, &dartr);
}

/// The identifiers of [text] at every [every]-th occurrence (outside
/// comments and strings is not checked: the servers answer the same for
/// both).
fn corpus_probes(path: &Path, every: usize, counter: &mut usize) -> Vec<Probe> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut units = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start_units = units;
        if c.is_ascii_alphabetic() || c == '_' {
            let mut j = i;
            let mut word = String::new();
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '$')
            {
                word.push(chars[j]);
                units += chars[j].len_utf16();
                j += 1;
            }
            *counter += 1;
            if *counter % every == 0 {
                let (line, column) = position_of(&text, start_units);
                let new_name = format!("{word}2");
                out.push(Probe {
                    file: path.to_path_buf(),
                    line,
                    column,
                    new_name: new_name.clone(),
                    label: format!(
                        "{}:{}:{} {word} -> {new_name}",
                        path.display(),
                        line,
                        column
                    ),
                });
            }
            i = j;
            continue;
        }
        units += c.len_utf16();
        i += 1;
    }
    out
}

fn dart_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "dart") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// `DARTR_LSP_RENAME_CORPUS=<folder>[:<every>[:<max files>]]`.
#[test]
fn lsp_rename_corpus() {
    let Some(spec) = std::env::var_os("DARTR_LSP_RENAME_CORPUS") else {
        eprintln!("skipped: set DARTR_LSP_RENAME_CORPUS=<folder>[:<every>[:<max files>]]");
        return;
    };
    let spec = spec.to_string_lossy().to_string();
    let mut parts = spec.split(':');
    let root = PathBuf::from(parts.next().unwrap()).canonicalize().unwrap();
    let every: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(50);
    let max_files: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let lib = root.join("lib");
    let mut files = dart_files(if lib.is_dir() { &lib } else { &root });
    if files.len() > max_files {
        let step = files.len() / max_files;
        files = files
            .into_iter()
            .step_by(step.max(1))
            .take(max_files)
            .collect();
    }
    let mut counter = 0;
    let probes: Vec<Probe> = files
        .iter()
        .flat_map(|f| corpus_probes(f, every, &mut counter))
        .collect();
    let label = format!(
        "{}-{every}-{max_files}",
        root.file_name().unwrap().to_string_lossy()
    );
    let dart = run_or_reuse("dart", &label, &root, &probes);
    let dartr = run_or_reuse(dartr_bin(), &label, &root, &probes);
    compare(&label, &dart, &dartr);
}
