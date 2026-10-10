//! Differential tests of `textDocument/prepareRename`, `textDocument/rename`,
//! `workspace/willRenameFiles`, refactor `textDocument/codeAction`,
//! `refactor.validate`, `refactor.perform`, and `dart.refactor.*` commands:
//! the same requests against `dart language-server` and `dartr language-server`.

mod lsp_support;

use std::collections::BTreeMap;
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

/// A rename probe: element kind category, file, position (line, UTF-16 column),
/// new name, and whether Dart is expected to return a valid prepare+rename.
#[derive(Clone, Debug)]
struct Probe {
    kind: &'static str,
    file: PathBuf,
    line: u32,
    column: u32,
    new_name: String,
    label: String,
    expect_valid: bool,
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

/// UTF-16 offset of the [occurrence]-th match of [marker] in [file], plus [delta].
fn offset_of_marker(
    root: &Path,
    file: &str,
    marker: &str,
    occurrence: usize,
    delta: usize,
) -> usize {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let byte = text
        .match_indices(marker)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("no {marker:?} #{occurrence} in {file}"))
        .0;
    text[..byte].encode_utf16().count() + delta
}

/// The probe at the [occurrence]-th match of [marker] in [file], [delta]
/// UTF-16 units into the marker.
#[allow(clippy::too_many_arguments)]
fn probe_kind(
    kind: &'static str,
    root: &Path,
    file: &str,
    marker: &str,
    occurrence: usize,
    delta: usize,
    new_name: &str,
    expect_valid: bool,
) -> Probe {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let offset = offset_of_marker(root, file, marker, occurrence, delta);
    let (line, column) = position_of(&text, offset);
    Probe {
        kind,
        file: path,
        line,
        column,
        new_name: new_name.to_string(),
        label: format!("[{kind}] {file}:{marker}#{occurrence}+{delta} -> {new_name:?}"),
        expect_valid,
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
        "app/.dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {
      "name": "app",
      "rootUri": "../",
      "packageUri": "lib/",
      "languageVersion": "3.9"
    },
    {
      "name": "flutter",
      "rootUri": "../../flutter_stub",
      "packageUri": "lib/",
      "languageVersion": "3.9"
    },
    {
      "name": "meta",
      "rootUri": "../../meta_stub",
      "packageUri": "lib/",
      "languageVersion": "3.9"
    }
  ]
}
"#,
    );
    write(
        "meta_stub/lib/meta.dart",
        "const String required = 'required';\n",
    );
    write(
        "flutter_stub/lib/src/foundation/key.dart",
        "abstract class Key {\n  const Key(String value);\n}\n",
    );
    write(
        "flutter_stub/lib/src/widgets/framework.dart",
        r#"import 'package:flutter/src/foundation/key.dart';
export 'package:flutter/src/foundation/key.dart';

abstract class BuildContext {}

abstract class Widget {
  final Key? key;
  const Widget({this.key});
}

abstract class StatelessWidget extends Widget {
  const StatelessWidget({super.key});
  Widget build(BuildContext context);
}

abstract class StatefulWidget extends Widget {
  const StatefulWidget({super.key});
  State createState();
}

abstract class State<T extends StatefulWidget> {
  BuildContext get context => throw UnimplementedError();
  Widget build(BuildContext context);
}
"#,
    );
    write(
        "flutter_stub/lib/src/widgets/basic.dart",
        r#"import 'framework.dart';

class Row extends Widget {
  final List<Widget> children;
  const Row({super.key, required this.children});
}
"#,
    );
    write(
        "flutter_stub/lib/src/widgets/container.dart",
        r#"import 'framework.dart';

class Container extends Widget {
  final Widget? child;
  const Container({super.key, this.child});
}
"#,
    );
    write(
        "flutter_stub/lib/src/widgets/text.dart",
        r#"import 'framework.dart';

class Text extends Widget {
  final String data;
  const Text(this.data, {super.key});
}
"#,
    );
    write(
        "flutter_stub/lib/widgets.dart",
        r#"export 'src/foundation/key.dart';
export 'src/widgets/framework.dart';
export 'src/widgets/basic.dart';
export 'src/widgets/container.dart';
export 'src/widgets/text.dart';
"#,
    );
    write(
        "app/lib/shapes.dart",
        r#"/// Shapes.
library shapes;

import 'dart:math' as math;

part 'shapes_part.dart';

abstract class Shape {
  double get area;
  String describe({bool verbose = false}) => 'shape';
}

class Circle extends Shape {
  final double radius;
  final int? _secret;
  Circle(this.radius, {this._secret});
  Circle.unit() : _secret = null, this(1);
  factory Circle.fromDiameter(double d) => Circle(d / 2);

  @override
  double get area => math.pi * radius * radius;

  Circle scale(double factor, [int times = 1]) => Circle(radius * factor * times);

  Circle shift(double dx, double dy) => Circle(radius + dx + dy);

  Circle operator +(Circle other) => Circle(radius + other.radius);

  static Circle zero() => Circle(0);
}

class Oval extends Circle {
  final double minorRadius;
  Oval(super.radius, this.minorRadius, {super._secret});
}

class Square extends Shape {
  final double side;
  Square(this.side);

  @override
  double get area => side * side;

  @override
  String describe({bool verbose = false}) => verbose ? 'square $side' : 'square';
}

class Box<TValue> {
  final TValue item;
  Box(this.item);
  TValue unwrap() => item;
}

enum Color {
  red,
  green,
  blue;

  String get label => name;
}

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
        "app/lib/shapes_part.dart",
        r#"part of 'shapes.dart';

class Polygon extends Shape {
  final List<double> sides;
  Polygon(this.sides);

  @override
  double get area => sides.fold(0.0, (a, b) => a + b);

  @override
  String describe({bool verbose = false}) => verbose ? 'polygon ${sides.length}' : 'polygon';
}

int partHelper(int x) => x + _counter;

class NamedTag {
  final String id;
  NamedTag.of(this.id);
}
"#,
    );
    write(
        "app/lib/circle_card.dart",
        r#"import 'package:flutter/widgets.dart';
import 'shapes.dart';

class CircleCard extends StatelessWidget {
  final String title;
  const CircleCard(this.title, {super.key});

  @override
  Widget build(BuildContext context) {
    var greeting = 'Hello $title';
    return Container(
      child: Row(
        children: [
          Text(greeting),
          buildBadge(greeting),
        ],
      ),
    );
  }

  Widget buildBadge(String label) {
    return Container(child: Text(label));
  }
}

int computePair(int alpha, int beta, {int scale = 1}) {
  var temp = alpha + beta;
  var scaled = temp * scale;
  return scaled;
}

class Newer {
  final int x;
  Newer.new(this.x);
}
"#,
    );
    write(
        "app/lib/main.dart",
        r#"import 'dart:async';
import 'shapes.dart';
import 'shapes.dart' as s;
import 'circle_card.dart';

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
  canvas.add(Circle.fromDiameter(4));
  canvas.add(Circle.zero());
  canvas.add(Oval(2, 1));
  canvas.add(s.Square(2));
  canvas.add(Polygon([3, 4, 5]));
  final box = Box<int>(42);
  print(box.unwrap());
  final value = total(canvas.shapes, scale: 2);
  print(value);
  print(Color.red.label);
  print(first([1, 2]));
  counter = counter + 1;
  print(partHelper(counter));
  Timer(const Duration(seconds: 1), () {});
  final a = 1;
  final b = 2;
  print('$a ${b + 1}');
  final (left, right) = (a, b);
  print(left + right);
  int bump(int v) => v + 1;
  print(bump(a));
  print(Meters(2).twice());
  print([Circle(1)].totalArea);
  print(CircleCard('hi'));
  print(computePair(1, 2, scale: 3));
  print(Newer(5).x);
}
"#,
    );
    root
}

