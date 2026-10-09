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
//   tokens   token stream and scanner diagnostics (scanner only, no parser)
//   events   parser events of the shared parser (see events.dart)
//   ast      unresolved AST (parser output) and parse diagnostics
//   resolved diagnostics and static types of expressions of a resolved
//            library (resolved_el.dart)
//   resolved-el the elements of identifiers, named types and constructor
//            names of a resolved library (resolved_el.dart)
//   elements the element model of a library (elements.dart)
//   interface the interfaces of the classes of a library (interface.dart)
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/syntactic_entity.dart';
import 'package:analyzer/dart/ast/token.dart';
import 'package:analyzer/diagnostic/diagnostic.dart';
import 'package:analyzer/error/listener.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/scanner/scanner.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/scanner/translate_error_token.dart';
// ignore: implementation_imports
import 'package:analyzer/src/error/listener.dart';
// ignore: implementation_imports
import 'package:analyzer/src/string_source.dart';
// ignore: implementation_imports
import 'package:_fe_analyzer_shared/src/scanner/error_token.dart';

import 'elements.dart';
import 'events.dart';
import 'interface.dart';
import 'resolved_el.dart';

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln('usage: oracle <tokens|events|ast|resolved|resolved-el|elements|interface> [file ...]');
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
  // `dart:` URIs (modes `elements`, `interface`, `resolved`, `resolved-el`)
  // are kept as they are.
  paths = paths
      .map((p) => p.startsWith('dart:') ? p : File(p).absolute.path)
      .toList();

  switch (mode) {
    case 'tokens':
      for (var p in paths) {
        stdout.writeln(jsonEncode(guarded(p, dumpTokens)));
      }
    case 'events':
      for (var p in paths) {
        stdout.writeln(jsonEncode(guarded(p, dumpEvents)));
      }
    case 'ast':
      for (var p in paths) {
        stdout.writeln(jsonEncode(guarded(p, dumpAst)));
      }
    case 'resolved':
      await dumpResolvedLibraries(paths, elements: false);
    case 'resolved-el':
      await dumpResolvedLibraries(paths, elements: true);
    case 'elements':
      await dumpElements(paths);
    case 'interface':
      await dumpInterface(paths);
    default:
      stderr.writeln('unknown mode: $mode');
      exit(64);
  }
}

/// Runs [dump] for [path]; on an exception (for example a file that is not
/// valid UTF-8) returns `{"path": ..., "error": <exception type>}` so that one
/// file does not stop the batch.
Map<String, Object?> guarded(
  String path,
  Map<String, Object?> Function(String) dump,
) {
  try {
    return dump(path);
  } catch (e) {
    return {'path': path, 'error': e.runtimeType.toString()};
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

/// Scanner output only (no parser), as `parseString` sees it before parsing.
///
/// `parseString` scans with the analyzer `Scanner`, and the parser then skips
/// the error tokens at the start of the stream (`Parser.parseUnit`) and
/// reports them with `translateErrorToken`. This function does the same
/// without the parser, so that the parser cannot change the token stream
/// (for example by splitting `>>` or inserting synthetic tokens) and parse
/// diagnostics are not included.
Map<String, Object?> dumpTokens(String path) {
  var content = File(path).readAsStringSync();
  var featureSet = FeatureSet.latestLanguageVersion();
  var listener = RecordingDiagnosticListener();
  var reporter = DiagnosticReporter(listener, StringSource(content, path));
  var scanner = Scanner(inputText: content, reportError: reporter.report)
    ..configureFeatures(featureSetForOverriding: featureSet, featureSet: featureSet);
  Token first = scanner.tokenize();
  var errorTokens = <ErrorToken>[];
  while (first is ErrorToken) {
    errorTokens.add(first);
    first = first.next!;
  }
  for (var e in errorTokens) {
    translateErrorToken(e, reporter.report);
  }
  var tokens = <Object?>[];
  Token? t = first;
  while (t != null) {
    tokens.add(tokenJson(t, withComments: true));
    if (t.type == TokenType.EOF) break;
    t = t.next;
  }
  return {
    'path': path,
    'tokens': tokens,
    'diagnostics': listener.diagnostics.map(diagnosticJson).toList(),
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
