// Extracts the AST node schema of the pinned analyzer for the Rust code
// generator (`tools/codegen/gen_ast.py`).
//
// Reads `package:analyzer/src/dart/ast/ast.dart` (the `@GenerateNodeImpl`
// annotations, the same input as `pkg/analyzer/tool/generators/
// ast_generator.dart`) and `package:analyzer/dart/ast/visitor.dart`
// (`GeneralizingAstVisitor`, in visitor.g.dart), and writes the schema as JSON.
//
// Usage (from tools/oracle):
//   dart run bin/ast_schema.dart > ../../crates/dartr_ast/schema/ast.json
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';
import 'package:analyzer/file_system/physical_file_system.dart';

final uriAst = Uri.parse('package:analyzer/src/dart/ast/ast.dart');
final uriToken = Uri.parse(
  'package:_fe_analyzer_shared/src/scanner/token.dart',
);

Future<void> main(List<String> args) async {
  var self = File(Platform.script.toFilePath()).absolute.path;
  var collection = AnalysisContextCollection(
    includedPaths: [self],
    resourceProvider: PhysicalResourceProvider.INSTANCE,
  );
  var session = collection.contextFor(self).currentSession;
  var lib = await session.getLibraryByUri(uriAst.toString());
  lib as LibraryElementResult;
  var library = lib.element;

  bool isAstNode(InterfaceElement e) =>
      e.library.uri == uriAst && e.name == 'AstNode';

  bool isNodeType(DartType t) =>
      t is InterfaceType &&
      (isAstNode(t.element) ||
          t.element.allSupertypes.any((s) => isAstNode(s.element)));

  bool isToken(DartType t) =>
      t is InterfaceType &&
      t.element.library.uri == uriToken &&
      t.element.name == 'Token';

  bool isTokenList(DartType t) =>
      t is InterfaceType &&
      t.element.name == 'List' &&
      t.typeArguments.length == 1 &&
      isToken(t.typeArguments.single);

  bool isNodeList(DartType t) =>
      t is InterfaceType &&
      t.element.library.uri == uriAst &&
      (t.element.name == 'NodeList' || t.element.name == 'NodeListImpl');

  String publicName(InterfaceElement e) {
    var n = e.name!;
    return n.endsWith('Impl') ? n.substring(0, n.length - 4) : n;
  }

  String typeCode(DartType t) {
    var q = t.nullabilitySuffix == NullabilitySuffix.question ? '?' : '';
    if (t is InterfaceType) {
      if (t.typeArguments.isEmpty) return '${t.element.name}$q';
      return '${t.element.name}<${t.typeArguments.map(typeCode).join(', ')}>$q';
    }
    return t.getDisplayString();
  }

  bool hasDoNotGenerate(Element e) => e.metadata.annotations.any((a) {
    var el = a.element;
    return el is ConstructorElement &&
        el.enclosingElement.library.uri == uriAst &&
        el.enclosingElement.name == 'DoNotGenerate';
  });

  // Public interfaces: every class/interface of ast.dart that is AstNode or a
  // subtype, without `Impl`.
  var interfaces = <String, Object?>{};
  for (var c in library.classes) {
    var name = c.name!;
    if (name.endsWith('Impl')) continue;
    if (!(isAstNode(c) || c.allSupertypes.any((s) => isAstNode(s.element)))) {
      continue;
    }
    interfaces[name] = {
      'supertypes': [
        for (var s in c.interfaces)
          if (isAstNode(s.element) ||
              s.element.allSupertypes.any((x) => isAstNode(x.element)))
            s.element.name,
      ],
      'all_supertypes': [
        for (var s in c.allSupertypes)
          if (isAstNode(s.element) ||
              s.element.allSupertypes.any((x) => isAstNode(x.element)))
            s.element.name,
      ],
    };
  }

  var nodes = <Object?>[];
  for (var c in library.classes) {
    var name = c.name!;
    if (!name.endsWith('Impl') || c.isAbstract || c.isSealed) continue;
    if (!c.allSupertypes.any((s) => isAstNode(s.element))) continue;
    if (name == 'NodeListImpl') continue;
    var interface = c.interfaces.isEmpty ? null : c.interfaces.last.element;
    var annotation = c.metadata.annotations
        .map((a) => a.computeConstantValue())
        .where((v) => v?.type?.element?.name == 'GenerateNodeImpl')
        .firstOrNull;
    var implChain = <String>[];
    for (var s = c.supertype; s != null; s = s.element.supertype) {
      if (s.element.name == 'Object' || s.element.name == 'SyntacticEntity') {
        break;
      }
      implChain.add(s.element.name!);
    }
    var doNotGenerate = <String>[];
    for (var m in [...c.methods, ...c.getters, ...c.setters, ...c.fields]) {
      if (hasDoNotGenerate(m)) doNotGenerate.add(m.lookupName ?? m.name!);
    }
    for (var m in c.constructors) {
      if (hasDoNotGenerate(m)) doNotGenerate.add('new');
    }
    var isAnnotated = c.allSupertypes.any(
      (s) => s.element.library.uri == uriAst && s.element.name == 'AnnotatedNode',
    );
    List<Object?>? properties;
    if (annotation != null) {
      properties = [];
      for (var entity in annotation.getField('childEntitiesOrder')!.toListValue()!) {
        var pname = entity.getField('name')!.toStringValue()!;
        var type = entity.getField('type')!.toTypeValue();
        if (type == null) {
          GetterElement? getter;
          for (var t in [interface!.thisType, ...interface.allSupertypes]) {
            getter = t.getGetter(pname);
            if (getter != null) break;
          }
          if (getter == null) throw StateError('$name.$pname');
          type = getter.returnType;
        }
        String kind;
        String? elementType;
        if (isToken(type)) {
          kind = 'token';
        } else if (isTokenList(type)) {
          kind = 'token_list';
        } else if (isNodeType(type)) {
          kind = 'node';
          elementType = publicName((type as InterfaceType).element);
        } else if (isNodeList(type)) {
          kind = 'node_list';
          elementType = publicName(
            ((type as InterfaceType).typeArguments.single as InterfaceType)
                .element,
          );
        } else {
          kind = 'other';
        }
        properties.add({
          'name': pname,
          'kind': kind,
          'nullable': type.nullabilitySuffix == NullabilitySuffix.question,
          'type': ?elementType,
          if (kind == 'other') 'dart_type': typeCode(type),
          if (entity.getField('isSuper')!.toBoolValue()!) 'is_super': true,
          if (entity.getField('tokenGroupId')!.toIntValue() case var g?)
            'token_group': g,
          if (entity.getField('isInValueExpressionSlot')!.toBoolValue()!)
            'value_slot': true,
        });
      }
    }
    nodes.add({
      'name': interface == null ? publicName(c) : interface.name,
      'impl': name,
      'impl_supertypes': implChain,
      'annotated': isAnnotated,
      'generated': annotation != null,
      if (doNotGenerate.isNotEmpty) 'do_not_generate': doNotGenerate,
      'properties': ?properties,
    });
  }

  // GeneralizingAstVisitor: visitX(X node) => visitY(node).
  var visitorPath = library.session.uriConverter.uriToPath(
    Uri.parse('package:analyzer/dart/ast/visitor.g.dart'),
  )!;
  var visitorUnit = parseString(
    content: File(visitorPath).readAsStringSync(),
  ).unit;
  var generalizing = <String, String?>{};
  for (var d in visitorUnit.declarations) {
    if (d is ClassDeclaration && d.namePart.typeName.lexeme == 'GeneralizingAstVisitor') {
      for (var m in (d.body as BlockClassBody).members) {
        if (m is! MethodDeclaration) continue;
        var mname = m.name.lexeme;
        if (!mname.startsWith('visit')) continue;
        var param = m.parameters?.parameters.firstOrNull;
        String? typeName;
        if (param is RegularFormalParameter) {
          typeName = (param.type as NamedType?)?.name.lexeme;
        }
        typeName ??= mname.substring(5);
        String? target;
        var body = m.body;
        Expression? e;
        if (body is ExpressionFunctionBody) {
          e = body.expression;
        } else if (body is BlockFunctionBody) {
          var s = body.block.statements.firstOrNull;
          if (s is ExpressionStatement) e = s.expression;
          if (s is ReturnStatement) e = s.expression;
        }
        if (e is MethodInvocation) {
          var n = e.methodName.name;
          if (n.startsWith('visit')) target = n.substring(5);
          if (n == 'visitChildren') target = null;
        }
        generalizing[typeName] = target;
      }
    }
  }

  stdout.writeln(
    const JsonEncoder.withIndent('  ').convert({
      'sdk': 'pinned analyzer, third_party/dart-sdk (tag 3.13.3)',
      'nodes': nodes,
      'interfaces': interfaces,
      'generalizing': generalizing,
    }),
  );
}