fn fixture_probes(root: &Path) -> Vec<Probe> {
    let m = "app/lib/main.dart";
    let sh = "app/lib/shapes.dart";
    let pt = "app/lib/shapes_part.dart";
    let cc = "app/lib/circle_card.dart";
    vec![
        // 1. Classes (declarations, references, prefixed references, part classes)
        probe_kind(
            "class",
            root,
            sh,
            "abstract class Shape",
            0,
            15,
            "Figure",
            true,
        ),
        probe_kind("class", root, m, "List<Shape>", 0, 5, "Figure", true),
        probe_kind("class", root, m, "class Canvas", 0, 6, "Board", true),
        probe_kind("class", root, m, "s.Square(2)", 0, 2, "Rect", true),
        probe_kind(
            "class",
            root,
            pt,
            "class Polygon",
            0,
            6,
            " Ngon".trim(),
            true,
        ),
        probe_kind("class", root, m, "Polygon([3", 0, 0, "MultiSide", true),
        probe_kind("class", root, sh, "class Box<TValue>", 0, 6, "Holder", true),
        // 2. Mixins
        probe_kind("mixin", root, sh, "mixin Named", 0, 6, "Titled", true),
        probe_kind("mixin", root, m, "with Named", 0, 5, "Labeled", true),
        // 3. Enums & enum constants
        probe_kind("enum", root, sh, "enum Color", 0, 5, "Hue", true),
        probe_kind("enum_constant", root, m, "Color.red", 0, 6, "crimson", true),
        probe_kind("enum_constant", root, sh, "  green,", 0, 2, "emerald", true),
        // 4. Extensions & extension types
        probe_kind(
            "extension",
            root,
            sh,
            "extension ShapeList",
            0,
            10,
            "ShapesExt",
            true,
        ),
        probe_kind(
            "extension_type",
            root,
            sh,
            "extension type Meters",
            0,
            15,
            "Distance",
            true,
        ),
        probe_kind(
            "extension_type",
            root,
            sh,
            "Meters(double value)",
            0,
            14,
            "amount",
            true,
        ),
        probe_kind(
            "extension_type",
            root,
            sh,
            "Meters twice",
            0,
            7,
            "doubled",
            true,
        ),
        // 5. Typedefs
        probe_kind("typedef", root, sh, "typedef Measure", 0, 8, "Metric", true),
        // 6. Type parameters
        probe_kind("type_parameter", root, sh, "first<T>", 0, 6, "E", true),
        probe_kind(
            "type_parameter",
            root,
            sh,
            "class Box<TValue>",
            0,
            10,
            "TItem",
            true,
        ),
        probe_kind(
            "type_parameter",
            root,
            sh,
            "final TValue item",
            0,
            6,
            "TElement",
            true,
        ),
        // 7. Constructors (unnamed, named, factory)
        probe_kind("constructor", root, sh, "Circle.unit()", 0, 7, "one", true),
        probe_kind("constructor", root, m, "Circle.unit()", 0, 7, "one", true),
        probe_kind(
            "constructor",
            root,
            sh,
            "Circle(this.radius",
            0,
            0,
            "create",
            true,
        ),
        probe_kind("constructor", root, m, "Circle(1)", 0, 0, "Disc", true),
        probe_kind(
            "constructor",
            root,
            sh,
            "Circle.fromDiameter",
            0,
            7,
            "byDiameter",
            true,
        ),
        probe_kind(
            "constructor",
            root,
            m,
            "Circle.fromDiameter",
            0,
            7,
            "byDiameter",
            true,
        ),
        probe_kind(
            "constructor",
            root,
            cc,
            "Newer.new(this.x)",
            0,
            6,
            "build",
            true,
        ),
        // 8. Fields, getters, setters, top-level variables
        probe_kind("field", root, m, "int count", 0, 4, "total", true),
        probe_kind("field", root, m, "this.count", 0, 5, "size", true),
        probe_kind("field", root, sh, "final double radius", 0, 13, "r", true),
        probe_kind("field", root, sh, "this.radius", 0, 5, "size", true),
        probe_kind(
            "field",
            root,
            sh,
            "final int? _secret",
            0,
            11,
            "_code",
            true,
        ),
        probe_kind(
            "getter_setter",
            root,
            sh,
            "double get area;",
            0,
            11,
            "size",
            true,
        ),
        probe_kind("getter_setter", root, m, "shape.area", 0, 6, "extent", true),
        probe_kind(
            "getter_setter",
            root,
            pt,
            "double get area =>",
            0,
            11,
            "surface",
            true,
        ),
        probe_kind(
            "getter_setter",
            root,
            m,
            "String get name",
            0,
            11,
            "title",
            true,
        ),
        probe_kind(
            "getter_setter",
            root,
            sh,
            "double get totalArea",
            0,
            11,
            "sumArea",
            true,
        ),
        probe_kind(
            "getter_setter",
            root,
            sh,
            "String get label =>",
            0,
            11,
            "tag",
            true,
        ),
        probe_kind(
            "top_level_variable",
            root,
            sh,
            "int _counter",
            0,
            4,
            "_count",
            true,
        ),
        probe_kind(
            "top_level_variable",
            root,
            sh,
            "int _counter",
            0,
            4,
            "count",
            true,
        ),
        probe_kind(
            "top_level_variable",
            root,
            m,
            "counter = counter",
            0,
            0,
            "tally",
            true,
        ),
        // 9. Methods & operators
        probe_kind("method", root, m, "void add", 0, 5, "append", true),
        probe_kind("method", root, m, "canvas.add(", 0, 7, "insert", true),
        probe_kind("method", root, sh, "String describe(", 0, 7, "format", true),
        probe_kind(
            "method",
            root,
            pt,
            "String describe(",
            0,
            7,
            "explain",
            true,
        ),
        probe_kind(
            "method",
            root,
            sh,
            "static Circle zero()",
            0,
            14,
            "origin",
            true,
        ),
        probe_kind("method", root, sh, "TValue unwrap()", 0, 7, "take", true),
        probe_kind("operator", root, sh, "operator +", 0, 9, "plus", false),
        // 10. Top-level functions (including part functions)
        probe_kind(
            "top_level_function",
            root,
            m,
            "double total(",
            0,
            7,
            "sumAll",
            true,
        ),
        probe_kind(
            "top_level_function",
            root,
            sh,
            "T first<T>",
            0,
            2,
            "head",
            true,
        ),
        probe_kind(
            "top_level_function",
            root,
            pt,
            "int partHelper(",
            0,
            4,
            "calcPart",
            true,
        ),
        probe_kind(
            "top_level_function",
            root,
            m,
            "partHelper(counter)",
            0,
            0,
            "assistPart",
            true,
        ),
        // 11. Parameters (positional, optional, named, hierarchy named, super formal, private named)
        probe_kind(
            "parameter",
            root,
            m,
            "shapes.add(shape)",
            0,
            11,
            "item",
            true,
        ),
        probe_kind("parameter", root, m, "{double scale", 0, 8, "factor", true),
        probe_kind("parameter", root, m, "scale: 2", 0, 0, "zoom", true),
        probe_kind("parameter", root, sh, "double factor", 0, 7, "f", true),
        probe_kind("parameter", root, sh, "[int times", 0, 5, "repeat", true),
        probe_kind(
            "parameter",
            root,
            sh,
            "{bool verbose",
            0,
            6,
            "detailed",
            true,
        ),
        probe_kind("parameter", root, pt, "{bool verbose", 0, 6, "full", true),
        probe_kind(
            "parameter",
            root,
            sh,
            "{bool verbose",
            0,
            6,
            "_detailed",
            true,
        ),
        probe_kind(
            "parameter",
            root,
            sh,
            "Oval(super.radius",
            0,
            11,
            "rad",
            true,
        ),
        // 12. Locals, pattern variables, local functions
        probe_kind("local_variable", root, m, "var count", 0, 4, "n", true),
        probe_kind(
            "local_variable",
            root,
            m,
            "final canvas",
            0,
            6,
            "board",
            true,
        ),
        probe_kind(
            "pattern_variable",
            root,
            m,
            "final (left, right)",
            0,
            7,
            "firstVal",
            true,
        ),
        probe_kind(
            "local_function",
            root,
            m,
            "int bump(int v)",
            0,
            4,
            "inc",
            true,
        ),
        probe_kind("local_function", root, m, "bump(a)", 0, 0, "step", true),
        // 13. Labels
        probe_kind("label", root, m, "outer:", 0, 0, "loop", true),
        probe_kind("label", root, m, "break outer", 0, 6, "loop", true),
        // 14. Import prefixes & library directives
        probe_kind("import_prefix", root, m, "as s;", 0, 3, "shapesLib", true),
        probe_kind("import_prefix", root, m, "as s;", 0, 3, "", true),
        probe_kind("import_prefix", root, m, "s.Square", 0, 0, "geo", true),
        probe_kind(
            "import_prefix",
            root,
            m,
            "import 'shapes.dart';",
            0,
            0,
            "geo",
            true,
        ),
        probe_kind("import_prefix", root, sh, "math.pi", 0, 0, "m", true),
        probe_kind(
            "library",
            root,
            sh,
            "library shapes",
            0,
            8,
            "geometry",
            true,
        ),
        // 15. Conflicts, warnings & unrenamable failure cases
        probe_kind("conflict", root, m, "var count", 0, 4, "shapes", true),
        probe_kind("conflict", root, m, "final a", 0, 6, "b", true),
        probe_kind("conflict", root, m, "final a", 0, 6, "class", false),
        probe_kind("conflict", root, m, "final a", 0, 6, "Alpha", true),
        probe_kind("conflict", root, m, "final a", 0, 6, "1x", false),
        probe_kind("conflict", root, m, "final a", 0, 6, "a", false),
        probe_kind("conflict", root, m, "void add", 0, 5, "clear", true),
        probe_kind("conflict", root, m, "class Canvas", 0, 6, "Square", true),
        probe_kind("unrenamable", root, m, "print(value)", 0, 0, "show", false),
        probe_kind("unrenamable", root, m, "Timer(", 0, 0, "Clock", false),
        probe_kind("unrenamable", root, m, "void main", 0, 0, "x", false),
        probe_kind("unrenamable", root, m, "'canvas'", 0, 2, "x", false),
    ]
}

