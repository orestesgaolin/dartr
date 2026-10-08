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

import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
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
      'errors': [...collector.errors.map(_error), _error(error)],
    };
  } catch (error) {
    // package:yaml exposes a few implementation exceptions (for example an
    // explicitly tagged empty integer). Keep one response per request so a
    // corpus run can report the behavior without losing protocol alignment.
    return {
      'node': null,
      'errors': collector.errors.map(_error).toList(),
      'runtimeError': error.toString(),
    };
  }
}

String _cleanUpLiteral(String text) {
  final lines = text.split('\n');
  if (lines.length <= 1) return text;
  for (var index = 0; index < lines.length; index++) {
    lines[index] = lines[index].length > 8 ? lines[index].substring(8) : '';
  }
  return lines.join('\n');
}

String _indentLiteral(String text) => text.contains('\n')
    ? text.split('\n').map((line) => '        $line').join('\n')
    : text;

String? _literal(
  Expression expression, [
  Map<String, String> variables = const {},
]) {
  if (expression is StringLiteral) return expression.stringValue;
  if (expression is SimpleIdentifier) return variables[expression.name];
  if (expression is ParenthesizedExpression) {
    return _literal(expression.expression, variables);
  }
  if (expression is MethodInvocation &&
      (expression.methodName.name == 'cleanUpLiteral' ||
          expression.methodName.name == 'indentLiteral') &&
      expression.argumentList.arguments.isNotEmpty) {
    final value = _literal(
      expression.argumentList.arguments.first.argumentExpression,
      variables,
    );
    if (value == null) return null;
    return expression.methodName.name == 'cleanUpLiteral'
        ? _cleanUpLiteral(value)
        : _indentLiteral(value);
  }
  return null;
}

class _YamlTestLiteralVisitor extends RecursiveAstVisitor<void> {
  final inputs = <String>[];
  final _variables = <String, String>{};

  @override
  void visitVariableDeclaration(VariableDeclaration node) {
    final initializer = node.initializer;
    if (initializer != null) {
      final value = _literal(initializer, _variables);
      if (value != null) _variables[node.name.lexeme] = value;
    }
    super.visitVariableDeclaration(node);
  }

  @override
  void visitMethodInvocation(MethodInvocation node) {
    final name = node.methodName.name;
    final arguments = node.argumentList.arguments;
    int? index;
    var clean = false;
    if (name == 'expectYamlFails') {
      index = 0;
      clean = true;
    } else if (name == 'expectYamlLoads' ||
        name == 'expectYamlStreamLoads' ||
        name == 'expectYamlLoadsWithWarning') {
      index = 1;
      clean = true;
    } else if (name == 'loadYaml' ||
        name == 'loadYamlNode' ||
        name == 'loadYamlDocument' ||
        name == 'loadYamlDocuments' ||
        name == 'loadYamlStream') {
      index = 0;
    }
    if (index != null && arguments.length > index) {
      final value = _literal(arguments[index].argumentExpression, _variables);
      if (value != null) inputs.add(clean ? _cleanUpLiteral(value) : value);
    }
    super.visitMethodInvocation(node);
  }
}

Map<String, Object?> _extractTestInputs(List<Object?> paths) {
  final visitor = _YamlTestLiteralVisitor();
  for (final path in paths) {
    if (path is! String)
      throw const FormatException('testFiles must be strings');
    parseString(
      content: File(path).readAsStringSync(),
      path: path,
    ).unit.accept(visitor);
  }
  return {'testInputs': visitor.inputs};
}

void main() {
  // Warnings are not part of this dump and would corrupt the JSONL protocol.
  yamlWarningCallback = (_, [__]) {};
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
          final testFiles = decoded['testFiles'];
          stdout.writeln(
            jsonEncode(
              testFiles is List
                  ? _extractTestInputs(testFiles)
                  : _load(decoded),
            ),
          );
        } catch (error) {
          stderr.writeln('yaml_oracle: $error');
          exitCode = 64;
        }
      });
}
