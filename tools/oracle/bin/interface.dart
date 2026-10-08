// Oracle mode `interface`: the interfaces of the classes, enums, mixins and
// extension types of each library, from `InheritanceManager3.getInterface`
// (see docs/design/semantics.md §5.2). `dartr dump interface` writes the
// same format.
//
// One JSON line per input path (or `dart:` URI, as in mode `elements`). The
// path must be the defining unit of a library; otherwise the line is
// `{"path":p,"error":"<result.runtimeType>"}`. An exception gives
// `{"path":p,"error":"<exception type>"}`.
//
// Line format (keys in this order):
//   {"path", "uri", "interfaces": [
//     {"n": <element name>, "k": "class"|"enum"|"mixin"|"extensionType",
//      "map": [[<Name>, R(member), <type>], ...],
//      "implemented": [...], "inherited": [...],
//      "forwarders": [<Name>, ...],
//      "conflicts": [{"k": <Conflict class>, "n": <Name>,
//                     "members": [R(member), ...]}, ...]}]}
//
// - Elements are in the order of `LibraryElement.classes`, `enums`,
//   `mixins`, `extensionTypes`.
// - <Name> is `Name.toString()` (`<libraryUri>::_name` for private names);
//   entries are sorted by it (code unit order), so the output does not
//   depend on map order.
// - R(member) is R(member.baseElement) of mode `elements` (substituted
//   members print their declaration; members that the inheritance manager
//   synthesizes for a combined signature print as members of the target
//   element). <type> is `member.type.getDisplayString()`, so substitution
//   and combined signatures (`topMerge`) are visible.
// - "inherited" is `getInheritedMap(element)`; "forwarders" is
//   `noSuchMethodForwarders`, sorted.
// - "members" of a conflict, by kind: CandidatesConflict and
//   NotUniqueExtensionMemberConflict: the candidates; GetterMethodConflict:
//   getter, method; HasNonExtensionAndExtensionMemberConflict: nonExtension,
//   then extension; ExtensionTypeConflictingInheritedMethodAndSetterConflict:
//   method, setter; ExtensionTypeConflictingStaticAndInstanceConflict:
//   declared, inherited. Conflicts keep the order of `Interface.conflicts`.

import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/element/element.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/element/inheritance_manager3.dart';

import 'elements.dart' show oracleSdkPath, ref;

Future<void> dumpInterface(List<String> inputs) async {
  var sdkPath = oracleSdkPath();
  var paths = inputs.where((p) => !p.startsWith('dart:')).toList();
  var collection = AnalysisContextCollection(
    includedPaths: paths,
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    sdkPath: sdkPath,
  );
  // `dart:` URIs: one context rooted at an empty temporary folder (see
  // `dumpElements`).
  AnalysisContextCollection? sdkCollection;
  Directory? emptyRoot;
  for (var p in inputs) {
    Map<String, Object?> json;
    try {
      if (p.startsWith('dart:')) {
        emptyRoot ??= Directory.systemTemp.createTempSync('dartr_oracle_');
        sdkCollection ??= AnalysisContextCollection(
          includedPaths: [emptyRoot.resolveSymbolicLinksSync()],
          resourceProvider: PhysicalResourceProvider.INSTANCE,
          sdkPath: sdkPath,
        );
        var session = sdkCollection.contexts.first.currentSession;
        var result = await session.getLibraryByUri(p);
        if (result is LibraryElementResult) {
          json = interfaceLibraryJson(p, result.element);
        } else {
          json = {'path': p, 'error': result.runtimeType.toString()};
        }
      } else {
        var context = collection.contextFor(p);
        var result = await context.currentSession.getResolvedLibrary(p);
        if (result is ResolvedLibraryResult) {
          json = interfaceLibraryJson(p, result.element);
        } else {
          json = {'path': p, 'error': result.runtimeType.toString()};
        }
      }
    } catch (e) {
      json = {'path': p, 'error': e.runtimeType.toString()};
    }
    stdout.writeln(jsonEncode(json));
  }
  emptyRoot?.deleteSync(recursive: true);
}

Map<String, Object?> interfaceLibraryJson(String path, LibraryElement library) {
  return {
    'path': path,
    'uri': library.uri.toString(),
    'interfaces': [
      for (var e in library.classes) interfaceJson(e, 'class'),
      for (var e in library.enums) interfaceJson(e, 'enum'),
      for (var e in library.mixins) interfaceJson(e, 'mixin'),
      for (var e in library.extensionTypes) interfaceJson(e, 'extensionType'),
    ],
  };
}

Map<String, Object?> interfaceJson(InterfaceElement element, String kind) {
  element as InterfaceElementImpl;
  var manager = element.inheritanceManager;
  var interface = manager.getInterface(element);
  return {
    'n': element.name,
    'k': kind,
    'map': nameMapJson(interface.map),
    'implemented': nameMapJson(interface.implemented),
    'inherited': nameMapJson(manager.getInheritedMap(element)),
    'forwarders': [
      for (var name in interface.noSuchMethodForwarders) name.toString(),
    ]..sort(),
    'conflicts': [for (var c in interface.conflicts) conflictJson(c)],
  };
}

List<List<String?>> nameMapJson(Map<Name, ExecutableElement> map) {
  var entries = map.entries.toList()
    ..sort((a, b) => a.key.toString().compareTo(b.key.toString()));
  return [
    for (var entry in entries)
      [
        entry.key.toString(),
        ref(entry.value.baseElement),
        entry.value.type.getDisplayString(),
      ],
  ];
}

Map<String, Object?> conflictJson(Conflict conflict) {
  List<ExecutableElement> members = switch (conflict) {
    CandidatesConflict c => c.candidates,
    NotUniqueExtensionMemberConflict c => c.candidates,
    GetterMethodConflict c => [c.getter, c.method],
    HasNonExtensionAndExtensionMemberConflict c => [
      ...c.nonExtension,
      ...c.extension,
    ],
    ExtensionTypeConflictingInheritedMethodAndSetterConflict c => [
      c.method,
      c.setter,
    ],
    ExtensionTypeConflictingStaticAndInstanceConflict c => [
      c.declared,
      c.inherited,
    ],
    _ => const [],
  };
  return {
    'k': conflict.runtimeType.toString(),
    'n': conflict.name.toString(),
    'members': [for (var m in members) ref(m.baseElement)],
  };
}
