//! Differential tests of `textDocument/completion`: the same requests at
//! sampled positions against `dart language-server` and
//! `dartr language-server`, on a fixture project and (with
//! `DARTR_LSP_COMPLETION_CORPUS=<folder>[:<every>[:<max files>]]`) on the
//! files of a real project. Prints per position class how many lists are
//! identical in labels, kinds, sort texts and insert texts, and the labels
//! that differ most often.
//!
//! Snippet items (kind 15) are unranked and compared separately.
//!
//! `DARTR_LSP_COMPLETION_DUMP=<folder>` writes the responses of each server;
//! with `DARTR_LSP_COMPLETION_REUSE_DART=1` the `dart` responses are read
//! from that folder when they exist.

mod lsp_support;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use lsp_support::*;
use serde_json::{Value, json};

/// A position: (class, line, UTF-16 column).
type Position = (&'static str, u32, u32);

/// The responses: (file URI, position, response).
type Responses = Vec<(String, Position, Value)>;

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

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// The code of [text] with comments and strings blanked (same length),
/// except that the strings of `import`/`export`/`part` directives keep a
/// marker so that the import class can find them.
fn code_mask(text: &str) -> Vec<char> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = chars.clone();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                out[i] = ' ';
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] != '\n' {
                    out[i] = ' ';
                }
                i += 1;
            }
            if i < chars.len() {
                out[i] = ' ';
                out[i + 1] = ' ';
                i += 2;
            }
        } else if c == '\'' || c == '"' {
            let triple = chars.get(i + 1) == Some(&c) && chars.get(i + 2) == Some(&c);
            let len = if triple { 3 } else { 1 };
            let start = i;
            i += len;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == c
                    && (!triple || (chars.get(i + 1) == Some(&c) && chars.get(i + 2) == Some(&c)))
                {
                    i += len;
                    break;
                }
                if !triple && chars[i] == '\n' {
                    break;
                }
                i += 1;
            }
            for (k, ch) in out
                .iter_mut()
                .enumerate()
                .take(i.min(chars.len()))
                .skip(start)
            {
                if chars[k] != '\n' {
                    *ch = 'S';
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// (line, UTF-16 column) of each char index.
fn positions_of(text: &str) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let (mut line, mut col) = (0u32, 0u32);
    for c in text.chars() {
        out.push((line, col));
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += c.len_utf16() as u32;
        }
    }
    out.push((line, col));
    out
}

