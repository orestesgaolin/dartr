// Oracle mode `elements`: the element model of each library (see
// docs/design/semantics.md §5.1). `dartr dump elements` writes the same
// format.
//
// One JSON line per input path. The path must be the defining unit of a
// library; for a part file (or a file that is not a library) the line is
// `{"path":p,"error":"<result.runtimeType>"}`. An exception gives
// `{"path":p,"error":"<exception type>"}`.
//
// Line format (keys in this order):
//   {"path", "uri", "lang", "units", "imports", "exports", "exportNamespace",
//    "elements"}
//
// Element references R(e) are `"<libraryUri>::<path>"`. The path is the names
// of the enclosing elements, from the top-level element down, joined by `.`.
// Unnamed constructors are `new`, setters get `=` appended, an unnamed
// extension is `<unnamed>`. A null element is JSON null; a multiply-defined
// element is `"<multiply-defined>::<name>"`. An element without a library
// (for example `dynamic` in the export namespace of `dart:core`) uses
// `<no-library>` in place of the library URI.
//
// Types are `type.getDisplayString()`; a null type is JSON null.
//
// An input that starts with `dart:` (for example `dart:core`) is a library
// URI: it is resolved with `getLibraryByUri` in a context rooted at the SDK,
// and the output `"path"` is the input string itself.
//
// `"inf"` marks inferred types: for a variable `hasImplicitType`; for an
// executable `hasImplicitReturnType` or a formal parameter with
// `hasImplicitType`; for a getter or setter with `isOriginVariable` (induced
// by a variable) the `hasImplicitType` of that variable; for the `value`
// parameter of such a setter also the `hasImplicitType` of the variable.
//
// A method has `"typeInferenceError":"overrideNoCombinedSuperSignature"` and
// `"candidateSignatures"` when override inference found no combined
// signature, and `"opEqParamFromObject":true` when
// `isOperatorEqualWithParameterTypeFromObject`; the keys are missing
// otherwise.
//
// With `--with-const` (`dumpElements(withConst: true)`), a top-level
// variable or field (enum constants included) with `isConst` gets the key
// `"const"` after its other keys: `computeConstantValue()` written with
// `DartObjectImpl.toString()`, or null when there is no value (an invalid
// constant). Without the flag the key is missing.
import 'dart:convert';
import 'dart:io';

import 'package:_fe_analyzer_shared/src/types/shared_type.dart' as shared;
import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/type.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/element/element.dart';
// ignore: implementation_imports
import 'package:analyzer/src/error/inference_error.dart';
import 'package:path/path.dart' as path;

/// The fixed flag list: `(getter, flag name)`. For each entry, when the
/// element has the getter (checked by element type in [_flagValue]) and it
/// is `true`, the flag name is added to `"f"`. `"f"` is sorted by code unit
/// order. Applies to elements and formal parameters, not to type parameters.
///
/// `dartr dump elements` copies this list. Notes on 3.13.3:
/// - `isSynthetic` is not in the list: in 3.13.3 only `LibraryElement` and
///   `LibraryImport` have it, and neither is written as an element. Synthetic
///   elements are marked by the `isOrigin*` flags.
/// - `isAsynchronous` and `isGenerator` exist only on `ExecutableFragment`;
///   they are taken from the first fragment of an executable element.
/// - `FieldFormalParameterElement` / `SuperFormalParameterElement` are type
///   tests, not getters.
const flags = <(String, String)>[
  ('isAbstract', 'abstract'),
  ('isBase', 'base'),
  ('isFinal', 'final'),
  ('isInterface', 'interface'),
  ('isSealed', 'sealed'),
  ('isMixinClass', 'mixinClass'),
  ('isMixinApplication', 'mixinApplication'),
  ('isSimplyBounded', 'simplyBounded'),
  ('hasNonFinalField', 'hasNonFinalField'),
  ('isConst', 'const'),
  ('isFactory', 'factory'),
  ('isPrimary', 'primary'),
  ('isStatic', 'static'),
  ('isExternal', 'external'),
  ('isLate', 'late'),
  ('isCovariant', 'covariant'),
  ('isPromotable', 'promotable'),
  ('isEnumConstant', 'enumConstant'),
  ('isAsynchronous', 'async'),
  ('isGenerator', 'generator'),
  ('isOperator', 'operator'),
  ('isExtensionTypeMember', 'extensionTypeMember'),
  ('hasInitializer', 'hasInitializer'),
  ('hasDefaultValue', 'hasDefaultValue'),
  ('isDeclaring', 'declaring'),
  ('FieldFormalParameterElement', 'fieldFormal'),
  ('SuperFormalParameterElement', 'superFormal'),
  ('isOriginImplicitDefault', 'originImplicitDefault'),
  ('isOriginMixinApplication', 'originMixinApplication'),
  ('isOriginEnumValues', 'originEnumValues'),
  ('isOriginGetterSetter', 'originGetterSetter'),
  ('isOriginVariable', 'originVariable'),
  ('isOriginInterface', 'originInterface'),
  ('isOriginLoadLibrary', 'originLoadLibrary'),
  ('isOriginDeclaringFormalParameter', 'originDeclaringFormalParameter'),
  ('isOriginExtensionTypeRecovery', 'originExtensionTypeRecovery'),
  (
    'isOriginExtensionTypeRecoveryRepresentation',
    'originExtensionTypeRecoveryRepresentation',
  ),
];

