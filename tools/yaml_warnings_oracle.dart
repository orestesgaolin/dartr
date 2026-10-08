// Warning oracle for package:yaml 3.1.4.

import 'dart:convert';
import 'dart:io';

import 'package:source_span/source_span.dart';
import 'package:yaml/src/error_listener.dart';
import 'package:yaml/src/utils.dart';
import 'package:yaml/yaml.dart';

Map<String, Object> _location(SourceLocation location) => {
  'offset': location.offset,
  'line': location.line,
  'column': location.column,
};

Map<String, Object>? _span(SourceSpan? span) => span == null
    ? null
    : {'start': _location(span.start), 'end': _location(span.end)};

Map<String, Object?> _run(Map<String, Object?> request) {
  final text = request['text'];
  final recover = request['recover'];
  if (text is! String || recover is! bool) {
    throw const FormatException('Expected text and recover fields');
  }
  final warnings = <Map<String, Object?>>[];
  final errors = ErrorCollector();
  yamlWarningCallback = (message, [span]) {
    warnings.add({'message': message, 'span': _span(span)});
  };
  String? fatal;
  try {
    loadYamlNode(
      text,
      recover: recover,
      errorListener: recover ? errors : null,
    );
  } on YamlException catch (error) {
    fatal = error.message;
  } catch (error) {
    fatal = error.toString();
  }
  return {
    'warnings': warnings,
    'recoveredErrors': errors.errors.length,
    'fatal': fatal,
  };
}

void main() {
  stdin
      .transform(utf8.decoder)
      .transform(const LineSplitter())
      .where((line) => line.isNotEmpty)
      .forEach((line) => stdout.writeln(jsonEncode(_run(jsonDecode(line)))));
}
