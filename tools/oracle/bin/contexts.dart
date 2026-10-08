// Oracle for the project model (phase 4).
//
// Builds an AnalysisContextCollection with the pinned package:analyzer, the
// same way `dart analyze` does (lint rules registered, SDK of the running VM),
// and writes one JSON object to stdout: the SDK, and for each analysis context
// its root, workspace, included and excluded paths, options file, package
// config, packages, effective analysis options and analyzed files.
//
// `dartr_project`'s `contexts_dump` writes the same format, so the two outputs
// can be compared byte for byte.
//
// Usage:
//   dart run bin/contexts.dart <path> [path ...]
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/file_system/file_system.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:analyzer/src/analysis_options/analysis_options.dart';
import 'package:analyzer/src/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/src/dart/analysis/context_root.dart';
import 'package:analyzer/src/dart/analysis/experiments.dart';
import 'package:analyzer/src/dart/sdk/sdk.dart';
import 'package:analyzer/src/util/sdk.dart';
import 'package:analyzer/src/workspace/basic.dart';
import 'package:analyzer/src/workspace/blaze.dart';
import 'package:analyzer/src/workspace/gn.dart';
import 'package:analyzer/src/workspace/pub.dart';
import 'package:analyzer/src/workspace/workspace.dart';
import 'package:linter/src/rules.dart';
import 'package:path/path.dart' as p;

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('usage: contexts <path> [path ...]');
    exit(64);
  }
  // `dart analyze` registers the lint rules before it builds contexts.
  registerLintRules();

  var includedPaths = args.map((a) => p.normalize(p.absolute(a))).toList();
  var resourceProvider = PhysicalResourceProvider.INSTANCE;
  var sdkPath = getSdkPath();
  var collection = AnalysisContextCollectionImpl(
    includedPaths: includedPaths,
    resourceProvider: resourceProvider,
    sdkPath: sdkPath,
  );

  var sdk = FolderBasedDartSdk(
    resourceProvider,
    resourceProvider.getFolder(sdkPath),
  );
  var out = <String, Object?>{
    'sdk': {
      'path': sdkPath,
      'version': sdk.sdkVersion,
      'language_version': _version(sdk.languageVersion),
      'libraries': {
        for (var library
            in sdk.sdkLibraries
              ..sort((a, b) => a.shortName.compareTo(b.shortName)))
          library.shortName: sdk.mapDartUri(library.shortName)?.fullName,
      },
    },
    'contexts': [
      for (var context in collection.contexts)
        _contextJson(context.contextRoot, context),
    ],
  };
  stdout.writeln(const JsonEncoder.withIndent(' ').convert(out));
  exit(0);
}

Map<String, Object?> _contextJson(ContextRootImpl root, dynamic context) {
  var driver = context.driver;
  var workspace = root.workspace;
  var optionsMap = driver.analysisOptionsMap;

  var files = <Map<String, Object?>>[];
  var analyzed = root.analyzedFiles().toList()..sort();
  for (var path in analyzed) {
    var entry = <String, Object?>{'path': path, 'kind': _fileKind(path)};
    if (_fileKind(path) == 'dart') {
      var file = driver.fsState.getFileForPath(path);
      entry['uri'] = file.uri.toString();
      entry['language_version'] = _version(file.packageLanguageVersion);
      entry['options_file'] = file.analysisOptions.file?.path;
    }
    files.add(entry);
  }

  var options = <Map<String, Object?>>[];
  List<Folder> folders = optionsMap.folders;
  if (folders.isEmpty) {
    options.add({'folder': null, ..._optionsJson(optionsMap.options.first)});
  } else {
    var sorted = folders.toList()..sort((a, b) => a.path.compareTo(b.path));
    for (var folder in sorted) {
      var folderOptions =
          optionsMap[folder.getFile('_')] as AnalysisOptionsImpl;
      options.add({'folder': folder.path, ..._optionsJson(folderOptions)});
    }
  }

  var sdk = driver.sourceFactory.dartSdk;
  Map<String, String?>? embedderLibraries;
  if (sdk is EmbedderSdk) {
    var names = sdk.sdkLibraries.map((l) => l.shortName).toList()..sort();
    embedderLibraries = {
      for (var name in names) name: sdk.mapDartUri(name)?.fullName,
    };
  }

  var packages = workspace.packages.packages.toList()
    ..sort((a, b) => a.name.compareTo(b.name));

  return {
    'root': root.root.path,
    'workspace': _workspaceJson(workspace),
    'included': root.includedPaths.toList(),
    'excluded': root.excludedPaths.toList(),
    'options_file': root.optionsFile?.path,
    'package_config': root.packagesFile?.path,
    'packages': [
      for (var package in packages)
        {
          'name': package.name,
          'root': package.rootFolder.path,
          'lib': package.libFolder.path,
          'language_version': package.languageVersion == null
              ? null
              : _version(package.languageVersion!),
        },
    ],
    'embedder_libraries': embedderLibraries,
    'options': options,
    'files': files,
    'fix_data': _fixDataFiles(root.root),
  };
}

