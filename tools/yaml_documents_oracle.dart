// Directed document/stream oracle for package:yaml 3.1.4.
// Run with tools/oracle's pinned package configuration.

import 'dart:convert';
import 'dart:io';

import 'package:source_span/source_span.dart';
import 'package:yaml/yaml.dart';

Map<String, Object> _location(SourceLocation location) => {
  'offset': location.offset,
  'line': location.line,
  'column': location.column,
};

Map<String, Object>? _span(SourceSpan? span) => span == null
    ? null
    : {'start': _location(span.start), 'end': _location(span.end)};

Object? _value(YamlNode node) {
  if (node is YamlScalar) return node.value;
  if (node is YamlList) return node.nodes.map(_value).toList();
  if (node is YamlMap) {
    return [
      for (final entry in node.nodes.entries)
        {'key': _value(entry.key as YamlNode), 'value': _value(entry.value)},
    ];
  }
  throw StateError('Unexpected node type ${node.runtimeType}');
}

Map<String, Object?> _document(YamlDocument document) => {
  'contents': _value(document.contents),
  'span': _span(document.span),
  'version': switch (document.versionDirective) {
    final version? => {'major': version.major, 'minor': version.minor},
    null => null,
  },
  'tags': [
    for (final tag in document.tagDirectives)
      {'handle': tag.handle, 'prefix': tag.prefix},
  ],
  'startImplicit': document.startImplicit,
  'endImplicit': document.endImplicit,
};

Map<String, Object?> _run(Map<String, Object?> request) {
  final text = request['text'];
  final operation = request['operation'];
  if (text is! String || operation is! String) {
    throw const FormatException('Expected text and operation strings');
  }
  try {
    return switch (operation) {
      'document' => {
        'result': _document(loadYamlDocument(text)),
        'error': null,
      },
      'documents' => {
        'result': loadYamlDocuments(text).map(_document).toList(),
        'error': null,
      },
      'stream' => {
        'result': {
          'contents': _value(loadYamlStream(text)),
          'span': _span(loadYamlStream(text).span),
        },
        'error': null,
      },
      _ => throw FormatException('Unknown operation $operation'),
    };
  } on YamlException catch (error) {
    return {
      'result': null,
      'error': {'message': error.message, 'span': _span(error.span)},
    };
  }
}

void main() {
  stdin
      .transform(utf8.decoder)
      .transform(const LineSplitter())
      .where((line) => line.isNotEmpty)
      .forEach((line) => stdout.writeln(jsonEncode(_run(jsonDecode(line)))));
}
