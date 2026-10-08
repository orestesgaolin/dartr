// Differential oracle for package:yaml 3.1.4.
//
// Run with the package configuration pinned by tools/oracle:
//   dart --packages="$DARTR_ORACLE_PACKAGES" tools/yaml_oracle.dart
//
// The protocol is JSON Lines. Each request is
// {"text":"...", "recover":false}. Each response contains `node` (null on
// an unrecovered error) and `errors`. Offsets and columns are UTF-16 code-unit
// counts, as reported by package:source_span.

import 'dart:convert';
import 'dart:io';

import 'package:source_span/source_span.dart';
import 'package:yaml/src/error_listener.dart';
import 'package:yaml/yaml.dart';

Map<String, Object> _location(SourceLocation location) => {
  'offset': location.offset,
  'line': location.line,
  'column': location.column,
};

Map<String, Object?>? _span(SourceSpan? span) {
  if (span == null) return null;
  return {'start': _location(span.start), 'end': _location(span.end)};
}

Map<String, Object?> _error(YamlException error) => {
  'message': error.message,
  'span': _span(error.span),
};

Map<String, Object?> _node(YamlNode node) {
  final common = <String, Object?>{'span': _span(node.span)};
  if (node is YamlScalar) {
    final value = node.value;
    String scalarType;
    Object? encodedValue = value;
    if (value == null) {
      scalarType = 'null';
    } else if (value is bool) {
      scalarType = 'bool';
    } else if (value is int) {
      scalarType = 'int';
    } else if (value is double) {
      scalarType = 'float';
      if (value.isNaN) {
        encodedValue = 'nan';
      } else if (value == double.infinity) {
        encodedValue = '+infinity';
      } else if (value == double.negativeInfinity) {
        encodedValue = '-infinity';
      }
    } else if (value is String) {
      scalarType = 'string';
    } else {
      throw StateError('Unexpected YAML scalar type ${value.runtimeType}');
    }
    return {
      ...common,
      'kind': 'scalar',
      'scalarType': scalarType,
      'value': encodedValue,
      'scalarStyle': node.style.name,
    };
  }
  if (node is YamlList) {
    return {
      ...common,
      'kind': 'list',
      'collectionStyle': node.style.name,
      'items': node.nodes.map(_node).toList(),
    };
  }
  if (node is YamlMap) {
    return {
      ...common,
      'kind': 'map',
      'collectionStyle': node.style.name,
      'entries': [
        for (final entry in node.nodes.entries)
          {'key': _node(entry.key as YamlNode), 'value': _node(entry.value)},
      ],
    };
  }
  throw StateError('Unexpected YAML node type ${node.runtimeType}');
}

Map<String, Object?> _load(Map<String, Object?> request) {
  final text = request['text'];
  final recover = request['recover'] ?? false;
  if (text is! String || recover is! bool) {
    throw FormatException('Expected {"text": string, "recover": bool}');
  }

  final collector = ErrorCollector();
  try {
    final node = loadYamlNode(
      text,
      recover: recover,
      errorListener: recover ? collector : null,
    );
    return {
      'node': _node(node),
      'errors': collector.errors.map(_error).toList(),
    };
  } on YamlException catch (error) {
    return {
      'node': null,
      'errors': [_error(error)],
    };
  }
}

void main() {
  stdin
      .transform(utf8.decoder)
      .transform(const LineSplitter())
      .where((line) => line.isNotEmpty)
      .forEach((line) {
        try {
          final decoded = jsonDecode(line);
          if (decoded is! Map<String, dynamic>) {
            throw const FormatException('Expected a JSON object');
          }
          stdout.writeln(jsonEncode(_load(decoded)));
        } catch (error) {
          stderr.writeln('yaml_oracle: $error');
          exitCode = 64;
        }
      });
}
