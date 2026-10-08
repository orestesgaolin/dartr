// Oracle for differential tests.
//
// Uses the pinned package:analyzer (third_party/dart-sdk, tag 3.13.3) and
// writes one JSON object per input file to stdout (JSON Lines). `dartr dump`
// writes the same format, so the two outputs can be compared byte for byte.
//
// Usage:
//   dart run bin/oracle.dart <mode> [file ...]   (no files: read paths from stdin)
//
// Modes:
//   tokens   token stream and scanner diagnostics
//   ast      unresolved AST (parser output) and parse diagnostics
//   resolved diagnostics of a resolved unit, and static types of expressions
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/syntactic_entity.dart';
import 'package:analyzer/dart/ast/token.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/diagnostic/diagnostic.dart';
import 'package:analyzer/file_system/physical_file_system.dart';

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln('usage: oracle <tokens|ast|resolved> [file ...]');
    exit(64);
  }
  var mode = args.first;
  var files = args.length > 1
      ? args.sublist(1)
      : stdin
            .transform(utf8.decoder)
            .transform(const LineSplitter())
            .where((l) => l.isNotEmpty)
            .toList();
  var paths = files is List<String> ? files : await (files as Future<List<String>>);
  paths = paths.map((p) => File(p).absolute.path).toList();

  switch (mode) {
    case 'tokens':
      for (var p in paths) {
        stdout.writeln(jsonEncode(dumpTokens(p)));
      }
    case 'ast':
      for (var p in paths) {
        stdout.writeln(jsonEncode(dumpAst(p)));
      }
    case 'resolved':
      await dumpResolved(paths);
    default:
      stderr.writeln('unknown mode: $mode');
      exit(64);
  }
}

ParseStringResult parse(String path) {
  var content = File(path).readAsStringSync();
  return parseString(
    content: content,
    path: path,
    throwIfDiagnostics: false,
    featureSet: FeatureSet.latestLanguageVersion(),
  );
}

Map<String, Object?> dumpTokens(String path) {
  var result = parse(path);
  var tokens = <Object?>[];
  Token? t = result.unit.beginToken;
  while (t != null) {
    tokens.add(tokenJson(t, withComments: true));
    if (t.type == TokenType.EOF) break;
    t = t.next;
  }
  return {
    'path': path,
    'tokens': tokens,
    'diagnostics': result.errors.map(diagnosticJson).toList(),
  };
}

Map<String, Object?> dumpAst(String path) {
  var result = parse(path);
  return {
    'path': path,
    'ast': nodeJson(result.unit),
    'diagnostics': result.errors.map(diagnosticJson).toList(),
  };
}

Future<void> dumpResolved(List<String> paths) async {
  var collection = AnalysisContextCollection(
    includedPaths: paths,
    resourceProvider: PhysicalResourceProvider.INSTANCE,
  );
  for (var p in paths) {
    var context = collection.contextFor(p);
    var result = await context.currentSession.getResolvedUnit(p);
    if (result is! ResolvedUnitResult) {
      stdout.writeln(jsonEncode({'path': p, 'error': result.runtimeType.toString()}));
      continue;
    }
    var types = <Object?>[];
    result.unit.accept(_TypeCollector(types));
    stdout.writeln(
      jsonEncode({
        'path': p,
        'diagnostics': result.diagnostics.map(diagnosticJson).toList(),
        'types': types,
      }),
    );
  }
}

Map<String, Object?> tokenJson(Token t, {bool withComments = false}) {
  var json = <String, Object?>{
    'k': t.type.name,
    'o': t.offset,
    'l': t.length,
    'x': t.lexeme,
  };
  if (t.isSynthetic) json['syn'] = true;
  if (withComments) {
    var comments = <Object?>[];
    Token? c = t.precedingComments;
    while (c != null) {
      comments.add(tokenJson(c));
      c = c.next;
    }
    if (comments.isNotEmpty) json['c'] = comments;
  }
  return json;
}

Object? entityJson(SyntacticEntity e) =>
    e is AstNode ? nodeJson(e) : tokenJson(e as Token);

Map<String, Object?> nodeJson(AstNode node) {
  var name = node.runtimeType.toString();
  if (name.endsWith('Impl')) name = name.substring(0, name.length - 4);
  return {
    't': name,
    'o': node.offset,
    'e': node.end,
    'c': node.childEntities.map(entityJson).toList(),
  };
}

Map<String, Object?> diagnosticJson(Diagnostic d) => {
  'code': d.diagnosticCode.lowerCaseName,
  'severity': d.severity.name,
  'o': d.offset,
  'l': d.length,
  'msg': d.message,
};

class _TypeCollector extends GeneralizingAstVisitor<void> {
  final List<Object?> out;
  _TypeCollector(this.out);

  @override
  void visitExpression(Expression node) {
    var type = node.staticType;
    if (type != null) {
      out.add({'o': node.offset, 'e': node.end, 'type': type.getDisplayString()});
    }
    super.visitExpression(node);
  }
}
