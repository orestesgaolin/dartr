// Oracle for package:yaml 3.1.3 syntax errors.
// Sources:
//   package:yaml/src/parser.dart
//   package:yaml/src/scanner.dart
//
// Reads a JSON array of YAML strings from stdin and writes a JSON array with
// each parse error's message and source span. A valid document has a null
// result at the same index.

import 'dart:convert';
import 'dart:io';

import 'package:yaml/yaml.dart';

void main() {
  var inputs = (jsonDecode(stdin.readLineSync()!) as List).cast<String>();
  var results = <Map<String, Object?>?>[];
  for (var input in inputs) {
    try {
      loadYamlNode(input);
      results.add(null);
    } on YamlException catch (error) {
      var span = error.span;
      results.add({
        'message': error.message,
        'offset': span?.start.offset,
        'length': span?.length,
        'line': span?.start.line,
        'column': span?.start.column,
      });
    }
  }
  stdout.writeln(jsonEncode(results));
}