/// The sampled positions of [text] by class: every [every]-th match of each
/// class.
fn sample_positions(text: &str, every: usize) -> Vec<Position> {
    let chars: Vec<char> = text.chars().collect();
    let mask = code_mask(text);
    let pos = positions_of(text);
    let mut by_class: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();
    let mut add = |class: &'static str, i: usize| by_class.entry(class).or_default().push(i);
    let mut depth = 0i32;
    let word_before = |i: usize| -> String {
        let mut j = i;
        while j > 0 && is_ident_char(mask[j - 1]) {
            j -= 1;
        }
        mask[j..i].iter().collect()
    };
    for i in 0..chars.len() {
        let c = mask[i];
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
        }
        // After `.` of a member access (not `..`, not a number).
        if c == '.'
            && i > 0
            && (is_ident_char(mask[i - 1]) || mask[i - 1] == ')')
            && mask
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == '_')
            && !mask[i - 1].is_ascii_digit()
            && mask.get(i.wrapping_sub(1)) != Some(&'.')
        {
            add("dot", i + 1);
            // With a prefix of one character.
            add("dot-prefix", i + 2);
        }
        // After `(` of an invocation, and after `, ` in arguments.
        if c == '(' && i > 0 && is_ident_char(mask[i - 1]) && depth > 0 {
            let w = word_before(i);
            if !matches!(w.as_str(), "if" | "for" | "while" | "switch" | "catch") {
                add("args", i + 1);
            }
        }
        if c == ',' && mask.get(i + 1) == Some(&' ') && depth > 0 {
            add("args", i + 2);
        }
        // At the start of a statement.
        if depth >= 2 && c != ' ' && c != '\n' && c != '}' && i > 0 {
            let mut j = i;
            while j > 0 && mask[j - 1] == ' ' {
                j -= 1;
            }
            if j > 0 && mask[j - 1] == '\n' {
                let mut k = j - 1;
                while k > 0 && (mask[k - 1] == ' ' || mask[k - 1] == '\n') {
                    k -= 1;
                }
                if k > 0 && matches!(mask[k - 1], ';' | '{' | '}') && is_ident_char(c) {
                    add("statement", i);
                    if mask.get(i + 1).is_some_and(|n| is_ident_char(*n))
                        && mask.get(i + 2).is_some_and(|n| is_ident_char(*n))
                    {
                        add("statement-prefix", i + 2);
                    }
                }
            }
        }
        // In the URI of a directive.
        if (c == '\'' || c == '"' || c == 'S') && i > 7 {
            let before: String = chars[i.saturating_sub(7)..i].iter().collect();
            if (before.ends_with("import ") || before.ends_with("export "))
                && (chars[i] == '\'' || chars[i] == '"')
            {
                add("import", i + 1);
                // After `package:` or `dart:`.
                let rest: String = chars[i + 1..chars.len().min(i + 40)].iter().collect();
                if let Some(colon) = rest.find(':') {
                    add("import", i + 2 + colon);
                }
            }
        }
        // After `new ` / `const `.
        if c == ' ' && i >= 3 {
            let w = word_before(i);
            if (w == "new" || w == "const")
                && mask.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic())
            {
                add("new-const", i + 1);
            }
            if w == "case" && depth > 0 {
                add("pattern", i + 1);
            }
        }
    }
    let mut out = Vec::new();
    for (class, indexes) in by_class {
        for (n, &i) in indexes.iter().enumerate() {
            if n % every == 0 {
                let (l, col) = pos[i.min(pos.len() - 1)];
                out.push((class, l, col));
            }
        }
    }
    out
}

fn run(program: &str, root: &Path, files: &[PathBuf], every: usize) -> Responses {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    if std::env::var_os("DARTR_LSP_STEP_TIMEOUT").is_none() {
        c.step_timeout = std::time::Duration::from_secs(900);
    }
    let mut init = dart_code_initialize_params(root);
    // A large budget, so that the not-imported pass is complete.
    init["initializationOptions"]["completionBudgetMilliseconds"] = json!(600000);
    let response = c.request("initialize", init);
    assert!(response["result"]["capabilities"].is_object(), "{response}");
    c.notify("initialized", json!({}));
    c.settle(true);
    let mut out = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(file).unwrap();
        let uri = file_uri(file);
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
        );
        for p in sample_positions(&text, every) {
            let params = json!({
                "textDocument": {"uri": uri},
                "position": {"line": p.1, "character": p.2},
                "context": {"triggerKind": 1},
            });
            let mut response = c.request("textDocument/completion", params);
            if let Some(o) = response.as_object_mut() {
                o.remove("jsonrpc");
                o.remove("id");
            }
            out.push((uri.clone(), p, response));
        }
        c.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": uri}}),
        );
    }
    let _ = c.shutdown_and_exit();
    out
}

fn run_or_reuse(
    program: &str,
    label: &str,
    root: &Path,
    files: &[PathBuf],
    every: usize,
) -> Responses {
    let dump = std::env::var_os("DARTR_LSP_COMPLETION_DUMP").map(PathBuf::from);
    let server = if program == "dart" { "dart" } else { "dartr" };
    let path = dump
        .as_ref()
        .map(|d| d.join(format!("{label}.{server}.json")));
    if server == "dart" && std::env::var_os("DARTR_LSP_COMPLETION_REUSE_DART").is_some() {
        if let Some(text) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            let value: Value = serde_json::from_str(&text).unwrap();
            return value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    let class: &'static str =
                        Box::leak(v[1].as_str().unwrap().to_string().into_boxed_str());
                    (
                        v[0].as_str().unwrap().to_string(),
                        (
                            class,
                            v[2].as_u64().unwrap() as u32,
                            v[3].as_u64().unwrap() as u32,
                        ),
                        v[4].clone(),
                    )
                })
                .collect();
        }
    }
    let out = run(program, root, files, every);
    if let Some(path) = path {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let value: Vec<Value> = out
            .iter()
            .map(|(u, p, r)| json!([u, p.0, p.1, p.2, r]))
            .collect();
        std::fs::write(
            path,
            serde_json::to_string_pretty(&Value::Array(value)).unwrap(),
        )
        .unwrap();
    }
    out
}

