// Oracle for the `toSource()` round trip test of crates/dartr_ast.
//
// Writes one JSON object per input file (JSON Lines):
//   {"path": ..., "source": <unit.toSource()>}
// with the same parse as `oracle.dart ast` (`parseString`, latest language
// version, diagnostics ignored).
//
// Usage (the mode argument is optional, for the same command line as
// `oracle.dart <mode>`):
//   dart run bin/tosource.dart [tosource] [file ...]   (no files: read paths from stdin)
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';

Future<void> main(List<String> args) async {
  if (args.isNotEmpty && args.first == 'tosource') args = args.sublist(1);
  var paths = args.isNotEmpty
      ? args
      : await stdin
            .transform(utf8.decoder)
            .transform(const LineSplitter())
            .where((l) => l.isNotEmpty)
            .toList();
  for (var p in paths) {
    var path = File(p).absolute.path;
    Map<String, Object?> json;
    try {
      var result = parseString(
        content: File(path).readAsStringSync(),
        path: path,
        throwIfDiagnostics: false,
        featureSet: FeatureSet.latestLanguageVersion(),
      );
      json = {'path': path, 'source': result.unit.toSource()};
    } catch (e) {
      json = {'path': path, 'error': e.runtimeType.toString()};
    }
    stdout.writeln(jsonEncode(json));
  }
}
