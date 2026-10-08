// Lists every diagnostic code of the pinned analyzer (third_party/dart-sdk,
// tag 3.13.3) as JSON: the analyzer, linter and analysis_server codes, and
// the CFE codes of the shared scanner and parser.
//
// The messages are formatted with fixed sample arguments, one per parameter,
// chosen by parameter type and index. `crates/dartr_diagnostics/tests/parity.rs`
// produces the same JSON from Rust and compares.
//
// Run with tools/regen_diagnostics.sh (it needs a package config that also
// maps package:analysis_server, which pub cannot resolve here).
//
// Sample arguments (index i is the position of the parameter):
//   analyzer codes:
//     String / name           -> "s<i>"
//     int                     -> 100 + i
//     Uri                     -> package:p<i>/l<i>.dart
//     Object                  -> "o<i>"
//     DartType                -> the type of `class T<i> {}`
//     Element                 -> the element of `class E<i> {}`
//     Token                   -> identifier token with lexeme "tok<i>"
//   CFE codes:
//     String                  -> the letter 'A' + i (Character needs one rune)
//     int (int and Unicode)   -> 31 + i
//     Token                   -> identifier token with lexeme "tok<i>"
import 'dart:convert';
import 'dart:io';
import 'dart:mirrors';

import 'package:_fe_analyzer_shared/src/messages/codes.dart' as cfe;
import 'package:_fe_analyzer_shared/src/messages/diagnostic.dart' as cfe_diag;
import 'package:_fe_analyzer_shared/src/scanner/token.dart';
import 'package:analysis_server/src/diagnostic.dart' as server_diag;
import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/type.dart';
import 'package:analyzer/diagnostic/diagnostic.dart';
import 'package:analyzer/error/error.dart';
import 'package:analyzer/error/listener.dart';
import 'package:analyzer/src/diagnostic/diagnostic.dart' as analyzer_diag;
import 'package:analyzer/src/diagnostic/diagnostic_code_values.dart'
    show sharedAnalyzerCodes;
import 'package:analyzer/src/string_source.dart';
import 'package:linter/src/diagnostic.dart' as linter_diag;

late List<DartType> sampleTypes;
late List<Element> sampleElements;
final source = StringSource('', '/sample.dart');

Future<void> main(List<String> args) async {
  // Keep the imports used: the libraries are enumerated with mirrors.
  analyzer_diag.abstractClassMember;
  linter_diag.avoidPrint;
  server_diag.conflictingKey;
  cfe_diag.abstractClassMember;

  await _resolveSamples();

  var codes = <Map<String, Object?>>[];
  var seen = <DiagnosticCode>{};
  for (var (uri, origin) in [
    ('package:analyzer/src/diagnostic/diagnostic.dart', 'analyzer'),
    ('package:linter/src/diagnostic.dart', 'linter'),
    ('package:analysis_server/src/diagnostic.dart', 'analysis_server'),
  ]) {
    for (var value in _topLevelValues(uri)) {
      if (value is DiagnosticCode && seen.add(value)) {
        codes.add(_codeJson(value, origin));
      }
    }
  }
  // Sanity check: the analyzer codes are exactly `diagnosticCodeValues`.
  var analyzerCodes = seen.where((c) => !_isLinterOrServer(c)).toSet();
  if (analyzerCodes.length != diagnosticCodeValues.length ||
      !diagnosticCodeValues.every(analyzerCodes.contains)) {
    throw StateError('analyzer codes differ from diagnosticCodeValues');
  }
  codes.sort(
    (a, b) => (a['uniqueName'] as String).compareTo(b['uniqueName'] as String),
  );

  var cfeCodes = <Map<String, Object?>>[];
  for (var value in _topLevelValues(
    'package:_fe_analyzer_shared/src/messages/diagnostic.dart',
  )) {
    if (value is cfe.Code) cfeCodes.add(_cfeJson(value));
  }
  cfeCodes.sort((a, b) => (a['name'] as String).compareTo(b['name'] as String));

  var out = {
    'sdkVersion': Platform.version.split(' ').first,
    'codeCount': codes.length,
    'cfeCodeCount': cfeCodes.length,
    'codes': codes,
    'cfeCodes': cfeCodes,
  };
  stdout.writeln(const JsonEncoder.withIndent(' ').convert(out));
}

final _linterAndServer = <DiagnosticCode>{};

bool _isLinterOrServer(DiagnosticCode c) => _linterAndServer.contains(c);

Iterable<Object?> _topLevelValues(String uri) sync* {
  var library = currentMirrorSystem().libraries[Uri.parse(uri)]!;
  var names = library.declarations.values
      .whereType<VariableMirror>()
      .where((v) => v.isStatic || v.isTopLevel)
      .map((v) => v.simpleName)
      .toList();
  for (var name in names) {
    yield library.getField(name).reflectee;
  }
}

Future<void> _resolveSamples() async {
  var dir = Directory.systemTemp.createTempSync('dartr_diag_samples');
  try {
    var file = File('${dir.path}/samples.dart');
    file.writeAsStringSync(
      [for (var i = 0; i < 8; i++) 'class T$i {}\nclass E$i {}\n'].join(),
    );
    var collection = AnalysisContextCollection(
      includedPaths: [dir.resolveSymbolicLinksSync()],
    );
    var path = File(file.path).resolveSymbolicLinksSync();
    var context = collection.contextFor(path);
    var result =
        await context.currentSession.getResolvedUnit(path) as ResolvedUnitResult;
    var classes = result.libraryElement.classes;
    ClassElement named(String n) => classes.firstWhere((c) => c.name == n);
    sampleTypes = [for (var i = 0; i < 8; i++) named('T$i').thisType];
    sampleElements = [for (var i = 0; i < 8; i++) named('E$i')];
  } finally {
    dir.deleteSync(recursive: true);
  }
}