/// The ranked items of a response (without snippets) and the number of
/// snippets.
fn items(response: &Value) -> (Vec<Value>, usize) {
    let all = response["result"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let snippets = all.iter().filter(|i| i["kind"] == json!(15)).count();
    (
        all.into_iter().filter(|i| i["kind"] != json!(15)).collect(),
        snippets,
    )
}

/// The text that an item inserts.
fn insert_text(item: &Value) -> Value {
    item.get("textEditText")
        .or_else(|| item.pointer("/textEdit/newText"))
        .or_else(|| item.get("insertText"))
        .cloned()
        .unwrap_or_else(|| item["label"].clone())
}

#[derive(Default)]
struct ClassStats {
    total: usize,
    labels: usize,
    label_set: usize,
    kinds: usize,
    sort_texts: usize,
    insert_texts: usize,
    identical: usize,
    defaults: usize,
    snippets_same: usize,
    /// Label → (missing in dartr, extra in dartr).
    diffs: HashMap<String, (usize, usize)>,
    examples: Vec<String>,
}

fn compare(
    label: &str,
    dart: &Responses,
    dartr: &Responses,
) -> BTreeMap<&'static str, (usize, usize)> {
    println!("== {label}");
    let mut stats: BTreeMap<&'static str, ClassStats> = BTreeMap::new();
    for ((uri, pos, a), (_, _, b)) in dart.iter().zip(dartr) {
        let s = stats.entry(pos.0).or_default();
        s.total += 1;
        let (ia, sa) = items(a);
        let (ib, sb) = items(b);
        if sa == sb {
            s.snippets_same += 1;
        }
        let field = |items: &[Value], f: &dyn Fn(&Value) -> Value| -> Vec<Value> {
            items.iter().map(f).collect()
        };
        let la = field(&ia, &|i| i["label"].clone());
        let lb = field(&ib, &|i| i["label"].clone());
        if la == lb {
            s.labels += 1;
        }
        let mut sa_set: Vec<String> = la.iter().map(|v| v.to_string()).collect();
        let mut sb_set: Vec<String> = lb.iter().map(|v| v.to_string()).collect();
        sa_set.sort();
        sb_set.sort();
        if sa_set == sb_set {
            s.label_set += 1;
        }
        if field(&ia, &|i| json!([i["label"], i["kind"]]))
            == field(&ib, &|i| json!([i["label"], i["kind"]]))
        {
            s.kinds += 1;
        }
        if field(&ia, &|i| json!([i["label"], i["sortText"]]))
            == field(&ib, &|i| json!([i["label"], i["sortText"]]))
        {
            s.sort_texts += 1;
        }
        if field(&ia, &|i| json!([i["label"], insert_text(i)]))
            == field(&ib, &|i| json!([i["label"], insert_text(i)]))
        {
            s.insert_texts += 1;
        }
        if ia == ib {
            s.identical += 1;
        }
        if a["result"]["itemDefaults"] == b["result"]["itemDefaults"]
            && a["result"]["isIncomplete"] == b["result"]["isIncomplete"]
        {
            s.defaults += 1;
        }
        // Label differences (multisets).
        let mut count: HashMap<String, i64> = HashMap::new();
        for l in &sa_set {
            *count.entry(l.clone()).or_default() += 1;
        }
        for l in &sb_set {
            *count.entry(l.clone()).or_default() -= 1;
        }
        for (l, n) in count {
            if n > 0 {
                s.diffs.entry(l).or_default().0 += 1;
            } else if n < 0 {
                s.diffs.entry(l).or_default().1 += 1;
            }
        }
        if ia != ib && s.examples.len() < 3 {
            let first_diff = ia
                .iter()
                .zip(&ib)
                .position(|(x, y)| x != y)
                .unwrap_or(ia.len().min(ib.len()));
            s.examples.push(format!(
                "{uri}:{}:{} ({} vs {} items), first difference at {first_diff}:\n      dart:  {}\n      dartr: {}",
                pos.1 + 1,
                pos.2 + 1,
                ia.len(),
                ib.len(),
                ia.get(first_diff).map(|v| v.to_string()).unwrap_or_default().chars().take(400).collect::<String>(),
                ib.get(first_diff).map(|v| v.to_string()).unwrap_or_default().chars().take(400).collect::<String>(),
            ));
        }
    }
    let mut counts = BTreeMap::new();
    for (class, s) in &stats {
        println!(
            "  {class}: {} positions; identical {} | labels in order {} | label set {} | +kinds {} | +sortText {} | +insert text {} | defaults {} | snippet count {}",
            s.total,
            s.identical,
            s.labels,
            s.label_set,
            s.kinds,
            s.sort_texts,
            s.insert_texts,
            s.defaults,
            s.snippets_same
        );
        let mut diffs: Vec<(&String, &(usize, usize))> = s.diffs.iter().collect();
        diffs.sort_by(|a, b| (b.1.0 + b.1.1).cmp(&(a.1.0 + a.1.1)).then(a.0.cmp(b.0)));
        let top: Vec<String> = diffs
            .iter()
            .take(12)
            .map(|(l, (m, e))| format!("{l} -{m}/+{e}"))
            .collect();
        if !top.is_empty() {
            println!(
                "    most different labels (missing/extra in dartr): {}",
                top.join(", ")
            );
        }
        for e in &s.examples {
            println!("    {e}");
        }
        counts.insert(*class, (s.identical, s.total));
    }
    counts
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

/// The fixture: a package with a library, an import of `dart:math`, a
/// second library, and code with member accesses, invocations, statements,
/// directives, instance creations and patterns.
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
library;

import 'dart:math' as math;

/// A shape with an area.
abstract class Shape {
  /// The area of the shape.
  double get area;

  /// The name of the shape.
  String describe({bool verbose = false}) => 'shape';
}

/// A circle.
class Circle extends Shape {
  final double radius;
  const Circle(this.radius);
  Circle.unit() : radius = 1;

  @override
  double get area => math.pi * radius * radius;

  Circle scale(double factor, [int times = 1]) => Circle(radius * factor);
}

class Square implements Shape {
  final double side;
  Square(this.side);

  @override
  double get area => side * side;

  @override
  String describe({bool verbose = false}) => 'square';
}

enum Color { red, green, blue }

extension ShapeList on List<Shape> {
  double get totalArea => fold(0, (a, b) => a + b.area);
  Shape? largest() => isEmpty ? null : reduce((a, b) => a.area > b.area ? a : b);
}

typedef Point = ({int x, int y});

sealed class Result<T> {}

class Ok<T> extends Result<T> {
  final T value;
  Ok(this.value);
}

class Err<T> extends Result<T> {
  final String message;
  Err(this.message);
}

mixin Named {
  String get name;
}
"#,
    );
    write(
        "app/lib/main.dart",
        r#"import 'dart:async';
import 'dart:collection';
import 'package:app/shapes.dart';

int counter = 0;

void log(String message, {int level = 0, required bool flag}) {}

Future<int> compute(int input) async {
  final value = await Future.value(input);
  return value * 2;
}

class Canvas with Named {
  final List<Shape> shapes = [];
  @override
  String get name => 'canvas';

  void add(Shape shape) {
    shapes.add(shape);
    log(shape.describe(verbose: true), flag: false);
  }

  double paint(Color color) {
    var total = shapes.totalArea;
    final queue = Queue<Shape>();
    queue.addFirst(Circle(2));
    switch (color) {
      case Color.red:
        total += 1;
      case Color.green || Color.blue:
        total -= 1;
    }
    return total;
  }
}

String describe(Result<int> result) {
  return switch (result) {
    Ok(value: var v) when v > 0 => 'ok $v',
    Ok() => 'zero',
    Err(:var message) => message,
  };
}

void main() {
  var canvas = Canvas();
  canvas.add(const Circle(1));
  canvas.add(Square(2));
  final circle = Circle.unit().scale(2, 3);
  print(circle.radius);
  final Point p = (x: 1, y: 2);
  print(p.x + p.y);
  final shapes = <Shape>[circle, Square(1)];
  print(shapes.largest()?.area);
  if (shapes case [Circle c, ...]) {
    print(c.radius);
  }
  final timer = Timer(const Duration(seconds: 1), () {});
  timer.cancel();
  counter++;
  compute(counter).then((v) => print(v));
  final map = <String, int>{'a': 1};
  for (final entry in map.entries) {
    print(entry.key);
  }
}
"#,
    );
    root
}

