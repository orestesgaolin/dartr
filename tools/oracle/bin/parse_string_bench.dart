// Measures scan + parse + AST build throughput of the analyzer's
// `parseString` (single thread): the Dart counterpart of
// `cargo run --release -p dartr_ast_builder --example build_throughput`.
//
//   dart compile exe tools/oracle/bin/parse_string_bench.dart -o target/oracle/parse_string_bench
//   target/oracle/parse_string_bench <dir>... [--iterations N]
//
// Each file is parsed with `parseString(content:, path:,
// throwIfDiagnostics: false)` and the latest language version, like the
// oracle `ast` mode.

// ignore_for_file: implementation_imports

import 'dart:io';

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';

final FeatureSet featureSet = FeatureSet.latestLanguageVersion();

int scanAndParse(String content) {
  var result = parseString(
    content: content,
    path: '/bench.dart',
    throwIfDiagnostics: false,
    featureSet: featureSet,
  );
  return result.unit.end ^ result.errors.length;
}

void main(List<String> args) {
  args = [...args];
  var iterations = 10;
  var i = args.indexOf('--iterations');
  if (i >= 0) {
    iterations = int.parse(args[i + 1]);
    args.removeRange(i, i + 2);
  }
  var files = <String>[];
  for (var a in args) {
    if (FileSystemEntity.isFileSync(a)) {
      files.add(a);
    } else {
      for (var e in Directory(a).listSync(recursive: true)) {
        if (e is File && e.path.endsWith('.dart')) files.add(e.path);
      }
    }
  }
  files.sort();
  var sources = <String>[];
  for (var f in files) {
    try {
      sources.add(File(f).readAsStringSync());
    } catch (_) {}
  }
  var bytes = 0;
  for (var s in sources) {
    bytes += _utf8Length(s);
  }

  // Warm up.
  var sink = 0;
  for (var s in sources) {
    sink ^= scanAndParse(s);
  }

  var best = double.infinity;
  var total = 0.0;
  for (var n = 0; n < iterations; n++) {
    var sw = Stopwatch()..start();
    for (var s in sources) {
      sink ^= scanAndParse(s);
    }
    var t = sw.elapsedMicroseconds / 1e6;
    if (t < best) best = t;
    total += t;
  }
  var mb = bytes / 1e6;
  var mean = total / iterations;
  print('files:      ${sources.length}');
  print('source:     ${mb.toStringAsFixed(2)} MB (UTF-8)');
  print('iterations: $iterations');
  print(
    'mean:       ${(mean * 1e3).toStringAsFixed(1)} ms/iteration, '
    '${(mb / mean).toStringAsFixed(1)} MB/s',
  );
  print(
    'best:       ${(best * 1e3).toStringAsFixed(1)} ms/iteration, '
    '${(mb / best).toStringAsFixed(1)} MB/s',
  );
  if (sink == 42) print('');
}

/// UTF-8 length of [s] (without a byte order mark), like the Rust side.
int _utf8Length(String s) {
  var n = 0;
  var start = s.startsWith('﻿') ? 1 : 0;
  for (var i = start; i < s.length; i++) {
    var c = s.codeUnitAt(i);
    if (c < 0x80) {
      n += 1;
    } else if (c < 0x800) {
      n += 2;
    } else if (c >= 0xD800 && c <= 0xDBFF && i + 1 < s.length) {
      n += 4;
      i++;
    } else {
      n += 3;
    }
  }
  return n;
}