/// The value of the flag [getter] of [e]; `false` when [e] does not have it.
bool _flagValue(Element e, String getter) => switch (getter) {
  'isAbstract' => switch (e) {
    ClassElement e => e.isAbstract,
    ExecutableElement e => e.isAbstract,
    FieldElement e => e.isAbstract,
    _ => false,
  },
  'isBase' => switch (e) {
    ClassElement e => e.isBase,
    MixinElement e => e.isBase,
    _ => false,
  },
  'isFinal' => switch (e) {
    ClassElement e => e.isFinal,
    VariableElement e => e.isFinal,
    _ => false,
  },
  'isInterface' => e is ClassElement && e.isInterface,
  'isSealed' => e is ClassElement && e.isSealed,
  'isMixinClass' => e is ClassElement && e.isMixinClass,
  'isMixinApplication' => e is ClassElement && e.isMixinApplication,
  'isSimplyBounded' => e is TypeParameterizedElement && e.isSimplyBounded,
  'hasNonFinalField' => e is ClassElement && e.hasNonFinalField,
  'isConst' => switch (e) {
    ConstructorElement e => e.isConst,
    VariableElement e => e.isConst,
    _ => false,
  },
  'isFactory' => e is ConstructorElement && e.isFactory,
  'isPrimary' => e is ConstructorElement && e.isPrimary,
  'isStatic' => switch (e) {
    ExecutableElement e => e.isStatic,
    VariableElement e => e.isStatic,
    _ => false,
  },
  'isExternal' => switch (e) {
    ExecutableElement e => e.isExternal,
    FieldElement e => e.isExternal,
    TopLevelVariableElement e => e.isExternal,
    _ => false,
  },
  'isLate' => e is VariableElement && e.isLate,
  'isCovariant' => switch (e) {
    FieldElement e => e.isCovariant,
    FormalParameterElement e => e.isCovariant,
    _ => false,
  },
  'isPromotable' => e is FieldElement && e.isPromotable,
  'isEnumConstant' => e is FieldElement && e.isEnumConstant,
  'isAsynchronous' => e is ExecutableElement && e.firstFragment.isAsynchronous,
  'isGenerator' => e is ExecutableElement && e.firstFragment.isGenerator,
  'isOperator' => e is MethodElement && e.isOperator,
  'isExtensionTypeMember' => e is ExecutableElement && e.isExtensionTypeMember,
  'hasInitializer' => e is PropertyInducingElement && e.hasInitializer,
  'hasDefaultValue' => e is FormalParameterElement && e.hasDefaultValue,
  'isDeclaring' => e is FieldFormalParameterElement && e.isDeclaring,
  'FieldFormalParameterElement' => e is FieldFormalParameterElement,
  'SuperFormalParameterElement' => e is SuperFormalParameterElement,
  'isOriginImplicitDefault' =>
    e is ConstructorElement && e.isOriginImplicitDefault,
  'isOriginMixinApplication' =>
    e is ConstructorElement && e.isOriginMixinApplication,
  'isOriginEnumValues' => e is FieldElement && e.isOriginEnumValues,
  'isOriginGetterSetter' =>
    e is PropertyInducingElement && e.isOriginGetterSetter,
  'isOriginVariable' => e is PropertyAccessorElement && e.isOriginVariable,
  'isOriginInterface' => switch (e) {
    MethodElement e => e.isOriginInterface,
    PropertyAccessorElement e => e.isOriginInterface,
    _ => false,
  },
  'isOriginLoadLibrary' =>
    e is TopLevelFunctionElement && e.isOriginLoadLibrary,
  'isOriginDeclaringFormalParameter' =>
    e is FieldElement && e.isOriginDeclaringFormalParameter,
  'isOriginExtensionTypeRecovery' =>
    e is ConstructorElement && e.isOriginExtensionTypeRecovery,
  'isOriginExtensionTypeRecoveryRepresentation' =>
    e is FieldElement && e.isOriginExtensionTypeRecoveryRepresentation,
  _ => throw ArgumentError('unknown flag getter: $getter'),
};

