// Extracts the element model schema of the pinned analyzer for the Rust code
// generator (`tools/codegen/gen_element.py`).
//
// Reads `package:analyzer/src/dart/element/element.dart` and writes, for each
// `*ElementImpl` / `*FragmentImpl` class (and the directive classes used by
// them): the superclass, the instance fields declared in the class (name,
// declared type, `late`, `final`, initializer), and the flag enums of the
// generated flag storage (`_ElementStorageFlag`, `_FragmentStorageFlag`).
//
// Usage (from tools/oracle):
//   dart run bin/element_schema.dart > ../../crates/dartr_element/schema/element.json
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';
import 'package:analyzer/file_system/physical_file_system.dart';

final uriElement = Uri.parse('package:analyzer/src/dart/element/element.dart');

/// Classes that are not `*ElementImpl` / `*FragmentImpl` but hold element
/// data that the Rust model keeps.
const extraClasses = {
  'ElementAnnotationImpl',
  'MetadataImpl',
  'ElementDirectiveImpl',
  'LibraryImportImpl',
  'LibraryExportImpl',
  'PartIncludeImpl',
  'DirectiveUriImpl',
  'DirectiveUriWithRelativeUriStringImpl',
  'DirectiveUriWithRelativeUriImpl',
  'DirectiveUriWithSourceImpl',
  'DirectiveUriWithLibraryImpl',
  'DirectiveUriWithUnitImpl',
  'HideElementCombinatorImpl',
  'ShowElementCombinatorImpl',
  'ConstantInitializerImpl',
};

Future<void> main(List<String> args) async {
  var self = File(Platform.script.toFilePath()).absolute.path;
  var collection = AnalysisContextCollection(
    includedPaths: [self],
    resourceProvider: PhysicalResourceProvider.INSTANCE,
  );
  var session = collection.contextFor(self).currentSession;
  var lib = await session.getLibraryByUri(uriElement.toString());
  lib as LibraryElementResult;
  var library = lib.element;
  var path = library.firstFragment.source.fullName;
  var unitResult = await session.getResolvedUnit(path) as ResolvedUnitResult;
  var lineInfo = unitResult.lineInfo;

  String typeCode(DartType t) {
    var q = t.nullabilitySuffix == NullabilitySuffix.question ? '?' : '';
    if (t is InterfaceType) {
      if (t.typeArguments.isEmpty) return '${t.element.name}$q';
      return '${t.element.name}<${t.typeArguments.map(typeCode).join(', ')}>$q';
    }
    return t.getDisplayString();
  }

  bool selected(InterfaceElement e) {
    var n = e.name!;
    return n.endsWith('ElementImpl') ||
        n.endsWith('FragmentImpl') ||
        extraClasses.contains(n);
  }

  // Field declarations by name, for `late` and initializers.
  var declarations = <String, Map<String, VariableDeclaration>>{};
  for (var d in unitResult.unit.declarations) {
    if (d is ClassDeclaration) {
      var map = declarations[d.namePart.typeName.lexeme] = {};
      for (var m in d.body.members) {
        if (m is FieldDeclaration && !m.isStatic) {
          for (var v in m.fields.variables) {
            map[v.name.lexeme] = v;
          }
        }
      }
    }
  }

  var classes = <Object?>[];
  for (var e in library.children.whereType<InterfaceElement>()) {
    if (e is! ClassElement && e is! MixinElement) continue;
    if (!selected(e)) continue;
    var name = e.name!;
    var fields = <Object?>[];
    for (var v in (declarations[name] ?? {}).values) {
      var f = v.declaredFragment!.element as FieldElement;
      var list = v.parent as VariableDeclarationList;
      fields.add({
        'name': f.name,
        'type': typeCode(f.type),
        'final': list.isFinal,
        'late': list.isLate,
        'init': v.initializer?.toSource(),
        'line': lineInfo.getLocation(v.name.offset).lineNumber,
      });
    }
    var flags = <String>[];
    for (var a in e.metadata.annotations) {
      var src = a.toSource();
      if (src.startsWith('@GenerateElementFlags')) {
        flags.add(src);
      }
    }
    classes.add({
      'name': name,
      'kind': e is MixinElement ? 'mixin' : 'class',
      'abstract': e is ClassElement && e.isAbstract,
      'sealed': e is ClassElement && e.isSealed,
      'super': e.supertype?.element.name,
      'mixins': e.mixins.map((m) => m.element.name).toList(),
      'interfaces': e.interfaces.map((m) => m.element.name).toList(),
      'line': lineInfo.getLocation(e.firstFragment.nameOffset!).lineNumber,
      'flagEnums': flags,
      'fields': fields,
    });
  }

  // The flag enums.
  var enums = <String, Object?>{};
  for (var e in library.children.whereType<EnumElement>()) {
    var n = e.name!;
    if (n == '_ElementStorageFlag' ||
        n == '_FragmentStorageFlag' ||
        RegExp(r'^_\w+Flags$').hasMatch(n)) {
      enums[n] = [
        for (var c in e.constants)
          {
            'name': c.name,
            'fragment': c.computeConstantValue()?.getField('fragment')?.toBoolValue(),
            'element': c
                .computeConstantValue()
                ?.getField('element')
                ?.getField('_name')
                ?.toStringValue(),
          },
      ];
    }
  }

  stdout.writeln(
    const JsonEncoder.withIndent(' ').convert({
      'source': 'pkg/analyzer/lib/src/dart/element/element.dart',
      'classes': classes,
      'enums': enums,
    }),
  );
  exit(0);
}
