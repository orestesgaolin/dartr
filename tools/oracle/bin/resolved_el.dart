// Oracle modes `resolved-el` and `resolved`: the resolution of each library
// (see docs/design/semantics.md §5.4). `dartr dump resolved-el` and
// `dartr dump resolved` (crates/dartr/src/resolved.rs) write the same format.
//
// One JSON line per input path. The input is the defining unit of a library
// or a `dart:` URI, grouped by analysis context like mode `elements`
// (elements.dart). The library is resolved with `getResolvedLibrary` (for a
// `dart:` URI: `getLibraryByUri` and `getResolvedLibraryByElement` in a
// context rooted at an empty folder). For a part file (or a file that is not
// a library) the line is `{"path":p,"error":"<result.runtimeType>"}`; an
// exception gives `{"path":p,"error":"<exception type>"}`.
//
// Line format (keys in this order):
//   {"path": <input>, "uri": <library URI>, "units": [<unit>, ...]}
// The units are `ResolvedLibraryResult.units`: the defining unit, then the
// parts (depth first, in `part` directive order). `<unit>`:
//   resolved-el: {"path": <file path>, "nodes": [<node>, ...]}
//   resolved:    {"path": <file path>, "diagnostics": [...], "types": [...]}
//
// The nodes of a unit are visited in pre-order over `childEntities` (the
// order of the `ast` dump), after resolution (with the AST rewrites of the
// resolver).
//
// `<node>` (resolved-el), for every `SimpleIdentifier`, `NamedType` and
// `ConstructorName` (and their subclasses):
//   {"o": offset, "e": end, "k": <node kind>, "el": <element ref>,
//    "member": <substitution>}
// - `"k"`: the runtime class name without `Impl` (as `"t"` of the `ast`
//   dump), for example `SimpleIdentifier`.
// - `"el"`: `node.element`, written with [elRef]: JSON null for no element;
//   for a local element `"local:<kind>:<name>@<offset>"`; else R(e) of
//   elements.dart (`"<libraryUri>::<path>"`, `"<no-library>::dynamic"`,
//   `"<no-library>::Never"`, `"<multiply-defined>::<name>"`; an import
//   prefix is `"<libraryUri>::<prefix>"`). For a substituted member
//   (`SubstitutedElementImpl`) the reference is that of `baseElement`.
// - `"member"`: only for a substituted member: the entries of its
//   substitution map as `"<type parameter name>: <type display string>"`,
//   without the type parameters of the base element itself (fresh type
//   parameters of a generic method), sorted, joined by `", "`. The key is
//   left out when no entry is left.
//
// Local elements: `<kind>` is `variable` (LocalVariableElement),
// `patternVariable` (a BindPatternVariableElement, or another
// PatternVariableElement), `joinPatternVariable`, `function`
// (LocalFunctionElement, also a function expression), `label`, `parameter`
// (a FormalParameterElement) and `typeParameter`; a formal parameter or a
// type parameter is local when one of its enclosing elements is a local
// variable, local function or label. `<name>` is `element.name` (empty for
// null), `<offset>` is `firstFragment.nameOffset ?? firstFragment.offset`.
//
// `"diagnostics"` (resolved): `ResolvedUnitResult.diagnostics`, in the format
// of the `ast` mode (`code`, `severity`, `o`, `l`, `msg`).
//
// `"types"` (resolved), for every `Expression` with a non-null `staticType`:
//   {"o": offset, "e": end, "k": <node kind>, "type": <display string>,
//    "inv": <staticInvokeType>, "targs": [<typeArgumentTypes>]}
// `"inv"` and `"targs"` only for an `InvocationExpression`; a null value is
// JSON null. Types are `type.getDisplayString()`.
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/syntactic_entity.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/type.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/element/member.dart';
import 'package:path/path.dart' as path;

import 'elements.dart' show oracleSdkPath, ref;
import 'oracle.dart' show diagnosticJson;

/// Writes one line per input for mode `resolved-el` ([elements] true) or
/// `resolved` ([elements] false).
Future<void> dumpResolvedLibraries(
  List<String> inputs, {
  required bool elements,
}) async {
  Map<String, Object?> unitJson(ResolvedUnitResult unit) {
    var nodes = <AstNode>[];
    _preOrder(unit.unit, nodes);
    if (elements) {
      return {
        'path': unit.path,
        'nodes': [
          for (var n in nodes)
            if (n is SimpleIdentifier || n is NamedType || n is ConstructorName)
              _nodeElementJson(n),
        ],
      };
    }
    return {
      'path': unit.path,
      'diagnostics': unit.diagnostics.map(diagnosticJson).toList(),
      'types': [
        for (var n in nodes)
          if (n is Expression && n.staticType != null) _nodeTypeJson(n),
      ],
    };
  }

  await forEachResolvedLibrary(inputs, (p, result) {
    return {
      'path': p,
      'uri': result.element.uri.toString(),
      'units': [for (var u in result.units) unitJson(u)],
    };
  });
}

