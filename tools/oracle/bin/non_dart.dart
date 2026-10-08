// Oracle for raw non-Dart diagnostics reported by the analysis server.
// Source: pkg/analysis_server/lib/src/context_manager.dart
//
// Usage:
//   dart --packages=.dart_tool/package_config.json bin/non_dart.dart <path>

import 'dart:convert';
import 'dart:io';

import 'package:analyzer/diagnostic/diagnostic.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:analyzer/source/error_processor.dart';
import 'package:analyzer/source/file_source.dart';
import 'package:analyzer/src/analysis_options/analysis_options.dart';
import 'package:analyzer/src/analysis_options/analysis_options_parser.dart';
import 'package:analyzer/src/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/src/dart/analysis/driver.dart';
import 'package:analyzer/src/manifest/manifest_validator.dart';
import 'package:analyzer/src/pubspec/pubspec_validator.dart';
import 'package:analyzer/src/util/file_paths.dart' as file_paths;
import 'package:analyzer/src/util/sdk.dart';
import 'package:analyzer/src/workspace/pub.dart';
import 'package:linter/src/rules.dart';
import 'package:path/path.dart' as p;
import 'package:yaml/yaml.dart';

void main(List<String> arguments) {
  if (arguments.isEmpty) {
    stderr.writeln('usage: non_dart <path> [path ...]');
    exit(64);
  }

  registerLintRules();
  var provider = PhysicalResourceProvider.INSTANCE;
  var includedPaths = arguments
      .map((argument) => p.normalize(p.absolute(argument)))
      .toList();
  var collection = AnalysisContextCollectionImpl(
    includedPaths: includedPaths,
    resourceProvider: provider,
    sdkPath: getSdkPath(),
  );
  var parseSession = AnalysisOptionsParseSession();
  var output = <Map<String, Object?>>[];

  for (var context in collection.contexts) {
    var driver = context.driver;
    for (var path in context.contextRoot.analyzedFiles()) {
      if (file_paths.isAnalysisOptionsYaml(provider.pathContext, path)) {
        var package = context.contextRoot.workspace.findPackageFor(path);
        output.addAll(
          _analysisOptionsDiagnostics(
            provider: provider,
            driver: driver,
            path: path,
            parseSession: parseSession,
            package: package is PubPackage ? package : null,
          ),
        );
      } else if (file_paths.isAndroidManifestXml(provider.pathContext, path)) {
        output.addAll(
          _manifestDiagnostics(provider: provider, driver: driver, path: path),
        );
      } else if (file_paths.isPubspecYaml(provider.pathContext, path)) {
        output.addAll(
          _pubspecDiagnostics(provider: provider, driver: driver, path: path),
        );
      }
    }
  }

  output.sort(_compareDiagnostics);
  stdout.writeln(jsonEncode(output));
}

List<Map<String, Object?>> _analysisOptionsDiagnostics({
  required PhysicalResourceProvider provider,
  required AnalysisDriver driver,
  required String path,
  required AnalysisOptionsParseSession parseSession,
  required PubPackage? package,
}) {
  try {
    var file = provider.getFile(path);
    var result = parseSession.parse(
      sourceFactory: driver.sourceFactory,
      contextRoot: driver.currentSession.analysisContext.contextRoot.root,
      file: file,
      sdkVersionConstraint: package?.sdkVersionConstraint,
    );
    if (result.content == null) return const [];
    return _convert(result.diagnostics, result.analysisOptions);
  } catch (_) {
    return const [];
  }
}

List<Map<String, Object?>> _pubspecDiagnostics({
  required PhysicalResourceProvider provider,
  required AnalysisDriver driver,
  required String path,
}) {
  try {
    var file = provider.getFile(path);
    var content = file.readAsStringSync();
    YamlNode node = loadYamlNode(content, sourceUrl: file.toUri());
    if (node is! YamlMap) node = YamlMap();
    var options = driver.getAnalysisOptionsForFile(file);
    var diagnostics = validatePubspec(
      contents: node,
      source: FileSource(file),
      provider: provider,
      analysisOptions: options,
    );
    return _convert(diagnostics, options);
  } catch (_) {
    return const [];
  }
}

List<Map<String, Object?>> _manifestDiagnostics({
  required PhysicalResourceProvider provider,
  required AnalysisDriver driver,
  required String path,
}) {
  try {
    var file = provider.getFile(path);
    var content = file.readAsStringSync();
    var source = FileSource(file);
    var options = driver.getAnalysisOptionsForFile(file);
    var diagnostics = ManifestValidator(
      source,
    ).validate(content, options.chromeOsManifestChecks);
    return _convert(diagnostics, options);
  } catch (_) {
    return const [];
  }
}

List<Map<String, Object?>> _convert(
  Iterable<Diagnostic> diagnostics,
  AnalysisOptionsImpl options,
) {
  var result = <Map<String, Object?>>[];
  for (var diagnostic in diagnostics) {
    var processor = ErrorProcessor.getProcessor(options, diagnostic);
    if (processor != null && processor.severity == null) continue;
    var severity = processor?.severity ?? diagnostic.diagnosticCode.severity;
    result.add({
      'file': diagnostic.source.fullName,
      'code': diagnostic.diagnosticCode.lowerCaseName,
      'severity': severity.name,
      'offset': diagnostic.offset,
      'length': diagnostic.length,
      'message': diagnostic.problemMessage.messageText(includeUrl: false),
      'correction': diagnostic.correctionMessage,
    });
  }
  return result;
}

int _compareDiagnostics(
  Map<String, Object?> first,
  Map<String, Object?> second,
) {
  var result = (first['file'] as String).compareTo(second['file'] as String);
  if (result != 0) return result;
  result = (first['offset'] as int).compareTo(second['offset'] as int);
  if (result != 0) return result;
  result = (first['code'] as String).compareTo(second['code'] as String);
  if (result != 0) return result;
  return jsonEncode(first).compareTo(jsonEncode(second));
}