List<String> flagsOf(Element e) => [
  for (var (getter, name) in flags)
    if (_flagValue(e, getter)) name,
]..sort();

/// Set by [dumpElements]: add `"const"` to const variables.
bool _withConst = false;

Future<void> dumpElements(List<String> inputs, {bool withConst = false}) async {
  _withConst = withConst;
  var sdkPath = oracleSdkPath();
  // `AnalysisContextCollection` throws for a path that is not absolute and
  // normalized (for example `/a//b.dart`), which would end the whole run.
  // Such a path is left out here; `contextFor` throws the same
  // `ArgumentError` for it below, which gives a per-file error line.
  var paths = inputs
      .where((p) => !p.startsWith('dart:') && p == path.normalize(p))
      .toList();
  var collection = AnalysisContextCollection(
    includedPaths: paths,
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    sdkPath: sdkPath,
  );
  // `dart:` URIs are resolved in one context rooted at an empty temporary
  // folder, so that their output does not depend on the other inputs. (A
  // context rooted at the SDK folder itself cannot resolve `dart:cli`,
  // `dart:mirrors`, `dart:_js_helper`, ...: `CannotResolveUriResult`.)
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
          json = libraryJson(p, result.element);
        } else {
          json = {'path': p, 'error': result.runtimeType.toString()};
        }
      } else {
        var context = collection.contextFor(p);
        var result = await context.currentSession.getResolvedLibrary(p);
        if (result is ResolvedLibraryResult) {
          json = libraryJson(p, result.element);
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

/// The SDK folder: the SDK of the running VM (`dart run`), else the SDK of
/// the `dart` executable on `PATH` (the AOT-compiled oracle in
/// `target/oracle/oracle` is not inside an SDK). Same lookup as
/// `dartr_project::sdk::find_sdk_path`, including the Flutter layout
/// (`bin/dart` wrapper, SDK in `bin/cache/dart-sdk`).
String? oracleSdkPath() {
  bool isSdk(String path) =>
      File('$path/version').existsSync() && Directory('$path/lib').existsSync();
  var vmSdk = File(Platform.resolvedExecutable).parent.parent.path;
  if (isSdk(vmSdk)) return vmSdk;
  var separator = Platform.isWindows ? ';' : ':';
  for (var dir in (Platform.environment['PATH'] ?? '').split(separator)) {
    var candidate = File('$dir/dart');
    if (dir.isEmpty || !candidate.existsSync()) continue;
    var bin = File(candidate.resolveSymbolicLinksSync()).parent;
    var root = bin.parent.path;
    if (isSdk(root)) return root;
    var flutterSdk = '${bin.path}/cache/dart-sdk';
    if (isSdk(flutterSdk))
      return Directory(flutterSdk).resolveSymbolicLinksSync();
  }
  return null;
}

Map<String, Object?> libraryJson(String path, LibraryElement library) {
  var fragments = library.fragments;
  var unitIndex = <LibraryFragment, int>{
    for (var i = 0; i < fragments.length; i++) fragments[i]: i,
  };
  var lang = library.languageVersion.effective;

  var imports = <Object?>[];
  var exports = <Object?>[];
  for (var fragment in fragments) {
    for (var import in fragment.libraryImports) {
      var prefix = import.prefix;
      imports.add({
        'uri': directiveUri(import.importedLibrary, import.uri),
        'prefix': prefix?.name,
        'show': [
          for (var c in import.combinators)
            if (c is ShowElementCombinator) ...c.shownNames,
        ],
        'hide': [
          for (var c in import.combinators)
            if (c is HideElementCombinator) ...c.hiddenNames,
        ],
        'deferred': prefix?.isDeferred ?? false,
        'synthetic': import.isSynthetic,
      });
    }
    for (var export in fragment.libraryExports) {
      exports.add(directiveUri(export.exportedLibrary, export.uri));
    }
  }

  var definedNames = library.exportNamespace.definedNames2;
  var names = definedNames.keys.toList()..sort();

  // Top-level elements, ordered by (unit, offset, kind rank, list index).
  var topLevel = <Element>[
    ...library.classes,
    ...library.mixins,
    ...library.enums,
    ...library.extensionTypes,
    ...library.extensions,
    ...library.typeAliases,
    ...library.topLevelFunctions,
    ...library.topLevelVariables,
    ...library.getters,
    ...library.setters,
  ];
  int unitOf(Element e) => unitIndex[e.firstFragment.libraryFragment] ?? -1;
  var sorted = sortElements(topLevel, unitOf);

  return {
    'path': path,
    'uri': library.uri.toString(),
    'lang': '${lang.major}.${lang.minor}',
    'units': [
      for (var i = 0; i < fragments.length; i++)
        if (i == 0)
          {'path': fragments[i].source.fullName}
        else
          {'path': fragments[i].source.fullName, 'part': true},
    ],
    'imports': imports,
    'exports': exports,
    'exportNamespace': {for (var n in names) n: ref(definedNames[n])},
    'elements': [for (var e in sorted) elementJson(e, unit: unitOf(e))],
  };
}

Object? directiveUri(LibraryElement? library, DirectiveUri uri) {
  if (library != null) return library.uri.toString();
  if (uri is DirectiveUriWithRelativeUriString) return uri.relativeUriString;
  return null;
}

/// The kind name and the kind rank (for ties at the same offset).
(String, int) kindOf(Element e) => switch (e) {
  ClassElement() => ('class', 0),
  MixinElement() => ('mixin', 1),
  EnumElement() => ('enum', 2),
  ExtensionTypeElement() => ('extensionType', 3),
  ExtensionElement() => ('extension', 4),
  TypeAliasElement() => ('typeAlias', 5),
  TopLevelFunctionElement() => ('function', 6),
  TopLevelVariableElement() => ('topVar', 7),
  FieldElement() => ('field', 10),
  GetterElement() => ('getter', 11),
  SetterElement() => ('setter', 12),
  ConstructorElement() => ('ctor', 13),
  MethodElement() => ('method', 14),
  _ => throw ArgumentError('unexpected element: ${e.runtimeType}'),
};

/// Sorts [elements] by (unit, firstFragment.offset, kind rank, list index).
List<Element> sortElements(List<Element> elements, int Function(Element) unit) {
  var keyed = [
    for (var i = 0; i < elements.length; i++)
      (
        unit(elements[i]),
        elements[i].firstFragment.offset,
        kindOf(elements[i]).$2,
        i,
        elements[i],
      ),
  ];
  keyed.sort((a, b) {
    var c = a.$1.compareTo(b.$1);
    if (c != 0) return c;
    c = a.$2.compareTo(b.$2);
    if (c != 0) return c;
    c = a.$3.compareTo(b.$3);
    if (c != 0) return c;
    return a.$4.compareTo(b.$4);
  });
  return [for (var k in keyed) k.$5];
}

String? typeStr(DartType? t) => t?.getDisplayString();

/// R(e), see the file comment.
String? ref(Element? e) {
  if (e == null) return null;
  if (e is MultiplyDefinedElement) return '<multiply-defined>::${e.name}';
  var names = <String>[];
  Element? current = e;
  while (current != null && current is! LibraryElement) {
    names.add(refName(current));
    current = current.enclosingElement;
  }
  var uri = e.library?.uri.toString() ?? '<no-library>';
  return '$uri::${names.reversed.join('.')}';
}

String refName(Element e) {
  var name = e.name;
  if (e is ConstructorElement && (name == null || name.isEmpty)) name = 'new';
  name ??= '<unnamed>';
  if (e is SetterElement) name = '$name=';
  return name;
}

Map<String, Object?> elementJson(Element e, {int? unit}) {
  var (kind, _) = kindOf(e);
  var json = <String, Object?>{'k': kind, 'n': e.name};
  if (unit != null) json['u'] = unit;
  var nameOffset = e.firstFragment.nameOffset;
  if (nameOffset != null) json['o'] = nameOffset;
  json['f'] = flagsOf(e);

  switch (e) {
    case InterfaceElement():
      json['tp'] = typeParametersJson(e.typeParameters);
      json['super'] = typeStr(e.supertype);
      json['mixins'] = e.mixins.map(typeStr).toList();
      json['interfaces'] = e.interfaces.map(typeStr).toList();
      if (e is MixinElement) {
        json['on'] = e.superclassConstraints.map(typeStr).toList();
        json['superInvoked'] =
            (e.firstFragment as MixinFragmentImpl).superInvokedNames;
      }
      if (e is ExtensionTypeElement) {
        json['rep'] = typeStr(e.representation.type);
        json['primaryCtor'] = ref(e.primaryConstructor);
      }
      json['members'] = membersJson(e);
    case ExtensionElement():
      json['tp'] = typeParametersJson(e.typeParameters);
      json['on'] = typeStr(e.extendedType);
      json['members'] = membersJson(e);
    case TypeAliasElement():
      json['tp'] = typeParametersJson(e.typeParameters);
      json['aliased'] = typeStr(e.aliasedType);
    case PropertyInducingElement():
      json['type'] = typeStr(e.type);
      json['inf'] = e.hasImplicitType;
      var error = (e as PropertyInducingElementImpl).typeInferenceError;
      json['typeInferenceError'] = switch (error) {
        null => null,
        TopLevelInferenceErrorDependencyCycle() =>
          TopLevelInferenceErrorKind.dependencyCycle.name,
        TopLevelInferenceErrorNoCombinedSuperSignature() =>
          TopLevelInferenceErrorKind.overrideNoCombinedSuperSignature.name,
      };
      if (_withConst && e.isConst) {
        json['const'] = e.computeConstantValue()?.toString();
      }
    case PropertyAccessorElement():
      json['type'] = typeStr(e.type);
      // A synthetic accessor of a variable has the type of the variable.
      json['inf'] = e.isOriginVariable
          ? e.variable.hasImplicitType
          : executableInferred(e);
      json['var'] = ref(e.variable);
      if (e is SetterElement) json['params'] = parametersJson(e);
    case ConstructorElement():
      json['type'] = typeStr(e.type);
      json['inf'] = executableInferred(e);
      json['params'] = parametersJson(e);
      json['redirected'] = ref(e.redirectedConstructor?.baseElement);
      json['superCtor'] = ref(e.superConstructor?.baseElement);
    case ExecutableElement():
      json['type'] = typeStr(e.type);
      json['inf'] = executableInferred(e);
      json['tp'] = typeParametersJson(e.typeParameters);
      json['params'] = parametersJson(e);
      if (e is MethodElementImpl) {
        // Override inference (InstanceMemberInferrer): written only when
        // set, so that other methods keep their shape.
        if (e.typeInferenceError
            case TopLevelInferenceErrorNoCombinedSuperSignature error) {
          json['typeInferenceError'] =
              TopLevelInferenceErrorKind.overrideNoCombinedSuperSignature.name;
          json['candidateSignatures'] = error.candidateSignatures;
        }
        if (e.isOperatorEqualWithParameterTypeFromObject) {
          json['opEqParamFromObject'] = true;
        }
      }
    default:
      throw ArgumentError('unexpected element: ${e.runtimeType}');
  }
  return json;
}

bool executableInferred(ExecutableElement e) =>
    e.hasImplicitReturnType || e.formalParameters.any((p) => p.hasImplicitType);

List<Object?> membersJson(InstanceElement e) {
  var members = <Element>[
    ...e.fields,
    ...e.getters,
    ...e.setters,
    if (e is InterfaceElement) ...e.constructors,
    ...e.methods,
  ];
  return [for (var m in sortElements(members, (_) => 0)) elementJson(m)];
}

List<Object?> typeParametersJson(List<TypeParameterElement> typeParameters) => [
  for (var tp in typeParameters)
    {
      'n': tp.name,
      'bound': typeStr(tp.bound),
      'default': typeStr((tp as TypeParameterElementImpl).defaultType),
      'variance': tp.isLegacyCovariant ? null : varianceName(tp.variance),
    },
];

String varianceName(shared.Variance v) => switch (v) {
  shared.Variance.contravariant => 'in',
  shared.Variance.covariant => 'out',
  shared.Variance.invariant => 'inout',
  shared.Variance.unrelated => 'unrelated',
};

List<Object?> parametersJson(ExecutableElement e) => [
  for (var p in e.formalParameters)
    {
      'n': p.name,
      'kind': p.isRequiredPositional
          ? 'requiredPositional'
          : p.isOptionalPositional
          ? 'optionalPositional'
          : p.isRequiredNamed
          ? 'requiredNamed'
          : 'optionalNamed',
      'type': typeStr(p.type),
      'inf': parameterInferred(e, p),
      'f': flagsOf(p),
      'default': p.defaultValueCode,
    },
];

/// `"inf"` of a formal parameter: `hasImplicitType`; for the `value`
/// parameter of a synthetic setter of a variable (`isOriginVariable`), the
/// `hasImplicitType` of that variable (its type is the variable type, so it
/// is inferred exactly when the variable type is).
bool parameterInferred(ExecutableElement e, FormalParameterElement p) {
  if (e is SetterElement && e.isOriginVariable) {
    return e.variable.hasImplicitType;
  }
  return p.hasImplicitType;
}