/// Resolves the library of each input and writes `toJson(input, result)`
/// (or an error line) to stdout. Grouping and errors as in
/// `dumpElements` (elements.dart).
Future<void> forEachResolvedLibrary(
  List<String> inputs,
  Map<String, Object?> Function(String, ResolvedLibraryResult) toJson,
) async {
  var sdkPath = oracleSdkPath();
  var paths = inputs
      .where((p) => !p.startsWith('dart:') && p == path.normalize(p))
      .toList();
  var collection = AnalysisContextCollection(
    includedPaths: paths,
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    sdkPath: sdkPath,
  );
  AnalysisContextCollection? sdkCollection;
  Directory? emptyRoot;
  for (var p in inputs) {
    Map<String, Object?> json;
    try {
      SomeResolvedLibraryResult result;
      if (p.startsWith('dart:')) {
        emptyRoot ??= Directory.systemTemp.createTempSync('dartr_oracle_');
        sdkCollection ??= AnalysisContextCollection(
          includedPaths: [emptyRoot.resolveSymbolicLinksSync()],
          resourceProvider: PhysicalResourceProvider.INSTANCE,
          sdkPath: sdkPath,
        );
        var session = sdkCollection.contexts.first.currentSession;
        var library = await session.getLibraryByUri(p);
        if (library is LibraryElementResult) {
          result = await session.getResolvedLibraryByElement(library.element);
        } else {
          stdout.writeln(
            jsonEncode({'path': p, 'error': library.runtimeType.toString()}),
          );
          continue;
        }
      } else {
        var context = collection.contextFor(p);
        result = await context.currentSession.getResolvedLibrary(p);
      }
      if (result is ResolvedLibraryResult) {
        json = toJson(p, result);
      } else {
        json = {'path': p, 'error': result.runtimeType.toString()};
      }
    } catch (e) {
      json = {'path': p, 'error': e.runtimeType.toString()};
    }
    stdout.writeln(jsonEncode(json));
  }
  emptyRoot?.deleteSync(recursive: true);
}

/// Pre-order over `childEntities` (iterative: deeply nested code).
void _preOrder(AstNode root, List<AstNode> out) {
  var stack = <AstNode>[root];
  while (stack.isNotEmpty) {
    var node = stack.removeLast();
    out.add(node);
    var children = <AstNode>[
      for (SyntacticEntity e in node.childEntities)
        if (e is AstNode) e,
    ];
    for (var i = children.length - 1; i >= 0; i--) {
      stack.add(children[i]);
    }
  }
}

String _kind(AstNode node) {
  var name = node.runtimeType.toString();
  if (name.endsWith('Impl')) name = name.substring(0, name.length - 4);
  return name;
}

Map<String, Object?> _nodeElementJson(AstNode node) {
  var element = switch (node) {
    SimpleIdentifier n => n.element,
    NamedType n => n.element,
    ConstructorName n => n.element,
    _ => null,
  };
  var json = <String, Object?>{
    'o': node.offset,
    'e': node.end,
    'k': _kind(node),
    'el': elRef(element),
  };
  var member = memberString(element);
  if (member != null) json['member'] = member;
  return json;
}

Map<String, Object?> _nodeTypeJson(Expression node) {
  var json = <String, Object?>{
    'o': node.offset,
    'e': node.end,
    'k': _kind(node),
    'type': node.staticType!.getDisplayString(),
  };
  if (node is InvocationExpression) {
    json['inv'] = node.staticInvokeType?.getDisplayString();
    json['targs'] = node.typeArgumentTypes
        ?.map((DartType t) => t.getDisplayString())
        .toList();
  }
  return json;
}

/// The element reference of [e] (see the file comment).
String? elRef(Element? e) {
  if (e == null) return null;
  if (e is SubstitutedElementImpl) e = e.baseElement;
  var local = localRef(e);
  if (local != null) return local;
  return ref(e);
}

/// The local kind of [e], or null when [e] is not a local element.
String? _localKind(Element e) => switch (e) {
  JoinPatternVariableElement() => 'joinPatternVariable',
  PatternVariableElement() => 'patternVariable',
  LocalVariableElement() => 'variable',
  LocalFunctionElement() => 'function',
  LabelElement() => 'label',
  _ => null,
};

/// `"local:<kind>:<name>@<offset>"` for a local element, else null.
String? localRef(Element e) {
  var kind = _localKind(e);
  if (kind == null) {
    if (e is FormalParameterElement) {
      kind = 'parameter';
    } else if (e is TypeParameterElement) {
      kind = 'typeParameter';
    } else {
      return null;
    }
    var current = e.enclosingElement;
    while (current != null && _localKind(current) == null) {
      current = current.enclosingElement;
    }
    if (current == null) return null;
  }
  var fragment = e.firstFragment;
  var offset = fragment.nameOffset ?? fragment.offset;
  return 'local:$kind:${e.name ?? ''}@$offset';
}

/// The substitution of a substituted member (see the file comment).
String? memberString(Element? e) {
  if (e is! SubstitutedElementImpl) return null;
  var base = e.baseElement;
  var entries = [
    for (var MapEntry(:key, :value) in e.substitution.map.entries)
      if (key.enclosingElement != base)
        '${key.name ?? ''}: ${value.getDisplayString()}',
  ]..sort();
  return entries.isEmpty ? null : entries.join(', ');
}