/// The responses of one probe: prepare, rename and the prompts.
fn run_with_config(program: &str, root: &Path, probes: &[Probe], config: Value) -> Vec<Value> {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    if std::env::var_os("DARTR_LSP_STEP_TIMEOUT").is_none() {
        c.step_timeout = std::time::Duration::from_secs(900);
    }
    c.configuration = config;
    // Choose first action ("Rename Anyway" or "Yes") for prompts.
    c.show_message_choice = Some(0);
    let init = dart_code_initialize_params(root);
    let response = c.request("initialize", init);
    assert!(response["result"]["capabilities"].is_object(), "{response}");
    c.notify("initialized", json!({}));
    c.settle(true);
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
            "kind": p.kind,
            "label": p.label,
            "expect_valid": p.expect_valid,
            "prepare": strip(prepare),
            "rename": strip(rename),
            "prompts": prompts,
        }));
    }
    let _ = c.shutdown_and_exit();
    out
}

fn run(program: &str, root: &Path, probes: &[Probe]) -> Vec<Value> {
    run_with_config(program, root, probes, json!({}))
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
    for ptr in [
        "/result/documentChanges",
        "/edit/documentChanges",
        "/documentChanges",
    ] {
        if let Some(changes) = edit.pointer_mut(ptr).and_then(Value::as_array_mut) {
            changes.sort_by_key(|c| {
                c.pointer("/textDocument/uri")
                    .or(c.get("uri"))
                    .or(c.get("oldUri"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            });
        }
    }
    edit
}

#[derive(Default)]
struct KindStats {
    total: usize,
    prepare_ok: usize,
    rename_ok: usize,
    prompts_ok: usize,
}

fn compare(title: &str, dart: &[Value], dartr: &[Value], enforce_parity: bool) {
    assert!(!dart.is_empty(), "dart responses must not be empty");
    assert_eq!(dart.len(), dartr.len(), "step count mismatch");
    let (mut prepare, mut rename, mut rename_unordered, mut prompts) = (0, 0, 0, 0);
    let mut by_kind: BTreeMap<String, KindStats> = BTreeMap::new();
    let mut diffs = Vec::new();
    let mut shown = 0;
    for (d, r) in dart.iter().zip(dartr) {
        if d["expect_valid"] == json!(true) {
            assert!(
                !d["rename"]["result"].is_null(),
                "expected non-empty Dart rename result for {}: {}",
                d["label"],
                d["rename"]
            );
        }
        let p_ok = d["prepare"] == r["prepare"];
        let r_ok = d["rename"] == r["rename"];
        let ru_ok = sorted_edit(&d["rename"]) == sorted_edit(&r["rename"]);
        let m_ok = d["prompts"] == r["prompts"];
        prepare += p_ok as usize;
        rename += r_ok as usize;
        rename_unordered += ru_ok as usize;
        prompts += m_ok as usize;
        let kind = d["kind"].as_str().unwrap_or("other").to_string();
        let entry = by_kind.entry(kind).or_default();
        entry.total += 1;
        entry.prepare_ok += p_ok as usize;
        entry.rename_ok += ru_ok as usize;
        entry.prompts_ok += m_ok as usize;
        if !p_ok || !ru_ok || !m_ok {
            diffs.push(d["label"].as_str().unwrap_or("?").to_string());
            if shown < 40 {
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
    }
    let n = dart.len();
    eprintln!(
        "{title}: {n} probes; prepare identical {prepare} | rename identical {rename} | rename identical up to file order {rename_unordered} | prompts identical {prompts}"
    );
    for (kind, st) in &by_kind {
        eprintln!(
            "  kind {:<20}: {:>2}/{:>2} (prepare {}/{}, rename {}/{}, prompts {}/{})",
            kind,
            st.prepare_ok.min(st.rename_ok).min(st.prompts_ok),
            st.total,
            st.prepare_ok,
            st.total,
            st.rename_ok,
            st.total,
            st.prompts_ok,
            st.total
        );
    }
    if enforce_parity {
        assert!(
            diffs.is_empty(),
            "{title} had {} differing probes: {diffs:?}",
            diffs.len()
        );
    }
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
    compare("fixture project", &dart, &dartr, true);

    // Also test renameFilesWithClasses ("prompt" and "always") on CircleCard in circle_card.dart.
    let class_file_probes = vec![probe_kind(
        "class_file_rename",
        &root,
        "app/lib/circle_card.dart",
        "class CircleCard",
        0,
        6,
        "BadgeCard",
        true,
    )];
    for mode in ["prompt", "always"] {
        let cfg = json!({"renameFilesWithClasses": mode});
        let d_cf = run_with_config("dart", &root, &class_file_probes, cfg.clone());
        let r_cf = run_with_config(dartr_bin(), &root, &class_file_probes, cfg);
        compare(
            &format!("renameFilesWithClasses={mode}"),
            &d_cf,
            &r_cf,
            true,
        );
    }
}

#[derive(Clone, Debug)]
struct RefactorStep {
    category: &'static str,
    label: String,
    request_method: &'static str,
    params: Value,
    expect_non_empty: bool,
}

fn lsp_range(
    root: &Path,
    file: &str,
    start_marker: &str,
    start_occ: usize,
    start_delta: usize,
    len: usize,
) -> (usize, usize, Value) {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let offset = offset_of_marker(root, file, start_marker, start_occ, start_delta);
    let (sl, sc) = position_of(&text, offset);
    let (el, ec) = position_of(&text, offset + len);
    (
        offset,
        len,
        json!({
            "start": {"line": sl, "character": sc},
            "end": {"line": el, "character": ec},
        }),
    )
}

fn refactor_steps(root: &Path) -> Vec<RefactorStep> {
    let sh = "app/lib/shapes.dart";
    let pt = "app/lib/shapes_part.dart";
    let m = "app/lib/main.dart";
    let cc = "app/lib/circle_card.dart";
    let sh_path = root.join(sh).display().to_string();
    let cc_path = root.join(cc).display().to_string();
    let sh_uri = file_uri(&root.join(sh));
    let m_uri = file_uri(&root.join(m));
    let cc_uri = file_uri(&root.join(cc));

    let mut steps = Vec::new();

    // 1. textDocument/codeAction (refactor) at various selections
    let ca_probes = [
        (
            "codeAction:extract_expr",
            cc,
            &cc_uri,
            "alpha + beta",
            0,
            0,
            12,
        ),
        (
            "codeAction:extract_widget",
            cc,
            &cc_uri,
            "Container(\n      child: Row",
            0,
            0,
            9,
        ),
        ("codeAction:inline_local", cc, &cc_uri, "var temp", 0, 4, 0),
        (
            "codeAction:inline_method",
            cc,
            &cc_uri,
            "Widget buildBadge",
            0,
            7,
            0,
        ),
        (
            "codeAction:convert_getter",
            sh,
            &sh_uri,
            "double get totalArea",
            0,
            11,
            0,
        ),
        (
            "codeAction:convert_method",
            sh,
            &sh_uri,
            "Meters twice()",
            0,
            7,
            0,
        ),
        (
            "codeAction:formal_params",
            cc,
            &cc_uri,
            "int alpha",
            0,
            4,
            0,
        ),
        (
            "codeAction:ctor_name",
            sh,
            &sh_uri,
            "Square(this.side)",
            0,
            0,
            0,
        ),
        (
            "codeAction:import_prefix",
            m,
            &m_uri,
            "import 'shapes.dart';",
            0,
            0,
            0,
        ),
    ];
    for (label, file, uri, marker, occ, delta, len) in ca_probes {
        let (_, _, range) = lsp_range(root, file, marker, occ, delta, len);
        steps.push(RefactorStep {
            category: "codeAction",
            label: label.to_string(),
            request_method: "textDocument/codeAction",
            params: json!({
                "textDocument": {"uri": uri},
                "range": range,
                "context": {"diagnostics": [], "only": ["refactor"]},
            }),
            expect_non_empty: true,
        });
    }

    // 2. EXTRACT_LOCAL_VARIABLE (validate + perform on 2 shapes)
    {
        let (off1, len1, _) = lsp_range(root, cc, "alpha + beta", 0, 0, 12);
        let (off2, len2, _) = lsp_range(root, sh, "side * side", 0, 0, 11);
        for (lbl, path, off, len, name) in [
            (
                "EXTRACT_LOCAL_VARIABLE:sum_expr",
                &cc_path,
                off1,
                len1,
                "sumVal",
            ),
            (
                "EXTRACT_LOCAL_VARIABLE:area_expr",
                &sh_path,
                off2,
                len2,
                "sqArea",
            ),
        ] {
            for cmd in ["refactor.validate", "refactor.perform"] {
                steps.push(RefactorStep {
                    category: "EXTRACT_LOCAL_VARIABLE",
                    label: format!("{lbl}:{cmd}"),
                    request_method: "workspace/executeCommand",
                    params: json!({
                        "command": cmd,
                        "arguments": ["EXTRACT_LOCAL_VARIABLE", path, null, off, len, {"name": name, "extractAll": true}],
                    }),
                    expect_non_empty: true,
                });
            }
        }
    }

    // 3. EXTRACT_METHOD (validate + perform on 3 shapes: function expr, class method expr, statements)
    {
        let (off1, len1, _) = lsp_range(root, cc, "alpha + beta", 0, 0, 12);
        let (off2, len2, _) = lsp_range(root, sh, "radius * factor * times", 0, 0, 23);
        let (off3, len3, _) = lsp_range(
            root,
            cc,
            "var temp = alpha + beta;\n  var scaled = temp * scale;",
            0,
            0,
            53,
        );
        for (lbl, path, off, len, name) in [
            (
                "EXTRACT_METHOD:top_fn_expr",
                &cc_path,
                off1,
                len1,
                "addPair",
            ),
            (
                "EXTRACT_METHOD:class_method_expr",
                &sh_path,
                off2,
                len2,
                "scaledRadius",
            ),
            (
                "EXTRACT_METHOD:statements",
                &cc_path,
                off3,
                len3,
                "computeScaled",
            ),
        ] {
            for cmd in ["refactor.validate", "refactor.perform"] {
                steps.push(RefactorStep {
                    category: "EXTRACT_METHOD",
                    label: format!("{lbl}:{cmd}"),
                    request_method: "workspace/executeCommand",
                    params: json!({
                        "command": cmd,
                        "arguments": ["EXTRACT_METHOD", path, null, off, len, {"name": name, "extractAll": true, "createGetter": false}],
                    }),
                    expect_non_empty: true,
                });
            }
        }
    }

    // 4. EXTRACT_WIDGET (validate + perform on 2 shapes: widget expression in build, widget method)
    {
        let (off1, len1, _) = lsp_range(root, cc, "Text(greeting)", 0, 0, 14);
        let (off2, len2, _) = lsp_range(root, cc, "buildBadge", 1, 0, 0);
        for (lbl, off, len, name) in [
            ("EXTRACT_WIDGET:expr", off1, len1, "GreetingText"),
            ("EXTRACT_WIDGET:method", off2, len2, "BadgeWidget"),
        ] {
            for cmd in ["refactor.validate", "refactor.perform"] {
                steps.push(RefactorStep {
                    category: "EXTRACT_WIDGET",
                    label: format!("{lbl}:{cmd}"),
                    request_method: "workspace/executeCommand",
                    params: json!({
                        "command": cmd,
                        "arguments": ["EXTRACT_WIDGET", &cc_path, null, off, len, {"name": name}],
                    }),
                    expect_non_empty: true,
                });
            }
        }
    }

    // 5. INLINE_LOCAL_VARIABLE (validate + perform on 2 shapes: simple local, string interpolation local)
    {
        let (off1, _, _) = lsp_range(root, cc, "var temp", 0, 4, 0);
        let (off2, _, _) = lsp_range(root, cc, "var greeting", 0, 4, 0);
        for (lbl, off) in [
            ("INLINE_LOCAL_VARIABLE:temp", off1),
            ("INLINE_LOCAL_VARIABLE:greeting_interp", off2),
        ] {
            for cmd in ["refactor.validate", "refactor.perform"] {
                steps.push(RefactorStep {
                    category: "INLINE_LOCAL_VARIABLE",
                    label: format!("{lbl}:{cmd}"),
                    request_method: "workspace/executeCommand",
                    params: json!({
                        "command": cmd,
                        "arguments": ["INLINE_LOCAL_VARIABLE", &cc_path, null, off, 0, {}],
                    }),
                    expect_non_empty: true,
                });
            }
        }
    }

    // 6. INLINE_METHOD (validate + perform on 3 shapes: method, getter, top-level function)
    {
        let (off1, _, _) = lsp_range(root, cc, "Widget buildBadge", 0, 7, 0);
        let (off2, _, _) = lsp_range(root, sh, "TValue unwrap()", 0, 7, 0);
        let (off3, _, _) = lsp_range(root, pt, "int partHelper", 0, 4, 0);
        for (lbl, path, off) in [
            ("INLINE_METHOD:buildBadge", &cc_path, off1),
            ("INLINE_METHOD:unwrap", &sh_path, off2),
            (
                "INLINE_METHOD:partHelper",
                &root.join(pt).display().to_string(),
                off3,
            ),
        ] {
            for cmd in ["refactor.validate", "refactor.perform"] {
                steps.push(RefactorStep {
                    category: "INLINE_METHOD",
                    label: format!("{lbl}:{cmd}"),
                    request_method: "workspace/executeCommand",
                    params: json!({
                        "command": cmd,
                        "arguments": ["INLINE_METHOD", path, null, off, 0, {}],
                    }),
                    expect_non_empty: true,
                });
            }
        }
    }

    // 7. CONVERT_GETTER_TO_METHOD (2 shapes: extension getter, hierarchy getter)
    {
        let (off1, _, _) = lsp_range(root, sh, "double get totalArea", 0, 11, 0);
        let (off2, _, _) = lsp_range(root, sh, "int get counter", 0, 8, 0);
        for (lbl, off) in [
            ("CONVERT_GETTER_TO_METHOD:totalArea", off1),
            ("CONVERT_GETTER_TO_METHOD:counter", off2),
        ] {
            steps.push(RefactorStep {
                category: "CONVERT_GETTER_TO_METHOD",
                label: format!("{lbl}:perform"),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "refactor.perform",
                    "arguments": ["CONVERT_GETTER_TO_METHOD", &sh_path, null, off, 0, {}],
                }),
                expect_non_empty: true,
            });
        }
    }

    // 8. CONVERT_METHOD_TO_GETTER (2 shapes: extension type method, generic class method)
    {
        let (off1, _, _) = lsp_range(root, sh, "Meters twice()", 0, 7, 0);
        let (off2, _, _) = lsp_range(root, sh, "TValue unwrap()", 0, 7, 0);
        for (lbl, off) in [
            ("CONVERT_METHOD_TO_GETTER:twice", off1),
            ("CONVERT_METHOD_TO_GETTER:unwrap", off2),
        ] {
            steps.push(RefactorStep {
                category: "CONVERT_METHOD_TO_GETTER",
                label: format!("{lbl}:perform"),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "refactor.perform",
                    "arguments": ["CONVERT_METHOD_TO_GETTER", &sh_path, null, off, 0, {}],
                }),
                expect_non_empty: true,
            });
        }
    }

    // 9. MOVE_FILE via workspace/willRenameFiles (3 shapes: rename in same dir, move to subdir, rename part file)
    {
        let pt_uri = file_uri(&root.join(pt));
        let new_cc_uri = file_uri(&root.join("app/lib/badge_card.dart"));
        let sub_sh_uri = file_uri(&root.join("app/lib/sub/shapes.dart"));
        let new_pt_uri = file_uri(&root.join("app/lib/polygon_part.dart"));
        for (lbl, old_u, new_u) in [
            ("MOVE_FILE:same_dir", &cc_uri, &new_cc_uri),
            ("MOVE_FILE:to_subdir", &sh_uri, &sub_sh_uri),
            ("MOVE_FILE:part_file", &pt_uri, &new_pt_uri),
        ] {
            steps.push(RefactorStep {
                category: "MOVE_FILE",
                label: lbl.to_string(),
                request_method: "workspace/willRenameFiles",
                params: json!({
                    "files": [{"oldUri": old_u, "newUri": new_u}],
                }),
                expect_non_empty: true,
            });
        }
    }

    // 10. 8 Framework dart.refactor.* commands (2 shapes each)
    let m_path = root.join(m).display().to_string();
    // 10a. convert_all_formal_parameters_to_named
    {
        let (off1, len1, _) = lsp_range(root, cc, "computePair(", 0, 0, 0);
        let (off2, len2, _) = lsp_range(root, sh, "scale(double factor", 0, 0, 0);
        for (lbl, path, off, len) in [
            (
                "convert_all_formal_parameters_to_named:computePair",
                &cc_path,
                off1,
                len1,
            ),
            (
                "convert_all_formal_parameters_to_named:scale",
                &sh_path,
                off2,
                len2,
            ),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.convert_all_formal_parameters_to_named",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.convert_all_formal_parameters_to_named",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10b. convert_selected_formal_parameters_to_named
    {
        let (off1, len1, _) = lsp_range(root, cc, "int beta", 0, 4, 0);
        let (off2, len2, _) = lsp_range(root, sh, "double dx", 0, 7, 0);
        for (lbl, path, off, len) in [
            (
                "convert_selected_formal_parameters_to_named:beta",
                &cc_path,
                off1,
                len1,
            ),
            (
                "convert_selected_formal_parameters_to_named:dx",
                &sh_path,
                off2,
                len2,
            ),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.convert_selected_formal_parameters_to_named",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.convert_selected_formal_parameters_to_named",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10c. move_selected_formal_parameters_left
    {
        let (off1, len1, _) = lsp_range(root, cc, "int beta", 0, 4, 0);
        let (off2, len2, _) = lsp_range(root, sh, "double dy", 0, 7, 0);
        for (lbl, path, off, len) in [
            (
                "move_selected_formal_parameters_left:beta",
                &cc_path,
                off1,
                len1,
            ),
            (
                "move_selected_formal_parameters_left:dy",
                &sh_path,
                off2,
                len2,
            ),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.move_selected_formal_parameters_left",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.move_selected_formal_parameters_left",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10d. move_top_level_to_file
    {
        let (off1, len1, _) = lsp_range(root, sh, "class Square", 0, 6, 0);
        let (off2, len2, _) = lsp_range(root, cc, "class Newer", 0, 6, 0);
        let dest1 = file_uri(&root.join("app/lib/square.dart"));
        let dest2 = file_uri(&root.join("app/lib/newer.dart"));
        for (lbl, path, off, len, dest) in [
            ("move_top_level_to_file:Square", &sh_path, off1, len1, dest1),
            ("move_top_level_to_file:Newer", &cc_path, off2, len2, dest2),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.move_top_level_to_file",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.move_top_level_to_file",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [dest],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10e. add_constructor_name
    {
        let (off1, len1, _) = lsp_range(root, sh, "Square(this.side)", 0, 0, 0);
        let (off2, len2, _) = lsp_range(root, sh, "Box(this.item)", 0, 0, 0);
        for (lbl, path, off, len) in [
            ("add_constructor_name:Square", &sh_path, off1, len1),
            ("add_constructor_name:Box", &sh_path, off2, len2),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.add_constructor_name",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.add_constructor_name",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": ["named"],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10f. remove_constructor_name
    {
        let pt_path = root.join(pt).display().to_string();
        let (off0, len0, _) = lsp_range(root, pt, "NamedTag.of(this.id)", 0, 9, 0);
        let (off1, len1, _) = lsp_range(root, cc, "Newer.new(this.x)", 0, 6, 0);
        let (off2, len2, _) = lsp_range(root, sh, "Circle.unit()", 0, 7, 0);
        for (lbl, path, off, len, non_empty) in [
            (
                "remove_constructor_name:NamedTag.of",
                &pt_path,
                off0,
                len0,
                true,
            ),
            (
                "remove_constructor_name:Newer.new",
                &cc_path,
                off1,
                len1,
                false,
            ),
            (
                "remove_constructor_name:Circle.unit",
                &sh_path,
                off2,
                len2,
                false,
            ),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.remove_constructor_name",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.remove_constructor_name",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [],
                    }],
                }),
                expect_non_empty: non_empty,
            });
        }
    }
    // 10g. add_import_prefix (dart.refactor_add.import_prefix)
    {
        let (off1, len1, _) = lsp_range(root, m, "import 'circle_card.dart';", 0, 0, 0);
        let (off2, len2, _) = lsp_range(root, cc, "import 'shapes.dart';", 0, 0, 0);
        for (lbl, path, off, len, pfx) in [
            ("add_import_prefix:circle_card", &m_path, off1, len1, "cc"),
            ("add_import_prefix:shapes", &cc_path, off2, len2, "sh"),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor_add.import_prefix",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor_add.import_prefix",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [pfx],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }
    // 10h. remove_import_prefix (dart.refactor.remove_import_prefix)
    {
        let (off1, len1, _) = lsp_range(root, m, "import 'shapes.dart' as s;", 0, 0, 0);
        let (off2, len2, _) = lsp_range(root, sh, "import 'dart:math' as math;", 0, 0, 0);
        for (lbl, path, off, len) in [
            ("remove_import_prefix:s", &m_path, off1, len1),
            ("remove_import_prefix:math", &sh_path, off2, len2),
        ] {
            steps.push(RefactorStep {
                category: "dart.refactor.remove_import_prefix",
                label: lbl.to_string(),
                request_method: "workspace/executeCommand",
                params: json!({
                    "command": "dart.refactor.remove_import_prefix",
                    "arguments": [{
                        "filePath": path,
                        "selectionOffset": off,
                        "selectionLength": len,
                        "arguments": [],
                    }],
                }),
                expect_non_empty: true,
            });
        }
    }

    steps
}

fn run_refactor_steps(program: &str, root: &Path, steps: &[RefactorStep]) -> Vec<Value> {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    if std::env::var_os("DARTR_LSP_STEP_TIMEOUT").is_none() {
        c.step_timeout = std::time::Duration::from_secs(900);
    }
    c.configuration = json!({"experimentalRefactors": true});
    c.show_message_choice = Some(0);
    let init = dart_code_initialize_params(root);
    let response = c.request("initialize", init);
    assert!(response["result"]["capabilities"].is_object(), "{response}");
    c.notify("initialized", json!({}));
    c.settle(true);
    for rel in [
        "app/lib/shapes.dart",
        "app/lib/shapes_part.dart",
        "app/lib/circle_card.dart",
        "app/lib/main.dart",
    ] {
        let p = root.join(rel);
        let text = std::fs::read_to_string(&p).unwrap();
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": file_uri(&p), "languageId": "dart", "version": 1, "text": text}}),
        );
    }
    c.settle(false);

    let mut out = Vec::new();
    for s in steps {
        let before = c.server_requests.len();
        let mut resp = strip(c.request(s.request_method, s.params.clone()));
        if s.request_method == "textDocument/codeAction" {
            if let Some(arr) = resp["result"].as_array_mut() {
                arr.retain(|a| {
                    let cmd = a
                        .pointer("/command/command")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
                    kind.starts_with("refactor") && cmd != "dart.logAction"
                });
            }
        }
        let apply_edits: Vec<Value> = c.server_requests[before..]
            .iter()
            .filter(|(m, _)| m == "workspace/applyEdit")
            .map(|(_, p)| sorted_edit(p))
            .collect();
        let prompts: Vec<Value> = c.server_requests[before..]
            .iter()
            .filter(|(m, _)| m == "window/showMessageRequest")
            .map(|(_, p)| p.clone())
            .collect();
        out.push(json!({
            "category": s.category,
            "label": s.label,
            "expect_non_empty": s.expect_non_empty,
            "response": sorted_edit(&resp),
            "apply_edits": apply_edits,
            "prompts": prompts,
        }));
    }
    let _ = c.shutdown_and_exit();
    out
}

#[test]
fn lsp_refactoring_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_project("lsp_refactoring");
    let steps = refactor_steps(&root);
    let dart = run_refactor_steps("dart", &root, &steps);
    let dartr = run_refactor_steps(dartr_bin(), &root, &steps);
    assert!(!dart.is_empty(), "dart refactor steps must not be empty");
    assert_eq!(dart.len(), dartr.len(), "refactor step count mismatch");

    let mut by_cat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut diffs = Vec::new();
    for (d, r) in dart.iter().zip(&dartr) {
        let cat = d["category"].as_str().unwrap_or("?").to_string();
        let label = d["label"].as_str().unwrap_or("?");
        if d["expect_non_empty"] == json!(true) {
            let has_resp =
                !d["response"]["result"].is_null() && d["response"]["result"] != json!([]);
            let has_edit = !d["apply_edits"].as_array().unwrap().is_empty();
            assert!(
                has_resp || has_edit,
                "Dart returned empty answer for {label}: {d}"
            );
        }
        let ok = d["response"] == r["response"]
            && d["apply_edits"] == r["apply_edits"]
            && d["prompts"] == r["prompts"];
        let entry = by_cat.entry(cat).or_default();
        entry.0 += ok as usize;
        entry.1 += 1;
        if !ok {
            diffs.push(label.to_string());
            eprintln!("DIFF {label}");
            for key in ["response", "apply_edits", "prompts"] {
                if d[key] != r[key] {
                    let ds = d[key].to_string();
                    let rs = r[key].to_string();
                    eprintln!(
                        "  {key}\n    dart:  {}\n    dartr: {}",
                        &ds[..ds.len().min(1200)],
                        &rs[..rs.len().min(1200)]
                    );
                }
            }
        }
    }
    eprintln!("refactoring parity: {} steps total", dart.len());
    for (cat, (ok, total)) in &by_cat {
        eprintln!(
            "  refactoring {:<48}: {:>2}/{:>2} identical",
            cat, ok, total
        );
    }
    assert!(
        diffs.is_empty(),
        "refactoring parity had {} differing steps: {diffs:?}",
        diffs.len()
    );
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
                    kind: "corpus",
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
                    expect_valid: false,
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
    compare(&label, &dart, &dartr, false);
}