#[test]
fn lsp_completion_parity() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_project("lsp_completion");
    let files = dart_files(&root);
    let dart = run_or_reuse("dart", "fixture", &root, &files, 1);
    let dartr = run_or_reuse(dartr_bin(), "fixture", &root, &files, 1);
    compare("fixture project", &dart, &dartr);
}

/// The completion results of [program] at [positions] (path, line,
/// column; the files are not opened) when `initialize` has only a
/// `rootUri` (no `workspaceFolders`), after the analysis.
fn run_root_uri_only(program: &str, root: &Path, positions: &[(&Path, u32, u32)]) -> Vec<Value> {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    let mut init = dart_code_initialize_params(root);
    init.as_object_mut().unwrap().remove("workspaceFolders");
    init["initializationOptions"]["completionBudgetMilliseconds"] = json!(600000);
    let response = c.request("initialize", init);
    assert!(response["result"]["capabilities"].is_object(), "{response}");
    c.notify("initialized", json!({}));
    c.settle(true);
    let mut out = Vec::new();
    for (path, line, character) in positions {
        let params = json!({
            "textDocument": {"uri": file_uri(path)},
            "position": {"line": line, "character": character},
            "context": {"triggerKind": 1},
        });
        out.push(c.request("textDocument/completion", params));
    }
    let _ = c.shutdown_and_exit();
    out
}