Token _token(int i) => StringToken(TokenType.IDENTIFIER, 'tok$i', 0);

Object _analyzerSample(String type, int i) => switch (type) {
  'String' || 'name' || 'string' => 's$i',
  'int' => 100 + i,
  'Uri' || 'uri' => Uri.parse('package:p$i/l$i.dart'),
  'Object' || 'object' => 'o$i',
  'DartType' || 'type' => sampleTypes[i],
  'Element' || 'element' => sampleElements[i],
  'Token' || 'token' => _token(i),
  _ => throw StateError('no sample for $type'),
};

Map<String, Object?> _codeJson(DiagnosticCode code, String origin) {
  if (origin != 'analyzer') _linterAndServer.add(code);
  var expectedTypes = code is analyzer_diag.DiagnosticCodeWithExpectedTypes
      ? code.expectedTypes?.map((e) => e.name).toList()
      : null;

  // Build the arguments.
  List<Object> arguments;
  if (code is analyzer_diag.DiagnosticWithArguments) {
    var function = (code as analyzer_diag.DiagnosticWithArguments).withArguments
        as Function;
    var parameters = (reflect(function) as ClosureMirror).function.parameters;
    var named = <Symbol, Object>{};
    for (var (i, p) in parameters.indexed) {
      named[p.simpleName] = _analyzerSample(
        MirrorSystem.getName(p.type.simpleName),
        i,
      );
    }
    var locatable =
        Function.apply(function, const [], named)
            as analyzer_diag.LocatableDiagnostic;
    arguments = locatable.arguments;
  } else {
    arguments = [
      for (var (i, t) in (expectedTypes ?? const <String>[]).indexed)
        _analyzerSample(t, i),
    ];
  }
  if (arguments.length != code.numParameters) {
    throw StateError('${code.lowerCaseUniqueName}: argument count mismatch');
  }

  // Format like the analyzer does.
  Diagnostic diagnostic;
  if (arguments.any((a) => a is Token)) {
    // Codes with token arguments are reported by FastaErrorReporter, which
    // uses Diagnostic.tmp without type conversion.
    diagnostic = Diagnostic.tmp(
      source: source,
      offset: 0,
      length: 0,
      diagnosticCode: code,
      arguments: arguments,
    );
  } else {
    var listener = RecordingDiagnosticListener();
    var reporter = DiagnosticReporter(listener, source);
    diagnostic = reporter.atOffset(
      offset: 0,
      length: 0,
      diagnosticCode: code,
      arguments: arguments,
    );
  }

  return {
    'origin': origin,
    // ignore: deprecated_member_use
    'name': code.name,
    // ignore: deprecated_member_use
    'uniqueName': code.uniqueName,
    'lowerCaseName': code.lowerCaseName,
    'lowerCaseUniqueName': code.lowerCaseUniqueName,
    'type': code.type.name,
    'severity': code.severity.name,
    'isIgnorable': code.isIgnorable,
    'hasPublishedDocs': code.hasPublishedDocs,
    'isUnresolvedIdentifier': code.isUnresolvedIdentifier,
    'url': code.url,
    'numParameters': code.numParameters,
    'expectedTypes': expectedTypes,
    'problemMessageTemplate': code.problemMessage,
    'correctionMessageTemplate': code.correctionMessage,
    'problemMessage': diagnostic.message,
    'correctionMessage': diagnostic.correctionMessage,
    'diagnosticSeverity': diagnostic.severity.name,
    'contextMessages': diagnostic.contextMessages.length,
  };
}

Object _cfeSample(String type, int i) => switch (type) {
  'String' => String.fromCharCode(0x41 + i),
  'int' => 31 + i,
  'Token' => _token(i),
  _ => throw StateError('no CFE sample for $type'),
};

Map<String, Object?> _cfeJson(cfe.Code code) {
  cfe.Message message;
  if (code is cfe.MessageCode) {
    message = code;
  } else if (code is cfe.Template) {
    var function = code.withArguments as Function;
    var parameters = (reflect(function) as ClosureMirror).function.parameters;
    var named = <Symbol, Object>{};
    for (var (i, p) in parameters.indexed) {
      named[p.simpleName] = _cfeSample(
        MirrorSystem.getName(p.type.simpleName),
        i,
      );
    }
    message = Function.apply(function, const [], named) as cfe.Message;
  } else {
    throw StateError('unknown code kind: $code');
  }

  Map<String, Object?>? analyzer;
  if (code.sharedCode case var sharedCode?) {
    var analyzerCode = sharedAnalyzerCodes[sharedCode.index];
    var diagnostic = Diagnostic.tmp(
      source: source,
      offset: 0,
      length: 0,
      diagnosticCode: analyzerCode,
      arguments: message.arguments.values.toList(),
    );
    analyzer = {
      'uniqueName': analyzerCode.lowerCaseUniqueName,
      'problemMessage': diagnostic.message,
      'correctionMessage': diagnostic.correctionMessage,
    };
  }

  return {
    'name': code.name,
    'severity': code.severity.name,
    'sharedCode': code.sharedCode?.name,
    'pseudoSharedCode': code.pseudoSharedCode?.name,
    'problemMessage': message.problemMessage,
    'correctionMessage': message.correctionMessage,
    'arguments': message.arguments.length,
    'analyzer': analyzer,
  };
}