/// The `fix_data.yaml` files that the analysis server analyzes for a context
/// (see `ContextManagerImpl._createAnalysisContexts`).
List<String> _fixDataFiles(Folder root) {
  var result = <String>[];
  var lib = root.getFolder('lib');
  var file = lib.getFile('fix_data.yaml');
  if (file.exists) result.add(file.path);
  void walk(Folder folder) {
    List<Resource> children;
    try {
      children = folder.getChildren();
    } on FileSystemException {
      return;
    }
    for (var child in children) {
      if (child is File) {
        if (child.shortName.endsWith('.yaml')) result.add(child.path);
      } else if (child is Folder) {
        walk(child);
      }
    }
  }

  var folder = lib.getFolder('fix_data');
  if (folder.exists) walk(folder);
  result.sort();
  return result;
}

String _fileKind(String path) {
  var name = p.basename(path);
  if (name == 'analysis_options.yaml') return 'analysis_options';
  if (name == 'pubspec.yaml') return 'pubspec';
  if (name == 'AndroidManifest.xml') return 'android_manifest';
  if (p.extension(path) == '.dart') return 'dart';
  return 'other';
}

Map<String, Object?> _workspaceJson(Workspace workspace) {
  String kind;
  bool? isPubWorkspace;
  if (workspace is PackageConfigWorkspace) {
    kind = 'package_config';
    isPubWorkspace = workspace.isPubWorkspace;
  } else if (workspace is BasicWorkspace) {
    kind = 'basic';
  } else if (workspace is BlazeWorkspace) {
    kind = 'blaze';
  } else if (workspace is GnWorkspace) {
    kind = 'gn';
  } else {
    kind = workspace.runtimeType.toString();
  }
  return {
    'kind': kind,
    'root': workspace.root,
    'is_pub_workspace': ?isPubWorkspace,
  };
}

Map<String, Object?> _optionsJson(AnalysisOptionsImpl options) {
  var features = options.contextFeatures;
  var experiments = [
    for (var feature in ExperimentStatus.knownFeatures.values)
      if (!feature.isEnabledByDefault &&
          !feature.isExpired &&
          features.isEnabled(feature))
        feature.enableString,
  ]..sort();
  return {
    'file': options.file?.path,
    'errors': [
      for (var processor in options.errorProcessors)
        {'code': processor.code, 'severity': processor.severity?.name},
    ],
    'exclude': options.excludePatterns,
    'strict_casts': options.strictCasts,
    'strict_inference': options.strictInference,
    'strict_raw_types': options.strictRawTypes,
    'experiments': experiments,
    'lint': options.lint,
    'lint_rules': options.lintRules.map((r) => r.name).toList()..sort(),
    'legacy_plugins': options.enabledLegacyPluginNames,
    'plugins': [
      for (var c in options.pluginsOptions.configurations)
        {
          'name': c.name,
          'source': _pluginSourceJson(c.source),
          'diagnostics': {
            for (var e in c.diagnosticConfigs.entries)
              e.key: e.value.severity.name,
          },
        },
    ],
    'plugin_dependency_overrides': options.pluginsOptions.dependencyOverrides
        ?.map((k, v) => MapEntry(k, _pluginSourceJson(v))),
    'cannot_ignore': options.unignorableDiagnosticCodeNames.toList()..sort(),
    'formatter_page_width': options.formatterOptions.pageWidth,
    'formatter_trailing_commas': options.formatterOptions.trailingCommas?.name,
    'code_style_format': options.codeStyleOptions.useFormatter,
    'chrome_os_manifest_checks': options.chromeOsManifestChecks,
    'propagate_linter_exceptions': options.propagateLinterExceptions,
  };
}

Map<String, Object?> _pluginSourceJson(PluginSource source) {
  return switch (source) {
    VersionedPluginSource() => {
      'kind': 'version',
      'constraint': source.constraint,
      'hosted': source.hostedUrl,
    },
    GitPluginSource() => {
      'kind': 'git',
      'url': source.url,
      'path': source.path,
      'ref': source.ref,
      'tag_pattern': source.tagPattern,
    },
    PathPluginSource() => {'kind': 'path', 'path': source.path},
  };
}

String _version(dynamic version) => '${version.major}.${version.minor}';