/// `initialize` with only `rootUri` (no `workspaceFolders`): completion in a
/// file under the root, and in a file that no root contains (not open; Dart
/// still resolves it, but gives no snippets because the file is not in the
/// root of its context). Both lists must be the same as the lists of Dart.
#[test]
fn lsp_completion_root_uri_only() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = write_project("lsp_completion_root_uri");
    let outside = root.join("outside/lib/x.dart");
    std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
    std::fs::write(root.join("outside/pubspec.yaml"), "name: outside\n").unwrap();
    std::fs::write(&outside, "void f() {\n  \n}\n").unwrap();
    let app = root.join("app");
    let main = app.join("lib/main.dart");
    let positions = [(main.as_path(), 47u32, 2u32), (outside.as_path(), 1, 2)];
    let dart = run_root_uri_only("dart", &app, &positions);
    let dartr = run_root_uri_only(dartr_bin(), &app, &positions);
    let labels = |r: &Value| -> Vec<String> {
        r["result"]["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|i| i["label"].as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    eprintln!(
        "under the root: dart {} items, dartr {} items; outside: dart {} items, dartr {} items",
        labels(&dart[0]).len(),
        labels(&dartr[0]).len(),
        labels(&dart[1]).len(),
        labels(&dartr[1]).len()
    );
    assert!(!labels(&dart[0]).is_empty(), "{}", dart[0]);
    assert_eq!(labels(&dart[0]), labels(&dartr[0]));
    assert_eq!(dart[1], dartr[1]);
}

/// `DARTR_LSP_COMPLETION_CORPUS=<folder>[:<every>[:<max files>]]`.
#[test]
fn lsp_completion_corpus() {
    let Some(spec) = std::env::var_os("DARTR_LSP_COMPLETION_CORPUS") else {
        eprintln!("skipped: set DARTR_LSP_COMPLETION_CORPUS=<folder>[:<every>[:<max files>]]");
        return;
    };
    let spec = spec.to_string_lossy().to_string();
    let mut parts = spec.split(':');
    let root = PathBuf::from(parts.next().unwrap()).canonicalize().unwrap();
    let every: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(10);
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
    let label = format!(
        "{}-{every}-{max_files}",
        root.file_name().unwrap().to_string_lossy()
    );
    let dart = run_or_reuse("dart", &label, &root, &files, every);
    let dartr = run_or_reuse(dartr_bin(), &label, &root, &files, every);
    compare(&format!("corpus {}", root.display()), &dart, &dartr);
}
